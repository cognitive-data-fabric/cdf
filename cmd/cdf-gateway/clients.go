// Package main: client wrappers for downstream CDF services.
//
// These clients let the gateway actually talk to the router, embed service,
// drift detector, and meta service. They are deliberately small and use
// only stdlib + the proto-generated types so they remain easy to test.
package main

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"time"
)

// ServiceClients holds lazily-initialized HTTP clients for each downstream
// service. Timeouts and base URLs come from environment variables.
type ServiceClients struct {
	RouterURL  string
	MetaURL    string
	EmbedURL   string
	DriftURL   string
	UnifiedURL string
	HTTPClient *http.Client
}

// NewServiceClients builds a ServiceClients from environment variables.
// Defaults are wired for the docker-compose stack so the gateway can be
// run in dev mode without extra config.
func NewServiceClients() *ServiceClients {
	return &ServiceClients{
		RouterURL:  envOr("CDF_ROUTER_HTTP_URL", "http://router:50050"),
		MetaURL:    envOr("CDF_META_HTTP_URL", "http://meta:50054"),
		EmbedURL:   envOr("EMBED_URL", "http://embed-service:8001"),
		DriftURL:   envOr("DRIFT_URL", "http://drift-detector:8002"),
		UnifiedURL: envOr("CDF_UNIFIED_URL", "http://cdf-unified:8085"),
		HTTPClient: &http.Client{Timeout: 10 * time.Second},
	}
}

func envOr(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

// CallJSON issues an HTTP POST to a downstream service and decodes the
// JSON response into `out`. The `in` argument is encoded as JSON and sent
// as the request body. If `out` is nil, the response is discarded.
func (c *ServiceClients) CallJSON(
	ctx context.Context,
	method, url string,
	in, out interface{},
) error {
	var body io.Reader
	if in != nil {
		b, err := json.Marshal(in)
		if err != nil {
			return fmt.Errorf("marshal request: %w", err)
		}
		body = bytes.NewReader(b)
	}
	req, err := http.NewRequestWithContext(ctx, method, url, body)
	if err != nil {
		return fmt.Errorf("build request: %w", err)
	}
	if in != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	req.Header.Set("Accept", "application/json")
	resp, err := c.HTTPClient.Do(req)
	if err != nil {
		return fmt.Errorf("http call: %w", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 400 {
		b, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("downstream %s returned %d: %s", url, resp.StatusCode, string(b))
	}
	if out == nil {
		return nil
	}
	return json.NewDecoder(resp.Body).Decode(out)
}

// EmbedRequest / EmbedResponse mirror the embed-service contract.
type EmbedRequest struct {
	Texts     []string `json:"texts"`
	ModelID   string   `json:"model_id,omitempty"`
	Normalize bool     `json:"normalize"`
	BatchSize int      `json:"batch_size,omitempty"`
}

type EmbedResponse struct {
	Embeddings       [][]float32 `json:"embeddings"`
	ModelID          string      `json:"model_id"`
	Dimensions       int         `json:"dimensions"`
	ProcessingTimeMS float64     `json:"processing_time_ms"`
}

// EmbedTexts asks the embed service to embed one or more texts.
func (c *ServiceClients) EmbedTexts(ctx context.Context, texts []string) (*EmbedResponse, error) {
	req := EmbedRequest{
		Texts:     texts,
		ModelID:   "all-MiniLM-L6-v2",
		Normalize: true,
		BatchSize: 32,
	}
	var resp EmbedResponse
	if err := c.CallJSON(ctx, "POST", c.EmbedURL+"/embed", req, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// EmbedSingle is a convenience wrapper for the single-text endpoint.
func (c *ServiceClients) EmbedSingle(ctx context.Context, text string) ([]float32, error) {
	req := EmbedRequest{
		Texts:     []string{text},
		ModelID:   "all-MiniLM-L6-v2",
		Normalize: true,
	}
	var resp EmbedResponse
	if err := c.CallJSON(ctx, "POST", c.EmbedURL+"/embed", req, &resp); err != nil {
		return nil, err
	}
	if len(resp.Embeddings) == 0 {
		return nil, fmt.Errorf("embed service returned no embeddings")
	}
	return resp.Embeddings[0], nil
}

// DriftSnapshot is the request body for both /register-reference and
// /check-drift on the drift-detector service.
type DriftSnapshot struct {
	ConceptID  string      `json:"concept_id"`
	Embeddings [][]float32 `json:"embeddings"`
	Timestamp  float64     `json:"timestamp,omitempty"`
}

type DriftReport struct {
	ConceptID     string    `json:"concept_id"`
	DriftDetected bool      `json:"drift_detected"`
	DriftScore    float64   `json:"drift_score"`
	ReferenceMean []float32 `json:"reference_mean"`
	CurrentMean   []float32 `json:"current_mean"`
	PValue        *float64  `json:"p_value"`
	Method        string    `json:"method"`
}

// RegisterDriftReference registers a concept's reference distribution.
func (c *ServiceClients) RegisterDriftReference(ctx context.Context, snap DriftSnapshot) error {
	return c.CallJSON(ctx, "POST", c.DriftURL+"/register-reference", snap, nil)
}

// CheckDrift compares a current batch against a registered reference.
func (c *ServiceClients) CheckDrift(ctx context.Context, snap DriftSnapshot, threshold float64) (*DriftReport, error) {
	url := fmt.Sprintf("%s/check-drift?threshold=%f", c.DriftURL, threshold)
	var resp DriftReport
	if err := c.CallJSON(ctx, "POST", url, snap, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// RouterRequest is the body the gateway sends to the router's HTTP shim.
type RouterRequest struct {
	RequestID string                 `json:"request_id"`
	CQL       string                 `json:"cql"`
	Params    map[string]interface{} `json:"params,omitempty"`
	Namespace string                 `json:"namespace,omitempty"`
}

type RouterResponse struct {
	RequestID string                   `json:"request_id"`
	Status    string                   `json:"status"`
	Results   []map[string]interface{} `json:"results,omitempty"`
	Meta      map[string]int64         `json:"meta,omitempty"`
	Error     string                   `json:"error,omitempty"`
}

// RouteQuery forwards a CQL query to the router. If the router URL is
// unreachable, the error is returned so the gateway can decide whether
// to fall back to a degraded mode.
func (c *ServiceClients) RouteQuery(ctx context.Context, req RouterRequest) (*RouterResponse, error) {
	var resp RouterResponse
	if err := c.CallJSON(ctx, "POST", c.RouterURL+"/v1/route", req, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// MetaServiceStatus is the response from the meta service /status endpoint.
type MetaServiceStatus struct {
	Version string         `json:"version"`
	Nodes   []MetaNodeInfo `json:"nodes"`
	Shards  MetaShardStats `json:"shards"`
}

type MetaNodeInfo struct {
	NodeID     string  `json:"node_id"`
	Role       string  `json:"role"`
	Status     string  `json:"status"`
	ShardRange []int64 `json:"shard_range,omitempty"`
	CPUPercent float64 `json:"cpu_percent"`
	MemoryPct  float64 `json:"memory_percent"`
}

type MetaShardStats struct {
	Total       int  `json:"total"`
	Active      int  `json:"active"`
	Rebalancing bool `json:"rebalancing"`
}

// GetClusterStatus asks the meta service for current cluster status.
func (c *ServiceClients) GetClusterStatus(ctx context.Context) (*MetaServiceStatus, error) {
	var resp MetaServiceStatus
	if err := c.CallJSON(ctx, "GET", c.MetaURL+"/v1/cluster/status", nil, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}
