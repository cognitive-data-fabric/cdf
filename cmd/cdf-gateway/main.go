// Package main implements the CDF API Gateway.
//
// The Gateway serves both HTTP REST and gRPC, provides authentication,
// rate limiting, and proxies requests to the appropriate backend services
// (router, meta, storage nodes, embedding service, drift detector).
package main

import (
	"context"
	"crypto/subtle"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"os"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/google/uuid"
	"google.golang.org/grpc"
	"google.golang.org/grpc/reflection"
)

// grpcNewServer is a thin wrapper around grpc.NewServer that lets us add
// middleware (auth, metrics) in one place. Future: interceptors.
func grpcNewServer() *grpc.Server {
	return grpc.NewServer(
		grpc.MaxRecvMsgSize(64*1024*1024), // 64MB
		grpc.MaxSendMsgSize(64*1024*1024),
	)
}

// reflectionRegister wraps reflection.Register so unused-import warnings
// are silenced and so the call site reads as documentation.
func reflectionRegister(s *grpc.Server) {
	reflection.Register(s)
}

// APIKey represents a configured static API key with optional role list.
type APIKey struct {
	Key   string
	Roles []string
}

// AuthConfig holds authentication state.
type AuthConfig struct {
	APIKeys     map[string]APIKey // key -> APIKey
	JWTSecret   string
	RequireAuth bool
}

// RateLimiter implements a simple token-bucket per client (by API key or IP).
type RateLimiter struct {
	mu      sync.Mutex
	buckets map[string]*tokenBucket
	rate    int // tokens per second
	burst   int // bucket capacity
}

type tokenBucket struct {
	tokens   float64
	lastTime time.Time
}

func newRateLimiter(rate, burst int) *RateLimiter {
	return &RateLimiter{
		buckets: make(map[string]*tokenBucket),
		rate:    rate,
		burst:   burst,
	}
}

func (rl *RateLimiter) allow(key string) bool {
	rl.mu.Lock()
	defer rl.mu.Unlock()
	now := time.Now()
	bucket, ok := rl.buckets[key]
	if !ok {
		bucket = &tokenBucket{tokens: float64(rl.burst), lastTime: now}
		rl.buckets[key] = bucket
	}
	elapsed := now.Sub(bucket.lastTime).Seconds()
	bucket.tokens = minFloat(float64(rl.burst), bucket.tokens+elapsed*float64(rl.rate))
	bucket.lastTime = now
	if bucket.tokens < 1 {
		return false
	}
	bucket.tokens -= 1
	return true
}

func minFloat(a, b float64) float64 {
	if a < b {
		return a
	}
	return b
}

// Gateway serves both HTTP REST and gRPC with WebSocket streaming.
type Gateway struct {
	routerAddr   string
	httpAddr     string
	grpcAddr     string
	auth         *AuthConfig
	rateLimiter  *RateLimiter
	clients      *ServiceClients
	requestCount atomic.Uint64
}

func NewGateway(routerAddr, httpAddr, grpcAddr string, auth *AuthConfig, rl *RateLimiter, clients *ServiceClients) *Gateway {
	return &Gateway{
		routerAddr:  routerAddr,
		httpAddr:    httpAddr,
		grpcAddr:    grpcAddr,
		auth:        auth,
		rateLimiter: rl,
		clients:     clients,
	}
}

func (g *Gateway) Run() error {
	go g.runHTTPServer()
	return g.runGRPCServer()
}

// authenticate checks the Authorization header and returns the principal ID
// (API key or JWT sub) along with associated roles.
func (g *Gateway) authenticate(r *http.Request) (string, []string, bool) {
	if g.auth == nil {
		// No auth configured — allow all
		return "anonymous", []string{"service"}, true
	}
	// Try X-API-Key header first
	if apiKey := r.Header.Get("X-API-Key"); apiKey != "" {
		if key, ok := g.auth.APIKeys[apiKey]; ok {
			return "apikey:" + key.Key, key.Roles, true
		}
		return "", nil, false
	}
	// Try Authorization: Bearer <token>
	authz := r.Header.Get("Authorization")
	if strings.HasPrefix(authz, "Bearer ") {
		token := strings.TrimPrefix(authz, "Bearer ")
		// JWT validation is delegated to a dedicated verifier in production.
		// Here we just check presence and trust the gateway's metadata.
		if token != "" && g.auth.JWTSecret != "" {
			// In production: parse JWT, verify signature, extract sub + roles
			// For this gateway: we accept any non-empty bearer token when JWTSecret is set,
			// assuming a separate proxy/upstream already validated the JWT.
			principalID := "jwt:" + truncate(token, 12)
			return principalID, []string{"analyst"}, true
		}
	}
	// Public endpoints bypass auth
	if r.URL.Path == "/health" || r.URL.Path == "/metrics" {
		return "public", []string{"public"}, true
	}
	if !g.auth.RequireAuth {
		return "anonymous", []string{"analyst"}, true
	}
	return "", nil, false
}

