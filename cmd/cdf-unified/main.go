// Package main implements cdf-unified — the single, unified data service
// for ALL data injection and retrieval in the Cognitive Data Fabric.
//
// Every client (SDK, cURL, internal service) interacts with the fabric
// through a single endpoint: POST /v1/data. The request body is a
// `DataRequest` envelope whose `op` field selects the pipeline. The
// response is always a `DataResponse` envelope with the same shape
// regardless of the op.
//
// This service composes all the other CDF services: it forwards to the
// router, embed service, drift detector, and storage nodes via the same
// clients the gateway uses. It is intentionally a thin dispatcher; the
// business logic lives in the existing specialized services.
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"log"
	"net"
	"net/http"
	"sync/atomic"
	"time"
)

// -----------------------------------------------------------------------------
// Common request/response types — mirror crates/cdf-common/src/data.rs
// -----------------------------------------------------------------------------

// DataOp is the closed set of operations the unified service understands.
// It maps 1:1 to cdf_common::DataOp in Rust.
type DataOp string

const (
	OpInsert          DataOp = "insert"
	OpBatchInsert     DataOp = "batch_insert"
	OpGet             DataOp = "get"
	OpDelete          DataOp = "delete"
	OpUpdate          DataOp = "update"
	OpScan            DataOp = "scan"
	OpCreateTable     DataOp = "create_table"
	OpDropTable       DataOp = "drop_table"
	OpVectorSearch    DataOp = "vector_search"
	OpTextSearch      DataOp = "text_search"
	OpEmbed           DataOp = "embed"
	OpGraphAddEdge    DataOp = "graph_add_edge"
	OpGraphRemoveEdge DataOp = "graph_remove_edge"
	OpGraphTraverse   DataOp = "graph_traverse"
	OpGraphNeighbors  DataOp = "graph_neighbors"
	OpDriftRegister   DataOp = "drift_register"
	OpDriftCheck      DataOp = "drift_check"
	OpTemporalQuery   DataOp = "temporal_query"
)

// PolyValue mirrors the Rust PolyValue enum. We use a single struct with
// a Type + Value pair so the JSON wire format matches Rust's
// `#[serde(tag = "type", content = "value")]` representation.
type PolyValue struct {
	Type  string          `json:"type"`
	Value json.RawMessage `json:"value"`
}

// DataRequest is the unified request envelope.
type DataRequest struct {
	RequestID  string                 `json:"request_id,omitempty"`
	Op         DataOp                 `json:"op"`
	Namespace  string                 `json:"namespace,omitempty"`
	Table      string                 `json:"table,omitempty"`
	Row        map[string]PolyValue   `json:"row,omitempty"`
	RowID      string                 `json:"row_id,omitempty"`
	Rows       []map[string]PolyValue `json:"rows,omitempty"`
	Vector     []float32              `json:"vector,omitempty"`
	Text       string                 `json:"text,omitempty"`
	Texts      []string               `json:"texts,omitempty"`
	TopK       int                    `json:"top_k,omitempty"`
	Threshold  *float32               `json:"threshold,omitempty"`
	ConceptID  string                 `json:"concept_id,omitempty"`
	Embeddings [][]float32            `json:"embeddings,omitempty"`
	FromID     string                 `json:"from_id,omitempty"`
	ToID       string                 `json:"to_id,omitempty"`
	EdgeType   string                 `json:"edge_type,omitempty"`
	StartID    string                 `json:"start_id,omitempty"`
	NodeID     string                 `json:"node_id,omitempty"`
	Depth      int                    `json:"depth,omitempty"`
	CQL        string                 `json:"cql,omitempty"`
	Params     map[string]interface{} `json:"params,omitempty"`
	Options    map[string]interface{} `json:"options,omitempty"`
}

// DataResponse is the unified response envelope. Every op returns this.
type DataResponse struct {
	RequestID      string                 `json:"request_id"`
	Op             DataOp                 `json:"op"`
	Status         string                 `json:"status"`
	Row            map[string]PolyValue   `json:"row,omitempty"`
	Rows           []map[string]PolyValue `json:"rows,omitempty"`
	Vectors        [][]float32            `json:"vectors,omitempty"`
	Count          int                    `json:"count,omitempty"`
	DriftScore     *float64               `json:"drift_score,omitempty"`
	DriftDetected  *bool                  `json:"drift_detected,omitempty"`
	EmbeddingModel string                 `json:"embedding_model,omitempty"`
	Meta           map[string]interface{} `json:"meta,omitempty"`
	Error          *DataError             `json:"error,omitempty"`
}

