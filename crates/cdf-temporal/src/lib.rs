//! Temporal index for time-slice queries and vector drift tracking over time.
//! Wraps cdf_common::TemporalStore with efficient indexing.

use cdf_common::{TemporalBounds, Timestamp};
use cdf_common::temporal::TemporalQuery;
use std::collections::BTreeMap;

/// Temporal index: maps time ranges to sequence numbers for fast lookup.
pub struct TemporalIndex {
    valid_time_index: BTreeMap<Timestamp, Vec<u64>>,
    transaction_index: BTreeMap<Timestamp, Vec<u64>>,
}

impl TemporalIndex {
    pub fn new() -> Self {
        Self {
            valid_time_index: BTreeMap::new(),
            transaction_index: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, bounds: TemporalBounds, seq: u64) {
        self.valid_time_index
            .entry(bounds.valid_time_start)
            .or_default()
            .push(seq);
        self.transaction_index
            .entry(bounds.transaction_time)
            .or_default()
            .push(seq);
    }

    pub fn query(&self, query: TemporalQuery) -> Vec<u64> {
        match query {
            TemporalQuery::Current => self.query_current(),
            TemporalQuery::AsOfValid(t) => self.query_as_of_valid(t),
            _ => vec![], // TODO: full bitemporal
        }
    }

    fn query_current(&self) -> Vec<u64> {
        // Return all entries with valid_time_end == forever
        // Simplified: return last entries
        self.transaction_index
            .values()
            .last()
            .cloned()
            .unwrap_or_default()
    }

    fn query_as_of_valid(&self, t: Timestamp) -> Vec<u64> {
        self.valid_time_index
            .range(..=t)
            .flat_map(|(_, v)| v.iter().copied())
            .collect()
    }
}

impl Default for TemporalIndex {
    fn default() -> Self {
        Self::new()
    }
}