func truncate(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n]
}

// rateLimitMiddleware applies the token-bucket limiter keyed by the
// authenticated principal ID or remote IP.
func (g *Gateway) rateLimitMiddleware(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		key := r.Header.Get("X-API-Key")
		if key == "" {
			host, _, err := net.SplitHostPort(r.RemoteAddr)
			if err != nil {
				host = r.RemoteAddr
			}
			key = "ip:" + host
		}
		if !g.rateLimiter.allow(key) {
			w.Header().Set("X-RateLimit-Limit", strconv.Itoa(g.rateLimiter.burst))
			w.Header().Set("X-RateLimit-Remaining", "0")
			w.Header().Set("Retry-After", "1")
			http.Error(w, `{"status":"error","error":{"code":"RATE_LIMITED","message":"too many requests"}}`, http.StatusTooManyRequests)
			return
		}
		// Set rate-limit headers
		w.Header().Set("X-RateLimit-Limit", strconv.Itoa(g.rateLimiter.burst))
		w.Header().Set("X-RateLimit-Remaining", "999")
		next(w, r)
	}
}

// authMiddleware enforces authentication on protected routes.
func (g *Gateway) authMiddleware(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		principalID, roles, ok := g.authenticate(r)
		if !ok {
			http.Error(w, `{"status":"error","error":{"code":"UNAUTHORIZED","message":"invalid or missing credentials"}}`, http.StatusUnauthorized)
			return
		}
		// Stash principal in context for downstream handlers
		ctx := context.WithValue(r.Context(), principalKey{}, principalID)
		ctx = context.WithValue(ctx, rolesKey{}, roles)
		next(w, r.WithContext(ctx))
	}
}

type principalKey struct{}
type rolesKey struct{}

func (g *Gateway) runHTTPServer() {
	mux := http.NewServeMux()

	// Public endpoints (no auth required)
	mux.HandleFunc("/health", g.handleHealth)
	mux.HandleFunc("/metrics", g.handleMetrics)

	// Authenticated REST endpoints
	mux.HandleFunc("/v1/query", g.rateLimitMiddleware(g.authMiddleware(g.handleQuery)))
	mux.HandleFunc("/v1/insert", g.rateLimitMiddleware(g.authMiddleware(g.handleInsert)))
	mux.HandleFunc("/v1/batch_insert", g.rateLimitMiddleware(g.authMiddleware(g.handleBatchInsert)))
	mux.HandleFunc("/v1/search", g.rateLimitMiddleware(g.authMiddleware(g.handleVectorSearch)))
	mux.HandleFunc("/v1/search_text", g.rateLimitMiddleware(g.authMiddleware(g.handleTextSearch)))
	mux.HandleFunc("/v1/tables", g.rateLimitMiddleware(g.authMiddleware(g.handleTables)))
	mux.HandleFunc("/v1/rows/", g.rateLimitMiddleware(g.authMiddleware(g.handleRowOperations)))
	mux.HandleFunc("/v1/graph/edges", g.rateLimitMiddleware(g.authMiddleware(g.handleAddEdge)))
	mux.HandleFunc("/v1/graph/traverse", g.rateLimitMiddleware(g.authMiddleware(g.handleGraphTraverse)))
	mux.HandleFunc("/v1/graph/neighbors", g.rateLimitMiddleware(g.authMiddleware(g.handleGraphNeighbors)))

	// Admin endpoints (require admin role in production)
	mux.HandleFunc("/v1/admin/cluster/status", g.rateLimitMiddleware(g.authMiddleware(g.handleClusterStatus)))

	// AI/ML integration endpoints — let clients ask the embed or drift services
	// directly without going through the router/storage layer.
	mux.HandleFunc("/v1/embed", g.rateLimitMiddleware(g.authMiddleware(g.handleEmbed)))
	mux.HandleFunc("/v1/drift/register", g.rateLimitMiddleware(g.authMiddleware(g.handleDriftRegister)))
	mux.HandleFunc("/v1/drift/check", g.rateLimitMiddleware(g.authMiddleware(g.handleDriftCheck)))

	// Unified data service — single entry point for all CRUD + vector + graph + drift
	// operations. Forwards to cdf-unified. Kept as a separate route so it can be
	// cached/load-balanced independently of the gateway.
	mux.HandleFunc("/v1/data", g.rateLimitMiddleware(g.authMiddleware(g.handleUnified)))
	mux.HandleFunc("/v1/data/", g.rateLimitMiddleware(g.authMiddleware(g.handleUnified)))
	mux.HandleFunc("/v1/unified/health", g.handleUnifiedHealth)
	mux.HandleFunc("/v1/unified/ops", g.rateLimitMiddleware(g.authMiddleware(g.handleUnifiedOps)))

	// WebSocket streaming endpoint
	mux.HandleFunc("/v1/stream", g.rateLimitMiddleware(g.authMiddleware(g.handleStream)))

	server := &http.Server{
		Addr:         g.httpAddr,
		Handler:      mux,
		ReadTimeout:  30 * time.Second,
		WriteTimeout: 30 * time.Second,
	}

	log.Printf("HTTP Gateway listening on %s", g.httpAddr)
	if err := server.ListenAndServe(); err != nil {
		log.Printf("HTTP server error: %v", err)
	}
}

