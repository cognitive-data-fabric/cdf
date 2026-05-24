package main

import (
	"context"
	"fmt"
	"log"
	"net"
	"sync"

	"google.golang.org/grpc"
	"google.golang.org/grpc/reflection"
)

// MetaService manages cluster membership, schema registry, and node health
type MetaService struct {
	mu          sync.RWMutex
	members     map[string]*NodeMember
	schemas     map[string]*TableSchema
	leader      string
	term        uint64
}

type NodeMember struct {
	ID       string `json:"id"`
	Address  string `json:"address"`
	Role     string `json:"role"`
	Status   string `json:"status"`
	LastSeen int64  `json:"last_seen"`
}

type TableSchema struct {
	Name    string            `json:"name"`
	Version uint64            `json:"version"`
	Columns map[string]string `json:"columns"`
	Indexes []string          `json:"indexes"`
}

func NewMetaService() *MetaService {
	return &MetaService{
		members: make(map[string]*NodeMember),
		schemas: make(map[string]*TableSchema),
	}
}

func (m *MetaService) RegisterNode(ctx context.Context, req *RegisterRequest) (*RegisterResponse, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	m.members[req.NodeID] = &NodeMember{
		ID:       req.NodeID,
		Address:  req.Address,
		Role:     req.Role,
		Status:   "healthy",
		LastSeen: req.Timestamp,
	}

	log.Printf("Node registered: %s at %s (%s)", req.NodeID, req.Address, req.Role)

	return &RegisterResponse{
		AssignedShard: hashShard(req.NodeID),
		Leader:        m.leader,
	}, nil
}

func (m *MetaService) Heartbeat(ctx context.Context, req *HeartbeatRequest) (*HeartbeatResponse, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	if member, ok := m.members[req.NodeID]; ok {
		member.LastSeen = req.Timestamp
		member.Status = "healthy"
	}

	return &HeartbeatResponse{
		Leader:    m.leader,
		Term:      m.term,
		ConfIndex: 0,
	}, nil
}

func (m *MetaService) GetMembers() []*NodeMember {
	m.mu.RLock()
	defer m.mu.RUnlock()

	result := make([]*NodeMember, 0, len(m.members))
	for _, m := range m.members {
		result = append(result, m)
	}
	return result
}

func (m *MetaService) RegisterSchema(ctx context.Context, req *SchemaRequest) (*SchemaResponse, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	m.schemas[req.Name] = &TableSchema{
		Name:    req.Name,
		Version:   req.Version,
		Columns:   req.Columns,
		Indexes:   req.Indexes,
	}

	return &SchemaResponse{Status: "registered"}, nil
}

func (m *MetaService) GetSchema(name string) (*TableSchema, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	if schema, ok := m.schemas[name]; ok {
		return schema, nil
	}
	return nil, fmt.Errorf("schema not found: %s", name)
}

// gRPC request/response types
type RegisterRequest struct {
	NodeID    string
	Address   string
	Role      string
	Timestamp int64
}

type RegisterResponse struct {
	AssignedShard uint64
	Leader        string
}

type HeartbeatRequest struct {
	NodeID    string
	Timestamp int64
	Stats     map[string]float64
}

type HeartbeatResponse struct {
	Leader    string
	Term      uint64
	ConfIndex uint64
}

type SchemaRequest struct {
	Name    string
	Version uint64
	Columns map[string]string
	Indexes []string
}

type SchemaResponse struct {
	Status string
}

func hashShard(nodeID string) uint64 {
	var h uint64 = 14695981039346656037
	for i := 0; i < len(nodeID); i++ {
		h ^= uint64(nodeID[i])
		h *= 1099511628211
	}
	return h
}

type MetaServer struct{}

func (s *MetaServer) RegisterNode(ctx context.Context, req *RegisterRequest) (*RegisterResponse, error) {
	// Implementation would delegate to MetaService
	return &RegisterResponse{}, nil
}

func main() {
	meta := NewMetaService()
	meta.leader = "meta-1"

	lis, err := net.Listen("tcp", ":50054")
	if err != nil {
		log.Fatalf("failed to listen: %v", err)
	}

	s := grpc.NewServer()
	reflection.Register(s)

	log.Printf("Meta Service listening on %s", lis.Addr())
	if err := s.Serve(lis); err != nil {
		log.Fatalf("failed to serve: %v", err)
	}
}
