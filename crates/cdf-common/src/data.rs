//! Unified data service types — common contract for ALL data injection
//! and retrieval in CDF.
//!
//! Instead of having per-operation endpoints with bespoke request/response
//! shapes, every client interacts with the fabric through a single
//! `DataRequest` / `DataResponse` envelope. The `op` field is the
//! discriminated union that selects which pipeline runs.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What the caller wants to do. The list is intentionally closed so the
/// service can validate at the boundary and route to the right pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataOp {
    // --- Storage ---
    Insert,
    BatchInsert,
    Get,
    Delete,
    Update,
    Scan,
    CreateTable,
    DropTable,
    // --- Vector / semantic ---
    VectorSearch,
    TextSearch,        // auto-embeds the text and runs vector search
    Embed,             // just returns the embedding(s), no storage
    // --- Graph ---
    GraphAddEdge,
    GraphRemoveEdge,
    GraphTraverse,
    GraphNeighbors,
    // --- Probability / drift ---
    DriftRegister,     // register a concept reference distribution
    DriftCheck,        // compare current batch to reference
    // --- Time travel ---
    TemporalQuery,     // AS OF / DURING
}

impl DataOp {
pub fn is_write(&self) -> bool {
matches!(
self,
DataOp::Insert
| DataOp::BatchInsert
| DataOp::Delete
| DataOp::Update
| DataOp::CreateTable
| DataOp::DropTable
| DataOp::GraphAddEdge
| DataOp::GraphRemoveEdge
| DataOp::DriftRegister
)
}

pub fn needs_table(&self) -> bool {
matches!(
self,
DataOp::Insert
| DataOp::BatchInsert
| DataOp::Get
| DataOp::Delete
| DataOp::Update
| DataOp::Scan
| DataOp::VectorSearch
| DataOp::TextSearch
)
}
}

/// Single request envelope used by `POST /v1/data` and the `Data` gRPC method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRequest {
/// Stable client-supplied ID for tracing / idempotency.
#[serde(default)]
pub request_id: String,
/// Operation to perform.
pub op: DataOp,
/// Logical namespace. Optional; defaults to "default".
#[serde(default)]
pub namespace: String,
/// Target table / collection. Required for table-scoped ops.
#[serde(default)]
pub table: String,
/// Single row payload for `Insert` / `Update` / `Get` / `Delete`.
#[serde(default)]
pub row: BTreeMap<String, PolyValue>,
/// Row id for `Get` / `Delete` / `Update`.
#[serde(default)]
pub row_id: String,
/// Batch of rows for `BatchInsert`.
#[serde(default)]
pub rows: Vec<BTreeMap<String, PolyValue>>,
/// Vector for `VectorSearch`.
#[serde(default)]
pub vector: Vec<f32>,
/// Text for `TextSearch` or `Embed`.
#[serde(default)]
pub text: String,
/// Multiple texts for batched `Embed`.
#[serde(default)]
pub texts: Vec<String>,
/// Top-K for search ops.
#[serde(default)]
pub top_k: usize,
/// Threshold for search ops.
#[serde(default)]
pub threshold: Option<f32>,
/// Concept id for `Drift*` ops.
#[serde(default)]
pub concept_id: String,
/// Embedding batch for `Drift*` ops.
#[serde(default)]
pub embeddings: Vec<Vec<f32>>,
/// Graph source / target.
#[serde(default)]
pub from_id: String,
#[serde(default)]
pub to_id: String,
#[serde(default)]
pub edge_type: String,
#[serde(default)]
pub start_id: String,
#[serde(default)]
pub node_id: String,
#[serde(default)]
pub depth: usize,
/// CQL query string (escape hatch for ops not yet in the union).
#[serde(default)]
pub cql: String,
/// Free-form parameters bag.
#[serde(default)]
pub params: BTreeMap<String, serde_json::Value>,
/// Per-op extras (e.g. drift threshold, search metric).
#[serde(default)]
pub options: BTreeMap<String, serde_json::Value>,
}

/// Normalized response envelope. Every operation returns this same shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataResponse {
/// Echoes the request id (or generates one if client didn't supply).
pub request_id: String,
/// Operation that was performed.
pub op: DataOp,
/// "ok" | "error" | "partial"
pub status: String,
/// Single row, if the op produced one.
#[serde(default)]
pub row: Option<BTreeMap<String, PolyValue>>,
/// Batch of rows, if the op produced many.
#[serde(default)]
pub rows: Vec<BTreeMap<String, PolyValue>>,
/// Vector(s) for embed / search results.
#[serde(default)]
pub vectors: Vec<Vec<f32>>,
/// Scalar count.
#[serde(default)]
pub count: usize,
/// Drift score, when applicable.
#[serde(default)]
pub drift_score: Option<f64>,
/// Embedding model id used, when applicable.
#[serde(default)]
pub embedding_model: Option<String>,
/// Numeric / structured metadata.
#[serde(default)]
pub meta: BTreeMap<String, serde_json::Value>,
/// Error, if `status == "error"`.
#[serde(default)]
pub error: Option<DataError>,
}

