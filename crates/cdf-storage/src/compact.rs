//! Background compaction for LSM-tree levels.

/// Trigger compaction for a given level.
pub fn compact_level(_level: u32) -> std::io::Result<u64> {
    // TODO: merge sorted segments, remove tombstones, write new segments
    unimplemented!("compaction")
}