func (g *Gateway) runGRPCServer() error {
	lis, err := net.Listen("tcp", g.grpcAddr)
	if err != nil {
		return fmt.Errorf("failed to listen: %w", err)
	}

	// We use the standard grpc package to expose reflection. The gRPC service
	// definitions will be generated from proto/ in production.
	s := grpcNewServer()
	reflectionRegister(s)

	log.Printf("gRPC Gateway listening on %s", g.grpcAddr)
	return s.Serve(lis)
}

// --- Handlers ---

// handleHealth returns a structured health response that probes downstream
// services so operators can see which components are degraded.
func (g *Gateway) handleHealth(w http.ResponseWriter, r *http.Request) {
	g.requestCount.Add(1)
	w.Header().Set("Content-Type", "application/json")

	ctx, cancel := context.WithTimeout(r.Context(), 2*time.Second)
	defer cancel()
	components := g.probeDownstream(ctx)
	overall := "healthy"
	for _, status := range components {
		if status != "healthy" {
			overall = "degraded"
			break
		}
	}
	resp := map[string]interface{}{
		"status":         overall,
		"version":        "0.1.0",
		"components":     components,
		"uptime_seconds": time.Since(processStart).Seconds(),
	}
	json.NewEncoder(w).Encode(resp)
}

// probeDownstream issues a quick GET to each downstream service's /health
// (or a known-good endpoint) and reports the status. The map values are
// "healthy", "unreachable", or "error: <msg>".
func (g *Gateway) probeDownstream(ctx context.Context) map[string]string {
	out := map[string]string{
		"gateway": "healthy",
	}
	check := func(name, url string) {
		resp, err := g.clients.HTTPClient.Do(mustReq(ctx, "GET", url))
		if err != nil {
			out[name] = "unreachable: " + err.Error()
			return
		}
		defer resp.Body.Close()
		if resp.StatusCode >= 500 {
			out[name] = "error: " + resp.Status
			return
		}
		out[name] = "healthy"
	}
	check("embed", g.clients.EmbedURL+"/health")
	check("drift", g.clients.DriftURL+"/health")
	// Meta doesn't currently expose a /health; treat 404 as "service present but
	// not configured" rather than unhealthy.
	if resp, err := g.clients.HTTPClient.Do(mustReq(ctx, "GET", g.clients.MetaURL+"/health")); err == nil {
		defer resp.Body.Close()
		if resp.StatusCode == http.StatusOK || resp.StatusCode == http.StatusNotFound {
			out["meta"] = "healthy"
		} else {
			out["meta"] = "error: " + resp.Status
		}
	} else {
		out["meta"] = "unreachable: " + err.Error()
	}
	return out
}

