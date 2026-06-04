//! HNSW vector index with SIMD-optimized distance computation and product quantization.

pub mod hnsw;
pub mod quantization;

use cdf_common::{DistanceMetric, Embedding, FabricId, Neighbor};

/// Re-export Result type using cdf-common's error
pub type Result<T> = std::result::Result<T, cdf_common::CdfError>;

/// Vector index trait — implementable by HNSW, IVF, flat, etc.
pub trait VectorIndex: Send + Sync {
    fn insert(&mut self, id: FabricId, embedding: &Embedding) -> Result<()>;
    fn search(
        &self,
        query: &[f32],
        k: usize,
        metric: DistanceMetric,
        ef: usize,
    ) -> Result<Vec<Neighbor>>;
    fn remove(&mut self, id: FabricId) -> Result<bool>;
    fn len(&self) -> usize;
}