impl DataResponse {
pub fn ok(req: &DataRequest) -> Self {
Self {
request_id: req.request_id.clone(),
op: req.op,
status: "ok".to_string(),
row: None,
rows: Vec::new(),
vectors: Vec::new(),
count: 0,
drift_score: None,
embedding_model: None,
meta: BTreeMap::new(),
error: None,
}
}

pub fn error(req: &DataRequest, code: &str, msg: impl Into<String>) -> Self {
Self {
request_id: req.request_id.clone(),
op: req.op,
status: "error".to_string(),
row: None,
rows: Vec::new(),
vectors: Vec::new(),
count: 0,
drift_score: None,
embedding_model: None,
meta: BTreeMap::new(),
error: Some(DataError {
code: code.to_string(),
message: msg.into(),
}),
}
}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataError {
pub code: String,
pub message: String,
}

/// Re-export of the poly-modal value from this crate's root so the
/// unified types can be `serde`-derived without extra dependencies.
pub use crate::PolyValue;

#[cfg(test)]
mod tests {
use super::*;

#[test]
fn test_data_op_is_write() {
assert!(DataOp::Insert.is_write());
assert!(DataOp::BatchInsert.is_write());
assert!(DataOp::CreateTable.is_write());
assert!(!DataOp::Get.is_write());
assert!(!DataOp::Embed.is_write());
assert!(!DataOp::TextSearch.is_write());
}

#[test]
fn test_data_op_needs_table() {
assert!(DataOp::Insert.needs_table());
assert!(DataOp::TextSearch.needs_table());
assert!(!DataOp::Embed.needs_table());
assert!(!DataOp::DriftRegister.needs_table());
}

#[test]
fn test_data_request_serialize() {
let req = DataRequest {
request_id: "req-1".to_string(),
op: DataOp::TextSearch,
namespace: "default".to_string(),
table: "papers".to_string(),
row: BTreeMap::new(),
row_id: "".to_string(),
rows: vec![],
vector: vec![],
text: "transformers".to_string(),
texts: vec![],
top_k: 10,
threshold: None,
concept_id: "".to_string(),
embeddings: vec![],
from_id: "".to_string(),
to_id: "".to_string(),
edge_type: "".to_string(),
start_id: "".to_string(),
node_id: "".to_string(),
depth: 0,
cql: "".to_string(),
params: BTreeMap::new(),
options: BTreeMap::new(),
};
let json = serde_json::to_string(&req).unwrap();
assert!(json.contains("\"op\":\"text_search\""));
assert!(json.contains("\"table\":\"papers\""));
assert!(json.contains("\"text\":\"transformers\""));
}

#[test]
fn test_data_request_deserialize() {
let json = r#"{"op":"drift_check","concept_id":"c1","embeddings":[[0.1,0.2],[0.3,0.4]]}"#;
let req: DataRequest = serde_json::from_str(json).unwrap();
assert_eq!(req.op, DataOp::DriftCheck);
assert_eq!(req.concept_id, "c1");
assert_eq!(req.embeddings.len(), 2);
}

#[test]
fn test_data_response_ok() {
let req = DataRequest {
request_id: "req-1".to_string(),
op: DataOp::Embed,
namespace: "".to_string(),
table: "".to_string(),
row: BTreeMap::new(),
row_id: "".to_string(),
rows: vec![],
vector: vec![],
text: "hi".to_string(),
texts: vec![],
top_k: 0,
threshold: None,
concept_id: "".to_string(),
embeddings: vec![],
from_id: "".to_string(),
to_id: "".to_string(),
edge_type: "".to_string(),
start_id: "".to_string(),
node_id: "".to_string(),
depth: 0,
cql: "".to_string(),
params: BTreeMap::new(),
options: BTreeMap::new(),
};
let resp = DataResponse::ok(&req);
assert_eq!(resp.status, "ok");
assert_eq!(resp.op, DataOp::Embed);
assert_eq!(resp.request_id, "req-1");
assert!(resp.error.is_none());
}

#[test]
fn test_data_response_error() {
let req = DataRequest {
request_id: "req-1".to_string(),
op: DataOp::TextSearch,
namespace: "".to_string(),
table: "".to_string(),
row: BTreeMap::new(),
row_id: "".to_string(),
rows: vec![],
vector: vec![],
text: "".to_string(),
texts: vec![],
top_k: 0,
threshold: None,
concept_id: "".to_string(),
embeddings: vec![],
from_id: "".to_string(),
to_id: "".to_string(),
edge_type: "".to_string(),
start_id: "".to_string(),
node_id: "".to_string(),
depth: 0,
cql: "".to_string(),
params: BTreeMap::new(),
options: BTreeMap::new(),
};
let resp = DataResponse::error(&req, "SCHEMA_VALIDATION", "text is required");
assert_eq!(resp.status, "error");
let err = resp.error.unwrap();
assert_eq!(err.code, "SCHEMA_VALIDATION");
assert_eq!(err.message, "text is required");
}

#[test]
fn test_data_op_equality() {
assert_eq!(DataOp::Insert, DataOp::Insert);
assert_ne!(DataOp::Insert, DataOp::Delete);
}

#[test]
fn test_data_op_hash() {
use std::collections::HashSet;
let mut s = HashSet::new();
s.insert(DataOp::Insert);
s.insert(DataOp::Delete);
s.insert(DataOp::Insert);
assert_eq!(s.len(), 2);
}
}