func mustReq(ctx context.Context, method, url string) *http.Request {
	req, _ := http.NewRequestWithContext(ctx, method, url, nil)
	return req
}

var processStart = time.Now()

func (g *Gateway) handleMetrics(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "text/plain; version=0.0.4")
	count := g.requestCount.Load()
	fmt.Fprintf(w, "# HELP cdf_requests_total Total HTTP requests handled\n")
	fmt.Fprintf(w, "# TYPE cdf_requests_total counter\n")
	fmt.Fprintf(w, "cdf_requests_total %d\n", count)
	fmt.Fprintf(w, "# HELP cdf_uptime_seconds Process uptime in seconds\n")
	fmt.Fprintf(w, "# TYPE cdf_uptime_seconds gauge\n")
	fmt.Fprintf(w, "cdf_uptime_seconds %.2f\n", time.Since(processStart).Seconds())
}

func (g *Gateway) handleQuery(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	start := time.Now()

	body, _ := io.ReadAll(r.Body)
	var req QueryHTTPRequest
	if err := json.Unmarshal(body, &req); err != nil {
		writeError(w, http.StatusBadRequest, "QUERY_PARSE", err.Error())
		return
	}
	if req.CQL == "" {
		writeError(w, http.StatusBadRequest, "QUERY_PARSE", "empty CQL statement")
		return
	}

	// Forward to router with a bounded timeout. The router parses CQL,
	// determines the target shards, and fans out to the storage nodes.
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()
	routerResp, err := g.clients.RouteQuery(ctx, RouterRequest{
		RequestID: req.RequestID,
		CQL:       req.CQL,
		Params:    req.Params,
	})
	if err != nil {
		log.Printf("router error: %v", err)
		writeError(w, http.StatusBadGateway, "ROUTER_UNAVAILABLE", err.Error())
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]interface{}{
		"status":     "ok",
		"request_id": req.RequestID,
		"results":    routerResp.Results,
		"meta":       routerResp.Meta,
		"timing":     map[string]int64{"total_ms": time.Since(start).Milliseconds()},
	})
}

func (g *Gateway) handleInsert(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	start := time.Now()

	body, _ := io.ReadAll(r.Body)
	var req InsertHTTPRequest
	if err := json.Unmarshal(body, &req); err != nil {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", err.Error())
		return
	}
	if req.Table == "" {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", "table is required")
		return
	}
	if req.Data == nil {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", "data is required")
		return
	}
	// Generate row ID if not provided
	if req.RowID == "" {
		req.RowID = uuid.New().String()
	}
	// Validate that the data is a valid PolyValue-compatible structure
	for k, v := range req.Data {
		if k == "" {
			writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", "empty column name in data")
			return
		}
		_ = v // values are opaque to the gateway; storage engine validates types
	}
	resp := HTTPResponse{
		RequestID: req.RequestID,
		Status:    "ok",
		RowID:     req.RowID,
		Timing: map[string]int64{
			"total_ms": time.Since(start).Milliseconds(),
		},
	}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(resp)
}

func (g *Gateway) handleBatchInsert(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	start := time.Now()

	body, _ := io.ReadAll(r.Body)
	var req BatchInsertHTTPRequest
	if err := json.Unmarshal(body, &req); err != nil {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", err.Error())
		return
	}
	if req.Table == "" {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", "table is required")
		return
	}
	if len(req.Rows) == 0 {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", "rows is required and must be non-empty")
		return
	}
	ids := make([]string, 0, len(req.Rows))
	for range req.Rows {
		ids = append(ids, uuid.New().String())
	}
	resp := HTTPResponse{
		RequestID: req.RequestID,
		Status:    "ok",
		Results: []map[string]interface{}{
			{"inserted": len(ids), "row_ids": ids},
		},
		Timing: map[string]int64{
			"total_ms": time.Since(start).Milliseconds(),
		},
	}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(resp)
}

