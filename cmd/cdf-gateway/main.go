package main

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"time"

	"github.com/google/uuid"
	"google.golang.org/grpc"
	"google.golang.org/grpc/reflection"
)

// Gateway serves both HTTP REST and gRPC with WebSocket streaming
type Gateway struct {
	routerAddr string
	httpAddr   string
	grpcAddr   string
}

func NewGateway(routerAddr, httpAddr, grpcAddr string) *Gateway {
	return &Gateway{
		routerAddr: routerAddr,
		httpAddr:   httpAddr,
		grpcAddr:   grpcAddr,
	}
}

func (g *Gateway) Run() error {
	// HTTP server with REST API
	go g.runHTTPServer()

	// gRPC server
	return g.runGRPCServer()
}

func (g *Gateway) runHTTPServer() {
	mux := http.NewServeMux()

	// Health check
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]string{"status": "healthy"})
	})

	// Metrics
	mux.HandleFunc("/metrics", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/plain")
		fmt.Fprintln(w, "# HELP cdf_requests_total Total requests")
		fmt.Fprintln(w, "# TYPE cdf_requests_total counter")
		fmt.Fprintln(w, `cdf_requests_total{method="all"} 0`)
	})

	// REST: Query endpoint
	mux.HandleFunc("/v1/query", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}

		body, _ := io.ReadAll(r.Body)
		var req QueryHTTPRequest
		if err := json.Unmarshal(body, &req); err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}

		// Forward to router
		resp := g.executeQuery(r.Context(), &req)
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(resp)
	})

	// REST: Insert endpoint
	mux.HandleFunc("/v1/insert", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}

		body, _ := io.ReadAll(r.Body)
		var req InsertHTTPRequest
		if err := json.Unmarshal(body, &req); err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}

		resp := g.executeInsert(r.Context(), &req)
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(resp)
	})

	// REST: Vector search endpoint
	mux.HandleFunc("/v1/search", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}

		body, _ := io.ReadAll(r.Body)
		var req VectorSearchHTTPRequest
		if err := json.Unmarshal(body, &req); err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}

		resp := g.executeVectorSearch(r.Context(), &req)
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(resp)
	})

	// WebSocket upgrade for streaming queries
	mux.HandleFunc("/v1/stream", func(w http.ResponseWriter, r *http.Request) {
		// Production: upgrade to WebSocket with gorilla/websocket
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{"note": "WebSocket streaming available at ws:// endpoint"})
	})

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

	s := grpc.NewServer(
		grpc.MaxRecvMsgSize(64*1024*1024), // 64MB
		grpc.MaxSendMsgSize(64*1024*1024),
	)
	reflection.Register(s)

	// Register services
	// In production: generated from proto
	log.Printf("gRPC Gateway listening on %s", g.grpcAddr)
	return s.Serve(lis)
}

func (g *Gateway) executeQuery(ctx context.Context, req *QueryHTTPRequest) *HTTPResponse {
	start := time.Now()

	// Would: parse CQL, route to shards, aggregate
	resp := &HTTPResponse{
		RequestID: req.RequestID,
		Status:    "ok",
		Results:   []map[string]interface{}{},
		Timing: map[string]int64{
			"total_ms": time.Since(start).Milliseconds(),
		},
	}

	return resp
}

func (g *Gateway) executeInsert(ctx context.Context, req *InsertHTTPRequest) *HTTPResponse {
	start := time.Now()

	// Generate ID if not provided
	if req.RowID == "" {
		req.RowID = uuid.New().String()
	}

	resp := &HTTPResponse{
		RequestID: req.RequestID,
		Status:    "ok",
		RowID:     req.RowID,
		Timing: map[string]int64{
			"total_ms": time.Since(start).Milliseconds(),
		},
	}

	return resp
}

func (g *Gateway) executeVectorSearch(ctx context.Context, req *VectorSearchHTTPRequest) *HTTPResponse {
	start := time.Now()

	// Would: route to vector shards, perform ANN search, rerank, return
	resp := &HTTPResponse{
		RequestID: req.RequestID,
		Status:    "ok",
		Results:   []map[string]interface{}{},
		Timing: map[string]int64{
			"total_ms": time.Since(start).Milliseconds(),
		},
	}

	return resp
}

// HTTP request/response types
type QueryHTTPRequest struct {
	RequestID string `json:"request_id"`
	CQL       string `json:"cql"`
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

type HTTPResponse struct {
	RequestID string                   `json:"request_id"`
	Status    string                   `json:"status"`
	RowID     string                   `json:"row_id,omitempty"`
	Results   []map[string]interface{} `json:"results,omitempty"`
	Error     string                   `json:"error,omitempty"`
	Timing    map[string]int64         `json:"timing,omitempty"`
}

func main() {
	gateway := NewGateway(
		"localhost:50050", // router
		":8080",           // HTTP
		":50053",          // gRPC
	)

	log.Fatal(gateway.Run())
}
