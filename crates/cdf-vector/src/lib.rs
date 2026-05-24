//! HNSW vector index with SIMD-optimized distance computation and product quantization.

pub mod hnsw;
pub mod quantization;

use cdf_common::{DistanceMetric, Embedding, FabricId, Neighbor};

/// Vector index trait — implementable by HNSW, IVF, flat, etc.
pub trait VectorIndex: Send + Sync {
    fn insert(&mut self, id: FabricId, embedding: &Embedding) -> crate::Result<()>;
    fn search(
        &self,
        query: &[f32],
        k: usize,
        metric: DistanceMetric,
        ef: usize,
    ) -> crate::Result<Vec<Neighbor>>;
    fn remove(&mut self, id: FabricId) -> crate::Result<bool>;
    fn len(&self) -> usize;
}