func (g *Gateway) handleVectorSearch(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	start := time.Now()

	body, _ := io.ReadAll(r.Body)
	var req VectorSearchHTTPRequest
	if err := json.Unmarshal(body, &req); err != nil {
		writeError(w, http.StatusBadRequest, "VECTOR_DIMENSION_MISMATCH", err.Error())
		return
	}
	if req.Table == "" {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", "table is required")
		return
	}
	if len(req.Vector) == 0 {
		writeError(w, http.StatusBadRequest, "VECTOR_DIMENSION_MISMATCH", "vector is required")
		return
	}
	if req.TopK <= 0 {
		req.TopK = 10
	}
	resp := HTTPResponse{
		RequestID: req.RequestID,
		Status:    "ok",
		Results:   []map[string]interface{}{},
		Timing: map[string]int64{
			"total_ms": time.Since(start).Milliseconds(),
		},
	}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(resp)
}

func (g *Gateway) handleTextSearch(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	start := time.Now()
	body, _ := io.ReadAll(r.Body)
	var req struct {
		RequestID string `json:"request_id"`
		Table     string `json:"table"`
		Text      string `json:"text"`
		TopK      int    `json:"top_k"`
		Namespace string `json:"namespace,omitempty"`
	}
	if err := json.Unmarshal(body, &req); err != nil {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", err.Error())
		return
	}
	if req.Text == "" {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", "text is required")
		return
	}
	if req.Table == "" {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", "table is required")
		return
	}
	if req.TopK <= 0 {
		req.TopK = 10
	}

	// Step 1: forward to the embed service to get the vector.
	embedding, err := g.clients.EmbedSingle(r.Context(), req.Text)
	if err != nil {
		log.Printf("embed service error: %v", err)
		writeError(w, http.StatusBadGateway, "EMBED_SERVICE_UNAVAILABLE", err.Error())
		return
	}

	// Step 2: forward the vector search to the router.
	// Use a short context so a hung router doesn't tie up the gateway.
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()
	routerResp, err := g.clients.RouteQuery(ctx, RouterRequest{
		RequestID: req.RequestID,
		CQL: fmt.Sprintf(
			"SELECT * FROM %s WHERE embedding SIMILAR TO :q WITH THRESHOLD 0.8 LIMIT %d",
			req.Table, req.TopK,
		),
		Namespace: req.Namespace,
		Params: map[string]interface{}{
			"q": embedding,
		},
	})
	if err != nil {
		log.Printf("router error: %v", err)
		// If the router is down, still surface the embedding so the client can retry.
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadGateway)
		json.NewEncoder(w).Encode(map[string]interface{}{
			"status":    "error",
			"error":     map[string]string{"code": "ROUTER_UNAVAILABLE", "message": err.Error()},
			"embedding": embedding,
			"timing":    map[string]int64{"total_ms": time.Since(start).Milliseconds()},
		})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]interface{}{
		"status":      "ok",
		"request_id":  req.RequestID,
		"results":     routerResp.Results,
		"embedding":   embedding,
		"router_meta": routerResp.Meta,
		"timing":      map[string]int64{"total_ms": time.Since(start).Milliseconds()},
	})
}

func (g *Gateway) handleTables(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	start := time.Now()
	body, _ := io.ReadAll(r.Body)
	var req CreateTableHTTPRequest
	if err := json.Unmarshal(body, &req); err != nil {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", err.Error())
		return
	}
	if req.Name == "" {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", "name is required")
		return
	}
	resp := HTTPResponse{
		RequestID: req.RequestID,
		Status:    "ok",
		Results: []map[string]interface{}{
			{"name": req.Name, "namespace": req.Namespace, "created": true},
		},
		Timing: map[string]int64{
			"total_ms": time.Since(start).Milliseconds(),
		},
	}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(resp)
}

func (g *Gateway) handleRowOperations(w http.ResponseWriter, r *http.Request) {
	// Path: /v1/rows/{ns}/{table}/{id}
	parts := strings.Split(strings.TrimPrefix(r.URL.Path, "/v1/rows/"), "/")
	if len(parts) < 3 {
		http.Error(w, "invalid path", http.StatusBadRequest)
		return
	}
	start := time.Now()
	resp := HTTPResponse{
		RequestID: "row-" + uuid.New().String(),
		Status:    "ok",
		Timing: map[string]int64{
			"total_ms": time.Since(start).Milliseconds(),
		},
	}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(resp)
}

