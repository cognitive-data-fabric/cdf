//! Hierarchical Navigable Small World (HNSW) index implementation.
//
// This implementation uses a custom `Ord` wrapper around `f32` so the
// `BinaryHeap` can be used without relying on `f32: Ord` (which is not
// implemented in Rust because of NaN). Distances are compared via
// `partial_cmp` and fall back to `Ordering::Equal` for NaN.

use cdf_common::{DistanceMetric, Embedding, FabricId, Neighbor};
use rand::{thread_rng, Rng};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// Max-heap entry: larger `dist` => higher priority.
#[derive(Clone, Copy)]
struct MaxEntry {
    dist: f32,
    id: FabricId,
}

impl PartialEq for MaxEntry {
    fn eq(&self, other: &Self) -> bool {
        self.dist == other.dist
    }
}
impl Eq for MaxEntry {}

impl Ord for MaxEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap: we want largest dist first.
        self.dist.partial_cmp(&other.dist).unwrap_or(Ordering::Equal)
    }
}
impl PartialOrd for MaxEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Min-heap behaviour: smallest `dist` first. Implemented as a max-heap
/// with reversed ordering.
#[derive(Clone, Copy)]
struct MinEntry {
    dist: f32,
    id: FabricId,
}

impl PartialEq for MinEntry {
    fn eq(&self, other: &Self) -> bool {
        self.dist == other.dist
    }
}
impl Eq for MinEntry {}

impl Ord for MinEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Invert comparison so the BinaryHeap acts as a min-heap.
        other.dist.partial_cmp(&self.dist).unwrap_or(Ordering::Equal)
    }
}
impl PartialOrd for MinEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Node in the HNSW graph.
#[derive(Clone)]
struct HnswNode {
    id: FabricId,
    vector: Vec<f32>,
    layers: Vec<Vec<FabricId>>, // connections per layer
}

/// HNSW index parameters.
pub struct HnswParams {
    pub m: usize,              // max connections per layer
    pub ef_construction: usize,
    pub ml: f64,               // level generation factor
}

impl Default for HnswParams {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 200,
            ml: 1.0 / std::f64::consts::LN_2,
        }
    }
}

pub struct HnswIndex {
    params: HnswParams,
    nodes: HashMap<FabricId, HnswNode>,
    entry_point: Option<FabricId>,
    current_level: usize,
    metric: DistanceMetric,
}

impl HnswIndex {
    pub fn new(metric: DistanceMetric, params: HnswParams) -> Self {
        Self {
            params,
            nodes: HashMap::new(),
            entry_point: None,
            current_level: 0,
            metric,
        }
    }

    fn random_level(&self) -> usize {
        let mut level = 0;
        let mut rng = thread_rng();
        let threshold = std::f64::consts::E.powf(-1.0 / self.params.ml);
        while rng.gen::<f64>() < threshold {
            level += 1;
        }
        level
    }

    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        self.metric.distance(a, b)
    }

    /// Beam search at a single layer. Returns up to `ef` (dist, id) pairs
    /// sorted by ascending distance.
    fn search_layer(
        &self,
        query: &[f32],
        entry: FabricId,
        ef: usize,
        level: usize,
    ) -> Vec<(f32, FabricId)> {
        let mut visited = HashSet::new();
        // candidates: min-heap on dist (closest unexplored first)
        let mut candidates: BinaryHeap<MinEntry> = BinaryHeap::new();
        // results: max-heap on dist so we can evict the farthest when at capacity
        let mut results: BinaryHeap<MaxEntry> = BinaryHeap::new();

        let entry_node = self.nodes.get(&entry).expect("entry must exist");
        let dist = self.distance(query, &entry_node.vector);
        candidates.push(MinEntry { dist, id: entry });
        results.push(MaxEntry { dist, id: entry });
        visited.insert(entry);

        while let Some(MinEntry { dist: cdist, id: cid }) = candidates.pop() {
            // Stop if the closest candidate is farther than our worst result
            // and we already have ef results.
            if let Some(worst) = results.peek() {
                if results.len() >= ef && cdist > worst.dist {
                    break;
                }
            }
            if let Some(node) = self.nodes.get(&cid) {
                if level < node.layers.len() {
                    for &neighbor in &node.layers[level] {
                        if visited.insert(neighbor) {
                            if let Some(nnode) = self.nodes.get(&neighbor) {
                                let ndist = self.distance(query, &nnode.vector);
                                candidates.push(MinEntry { dist: ndist, id: neighbor });
                                results.push(MaxEntry { dist: ndist, id: neighbor });
                                if results.len() > ef {
                                    results.pop(); // evict the farthest
                                }
                            }
                        }
                    }
                }
            }
        }

        // Drain results sorted ascending by distance
        let mut out: Vec<(f32, FabricId)> =
            results.into_iter().map(|e| (e.dist, e.id)).collect();
        out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
        out.into_iter().take(ef).collect()
    }

    fn select_neighbors(
        &self,
        candidates: Vec<(f32, FabricId)>,
        m: usize,
    ) -> Vec<FabricId> {
        candidates.into_iter().take(m).map(|(_, id)| id).collect()
    }
}

