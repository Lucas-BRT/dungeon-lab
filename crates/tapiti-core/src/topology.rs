//! Graph operations used by topology profiles.

use crate::generate::{Edge, Node};
use std::collections::VecDeque;

pub fn shortest_path(
    node_count: usize,
    edges: &[Edge],
    start: usize,
    goal: usize,
) -> Option<(Vec<usize>, Vec<usize>)> {
    if start >= node_count || goal >= node_count {
        return None;
    }
    let mut adj = vec![Vec::<(usize, usize)>::new(); node_count];
    for (i, edge) in edges.iter().enumerate() {
        adj[edge.u].push((edge.v, i));
        adj[edge.v].push((edge.u, i));
    }
    let mut parent = vec![None::<(usize, usize)>; node_count];
    let mut seen = vec![false; node_count];
    let mut queue = VecDeque::from([start]);
    seen[start] = true;
    while let Some(u) = queue.pop_front() {
        if u == goal {
            break;
        }
        for &(v, edge) in &adj[u] {
            if !seen[v] {
                seen[v] = true;
                parent[v] = Some((u, edge));
                queue.push_back(v);
            }
        }
    }
    if !seen[goal] {
        return None;
    }
    let mut nodes = vec![goal];
    let mut path_edges = Vec::new();
    let mut at = goal;
    while at != start {
        let (prev, edge) = parent[at]?;
        path_edges.push(edge);
        nodes.push(prev);
        at = prev;
    }
    nodes.reverse();
    path_edges.reverse();
    Some((nodes, path_edges))
}

#[derive(Clone, Copy)]
struct Arc {
    to: usize,
    reverse: usize,
    capacity: i32,
}

fn add_arc(graph: &mut [Vec<Arc>], u: usize, v: usize, capacity: i32) {
    let uv = graph[u].len();
    let vu = graph[v].len();
    graph[u].push(Arc {
        to: v,
        reverse: vu,
        capacity,
    });
    graph[v].push(Arc {
        to: u,
        reverse: uv,
        capacity: 0,
    });
}

fn level_graph(graph: &[Vec<Arc>], source: usize) -> Vec<i32> {
    let mut level = vec![-1; graph.len()];
    let mut queue = VecDeque::from([source]);
    level[source] = 0;
    while let Some(u) = queue.pop_front() {
        for arc in &graph[u] {
            if arc.capacity > 0 && level[arc.to] < 0 {
                level[arc.to] = level[u] + 1;
                queue.push_back(arc.to);
            }
        }
    }
    level
}

fn push_flow(
    graph: &mut [Vec<Arc>],
    level: &[i32],
    next: &mut [usize],
    u: usize,
    sink: usize,
    limit: i32,
) -> i32 {
    if u == sink {
        return limit;
    }
    while next[u] < graph[u].len() {
        let i = next[u];
        let arc = graph[u][i];
        if arc.capacity > 0 && level[arc.to] == level[u] + 1 {
            let flow = push_flow(graph, level, next, arc.to, sink, limit.min(arc.capacity));
            if flow > 0 {
                graph[u][i].capacity -= flow;
                graph[arc.to][arc.reverse].capacity += flow;
                return flow;
            }
        }
        next[u] += 1;
    }
    0
}

/// Returns edge connectivity and the residual source/sink regions. Adding an
/// edge between those two regions crosses every current minimum s-t cut, so it
/// raises the number of edge-disjoint routes by one.
pub fn edge_connectivity_frontiers(
    node_count: usize,
    edges: &[Edge],
    source: usize,
    sink: usize,
) -> (usize, Vec<bool>, Vec<bool>) {
    // The same node twice (an entrance with no corridor to leave by, so the exit stayed on it) has no routes to count; the flow
    // below would never end.
    if source == sink {
        return (0, vec![false; node_count], vec![false; node_count]);
    }
    let mut graph = vec![Vec::<Arc>::new(); node_count];
    for edge in edges {
        // An undirected unit edge is represented by one unit-capacity arc in
        // each direction, each with its own residual reverse arc.
        add_arc(&mut graph, edge.u, edge.v, 1);
        add_arc(&mut graph, edge.v, edge.u, 1);
    }
    let mut flow = 0usize;
    loop {
        let level = level_graph(&graph, source);
        if level[sink] < 0 {
            break;
        }
        let mut next = vec![0; node_count];
        loop {
            let sent = push_flow(&mut graph, &level, &mut next, source, sink, i32::MAX);
            if sent == 0 {
                break;
            }
            flow += sent as usize;
        }
    }

    let mut from_source = vec![false; node_count];
    let mut queue = VecDeque::from([source]);
    from_source[source] = true;
    while let Some(u) = queue.pop_front() {
        for arc in &graph[u] {
            if arc.capacity > 0 && !from_source[arc.to] {
                from_source[arc.to] = true;
                queue.push_back(arc.to);
            }
        }
    }
    let mut reverse_residual = vec![Vec::new(); node_count];
    for (u, arcs) in graph.iter().enumerate() {
        for arc in arcs {
            if arc.capacity > 0 {
                reverse_residual[arc.to].push(u);
            }
        }
    }
    let mut to_sink = vec![false; node_count];
    let mut queue = VecDeque::from([sink]);
    to_sink[sink] = true;
    while let Some(v) = queue.pop_front() {
        for &u in &reverse_residual[v] {
            if !to_sink[u] {
                to_sink[u] = true;
                queue.push_back(u);
            }
        }
    }
    (flow, from_source, to_sink)
}

