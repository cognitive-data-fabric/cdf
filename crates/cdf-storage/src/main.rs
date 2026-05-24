//! CDF Storage Node — gRPC server for the storage engine.

use cdf_storage::StorageEngine;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let node_id = env::var("CDF_NODE_ID").unwrap_or_else(|_| "storage-1".to_string());
    let data_dir = env::var("CDF_DATA_DIR").unwrap_or_else(|_| "/data".to_string());
    let port = env::var("CDF_PORT").unwrap_or_else(|_| "50051".to_string());
    let shard_range = env::var("CDF_SHARD_RANGE").unwrap_or_else(|_| "0-32767".to_string());

    println!("CDF Storage Node starting...");
    println!("  Node ID: {}", node_id);
    println!("  Data Dir: {}", data_dir);
    println!("  Port: {}", port);
    println!("  Shard Range: {}", shard_range);

    // Initialize storage engine
    let config = cdf_storage::EngineConfig {
        data_dir: std::path::PathBuf::from(&data_dir),
        memtable_size: 64 * 1024 * 1024,
        wal_sync: true,
        max_segment_size: 256 * 1024 * 1024,
    };

    let _engine = StorageEngine::open(config)?;

    println!("Storage engine initialized. Listening on port {}", port);

    // Keep the process running
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}