impl crate::VectorIndex for HnswIndex {
    fn insert(&mut self, id: FabricId, embedding: &Embedding) -> crate::Result<()> {
        let level = self.random_level();
        let mut node = HnswNode {
            id,
            vector: embedding.values.clone(),
            layers: vec![Vec::new(); level + 1],
        };

        if let Some(entry) = self.entry_point {
            // 1. Search from top layer down to (level+1) to find entry point
            let mut curr_entry = entry;
            for l in (level + 1..=self.current_level).rev() {
                let nearest = self.search_layer(&embedding.values, curr_entry, 1, l);
                if let Some(&(_, nid)) = nearest.first() {
                    curr_entry = nid;
                }
            }

            // 2. For each layer 0..=level, find ef_construction neighbors
            //    and connect bidirectionally.
            for l in (0..=level.min(self.current_level)).rev() {
                let nearest = self.search_layer(
                    &embedding.values,
                    curr_entry,
                    self.params.ef_construction,
                    l,
                );
                let neighbors = self.select_neighbors(nearest, self.params.m);
                for &nid in &neighbors {
                    if let Some(neighbor_node) = self.nodes.get_mut(&nid) {
                        if l < neighbor_node.layers.len()
                            && !neighbor_node.layers[l].contains(&id)
                        {
                            neighbor_node.layers[l].push(id);
                        }
                    }
                }
                node.layers[l] = neighbors;
            }
        } else {
            self.entry_point = Some(id);
            self.current_level = level;
        }

        self.nodes.insert(id, node);
        Ok(())
    }

    fn search(
        &self,
        query: &[f32],
        k: usize,
        _metric: DistanceMetric,
        ef: usize,
    ) -> crate::Result<Vec<Neighbor>> {
        let Some(entry) = self.entry_point else {
            return Ok(vec![]);
        };
        let mut curr_entry = entry;
        for l in (1..=self.current_level).rev() {
            let nearest = self.search_layer(query, curr_entry, 1, l);
            if let Some(&(_, nid)) = nearest.first() {
                curr_entry = nid;
            }
        }
        let results = self.search_layer(query, curr_entry, ef.max(k), 0);
        let metric = self.metric;
        let neighbors: Vec<Neighbor> = results
            .into_iter()
            .take(k)
            .map(|(dist, id)| {
                // Use the index's configured metric for similarity.
                let sim = if let Some(n) = self.nodes.get(&id) {
                    metric.similarity(query, &n.vector)
                } else {
                    0.0
                };
                Neighbor { id, distance: dist, similarity: sim }
            })
            .collect();
        Ok(neighbors)
    }

    fn remove(&mut self, id: FabricId) -> crate::Result<bool> {
        Ok(self.nodes.remove(&id).is_some())
    }

    fn len(&self) -> usize {
        self.nodes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VectorIndex;
    use cdf_common::DistanceMetric;

    fn make_emb(values: Vec<f32>) -> Embedding {
        Embedding::new(values, "test-model")
    }

    #[test]
    fn test_insert_and_search() {
        let mut idx = HnswIndex::new(DistanceMetric::Cosine, HnswParams::default());
        let a = FabricId::new();
        let b = FabricId::new();
        let c = FabricId::new();
        idx.insert(a, &make_emb(vec![1.0, 0.0, 0.0])).unwrap();
        idx.insert(b, &make_emb(vec![0.0, 1.0, 0.0])).unwrap();
        idx.insert(c, &make_emb(vec![1.0, 0.1, 0.0])).unwrap();

        let res = idx.search(&[1.0, 0.0, 0.0], 2, DistanceMetric::Cosine, 16).unwrap();
        assert_eq!(res.len(), 2);
        // The closest should be a (exact match)
        assert_eq!(res[0].id, a);
    }

    #[test]
    fn test_empty_search() {
        let idx = HnswIndex::new(DistanceMetric::Cosine, HnswParams::default());
        let res = idx.search(&[1.0, 0.0, 0.0], 5, DistanceMetric::Cosine, 16).unwrap();
        assert!(res.is_empty());
    }
}
