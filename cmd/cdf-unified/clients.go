// Client wrappers for the downstream services that cdf-unified forwards to.
//
// We avoid pulling in the gateway's `clients.go` so the unified service
// stays a small, self-contained binary.
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

// Clients holds the URLs and an HTTP client used to talk to downstream
// CDF services. Timeouts and base URLs come from environment variables.
type Clients struct {
	RouterURL  string
	EmbedURL   string
	DriftURL   string
	MetaURL    string
	HTTPClient *UnifiedHTTPClient
}

// UnifiedHTTPClient is a tiny wrapper that exposes GetWithContext so the
// health probe can use the same configured client as everything else.
type UnifiedHTTPClient struct {
	inner *http.Client
}

func (c *UnifiedHTTPClient) GetWithContext(ctx context.Context, url string) (*http.Response, error) {
	req, err := http.NewRequestWithContext(ctx, "GET", url, nil)
	if err != nil {
		return nil, err
	}
	return c.inner.Do(req)
}

func (c *UnifiedHTTPClient) Do(req *http.Request) (*http.Response, error) {
	return c.inner.Do(req)
}

// NewClients builds a Clients from env vars with sensible docker-compose defaults.
func NewClients() *Clients {
	return &Clients{
		RouterURL: envOr("CDF_ROUTER_HTTP_URL", "http://router:50050"),
		EmbedURL:  envOr("EMBED_URL", "http://embed-service:8001"),
		DriftURL:  envOr("DRIFT_URL", "http://drift-detector:8002"),
		MetaURL:   envOr("CDF_META_HTTP_URL", "http://meta:50054"),
		HTTPClient: &UnifiedHTTPClient{
			inner: &http.Client{Timeout: 10 * time.Second},
		},
	}
}

func envOr(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

// CallJSON is a small JSON POST helper.
func (c *Clients) CallJSON(
	ctx context.Context,
	method, url string,
	in, out interface{},
) error {
	var body io.Reader
	if in != nil {
		b, err := json.Marshal(in)
		if err != nil {
			return fmt.Errorf("marshal: %w", err)
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

// -----------------------------------------------------------------------------
// Downstream service contract types
// -----------------------------------------------------------------------------

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

// EmbedSingle returns the embedding for one text.
func (c *Clients) EmbedSingle(ctx context.Context, text string) ([]float32, error) {
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

// EmbedTexts returns embeddings for a batch of texts.
func (c *Clients) EmbedTexts(ctx context.Context, texts []string) (*EmbedResponse, error) {
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

func (c *Clients) RegisterDriftReference(ctx context.Context, snap DriftSnapshot) error {
	return c.CallJSON(ctx, "POST", c.DriftURL+"/register-reference", snap, nil)
}

func (c *Clients) CheckDrift(ctx context.Context, snap DriftSnapshot, threshold float64) (*DriftReport, error) {
	url := fmt.Sprintf("%s/check-drift?threshold=%f", c.DriftURL, threshold)
	var resp DriftReport
	if err := c.CallJSON(ctx, "POST", url, snap, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

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

func (c *Clients) RouteQuery(ctx context.Context, req RouterRequest) (*RouterResponse, error) {
	var resp RouterResponse
	if err := c.CallJSON(ctx, "POST", c.RouterURL+"/v1/route", req, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}
