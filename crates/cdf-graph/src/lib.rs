//! Native graph adjacency index using CSR (Compressed Sparse Row) format.
//! Supports efficient traversal and path queries alongside vector search.

use cdf_common::{FabricId, GraphEdgeRef};
use std::collections::{HashMap, VecDeque};

/// Re-export Result type using cdf-common's error
pub type Result<T> = std::result::Result<T, cdf_common::CdfError>;

/// Compressed Sparse Row graph representation.
pub struct CsrGraph {
    nodes: HashMap<FabricId, Vec<String>>, // node_id -> labels
    // All edges stored as a flat list, kept sorted by from_id for fast CSR lookup
    edges: Vec<GraphEdgeRef>,
    // CSR structure: sorted_from_ids[i] is the source of edges[row_ptr[i]..row_ptr[i+1]]
    sorted_from_ids: Vec<FabricId>,
    row_ptr: Vec<usize>,            // offsets into edges/col_idx
    col_idx: Vec<FabricId>,         // to_node for each edge
    edge_types: Vec<String>,        // edge_type for each edge
    edge_props: Vec<HashMap<String, String>>,
    dirty: bool,                    // CSR needs rebuild
}

impl CsrGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            sorted_from_ids: Vec::new(),
            row_ptr: vec![0],
            col_idx: Vec::new(),
            edge_types: Vec::new(),
            edge_props: Vec::new(),
            dirty: false,
        }
    }

    /// Ensure CSR structure is up-to-date.
    fn ensure_csr(&mut self) {
        if !self.dirty {
            return;
        }
        // Sort edges by from_id (stable, preserves insertion order for ties)
        self.edges.sort_by_key(|e| e.from_id);
        // Rebuild CSR
        self.sorted_from_ids.clear();
        self.row_ptr.clear();
        self.col_idx.clear();
        self.edge_types.clear();
        self.edge_props.clear();

        // row_ptr[i] points to the start of the i-th source's edges in col_idx.
        // After processing all edges, row_ptr.len() == sorted_from_ids.len() + 1
        // (the final entry is a sentinel = total edge count for binary search bounds).
        let mut current_from: Option<FabricId> = None;
        for edge in &self.edges {
            if Some(edge.from_id) != current_from {
                // Record the start offset for this new source node
                self.sorted_from_ids.push(edge.from_id);
                self.row_ptr.push(self.col_idx.len());
                current_from = Some(edge.from_id);
            }
            self.col_idx.push(edge.to_id);
            self.edge_types.push(edge.edge_type.clone());
            self.edge_props.push(
                edge.properties
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_string()))
                    .collect(),
            );
        }
        // Trailing sentinel: total edge count, used as the exclusive upper bound
        // when looking up the last row's range.
        self.row_ptr.push(self.col_idx.len());
        self.dirty = false;
    }

    pub fn add_node(&mut self, id: FabricId, labels: Vec<String>) {
        self.nodes.entry(id).or_insert(labels);
    }

    pub fn add_edge(&mut self, edge: GraphEdgeRef) {
        self.edges.push(edge);
        // Mark CSR for incremental rebuild on next query
        self.dirty = true;
    }

    /// Find the row index in CSR for a given from_id. Returns None if not present.
    fn find_row(&self, from: FabricId) -> Option<usize> {
        self.sorted_from_ids.binary_search(&from).ok()
    }

    /// O(degree) neighbor lookup using CSR.
    pub fn get_neighbors(&self, node: FabricId, edge_type: Option<&str>) -> Vec<FabricId> {
        // We can't call ensure_csr through &self (needs &mut); callers that need
        // a fully up-to-date view should call get_neighbors_mut or refresh() first.
        // For correctness, fall back to a linear scan if dirty.
        if self.dirty {
            return self
                .edges
                .iter()
                .filter(|e| e.from_id == node && edge_type.map_or(true, |t| e.edge_type == t))
                .map(|e| e.to_id)
                .collect();
        }
        let row_idx = match self.find_row(node) {
            Some(i) => i,
            None => return Vec::new(),
        };
        let start = self.row_ptr[row_idx];
        let end = self.row_ptr[row_idx + 1];
        (start..end)
            .filter_map(|i| {
                let et = &self.edge_types[i];
                if edge_type.map_or(true, |t| et == t) {
                    Some(self.col_idx[i])
                } else {
                    None
                }
            })
            .collect()
    }

    /// Mutable variant: rebuilds CSR first, then returns neighbors using CSR lookup.
    pub fn get_neighbors_csr(&mut self, node: FabricId, edge_type: Option<&str>) -> Vec<FabricId> {
        self.ensure_csr();
        self.get_neighbors(node, edge_type)
    }

    /// BFS from `start` up to `max_depth`, returning all visited paths.
    pub fn bfs(&self, start: FabricId, max_depth: usize) -> Vec<Vec<FabricId>> {
        let mut visited: HashMap<FabricId, bool> = HashMap::new();
        let mut queue: VecDeque<(FabricId, usize, Vec<FabricId>)> = VecDeque::new();
        let mut paths = Vec::new();

        queue.push_back((start, 0, vec![start]));
        visited.insert(start, true);

        while let Some((node, depth, path)) = queue.pop_front() {
            paths.push(path.clone());
            if depth >= max_depth {
                continue;
            }
            for neighbor in self.get_neighbors(node, None) {
                if !visited.get(&neighbor).copied().unwrap_or(false) {
                    visited.insert(neighbor, true);
                    let mut new_path = path.clone();
                    new_path.push(neighbor);
                    queue.push_back((neighbor, depth + 1, new_path));
                }
            }
        }
        paths
    }

    /// BFS that returns a single shortest path to a target, if one exists.
    pub fn shortest_path(&self, start: FabricId, target: FabricId, max_depth: usize) -> Option<Vec<FabricId>> {
        if start == target {
            return Some(vec![start]);
        }
        let mut visited: HashMap<FabricId, bool> = HashMap::new();
        let mut queue: VecDeque<(FabricId, Vec<FabricId>)> = VecDeque::new();
        visited.insert(start, true);
        queue.push_back((start, vec![start]));

        while let Some((node, path)) = queue.pop_front() {
            if path.len() - 1 > max_depth {
                continue;
            }
            for neighbor in self.get_neighbors(node, None) {
                if !visited.get(&neighbor).copied().unwrap_or(false) {
                    let mut new_path = path.clone();
                    new_path.push(neighbor);
                    if neighbor == target {
                        return Some(new_path);
                    }
                    visited.insert(neighbor, true);
                    queue.push_back((neighbor, new_path));
                }
            }
        }
        None
    }

    /// Get all edges originating from a node.
    pub fn get_outgoing_edges(&self, node: FabricId) -> Vec<GraphEdgeRef> {
        self.edges
            .iter()
            .filter(|e| e.from_id == node)
            .cloned()
            .collect()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Force CSR rebuild.
    pub fn refresh(&mut self) {
        self.ensure_csr();
    }
}

