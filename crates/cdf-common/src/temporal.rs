//! Bitemporal data management for CDF.
//! Supports valid-time (business time) and transaction-time (system time).

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

/// A nanosecond-precision timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp {
    pub seconds: i64,
    pub nanos: u32,
}

impl Timestamp {
    pub fn now() -> Self {
        let dur = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before epoch");
        Self {
            seconds: dur.as_secs() as i64,
            nanos: dur.subsec_nanos(),
        }
    }

    pub fn from_secs(seconds: i64) -> Self {
        Self {
            seconds,
            nanos: 0,
        }
    }

    /// Returns a timestamp representing "forever" (max valid-time end).
    pub fn forever() -> Self {
        Self {
            seconds: i64::MAX,
            nanos: 999_999_999,
        }
    }

    pub fn as_nanos(&self) -> u128 {
        (self.seconds as u128) * 1_000_000_000 + (self.nanos as u128)
    }

    pub fn duration_since(&self, other: Timestamp) -> Option<std::time::Duration> {
        let self_nanos = self.as_nanos();
        let other_nanos = other.as_nanos();
        self_nanos
            .checked_sub(other_nanos)
            .map(|n| std::time::Duration::from_nanos(n as u64))
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::now()
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.seconds, self.nanos)
    }
}

/// Bitemporal bounds: valid-time + transaction-time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TemporalBounds {
    pub valid_time_start: Timestamp,
    pub valid_time_end: Timestamp,
    pub transaction_time: Timestamp,
}

impl TemporalBounds {
    pub fn new() -> Self {
        let now = Timestamp::now();
        Self {
            valid_time_start: now,
            valid_time_end: Timestamp::forever(),
            transaction_time: now,
        }
    }

    pub fn with_valid_range(start: Timestamp, end: Timestamp) -> Self {
        Self {
            valid_time_start: start,
            valid_time_end: end,
            transaction_time: Timestamp::now(),
        }
    }

    /// Check if valid time contains a given timestamp.
    pub fn valid_contains(&self, t: Timestamp) -> bool {
        self.valid_time_start <= t && t < self.valid_time_end
    }

    /// Check if this is a current (non-historical) version.
    pub fn is_current(&self) -> bool {
        self.valid_time_end == Timestamp::forever()
    }
}

impl Default for TemporalBounds {
    fn default() -> Self {
        Self::new()
    }
}

/// Temporal query specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalQuery {
    /// Query as-of valid time (latest transaction).
    AsOfValid(Timestamp),
    /// Query as-of transaction time (any valid time).
    AsOfTransaction(Timestamp),
    /// Full bitemporal slice.
    Bitemporal {
        valid_time: Timestamp,
        transaction_time: Timestamp,
    },
    /// Default: current view (now, now).
    Current,
}

impl Default for TemporalQuery {
    fn default() -> Self {
        Self::Current
    }
}

/// Time-travel capable store trait.
pub trait TemporalStore {
    /// Insert a new version with valid-time start = now, end = forever.
    fn insert_current(&mut self, key: String, value: Vec<u8>) -> crate::Result<()>;

    /// Close a valid-time range (logical delete / update end).
    fn close_valid_range(&mut self, key: String, end: Timestamp) -> crate::Result<()>;

    /// Read at a specific temporal point.
    fn read_at(&self, key: &str, query: TemporalQuery) -> crate::Result<Option<Vec<u8>>>;
}

use std::fmt;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_ordering() {
        let t1 = Timestamp::from_secs(100);
        let t2 = Timestamp::from_secs(200);
        assert!(t1 < t2);
    }

    #[test]
    fn test_temporal_bounds_contains() {
        let bounds = TemporalBounds::with_valid_range(
            Timestamp::from_secs(100),
            Timestamp::from_secs(200),
        );
        assert!(bounds.valid_contains(Timestamp::from_secs(150)));
        assert!(!bounds.valid_contains(Timestamp::from_secs(200)));
        assert!(!bounds.valid_contains(Timestamp::from_secs(50)));
    }

    #[test]
    fn test_temporal_query_default() {
        let q = TemporalQuery::default();
        assert!(matches!(q, TemporalQuery::Current));
    }
}
