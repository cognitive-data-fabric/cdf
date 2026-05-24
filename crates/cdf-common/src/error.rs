//! Unified error types for the Cognitive Data Fabric.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The single error type used across all CDF components.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CdfError {
    // --- Storage errors ---
    #[serde(rename = "storage_corruption")]
    StorageCorruption { details: String },

    #[serde(rename = "storage_not_found")]
    StorageNotFound { key: String },

    #[serde(rename = "storage_full")]
    StorageFull,

    #[serde(rename = "wal_error")]
    WalError { reason: String },

    // --- Serialization errors ---
    #[serde(rename = "serialization")]
    Serialization { format: String, reason: String },

    // --- Vector errors ---
    #[serde(rename = "vector_dimension_mismatch")]
    VectorDimensionMismatch { expected: usize, got: usize },

    #[serde(rename = "vector_index_corrupted")]
    VectorIndexCorrupted { details: String },

    // --- Schema errors ---
    #[serde(rename = "schema_validation")]
    SchemaValidation { field: String, reason: String },

    #[serde(rename = "schema_conflict")]
    SchemaConflict {
        table: String,
        existing: String,
        proposed: String,
    },

    // --- Query errors ---
    #[serde(rename = "query_parse")]
    QueryParse { cql: String, reason: String },

    #[serde(rename = "query_plan")]
    QueryPlan { reason: String },

    #[serde(rename = "query_timeout")]
    QueryTimeout { elapsed_ms: u64 },

    // --- Distributed errors ---
    #[serde(rename = "network")]
    Network {
        target: String,
        operation: String,
        reason: String,
    },

    #[serde(rename = "quorum_unavailable")]
    QuorumUnavailable {
        required: usize,
        available: usize,
    },

    #[serde(rename = "shard_not_found")]
    ShardNotFound { shard_id: u64 },

    #[serde(rename = "node_unavailable")]
    NodeUnavailable { node_id: String },

    // --- Temporal errors ---
    #[serde(rename = "temporal_violation")]
    TemporalViolation { reason: String },

    // --- Graph errors ---
    #[serde(rename = "graph_cycle")]
    GraphCycle { path: Vec<String> },

    #[serde(rename = "graph_node_not_found")]
    GraphNodeNotFound { node_id: String },

    // --- Probabilistic errors ---
    #[serde(rename = "prob_invalid")]
    ProbInvalid { reason: String },

    // --- Embedding / AI errors ---
    #[serde(rename = "embed_model_unavailable")]
    EmbedModelUnavailable { model_id: String },

    #[serde(rename = "embed_generation_failed")]
    EmbedGenerationFailed { reason: String },

    // --- Generic / catch-all ---
    #[serde(rename = "internal")]
    Internal { component: String, reason: String },

    #[serde(rename = "not_implemented")]
    NotImplemented { feature: String },

    #[serde(rename = "invalid_argument")]
    InvalidArgument { name: String, value: String },
}

impl fmt::Display for CdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CdfError::StorageCorruption { details } => {
                write!(f, "storage corruption: {}", details)
            }
            CdfError::StorageNotFound { key } => write!(f, "key not found: {}", key),
            CdfError::StorageFull => write!(f, "storage full"),
            CdfError::WalError { reason } => write!(f, "WAL error: {}", reason),
            CdfError::Serialization { format, reason } => {
                write!(f, "{} serialization failed: {}", format, reason)
            }
            CdfError::VectorDimensionMismatch { expected, got } => {
                write!(
                    f,
                    "vector dimension mismatch: expected {}, got {}",
                    expected, got
                )
            }
            CdfError::VectorIndexCorrupted { details } => {
                write!(f, "vector index corrupted: {}", details)
            }
            CdfError::SchemaValidation { field, reason } => {
                write!(f, "schema validation failed for '{}': {}", field, reason)
            }
            CdfError::SchemaConflict {
                table,
                existing,
                proposed,
            } => {
                write!(
                    f,
                    "schema conflict in '{}': existing={}, proposed={}",
                    table, existing, proposed
                )
            }
            CdfError::QueryParse { cql, reason } => {
                write!(f, "failed to parse CQL '{}': {}", cql, reason)
            }
            CdfError::QueryPlan { reason } => write!(f, "query planning failed: {}", reason),
            CdfError::QueryTimeout { elapsed_ms } => {
                write!(f, "query timed out after {}ms", elapsed_ms)
            }
            CdfError::Network {
                target,
                operation,
                reason,
            } => {
                write!(
                    f,
                    "network error on {} during {}: {}",
                    target, operation, reason
                )
            }
            CdfError::QuorumUnavailable { required, available } => {
                write!(
                    f,
                    "quorum unavailable: required {}, available {}",
                    required, available
                )
            }
            CdfError::ShardNotFound { shard_id } => {
                write!(f, "shard {} not found", shard_id)
            }
            CdfError::NodeUnavailable { node_id } => {
                write!(f, "node {} unavailable", node_id)
            }
            CdfError::TemporalViolation { reason } => {
                write!(f, "temporal violation: {}", reason)
            }
            CdfError::GraphCycle { path } => {
                write!(f, "cycle detected: {}", path.join(" -> "))
            }
            CdfError::GraphNodeNotFound { node_id } => {
                write!(f, "graph node {} not found", node_id)
            }
            CdfError::ProbInvalid { reason } => {
                write!(f, "invalid probabilistic value: {}", reason)
            }
            CdfError::EmbedModelUnavailable { model_id } => {
                write!(f, "embedding model {} unavailable", model_id)
            }
            CdfError::EmbedGenerationFailed { reason } => {
                write!(f, "embedding generation failed: {}", reason)
            }
            CdfError::Internal { component, reason } => {
                write!(f, "internal error in {}: {}", component, reason)
            }
            CdfError::NotImplemented { feature } => {
                write!(f, "feature not implemented: {}", feature)
            }
            CdfError::InvalidArgument { name, value } => {
                write!(f, "invalid argument {}={}", name, value)
            }
        }
    }
}

impl std::error::Error for CdfError {}

/// Convenience Result extension for context.
pub trait ResultExt<T> {
    fn context(self, msg: impl Into<String>) -> Result<T, CdfError>;
    fn with_component(self, component: impl Into<String>) -> Result<T, CdfError>;
}

impl<T, E: std::fmt::Display> ResultExt<T> for Result<T, E> {
    fn context(self, msg: impl Into<String>) -> Result<T, CdfError> {
        self.map_err(|e| CdfError::Internal {
            component: msg.into(),
            reason: e.to_string(),
        })
    }

    fn with_component(self, component: impl Into<String>) -> Result<T, CdfError> {
        self.map_err(|e| CdfError::Internal {
            component: component.into(),
            reason: e.to_string(),
        })
    }
}

/// Ensure an option is Some, or return StorageNotFound.
pub fn ensure_found<T>(opt: Option<T>, key: impl Into<String>) -> Result<T, CdfError> {
    opt.ok_or_else(|| CdfError::StorageNotFound { key: key.into() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = CdfError::VectorDimensionMismatch {
            expected: 384,
            got: 512,
        };
        assert_eq!(
            err.to_string(),
            "vector dimension mismatch: expected 384, got 512"
        );
    }

    #[test]
    fn test_result_ext() {
        let r: Result<i32, std::io::Error> = Ok(42);
        assert_eq!(r.context("test").unwrap(), 42);
    }
}
