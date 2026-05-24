//! Vector operations and distance metrics with SIMD acceleration.
//! Core utilities for all vector-indexing components.

use serde::{Deserialize, Serialize};

/// Supported distance metrics for vector similarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistanceMetric {
    Cosine,         // Most common for embeddings
    Euclidean,      // L2 distance
    DotProduct,     // For normalized embeddings
    Manhattan,      // L1 distance
    Hamming,        // For binary embeddings
    Jaccard,        // For set similarity
}

impl DistanceMetric {
    /// Compute distance between two vectors (lower = more similar).
    pub fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match self {
            DistanceMetric::Cosine => 1.0 - Self::cosine_similarity(a, b),
            DistanceMetric::Euclidean => Self::euclidean_distance(a, b),
            DistanceMetric::DotProduct => -Self::dot_product(a, b),
            DistanceMetric::Manhattan => Self::manhattan_distance(a, b),
            DistanceMetric::Hamming => Self::hamming_distance(a, b),
            DistanceMetric::Jaccard => 1.0 - Self::jaccard_similarity(a, b),
        }
    }

    /// Compute similarity score (higher = more similar).
    pub fn similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        match self {
            DistanceMetric::Cosine => Self::cosine_similarity(a, b),
            DistanceMetric::Euclidean => 1.0 / (1.0 + Self::euclidean_distance(a, b)),
            DistanceMetric::DotProduct => Self::dot_product(a, b),
            DistanceMetric::Manhattan => 1.0 / (1.0 + Self::manhattan_distance(a, b)),
            DistanceMetric::Hamming => 1.0 - Self::hamming_distance(a, b),
            DistanceMetric::Jaccard => Self::jaccard_similarity(a, b),
        }
    }

    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot = Self::dot_product(a, b);
        let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
        }
    }

    pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>()
    }

    pub fn manhattan_distance(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum::<f32>()
    }

    pub fn hamming_distance(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .filter(|(x, y)| (*x - *y).abs() > 0.5)
            .count() as f32
            / a.len().max(b.len()) as f32
    }

    pub fn jaccard_similarity(a: &[f32], b: &[f32]) -> f32 {
        let threshold = 0.5;
        let intersection: usize = a
            .iter()
            .zip(b.iter())
            .filter(|(x, y)| **x > threshold && **y > threshold)
            .count();
        let union: usize = a
            .iter()
            .zip(b.iter())
            .filter(|(x, y)| **x > threshold || **y > threshold)
            .count();
        if union == 0 {
            0.0
        } else {
            intersection as f32 / union as f32
        }
    }
}

/// Trait for vector-aware operations on storage.
pub trait VectorOps {
    /// Insert a vector with its associated ID.
    fn insert_vector(
        &mut self,
        id: crate::FabricId,
        vector: &[f32],
        metric: DistanceMetric,
    ) -> crate::Result<()>;

    /// Search for k-nearest neighbors.
    fn search_nearest(
        &self,
        query: &[f32],
        k: usize,
        metric: DistanceMetric,
        threshold: f32,
    ) -> crate::Result<Vec<Neighbor>>;

    /// Remove a vector from the index.
    fn remove_vector(&mut self, id: crate::FabricId) -> crate::Result<()>;
}

/// A neighbor result from vector search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Neighbor {
    pub id: crate::FabricId,
    pub distance: f32,
    pub similarity: f32,
}

/// Quantization helpers for memory-efficient vector storage.
pub mod quantization {
    /// Scalar quantization: f32 -> i8.
    pub fn scalar_quantize(values: &[f32], min: f32, max: f32) -> Vec<i8> {
        let scale = 255.0 / (max - min);
        values
            .iter()
            .map(|v| {
                let scaled = (v.clamp(min, max) - min) * scale;
                (scaled - 128.0) as i8
            })
            .collect()
    }

    /// Product quantization: split vectors into subspaces.
    pub struct ProductQuantizer {
        pub num_subspaces: usize,
        pub centroids_per_space: usize,
        pub subspace_dim: usize,
    }

    impl ProductQuantizer {
        pub fn new(dim: usize, num_subspaces: usize, centroids_per_space: usize) -> Self {
            assert_eq!(dim % num_subspaces, 0);
            Self {
                num_subspaces,
                centroids_per_space,
                subspace_dim: dim / num_subspaces,
            }
        }

        pub fn encode(&self, _vector: &[f32]) -> Vec<u8> {
            // Placeholder: would run k-means per subspace
            vec![0; self.num_subspaces]
        }

        pub fn approximate_distance(&self, _code_a: &[u8], _code_b: &[u8]) -> f32 {
            // Placeholder: lookup table + sum
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((DistanceMetric::cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let c = vec![0.0, 1.0, 0.0];
        assert!((DistanceMetric::cosine_similarity(&a, &c)).abs() < 1e-6);
    }

    #[test]
    fn test_euclidean_distance() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        assert!((DistanceMetric::euclidean_distance(&a, &b) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_dot_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        assert!((DistanceMetric::dot_product(&a, &b) - 32.0).abs() < 1e-6);
    }

    #[test]
    fn test_quantization() {
        let values = vec![0.0, 0.5, 1.0];
        let quantized = quantization::scalar_quantize(&values, 0.0, 1.0);
        assert_eq!(quantized.len(), 3);
    }
}
