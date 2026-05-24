//! Main storage engine coordinating WAL, MemTable, and segments.

use crate::{memtable::MemTable, wal::WriteAheadLog, InternalValue, RowKey, SegmentInfo};
use cdf_common::{CdfError, FabricId, PolyRow, Result, TemporalBounds, Timestamp};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for the storage engine.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub data_dir: PathBuf,
    pub memtable_size: usize,
    pub wal_sync: bool,
    pub max_segment_size: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./cdf-data"),
            memtable_size: 64 * 1024 * 1024, // 64MB
            wal_sync: true,
            max_segment_size: 256 * 1024 * 1024, // 256MB
        }
    }
}

/// The core storage engine for a single node/shard.
pub struct StorageEngine {
    config: EngineConfig,
    wal: Arc<RwLock<WriteAheadLog>>,
    active_memtable: Arc<MemTable>,
    frozen_memtables: Arc<RwLock<Vec<Arc<MemTable>>>>,
    segments: Arc<RwLock<Vec<SegmentInfo>>>,
    sequence: std::sync::atomic::AtomicU64,
}

impl StorageEngine {
    pub fn open(config: EngineConfig) -> Result<Self> {
        std::fs::create_dir_all(&config.data_dir).map_err(|e| CdfError::Internal {
            component: "storage".to_string(),
            reason: e.to_string(),
        })?;

        let wal_path = config.data_dir.join("wal.active");
        let wal = WriteAheadLog::open(&wal_path, config.wal_sync).map_err(|e| {
            CdfError::WalError {
                reason: e.to_string(),
            }
        })?;

        let engine = Self {
            config,
            wal: Arc::new(RwLock::new(wal)),
            active_memtable: Arc::new(MemTable::new(64 * 1024 * 1024)),
            frozen_memtables: Arc::new(RwLock::new(Vec::new())),
            segments: Arc::new(RwLock::new(Vec::new())),
            sequence: std::sync::atomic::AtomicU64::new(1),
        };

        Ok(engine)
    }

    pub async fn insert(&self, table: &str, row: PolyRow) -> Result<FabricId> {
        let key = RowKey::new(table, row.id);
        let value = InternalValue::Active(row);

        // 1. Append to WAL
        let seq = {
            let mut wal = self.wal.write().await;
            let op = crate::wal::WalOp::Insert {
                table: table.to_string(),
                row_id: key.row_id,
                data: bincode::serialize(&value).unwrap_or_default(),
            };
            wal.append(op).map_err(|e| CdfError::WalError {
                reason: e.to_string(),
            })?
        };

        // 2. Insert into MemTable
        let _ = self
            .active_memtable
            .insert(key, value)
            .map_err(|e| CdfError::Internal {
                component: "memtable".to_string(),
                reason: format!("{:?}", e),
            })?;

        // 3. Check if flush needed
        if self.active_memtable.at_capacity() {
            self.trigger_flush().await?;
        }

        self.sequence
            .store(seq, std::sync::atomic::Ordering::Relaxed);
        Ok(key.row_id)
    }

    pub async fn get(&self, table: &str, id: FabricId) -> Result<Option<PolyRow>> {
        let key = RowKey::new(table, id);

        // 1. Check active memtable
        if let Some(val) = self.active_memtable.get(&key) {
            return match val {
                InternalValue::Active(row) => Ok(Some(row)),
                InternalValue::Tombstone { .. } => Ok(None),
            };
        }

        // 2. Check frozen memtables (newest first)
        {
            let frozen = self.frozen_memtables.read().await;
            for memtable in frozen.iter().rev() {
                if let Some(val) = memtable.get(&key) {
                    return match val {
                        InternalValue::Active(row) => Ok(Some(row)),
                        InternalValue::Tombstone { .. } => Ok(None),
                    };
                }
            }
        }

        // 3. Check on-disk segments (TODO: bloom filters, index)
        // Stub: would search segments here
        Ok(None)
    }

    pub async fn delete(&self, table: &str, id: FabricId) -> Result<bool> {
        let key = RowKey::new(table, id);
        let tombstone = InternalValue::Tombstone {
            row_id: id,
            temporal: TemporalBounds::new(),
        };

        let mut wal = self.wal.write().await;
        wal.append(crate::wal::WalOp::Delete {
            table: table.to_string(),
            row_id: id,
        })
        .map_err(|e| CdfError::WalError {
            reason: e.to_string(),
        })?;

        drop(wal);
        self.active_memtable
            .insert(key, tombstone)
            .map_err(|e| CdfError::Internal {
                component: "memtable".to_string(),
                reason: format!("{:?}", e),
            })?;

        Ok(true)
    }

    pub async fn scan_table(&self, table: &str) -> Result<Vec<PolyRow>> {
        let prefix = format!("{}\0", table).into_bytes();
        let mut results = Vec::new();

        // Active memtable
        for (_, value) in self.active_memtable.scan(Some(&prefix), None) {
            if let InternalValue::Active(row) = value {
                results.push(row);
            }
        }

        // Frozen memtables
        let frozen = self.frozen_memtables.read().await;
        for memtable in frozen.iter() {
            for (_, value) in memtable.scan(Some(&prefix), None) {
                if let InternalValue::Active(row) = value {
                    results.push(row);
                }
            }
        }

        // Segments (stub)
        Ok(results)
    }

    async fn trigger_flush(&self) -> Result<()> {
        // In production: freeze current memtable, spawn background flush task
        // For now: just log
        tracing::info!("MemTable at capacity, flush triggered");
        Ok(())
    }

    pub fn current_sequence(&self) -> u64 {
        self.sequence.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_engine_insert_and_get() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = EngineConfig::default();
        config.data_dir = dir.path().to_path_buf();
        config.wal_sync = false;

        let engine = StorageEngine::open(config).unwrap();
        let id = FabricId::new();
        let row = PolyRow {
            id,
            values: Default::default(),
            temporal: TemporalBounds::new(),
            version: 1,
        };

        let inserted = engine.insert("test", row.clone()).await.unwrap();
        assert_eq!(inserted, id);

        let fetched = engine.get("test", id).await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id, id);
    }

    #[tokio::test]
    async fn test_engine_delete() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = EngineConfig::default();
        config.data_dir = dir.path().to_path_buf();
        config.wal_sync = false;

        let engine = StorageEngine::open(config).unwrap();
        let id = FabricId::new();
        let row = PolyRow {
            id,
            values: Default::default(),
            temporal: TemporalBounds::new(),
            version: 1,
        };

        engine.insert("test", row).await.unwrap();
        engine.delete("test", id).await.unwrap();

        let fetched = engine.get("test", id).await.unwrap();
        assert!(fetched.is_none());
    }
}