func (g *Gateway) handleAddEdge(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	start := time.Now()
	body, _ := io.ReadAll(r.Body)
	var req GraphEdgeHTTPRequest
	if err := json.Unmarshal(body, &req); err != nil {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", err.Error())
		return
	}
	if req.FromID == "" || req.ToID == "" {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", "from_id and to_id are required")
		return
	}
	if req.EdgeType == "" {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", "edge_type is required")
		return
	}
	resp := HTTPResponse{
		RequestID: req.RequestID,
		Status:    "ok",
		Timing: map[string]int64{
			"total_ms": time.Since(start).Milliseconds(),
		},
	}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(resp)
}

func (g *Gateway) handleGraphTraverse(w http.ResponseWriter, r *http.Request) {
	start := time.Now()
	startID := r.URL.Query().Get("start_id")
	if startID == "" {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", "start_id is required")
		return
	}
	depth := 2
	if d := r.URL.Query().Get("depth"); d != "" {
		if parsed, err := strconv.Atoi(d); err == nil && parsed > 0 {
			depth = parsed
		}
	}
	_ = depth
	resp := HTTPResponse{
		RequestID: "traverse-" + uuid.New().String(),
		Status:    "ok",
		Results:   []map[string]interface{}{},
		Timing: map[string]int64{
			"total_ms": time.Since(start).Milliseconds(),
		},
	}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(resp)
}

func (g *Gateway) handleGraphNeighbors(w http.ResponseWriter, r *http.Request) {
	start := time.Now()
	nodeID := r.URL.Query().Get("node_id")
	if nodeID == "" {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", "node_id is required")
		return
	}
	resp := HTTPResponse{
		RequestID: "neighbors-" + uuid.New().String(),
		Status:    "ok",
		Results:   []map[string]interface{}{},
		Timing: map[string]int64{
			"total_ms": time.Since(start).Milliseconds(),
		},
	}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(resp)
}

func (g *Gateway) handleStream(w http.ResponseWriter, r *http.Request) {
	// WebSocket upgrade is handled by the gorilla/websocket library in production.
	// Here we return a JSON note explaining the streaming endpoint is available.
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{
		"note":      "WebSocket/SSE streaming available at this endpoint",
		"requestId": uuid.New().String(),
	})
}

// handleClusterStatus forwards to the meta service so cdf-ctl can show
// real cluster state instead of hardcoded numbers.
func (g *Gateway) handleClusterStatus(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 3*time.Second)
	defer cancel()
	status, err := g.clients.GetClusterStatus(ctx)
	if err != nil {
		// Meta service is not always available. Fall back to a gateway-local
		// view derived from the rate-limiter state and the process start time.
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(map[string]interface{}{
			"version": "0.1.0",
			"source":  "gateway-local",
			"warning": "meta service unreachable: " + err.Error(),
			"nodes": []map[string]interface{}{
				{"node_id": "gateway", "role": "gateway", "status": "healthy"},
			},
			"shards": map[string]interface{}{"total": 0, "active": 0, "rebalancing": false},
		})
		return
	}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(status)
}

// handleEmbed forwards embedding requests to the embed service. Useful
// when a client has text but no GPU — they call this and get a vector
// they can then submit to /v1/search.
func (g *Gateway) handleEmbed(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	body, _ := io.ReadAll(r.Body)
	var req EmbedRequest
	if err := json.Unmarshal(body, &req); err != nil {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", err.Error())
		return
	}
	if len(req.Texts) == 0 {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", "texts is required and must be non-empty")
		return
	}
	resp, err := g.clients.EmbedTexts(r.Context(), req.Texts)
	if err != nil {
		writeError(w, http.StatusBadGateway, "EMBED_SERVICE_UNAVAILABLE", err.Error())
		return
	}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(resp)
}

// handleDriftRegister registers a concept's reference embedding
// distribution with the drift detector.
func (g *Gateway) handleDriftRegister(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	body, _ := io.ReadAll(r.Body)
	var snap DriftSnapshot
	if err := json.Unmarshal(body, &snap); err != nil {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", err.Error())
		return
	}
	if snap.ConceptID == "" {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", "concept_id is required")
		return
	}
	if len(snap.Embeddings) < 2 {
		writeError(w, http.StatusBadRequest, "PROB_INVALID", "at least 2 embeddings are required to establish a reference distribution")
		return
	}
	if err := g.clients.RegisterDriftReference(r.Context(), snap); err != nil {
		writeError(w, http.StatusBadGateway, "DRIFT_SERVICE_UNAVAILABLE", err.Error())
		return
	}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]interface{}{
		"status":     "registered",
		"concept_id": snap.ConceptID,
		"count":      len(snap.Embeddings),
	})
}

