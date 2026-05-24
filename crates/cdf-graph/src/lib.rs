//! Native graph adjacency index using CSR (Compressed Sparse Row) format.
//! Supports efficient traversal and path queries alongside vector search.

use cdf_common::{FabricId, GraphEdgeRef};
use std::collections::{HashMap, HashSet};

/// Compressed Sparse Row graph representation.
pub struct CsrGraph {
    nodes: HashMap<FabricId, Vec<String>>, // node_id -> labels
    edges: Vec<GraphEdgeRef>,              // all edges (sorted by from_id)
    row_ptr: Vec<usize>,                   // offsets into edges
    col_idx: Vec<FabricId>,               // to_node for each edge
    edge_props: Vec<HashMap<String, String>>,
}

impl CsrGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            row_ptr: vec![0],
            col_idx: Vec::new(),
            edge_props: Vec::new(),
        }
    }

    pub fn add_node(&mut self, id: FabricId, labels: Vec<String>) {
        self.nodes.entry(id).or_insert(labels);
    }

    pub fn add_edge(&mut self, edge: GraphEdgeRef) {
        self.edges.push(edge.clone());
        self.col_idx.push(edge.to_id);
        self.edge_props.push(
            edge.properties
                .iter()
                .map(|(k, v)| (k.clone(), v.to_string()))
                .collect(),
        );
        // Rebuild CSR on next query or maintain incrementally
    }

    pub fn get_neighbors(&self, node: FabricId, edge_type: Option<&str>) -> Vec<FabricId> {
        // Find range in CSR (simplified: linear scan for now)
        self.edges
            .iter()
            .filter(|e| {
                e.from_id == node
                    && edge_type.map_or(true, |t| e.edge_type == t)
            })
            .map(|e| e.to_id)
            .collect()
    }

    pub fn bfs(&self, start: FabricId, max_depth: usize) -> Vec<Vec<FabricId>> {
        let mut visited = HashSet::new();
        let mut queue: Vec<(FabricId, usize, Vec<FabricId>)> = vec![(start, 0, vec![start])];
        let mut paths = Vec::new();

        while !queue.is_empty() {
            let (node, depth, path) = queue.remove(0);
            if depth > max_depth {
                continue;
            }
            paths.push(path.clone());
            for neighbor in self.get_neighbors(node, None) {
                if visited.insert(neighbor) {
                    let mut new_path = path.clone();
                    new_path.push(neighbor);
                    queue.push((neighbor, depth + 1, new_path));
                }
            }
        }
        paths
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
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
}
