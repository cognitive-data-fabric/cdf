//! Immutable on-disk segment files (SSTable-style).

use crate::{InternalValue, SegmentInfo};
use std::path::Path;

/// Write a sorted list of key-value pairs to a segment file.
pub fn write_segment(
    _path: &Path,
    _entries: &[(Vec<u8>, InternalValue)],
) -> std::io::Result<SegmentInfo> {
    // TODO: implement sorted string table with index blocks
    unimplemented!("segment write")
}

/// Load segment metadata without full data.
pub fn load_segment_info(_path: &Path) -> std::io::Result<SegmentInfo> {
    unimplemented!("segment info load")
}