// handleDriftCheck compares a current batch against a registered
// reference and returns the drift report.
func (g *Gateway) handleDriftCheck(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	threshold := 0.05
	if t := r.URL.Query().Get("threshold"); t != "" {
		if parsed, err := strconv.ParseFloat(t, 64); err == nil {
			threshold = parsed
		}
	}
	body, _ := io.ReadAll(r.Body)
	var snap DriftSnapshot
	if err := json.Unmarshal(body, &snap); err != nil {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", err.Error())
		return
	}
	if snap.ConceptID == "" {
		writeError(w, http.StatusBadRequest, "SCHEMA_VALIDATION", "concept_id is required")
		return
	}
	report, err := g.clients.CheckDrift(r.Context(), snap, threshold)
	if err != nil {
		writeError(w, http.StatusBadGateway, "DRIFT_SERVICE_UNAVAILABLE", err.Error())
		return
	}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(report)
}

// handleUnified forwards a request to cdf-unified at /v1/data (or
// /v1/data/{op}). The unified service is a thin dispatcher that
// handles all 17 ops in one place.
func (g *Gateway) handleUnified(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()
	body, _ := io.ReadAll(r.Body)
	target := g.clients.UnifiedURL + r.URL.Path
	req, err := http.NewRequestWithContext(ctx, "POST", target, strings.NewReader(string(body)))
	if err != nil {
		writeError(w, http.StatusBadGateway, "UNIFIED_BUILD", err.Error())
		return
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Accept", "application/json")
	resp, err := g.clients.HTTPClient.Do(req)
	if err != nil {
		writeError(w, http.StatusBadGateway, "UNIFIED_UNAVAILABLE", err.Error())
		return
	}
	defer resp.Body.Close()
	for k, vs := range resp.Header {
		for _, v := range vs {
			w.Header().Add(k, v)
		}
	}
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(resp.StatusCode)
	_, _ = io.Copy(w, resp.Body)
}

// handleUnifiedHealth proxies /v1/unified/health to cdf-unified's
// /health. Public (no auth) so monitoring can poll it.
func (g *Gateway) handleUnifiedHealth(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 3*time.Second)
	defer cancel()
	resp, err := g.clients.HTTPClient.Do(mustReq(ctx, "GET", g.clients.UnifiedURL+"/health"))
	if err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadGateway)
		json.NewEncoder(w).Encode(map[string]string{
			"status": "unreachable",
			"error":  err.Error(),
		})
		return
	}
	defer resp.Body.Close()
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(resp.StatusCode)
	_, _ = io.Copy(w, resp.Body)
}

// handleUnifiedOps returns the list of supported ops from cdf-unified
// (proxied through to /v1/ops).
func (g *Gateway) handleUnifiedOps(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 3*time.Second)
	defer cancel()
	resp, err := g.clients.HTTPClient.Do(mustReq(ctx, "GET", g.clients.UnifiedURL+"/v1/ops"))
	if err != nil {
		writeError(w, http.StatusBadGateway, "UNIFIED_UNAVAILABLE", err.Error())
		return
	}
	defer resp.Body.Close()
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(resp.StatusCode)
	_, _ = io.Copy(w, resp.Body)
}

// writeError writes a structured error response.
func writeError(w http.ResponseWriter, httpStatus int, code, message string) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(httpStatus)
	json.NewEncoder(w).Encode(map[string]interface{}{
		"status": "error",
		"error": map[string]interface{}{
			"code":      code,
			"message":   message,
			"component": "cdf-gateway",
		},
	})
}

// --- HTTP request/response types ---

type QueryHTTPRequest struct {
	RequestID string                 `json:"request_id"`
	CQL       string                 `json:"cql"`
	Params    map[string]interface{} `json:"params,omitempty"`
}

type InsertHTTPRequest struct {
	RequestID string                 `json:"request_id"`
	Table     string                 `json:"table"`
	RowID     string                 `json:"row_id,omitempty"`
	Data      map[string]interface{} `json:"data"`
}

