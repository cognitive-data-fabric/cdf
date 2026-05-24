//! Write-Ahead Log for crash recovery and replication.
//! Durable, append-only log of all mutations.

use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write, Seek, SeekFrom};
use std::path::Path;

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
    path: std::path::PathBuf,
    next_seq: u64,
    sync_on_write: bool,
}

impl WriteAheadLog {
    pub fn open(path: impl AsRef<Path>, sync_on_write: bool) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)?;
        Ok(Self {
            path: path.as_ref().to_path_buf(),
            file,
            next_seq: 1,
            sync_on_write,
        })
    }

    pub fn append(&mut self, op: WalOp) -> std::io::Result<u64> {
        let entry = WalEntry {
            seq: self.next_seq,
            op,
        };
        let bytes = bincode::serialize(&entry).unwrap();
        let len = bytes.len() as u32;
        self.file.write_all(&len.to_le_bytes())?;
        self.file.write_all(&bytes)?;
        if self.sync_on_write {
            self.file.sync_all()?;
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        Ok(seq)
    }

    pub fn sync(&mut self) -> std::io::Result<()> {
        self.file.sync_all()
    }

    pub fn replay(&mut self) -> std::io::Result<Vec<WalEntry>> {
        self.file.seek(SeekFrom::Start(0))?;
        let mut reader = BufReader::new(&self.file);
        let mut entries = Vec::new();
        loop {
            let mut len_buf = [0u8; 4];
            match reader.read_exact(&mut len_buf) {
                Ok(_) => {
                    let len = u32::from_le_bytes(len_buf) as usize;
                    let mut buf = vec![0u8; len];
                    reader.read_exact(&mut buf)?;
                    let entry: WalEntry = bincode::deserialize(&buf)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                    self.next_seq = self.next_seq.max(entry.seq + 1);
                    entries.push(entry);
                }
                Err(_) => break,
            }
        }
        Ok(entries)
    }

    pub fn rotate(&mut self) -> std::io::Result<std::path::PathBuf> {
        let new_path = self.path.with_extension(format!("wal.{}", self.next_seq));
        self.file.sync_all()?;
        drop(std::mem::replace(&mut self.file, File::open(&self.path)?));
        std::fs::rename(&self.path, &new_path)?;
        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&self.path)?;
        Ok(new_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wal_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut wal = WriteAheadLog::open(dir.path().join("test.wal"), false).unwrap();
        wal.append(WalOp::Insert {
            table: "docs".to_string(),
            row_id: cdf_common::FabricId::new(),
            data: vec![1, 2, 3],
        }).unwrap();

        let entries = wal.replay().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(matches!(entries[0].op, WalOp::Insert { .. }));
    }
}
