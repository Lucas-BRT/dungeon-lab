//! The dungeon model's generator (`docs/dungeon-model.md` §7): valid by construction. Every step only makes choices that keep the
//! constraints already fixed, so nothing is drawn and then thrown away whole. Deterministic: the same seed and Λ give the same stage.

use crate::geom::*;
use crate::params::*;
use crate::rng::{Const, Mulberry, Rand};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::f64::consts::PI;
use std::rc::Rc;

/// A door (§3): where on the room's outline a corridor meets it and the direction (0..7, ×45°) it leaves in.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Port {
    pub room: usize,
    pub p: V,
    pub k: i32,
    pub edge: i32,
    pub embed: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Start,
    Stairs,
    Ambush,
    Plain,
    Trap,
    Prize,
    Empty,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Room {
    pub id: usize,
    pub cls: ShapeClass,
    pub shape: Shape,
    #[serde(skip)]
    pub bb: Bounded,
    pub center: V,
    pub area: f64,
    #[serde(rename = "box")]
    pub rect: Rect,
    /// Admissible doors, sampled `portStep` px apart along each edge (the set is continuous; these are the positions tried).
    pub ports: Vec<Port>,
    pub used: Vec<Port>,
    pub leaf: bool,
    pub alive: bool,
    pub role: Role,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Node {
    Room { room: usize },
    Junction { p: V },
}

/// A branch of a corridor between two nodes of H (§5.1): a room or a junction at each end.
#[derive(Clone, Debug, Serialize)]
pub struct Edge {
    pub id: usize,
    pub u: usize,
    pub v: usize,
    pub pts: Vec<V>,
    pub ends: [Option<Port>; 2],
    pub region: Vec<Bounded>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum RouteType {
    I,
    L,
    Z,
}

#[derive(Clone, Debug, Serialize)]
pub struct Candidate {
    pub a: usize,
    pub b: usize,
    pub ports: [Port; 2],
    pub pts: Vec<V>,
    #[serde(rename = "type")]
    pub kind: RouteType,
    pub cost: f64,
    pub weight: f64,
    pub region: Vec<Bounded>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dungeon {
    pub params: Params,
    pub seed: u32,
    pub rooms: Vec<Room>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub start: usize,
    pub stairs: usize,
    pub beta_max: i32,
    pub beta_target: i32,
    pub candidates: Vec<Candidate>,
    pub pairs: Vec<[usize; 2]>,
    pub dropped: Vec<V>,
    pub log: Vec<String>,
}

/* ---------------- Shapes ---------------- */

#[derive(Clone, Copy)]
struct Bx {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

fn poly(pts: Vec<V>) -> Shape {
    Shape::Poly { pts }
}

/// fit_κ (§2.2): a shape of class `cls` inside `b`, its proportions drawn from `rng`. None when the class doesn't fit (the caller falls back to a rectangle).
fn fit(cls: ShapeClass, b: Bx, rng: &mut dyn Rand, p: &Params, cells: i32) -> Option<Shape> {
    let w_ = b.x1 - b.x0;
    let h_ = b.y1 - b.y0;
    let iota = p.width / 2.0 + p.clear;
    let min_side = 2.0 * iota + p.width;
    if w_ < min_side || h_ < min_side {
        return None;
    }
    match cls {
        ShapeClass::Rect => {
            let w = rng.range(if cells > 1 { w_ * 0.7 } else { w_.min(min_side.max(w_ * 0.55)) }, w_);
            let h = rng.range(if cells > 1 { h_ * 0.7 } else { h_.min(min_side.max(h_ * 0.55)) }, h_);
            let x = b.x0 + rng.next() * (w_ - w);
            let y = b.y0 + rng.next() * (h_ - h);
            Some(poly(vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]]))
        }
        ShapeClass::Disc => {
            let r = (w_.min(h_) / 2.0) * rng.range(0.8, 1.0);
            if r < p.width {
                return None;
            }
            let cx = b.x0 + r + rng.next() * (w_ - 2.0 * r);
            let cy = b.y0 + r + rng.next() * (h_ - 2.0 * r);
            Some(Shape::Disc { c: [cx, cy], r })
        }
        ShapeClass::Oct => {
            // Vertices at 22.5° + k·45°: the edges face the 8 directions of the alphabet.
            let half = (w_.min(h_) / 2.0) * rng.range(0.8, 1.0);
            let rho = half / (PI / 8.0).cos();
            // Every edge must hold a door: |e| = 2ρ·sin(22.5°) ≥ 2ι.
            if 2.0 * rho * (PI / 8.0).sin() < 2.0 * iota + 4.0 {
                return None;
            }
            let cx = b.x0 + half + rng.next() * (w_ - 2.0 * half);
            let cy = b.y0 + half + rng.next() * (h_ - 2.0 * half);
            let pts = (0..8)
                .map(|i| {
                    let a = PI / 8.0 + i as f64 * PI / 4.0;
                    add([cx, cy], mul([a.cos(), a.sin()], rho))
                })
                .collect();
            Some(poly(pts))
        }
        ShapeClass::L => {
            let w = rng.range(w_ * 0.75, w_);
            let h = rng.range(h_ * 0.75, h_);
            let a = w * rng.range(0.35, 0.55);
            let n = h * rng.range(0.35, 0.55);
            let x = b.x0 + rng.next() * (w_ - w);
            let y = b.y0 + rng.next() * (h_ - h);
            // The notch at the top-left corner, then mirrored to the corner drawn.
            let local: [V; 6] = [[a, 0.0], [w, 0.0], [w, h], [0.0, h], [0.0, n], [a, n]];
            let corner = (rng.next() * 4.0).floor() as i32;
            let fx = corner == 1 || corner == 2;
            let fy = corner == 2 || corner == 3;
            Some(poly(local.iter().map(|&[px, py]| [x + if fx { w - px } else { px }, y + if fy { h - py } else { py }]).collect()))
        }
        ShapeClass::Star => {
            if cells < p.star_cells {
                return None;
            }
            let n = if p.m == 8 && rng.next() < 0.5 { 8 } else { 4 };
            let rho = (w_.min(h_) / 2.0) * rng.range(0.9, 1.0);
            let inner = rho * rng.range(0.45, 0.55);
            let tau = 2.0 * iota + 8.0;
            let spread = (tau / 2.0).atan2(rho);
            if spread > (PI / n as f64) * 0.6 {
                return if n == 8 {
                    let mut q = p.clone();
                    q.m = 4;
                    fit(ShapeClass::Star, b, &mut Const(0.9), &q, cells)
                } else {
                    None
                };
            }
            let cx = b.x0 + rho + rng.next() * (w_ - 2.0 * rho);
            let cy = b.y0 + rho + rng.next() * (h_ - 2.0 * rho);
            let c = [cx, cy];
            let mut pts: Vec<V> = Vec::new();
            for k in 0..n {
                let psi = 2.0 * PI * k as f64 / n as f64;
                let tip = mul([psi.cos(), psi.sin()], rho);
                let side = mul([-psi.sin(), psi.cos()], tau / 2.0);
                let mid = psi + PI / n as f64;
                pts.push(add(c, sub(tip, side)));
                pts.push(add(c, add(tip, side)));
                pts.push(add(c, mul([mid.cos(), mid.sin()], inner)));
            }
            Some(poly(pts))
        }
    }
}

/* ---------------- Doors ---------------- */

/// The exit band F(p, θ) of §3.2, starting `from` px out of the wall.
fn exit_band(p: V, k: i32, pr: &Params, from: f64) -> Shape {
    band(add(p, mul(dir_of(k), from)), add(p, mul(dir_of(k), pr.exit)), pr.width)
}

/// Π(S) sampled (§3.1): the doors of every edge whose outward normal is in the alphabet, `portStep` apart inside [ι, |e| − ι], whose exit band clears the room.
fn ports_of(room: usize, s: &Shape, pr: &Params) -> Vec<Port> {
    let mut out = Vec::new();
    let iota = pr.width / 2.0 + pr.clear;
    let allowed = |k: i32| pr.m == 8 || k % 2 == 0;
    match s {
        Shape::Disc { c, r } => {
            let step = 8 / pr.disc_ports.max(1);
            // A flat corridor end on a round wall: the corridor starts this far inside so no wedge of rock is left at its sides.
            let embed = r - (r * r - (pr.width / 2.0) * (pr.width / 2.0)).max(0.0).sqrt() + 1.0;
            let mut k = 0;
            while k < 8 {
                if allowed(k) {
                    out.push(Port { room, p: add(*c, mul(dir_of(k), *r)), k, edge: -1, embed });
                }
                k += step;
            }
        }
        Shape::Poly { pts } => {
            let a2 = signed_area2(pts);
            let n = pts.len();
            for e in 0..n {
                let a = pts[e];
                let b = pts[(e + 1) % n];
                let l = dist(a, b);
                let d = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
                let nrm = if a2 > 0.0 { [d[1], -d[0]] } else { [-d[1], d[0]] };
                let mut k = -1;
                for i in 0..8 {
                    if (nrm[0] - dir_of(i)[0]).abs() < 1e-4 && (nrm[1] - dir_of(i)[1]).abs() < 1e-4 {
                        k = i;
                    }
                }
                if k < 0 || !allowed(k) || l < 2.0 * iota {
                    continue;
                }
                let mut t = iota;
                while t <= l - iota + 1e-9 {
                    let p = add(a, mul(d, t));
                    if shape_dist(&exit_band(p, k, pr, 0.5), s) > 0.0 {
                        out.push(Port { room, p, k, edge: e as i32, embed: 0.0 });
                    }
                    t += pr.port_step;
                }
            }
        }
    }
    out
}

/// The door of `port`'s room on the same edge, slid to where the line through `at` along direction `k` crosses that edge (doors are
/// continuous, §3.1): what makes a straight corridor exact. None when the crossing falls outside the edge's admissible interval.
fn slide(port: &Port, s: &Shape, at: V, k: i32, pr: &Params) -> Option<Port> {
    let Shape::Poly { pts } = s else { return None };
    if port.edge < 0 {
        return None;
    }
    let a = pts[port.edge as usize];
    let b = pts[(port.edge as usize + 1) % pts.len()];
    let l = dist(a, b);
    let d = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
    let u = dir_of(k);
    let den = cross(u, d);
    if den.abs() < 1e-9 {
        return None;
    }
    let t = cross(sub(a, at), d) / den;
    let q = add(at, mul(u, t));
    let along = dot(sub(q, a), d);
    let iota = pr.width / 2.0 + pr.clear;
    if along < iota || along > l - iota {
        return None;
    }
    if shape_dist(&exit_band(q, port.k, pr, 0.5), s) <= 0.0 {
        return None;
    }
    Some(Port { p: q, ..*port })
}

/* ---------------- Corridors between two doors ---------------- */

fn turn_ok(a: i32, b: i32, pr: &Params) -> bool {
    let t = turn_of(a, b);
    t == 0 || t == 2 || (t == 1 && pr.m == 8)
}

/// The ways (§4.3) to join door a to door b with at most `maxBends` bends: I, L and Z, each a polyline from p_a to p_b.
fn routes(pa: &Port, pb: &Port, pr: &Params) -> Vec<(Vec<V>, RouteType)> {
    let mut out = Vec::new();
    let ua = dir_of(pa.k);
    let in_k = (pb.k + 4) % 8;
    let uc = dir_of(in_k);
    let d = sub(pb.p, pa.p);
    // I: facing each other and on one line.
    if in_k == pa.k && cross(d, ua).abs() < 1e-6 * len(d) && dot(d, ua) >= 2.0 * pr.exit {
        out.push((vec![pa.p, pb.p], RouteType::I));
    }
    if pr.max_bends < 1 {
        return out;
    }
    // L: the two rays meet ahead of both doors.
    let ub = dir_of(pb.k);
    let den = cross(ua, ub);
    if den.abs() > 1e-9 && turn_ok(pa.k, in_k, pr) {
        let t = cross(d, ub) / den;
        let s = cross(d, ua) / den;
        if t >= pr.exit && s >= pr.exit {
            out.push((vec![pa.p, add(pa.p, mul(ua, t)), pb.p], RouteType::L));
        }
    }
    if pr.max_bends < 2 {
        return out;
    }
    // Z/U: a middle segment in any direction of the alphabet; t_a = t_b when that has a solution, else t_a = ℓs.
    for k in 0..8 {
        if pr.m == 4 && k % 2 == 1 {
            continue;
        }
        if k == pa.k || k == in_k || !turn_ok(pa.k, k, pr) || !turn_ok(k, in_k, pr) {
            continue;
        }
        let um = dir_of(k);
        let sum = add(ua, uc);
        let mut tries: Vec<[f64; 3]> = Vec::new();
        let det = cross(sum, um);
        if det.abs() > 1e-9 {
            let t = cross(d, um) / det;
            let tm = cross(sum, d) / det;
            tries.push([t, tm, t]);
        }
        let r = sub(d, mul(ua, pr.exit));
        let det2 = cross(um, uc);
        if det2.abs() > 1e-9 {
            tries.push([pr.exit, cross(r, uc) / det2, cross(um, r) / det2]);
        }
        for [ta, tm, tb] in tries {
            if ta < pr.exit || tb < pr.exit || tm < pr.seg {
                continue;
            }
            let q1 = add(pa.p, mul(ua, ta));
            let q2 = add(q1, mul(um, tm));
            out.push((vec![pa.p, q1, q2, pb.p], RouteType::Z));
            break;
        }
    }
    out
}

/// A polyline's region (§4.1), its ends pushed into round rooms by their door's `embed`.
fn region_of(pts: &[V], ends: &[Option<Port>; 2], pr: &Params) -> Vec<Bounded> {
    let mut p = pts.to_vec();
    let n = p.len();
    let push = |p: &mut Vec<V>, i: usize, j: usize, port: &Option<Port>| {
        if let Some(port) = port {
            if port.embed > 0.0 {
                let l = dist(p[j], p[i]);
                p[i] = add(p[i], mul(sub(p[i], p[j]), port.embed / l));
            }
        }
    };
    push(&mut p, 0, 1, &ends[0]);
    push(&mut p, n - 1, n - 2, &ends[1]);
    corridor_region(&p, pr.width)
}

/// The same region with its room ends pulled back out of the rooms, to test it against them.
pub fn outside_region(pts: &[V], ends: &[Option<Port>; 2], pr: &Params) -> Vec<Bounded> {
    corridor_region(&trim_ends(pts, if ends[0].is_some() { 1.0 } else { 0.0 }, if ends[1].is_some() { 1.0 } else { 0.0 }), pr.width)
}

/* ---------------- The graph H ---------------- */

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

/* ---------------- The generator ---------------- */

#[derive(Clone, Copy)]
struct Block {
    c: i32,
    r: i32,
    w: i32,
    h: i32,
}

enum Action {
    Add(usize),
    Branch { room: usize, port: Port, edge: usize, seg: usize, h: V },
    Cross { cand: usize, edge: usize, i: usize, seg: usize, h: V },
}

struct Opt {
    weight: f64,
    act: Action,
}

struct Gen<'a> {
    p: &'a Params,
    rng: Mulberry,
    log: Vec<String>,
    rooms: Vec<Room>,
    nodes: Vec<Node>,
    node_of_room: Vec<usize>,
    edges: Vec<Edge>,
    cache: HashMap<(usize, usize), Rc<Vec<Candidate>>>,
    statics: Vec<Candidate>,
    stairs_room: usize,
}

impl<'a> Gen<'a> {
    fn pick_class(&mut self, ws: &[f64]) -> usize {
        let sum: f64 = ws.iter().sum();
        if sum <= 0.0 {
            return 0;
        }
        let mut x = self.rng.next() * sum;
        for (i, w) in ws.iter().enumerate() {
            x -= w;
            if x < 0.0 {
                return i;
            }
        }
        ws.len() - 1
    }

    fn mix_of(&self, a: usize, b: usize) -> f64 {
        *self.p.mix.get(&mix_key(self.rooms[a].cls, self.rooms[b].cls)).unwrap_or(&1.0)
    }

    fn type_weight(&self, t: RouteType) -> f64 {
        match t {
            RouteType::I => self.p.p_i,
            RouteType::L => self.p.p_l,
            RouteType::Z => self.p.p_z,
        }
    }

    fn proximity_pairs(&self, kind: Proximity, near: f64) -> Vec<[usize; 2]> {
        let mut out = Vec::new();
        let d2 = |a: V, b: V| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2);
        let n = self.rooms.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let (ci, cj) = (self.rooms[i].center, self.rooms[j].center);
                let dij = d2(ci, cj);
                let mut ok = true;
                if kind == Proximity::Near {
                    ok = dij <= near * near;
                } else {
                    for k in 0..n {
                        if !ok {
                            break;
                        }
                        if k == i || k == j {
                            continue;
                        }
                        let ck = self.rooms[k].center;
                        ok = if kind == Proximity::Rng { !(d2(ci, ck).max(d2(cj, ck)) < dij) } else { !(d2(ck, mul(add(ci, cj), 0.5)) < dij / 4.0) };
                    }
                }
                if ok {
                    out.push([i, j]);
                }
            }
        }
        out
    }

    /// The corridors that may join rooms a and b (§7.3 steps 2 to 5), the best `keep` by cost, each with distinct doors.
    fn candidates_for(&self, a: usize, b: usize, keep: usize) -> Vec<Candidate> {
        let pr = self.p;
        let (ra, rb) = (&self.rooms[a], &self.rooms[b]);
        if self.mix_of(a, b) <= 0.0 || shape_dist(&ra.shape, &rb.shape) < pr.min_len {
            return Vec::new();
        }
        let facing = |r: &Room, other: &Room| -> Vec<Port> {
            let to = sub(other.center, r.center);
            let ps: Vec<Port> = r.ports.iter().filter(|p| dot(dir_of(p.k), to) > 0.0).copied().collect();
            let stride = ((ps.len() as f64 / 14.0).ceil() as usize).max(1);
            ps.into_iter().enumerate().filter(|(i, _)| i % stride == 0).map(|(_, p)| p).collect()
        };
        let (fa, fb) = (facing(ra, rb), facing(rb, ra));
        let mut tries: Vec<(Port, Port)> = Vec::new();
        for pa in &fa {
            for pb in &fb {
                tries.push((*pa, *pb));
                // A straight corridor needs the two doors on one line: slide one of them along its edge to meet the other's.
                if pb.k == (pa.k + 4) % 8 && pr.p_i > 0.0 {
                    if let Some(sb) = slide(pb, &rb.shape, pa.p, pa.k, pr) {
                        tries.push((*pa, sb));
                    } else if let Some(sa) = slide(pa, &ra.shape, pb.p, pb.k, pr) {
                        tries.push((sa, *pb));
                    }
                }
            }
        }
        let mut out: Vec<Candidate> = Vec::new();
        let (sa, sb) = (&ra.bb, &rb.bb);
        let others: Vec<&Bounded> = self.rooms.iter().filter(|o| o.id != a && o.id != b).map(|o| &o.bb).collect();
        for (pa, pb) in tries {
            if dot(dir_of(pa.k), sub(pb.p, pa.p)) <= 0.0 || dot(dir_of(pb.k), sub(pa.p, pb.p)) <= 0.0 {
                continue;
            }
            for (pts, kind) in routes(&pa, &pb, pr) {
                if self.type_weight(kind) <= 0.0 {
                    continue;
                }
                let segs: Vec<f64> = pts.windows(2).map(|w| dist(w[0], w[1])).collect();
                if segs[0] < pr.exit || segs[segs.len() - 1] < pr.exit || segs.iter().skip(1).take(segs.len().saturating_sub(2)).any(|l| *l < pr.seg) {
                    continue;
                }
                let length = polyline_length(&pts);
                if length < pr.min_len {
                    continue;
                }
                let ends = [Some(pa), Some(pb)];
                let region = region_of(&pts, &ends, pr);
                let outside = outside_region(&pts, &ends, pr);
                let mut ok = region_dist(&outside, std::slice::from_ref(sa)) > 0.0 && region_dist(&outside, std::slice::from_ref(sb)) > 0.0;
                if ok {
                    for o in &others {
                        if region_dist(&region, std::slice::from_ref(*o)) < pr.clear {
                            ok = false;
                            break;
                        }
                    }
                }
                if !ok {
                    continue;
                }
                let bends = (pts.len() - 2) as f64;
                let dirs: Vec<i32> = pts.windows(2).map(|w| seg_dir(w[0], w[1])).collect();
                let forty_five = dirs.windows(2).filter(|w| turn_of(w[0], w[1]) == 1).count() as f64;
                let cost = pr.cost_bend * bends + pr.cost_len * length + pr.cost45 * forty_five - if kind == RouteType::I { pr.cost_facing } else { 0.0 };
                out.push(Candidate { a, b, ports: [pa, pb], pts, kind, cost, weight: self.mix_of(a, b) * self.type_weight(kind) * (-cost).exp(), region });
            }
        }
        out.sort_by(|x, y| x.cost.partial_cmp(&y.cost).unwrap());
        let mut kept: Vec<Candidate> = Vec::new();
        for c in out {
            if kept.len() >= keep {
                break;
            }
            if kept.iter().any(|k| dist(k.ports[0].p, c.ports[0].p) < pr.width || dist(k.ports[1].p, c.ports[1].p) < pr.width) {
                continue;
            }
            kept.push(c);
        }
        kept
    }

    fn cand_of(&mut self, a: usize, b: usize) -> Rc<Vec<Candidate>> {
        if let Some(c) = self.cache.get(&(a, b)) {
            return c.clone();
        }
        let c = Rc::new(self.candidates_for(a, b, 4));
        self.cache.insert((a, b), c.clone());
        c
    }

    fn add_edge(&mut self, u: usize, v: usize, pts: Vec<V>, ends: [Option<Port>; 2]) {
        let region = region_of(&pts, &ends, self.p);
        let id = self.edges.len();
        for p in ends.iter().flatten() {
            self.rooms[p.room].used.push(*p);
        }
        self.edges.push(Edge { id, u, v, pts, ends, region });
    }

    /// Whether door `p` keeps its exit band σ away from every door already used in its room (V3).
    fn port_free(&self, p: &Port) -> bool {
        let pr = self.p;
        let mine = exit_band(p.p, p.k, pr, 0.0);
        self.rooms[p.room].used.iter().all(|q| shape_dist(&mine, &exit_band(q.p, q.k, pr, 0.0)) >= pr.clear)
    }

    /// Whether a candidate fits what is already built (§7.4): free doors, and σ from every corridor.
    fn fits_now(&self, c: &Candidate) -> bool {
        if !self.port_free(&c.ports[0]) || !self.port_free(&c.ports[1]) {
            return false;
        }
        self.edges.iter().all(|e| region_dist(&c.region, &e.region) >= self.p.clear)
    }

    fn degree(&self, r: usize) -> usize {
        self.rooms[r].used.len()
    }

    fn locked_full(&self, r: usize) -> bool {
        r == self.stairs_room && self.degree(r) >= self.p.stairs_doors as usize
    }

    fn weighted(&mut self, ws: &[f64]) -> Option<usize> {
        let sum: f64 = ws.iter().sum();
        if ws.is_empty() || sum <= 0.0 {
            return None;
        }
        let mut x = self.rng.next() * sum;
        for (i, w) in ws.iter().enumerate() {
            x -= w;
            if x < 0.0 {
                return Some(i);
            }
        }
        Some(ws.len() - 1)
    }

    /// Splits edge `e` at `h` on its segment `seg`: a junction node there, `e` up to it and a new edge on to where `e` went. Returns the junction.
    fn split_edge(&mut self, e: usize, seg: usize, h: V) -> usize {
        let j = self.nodes.len();
        self.nodes.push(Node::Junction { p: h });
        let old_v = self.edges[e].v;
        let old_end = self.edges[e].ends[1];
        let mut first: Vec<V> = self.edges[e].pts[..=seg].to_vec();
        first.push(h);
        let mut second: Vec<V> = vec![h];
        second.extend_from_slice(&self.edges[e].pts[seg + 1..]);
        let end0 = self.edges[e].ends[0];
        let mut region = region_of(&first, &[end0, None], self.p);
        region.push(bounded(joint(h, self.p.width)));
        let tail_region = region_of(&second, &[None, old_end], self.p);
        let edge = &mut self.edges[e];
        edge.pts = first;
        edge.v = j;
        edge.ends = [end0, None];
        edge.region = region;
        let id = self.edges.len();
        self.edges.push(Edge { id, u: j, v: old_v, pts: second, ends: [None, old_end], region: tail_region });
        j
    }

    fn room_of_node(&self, n: usize) -> Option<usize> {
        match self.nodes[n] {
            Node::Room { room } => Some(room),
            Node::Junction { .. } => None,
        }
    }

    /// T (§4.3): a straight arm from a free door of a room to a perpendicular corridor segment, met away from its ends.
    fn branches(&self, out: &mut Vec<Opt>) {
        let pr = self.p;
        let margin = pr.width / 2.0 + pr.seg;
        for r in &self.rooms {
            if !r.alive || self.locked_full(r.id) {
                continue;
            }
            for p in &r.ports {
                if !self.port_free(p) {
                    continue;
                }
                let u = dir_of(p.k);
                for e in &self.edges {
                    let rooms_of_e: Vec<usize> = [e.u, e.v].iter().filter_map(|n| self.room_of_node(*n)).collect();
                    if rooms_of_e.contains(&r.id) {
                        continue;
                    }
                    for s in 0..e.pts.len() - 1 {
                        let (a, b) = (e.pts[s], e.pts[s + 1]);
                        let sl = dist(a, b);
                        let ds = [(b[0] - a[0]) / sl, (b[1] - a[1]) / sl];
                        if dot(u, ds).abs() > 1e-6 {
                            continue;
                        }
                        let den = cross(u, ds);
                        let t = cross(sub(a, p.p), ds) / den;
                        let h = add(p.p, mul(u, t));
                        let along = dot(sub(h, a), ds);
                        if t < pr.exit + pr.width / 2.0 || along < margin || along > sl - margin {
                            continue;
                        }
                        let arm = [p.p, h];
                        let arm_out = corridor_region(&trim_ends(&arm, 1.0, pr.width / 2.0 + pr.clear), pr.width);
                        if region_dist(&arm_out, std::slice::from_ref(&r.bb)) <= 0.0 {
                            continue;
                        }
                        let mut ok = true;
                        for o in &self.rooms {
                            if o.alive && o.id != r.id && region_dist(&arm_out, std::slice::from_ref(&o.bb)) < pr.clear {
                                ok = false;
                                break;
                            }
                        }
                        if ok {
                            ok = self.edges.iter().all(|o| region_dist(&arm_out, &o.region) >= pr.clear);
                        }
                        if !ok {
                            continue;
                        }
                        // §4.4 between the new room and each room the corridor already reaches: straight distance and walk.
                        let to_u = polyline_length(&[&e.pts[..=s], &[h][..]].concat());
                        let to_v = polyline_length(&[&[h][..], &e.pts[s + 1..]].concat());
                        let reach: Vec<(usize, f64)> = [(e.u, t + to_u), (e.v, t + to_v)].into_iter().filter_map(|(n, w)| self.room_of_node(n).map(|room| (room, w))).collect();
                        if reach.iter().any(|(room, walk)| *walk < pr.min_len || shape_dist(&r.shape, &self.rooms[*room].shape) < pr.min_len) {
                            continue;
                        }
                        let mix_w: f64 = reach.iter().map(|(room, _)| self.mix_of(r.id, *room)).product();
                        let cost = pr.cost_len * t + pr.cost_bend;
                        out.push(Opt { weight: pr.p_t * mix_w * (-cost).exp(), act: Action::Branch { room: r.id, port: *p, edge: e.id, seg: s, h } });
                    }
                }
            }
        }
    }

    /// X (§4.3): candidate `ci` crosses exactly one corridor, at right angles, away from both segments' ends: the crossing becomes a junction.
    fn crossing(&self, ci: usize) -> Option<Action> {
        let pr = self.p;
        let c = &self.statics[ci];
        let hit: Vec<&Edge> = self.edges.iter().filter(|e| region_dist(&c.region, &e.region) < pr.clear).collect();
        if hit.len() != 1 {
            return None;
        }
        let e = hit[0];
        let margin = pr.width / 2.0 + pr.seg;
        let cut = pr.width + pr.clear;
        for i in 0..c.pts.len() - 1 {
            for s in 0..e.pts.len() - 1 {
                let (a, b, p, q) = (c.pts[i], c.pts[i + 1], e.pts[s], e.pts[s + 1]);
                if !segs_cross(a, b, p, q) {
                    continue;
                }
                let (da, dp) = (sub(b, a), sub(q, p));
                if dot(da, dp).abs() > 1e-6 * len(da) * len(dp) {
                    return None;
                }
                let t = cross(sub(p, a), dp) / cross(da, dp);
                let h = add(a, mul(da, t));
                let along_c = t * len(da);
                let along_e = dot(sub(h, p), dp) / len(dp);
                if along_c < margin || len(da) - along_c < margin || along_e < margin || len(dp) - along_e < margin {
                    return None;
                }
                // The rest of both must keep σ apart: test them with the crossing cut out.
                let with = |head: &[V], h: V, tail: bool| -> Vec<V> { if tail { [&[h][..], head].concat() } else { [head, &[h][..]].concat() } };
                let c_a = corridor_region(&trim_ends(&with(&c.pts[..=i], h, false), 0.0, cut), pr.width);
                let c_b = corridor_region(&trim_ends(&with(&c.pts[i + 1..], h, true), cut, 0.0), pr.width);
                let e_a = corridor_region(&trim_ends(&with(&e.pts[..=s], h, false), 0.0, cut), pr.width);
                let e_b = corridor_region(&trim_ends(&with(&e.pts[s + 1..], h, true), cut, 0.0), pr.width);
                for x in [&c_a, &c_b] {
                    for y in [&e_a, &e_b] {
                        if region_dist(x, y) < pr.clear {
                            return None;
                        }
                    }
                }
                let rooms_of_e: Vec<usize> = [e.u, e.v].iter().filter_map(|n| self.room_of_node(*n)).collect();
                if rooms_of_e.contains(&c.a) || rooms_of_e.contains(&c.b) {
                    return None;
                }
                for ra in [c.a, c.b] {
                    for rb in &rooms_of_e {
                        if shape_dist(&self.rooms[ra].shape, &self.rooms[*rb].shape) < pr.min_len {
                            return None;
                        }
                    }
                }
                return Some(Action::Cross { cand: ci, edge: e.id, i, seg: s, h });
            }
        }
        None
    }

    fn options(&self) -> Vec<Opt> {
        let mut out = Vec::new();
        for (ci, c) in self.statics.iter().enumerate() {
            if self.locked_full(c.a) || self.locked_full(c.b) {
                continue;
            }
            if self.fits_now(c) {
                out.push(Opt { weight: c.weight, act: Action::Add(ci) });
            } else if self.p.p_x > 0.0 && self.port_free(&c.ports[0]) && self.port_free(&c.ports[1]) {
                if let Some(act) = self.crossing(ci) {
                    out.push(Opt { weight: c.weight * self.p.p_x, act });
                }
            }
        }
        if self.p.p_t > 0.0 {
            self.branches(&mut out);
        }
        out
    }

    fn apply(&mut self, act: Action) {
        match act {
            Action::Add(ci) => {
                let c = self.statics[ci].clone();
                self.add_edge(self.node_of_room[c.a], self.node_of_room[c.b], c.pts, [Some(c.ports[0]), Some(c.ports[1])]);
            }
            Action::Branch { room, port, edge, seg, h } => {
                let j = self.split_edge(edge, seg, h);
                self.add_edge(self.node_of_room[room], j, vec![port.p, h], [Some(port), None]);
            }
            Action::Cross { cand, edge, i, seg, h } => {
                let c = self.statics[cand].clone();
                let j = self.split_edge(edge, seg, h);
                let first: Vec<V> = [&c.pts[..=i], &[h][..]].concat();
                let second: Vec<V> = [&[h][..], &c.pts[i + 1..]].concat();
                self.add_edge(self.node_of_room[c.a], j, first, [Some(c.ports[0]), None]);
                self.add_edge(j, self.node_of_room[c.b], second, [None, Some(c.ports[1])]);
            }
        }
    }
}