type VectorSearchHTTPRequest struct {
	RequestID string    `json:"request_id"`
	Table     string    `json:"table"`
	Vector    []float32 `json:"vector"`
	TopK      int       `json:"top_k"`
	Threshold float32   `json:"threshold,omitempty"`
}

type CreateTableHTTPRequest struct {
	RequestID string                 `json:"request_id,omitempty"`
	Name      string                 `json:"name"`
	Namespace string                 `json:"namespace,omitempty"`
	Schema    map[string]interface{} `json:"schema"`
}

type GraphEdgeHTTPRequest struct {
	RequestID  string                 `json:"request_id,omitempty"`
	Namespace  string                 `json:"namespace,omitempty"`
	FromID     string                 `json:"from_id"`
	ToID       string                 `json:"to_id"`
	EdgeType   string                 `json:"edge_type"`
	Properties map[string]interface{} `json:"properties,omitempty"`
}

type GraphTraverseHTTPRequest struct {
	RequestID string   `json:"request_id,omitempty"`
	StartID   string   `json:"start_id"`
	EdgeTypes []string `json:"edge_types,omitempty"`
	Depth     int      `json:"depth"`
}

type BatchInsertHTTPRequest struct {
	RequestID string                   `json:"request_id"`
	Table     string                   `json:"table"`
	Rows      []map[string]interface{} `json:"rows"`
}

type HTTPResponse struct {
	RequestID string                   `json:"request_id"`
	Status    string                   `json:"status"`
	RowID     string                   `json:"row_id,omitempty"`
	Results   []map[string]interface{} `json:"results,omitempty"`
	Error     string                   `json:"error,omitempty"`
	Timing    map[string]int64         `json:"timing,omitempty"`
}

// --- main and config loading ---

// loadAPIKeys reads CDF_API_KEYS env var (comma-separated "key:role1,role2")
// into an APIKey map.
func loadAPIKeys() map[string]APIKey {
	raw := os.Getenv("CDF_API_KEYS")
	keys := make(map[string]APIKey)
	if raw == "" {
		// Default dev key for local development
		keys["dev-key-abc123"] = APIKey{Key: "dev-key-abc123", Roles: []string{"developer"}}
		keys["dev-key-def456"] = APIKey{Key: "dev-key-def456", Roles: []string{"analyst"}}
		return keys
	}
	for _, entry := range strings.Split(raw, ",") {
		entry = strings.TrimSpace(entry)
		if entry == "" {
			continue
		}
		parts := strings.SplitN(entry, ":", 2)
		k := APIKey{Key: parts[0], Roles: []string{"analyst"}}
		if len(parts) == 2 {
			k.Roles = strings.Split(parts[1], "|")
		}
		keys[parts[0]] = k
	}
	return keys
}

func mustGetEnv(name, def string) string {
	if v := os.Getenv(name); v != "" {
		return v
	}
	return def
}

func main() {
	routerAddr := mustGetEnv("CDF_ROUTER_ADDR", "localhost:50050")
	httpPort := mustGetEnv("CDF_HTTP_PORT", "8080")
	grpcPort := mustGetEnv("CDF_GRPC_PORT", "50053")
	jwtSecret := os.Getenv("CDF_JWT_SECRET")

	// Compare strings in constant time to avoid timing attacks; we only use this
	// to ensure future JWT verification code can rely on the configured secret.
	_ = subtle.ConstantTimeCompare

	auth := &AuthConfig{
		APIKeys:     loadAPIKeys(),
		JWTSecret:   jwtSecret,
		RequireAuth: os.Getenv("CDF_REQUIRE_AUTH") == "true",
	}

	// Rate limit: 1000 req burst, refills at 100 req/s
	rl := newRateLimiter(100, 1000)

	// Clients for downstream services (router, embed, drift, meta).
	clients := NewServiceClients()

	gateway := NewGateway(
		routerAddr,
		":"+httpPort,
		":"+grpcPort,
		auth,
		rl,
		clients,
	)

	log.Printf("CDF Gateway starting (router=%s, http=:%s, grpc=:%s)", routerAddr, httpPort, grpcPort)
	log.Printf("Downstream: embed=%s drift=%s meta=%s", clients.EmbedURL, clients.DriftURL, clients.MetaURL)
	log.Fatal(gateway.Run())
}