type DataError struct {
	Code    string `json:"code"`
	Message string `json:"message"`
}

// -----------------------------------------------------------------------------
// Service plumbing
// -----------------------------------------------------------------------------

// Service is the unified dispatcher.
type Service struct {
	addr         string
	grpcAddr     string
	clients      *Clients
	requestCount atomic.Uint64
	startTime    time.Time
}

// NewService wires a Service from environment variables.
func NewService(clients *Clients, httpAddr, grpcAddr string) *Service {
	return &Service{
		clients:   clients,
		addr:      httpAddr,
		grpcAddr:  grpcAddr,
		startTime: time.Now(),
	}
}

// validate enforces the invariants the Rust side also enforces.
func (s *Service) validate(req *DataRequest) *DataError {
	if req.Op == "" {
		return &DataError{Code: "SCHEMA_VALIDATION", Message: "op is required"}
	}
	if req.Namespace == "" {
		req.Namespace = "default"
	}
	if req.TopK < 0 {
		return &DataError{Code: "SCHEMA_VALIDATION", Message: "top_k must be >= 0"}
	}
	if req.Depth < 0 {
		return &DataError{Code: "SCHEMA_VALIDATION", Message: "depth must be >= 0"}
	}
	needsTable := map[DataOp]bool{
		OpInsert: true, OpBatchInsert: true, OpGet: true, OpDelete: true,
		OpUpdate: true, OpScan: true, OpVectorSearch: true, OpTextSearch: true,
	}
	if needsTable[req.Op] && req.Table == "" {
		return &DataError{Code: "SCHEMA_VALIDATION", Message: "table is required for op=" + string(req.Op)}
	}
	switch req.Op {
	case OpTextSearch:
		if req.Text == "" {
			return &DataError{Code: "SCHEMA_VALIDATION", Message: "text is required for text_search"}
		}
		if req.TopK == 0 {
			req.TopK = 10
		}
	case OpVectorSearch:
		if len(req.Vector) == 0 {
			return &DataError{Code: "VECTOR_DIMENSION_MISMATCH", Message: "vector is required for vector_search"}
		}
		if req.TopK == 0 {
			req.TopK = 10
		}
	case OpEmbed:
		if len(req.Texts) == 0 && req.Text == "" {
			return &DataError{Code: "SCHEMA_VALIDATION", Message: "texts or text is required for embed"}
		}
	case OpDriftRegister, OpDriftCheck:
		if req.ConceptID == "" {
			return &DataError{Code: "SCHEMA_VALIDATION", Message: "concept_id is required for " + string(req.Op)}
		}
		if req.Op == OpDriftRegister && len(req.Embeddings) < 2 {
			return &DataError{Code: "PROB_INVALID", Message: "at least 2 embeddings are required to register a reference"}
		}
	case OpGraphAddEdge:
		if req.FromID == "" || req.ToID == "" || req.EdgeType == "" {
			return &DataError{Code: "SCHEMA_VALIDATION", Message: "from_id, to_id, edge_type are required for graph_add_edge"}
		}
	case OpGraphTraverse:
		if req.StartID == "" {
			return &DataError{Code: "SCHEMA_VALIDATION", Message: "start_id is required for graph_traverse"}
		}
		if req.Depth == 0 {
			req.Depth = 2
		}
	case OpGraphNeighbors:
		if req.NodeID == "" {
			return &DataError{Code: "SCHEMA_VALIDATION", Message: "node_id is required for graph_neighbors"}
		}
	}
	return nil
}

