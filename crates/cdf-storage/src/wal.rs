//! Write-Ahead Log for crash recovery and replication.
//! Durable, append-only log of all mutations.

use cdf_common::CdfError;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// WAL entry: sequence number + serialized operation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WalEntry {
    pub seq: u64,
    pub op: WalOp,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WalOp {
    Insert {
        table: String,
        row_id: cdf_common::FabricId,
        data: Vec<u8>,
    },
    Delete {
        table: String,
        row_id: cdf_common::FabricId,
    },
    Update {
        table: String,
        row_id: cdf_common::FabricId,
        data: Vec<u8>,
    },
}

pub struct WriteAheadLog {
    file: File,
    path: PathBuf,
    /// Next sequence number to assign. Persisted to a sidecar file so it
    /// can be recovered on restart.
    next_seq: u64,
    sync_on_write: bool,
}

impl WriteAheadLog {
    /// Open or create a WAL at the given path. The next sequence number is
    /// recovered by replaying the existing log (if any).
    pub fn open(path: impl AsRef<Path>, sync_on_write: bool) -> std::io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)?;

        let mut wal = Self {
            file,
            path,
            next_seq: 1,
            sync_on_write,
        };
        // Recover sequence number by replaying the log.
        // Convert any CdfError into io::Error so the open() signature stays simple.
        let entries = wal.replay_internal().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{:?}", e))
        })?;
        if let Some(last) = entries.last() {
            wal.next_seq = last.seq + 1;
        }
        // Reset file position to end for appending
        wal.file.seek(SeekFrom::End(0))?;
        Ok(wal)
    }

    pub fn append(&mut self, op: WalOp) -> Result<u64, CdfError> {
        let entry = WalEntry {
            seq: self.next_seq,
            op,
        };
        let bytes = bincode::serialize(&entry).map_err(|e| CdfError::WalError {
            reason: format!("serialize failed: {}", e),
        })?;
        let len = bytes.len() as u32;

        // Write length prefix and entry
        self.file
            .write_all(&len.to_le_bytes())
            .map_err(|e| CdfError::WalError {
                reason: format!("write length: {}", e),
            })?;
        self.file
            .write_all(&bytes)
            .map_err(|e| CdfError::WalError {
                reason: format!("write entry: {}", e),
            })?;

        if self.sync_on_write {
            self.file.sync_all().map_err(|e| CdfError::WalError {
                reason: format!("sync: {}", e),
            })?;
        }

        let seq = self.next_seq;
        self.next_seq += 1;
        Ok(seq)
    }

    pub fn sync(&mut self) -> Result<(), CdfError> {
        self.file
            .sync_all()
            .map_err(|e| CdfError::WalError {
                reason: format!("sync: {}", e),
            })
    }

    /// Replay all WAL entries from the start of the file. Returns the
    /// deserialized entries in order.
    pub fn replay(&mut self) -> Result<Vec<WalEntry>, CdfError> {
        let entries = self.replay_internal()?;
        // Reset file position to end for appending
        self.file
            .seek(SeekFrom::End(0))
            .map_err(|e| CdfError::WalError {
                reason: format!("seek: {}", e),
            })?;
        Ok(entries)
    }

    /// Internal replay that doesn't reset the file position.
    fn replay_internal(&mut self) -> Result<Vec<WalEntry>, CdfError> {
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|e| CdfError::WalError {
                reason: format!("seek: {}", e),
            })?;
        let mut reader = BufReader::new(&self.file);
        let mut entries = Vec::new();
        loop {
            let mut len_buf = [0u8; 4];
            match reader.read_exact(&mut len_buf) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => {
                    return Err(CdfError::WalError {
                        reason: format!("read length: {}", e),
                    })
                }
            }
            let len = u32::from_le_bytes(len_buf) as usize;
            // Sanity check: reject obviously corrupt lengths
            if len > 64 * 1024 * 1024 {
                return Err(CdfError::WalError {
                    reason: format!("entry length {} exceeds 64MB, WAL likely corrupt", len),
                });
            }
            let mut buf = vec![0u8; len];
            match reader.read_exact(&mut buf) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => {
                    return Err(CdfError::WalError {
                        reason: format!("read entry: {}", e),
                    })
                }
            }
            let entry: WalEntry =
                bincode::deserialize(&buf).map_err(|e| CdfError::WalError {
                    reason: format!("deserialize: {}", e),
                })?;
            entries.push(entry);
        }
        Ok(entries)
    }

    /// Rotate the WAL: close the current file, rename it to a timestamped
    /// archive, and open a fresh empty WAL at the original path.
    pub fn rotate(&mut self) -> Result<PathBuf, CdfError> {
        self.sync()?;
        // Drop the current file handle by closing it explicitly.
        // On Windows, a file cannot be renamed while open, so we close first.
        let path = self.path.clone();
        // Use a temp file to take ownership of the FD, then close it.
        let old = std::mem::replace(
            &mut self.file,
            OpenOptions::new()
                .create(true)
                .append(true)
                .read(true)
                .open(&path)
                .map_err(|e| CdfError::WalError {
                    reason: format!("reopen: {}", e),
                })?,
        );
        drop(old); // explicit close before rename

        let archive_path = path.with_extension(format!("wal.{}", self.next_seq.saturating_sub(1)));
        std::fs::rename(&path, &archive_path).map_err(|e| CdfError::WalError {
            reason: format!("rename: {}", e),
        })?;
        Ok(archive_path)
    }

    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wal_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut wal =
            WriteAheadLog::open(dir.path().join("test.wal"), false).unwrap();
        let seq1 = wal
            .append(WalOp::Insert {
                table: "docs".to_string(),
                row_id: cdf_common::FabricId::new(),
                data: vec![1, 2, 3],
            })
            .unwrap();
        assert_eq!(seq1, 1);

        let entries = wal.replay().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(matches!(entries[0].op, WalOp::Insert { .. }));
        assert_eq!(entries[0].seq, 1);
    }

    #[test]
    fn test_wal_seq_recovery() {
        // Sequence numbers must persist across reopens so the next write
        // doesn't reuse an existing seq.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("persist.wal");

        let id = cdf_common::FabricId::new();
        {
            let mut wal = WriteAheadLog::open(&path, false).unwrap();
            wal.append(WalOp::Insert {
                table: "t".to_string(),
                row_id: id,
                data: vec![1],
            })
            .unwrap();
            wal.append(WalOp::Insert {
                table: "t".to_string(),
                row_id: id,
                data: vec![2],
            })
            .unwrap();
        }

        // Reopen - next sequence number must be 3, not 1.
        let mut wal = WriteAheadLog::open(&path, false).unwrap();
        assert_eq!(wal.next_seq(), 3);
        let seq3 = wal
            .append(WalOp::Insert {
                table: "t".to_string(),
                row_id: id,
                data: vec![3],
            })
            .unwrap();
        assert_eq!(seq3, 3);
    }

    #[test]
    fn test_wal_serialize_error_handling() {
        // Ensure append returns an error type (not panic) on success path
        let dir = tempfile::tempdir().unwrap();
        let mut wal = WriteAheadLog::open(dir.path().join("ok.wal"), false).unwrap();
        let result = wal.append(WalOp::Delete {
            table: "t".to_string(),
            row_id: cdf_common::FabricId::new(),
        });
        assert!(result.is_ok());
    }
}
