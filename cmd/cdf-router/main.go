package main

import (
	"context"
	"fmt"
	"log"
	"net"
	"os"
	"strings"
	"sync"

	"google.golang.org/grpc"
	"google.golang.org/grpc/reflection"
)

// CQL Query structures
type CQLQuery struct {
	Select   []string
	From     string
	Where    *WhereClause
	Vector   *VectorClause
	Temporal *TemporalClause
	OrderBy  []OrderByExpr
	Limit    int
}

type WhereClause struct {
	Conditions []Condition
	CombineOp  string // AND | OR
}

type Condition struct {
	Column   string
	Operator string
	Value    interface{}
}

type VectorClause struct {
	Column    string
	QueryVec  []float32
	Threshold float32
	TopK      int
}

type TemporalClause struct {
	AsOfValid       int64
	AsOfTransaction int64
}

type OrderByExpr struct {
	Column string
	Asc    bool
}

// ShardRouter distributes queries across storage nodes
type ShardRouter struct {
	mu       sync.RWMutex
	shardMap map[uint64]string // shard_id -> node_address
	schema   map[string]*TableSchema
}

type TableSchema struct {
	Name    string
	Columns map[string]ColumnType
}

type ColumnType string

const (
	TypeScalar    ColumnType = "scalar"
	TypeVector    ColumnType = "vector"
	TypeGraphEdge ColumnType = "graph_edge"
	TypeTemporal  ColumnType = "temporal"
)

func NewShardRouter() *ShardRouter {
	return &ShardRouter{
		shardMap: make(map[uint64]string),
		schema:   make(map[string]*TableSchema),
	}
}

func (r *ShardRouter) RegisterShard(shardID uint64, nodeAddr string) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.shardMap[shardID] = nodeAddr
}

func (r *ShardRouter) RouteQuery(query *CQLQuery) ([]string, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()

	// Simple hash-based routing by table name
	// In production: consistent hashing on shard key
	shardCount := uint64(len(r.shardMap))
	if shardCount == 0 {
		return nil, fmt.Errorf("no shards available")
	}

	targetShards := make(map[uint64]bool)
	shardKey := hashString(query.From)
	targetShards[shardKey%shardCount] = true

	// If vector query, also route to shards with similar vector ranges
	if query.Vector != nil {
		// In production: use vector-aware routing (LSH/space partitioning)
		// For now: broadcast to all shards
		for i := uint64(0); i < shardCount; i++ {
			targetShards[i] = true
		}
	}

	var nodes []string
	for shardID := range targetShards {
		if addr, ok := r.shardMap[shardID]; ok {
			nodes = append(nodes, addr)
		}
	}
	return nodes, nil
}

func (r *ShardRouter) ParseCQL(cql string) (*CQLQuery, error) {
	// Simplified parser - production would use proper grammar (ANTLR/PEG)
	query := &CQLQuery{}
	upper := strings.ToUpper(cql)

	// Extract FROM
	if idx := strings.Index(upper, "FROM"); idx >= 0 {
		rest := strings.TrimSpace(cql[idx+4:])
		if spaceIdx := strings.Index(rest, " "); spaceIdx >= 0 {
			query.From = strings.Trim(rest[:spaceIdx], " \t\r\n;")
		} else {
			query.From = strings.Trim(rest, " \t\r\n;")
		}
	}

	// Extract SELECT columns
	if idx := strings.Index(upper, "SELECT"); idx >= 0 {
		fromIdx := strings.Index(upper, "FROM")
		if fromIdx > idx {
			cols := cql[idx+6 : fromIdx]
			for _, c := range strings.Split(cols, ",") {
				query.Select = append(query.Select, strings.TrimSpace(c))
			}
		}
	}

	// Extract WHERE
	if idx := strings.Index(upper, "WHERE"); idx >= 0 {
		query.Where = &WhereClause{CombineOp: "AND"}
		// Simplified: just detect conditions
		rest := cql[idx+5:]
		conds := strings.Split(rest, "AND")
		for _, cond := range conds {
			cond = strings.TrimSpace(cond)
			parts := strings.Fields(cond)
			if len(parts) >= 3 {
				query.Where.Conditions = append(query.Where.Conditions, Condition{
					Column:   parts[0],
					Operator: parts[1],
				})
			}
		}
	}

	// Extract LIMIT
	if idx := strings.Index(upper, "LIMIT"); idx >= 0 {
		var limit int
		rest := cql[idx+5:]
		fmt.Sscanf(rest, "%d", &limit)
		query.Limit = limit
	}

	return query, nil
}

func hashString(s string) uint64 {
	var h uint64 = 14695981039346656037
	for i := 0; i < len(s); i++ {
		h ^= uint64(s[i])
		h *= 1099511628211
	}
	return h
}

// QueryServer implements the gRPC query service
type QueryServer struct{}

func (s *QueryServer) ExecuteQuery(ctx context.Context, req *QueryRequest) (*QueryResponse, error) {
	// Would parse CQL, plan, route, and aggregate results
	return &QueryResponse{
		Status: "ok",
		Rows:   []byte(`{"result":"stub"}`),
	}, nil
}

type QueryRequest struct {
	CQL    string
	Params map[string]interface{}
}

type QueryResponse struct {
	Status string
	Rows   []byte
	Error  string
}

func main() {
	router := NewShardRouter()

	// Read storage nodes from environment
	storageNodesEnv := os.Getenv("CDF_STORAGE_NODES")
	if storageNodesEnv == "" {
		storageNodesEnv = "localhost:50051,localhost:50052"
	}

	var shardID uint64
	for _, addr := range strings.Split(storageNodesEnv, ",") {
		addr = strings.TrimSpace(addr)
		if addr != "" {
			router.RegisterShard(shardID, addr)
			shardID++
		}
	}
	log.Printf("Registered %d shards", shardID)

	// Test parsing
	query, err := router.ParseCQL("SELECT id, embedding FROM documents WHERE type = 'pdf' LIMIT 10")
	if err != nil {
		log.Fatal(err)
	}
	log.Printf("Parsed query: FROM=%s, cols=%v", query.From, query.Select)

	nodes, err := router.RouteQuery(query)
	if err != nil {
		log.Fatal(err)
	}
	log.Printf("Route to nodes: %v", nodes)

	// Start gRPC server
	port := os.Getenv("CDF_ROUTER_PORT")
	if port == "" {
		port = "50050"
	}
	lis, err := net.Listen("tcp", ":"+port)
	if err != nil {
		log.Fatalf("failed to listen: %v", err)
	}
	s := grpc.NewServer()
	reflection.Register(s)
	log.Printf("Query Router listening on %s", lis.Addr())
	if err := s.Serve(lis); err != nil {
		log.Fatalf("failed to serve: %v", err)
	}
}
