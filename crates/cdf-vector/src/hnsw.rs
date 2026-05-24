//! Hierarchical Navigable Small World (HNSW) index implementation.
use cdf_common::{DistanceMetric, Embedding, FabricId, Neighbor};
use rand::{thread_rng, Rng};
use std::collections::{BinaryHeap, HashMap, HashSet};

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

    fn search_layer(
        &self,
        query: &[f32],
        entry: FabricId,
        ef: usize,
        level: usize,
    ) -> Vec<(f32, FabricId)> {
        let mut visited = HashSet::new();
        let mut candidates: BinaryHeap<(std::cmp::Reverse<f32>, FabricId)> = BinaryHeap::new();
        let mut results: BinaryHeap<(f32, FabricId)> = BinaryHeap::new();

        let entry_node = self.nodes.get(&entry).unwrap();
        let dist = self.distance(query, &entry_node.vector);
        candidates.push((std::cmp::Reverse(dist), entry));
        results.push((dist, entry));
        visited.insert(entry);

        while let Some((std::cmp::Reverse(cdist), cid)) = candidates.pop() {
            if results.len() >= ef && cdist > results.peek().unwrap().0 {
                break;
            }
            if let Some(node) = self.nodes.get(&cid) {
                if level < node.layers.len() {
                    for &neighbor in &node.layers[level] {
                        if visited.insert(neighbor) {
                            let nvec = &self.nodes.get(&neighbor).unwrap().vector;
                            let ndist = self.distance(query, nvec);
                            candidates.push((std::cmp::Reverse(ndist), neighbor));
                            results.push((ndist, neighbor));
                            if results.len() > ef {
                                results.pop();
                            }
                        }
                    }
                }
            }
        }

        let mut sorted: Vec<_> = results.into_sorted_vec();
        sorted.reverse();
        sorted.into_iter().take(ef).collect()
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
        let node = HnswNode {
            id,
            vector: embedding.values.clone(),
            layers: vec![Vec::new(); level + 1],
        };

        if let Some(entry) = self.entry_point {
            // Search from top layer down
            let mut curr_entry = entry;
            for l in (level + 1..=self.current_level).rev() {
                let nearest = self.search_layer(&embedding.values, curr_entry, 1, l);
                if let Some(&(_, nid)) = nearest.first() {
                    curr_entry = nid;
                }
            }

            // Connect at each level
            for l in (0..=level.min(self.current_level)).rev() {
                let nearest = self.search_layer(&embedding.values, curr_entry, self.params.ef_construction, l);
                let neighbors = self.select_neighbors(nearest, self.params.m);
                // TODO: bidirectional connections, prune
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
        if let Some(entry) = self.entry_point {
            let mut curr_entry = entry;
            // Descend layers
            for l in (1..=self.current_level).rev() {
                let nearest = self.search_layer(query, curr_entry, 1, l);
                if let Some(&(_, nid)) = nearest.first() {
                    curr_entry = nid;
                }
            }
            // Bottom layer search
            let results = self.search_layer(query, curr_entry, ef.max(k), 0);
            let neighbors: Vec<_> = results
                .into_iter()
                .take(k)
                .map(|(dist, id)| {
                    let sim = 1.0 - dist; // approximate for cosine
                    Neighbor { id, distance: dist, similarity: sim }
                })
                .collect();
            Ok(neighbors)
        } else {
            Ok(vec![])
        }
    }

    fn remove(&mut self, id: FabricId) -> crate::Result<bool> {
        Ok(self.nodes.remove(&id).is_some())
    }

    fn len(&self) -> usize {
        self.nodes.len()
    }
}

// Required for BinaryHeap ordering
impl Ord for HnswNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for HnswNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for HnswNode {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for HnswNode {}