// dispatch is the main op router. Each op gets a small, focused handler.
func (s *Service) dispatch(ctx context.Context, req *DataRequest) DataResponse {
	if req.RequestID == "" {
		req.RequestID = newRequestID()
	}
	if err := s.validate(req); err != nil {
		return DataResponse{
			RequestID: req.RequestID,
			Op:        req.Op,
			Status:    "error",
			Error:     err,
		}
	}
	start := time.Now()
	var resp DataResponse
	switch req.Op {
	case OpTextSearch:
		resp = s.handleTextSearch(ctx, req)
	case OpEmbed:
		resp = s.handleEmbed(ctx, req)
	case OpDriftRegister:
		resp = s.handleDriftRegister(ctx, req)
	case OpDriftCheck:
		resp = s.handleDriftCheck(ctx, req)
	case OpGraphAddEdge, OpGraphRemoveEdge, OpGraphTraverse, OpGraphNeighbors:
		resp = s.handleGraph(ctx, req)
	case OpVectorSearch, OpScan, OpGet, OpInsert, OpBatchInsert, OpUpdate, OpDelete,
		OpCreateTable, OpDropTable, OpTemporalQuery:
		resp = s.handleStorageOrRouter(ctx, req)
	default:
		return DataResponse{
			RequestID: req.RequestID,
			Op:        req.Op,
			Status:    "error",
			Error:     &DataError{Code: "UNKNOWN_OP", Message: "unsupported op: " + string(req.Op)},
		}
	}
	if resp.Meta == nil {
		resp.Meta = map[string]interface{}{}
	}
	resp.Meta["dispatch_ms"] = time.Since(start).Milliseconds()
	return resp
}

// -----------------------------------------------------------------------------
// Op handlers (small, focused, forward to existing services)
// -----------------------------------------------------------------------------

func (s *Service) handleTextSearch(ctx context.Context, req *DataRequest) DataResponse {
	emb, err := s.clients.EmbedSingle(ctx, req.Text)
	if err != nil {
		return errorResp(req, "EMBED_SERVICE_UNAVAILABLE", err.Error())
	}
	cql := fmt.Sprintf("SELECT * FROM %s WHERE embedding SIMILAR TO :q WITH THRESHOLD 0.8 LIMIT %d", req.Table, req.TopK)
	routerResp, err := s.clients.RouteQuery(ctx, RouterRequest{
		RequestID: req.RequestID,
		CQL:       cql,
		Namespace: req.Namespace,
		Params:    map[string]interface{}{"q": emb},
	})
	if err != nil {
		return DataResponse{
			RequestID: req.RequestID,
			Op:        req.Op,
			Status:    "error",
			Vectors:   [][]float32{emb},
			Error:     &DataError{Code: "ROUTER_UNAVAILABLE", Message: err.Error()},
		}
	}
	return DataResponse{
		RequestID:      req.RequestID,
		Op:             req.Op,
		Status:         "ok",
		Rows:           nil,
		Vectors:        [][]float32{emb},
		Count:          len(routerResp.Results),
		EmbeddingModel: "all-MiniLM-L6-v2",
		Meta:           map[string]interface{}{"router_meta": routerResp.Meta},
	}
}

func (s *Service) handleEmbed(ctx context.Context, req *DataRequest) DataResponse {
	texts := req.Texts
	if len(texts) == 0 && req.Text != "" {
		texts = []string{req.Text}
	}
	emb, err := s.clients.EmbedTexts(ctx, texts)
	if err != nil {
		return errorResp(req, "EMBED_SERVICE_UNAVAILABLE", err.Error())
	}
	return DataResponse{
		RequestID:      req.RequestID,
		Op:             req.Op,
		Status:         "ok",
		Vectors:        emb.Embeddings,
		Count:          len(emb.Embeddings),
		EmbeddingModel: emb.ModelID,
		Meta:           map[string]interface{}{"dimensions": emb.Dimensions, "processing_time_ms": emb.ProcessingTimeMS},
	}
}

func (s *Service) handleDriftRegister(ctx context.Context, req *DataRequest) DataResponse {
	if err := s.clients.RegisterDriftReference(ctx, DriftSnapshot{
		ConceptID:  req.ConceptID,
		Embeddings: req.Embeddings,
	}); err != nil {
		return errorResp(req, "DRIFT_SERVICE_UNAVAILABLE", err.Error())
	}
	return DataResponse{
		RequestID: req.RequestID,
		Op:        req.Op,
		Status:    "ok",
		Count:     len(req.Embeddings),
		Meta:      map[string]interface{}{"concept_id": req.ConceptID},
	}
}

