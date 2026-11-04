//! The graph measures of a stage (§6.2), over the refined graph H (rooms and junctions as nodes, corridor branches as edges).

use crate::generate::*;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, VecDeque};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub rooms: usize,
    pub leaf_chambers: usize,
    pub junctions: usize,
    pub edges: usize,
    pub beta: i32,
    pub beta_max: i32,
    pub beta_target: i32,
    pub delta: f64,
    pub mean_degree: f64,
    pub leaves: usize,
    pub bridges: usize,
    pub bridge_share: f64,
    pub articulations: usize,
    pub hops_to_stairs: Option<f64>,
    pub diameter: f64,
    pub routes_to_stairs: usize,
    pub lambda2: f64,
    pub by_class: BTreeMap<String, usize>,
}

struct Tarjan<'a> {
    adj: &'a [Vec<(usize, usize)>],
    disc: Vec<i32>,
    low: Vec<i32>,
    time: i32,
    bridges: usize,
    cut: Vec<bool>,
}

impl Tarjan<'_> {
    fn dfs(&mut self, u: usize, via: Option<usize>) {
        self.disc[u] = self.time;
        self.low[u] = self.time;
        self.time += 1;
        let mut children = 0;
        for &(to, id) in &self.adj[u] {
            if Some(id) == via {
                continue;
            }
            if self.disc[to] < 0 {
                children += 1;
                self.dfs(to, Some(id));
                self.low[u] = self.low[u].min(self.low[to]);
                if self.low[to] > self.disc[u] {
                    self.bridges += 1;
                }
                if via.is_some() && self.low[to] >= self.disc[u] {
                    self.cut[u] = true;
                }
            } else {
                self.low[u] = self.low[u].min(self.disc[to]);
            }
        }
        if via.is_none() && children > 1 {
            self.cut[u] = true;
        }
    }
}

