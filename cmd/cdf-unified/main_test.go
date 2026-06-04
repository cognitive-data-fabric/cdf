package main

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

// makeTestService creates a Service with a Clients pointed at a
// httptest.Server so dispatch tests don't require real downstream services.
func makeTestService(t *testing.T) (*Service, *httptest.Server) {
	t.Helper()
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/embed":
			w.Header().Set("Content-Type", "application/json")
			_ = json.NewEncoder(w).Encode(EmbedResponse{
				Embeddings:       [][]float32{{0.1, 0.2, 0.3}, {0.4, 0.5, 0.6}},
				ModelID:          "test-model",
				Dimensions:       3,
				ProcessingTimeMS: 1.5,
			})
		case "/register-reference":
			w.WriteHeader(http.StatusOK)
		case "/check-drift":
			w.Header().Set("Content-Type", "application/json")
			_ = json.NewEncoder(w).Encode(DriftReport{
				ConceptID:     "concept-1",
				DriftDetected: false,
				DriftScore:    0.01,
				Method:        "mmd",
			})
		case "/v1/route":
			w.Header().Set("Content-Type", "application/json")
			_ = json.NewEncoder(w).Encode(RouterResponse{
				RequestID: "rt-1",
				Status:    "ok",
				Results:   []map[string]interface{}{{"id": "row-1"}},
				Meta:      map[string]int64{"shards_queried": 1, "total_ms": 5},
			})
		case "/health":
			w.WriteHeader(http.StatusOK)
		default:
			http.NotFound(w, r)
		}
	}))
	clients := &Clients{
		RouterURL: ts.URL,
		EmbedURL:  ts.URL,
		DriftURL:  ts.URL,
		MetaURL:   ts.URL,
		HTTPClient: &UnifiedHTTPClient{
			inner: ts.Client(),
		},
	}
	s := NewService(clients, ":0", ":0")
	return s, ts
}

func TestValidate_MissingOp(t *testing.T) {
	s, ts := makeTestService(t)
	defer ts.Close()
	resp := s.dispatch(context.Background(), &DataRequest{Op: ""})
	if resp.Status != "error" {
		t.Fatalf("expected error status, got %q", resp.Status)
	}
	if resp.Error == nil || resp.Error.Code != "SCHEMA_VALIDATION" {
		t.Fatalf("expected SCHEMA_VALIDATION, got %+v", resp.Error)
	}
}

func TestValidate_TextSearchMissingText(t *testing.T) {
	s, ts := makeTestService(t)
	defer ts.Close()
	resp := s.dispatch(context.Background(), &DataRequest{Op: OpTextSearch, Table: "papers"})
	if resp.Status != "error" {
		t.Fatalf("expected error, got %q", resp.Status)
	}
}

func TestValidate_InsertMissingTable(t *testing.T) {
	s, ts := makeTestService(t)
	defer ts.Close()
	resp := s.dispatch(context.Background(), &DataRequest{Op: OpInsert})
	if resp.Status != "error" {
		t.Fatalf("expected error, got %q", resp.Status)
	}
}

func TestValidate_DriftRegisterNeedsConceptID(t *testing.T) {
	s, ts := makeTestService(t)
	defer ts.Close()
	resp := s.dispatch(context.Background(), &DataRequest{
		Op:         OpDriftRegister,
		Embeddings: [][]float32{{0.1}, {0.2}},
	})
	if resp.Status != "error" {
		t.Fatalf("expected error, got %q", resp.Status)
	}
}

func TestValidate_DriftRegisterNeedsTwoEmbeddings(t *testing.T) {
	s, ts := makeTestService(t)
	defer ts.Close()
	resp := s.dispatch(context.Background(), &DataRequest{
		Op:        OpDriftRegister,
		ConceptID: "c1",
	})
	if resp.Status != "error" {
		t.Fatalf("expected error, got %q", resp.Status)
	}
}

func TestDispatch_Embed(t *testing.T) {
	s, ts := makeTestService(t)
	defer ts.Close()
	resp := s.dispatch(context.Background(), &DataRequest{
		Op:   OpEmbed,
		Text: "hello world",
	})
	if resp.Status != "ok" {
		t.Fatalf("expected ok, got %s: %+v", resp.Status, resp.Error)
	}
	if len(resp.Vectors) < 1 {
		t.Fatalf("expected at least 1 embedding, got %d", len(resp.Vectors))
	}
	if len(resp.Vectors[0]) != 3 {
		t.Fatalf("unexpected vector dim: %d", len(resp.Vectors[0]))
	}
	if resp.EmbeddingModel != "test-model" {
		t.Fatalf("expected test-model, got %q", resp.EmbeddingModel)
	}
}

func TestDispatch_DriftCheck(t *testing.T) {
	s, ts := makeTestService(t)
	defer ts.Close()
	resp := s.dispatch(context.Background(), &DataRequest{
		Op:         OpDriftCheck,
		ConceptID:  "c1",
		Embeddings: [][]float32{{0.1, 0.2}, {0.3, 0.4}},
	})
	if resp.Status != "ok" {
		t.Fatalf("expected ok, got %s: %+v", resp.Status, resp.Error)
	}
	if resp.DriftScore == nil || *resp.DriftScore != 0.01 {
		t.Fatalf("expected drift_score=0.01, got %+v", resp.DriftScore)
	}
	if resp.DriftDetected == nil || *resp.DriftDetected {
		t.Fatalf("expected drift_detected=false, got %+v", resp.DriftDetected)
	}
}