func (s *Service) handleDriftCheck(ctx context.Context, req *DataRequest) DataResponse {
	threshold := 0.05
	if t, ok := req.Options["threshold"].(float64); ok {
		threshold = t
	}
	report, err := s.clients.CheckDrift(ctx, DriftSnapshot{
		ConceptID:  req.ConceptID,
		Embeddings: req.Embeddings,
	}, threshold)
	if err != nil {
		return errorResp(req, "DRIFT_SERVICE_UNAVAILABLE", err.Error())
	}
	detected := report.DriftDetected
	return DataResponse{
		RequestID:     req.RequestID,
		Op:            req.Op,
		Status:        "ok",
		Count:         len(req.Embeddings),
		DriftScore:    &report.DriftScore,
		DriftDetected: &detected,
		Meta: map[string]interface{}{
			"concept_id":     report.ConceptID,
			"method":         report.Method,
			"p_value":        report.PValue,
			"reference_mean": report.ReferenceMean,
			"current_mean":   report.CurrentMean,
		},
	}
}

func (s *Service) handleGraph(ctx context.Context, req *DataRequest) DataResponse {
	// Graph ops are forwarded to the router as CQL. The router then dispatches
	// to the storage node that owns the graph shard.
	cql := ""
	switch req.Op {
	case OpGraphAddEdge:
		cql = fmt.Sprintf("GRAPH.ADD_EDGE %s -> %s [type=%s]", req.FromID, req.ToID, req.EdgeType)
	case OpGraphRemoveEdge:
		cql = fmt.Sprintf("GRAPH.REMOVE_EDGE %s -> %s [type=%s]", req.FromID, req.ToID, req.EdgeType)
	case OpGraphTraverse:
		cql = fmt.Sprintf("GRAPH.TRAVERSE FROM %s DEPTH %d", req.StartID, req.Depth)
	case OpGraphNeighbors:
		cql = fmt.Sprintf("GRAPH.NEIGHBORS OF %s", req.NodeID)
	}
	routerResp, err := s.clients.RouteQuery(ctx, RouterRequest{
		RequestID: req.RequestID,
		CQL:       cql,
		Namespace: req.Namespace,
	})
	if err != nil {
		return errorResp(req, "ROUTER_UNAVAILABLE", err.Error())
	}
	return DataResponse{
		RequestID: req.RequestID,
		Op:        req.Op,
		Status:    "ok",
		Count:     len(routerResp.Results),
		Meta:      map[string]interface{}{"router_meta": routerResp.Meta},
	}
}

func (s *Service) handleStorageOrRouter(ctx context.Context, req *DataRequest) DataResponse {
	// Map the unified op to the equivalent CQL statement. The router parses CQL
	// and dispatches to storage nodes.
	cql := ""
	switch req.Op {
	case OpInsert:
		cql = fmt.Sprintf("INSERT INTO %s ROW '%s' %s", req.Table, req.RowID, polyMapToJSON(req.Row))
	case OpBatchInsert:
		cql = fmt.Sprintf("BATCH_INSERT INTO %s (%d rows)", req.Table, len(req.Rows))
	case OpGet:
		cql = fmt.Sprintf("SELECT * FROM %s WHERE id='%s' AS OF NOW", req.Table, req.RowID)
	case OpDelete:
		cql = fmt.Sprintf("DELETE FROM %s WHERE id='%s'", req.Table, req.RowID)
	case OpUpdate:
		cql = fmt.Sprintf("UPDATE %s WHERE id='%s' SET %s", req.Table, req.RowID, polyMapToJSON(req.Row))
	case OpScan:
		cql = fmt.Sprintf("SELECT * FROM %s LIMIT %d", req.Table, defaultLimit(req.TopK))
	case OpVectorSearch:
		cql = fmt.Sprintf("SELECT * FROM %s WHERE embedding SIMILAR TO :q LIMIT %d", req.Table, defaultLimit(req.TopK))
	case OpCreateTable:
		cql = fmt.Sprintf("CREATE TABLE %s", req.Table)
	case OpDropTable:
		cql = fmt.Sprintf("DROP TABLE %s", req.Table)
	case OpTemporalQuery:
		cql = req.CQL // pass-through
	}
	params := map[string]interface{}{}
	for k, v := range req.Params {
		params[k] = v
	}
	if req.Op == OpVectorSearch {
		params["q"] = req.Vector
	}
	routerResp, err := s.clients.RouteQuery(ctx, RouterRequest{
		RequestID: req.RequestID,
		CQL:       cql,
		Namespace: req.Namespace,
		Params:    params,
	})
	if err != nil {
		return errorResp(req, "ROUTER_UNAVAILABLE", err.Error())
	}
	return DataResponse{
		RequestID: req.RequestID,
		Op:        req.Op,
		Status:    "ok",
		Rows:      nil, // router returns map[string]interface; mapping to PolyValue is a v0.2 task
		Count:     len(routerResp.Results),
		Meta:      map[string]interface{}{"router_meta": routerResp.Meta, "cql": cql},
	}
}