pub fn metrics_of(d: &Dungeon) -> Metrics {
    let live: Vec<usize> = (0..d.nodes.len()).filter(|i| match d.nodes[*i] { Node::Junction { .. } => true, Node::Room { room } => d.rooms[room].alive }).collect();
    let (v, e) = (live.len() as i32, d.edges.len() as i32);
    let rooms: Vec<&Room> = d.rooms.iter().filter(|r| r.alive).collect();
    let beta = e - v + 1;

    let mut adj: Vec<Vec<(usize, usize)>> = vec![Vec::new(); d.nodes.len()];
    for ed in &d.edges {
        adj[ed.u].push((ed.v, ed.id));
        adj[ed.v].push((ed.u, ed.id));
    }
    let node_of = |room: usize| d.nodes.iter().position(|n| matches!(n, Node::Room { room: r } if *r == room)).unwrap();
    let (start_node, stairs_node) = (node_of(d.start), node_of(d.stairs));
    let mut t = Tarjan { adj: &adj, disc: vec![-1; d.nodes.len()], low: vec![0; d.nodes.len()], time: 0, bridges: 0, cut: vec![false; d.nodes.len()] };
    t.dfs(start_node, None);

    let from_start = room_hops(&d.nodes, &d.edges, start_node);
    let mut diameter = 0.0f64;
    for r in &rooms {
        for (i, h) in room_hops(&d.nodes, &d.edges, node_of(r.id)).into_iter().enumerate() {
            if matches!(d.nodes[i], Node::Room { .. }) && h.is_finite() {
                diameter = diameter.max(h);
            }
        }
    }

    // Edge-disjoint routes start → stairs (Menger): unit-capacity max flow on H.
    let mut cap: HashMap<(usize, usize), i32> = HashMap::new();
    for ed in &d.edges {
        *cap.entry((ed.u, ed.v)).or_insert(0) += 1;
        *cap.entry((ed.v, ed.u)).or_insert(0) += 1;
    }
    let mut routes = 0;
    loop {
        let mut prev: HashMap<usize, Option<usize>> = HashMap::from([(start_node, None)]);
        let mut q: VecDeque<usize> = VecDeque::from([start_node]);
        while let Some(u) = q.pop_front() {
            if prev.contains_key(&stairs_node) {
                break;
            }
            for &(v, _) in &adj[u] {
                if !prev.contains_key(&v) && *cap.get(&(u, v)).unwrap_or(&0) > 0 {
                    prev.insert(v, Some(u));
                    q.push_back(v);
                }
            }
        }
        if !prev.contains_key(&stairs_node) {
            break;
        }
        let mut v = stairs_node;
        while v != start_node {
            let u = prev[&v].unwrap();
            *cap.get_mut(&(u, v)).unwrap() -= 1;
            *cap.entry((v, u)).or_insert(0) += 1;
            v = u;
        }
        routes += 1;
    }

    // λ₂ of the room graph's Laplacian (rooms joined through any corridor), by Jacobi rotations.
    let idx: HashMap<usize, usize> = rooms.iter().enumerate().map(|(i, r)| (r.id, i)).collect();
    let n = rooms.len();
    let mut lap = vec![vec![0.0f64; n]; n];
    for r in &rooms {
        let h = room_hops(&d.nodes, &d.edges, node_of(r.id));
        for o in &rooms {
            if o.id > r.id && h[node_of(o.id)] == 1.0 {
                let (a, b) = (idx[&r.id], idx[&o.id]);
                lap[a][b] -= 1.0;
                lap[b][a] -= 1.0;
                lap[a][a] += 1.0;
                lap[b][b] += 1.0;
            }
        }
    }
    let mut eig = jacobi(lap);
    eig.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut by_class: BTreeMap<String, usize> = BTreeMap::new();
    for r in &rooms {
        *by_class.entry(r.cls.name().to_string()).or_insert(0) += 1;
    }
    let hops = from_start[stairs_node];
    Metrics {
        rooms: rooms.iter().filter(|r| !r.leaf).count(),
        leaf_chambers: rooms.iter().filter(|r| r.leaf).count(),
        junctions: d.nodes.iter().filter(|n| matches!(n, Node::Junction { .. })).count(),
        edges: d.edges.len(),
        beta,
        beta_max: d.beta_max,
        beta_target: d.beta_target,
        delta: if d.beta_max > 0 { beta as f64 / d.beta_max as f64 } else { 0.0 },
        mean_degree: rooms.iter().map(|r| r.used.len() as f64).sum::<f64>() / rooms.len().max(1) as f64,
        leaves: rooms.iter().filter(|r| r.used.len() == 1).count(),
        bridges: t.bridges,
        bridge_share: if e > 0 { t.bridges as f64 / e as f64 } else { 0.0 },
        articulations: (0..d.nodes.len()).filter(|i| t.cut[*i] && matches!(d.nodes[*i], Node::Room { .. })).count(),
        hops_to_stairs: if hops.is_finite() { Some(hops) } else { None },
        diameter,
        routes_to_stairs: routes,
        lambda2: if eig.len() > 1 { eig[1] } else { 0.0 },
        by_class,
    }
}

/// The eigenvalues of a small symmetric matrix (cyclic Jacobi).
fn jacobi(mut a: Vec<Vec<f64>>) -> Vec<f64> {
    let n = a.len();
    for _ in 0..60 {
        let mut off = 0.0;
        for i in 0..n {
            for j in (i + 1)..n {
                off += a[i][j] * a[i][j];
            }
        }
        if off < 1e-18 {
            break;
        }
        for p in 0..n {
            for q in (p + 1)..n {
                if a[p][q].abs() < 1e-15 {
                    continue;
                }
                let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
                let sign = if theta >= 0.0 { 1.0 } else { -1.0 };
                let t = sign / (theta.abs() + (theta * theta + 1.0).sqrt());
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for row in a.iter_mut() {
                    let (akp, akq) = (row[p], row[q]);
                    row[p] = c * akp - s * akq;
                    row[q] = s * akp + c * akq;
                }
                for k in 0..n {
                    let (apk, aqk) = (a[p][k], a[q][k]);
                    a[p][k] = c * apk - s * aqk;
                    a[q][k] = s * apk + c * aqk;
                }
            }
        }
    }
    (0..n).map(|i| a[i][i]).collect()
}
