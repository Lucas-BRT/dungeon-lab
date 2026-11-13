//! The graph H over rooms and junctions.

use super::*;

/// Room-to-room hops from node `from` over H (a junction passes through without counting).
pub fn room_hops(nodes: &[Node], edges: &[Edge], from: usize) -> Vec<f64> {
    let mut adj = vec![Vec::new(); nodes.len()];
    for e in edges {
        adj[e.u].push(e.v);
        adj[e.v].push(e.u);
    }
    let mut out = vec![f64::INFINITY; nodes.len()];
    out[from] = 0.0;
    let mut dq: VecDeque<usize> = VecDeque::new();
    dq.push_back(from);
    while let Some(u) = dq.pop_front() {
        for &v in &adj[u] {
            let is_room = matches!(nodes[v], Node::Room { .. });
            let w = out[u] + if is_room { 1.0 } else { 0.0 };
            if w < out[v] {
                out[v] = w;
                if is_room {
                    dq.push_back(v);
                } else {
                    dq.push_front(v);
                }
            }
        }
    }
    out
}
