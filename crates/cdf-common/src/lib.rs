//! Cognitive Data Fabric — Common Types and Utilities
//! Foundation crate for all CDF components.

pub mod error;
pub mod schema;
pub mod temporal;
pub mod prob;
pub mod vector;

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier across the entire fabric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FabricId(pub uuid::Uuid);

impl FabricId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub fn nil() -> Self {
        Self(uuid::Uuid::nil())
    }
}

impl Default for FabricId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for FabricId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Content-addressed hash using BLAKE3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentHash {
    pub hash: [u8; 32],
}

impl ContentHash {
    pub fn from_bytes(data: &[u8]) -> Self {
        let digest = blake3::hash(data);
        Self {
            hash: *digest.as_bytes(),
        }
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.hash)
    }
}

/// Poly-modal value — the core data type of CDF.
/// Can represent scalars, text, embeddings, tensors, distributions, or blob references.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum PolyValue {
    Null,
    Bool(bool),
    Int64(i64),
    Float64(f64),
    Text(String),
    Bytes(Vec<u8>),
    Embedding(Embedding),
    Tensor(Tensor),
    Distribution(ProbabilisticValue),
    BlobRef(ContentHash),
    GraphEdge(GraphEdgeRef),
}

/// High-dimensional vector embedding with model metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Embedding {
    pub values: Vec<f32>,
    pub model_id: String,
    pub dimensions: usize,
}

impl Embedding {
    pub fn new(values: Vec<f32>, model_id: impl Into<String>) -> Self {
        let dimensions = values.len();
        Self {
            values,
            model_id: model_id.into(),
            dimensions,
        }
    }

    pub fn magnitude(&self) -> f32 {
        self.values.iter().map(|v| v * v).sum::<f32>().sqrt()
    }

    pub fn normalize(&self) -> Self {
        let mag = self.magnitude();
        if mag == 0.0 {
            return self.clone();
        }
        Self {
            values: self.values.iter().map(|v| v / mag).collect(),
            model_id: self.model_id.clone(),
            dimensions: self.dimensions,
        }
    }
}

/// Multi-dimensional tensor for attention maps, feature maps, etc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tensor {
    pub data: Vec<f32>,
    pub shape: Vec<u32>,
    pub dtype: TensorDtype,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TensorDtype {
    Float32,
    Float64,
    Int32,
    Int64,
}

/// Reference to a graph edge.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GraphEdgeRef {
    pub from_id: FabricId,
    pub to_id: FabricId,
    pub edge_type: String,
    pub properties: serde_json::Map<String, serde_json::Value>,
}

/// Node identity in the fabric (can be entity, document, concept, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Node {
    pub id: FabricId,
    pub labels: Vec<String>,
    pub properties: serde_json::Map<String, serde_json::Value>,
}

/// A complete poly-modal row in a table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolyRow {
    pub id: FabricId,
    pub values: std::collections::HashMap<String, PolyValue>,
    pub temporal: temporal::TemporalBounds,
    pub version: u64,
}

/// Consistency levels for distributed operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConsistencyLevel {
    Eventual,
    Session,
    BoundedStaleness { max_lag_ms: u64 },
    Strong,
}

/// Cluster node identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: FabricId,
    pub address: String,
    pub role: NodeRole,
    pub shard_range: Option<(u64, u64)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeRole {
    Storage,
    Router,
    Meta,
    Embedder,
    Gateway,
}

/// Sequence number for ordered operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SequenceNumber(pub u64);

/// Result type alias using CDF's unified error.
pub type Result<T> = std::result::Result<T, error::CdfError>;

// Re-export commonly used items
pub use error::CdfError;
pub use prob::ProbabilisticValue;
pub use temporal::{TemporalBounds, TemporalQuery, Timestamp};
pub use vector::{DistanceMetric, Neighbor, VectorOps};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fabric_id_display() {
        let id = FabricId::nil();
        assert_eq!(id.to_string(), "00000000-0000-0000-0000-000000000000");
    }

    #[test]
    fn test_content_hash() {
        let hash = ContentHash::from_bytes(b"hello world");
        assert_eq!(hash.hash.len(), 32);
        assert_eq!(hash.to_hex().len(), 64);
    }

    #[test]
    fn test_embedding_normalize() {
        let emb = Embedding::new(vec![3.0, 4.0], "test-model");
        let norm = emb.normalize();
        assert!((norm.magnitude() - 1.0).abs() < 1e-6);
    }
}
