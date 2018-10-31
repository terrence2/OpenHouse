// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::{bail, ensure, Fallible};
use std::collections::{HashMap, HashSet};
use tree::NodeRef;

/// A simplified graph that we can use to find paths from all inputs to the outputs they affect.
pub struct Graph {
    nodes: HashMap<String, NodeRef>,
    edges: Vec<Edge>,
}

struct Edge {
    start: String,
    end: String,
}

impl Graph {
    pub fn new_empty() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: &NodeRef) {
        let path = node.path_str();
        if self.nodes.contains_key(&path) {
            return;
        }
        self.nodes.insert(path, node.to_owned());
    }

    pub fn add_edge(&mut self, src_node: &NodeRef, tgt_node: &NodeRef) {
        self.edges.push(Edge {
            start: src_node.path_str(),
            end: tgt_node.path_str(),
        })
    }

    pub fn invert(mut self) -> Fallible<Self> {
        let mut next_edges = Vec::new();
        for edge in self.edges.drain(..) {
            next_edges.push(Edge {
                start: edge.end,
                end: edge.start,
            });
        }
        return Ok(Self {
            nodes: self.nodes,
            edges: next_edges,
        });
    }

    pub fn connected_nodes(&self, from: &NodeRef, to: &[NodeRef]) -> Fallible<Vec<NodeRef>> {
        let mut found = HashSet::new();
        let mut visited = HashSet::new();
        let mut targets = HashMap::new();
        for node in to.iter() {
            targets.insert(node.path_str(), node.to_owned());
        }
        self._connected_at(from, &targets, &mut visited, &mut found)?;
        let mut out = Vec::new();
        for node in to.iter() {
            if found.contains(&node.path_str()) {
                out.push(node.to_owned());
            }
        }
        return Ok(out);
    }

    fn _connected_at(
        &self,
        current: &NodeRef,
        targets: &HashMap<String, NodeRef>,
        visited: &mut HashSet<String>,
        out: &mut HashSet<String>,
    ) -> Fallible<()> {
        let current_path = current.path_str();

        if visited.contains(&current_path) {
            return Ok(());
        }
        visited.insert(current_path.clone());

        if targets.contains_key(&current_path) {
            out.insert(current_path.clone());
        }

        // Find all targets of current.
        for edge in &self.edges {
            if edge.start == current_path {
                // FIXME: is there a way to check for existance up front? Otherwise we need to propogate this error.
                ensure!(
                    self.nodes.contains_key(&edge.end),
                    "dataflow error: source node {} does not exist",
                    &edge.end
                );
                let next = &self.nodes[&edge.end];
                self._connected_at(next, targets, visited, out)?;
            }
        }

        return Ok(());
    }
}