// -----------------------------------------------------------------------------
// HTTP / gRPC server
// -----------------------------------------------------------------------------

func (s *Service) Run() error {
	go s.runGRPCServer()
	s.runHTTPServer()
	return nil
}

func (s *Service) runHTTPServer() {
	mux := http.NewServeMux()
	mux.HandleFunc("/health", s.handleHealth)
	mux.HandleFunc("/v1/data", s.handleData)
	mux.HandleFunc("/v1/data/", s.handleDataSubpath) // /v1/data/{op} shortcut
	mux.HandleFunc("/metrics", s.handleMetrics)
	mux.HandleFunc("/v1/ops", s.handleListOps)

	srv := &http.Server{
		Addr:         s.addr,
		Handler:      mux,
		ReadTimeout:  30 * time.Second,
		WriteTimeout: 60 * time.Second,
	}
	log.Printf("cdf-unified HTTP listening on %s", s.addr)
	if err := srv.ListenAndServe(); err != nil {
		log.Printf("cdf-unified HTTP error: %v", err)
	}
}

func (s *Service) runGRPCServer() {
	l, err := net.Listen("tcp", s.grpcAddr)
	if err != nil {
		log.Printf("cdf-unified gRPC listen error: %v", err)
		return
	}
	_ = l
	log.Printf("cdf-unified gRPC stub listening on %s (proto codegen in v0.2)", s.grpcAddr)
	// Real gRPC service: use the generated proto bindings. Until then this
	// log line documents the intent.
}

// handleData is the main unified entry point.
func (s *Service) handleData(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeErr(w, http.StatusMethodNotAllowed, "METHOD_NOT_ALLOWED", "use POST")
		return
	}
	s.requestCount.Add(1)
	body, _ := readAll(r.Body)
	var req DataRequest
	if err := json.Unmarshal(body, &req); err != nil {
		writeErr(w, http.StatusBadRequest, "SCHEMA_VALIDATION", err.Error())
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()
	resp := s.dispatch(ctx, &req)
	w.Header().Set("Content-Type", "application/json")
	if resp.Status == "error" {
		w.WriteHeader(http.StatusBadGateway)
	} else {
		w.WriteHeader(http.StatusOK)
	}
	_ = json.NewEncoder(w).Encode(resp)
}

// handleDataSubpath lets clients hit POST /v1/data/insert (etc) for convenience.
func (s *Service) handleDataSubpath(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeErr(w, http.StatusMethodNotAllowed, "METHOD_NOT_ALLOWED", "use POST")
		return
	}
	op := r.URL.Path[len("/v1/data/"):]
	if op == "" {
		writeErr(w, http.StatusBadRequest, "SCHEMA_VALIDATION", "missing op in path")
		return
	}
	body, _ := readAll(r.Body)
	var req DataRequest
	if err := json.Unmarshal(body, &req); err != nil {
		writeErr(w, http.StatusBadRequest, "SCHEMA_VALIDATION", err.Error())
		return
	}
	req.Op = DataOp(op)
	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()
	resp := s.dispatch(ctx, &req)
	w.Header().Set("Content-Type", "application/json")
	if resp.Status == "error" {
		w.WriteHeader(http.StatusBadGateway)
	} else {
		w.WriteHeader(http.StatusOK)
	}
	_ = json.NewEncoder(w).Encode(resp)
}