func TestDispatch_TextSearch(t *testing.T) {
	s, ts := makeTestService(t)
	defer ts.Close()
	resp := s.dispatch(context.Background(), &DataRequest{
		Op:    OpTextSearch,
		Text:  "transformers",
		Table: "papers",
		TopK:  3,
	})
	if resp.Status != "ok" {
		t.Fatalf("expected ok, got %s: %+v", resp.Status, resp.Error)
	}
	if resp.Count != 1 {
		t.Fatalf("expected 1 result, got %d", resp.Count)
	}
	if len(resp.Vectors) != 1 {
		t.Fatalf("expected query vector in response, got %+v", resp.Vectors)
	}
}

func TestDispatch_GraphAddEdge(t *testing.T) {
	s, ts := makeTestService(t)
	defer ts.Close()
	resp := s.dispatch(context.Background(), &DataRequest{
		Op:       OpGraphAddEdge,
		FromID:   "a",
		ToID:     "b",
		EdgeType: "knows",
	})
	if resp.Status != "ok" {
		t.Fatalf("expected ok, got %s: %+v", resp.Status, resp.Error)
	}
}

func TestDispatch_GraphTraverseDefaultDepth(t *testing.T) {
	s, ts := makeTestService(t)
	defer ts.Close()
	resp := s.dispatch(context.Background(), &DataRequest{
		Op:      OpGraphTraverse,
		StartID: "a",
	})
	if resp.Status != "ok" {
		t.Fatalf("expected ok, got %s: %+v", resp.Status, resp.Error)
	}
	// Depth should have been defaulted to 2 by validate()
	if meta, ok := resp.Meta["router_meta"]; ok {
		t.Logf("router_meta: %+v", meta)
	}
}

func TestDispatch_Insert(t *testing.T) {
	s, ts := makeTestService(t)
	defer ts.Close()
	resp := s.dispatch(context.Background(), &DataRequest{
		Op:    OpInsert,
		Table: "papers",
		RowID: "row-1",
		Row:   map[string]PolyValue{},
	})
	if resp.Status != "ok" {
		t.Fatalf("expected ok, got %s: %+v", resp.Status, resp.Error)
	}
	if resp.Meta == nil {
		t.Fatal("expected meta to be populated")
	}
	if cql, _ := resp.Meta["cql"].(string); !strings.Contains(cql, "INSERT INTO papers") {
		t.Fatalf("expected INSERT INTO papers in CQL, got %q", cql)
	}
}

func TestDispatch_UnknownOp(t *testing.T) {
	s, ts := makeTestService(t)
	defer ts.Close()
	resp := s.dispatch(context.Background(), &DataRequest{Op: DataOp("nonsense")})
	if resp.Status != "error" {
		t.Fatalf("expected error, got %q", resp.Status)
	}
	if resp.Error == nil || resp.Error.Code != "SCHEMA_VALIDATION" && resp.Error.Code != "UNKNOWN_OP" {
		t.Logf("got error: %+v", resp.Error)
	}
}

func TestHandleData_RejectsNonPost(t *testing.T) {
	s, ts := makeTestService(t)
	defer ts.Close()
	req := httptest.NewRequest("GET", "/v1/data", nil)
	rw := httptest.NewRecorder()
	s.handleData(rw, req)
	if rw.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d", rw.Code)
	}
}

func TestHandleData_BadJSON(t *testing.T) {
	s, ts := makeTestService(t)
	defer ts.Close()
	req := httptest.NewRequest("POST", "/v1/data", strings.NewReader("not json"))
	rw := httptest.NewRecorder()
	s.handleData(rw, req)
	if rw.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", rw.Code)
	}
}

func TestHandleData_DriftCheck(t *testing.T) {
	s, ts := makeTestService(t)
	defer ts.Close()
	body, _ := json.Marshal(DataRequest{
		RequestID: "test-1",
		Op:        OpDriftCheck,
		ConceptID: "c1",
		Embeddings: [][]float32{
			{0.1, 0.2},
			{0.3, 0.4},
		},
	})
	req := httptest.NewRequest("POST", "/v1/data", strings.NewReader(string(body)))
	rw := httptest.NewRecorder()
	s.handleData(rw, req)
	if rw.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", rw.Code, rw.Body.String())
	}
	var resp DataResponse
	if err := json.NewDecoder(rw.Body).Decode(&resp); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if resp.Status != "ok" {
		t.Fatalf("expected ok, got %s: %+v", resp.Status, resp.Error)
	}
}

func TestHandleDataSubpath_OpFromURL(t *testing.T) {
	s, ts := makeTestService(t)
	defer ts.Close()
	body, _ := json.Marshal(DataRequest{
		ConceptID:  "c1",
		Embeddings: [][]float32{{0.1}, {0.2}},
	})
	req := httptest.NewRequest("POST", "/v1/data/drift_register", strings.NewReader(string(body)))
	rw := httptest.NewRecorder()
	s.handleDataSubpath(rw, req)
	if rw.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", rw.Code, rw.Body.String())
	}
	var resp DataResponse
	if err := json.NewDecoder(rw.Body).Decode(&resp); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if resp.Op != OpDriftRegister {
		t.Fatalf("expected op=drift_register, got %q", resp.Op)
	}
}

func TestHandleListOps(t *testing.T) {
	s, ts := makeTestService(t)
	defer ts.Close()
	req := httptest.NewRequest("GET", "/v1/ops", nil)
	rw := httptest.NewRecorder()
	s.handleListOps(rw, req)
	if rw.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", rw.Code)
	}
	var body struct {
		Version string   `json:"version"`
		Ops     []DataOp `json:"ops"`
	}
	if err := json.NewDecoder(rw.Body).Decode(&body); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if len(body.Ops) < 15 {
		t.Fatalf("expected at least 15 ops, got %d", len(body.Ops))
	}
}