pub fn generate(seed: u32, p: &Params) -> Dungeon {
    let mut g = Gen { p, rng: Mulberry(seed), log: Vec::new(), rooms: Vec::new(), nodes: Vec::new(), node_of_room: Vec::new(), edges: Vec::new(), cache: HashMap::new(), statics: Vec::new(), stairs_room: usize::MAX };

    /* §7.1 Scaffold: blocks on a grid, the big shapes first, then a share of the 1×1 cells dropped (the rest kept in one piece). */
    let (cols, rows, cell_w, cell_h, gap) = (p.cols.max(1), p.rows.max(1), p.cell_w, p.cell_h, p.gap);
    let total = (cols * rows) as usize;
    let mut blocks: Vec<Block> = Vec::new();
    let mut owner = vec![-1i32; total];
    let cells_of = |b: &Block| -> Vec<usize> { (0..b.w * b.h).map(|i| ((b.r + i / b.w) * cols + b.c + i % b.w) as usize).collect() };
    let shapes: [(i32, i32); 5] = [(2, 1), (1, 2), (2, 2), (3, 2), (2, 3)];
    let (mut tries, mut made, mut used) = (0, 0, 0);
    while tries < p.big * 12 && made < p.big {
        tries += 1;
        let (w, h) = shapes[(g.rng.next() * shapes.len() as f64).floor() as usize];
        let b = Block { c: (g.rng.next() * cols as f64).floor() as i32, r: (g.rng.next() * rows as f64).floor() as i32, w, h };
        let fits = b.c + b.w <= cols && b.r + b.h <= rows && cells_of(&b).iter().all(|c| owner[*c] < 0);
        if !fits || (used + w * h) as f64 > total as f64 * 0.45 {
            continue;
        }
        blocks.push(b);
        for c in cells_of(&b) {
            owner[c] = (blocks.len() - 1) as i32;
        }
        made += 1;
        used += w * h;
    }
    for c in 0..total {
        if owner[c] < 0 {
            let b = Block { c: c as i32 % cols, r: c as i32 / cols, w: 1, h: 1 };
            blocks.push(b);
            owner[c] = (blocks.len() - 1) as i32;
        }
    }
    let touching = |i: usize| -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        for cell in cells_of(&blocks[i]) {
            let (c, r) = (cell as i32 % cols, cell as i32 / cols);
            for (dc, dr) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                if c + dc < 0 || r + dr < 0 || c + dc >= cols || r + dr >= rows {
                    continue;
                }
                let o = owner[((r + dr) * cols + c + dc) as usize] as usize;
                if o != i && !out.contains(&o) {
                    out.push(o);
                }
            }
        }
        out.sort();
        out
    };
    let mut alive = vec![true; blocks.len()];
    let whole = |alive: &[bool]| -> bool {
        let first = alive.iter().position(|a| *a).unwrap();
        let mut seen = HashSet::from([first]);
        let mut todo = vec![first];
        let mut i = 0;
        while i < todo.len() {
            for n in touching(todo[i]) {
                if alive[n] && seen.insert(n) {
                    todo.push(n);
                }
            }
            i += 1;
        }
        seen.len() == alive.iter().filter(|a| **a).count()
    };
    let mut smalls: Vec<usize> = (0..blocks.len()).filter(|i| blocks[*i].w * blocks[*i].h == 1).collect();
    for i in (1..smalls.len()).rev() {
        let j = (g.rng.next() * (i + 1) as f64).floor() as usize;
        smalls.swap(i, j);
    }
    let mut to_drop = (total as f64 * p.drop).floor() as i32;
    for &i in &smalls {
        if to_drop <= 0 {
            break;
        }
        alive[i] = false;
        if whole(&alive) {
            to_drop -= 1;
        } else {
            alive[i] = true;
        }
    }
    let box_of = |b: &Block| Bx { x0: b.c as f64 * cell_w + gap, y0: b.r as f64 * cell_h + gap, x1: (b.c + b.w) as f64 * cell_w - gap, y1: (b.r + b.h) as f64 * cell_h - gap };
    let live: Vec<usize> = (0..blocks.len()).filter(|i| alive[*i]).collect();
    let dropped: Vec<usize> = (0..blocks.len()).filter(|i| !alive[*i]).collect();

    /* §7.2 Shape classes: weights by block size, a Potts model for the homophily, then the caps. */
    let weight_of = |c: ShapeClass, cells: i32| if c == ShapeClass::Star && cells < p.star_cells { 0.0 } else { p.weights.get(c).max(0.0) };
    let mut cls: HashMap<usize, usize> = HashMap::new();
    for &i in &live {
        let cells = blocks[i].w * blocks[i].h;
        let ws: Vec<f64> = CLASSES.iter().map(|c| weight_of(*c, cells)).collect();
        let k = g.pick_class(&ws);
        cls.insert(i, k);
    }
    if p.homophily != 0.0 {
        for _sweep in 0..4 {
            for &i in &live {
                let cells = blocks[i].w * blocks[i].h;
                let nb: Vec<usize> = touching(i).into_iter().filter(|n| alive[*n]).collect();
                let ws: Vec<f64> = CLASSES.iter().enumerate().map(|(ci, c)| weight_of(*c, cells) * (p.homophily * nb.iter().filter(|n| cls.get(n) == Some(&ci)).count() as f64).exp()).collect();
                let k = g.pick_class(&ws);
                cls.insert(i, k);
            }
        }
    }
    for (ci, c) in CLASSES.iter().enumerate() {
        let mine: Vec<usize> = live.iter().copied().filter(|i| cls[i] == ci).collect();
        let cap = p.max_of.get(*c).max(0.0) as usize;
        for &i in mine.iter().skip(cap) {
            let cells = blocks[i].w * blocks[i].h;
            let ws: Vec<f64> = CLASSES.iter().enumerate().map(|(oi, o)| if oi == ci { 0.0 } else { weight_of(*o, cells) }).collect();
            let k = g.pick_class(&ws);
            cls.insert(i, k);
        }
    }

    /* Rooms: each block's shape, fitted (a rectangle when its class can't fit). */
    for &i in &live {
        let b = blocks[i];
        let bx = box_of(&b);
        let mut c = CLASSES[cls[&i]];
        let cells = b.w * b.h;
        let shape = match fit(c, bx, &mut g.rng, p, cells) {
            Some(s) => s,
            None => {
                g.log.push(format!("block {}: {} does not fit, rect instead", i, c.name()));
                c = ShapeClass::Rect;
                fit(ShapeClass::Rect, bx, &mut g.rng, p, cells).expect("a rect always fits a block")
            }
        };
        let id = g.rooms.len();
        g.rooms.push(Room { id, cls: c, bb: bounded(shape.clone()), center: centroid(&shape), area: area_of(&shape), rect: Rect { x0: bx.x0, y0: bx.y0, x1: bx.x1, y1: bx.y1 }, ports: ports_of(id, &shape, p), shape, used: Vec::new(), leaf: false, alive: true, role: Role::Plain });
    }
    g.nodes = g.rooms.iter().map(|r| Node::Room { room: r.id }).collect();
    g.node_of_room = g.rooms.iter().map(|r| r.id).collect();

    /* §7.5.1 The spanning tree: Prim from the start, weighted by ω; the proximity graph relaxed when it gets stuck. */
    let start_room = (g.rng.next() * g.rooms.len() as f64).floor() as usize;
    let order: Vec<Proximity> = match p.proximity {
        Proximity::Rng => vec![Proximity::Rng, Proximity::Gabriel, Proximity::Near],
        Proximity::Gabriel => vec![Proximity::Gabriel, Proximity::Near],
        Proximity::Near => vec![Proximity::Near],
    };
    let mut level = 0;
    let mut near = p.near;
    let mut pairs = g.proximity_pairs(order[0], near);
    let mut in_tree: HashSet<usize> = HashSet::from([start_room]);
    while in_tree.len() < g.rooms.len() {
        let mut open: Vec<Candidate> = Vec::new();
        for [a, b] in pairs.clone() {
            if in_tree.contains(&a) != in_tree.contains(&b) {
                for c in g.cand_of(a, b).iter() {
                    if g.fits_now(c) {
                        open.push(c.clone());
                    }
                }
            }
        }
        let ws: Vec<f64> = open.iter().map(|c| c.weight).collect();
        if let Some(k) = g.weighted(&ws) {
            let c = open[k].clone();
            g.add_edge(g.node_of_room[c.a], g.node_of_room[c.b], c.pts, [Some(c.ports[0]), Some(c.ports[1])]);
            in_tree.insert(c.a);
            in_tree.insert(c.b);
            continue;
        }
        if level < order.len() - 1 {
            level += 1;
            g.log.push(format!("tree stuck: candidates from {:?}", order[level]).to_lowercase());
            pairs = g.proximity_pairs(order[level], near);
        } else if near < 4.0 * cell_w.max(cell_h) {
            near *= 1.5;
            g.log.push(format!("tree stuck: near {} px", near.round()));
            pairs = g.proximity_pairs(Proximity::Near, near);
        } else {
            for r in g.rooms.iter_mut() {
                if !in_tree.contains(&r.id) {
                    r.alive = false;
                }
            }
            g.log.push(format!("{} room(s) left out: no valid corridor reaches them", g.rooms.len() - in_tree.len()));
            break;
        }
    }

    /* §7.5.2 Roles: the start, the stairs at a far leaf. */
    let h0 = room_hops(&g.nodes, &g.edges, g.node_of_room[start_room]);
    let leaves: Vec<usize> = g.rooms.iter().filter(|r| r.alive && r.id != start_room && g.degree(r.id) == 1).map(|r| r.id).collect();
    let pool: Vec<usize> = if leaves.is_empty() { g.rooms.iter().filter(|r| r.alive && r.id != start_room).map(|r| r.id).collect() } else { leaves };
    let mut stairs_room = pool[0];
    for &r in &pool {
        if h0[g.node_of_room[r]] > h0[g.node_of_room[stairs_room]] {
            stairs_room = r;
        }
    }
    if h0[g.node_of_room[stairs_room]] < p.min_hops as f64 {
        g.log.push(format!("stairs only {} hops from the start (wanted {})", h0[g.node_of_room[stairs_room]], p.min_hops));
    }
    g.rooms[start_room].role = Role::Start;
    g.rooms[stairs_room].role = Role::Stairs;
    g.stairs_room = stairs_room;

    /* §7.4 β_max: a maximal selection of the remaining candidates, greedy by weight, on a copy. */
    let all_pairs = g.proximity_pairs(order[level], near);
    let mut statics: Vec<Candidate> = Vec::new();
    for [a, b] in &all_pairs {
        if g.rooms[*a].alive && g.rooms[*b].alive {
            statics.extend(g.cand_of(*a, *b).iter().cloned());
        }
    }
    g.statics = statics;
    let alive_rooms = g.rooms.iter().filter(|r| r.alive).count() as i32;
    let beta_now = |g: &Gen| g.edges.len() as i32 - (alive_rooms + g.nodes.iter().filter(|n| matches!(n, Node::Junction { .. })).count() as i32) + 1;
    let beta_max;
    {
        let save_used: Vec<Vec<Port>> = g.rooms.iter().map(|r| r.used.clone()).collect();
        let save_edges = g.edges.len();
        let mut sorted: Vec<usize> = (0..g.statics.len()).collect();
        sorted.sort_by(|x, y| g.statics[*y].weight.partial_cmp(&g.statics[*x].weight).unwrap());
        for ci in sorted {
            let c = g.statics[ci].clone();
            if !g.locked_full(c.a) && !g.locked_full(c.b) && g.fits_now(&c) {
                g.apply(Action::Add(ci));
            }
        }
        let m_max = g.edges.len() as i32;
        g.edges.truncate(save_edges);
        for (r, u) in g.rooms.iter_mut().zip(save_used) {
            r.used = u;
        }
        beta_max = m_max - alive_rooms + 1;
    }
    let beta_target = (p.delta * beta_max as f64).round() as i32;

    /* §7.5.3 Cycles: corridors between rooms already joined, T branches off a corridor and X crossings, until β reaches β*. */
    while beta_now(&g) < beta_target {
        let mut opts = g.options();
        let ws: Vec<f64> = opts.iter().map(|o| o.weight).collect();
        match g.weighted(&ws) {
            Some(k) => {
                let act = opts.swap_remove(k).act;
                g.apply(act);
            }
            None => {
                let b = beta_now(&g);
                g.log.push(format!("β* = {} not reached: nothing more fits (β = {})", beta_target, b));
                break;
            }
        }
    }

    /* §7.6 Leaf chambers: small rooms in the dropped cells, each hung by one corridor off a room nearby. */
    let leaf_weights: Vec<f64> = CLASSES.iter().map(|c| p.leaf_classes.get(*c).max(0.0)).collect();
    let mut spots = dropped.clone();
    for i in (1..spots.len()).rev() {
        let j = (g.rng.next() * (i + 1) as f64).floor() as usize;
        spots.swap(i, j);
    }
    let mut made_leaves = 0;
    for bi in spots {
        if made_leaves >= p.leaves {
            break;
        }
        let bx = box_of(&blocks[bi]);
        let mut c = CLASSES[g.pick_class(&leaf_weights)];
        let mut shape = fit(c, bx, &mut g.rng, p, 1);
        if shape.is_none() {
            c = ShapeClass::Rect;
            shape = fit(ShapeClass::Rect, bx, &mut g.rng, p, 1);
        }
        let Some(shape) = shape else { continue };
        let sb = bounded(shape.clone());
        if g.rooms.iter().any(|o| o.alive && shape_dist(&o.shape, &shape) < 2.0 * gap) || g.edges.iter().any(|e| region_dist(std::slice::from_ref(&sb), &e.region) < p.clear) {
            continue;
        }
        let id = g.rooms.len();
        let room = Room { id, cls: c, bb: bounded(shape.clone()), center: centroid(&shape), area: area_of(&shape), rect: Rect { x0: bx.x0, y0: bx.y0, x1: bx.x1, y1: bx.y1 }, ports: ports_of(id, &shape, p), shape, used: Vec::new(), leaf: true, alive: true, role: Role::Empty };
        let center = room.center;
        g.rooms.push(room);
        g.nodes.push(Node::Room { room: id });
        g.node_of_room.push(g.nodes.len() - 1);
        let near_rooms: Vec<usize> = g.rooms.iter().filter(|o| o.alive && !o.leaf && !g.locked_full(o.id) && dist(o.center, center) <= 1.8 * cell_w.max(cell_h)).map(|o| o.id).collect();
        let mut options: Vec<Candidate> = Vec::new();
        for o in near_rooms {
            for k in g.candidates_for(id, o, 2) {
                if g.fits_now(&k) {
                    options.push(k);
                }
            }
        }
        options.sort_by(|x, y| x.cost.partial_cmp(&y.cost).unwrap());
        let Some(best) = options.into_iter().next() else {
            g.rooms.pop();
            g.nodes.pop();
            g.node_of_room.pop();
            continue;
        };
        g.add_edge(g.node_of_room[best.a], g.node_of_room[best.b], best.pts, [Some(best.ports[0]), Some(best.ports[1])]);
        let rw = p.leaf_roles;
        let k = g.pick_class(&[rw.trap, rw.prize, rw.empty]);
        g.rooms[id].role = [Role::Trap, Role::Prize, Role::Empty][k];
        made_leaves += 1;
    }
    if made_leaves < p.leaves {
        g.log.push(format!("{} of {} leaf chambers fit", made_leaves, p.leaves));
    }

    /* §7.7 (part) Ambush rooms: the biggest plain rooms. */
    let mut plain: Vec<usize> = g.rooms.iter().filter(|r| r.alive && !r.leaf && r.role == Role::Plain).map(|r| r.id).collect();
    plain.sort_by(|a, b| g.rooms[*b].area.partial_cmp(&g.rooms[*a].area).unwrap());
    for id in plain.into_iter().take(p.ambush.max(0) as usize) {
        g.rooms[id].role = Role::Ambush;
    }

    let dropped_centers: Vec<V> = dropped.iter().map(|i| { let b = box_of(&blocks[*i]); [(b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0] }).collect();
    Dungeon { params: p.clone(), seed, rooms: g.rooms, nodes: g.nodes, edges: g.edges, start: start_room, stairs: stairs_room, beta_max, beta_target, candidates: g.statics, pairs: all_pairs, dropped: dropped_centers, log: g.log }
}
