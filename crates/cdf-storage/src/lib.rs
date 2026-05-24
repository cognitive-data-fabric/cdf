//! CDF Storage Engine — LSM-tree with WAL, poly-modal segments, and bitemporal support.

pub mod wal;
pub mod segment;
pub mod memtable;
pub mod compact;
pub mod engine;

pub use engine::{StorageEngine, EngineConfig};

pub type Result<T> = std::result::Result<T, cdf_common::CdfError>;

use cdf_common::{FabricId, PolyRow, TemporalBounds, Timestamp};
use serde::{Deserialize, Serialize};

/// Key for stored rows: table_name + row_id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RowKey {
    pub table: String,
    pub row_id: FabricId,
}

impl RowKey {
    pub fn new(table: impl Into<String>, row_id: FabricId) -> Self {
        Self {
            table: table.into(),
            row_id,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = self.table.as_bytes().to_vec();
        buf.push(0); // delimiter
        buf.extend_from_slice(self.row_id.0.as_bytes());
        buf
    }
}

/// Internal value representation with versioning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InternalValue {
    /// Active row with data.
    Active(PolyRow),
    /// Tombstone (deleted).
    Tombstone {
        row_id: FabricId,
        temporal: TemporalBounds,
    },
}

/// Immutable segment file metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentInfo {
    pub segment_id: u64,
    pub file_path: String,
    pub level: u32,
    pub key_range: (Vec<u8>, Vec<u8>),
    pub row_count: u64,
    pub size_bytes: u64,
    pub created_at: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_row_key_encoding() {
        let id = FabricId::nil();
        let key = RowKey::new("documents", id);
        let encoded = key.encode();
        assert!(encoded.len() > 16);
    }
}
