//! In-memory sorted buffer for recent writes before flushing to disk.

use crate::{InternalValue, RowKey};
use std::collections::BTreeMap;

/// Thread-safe in-memory table backed by BTreeMap for ordered iteration.
pub struct MemTable {
    data: parking_lot::RwLock<BTreeMap<Vec<u8>, InternalValue>>,
    approximate_size: std::sync::atomic::AtomicUsize,
    max_size: usize,
}

impl MemTable {
    pub fn new(max_size: usize) -> Self {
        Self {
            data: parking_lot::RwLock::new(BTreeMap::new()),
            approximate_size: std::sync::atomic::AtomicUsize::new(0),
            max_size,
        }
    }

    pub fn insert(&self, key: RowKey, value: InternalValue) -> crate::Result<bool> {
        let size = std::mem::size_of::<RowKey>() + std::mem::size_of::<InternalValue>();
        let current = self
            .approximate_size
            .fetch_add(size, std::sync::atomic::Ordering::Relaxed)
            + size;

        if current > self.max_size {
            // Signal that flush is needed, but still insert
            self.data.write().insert(key.encode(), value);
            return Ok(false); // at capacity
        }

        self.data.write().insert(key.encode(), value);
        Ok(true)
    }

    pub fn get(&self, key: &RowKey) -> Option<InternalValue> {
        self.data.read().get(&key.encode()).cloned()
    }

    pub fn scan(
        &self,
        start: Option<&[u8]>,
        end: Option<&[u8]>,
    ) -> Vec<(Vec<u8>, InternalValue)> {
        let read_guard = self.data.read();
        match (start, end) {
            (Some(s), Some(e)) => read_guard
                .range(s.to_vec()..e.to_vec())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            (Some(s), None) => read_guard
                .range(s.to_vec()..)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            (None, Some(e)) => read_guard
                .range(..e.to_vec())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            (None, None) => read_guard
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.data.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn at_capacity(&self) -> bool {
        self.approximate_size
            .load(std::sync::atomic::Ordering::Relaxed)
            >= self.max_size
    }

    /// Drain all entries for flushing to disk.
    pub fn drain(&self) -> Vec<(Vec<u8>, InternalValue)> {
        let mut guard = self.data.write();
        let entries: Vec<_> = guard.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        guard.clear();
        self.approximate_size
            .store(0, std::sync::atomic::Ordering::Relaxed);
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdf_common::{FabricId, PolyRow, TemporalBounds};

    #[test]
    fn test_memtable_basic() {
        let mt = MemTable::new(1024 * 1024);
        let key = RowKey::new("test", FabricId::new());
        let row = PolyRow {
            id: key.row_id,
            values: Default::default(),
            temporal: TemporalBounds::new(),
            version: 1,
        };

        assert!(mt.insert(key.clone(), InternalValue::Active(row)).unwrap());
        assert!(mt.get(&key).is_some());
        assert_eq!(mt.len(), 1);
    }

    #[test]
    fn test_memtable_scan() {
        let mt = MemTable::new(1024 * 1024);
        let id1 = FabricId::new();
        let id2 = FabricId::new();

        mt.insert(
            RowKey::new("docs", id1),
            InternalValue::Active(PolyRow {
                id: id1,
                values: Default::default(),
                temporal: TemporalBounds::new(),
                version: 1,
            }),
        ).unwrap();

        mt.insert(
            RowKey::new("docs", id2),
            InternalValue::Active(PolyRow {
                id: id2,
                values: Default::default(),
                temporal: TemporalBounds::new(),
                version: 1,
            }),
        ).unwrap();

        let results = mt.scan(None, None);
        assert_eq!(results.len(), 2);
    }
}