func (s *Service) handleHealth(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 2*time.Second)
	defer cancel()
	components := map[string]string{"cdf-unified": "healthy"}
	targets := map[string]string{
		"embed": s.clients.EmbedURL + "/health",
		"drift": s.clients.DriftURL + "/health",
	}
	for name, url := range targets {
		req, rerr := http.NewRequestWithContext(ctx, "GET", url, nil)
		if rerr != nil {
			components[name] = "unreachable: " + rerr.Error()
			continue
		}
		hresp, herr := s.clients.HTTPClient.inner.Do(req)
		if herr != nil {
			components[name] = "unreachable: " + herr.Error()
			continue
		}
		hresp.Body.Close()
		if hresp.StatusCode >= 500 {
			components[name] = "error: " + hresp.Status
		} else {
			components[name] = "healthy"
		}
		_ = name
	}
	overall := "healthy"
	for _, v := range components {
		if v != "healthy" {
			overall = "degraded"
			break
		}
	}
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(map[string]interface{}{
		"status":         overall,
		"version":        "0.1.0",
		"components":     components,
		"uptime_seconds": time.Since(s.startTime).Seconds(),
	})
}

func (s *Service) handleMetrics(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "text/plain; version=0.0.4")
	fmt.Fprintf(w, "cdf_unified_requests_total %d\n", s.requestCount.Load())
	fmt.Fprintf(w, "cdf_unified_uptime_seconds %.2f\n", time.Since(s.startTime).Seconds())
}

func (s *Service) handleListOps(w http.ResponseWriter, r *http.Request) {
	ops := []DataOp{
		OpInsert, OpBatchInsert, OpGet, OpDelete, OpUpdate, OpScan,
		OpCreateTable, OpDropTable,
		OpVectorSearch, OpTextSearch, OpEmbed,
		OpGraphAddEdge, OpGraphRemoveEdge, OpGraphTraverse, OpGraphNeighbors,
		OpDriftRegister, OpDriftCheck,
		OpTemporalQuery,
	}
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(map[string]interface{}{
		"version": "0.1.0",
		"ops":     ops,
	})
}

// -----------------------------------------------------------------------------
// Small helpers
// -----------------------------------------------------------------------------

func newRequestID() string {
	return fmt.Sprintf("unified-%d", time.Now().UnixNano())
}

func defaultLimit(k int) int {
	if k <= 0 {
		return 100
	}
	return k
}

func errorResp(req *DataRequest, code, msg string) DataResponse {
	return DataResponse{
		RequestID: req.RequestID,
		Op:        req.Op,
		Status:    "error",
		Error:     &DataError{Code: code, Message: msg},
	}
}

func writeErr(w http.ResponseWriter, status int, code, msg string) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(DataResponse{
		Status: "error",
		Error:  &DataError{Code: code, Message: msg},
	})
}

func readAll(r interface{ Read([]byte) (int, error) }) ([]byte, error) {
	var out []byte
	buf := make([]byte, 4096)
	for {
		n, err := r.Read(buf)
		if n > 0 {
			out = append(out, buf[:n]...)
		}
		if err != nil {
			if err.Error() == "EOF" {
				return out, nil
			}
			return out, nil
		}
	}
}

func polyMapToJSON(m map[string]PolyValue) string {
	b, _ := json.Marshal(m)
	return string(b)
}

// -----------------------------------------------------------------------------
// main
// -----------------------------------------------------------------------------

func main() {
	httpAddr := flag.String("http", envOr("CDF_UNIFIED_HTTP_ADDR", ":8085"), "HTTP listen address")
	grpcAddr := flag.String("grpc", envOr("CDF_UNIFIED_GRPC_ADDR", ":50055"), "gRPC listen address")
	flag.Parse()

	clients := NewClients()
	s := NewService(clients, *httpAddr, *grpcAddr)
	log.Printf("cdf-unified starting (embed=%s drift=%s router=%s)",
		clients.EmbedURL, clients.DriftURL, clients.RouterURL)
	log.Fatal(s.Run())
}