/// Counts components hanging from exactly one edge of the designated main
/// path, and the greatest room-hop depth among those components.
pub fn side_branch_stats(nodes: &[Node], edges: &[Edge], main_path: &[usize]) -> (usize, usize) {
    let mut on_path = vec![false; nodes.len()];
    for &node in main_path {
        if node < on_path.len() {
            on_path[node] = true;
        }
    }
    if main_path.is_empty() {
        return (0, 0);
    }
    let mut adj = vec![Vec::new(); nodes.len()];
    for edge in edges {
        adj[edge.u].push(edge.v);
        adj[edge.v].push(edge.u);
    }
    let mut component_of = vec![usize::MAX; nodes.len()];
    let mut components: Vec<Vec<usize>> = Vec::new();
    for root in 0..nodes.len() {
        if on_path[root] || component_of[root] != usize::MAX {
            continue;
        }
        let component_id = components.len();
        let mut component = Vec::new();
        let mut queue = VecDeque::from([root]);
        component_of[root] = component_id;
        while let Some(u) = queue.pop_front() {
            component.push(u);
            for &v in &adj[u] {
                if !on_path[v] && component_of[v] == usize::MAX {
                    component_of[v] = component_id;
                    queue.push_back(v);
                }
            }
        }
        components.push(component);
    }

    let mut boundary_count = vec![0usize; components.len()];
    let mut boundary_node = vec![usize::MAX; components.len()];
    for edge in edges {
        let (u_component, v_component) = (component_of[edge.u], component_of[edge.v]);
        if u_component != usize::MAX && on_path[edge.v] {
            boundary_count[u_component] += 1;
            boundary_node[u_component] = edge.u;
        } else if v_component != usize::MAX && on_path[edge.u] {
            boundary_count[v_component] += 1;
            boundary_node[v_component] = edge.v;
        }
    }

    let mut branches = 0;
    let mut max_depth = 0;
    let mut distance = vec![usize::MAX; nodes.len()];
    for (component_id, component) in components.iter().enumerate() {
        if boundary_count[component_id] != 1 {
            continue;
        }
        branches += 1;
        let root = boundary_node[component_id];
        distance[root] = usize::from(matches!(nodes[root], Node::Room { .. }));
        let mut queue = VecDeque::from([root]);
        while let Some(u) = queue.pop_front() {
            max_depth = max_depth.max(distance[u]);
            for &v in &adj[u] {
                if component_of[v] == component_id && distance[v] == usize::MAX {
                    distance[v] = distance[u] + usize::from(matches!(nodes[v], Node::Room { .. }));
                    queue.push_back(v);
                }
            }
        }
        for &node in component {
            distance[node] = usize::MAX;
        }
    }
    (branches, max_depth)
}

pub fn max_degree(node_count: usize, edges: &[Edge]) -> usize {
    let mut degree = vec![0usize; node_count];
    for edge in edges {
        degree[edge.u] += 1;
        degree[edge.v] += 1;
    }
    degree.into_iter().max().unwrap_or(0)
}

pub fn cycle_girth(node_count: usize, edges: &[Edge]) -> Option<usize> {
    let mut best = usize::MAX;
    let mut adj = vec![Vec::<(usize, usize)>::new(); node_count];
    for (i, edge) in edges.iter().enumerate() {
        adj[edge.u].push((edge.v, i));
        adj[edge.v].push((edge.u, i));
    }
    for (excluded, edge) in edges.iter().enumerate() {
        let mut distance = vec![usize::MAX; node_count];
        let mut queue = VecDeque::from([edge.u]);
        distance[edge.u] = 0;
        while let Some(u) = queue.pop_front() {
            if distance[u] + 1 >= best {
                continue;
            }
            for &(v, i) in &adj[u] {
                if i == excluded {
                    continue;
                }
                if v == edge.v {
                    best = best.min(distance[u] + 1);
                } else if distance[v] == usize::MAX {
                    distance[v] = distance[u] + 1;
                    queue.push_back(v);
                }
            }
        }
    }
    (best != usize::MAX).then(|| best + 1)
}
