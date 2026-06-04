//! Main storage engine coordinating WAL, MemTable, and segments.

use crate::{memtable::MemTable, wal::WriteAheadLog, InternalValue, RowKey, SegmentInfo};
use cdf_common::{CdfError, FabricId, PolyRow, Result, TemporalBounds};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
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
    /// High-watermark sequence number. Updated on every successful WAL
    /// append (insert or delete).
    sequence: AtomicU64,
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
        // Recover sequence number from WAL so that we don't restart from 1
        // and overwrite existing entries' sequence numbers.
        let initial_seq = wal.next_seq().saturating_sub(1);

        let engine = Self {
            config,
            wal: Arc::new(RwLock::new(wal)),
            active_memtable: Arc::new(MemTable::new(64 * 1024 * 1024)),
            frozen_memtables: Arc::new(RwLock::new(Vec::new())),
            segments: Arc::new(RwLock::new(Vec::new())),
            sequence: AtomicU64::new(initial_seq),
        };

        Ok(engine)
    }

    pub async fn insert(&self, table: &str, row: PolyRow) -> Result<FabricId> {
        let row_id = row.id;
        let key = RowKey::new(table, row_id);
        let value = InternalValue::Active(row);

        // Serialize BEFORE acquiring the WAL lock so we don't hold the lock
        // across a CPU-bound encode. A failure here aborts the write cleanly.
        let data = bincode::serialize(&value).map_err(|e| CdfError::Serialization {
            format: "bincode".to_string(),
            reason: e.to_string(),
        })?;

        // 1. Append to WAL
        let seq = {
            let mut wal = self.wal.write().await;
            let op = crate::wal::WalOp::Insert {
                table: table.to_string(),
                row_id: key.row_id,
                data,
            };
            wal.append(op)?
        };

        // 2. Insert into MemTable
        self.active_memtable
            .insert(key, value)
            .map_err(|e| CdfError::Internal {
                component: "memtable".to_string(),
                reason: format!("{:?}", e),
            })?;

        // 3. Check if flush needed
        if self.active_memtable.at_capacity() {
            self.trigger_flush().await?;
        }

        self.sequence.store(seq, Ordering::Release);
        Ok(row_id)
    }

    pub async fn get(&self, table: &str, id: FabricId) -> Result<Option<PolyRow>> {
        let key = RowKey::new(table, id);

        // 1. Check active memtable
        if let Some(val) = self.active_memtable.get(&key) {
            return Ok(materialize(&val));
        }

        // 2. Check frozen memtables (newest first)
        {
            let frozen = self.frozen_memtables.read().await;
            for memtable in frozen.iter().rev() {
                if let Some(val) = memtable.get(&key) {
                    return Ok(materialize(&val));
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

        // 1. Append to WAL first (durable record of the delete)
        let seq = {
            let mut wal = self.wal.write().await;
            wal.append(crate::wal::WalOp::Delete {
                table: table.to_string(),
                row_id: id,
            })?
        };

        // 2. Insert tombstone into MemTable
        self.active_memtable
            .insert(key, tombstone)
            .map_err(|e| CdfError::Internal {
                component: "memtable".to_string(),
                reason: format!("{:?}", e),
            })?;

        self.sequence.store(seq, Ordering::Release);
        Ok(true)
    }

    pub async fn scan_table(&self, table: &str) -> Result<Vec<PolyRow>> {
        let prefix = format!("{}\0", table).into_bytes();
        let mut results = Vec::new();
        let mut seen: std::collections::HashSet<FabricId> = std::collections::HashSet::new();

        // Active memtable (newest data first)
        for (_, value) in self.active_memtable.scan(Some(&prefix), None) {
            if let Some(row) = materialize(&value) {
                if seen.insert(row.id) {
                    results.push(row);
                }
            }
        }

        // Frozen memtables (newest to oldest)
        let frozen = self.frozen_memtables.read().await;
        for memtable in frozen.iter().rev() {
            for (_, value) in memtable.scan(Some(&prefix), None) {
                if let Some(row) = materialize(&value) {
                    if seen.insert(row.id) {
                        results.push(row);
                    }
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
        self.sequence.load(Ordering::Acquire)
    }
}

/// Convert an InternalValue reference to an Optional PolyRow, treating
/// tombstones as deletes.
fn materialize(val: &InternalValue) -> Option<PolyRow> {
    match val {
        InternalValue::Active(row) => Some(row.clone()),
        InternalValue::Tombstone { .. } => None,
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

    #[tokio::test]
    async fn test_engine_sequence_advances() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = EngineConfig::default();
        config.data_dir = dir.path().to_path_buf();
        config.wal_sync = false;
        let engine = StorageEngine::open(config).unwrap();

        let initial = engine.current_sequence();
        let id = FabricId::new();
        engine
            .insert(
                "test",
                PolyRow {
                    id,
                    values: Default::default(),
                    temporal: TemporalBounds::new(),
                    version: 1,
                },
            )
            .await
            .unwrap();
        let after_insert = engine.current_sequence();
        assert!(after_insert > initial, "sequence must advance after insert");
    }

    #[tokio::test]
    async fn test_engine_scan_dedups() {
        // Inserting the same row id twice should return it once in a scan.
        let dir = tempfile::tempdir().unwrap();
        let mut config = EngineConfig::default();
        config.data_dir = dir.path().to_path_buf();
        config.wal_sync = false;
        let engine = StorageEngine::open(config).unwrap();

        let id = FabricId::new();
        for _ in 0..3 {
            engine
                .insert(
                    "test",
                    PolyRow {
                        id,
                        values: Default::default(),
                        temporal: TemporalBounds::new(),
                        version: 1,
                    },
                )
                .await
                .unwrap();
        }
        let rows = engine.scan_table("test").await.unwrap();
        assert_eq!(rows.len(), 1, "scan should dedup by row id");
        assert_eq!(rows[0].id, id);
    }
}