impl Default for CsrGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_bfs() {
        let mut g = CsrGraph::new();
        let n1 = FabricId::new();
        let n2 = FabricId::new();
        let n3 = FabricId::new();

        g.add_node(n1, vec!["A".to_string()]);
        g.add_node(n2, vec!["B".to_string()]);
        g.add_node(n3, vec!["C".to_string()]);

        g.add_edge(GraphEdgeRef {
            from_id: n1,
            to_id: n2,
            edge_type: "knows".to_string(),
            properties: Default::default(),
        });
        g.add_edge(GraphEdgeRef {
            from_id: n2,
            to_id: n3,
            edge_type: "knows".to_string(),
            properties: Default::default(),
        });

        let paths = g.bfs(n1, 2);
        assert_eq!(paths.len(), 3);
    }

    #[test]
    fn test_csr_neighbors_after_refresh() {
        let mut g = CsrGraph::new();
        let n1 = FabricId::new();
        let n2 = FabricId::new();
        let n3 = FabricId::new();

        g.add_node(n1, vec!["A".to_string()]);
        g.add_node(n2, vec!["B".to_string()]);
        g.add_node(n3, vec!["C".to_string()]);

        g.add_edge(GraphEdgeRef {
            from_id: n1,
            to_id: n2,
            edge_type: "cites".to_string(),
            properties: Default::default(),
        });
        g.add_edge(GraphEdgeRef {
            from_id: n1,
            to_id: n3,
            edge_type: "cites".to_string(),
            properties: Default::default(),
        });
        g.add_edge(GraphEdgeRef {
            from_id: n2,
            to_id: n3,
            edge_type: "cites".to_string(),
            properties: Default::default(),
        });

        g.refresh();
        let neighbors = g.get_neighbors(n1, None);
        assert_eq!(neighbors.len(), 2, "n1 should have 2 outgoing edges");

        let knows_neighbors = g.get_neighbors(n1, Some("cites"));
        assert_eq!(knows_neighbors.len(), 2);

        let unknown_neighbors = g.get_neighbors(n1, Some("unknown"));
        assert_eq!(unknown_neighbors.len(), 0);
    }

    #[test]
    fn test_shortest_path() {
        let mut g = CsrGraph::new();
        let n1 = FabricId::new();
        let n2 = FabricId::new();
        let n3 = FabricId::new();
        let n4 = FabricId::new();

        g.add_edge(GraphEdgeRef {
            from_id: n1,
            to_id: n2,
            edge_type: "link".to_string(),
            properties: Default::default(),
        });
        g.add_edge(GraphEdgeRef {
            from_id: n2,
            to_id: n3,
            edge_type: "link".to_string(),
            properties: Default::default(),
        });
        g.add_edge(GraphEdgeRef {
            from_id: n1,
            to_id: n4,
            edge_type: "link".to_string(),
            properties: Default::default(),
        });
        g.add_edge(GraphEdgeRef {
            from_id: n4,
            to_id: n3,
            edge_type: "link".to_string(),
            properties: Default::default(),
        });
        g.refresh();

        let path = g.shortest_path(n1, n3, 5).unwrap();
        // Should pick the 2-hop path through either n2 or n4
        assert_eq!(path.len(), 3);
        assert_eq!(path[0], n1);
        assert_eq!(path[2], n3);
    }
}
