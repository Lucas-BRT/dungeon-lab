//! The dungeon model's generator (`docs/dungeon-model.md` §7): valid by construction. Every step only makes choices that keep the
//! constraints already fixed, so nothing is drawn and then thrown away whole. Deterministic: the same seed and Λ give the same stage.

use crate::geom::*;
use crate::par::{pflat_min, pmap_min};
use crate::params::*;
use crate::rng::{Const, Mulberry, Rand};
use crate::spatial::SpatialHash;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::f64::consts::PI;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "diagnostic-profile")]
use std::time::{Duration, Instant};

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
    Boss,
    Rest,
    Key,
    Switch,
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
    pub bounds: Bounds,
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
    /// Monotone inventory bits granted when this room is entered.
    pub keys: u8,
    /// Switch bits that can be activated while the player is in this room.
    pub switches: u8,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Node {
    Room { room: usize },
    Junction { p: Box<V> },
}

/// A branch of a corridor between two nodes of H (§5.1): a room or a junction at each end.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub id: usize,
    pub u: usize,
    pub v: usize,
    pub pts: Vec<V>,
    pub ends: [Option<Port>; 2],
    /// Collision geometry is kept in Rust for generation/validation but is not
    /// needed by the viewer's map representation.
    #[serde(skip)]
    pub region: Vec<Bounded>,
    /// Required key and switch masks for traversing this corridor branch.
    pub requires_keys: u8,
    pub requires_switches: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum RouteType {
    I,
    L,
    Z,
    W,
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

/// The compact candidate geometry sent to the viewer. Search-only ports,
/// weights and collision regions are released after generation.
#[derive(Clone, Debug, Serialize)]
pub struct CandidateView {
    pub a: usize,
    pub b: usize,
    pub pts: Vec<V>,
    #[serde(rename = "type")]
    pub kind: RouteType,
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
    /// Node IDs on the designated start-to-stairs path in H, before leaf rooms are added.
    pub main_path: Vec<usize>,
    /// Grid line coordinates; interior lines may be jittered while the outer bounds stay fixed.
    pub grid_x: Vec<f64>,
    pub grid_y: Vec<f64>,
    pub beta_max: i32,
    pub beta_target: i32,
    pub candidates: Vec<CandidateView>,
    pub pairs: Vec<[usize; 2]>,
    pub dropped: Vec<V>,
    pub log: Vec<String>,
    /// Milliseconds each phase of the generator took, in order (rooms, candidates, tree, β_max, cycles, leaf chambers).
    pub phases: Vec<(String, f64)>,
}

/// A lightweight event stream for drawing a dungeon while it is being built.
#[derive(Clone, Debug)]
pub enum GenerationEvent {
    AttemptStarted {
        draw: u32,
        seed: u32,
    },
    Layout {
        grid_x: Vec<f64>,
        grid_y: Vec<f64>,
        corridor_width: f64,
    },
    RoomAdded(Shape),
    EdgeUpdated {
        id: usize,
        pts: Vec<V>,
    },
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

fn rotate_poly_in_box(shape: Shape, b: Bx, rng: &mut dyn Rand, m: i32) -> Shape {
    let Shape::Poly { pts } = shape else {
        return shape;
    };
    let steps = if m == 8 { 8 } else { 4 };
    let first = (rng.next() * steps as f64).floor() as i32;
    let center = centroid(&Shape::Poly { pts: pts.clone() });
    for offset in 0..steps {
        let k = (first + offset) % steps;
        let a = 2.0 * PI * k as f64 / steps as f64;
        let (sin, cos) = a.sin_cos();
        let rotated: Vec<V> = pts
            .iter()
            .map(|q| {
                let d = sub(*q, center);
                [
                    center[0] + cos * d[0] - sin * d[1],
                    center[1] + sin * d[0] + cos * d[1],
                ]
            })
            .collect();
        let mut lo = [f64::INFINITY; 2];
        let mut hi = [f64::NEG_INFINITY; 2];
        for q in &rotated {
            lo = [lo[0].min(q[0]), lo[1].min(q[1])];
            hi = [hi[0].max(q[0]), hi[1].max(q[1])];
        }
        let span = [hi[0] - lo[0], hi[1] - lo[1]];
        let w = b.x1 - b.x0;
        let h = b.y1 - b.y0;
        if span[0] <= w && span[1] <= h {
            let dx = b.x0 - lo[0] + rng.next() * (w - span[0]);
            let dy = b.y0 - lo[1] + rng.next() * (h - span[1]);
            return poly(
                rotated
                    .into_iter()
                    .map(|q| [q[0] + dx, q[1] + dy])
                    .collect(),
            );
        }
    }
    poly(pts)
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
    let shape = match cls {
        ShapeClass::Rect => {
            let w = rng.range(
                if cells > 1 {
                    w_ * 0.7
                } else {
                    w_.min(min_side.max(w_ * 0.55))
                },
                w_,
            );
            let h = rng.range(
                if cells > 1 {
                    h_ * 0.7
                } else {
                    h_.min(min_side.max(h_ * 0.55))
                },
                h_,
            );
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
            Some(poly(
                local
                    .iter()
                    .map(|&[px, py]| {
                        [
                            x + if fx { w - px } else { px },
                            y + if fy { h - py } else { py },
                        ]
                    })
                    .collect(),
            ))
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
            // The two shoulders at each tip extend tangentially, so the star's
            // axis-aligned bounds can be slightly wider than `rho`. Build it
            // around the origin first, then place by its actual bounds.
            let mut local: Vec<V> = Vec::new();
            for k in 0..n {
                let psi = 2.0 * PI * k as f64 / n as f64;
                let tip = mul([psi.cos(), psi.sin()], rho);
                let side = mul([-psi.sin(), psi.cos()], tau / 2.0);
                let mid = psi + PI / n as f64;
                local.push(sub(tip, side));
                local.push(add(tip, side));
                local.push(mul([mid.cos(), mid.sin()], inner));
            }
            let mut lo = [f64::INFINITY; 2];
            let mut hi = [f64::NEG_INFINITY; 2];
            for q in &local {
                lo = [lo[0].min(q[0]), lo[1].min(q[1])];
                hi = [hi[0].max(q[0]), hi[1].max(q[1])];
            }
            let span = [hi[0] - lo[0], hi[1] - lo[1]];
            if span[0] > w_ || span[1] > h_ {
                return None;
            }
            let cx = b.x0 - lo[0] + rng.next() * (w_ - span[0]);
            let cy = b.y0 - lo[1] + rng.next() * (h_ - span[1]);
            let pts = local.iter().map(|q| [cx + q[0], cy + q[1]]).collect();
            Some(poly(pts))
        }
    }?;
    Some(if p.rotate_rooms {
        rotate_poly_in_box(shape, b, rng, p.m)
    } else {
        shape
    })
}

/* ---------------- Doors ---------------- */

/// The exit band F(p, θ) of §3.2, starting `from` px out of the wall.
fn exit_band(p: V, k: i32, pr: &Params, from: f64) -> Shape {
    band(
        add(p, mul(dir_of(k), from)),
        add(p, mul(dir_of(k), pr.exit)),
        pr.width,
    )
}

/// Π(S) sampled (§3.1): the doors of every edge whose outward normal is in the alphabet, `portStep` apart inside [ι, |e| − ι], whose exit band clears the room.
fn ports_of(room: usize, s: &Shape, pr: &Params) -> Vec<Port> {
    let mut out = Vec::new();
    let iota = pr.width / 2.0 + pr.clear;
    let allowed = |k: i32| pr.m == 8 || k % 2 == 0;
    match s {
        Shape::Disc { c, r } => {
            // A flat corridor end on a round wall: the corridor starts this far inside so no wedge of rock is left at its sides.
            let embed = r
                - (r * r - (pr.width / 2.0) * (pr.width / 2.0))
                    .max(0.0)
                    .sqrt()
                + 1.0;
            // q directions are equally spaced on the circle; keep only those
            // that lie in Θ₈. q=16 consequently contributes the eight 45° doors.
            let q = pr.disc_ports.clamp(1, 16);
            for i in 0..q {
                let numerator = i * 8;
                if numerator % q != 0 {
                    continue;
                }
                let k = numerator / q;
                if allowed(k) {
                    out.push(Port {
                        room,
                        p: add(*c, mul(dir_of(k), *r)),
                        k,
                        edge: -1,
                        embed,
                    });
                }
            }
        }
        Shape::Poly { pts } => {
            // A zero/negative sampling step makes the polygon-edge loop
            // nonterminating. Invalid Λ yields no sampled polygon doors.
            if !pr.port_step.is_finite() || pr.port_step <= 0.0 {
                return out;
            }
            let a2 = signed_area2(pts);
            let n = pts.len();
            for e in 0..n {
                let a = pts[e];
                let b = pts[(e + 1) % n];
                let l = dist(a, b);
                let d = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
                let nrm = if a2 > 0.0 {
                    [d[1], -d[0]]
                } else {
                    [-d[1], d[0]]
                };
                let mut k = -1;
                for i in 0..8 {
                    if (nrm[0] - dir_of(i)[0]).abs() < 1e-4 && (nrm[1] - dir_of(i)[1]).abs() < 1e-4
                    {
                        k = i;
                    }
                }
                if k < 0 || !allowed(k) || l < 2.0 * iota {
                    continue;
                }
                let mut t = iota;
                while t <= l - iota + 1e-9 {
                    let p = add(a, mul(d, t));
                    if !shape_within(&exit_band(p, k, pr, 0.5), s, TOUCH) {
                        out.push(Port {
                            room,
                            p,
                            k,
                            edge: e as i32,
                            embed: 0.0,
                        });
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
    if shape_within(&exit_band(q, port.k, pr, 0.5), s, TOUCH) {
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
fn emit_route(
    pts: Vec<V>,
    kind: RouteType,
    pr: &Params,
    visit: &mut impl FnMut(Vec<V>, RouteType),
) {
    let bends = pts.len().saturating_sub(2) as i32;
    if bends <= pr.max_bends {
        visit(pts.clone(), kind);
    }
    // A rectangular dogleg adds four legal 90° bends to a long straight
    // section. Exact corridor validation later rejects detours that hit rooms
    // or other corridors, so this expands the search without weakening rules.
    if kind == RouteType::W || pr.p_w <= 0.0 || bends + 4 > pr.max_bends {
        return;
    }
    let height = pr.seg.max(pr.width + pr.clear);
    for i in 0..pts.len().saturating_sub(1) {
        let (a, b) = (pts[i], pts[i + 1]);
        let length = dist(a, b);
        if length < 3.0 * pr.seg {
            continue;
        }
        let d = mul(sub(b, a), 1.0 / length);
        let normal = [-d[1], d[0]];
        for side in [-1.0, 1.0] {
            let n = mul(normal, side);
            let q1 = add(a, mul(d, pr.seg));
            let q2 = add(q1, mul(n, height));
            let q4 = sub(b, mul(d, pr.seg));
            let q3 = add(q4, mul(n, height));
            let mut detour = Vec::with_capacity(pts.len() + 4);
            detour.extend_from_slice(&pts[..=i]);
            detour.extend([q1, q2, q3, q4]);
            detour.extend_from_slice(&pts[i + 1..]);
            visit(detour, RouteType::W);
        }
    }
}

fn for_each_route(pa: &Port, pb: &Port, pr: &Params, mut visit: impl FnMut(Vec<V>, RouteType)) {
    let ua = dir_of(pa.k);
    let in_k = (pb.k + 4) % 8;
    let uc = dir_of(in_k);
    let d = sub(pb.p, pa.p);
    // I: facing each other and on one line.
    if in_k == pa.k && cross(d, ua).abs() < 1e-6 * len(d) && dot(d, ua) >= 2.0 * pr.exit {
        emit_route(vec![pa.p, pb.p], RouteType::I, pr, &mut visit);
    }
    if pr.max_bends < 1 {
        return;
    }
    // L: the two rays meet ahead of both doors.
    let ub = dir_of(pb.k);
    let den = cross(ua, ub);
    if den.abs() > 1e-9 && turn_ok(pa.k, in_k, pr) {
        let t = cross(d, ub) / den;
        let s = cross(d, ua) / den;
        if t >= pr.exit && s >= pr.exit {
            emit_route(
                vec![pa.p, add(pa.p, mul(ua, t)), pb.p],
                RouteType::L,
                pr,
                &mut visit,
            );
        }
    }
    if pr.max_bends < 2 {
        return;
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
        let mut route = None;
        let det = cross(sum, um);
        if det.abs() > 1e-9 {
            let t = cross(d, um) / det;
            let tm = cross(sum, d) / det;
            if !(t < pr.exit || tm < pr.seg) {
                let q1 = add(pa.p, mul(ua, t));
                let q2 = add(q1, mul(um, tm));
                route = Some(vec![pa.p, q1, q2, pb.p]);
            }
        }
        if route.is_none() {
            let r = sub(d, mul(ua, pr.exit));
            let det2 = cross(um, uc);
            if det2.abs() > 1e-9 {
                let tm = cross(r, uc) / det2;
                let tb = cross(um, r) / det2;
                if !(tb < pr.exit || tm < pr.seg) {
                    let q1 = add(pa.p, mul(ua, pr.exit));
                    let q2 = add(q1, mul(um, tm));
                    route = Some(vec![pa.p, q1, q2, pb.p]);
                }
            }
        }
        if let Some(pts) = route {
            emit_route(pts, RouteType::Z, pr, &mut visit);
        }
    }
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
    corridor_region(
        &trim_ends(
            pts,
            if ends[0].is_some() { 1.0 } else { 0.0 },
            if ends[1].is_some() { 1.0 } else { 0.0 },
        ),
        pr.width,
    )
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

#[derive(Clone, Copy)]
enum Action {
    Add(usize),
    Branch {
        room: usize,
        port_i: usize,
        edge: usize,
        seg: usize,
        h: V,
    },
    Cross {
        cand: usize,
        edge: usize,
        i: usize,
        seg: usize,
        h: V,
    },
}

/// A corridor segment with what the search for T branches needs of it.
struct SegRef {
    edge: usize,
    seg: usize,
    a: V,
    ds: V,
    sl: f64,
}

type Axes = [Vec<SegRef>; 4];

#[derive(Clone, Copy)]
struct Opt {
    weight: f64,
    act: Action,
}

struct Changes {
    edges: Vec<usize>,
    rooms: Vec<usize>,
}

fn box_gap(a: &Bounded, b: &Bounded) -> f64 {
    let dx = (a.lo[0] - b.hi[0]).max(b.lo[0] - a.hi[0]).max(0.0);
    let dy = (a.lo[1] - b.hi[1]).max(b.lo[1] - a.hi[1]).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

fn box_gap_raw(a_lo: V, a_hi: V, b_lo: V, b_hi: V) -> f64 {
    let dx = (a_lo[0] - b_hi[0]).max(b_lo[0] - a_hi[0]).max(0.0);
    let dy = (a_lo[1] - b_hi[1]).max(b_lo[1] - a_hi[1]).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

/// A cheap conservative test: false guarantees the two regions cannot be within `limit`.
fn regions_may_be_within(a: &[Bounded], b: &[Bounded], limit: f64) -> bool {
    a.iter().any(|x| b.iter().any(|y| box_gap(x, y) <= limit))
}

fn mix_of(p: &Params, rooms: &[Room], a: usize, b: usize) -> f64 {
    *p.mix
        .get(&mix_key(rooms[a].cls, rooms[b].cls))
        .unwrap_or(&1.0)
}

fn type_weight(p: &Params, t: RouteType) -> f64 {
    match t {
        RouteType::I => p.p_i,
        RouteType::L => p.p_l,
        RouteType::Z => p.p_z,
        RouteType::W => p.p_w,
    }
}

/// The corridors that may join rooms a and b (§7.3 steps 2 to 5), the best `keep` by cost, each with distinct doors.
pub fn candidates_for(
    pr: &Params,
    rooms: &[Room],
    a: usize,
    b: usize,
    keep: usize,
) -> Vec<Candidate> {
    candidates_for_with_index(pr, rooms, a, b, keep, None)
}

fn candidates_for_indexed(
    pr: &Params,
    rooms: &[Room],
    a: usize,
    b: usize,
    keep: usize,
    room_index: &SpatialHash,
) -> Vec<Candidate> {
    candidates_for_with_index(pr, rooms, a, b, keep, Some(room_index))
}

fn candidates_for_with_index(
    pr: &Params,
    rooms: &[Room],
    a: usize,
    b: usize,
    keep: usize,
    room_index: Option<&SpatialHash>,
) -> Vec<Candidate> {
    let (ra, rb) = (&rooms[a], &rooms[b]);
    if mix_of(pr, rooms, a, b) <= 0.0 || shape_within(&ra.shape, &rb.shape, pr.min_len) {
        return Vec::new();
    }
    let facing = |r: &Room, other: &Room| -> Vec<Port> {
        let to = sub(other.center, r.center);
        let ps: Vec<Port> = r
            .ports
            .iter()
            .filter(|p| dot(dir_of(p.k), to) > 0.0)
            .copied()
            .collect();
        let stride = ((ps.len() as f64 / 14.0).ceil() as usize).max(1);
        ps.into_iter()
            .enumerate()
            .filter(|(i, _)| i % stride == 0)
            .map(|(_, p)| p)
            .collect()
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
    let others: Vec<&Room> = if room_index.is_none() {
        rooms.iter().filter(|o| o.id != a && o.id != b).collect()
    } else {
        Vec::new()
    };
    for (pa, pb) in tries {
        if dot(dir_of(pa.k), sub(pb.p, pa.p)) <= 0.0 || dot(dir_of(pb.k), sub(pa.p, pb.p)) <= 0.0 {
            continue;
        }
        for_each_route(&pa, &pb, pr, |pts, kind| {
            if type_weight(pr, kind) <= 0.0 {
                return;
            }
            let segment_count = pts.len() - 1;
            if pts.windows(2).enumerate().any(|(i, w)| {
                let minimum = if i == 0 || i + 1 == segment_count {
                    pr.exit
                } else {
                    pr.seg
                };
                dist(w[0], w[1]) < minimum
            }) {
                return;
            }
            let length = polyline_length(&pts);
            if length < pr.min_len {
                return;
            }
            let ends = [Some(pa), Some(pb)];
            let region = region_of(&pts, &ends, pr);
            let outside = outside_region(&pts, &ends, pr);
            let mut ok = !region_shape_within(&outside, &ra.shape, ra.bounds, TOUCH)
                && !region_shape_within(&outside, &rb.shape, rb.bounds, TOUCH);
            if ok {
                ok = if let Some(index) = room_index {
                    !index.query_region(&region, pr.clear).into_iter().any(|id| {
                        id != a
                            && id != b
                            && region_shape_within(
                                &region,
                                &rooms[id].shape,
                                rooms[id].bounds,
                                pr.clear,
                            )
                    })
                } else {
                    !others.iter().any(|other| {
                        region_shape_within(&region, &other.shape, other.bounds, pr.clear)
                    })
                };
            }
            if !ok {
                return;
            }
            let bends = (pts.len() - 2) as f64;
            let mut forty_five = 0;
            let mut previous = None;
            for w in pts.windows(2) {
                let direction = seg_dir(w[0], w[1]);
                if previous.is_some_and(|old| turn_of(old, direction) == 1) {
                    forty_five += 1;
                }
                previous = Some(direction);
            }
            let cost = pr.cost_bend * bends + pr.cost_len * length + pr.cost45 * forty_five as f64
                - if kind == RouteType::I {
                    pr.cost_facing
                } else {
                    0.0
                };
            out.push(Candidate {
                a,
                b,
                ports: [pa, pb],
                pts,
                kind,
                cost,
                weight: mix_of(pr, rooms, a, b) * type_weight(pr, kind) * (-cost).exp(),
                region,
            });
        });
    }
    out.sort_by(|x, y| x.cost.partial_cmp(&y.cost).unwrap());
    let mut kept: Vec<Candidate> = Vec::new();
    for c in out {
        if kept.len() >= keep {
            break;
        }
        if kept.iter().any(|k| {
            dist(k.ports[0].p, c.ports[0].p) < pr.width
                || dist(k.ports[1].p, c.ports[1].p) < pr.width
        }) {
            continue;
        }
        kept.push(c);
    }
    kept
}

#[cfg(feature = "diagnostic-profile")]
#[derive(Default)]
struct TreeDiagnostics {
    iterations: usize,
    frontier_items_seen: usize,
    max_frontier: usize,
    fit_checks: usize,
    open_candidates_seen: usize,
    frontier_scan: Duration,
    fit_time: Duration,
    weighted_pick: Duration,
    edge_update: Duration,
    frontier_ensure: Duration,
    graph_rebuild: Duration,
    fallbacks: Vec<String>,
}

struct Gen<'a> {
    p: &'a Params,
    rng: Mulberry,
    log: Vec<String>,
    rooms: Vec<Room>,
    nodes: Vec<Node>,
    node_of_room: Vec<usize>,
    edges: Vec<Edge>,
    cache: HashMap<(usize, usize), Vec<Candidate>>,
    statics: Vec<Candidate>,
    /// Currently valid direct-link or X-crossing option for each static candidate.
    static_opts: Vec<Option<Opt>>,
    /// Currently valid T branches, ordered like the full room/port/edge/segment scan.
    branch_opts: BTreeMap<(usize, usize, usize, usize), Opt>,
    /// Static candidates incident to each room, for localized port invalidation.
    candidates_by_room: Vec<Vec<usize>>,
    /// `statics[i]` is known not to fit: edges and doors in use only grow while the cycles are added, so a candidate that does not fit never will.
    no_fit: Vec<AtomicBool>,
    /// `statics[i]` can never become a crossing: its doors are taken or it already touches two corridors (both only grow).
    no_cross: Vec<AtomicBool>,
    stairs_room: usize,
    /// Broad phase over the regions of `statics`, to find the candidates near a corridor that changed.
    static_index: SpatialHash,
    /// Broad phase over the rooms and, kept up to date by `add_edge`/`split_edge`, over the corridor regions (a superset after a rollback: callers re-test exactly).
    room_grid: SpatialHash,
    edge_index: SpatialHash,
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
        mix_of(self.p, &self.rooms, a, b)
    }

    fn t_weight(&self) -> f64 {
        let baseline = if self.p.topology.requirements().min_hub_degree > 0 {
            0.15
        } else {
            0.0
        };
        self.p.p_t.max(baseline)
    }

    fn x_weight(&self) -> f64 {
        let baseline = if self.p.topology.requirements().min_hub_degree >= 4 {
            0.12
        } else {
            0.0
        };
        self.p.p_x.max(baseline)
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
                        ok = if kind == Proximity::Rng {
                            !(d2(ci, ck).max(d2(cj, ck)) < dij)
                        } else {
                            !(d2(ck, mul(add(ci, cj), 0.5)) < dij / 4.0)
                        };
                    }
                }
                if ok {
                    out.push([i, j]);
                }
            }
        }
        out
    }

    /// Candidates for every pair in `pairs` not seen yet, worked out together: each depends only on the rooms, so they split over the threads.
    fn ensure(&mut self, pairs: &[[usize; 2]], room_index: &SpatialHash) {
        let mut missing: Vec<[usize; 2]> = pairs
            .iter()
            .copied()
            .filter(|[a, b]| !self.cache.contains_key(&(*a, *b)))
            .collect();
        missing.sort();
        missing.dedup();
        let (p, rooms) = (self.p, &self.rooms);
        let found = pmap_min(&missing, 8, |[a, b]| {
            candidates_for_indexed(p, rooms, *a, *b, 4, room_index)
        });
        for ([a, b], c) in missing.into_iter().zip(found) {
            self.cache.insert((a, b), c);
        }
    }

    fn add_edge(&mut self, u: usize, v: usize, pts: Vec<V>, ends: [Option<Port>; 2]) -> usize {
        let region = region_of(&pts, &ends, self.p);
        let id = self.edges.len();
        for p in ends.iter().flatten() {
            self.rooms[p.room].used.push(*p);
        }
        self.edges.push(Edge {
            id,
            u,
            v,
            pts,
            ends,
            region,
            requires_keys: 0,
            requires_switches: 0,
        });
        self.edge_index.insert_region(id, &self.edges[id].region);
        id
    }

    /// Whether door `p` keeps its exit band σ away from every door already used in its room (V3).
    fn port_free(&self, p: &Port) -> bool {
        let pr = self.p;
        let mine = exit_band(p.p, p.k, pr, 0.0);
        self.rooms[p.room]
            .used
            .iter()
            .all(|q| !shape_within(&mine, &exit_band(q.p, q.k, pr, 0.0), pr.clear))
    }

    /// Whether a candidate fits what is already built (§7.4): free doors, and σ from every corridor.
    fn fits_now(&self, c: &Candidate) -> bool {
        if !self.port_free(&c.ports[0]) || !self.port_free(&c.ports[1]) {
            return false;
        }
        self.edges
            .iter()
            .all(|e| !region_within(&c.region, &e.region, self.p.clear))
    }

    /// `fits_now` of `statics[ci]`, remembering a no for good.
    fn fits_cached(&self, ci: usize) -> bool {
        if self.no_fit[ci].load(Ordering::Relaxed) {
            return false;
        }
        let ok = self.fits_now(&self.statics[ci]);
        if !ok {
            self.no_fit[ci].store(true, Ordering::Relaxed);
        }
        ok
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
        self.nodes.push(Node::Junction { p: Box::new(h) });
        let old_v = self.edges[e].v;
        let old_end = self.edges[e].ends[1];
        let requires_keys = self.edges[e].requires_keys;
        let requires_switches = self.edges[e].requires_switches;
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
        self.edges.push(Edge {
            id,
            u: j,
            v: old_v,
            pts: second,
            ends: [None, old_end],
            region: tail_region,
            requires_keys,
            requires_switches,
        });
        self.edges[e].requires_keys = requires_keys;
        self.edges[e].requires_switches = requires_switches;
        self.edge_index.insert_region(e, &self.edges[e].region);
        self.edge_index.insert_region(id, &self.edges[id].region);
        j
    }

    /// Every corridor segment filed by axis (its direction mod 180°, in steps of 45°), each list in the order of the edges.
    fn axes(&self) -> Axes {
        let mut out: Axes = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        for e in &self.edges {
            for s in 0..e.pts.len() - 1 {
                let (a, b) = (e.pts[s], e.pts[s + 1]);
                let k = seg_dir(a, b);
                if k < 0 {
                    continue;
                }
                let sl = dist(a, b);
                out[(k % 4) as usize].push(SegRef {
                    edge: e.id,
                    seg: s,
                    a,
                    ds: [(b[0] - a[0]) / sl, (b[1] - a[1]) / sl],
                    sl,
                });
            }
        }
        out
    }

    /// The indexed segments of only the edges changed by the last action.
    fn axes_for_edges(&self, edge_ids: &[usize]) -> Axes {
        let mut out: Axes = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let mut ids = edge_ids.to_vec();
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            let e = &self.edges[id];
            for s in 0..e.pts.len() - 1 {
                let (a, b) = (e.pts[s], e.pts[s + 1]);
                let k = seg_dir(a, b);
                if k < 0 {
                    continue;
                }
                let sl = dist(a, b);
                out[(k % 4) as usize].push(SegRef {
                    edge: e.id,
                    seg: s,
                    a,
                    ds: [(b[0] - a[0]) / sl, (b[1] - a[1]) / sl],
                    sl,
                });
            }
        }
        out
    }

    fn room_of_node(&self, n: usize) -> Option<usize> {
        match self.nodes[n] {
            Node::Room { room } => Some(room),
            Node::Junction { .. } => None,
        }
    }

    /// T (§4.3): a straight arm from a free door of a room to a perpendicular corridor segment, met away from its ends.
    fn branches_of(&self, r: &Room, axes: &Axes, edge_filter: Option<&HashSet<usize>>) -> Vec<Opt> {
        let mut out = Vec::new();
        let pr = self.p;
        // Two junctions on one corridor with arms on opposite sides keep σ between the arms only if they are w + σ apart.
        let margin = (pr.width / 2.0 + pr.seg).max(pr.width + pr.clear);
        {
            if !r.alive || self.locked_full(r.id) {
                return out;
            }
            // The doors in use only change when an edge is added, so the free ones are worked out once for the room, not once per segment.
            let mut free: Vec<bool> = vec![false; r.ports.len()];
            let mut free_known: Vec<bool> = vec![false; r.ports.len()];
            for (pi, p) in r.ports.iter().enumerate() {
                let u = dir_of(p.k);
                // Only a segment perpendicular to the door can take an arm: those of the axis a quarter turn from its direction, in the order the edges list them.
                for sr in &axes[((p.k + 2) % 4) as usize] {
                    if edge_filter.is_some_and(|ids| !ids.contains(&sr.edge)) {
                        continue;
                    }
                    let e = &self.edges[sr.edge];
                    let rooms_of_e: Vec<usize> = [e.u, e.v]
                        .iter()
                        .filter_map(|n| self.room_of_node(*n))
                        .collect();
                    if rooms_of_e.contains(&r.id) {
                        continue;
                    }
                    {
                        let (s, a, sl, ds) = (sr.seg, sr.a, sr.sl, sr.ds);
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
                        if !free_known[pi] {
                            free_known[pi] = true;
                            free[pi] = self.port_free(p);
                        }
                        if !free[pi] {
                            continue;
                        }
                        let arm = [p.p, h];
                        let arm_out = corridor_region(
                            &trim_ends(&arm, 1.0, pr.width / 2.0 + pr.clear),
                            pr.width,
                        );
                        if region_shape_within(&arm_out, &r.shape, r.bounds, TOUCH) {
                            continue;
                        }
                        // What the corridor will occupy, door embedding included: the validator measures the stored region, not the part outside the room.
                        let mut embedded = arm.to_vec();
                        if p.embed > 0.0 {
                            let l = dist(arm[0], arm[1]);
                            embedded[0] = add(arm[0], mul(sub(arm[0], arm[1]), p.embed / l));
                        }
                        let arm_full = corridor_region(
                            &trim_ends(&embedded, 0.0, pr.width / 2.0 + pr.clear),
                            pr.width,
                        );
                        let mut ok = true;
                        for oi in self.room_grid.query_region(&arm_out, pr.clear) {
                            let o = &self.rooms[oi];
                            if o.alive
                                && o.id != r.id
                                && region_shape_within(&arm_out, &o.shape, o.bounds, pr.clear)
                            {
                                ok = false;
                                break;
                            }
                        }
                        if ok {
                            ok = self
                                .edge_index
                                .query_region(&arm_full, pr.clear)
                                .into_iter()
                                .all(|oi| {
                                    oi >= self.edges.len()
                                        || !region_within(
                                            &arm_full,
                                            &self.edges[oi].region,
                                            pr.clear,
                                        )
                                });
                        }
                        if !ok {
                            continue;
                        }
                        // §4.4 between the new room and each room the corridor already reaches: straight distance and walk.
                        let to_u = polyline_length(&[&e.pts[..=s], &[h][..]].concat());
                        let to_v = polyline_length(&[&[h][..], &e.pts[s + 1..]].concat());
                        let reach: Vec<(usize, f64)> = [(e.u, t + to_u), (e.v, t + to_v)]
                            .into_iter()
                            .filter_map(|(n, w)| self.room_of_node(n).map(|room| (room, w)))
                            .collect();
                        if reach.iter().any(|(room, walk)| {
                            *walk < pr.min_len
                                || shape_within(&r.shape, &self.rooms[*room].shape, pr.min_len)
                        }) {
                            continue;
                        }
                        // Splitting the corridor hands each piece the validator's σ rule against the rooms it no longer touches: check both pieces as stored.
                        let first_pts = [&e.pts[..=s], &[h][..]].concat();
                        let second_pts = [&[h][..], &e.pts[s + 1..]].concat();
                        let mut first_region = region_of(&first_pts, &[e.ends[0], None], pr);
                        first_region.push(bounded(joint(h, pr.width)));
                        let tail_region = region_of(&second_pts, &[None, e.ends[1]], pr);
                        let too_close = |region: &[Bounded], own: Option<Port>| {
                            self.room_grid
                                .query_region(region, pr.clear)
                                .into_iter()
                                .any(|ri| {
                                    let o = &self.rooms[ri];
                                    o.alive
                                        && own.map_or(true, |q| q.room != ri)
                                        && region_shape_dist(region, &o.shape, o.bounds)
                                            < pr.clear - 1e-6
                                })
                        };
                        if too_close(&first_region, e.ends[0]) || too_close(&tail_region, e.ends[1])
                        {
                            continue;
                        }
                        let mix_w: f64 = reach
                            .iter()
                            .map(|(room, _)| self.mix_of(r.id, *room))
                            .product();
                        let cost = pr.cost_len * t + pr.cost_bend;
                        out.push(Opt {
                            weight: self.t_weight() * mix_w * (-cost).exp(),
                            act: Action::Branch {
                                room: r.id,
                                port_i: pi,
                                edge: e.id,
                                seg: s,
                                h,
                            },
                        });
                    }
                }
            }
        }
        out
    }

    /// X (§4.3): candidate `ci` crosses exactly one corridor, at right angles, away from both segments' ends: the crossing becomes a junction.
    fn crossing(&self, ci: usize) -> Option<Action> {
        let pr = self.p;
        let c = &self.statics[ci];
        let hit: Vec<&Edge> = self
            .edges
            .iter()
            .filter(|e| region_within(&c.region, &e.region, pr.clear))
            .collect();
        if hit.len() != 1 {
            if hit.len() > 1 {
                self.no_cross[ci].store(true, Ordering::Relaxed);
            }
            return None;
        }
        let e = hit[0];
        // Two junctions on one corridor with arms on opposite sides keep σ between the arms only if they are w + σ apart.
        let margin = (pr.width / 2.0 + pr.seg).max(pr.width + pr.clear);
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
                if along_c < margin
                    || len(da) - along_c < margin
                    || along_e < margin
                    || len(dp) - along_e < margin
                {
                    return None;
                }
                // The rest of both must keep σ apart: test them with the crossing cut out.
                let with = |head: &[V], h: V, tail: bool| -> Vec<V> {
                    if tail {
                        [&[h][..], head].concat()
                    } else {
                        [head, &[h][..]].concat()
                    }
                };
                let c_a = corridor_region(
                    &trim_ends(&with(&c.pts[..=i], h, false), 0.0, cut),
                    pr.width,
                );
                let c_b = corridor_region(
                    &trim_ends(&with(&c.pts[i + 1..], h, true), cut, 0.0),
                    pr.width,
                );
                let e_a = corridor_region(
                    &trim_ends(&with(&e.pts[..=s], h, false), 0.0, cut),
                    pr.width,
                );
                let e_b = corridor_region(
                    &trim_ends(&with(&e.pts[s + 1..], h, true), cut, 0.0),
                    pr.width,
                );
                for x in [&c_a, &c_b] {
                    for y in [&e_a, &e_b] {
                        if region_within(x, y, pr.clear) {
                            return None;
                        }
                    }
                }
                let rooms_of_e: Vec<usize> = [e.u, e.v]
                    .iter()
                    .filter_map(|n| self.room_of_node(*n))
                    .collect();
                if rooms_of_e.contains(&c.a) || rooms_of_e.contains(&c.b) {
                    return None;
                }
                for ra in [c.a, c.b] {
                    for rb in &rooms_of_e {
                        if shape_within(&self.rooms[ra].shape, &self.rooms[*rb].shape, pr.min_len) {
                            return None;
                        }
                    }
                }
                return Some(Action::Cross {
                    cand: ci,
                    edge: e.id,
                    i,
                    seg: s,
                    h,
                });
            }
        }
        None
    }

    fn static_option(&self, ci: usize) -> Option<Opt> {
        let c = &self.statics[ci];
        if self.locked_full(c.a) || self.locked_full(c.b) {
            return None;
        }
        if self.fits_cached(ci) {
            Some(Opt {
                weight: c.weight,
                act: Action::Add(ci),
            })
        } else if self.x_weight() > 0.0 && !self.no_cross[ci].load(Ordering::Relaxed) {
            if self.port_free(&c.ports[0]) && self.port_free(&c.ports[1]) {
                self.crossing(ci).map(|act| Opt {
                    weight: c.weight * self.x_weight(),
                    act,
                })
            } else {
                self.no_cross[ci].store(true, Ordering::Relaxed);
                None
            }
        } else {
            None
        }
    }

    fn branch_key(opt: &Opt) -> Option<(usize, usize, usize, usize)> {
        match opt.act {
            Action::Branch {
                room,
                port_i,
                edge,
                seg,
                ..
            } => Some((room, port_i, edge, seg)),
            _ => None,
        }
    }

    /// Construct the option cache for a new cycle state.
    fn rebuild_option_cache(&mut self) {
        let idx: Vec<usize> = (0..self.statics.len()).collect();
        self.static_opts = pmap_min(&idx, 128, |&ci| self.static_option(ci));
        self.candidates_by_room = vec![Vec::new(); self.rooms.len()];
        let cell = [self.p.cell_w, self.p.cell_h]
            .into_iter()
            .filter(|v| v.is_finite())
            .map(f64::abs)
            .fold(1.0, f64::max);
        self.static_index = SpatialHash::new(cell);
        for (ci, c) in self.statics.iter().enumerate() {
            self.candidates_by_room[c.a].push(ci);
            self.candidates_by_room[c.b].push(ci);
            self.static_index.insert_region(ci, &c.region);
        }
        self.branch_opts.clear();
        if self.t_weight() > 0.0 {
            let axes = self.axes();
            let per_room = pmap_min(&self.rooms, 16, |r| self.branches_of(r, &axes, None));
            for opts in per_room {
                for opt in opts {
                    if let Some(key) = Self::branch_key(&opt) {
                        self.branch_opts.insert(key, opt);
                    }
                }
            }
        }
    }

    fn branch_arm_may_be_blocked(&self, port: &Port, h: V, region: &[Bounded]) -> bool {
        let pad = self.p.width / 2.0;
        let lo = [port.p[0].min(h[0]) - pad, port.p[1].min(h[1]) - pad];
        let hi = [port.p[0].max(h[0]) + pad, port.p[1].max(h[1]) + pad];
        region
            .iter()
            .any(|b| box_gap_raw(lo, hi, b.lo, b.hi) <= self.p.clear)
    }

    /// Recheck a cached T option against its target edge and the current whole dungeon.
    fn branch_is_current(&self, opt: &Opt) -> bool {
        let Some((room, _, edge, _)) = Self::branch_key(opt) else {
            return false;
        };
        let axes = self.axes_for_edges(&[edge]);
        let current = self.branches_of(&self.rooms[room], &axes, None);
        let key = Self::branch_key(opt);
        current
            .iter()
            .any(|candidate| Self::branch_key(candidate) == key)
    }

    /// Update only options whose ports or nearby geometry could have changed.
    fn refresh_option_cache(&mut self, changes: &Changes) {
        let mut dirty = vec![false; self.statics.len()];
        for &room in &changes.rooms {
            for &ci in &self.candidates_by_room[room] {
                dirty[ci] = true;
            }
        }
        for &edge in &changes.edges {
            let region = &self.edges[edge].region;
            for ci in self.static_index.query_region(region, self.p.clear) {
                if !dirty[ci]
                    && regions_may_be_within(&self.statics[ci].region, region, self.p.clear)
                {
                    dirty[ci] = true;
                }
            }
        }
        let dirty_ids: Vec<usize> = dirty
            .iter()
            .enumerate()
            .filter_map(|(ci, yes)| yes.then_some(ci))
            .collect();
        let refreshed = pmap_min(&dirty_ids, 128, |&ci| (ci, self.static_option(ci)));
        for (ci, opt) in refreshed {
            self.static_opts[ci] = opt;
        }

        if self.t_weight() <= 0.0 {
            return;
        }
        let changed_edges: HashSet<usize> = changes.edges.iter().copied().collect();
        let changed_rooms: HashSet<usize> = changes.rooms.iter().copied().collect();
        let mut remove = Vec::new();
        for (&key, opt) in &self.branch_opts {
            let Action::Branch {
                room,
                port_i,
                edge,
                h,
                ..
            } = opt.act
            else {
                continue;
            };
            let port = self.rooms[room].ports[port_i];
            if changed_edges.contains(&edge) {
                remove.push(key);
                continue;
            }
            if changed_rooms.contains(&room) && (self.locked_full(room) || !self.port_free(&port)) {
                remove.push(key);
                continue;
            }
            let nearby = changes.edges.iter().any(|&changed| {
                self.branch_arm_may_be_blocked(&port, h, &self.edges[changed].region)
            });
            if nearby && !self.branch_is_current(opt) {
                remove.push(key);
            }
        }
        for key in remove {
            self.branch_opts.remove(&key);
        }
        let axes = self.axes_for_edges(&changes.edges);
        let segment_count: usize = axes.iter().map(Vec::len).sum();
        let estimated_work = self
            .rooms
            .len()
            .saturating_mul(segment_count)
            .saturating_mul(8);
        let new_branches = if estimated_work >= 4096 {
            pflat_min(&self.rooms, 64, |r| {
                self.branches_of(r, &axes, Some(&changed_edges))
            })
        } else {
            self.rooms
                .iter()
                .flat_map(|r| self.branches_of(r, &axes, Some(&changed_edges)))
                .collect()
        };
        for opts in new_branches {
            if let Some(key) = Self::branch_key(&opts) {
                self.branch_opts.insert(key, opts);
            }
        }
    }

    /// The cached list keeps the same static/room/port/edge/segment order as a full scan.
    fn options(&self) -> Vec<Opt> {
        let mut out: Vec<Opt> = self.static_opts.iter().filter_map(|opt| *opt).collect();
        out.extend(self.branch_opts.values().copied());
        out
    }

    fn apply(&mut self, act: Action) -> Changes {
        let (edges, rooms) = match act {
            Action::Add(ci) => {
                let c = self.statics[ci].clone();
                let id = self.add_edge(
                    self.node_of_room[c.a],
                    self.node_of_room[c.b],
                    c.pts,
                    [Some(c.ports[0]), Some(c.ports[1])],
                );
                (vec![id], vec![c.a, c.b])
            }
            Action::Branch {
                room,
                port_i,
                edge,
                seg,
                h,
            } => {
                let port = self.rooms[room].ports[port_i];
                let tail = self.edges.len();
                let j = self.split_edge(edge, seg, h);
                let arm = self.add_edge(
                    self.node_of_room[room],
                    j,
                    vec![port.p, h],
                    [Some(port), None],
                );
                (vec![edge, tail, arm], vec![room])
            }
            Action::Cross {
                cand,
                edge,
                i,
                seg,
                h,
            } => {
                let c = self.statics[cand].clone();
                let tail = self.edges.len();
                let j = self.split_edge(edge, seg, h);
                let first: Vec<V> = [&c.pts[..=i], &[h][..]].concat();
                let second: Vec<V> = [&[h][..], &c.pts[i + 1..]].concat();
                let left =
                    self.add_edge(self.node_of_room[c.a], j, first, [Some(c.ports[0]), None]);
                let right =
                    self.add_edge(j, self.node_of_room[c.b], second, [None, Some(c.ports[1])]);
                (vec![edge, tail, left, right], vec![c.a, c.b])
            }
        };
        Changes { edges, rooms }
    }
}

fn path_rooms(nodes: &[Node], path: &[usize]) -> Vec<usize> {
    path.iter()
        .filter_map(|&node| match nodes.get(node) {
            Some(Node::Room { room }) => Some(*room),
            _ => None,
        })
        .collect()
}

fn assign_pacing(g: &mut Gen<'_>, path: &[usize], ambushes: usize) {
    let rooms = path_rooms(&g.nodes, path);
    if rooms.len() < 3 {
        return;
    }
    let choose_at = |fraction: f64, rooms: &[usize], data: &[Room]| -> Option<usize> {
        let lo = 1;
        let hi = rooms.len() - 1;
        (lo..hi)
            .filter_map(|i| {
                let id = rooms[i];
                (data[id].role == Role::Plain)
                    .then_some((id, (i as f64 / (rooms.len() - 1) as f64 - fraction).abs()))
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(id, _)| id)
    };
    if let Some(rest) = choose_at(0.45, &rooms, &g.rooms) {
        g.rooms[rest].role = Role::Rest;
    }
    if let Some(boss) = choose_at(0.85, &rooms, &g.rooms) {
        g.rooms[boss].role = Role::Boss;
    }
    for i in 0..ambushes {
        let fraction = 0.2 + 0.55 * ((i + 1) as f64 / (ambushes + 1) as f64);
        if let Some(room) = choose_at(fraction, &rooms, &g.rooms) {
            g.rooms[room].role = Role::Ambush;
        }
    }
}

fn assign_progression(g: &mut Gen<'_>, path: &[usize], path_edges: &[usize]) {
    let requested_keys = g.p.progression.key_locks.min(8);
    let requested_switches = g.p.progression.switch_locks.min(8);
    if g.p.progression.key_locks > 8 || g.p.progression.switch_locks > 8 {
        g.log
            .push("progression: each lock type is limited to eight bits".to_string());
    }
    let requested = requested_keys + requested_switches;
    if requested == 0 || path_edges.is_empty() {
        return;
    }
    let count = requested.min(path_edges.len());
    if count < requested {
        g.log.push(format!(
            "progression: only {} of {} requested locks fit on the main path",
            count, requested
        ));
    }
    let key_count = requested_keys.min(count);

    // Assign every node to the closest node on the designated path. Each lock
    // then gates the complete cut between two path positions, so a loop cannot
    // bypass the corresponding key or switch.
    let mut path_position = vec![usize::MAX; g.nodes.len()];
    let mut queue = VecDeque::new();
    for (position, &node) in path.iter().enumerate() {
        path_position[node] = position;
        queue.push_back(node);
    }
    let mut adjacency = vec![Vec::new(); g.nodes.len()];
    for edge in &g.edges {
        adjacency[edge.u].push(edge.v);
        adjacency[edge.v].push(edge.u);
    }
    while let Some(node) = queue.pop_front() {
        for &next in &adjacency[node] {
            if path_position[next] == usize::MAX {
                path_position[next] = path_position[node];
                queue.push_back(next);
            }
        }
    }

    for index in 0..count {
        let edge_pos = (((index + 1) * path_edges.len()) / (count + 1)).min(path_edges.len() - 1);
        let bit = if index < key_count {
            1u8 << index
        } else {
            1u8 << (index - key_count)
        };
        let is_key = index < key_count;
        for edge in &mut g.edges {
            let (a, b) = (path_position[edge.u], path_position[edge.v]);
            if a.min(b) <= edge_pos && a.max(b) > edge_pos {
                if is_key {
                    edge.requires_keys |= bit;
                } else {
                    edge.requires_switches |= bit;
                }
            }
        }

        let candidate_rooms = path[..=edge_pos.min(path.len() - 1)]
            .iter()
            .rev()
            .filter_map(|node| match g.nodes[*node] {
                Node::Room { room }
                    if matches!(g.rooms[room].role, Role::Plain | Role::Key | Role::Switch) =>
                {
                    Some(room)
                }
                _ => None,
            });
        let room = candidate_rooms
            .into_iter()
            .find(|room| g.rooms[*room].role == Role::Plain)
            .or_else(|| {
                path[..=edge_pos.min(path.len() - 1)]
                    .iter()
                    .rev()
                    .find_map(|node| match g.nodes[*node] {
                        Node::Room { room }
                            if matches!(g.rooms[room].role, Role::Key | Role::Switch) =>
                        {
                            Some(room)
                        }
                        _ => None,
                    })
            })
            .or_else(|| path_rooms(&g.nodes, path).first().copied());
        let Some(room) = room else { continue };
        if is_key {
            g.rooms[room].keys |= bit;
            if !matches!(
                g.rooms[room].role,
                Role::Start | Role::Stairs | Role::Boss | Role::Rest | Role::Ambush
            ) {
                g.rooms[room].role = Role::Key;
            }
        } else {
            g.rooms[room].switches |= bit;
            if !matches!(
                g.rooms[room].role,
                Role::Start | Role::Stairs | Role::Boss | Role::Rest | Role::Ambush | Role::Key
            ) {
                g.rooms[room].role = Role::Switch;
            }
        }
    }
}

/// Prototype of a grown layout: from a first room, each room spawns 1..=`branch` children off its doors down to generation `depth`,
/// every child placed in free space and joined by a corridor on the spot, until `target` rooms stand. Replaces the grid rooms and the tree phase.
fn grow_rooms(
    g: &mut Gen<'_>,
    world: Bx,
    target: usize,
    progress: &mut Option<&mut dyn FnMut(GenerationEvent)>,
) -> Vec<usize> {
    let p = g.p;
    let (cw, ch) = (p.cell_w.max(1.0), p.cell_h.max(1.0));
    let cell = cw.max(ch);
    g.room_grid = SpatialHash::new(cell);
    g.edge_index = SpatialHash::new(cell);
    let weight_of = |c: ShapeClass, cells: i32| {
        if c == ShapeClass::Star && cells < p.star_cells {
            0.0
        } else {
            p.weights.get(c).max(0.0)
        }
    };
    // A box of 1..3 cells a side, mostly small.
    let pick_box = |rng: &mut Mulberry| -> (f64, f64, i32) {
        let k = |u: f64| {
            if u < 0.55 {
                1
            } else if u < 0.85 {
                2
            } else {
                3
            }
        };
        let (kw, kh) = (k(rng.next()), k(rng.next()));
        (
            kw as f64 * cw - 2.0 * p.gap,
            kh as f64 * ch - 2.0 * p.gap,
            kw * kh,
        )
    };
    let fit_room = |g: &mut Gen<'_>, bx: Bx, cells: i32| -> Option<(ShapeClass, Shape)> {
        let ws: Vec<f64> = CLASSES.iter().map(|c| weight_of(*c, cells)).collect();
        let mut c = CLASSES[g.pick_class(&ws)];
        let shape = match fit(c, bx.clone(), &mut g.rng, p, cells) {
            Some(s) => s,
            None => {
                c = ShapeClass::Rect;
                fit(ShapeClass::Rect, bx, &mut g.rng, p, cells)?
            }
        };
        Some((c, shape))
    };
    let make_room = |id: usize, c: ShapeClass, shape: Shape, bx: &Bx| {
        let bounds = shape_bounds(&shape);
        Room {
            id,
            cls: c,
            bounds,
            center: centroid(&shape),
            area: area_of(&shape),
            rect: Rect {
                x0: bx.x0,
                y0: bx.y0,
                x1: bx.x1,
                y1: bx.y1,
            },
            ports: ports_of(id, &shape, p),
            shape,
            used: Vec::new(),
            leaf: false,
            alive: true,
            role: Role::Plain,
            keys: 0,
            switches: 0,
        }
    };

    // The first room, in the middle of the world.
    let (w0, h0, cells0) = pick_box(&mut g.rng);
    let (mx, my) = ((world.x0 + world.x1) / 2.0, (world.y0 + world.y1) / 2.0);
    let bx0 = Bx {
        x0: mx - w0 / 2.0,
        y0: my - h0 / 2.0,
        x1: mx + w0 / 2.0,
        y1: my + h0 / 2.0,
    };
    let Some((c0, s0)) = fit_room(g, bx0.clone(), cells0) else {
        return Vec::new();
    };
    let r0 = make_room(0, c0, s0, &bx0);
    g.room_grid.insert_bounds(0, r0.bounds.lo, r0.bounds.hi);
    report_progress(progress, || GenerationEvent::RoomAdded(r0.shape.clone()));
    g.rooms.push(r0);
    g.nodes.push(Node::Room { room: 0 });
    g.node_of_room.push(0);
    let mut depth = vec![0i32];
    let mut island_of = vec![0usize];
    // Rooms within 2.5 cells of each room: the local density the clump bias works on.
    let near_radius = 2.5 * cell;
    let mut near: Vec<f64> = vec![0.0];
    let mut open: Vec<usize> = vec![0];
    let max_depth = p.growth.depth.max(1);
    let branch = p.growth.branch.max(1);
    let mut budget = target.saturating_mul(60).max(200);
    // Islands: the rooms are split into clumps; each clump grows on its own and a long bridge corridor starts the next one.
    let mut islands = p.growth.islands.max(1) as usize;
    let (mut island_idx, mut island_size) = (0usize, 1usize);
    let mut quota = (target / islands).max(1);
    let (mut need_bridge, mut bridge_tries) = (false, 0);
    // A clump stays within a radius of its first room that fits its share of the rooms.
    let island_radius = |share: usize| (0.8 * (share as f64 * 1.6).sqrt() + 0.8) * cell;
    let mut root_c = g.rooms[0].center;
    let mut radius = island_radius(quota);

    while g.rooms.len() < target && budget > 0 {
        if !need_bridge && (open.is_empty() || (island_idx + 1 < islands && island_size >= quota)) {
            if island_idx + 1 < islands {
                need_bridge = true;
                bridge_tries = 0;
            } else {
                break;
            }
        }
        let (parent, k, children, bridge) = if need_bridge {
            bridge_tries += 1;
            if bridge_tries > 60 {
                // No room for another clump: keep growing the last one.
                islands = island_idx + 1;
                need_bridge = false;
                continue;
            }
            // Any room with a free door may start a bridge.
            let any: Vec<usize> = (0..g.rooms.len())
                .filter(|&i| g.rooms[i].ports.iter().any(|q| g.port_free(q)))
                .collect();
            if any.is_empty() {
                break;
            }
            (
                any[(g.rng.next() * any.len() as f64).floor() as usize],
                usize::MAX,
                1,
                true,
            )
        } else {
            // The parent is drawn with weight e^(cluster·density): preferential attachment, so crowded spots keep growing.
            let ws: Vec<f64> = open
                .iter()
                .map(|&i| (p.growth.cluster * near[i]).clamp(-30.0, 30.0).exp())
                .collect();
            let Some(k) = g.weighted(&ws) else {
                break;
            };
            (
                open[k],
                k,
                1 + (g.rng.next() * branch as f64).floor() as usize,
                false,
            )
        };
        let mut failed = false;
        for _ in 0..children {
            if g.rooms.len() >= target {
                break;
            }
            let mut placed = false;
            for _attempt in 0..8 {
                budget = budget.saturating_sub(1);
                let free: Vec<Port> = g.rooms[parent]
                    .ports
                    .iter()
                    .copied()
                    .filter(|q| g.port_free(q))
                    .collect();
                if free.is_empty() {
                    break;
                }
                let mut proposals: Vec<(Room, f64)> = Vec::new();
                for _ in 0..8 {
                    if proposals.len() >= 4 {
                        break;
                    }
                    let door = free[(g.rng.next() * free.len() as f64).floor() as usize];
                    let u = dir_of(door.k);
                    let (bw, bh, cells) = pick_box(&mut g.rng);
                    let reach = if bridge {
                        p.min_len + (3.0 + 3.0 * g.rng.next()) * cell
                    } else {
                        let r = p.min_len
                            + p.growth.reach.max(0.0) * 0.6 * cell * -(1.0 - g.rng.next()).ln();
                        r.min(3.0 * cell)
                    };
                    let half = (bw * u[0].abs() + bh * u[1].abs()) / 2.0;
                    let lateral = (g.rng.next() - 0.5) * 0.8;
                    let (cx, cy) = (
                        door.p[0] + u[0] * (reach + half) - u[1] * lateral * bw,
                        door.p[1] + u[1] * (reach + half) + u[0] * lateral * bh,
                    );
                    let bx = Bx {
                        x0: cx - bw / 2.0,
                        y0: cy - bh / 2.0,
                        x1: cx + bw / 2.0,
                        y1: cy + bh / 2.0,
                    };
                    if bx.x0 < world.x0 || bx.y0 < world.y0 || bx.x1 > world.x1 || bx.y1 > world.y1
                    {
                        continue;
                    }
                    let Some((c, shape)) = fit_room(g, bx.clone(), cells) else {
                        continue;
                    };
                    let room = make_room(g.rooms.len(), c, shape, &bx);
                    let crowded = g
                        .room_grid
                        .query_bounds(room.bounds.lo, room.bounds.hi, 2.0 * p.gap)
                        .into_iter()
                        .any(|o| shape_within(&room.shape, &g.rooms[o].shape, 2.0 * p.gap));
                    // σ from every corridor already built (V2): the corridor to the parent does not exist yet.
                    let on_corridor = g
                        .edge_index
                        .query_bounds(room.bounds.lo, room.bounds.hi, p.clear)
                        .into_iter()
                        .any(|e| {
                            e < g.edges.len()
                                && region_shape_within(
                                    &g.edges[e].region,
                                    &room.shape,
                                    room.bounds,
                                    p.clear,
                                )
                        });
                    if crowded
                        || on_corridor
                        || (!bridge && islands > 1 && dist(room.center, root_c) > radius)
                    {
                        continue;
                    }
                    let here = g
                        .room_grid
                        .query_bounds(
                            [room.center[0] - near_radius, room.center[1] - near_radius],
                            [room.center[0] + near_radius, room.center[1] + near_radius],
                            0.0,
                        )
                        .len() as f64;
                    // A bridge goes where it likes; ordinary children follow the clump bias.
                    let w = if bridge {
                        // A bridge lands in open ground: weight grows with the distance to the nearest room.
                        let nearest = g
                            .room_grid
                            .query_bounds(
                                [room.center[0] - 6.0 * cell, room.center[1] - 6.0 * cell],
                                [room.center[0] + 6.0 * cell, room.center[1] + 6.0 * cell],
                                0.0,
                            )
                            .into_iter()
                            .map(|o| dist(g.rooms[o].center, room.center))
                            .fold(6.0 * cell, f64::min);
                        (nearest / cell).exp()
                    } else {
                        (p.growth.cluster * here).clamp(-30.0, 30.0).exp()
                    };
                    proposals.push((room, w));
                }
                if proposals.is_empty() {
                    continue;
                }
                let ws: Vec<f64> = proposals.iter().map(|(_, w)| *w).collect();
                let Some(k) = g.weighted(&ws) else {
                    continue;
                };
                let (room, _) = proposals.swap_remove(k);
                let id = room.id;
                g.rooms.push(room);
                let found: Vec<Candidate> =
                    candidates_for_indexed(p, &g.rooms, parent, id, 4, &g.room_grid)
                        .into_iter()
                        .filter(|c| g.fits_now(c))
                        .collect();
                let ws: Vec<f64> = found.iter().map(|c| c.weight).collect();
                let Some(k) = g.weighted(&ws) else {
                    g.rooms.pop();
                    continue;
                };
                let c = found.into_iter().nth(k).unwrap();
                g.room_grid
                    .insert_bounds(id, g.rooms[id].bounds.lo, g.rooms[id].bounds.hi);
                g.nodes.push(Node::Room { room: id });
                g.node_of_room.push(id);
                report_progress(progress, || {
                    GenerationEvent::RoomAdded(g.rooms[id].shape.clone())
                });
                let edge = g.add_edge(
                    g.node_of_room[parent],
                    g.node_of_room[id],
                    c.pts,
                    [Some(c.ports[0]), Some(c.ports[1])],
                );
                report_progress(progress, || GenerationEvent::EdgeUpdated {
                    id: edge,
                    pts: g.edges[edge].pts.clone(),
                });
                depth.push(depth[parent] + 1);
                island_of.push(if bridge { island_idx + 1 } else { island_idx });
                let centre = g.rooms[id].center;
                let mut count = 0.0;
                for o in 0..id {
                    let d = dist(g.rooms[o].center, centre);
                    if d <= near_radius {
                        near[o] += 1.0;
                        count += 1.0;
                    }
                }
                near.push(count);
                if bridge {
                    // The bridge's far end roots a new clump.
                    open.clear();
                    island_idx += 1;
                    island_size = 1;
                    quota = ((target - g.rooms.len()) / (islands - island_idx).max(1)).max(1);
                    root_c = g.rooms[id].center;
                    radius = island_radius(quota);
                    need_bridge = false;
                    if depth[id] < max_depth {
                        open.push(id);
                    }
                } else {
                    island_size += 1;
                    if depth[id] < max_depth {
                        open.push(id);
                    }
                }
                placed = true;
                break;
            }
            if !placed {
                failed = true;
                break;
            }
        }
        // A parent that could not place a child (no door, no space) leaves the pool.
        if !bridge && (failed || depth[parent] >= max_depth) {
            open.swap_remove(k);
        }
    }
    if g.rooms.len() < target {
        g.log.push(format!(
            "growth stopped at {} of {} rooms",
            g.rooms.len(),
            target
        ));
    }
    if islands > 1 || p.growth.islands > 1 {
        g.log.push(format!(
            "growth islands: {} of {} made",
            island_idx + 1,
            p.growth.islands.max(1)
        ));
    }
    island_of
}

pub fn generate(seed: u32, p: &Params) -> Dungeon {
    generate_inner(seed, p, None)
}

/// Generate one dungeon and report drawable geometry as it becomes available.
pub fn generate_with_progress(
    seed: u32,
    p: &Params,
    progress: &mut dyn FnMut(GenerationEvent),
) -> Dungeon {
    generate_inner(seed, p, Some(progress))
}

fn report_progress(
    progress: &mut Option<&mut dyn FnMut(GenerationEvent)>,
    event: impl FnOnce() -> GenerationEvent,
) {
    if let Some(callback) = progress.as_mut() {
        (**callback)(event());
    }
}

fn generate_inner(
    seed: u32,
    p: &Params,
    mut progress: Option<&mut dyn FnMut(GenerationEvent)>,
) -> Dungeon {
    let mut phases: Vec<(String, f64)> = Vec::new();
    let mut t_last = crate::clock::now_ms();
    let mut lap = |name: &str| {
        let t = crate::clock::now_ms();
        phases.push((name.to_string(), t - t_last));
        t_last = t;
    };
    let mut g = Gen {
        p,
        rng: Mulberry(seed),
        log: Vec::new(),
        rooms: Vec::new(),
        nodes: Vec::new(),
        node_of_room: Vec::new(),
        edges: Vec::new(),
        cache: HashMap::new(),
        statics: Vec::new(),
        static_opts: Vec::new(),
        branch_opts: BTreeMap::new(),
        candidates_by_room: Vec::new(),
        no_fit: Vec::new(),
        no_cross: Vec::new(),
        stairs_room: usize::MAX,
        static_index: SpatialHash::new(1.0),
        room_grid: SpatialHash::new(1.0),
        edge_index: SpatialHash::new(1.0),
    };

    /* §7.1 Scaffold: blocks on a grid, the big shapes first, then a share of the 1×1 cells dropped (the rest kept in one piece). */
    let (cols, rows, cell_w, cell_h, gap) =
        (p.cols.max(1), p.rows.max(1), p.cell_w, p.cell_h, p.gap);
    let total = (cols * rows) as usize;
    let mut blocks: Vec<Block> = Vec::new();
    let mut owner = vec![-1i32; total];
    let cells_of = |b: &Block| -> Vec<usize> {
        (0..b.w * b.h)
            .map(|i| ((b.r + i / b.w) * cols + b.c + i % b.w) as usize)
            .collect()
    };
    let shapes: [(i32, i32); 5] = [(2, 1), (1, 2), (2, 2), (3, 2), (2, 3)];
    let (mut tries, mut made, mut used) = (0, 0, 0);
    while tries < p.big * 12 && made < p.big {
        tries += 1;
        let (w, h) = shapes[(g.rng.next() * shapes.len() as f64).floor() as usize];
        let b = Block {
            c: (g.rng.next() * cols as f64).floor() as i32,
            r: (g.rng.next() * rows as f64).floor() as i32,
            w,
            h,
        };
        let fits =
            b.c + b.w <= cols && b.r + b.h <= rows && cells_of(&b).iter().all(|c| owner[*c] < 0);
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
            let b = Block {
                c: c as i32 % cols,
                r: c as i32 / cols,
                w: 1,
                h: 1,
            };
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
    let mut smalls: Vec<usize> = (0..blocks.len())
        .filter(|i| blocks[*i].w * blocks[*i].h == 1)
        .collect();
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
    let mut grid_x: Vec<f64> = (0..=cols).map(|i| i as f64 * cell_w).collect();
    let mut grid_y: Vec<f64> = (0..=rows).map(|i| i as f64 * cell_h).collect();
    let requested_jitter = p.layout_jitter.clamp(0.0, 0.45);
    let minimum_cell_span = 2.0 * gap + 2.0 * (p.width / 2.0 + p.clear) + p.width;
    let jitter_limit = [cell_w, cell_h].into_iter().fold(0.45f64, |limit, span| {
        if span > 0.0 {
            limit.min(((span - minimum_cell_span) / (2.0 * span)).max(0.0))
        } else {
            0.0
        }
    });
    let jitter = requested_jitter.min(jitter_limit);
    if jitter + 1e-9 < requested_jitter {
        g.log.push(format!(
            "layout jitter limited to {:.3} (requested {:.3}) to preserve minimum room size",
            jitter, requested_jitter
        ));
    }
    if jitter > 0.0 {
        for i in 1..cols as usize {
            grid_x[i] += (g.rng.next() * 2.0 - 1.0) * cell_w * jitter;
        }
        for i in 1..rows as usize {
            grid_y[i] += (g.rng.next() * 2.0 - 1.0) * cell_h * jitter;
        }
    }
    report_progress(&mut progress, || GenerationEvent::Layout {
        grid_x: grid_x.clone(),
        grid_y: grid_y.clone(),
        corridor_width: p.width,
    });
    let box_of = |b: &Block| Bx {
        x0: grid_x[b.c as usize] + gap,
        y0: grid_y[b.r as usize] + gap,
        x1: grid_x[(b.c + b.w) as usize] - gap,
        y1: grid_y[(b.r + b.h) as usize] - gap,
    };
    let live: Vec<usize> = (0..blocks.len()).filter(|i| alive[*i]).collect();
    let dropped: Vec<usize> = (0..blocks.len())
        .filter(|i| !alive[*i] && !p.growth.enabled)
        .collect();

    /* §7.2 Shape classes: weights by block size, a Potts model for the homophily, then the caps. */
    let weight_of = |c: ShapeClass, cells: i32| {
        if c == ShapeClass::Star && cells < p.star_cells {
            0.0
        } else {
            p.weights.get(c).max(0.0)
        }
    };
    let mut cls: HashMap<usize, usize> = HashMap::new();
    #[cfg(feature = "frontier-growth")]
    let mut room_of_block = vec![usize::MAX; blocks.len()];
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
                let ws: Vec<f64> = CLASSES
                    .iter()
                    .enumerate()
                    .map(|(ci, c)| {
                        weight_of(*c, cells)
                            * (p.homophily
                                * nb.iter().filter(|n| cls.get(n) == Some(&ci)).count() as f64)
                                .exp()
                    })
                    .collect();
                let k = g.pick_class(&ws);
                cls.insert(i, k);
            }
        }
    }
    let caps: Vec<usize> = CLASSES
        .iter()
        .map(|c| p.max_of.get(*c).max(0.0) as usize)
        .collect();
    let mut counts = [0usize; 5];
    for &ci in cls.values() {
        counts[ci] += 1;
    }
    for ci in 0..CLASSES.len() {
        if counts[ci] <= caps[ci] {
            continue;
        }
        let excess: Vec<usize> = live.iter().copied().filter(|i| cls[i] == ci).collect();
        for &i in excess.iter().skip(caps[ci]) {
            if counts[ci] <= caps[ci] {
                break;
            }
            let cells = blocks[i].w * blocks[i].h;
            let mut ws: Vec<f64> = CLASSES
                .iter()
                .enumerate()
                .map(|(oi, c)| {
                    if oi != ci && counts[oi] < caps[oi] {
                        weight_of(*c, cells)
                    } else {
                        0.0
                    }
                })
                .collect();
            if ws.iter().sum::<f64>() <= 0.0 {
                // If the caps are feasible but every remaining class has zero
                // sampling weight for this block, preserve the hard cap anyway.
                for oi in 0..ws.len() {
                    if oi != ci && counts[oi] < caps[oi] {
                        ws[oi] = 1.0;
                    }
                }
            }
            let Some(new_class) = g.weighted(&ws) else {
                break;
            };
            cls.insert(i, new_class);
            counts[ci] -= 1;
            counts[new_class] += 1;
        }
    }
    if counts.iter().zip(&caps).any(|(count, cap)| count > cap) {
        g.log.push(
            "shape class caps cannot fit every room; at least one cap is exceeded".to_string(),
        );
    }

    /* Rooms: each block's shape, fitted (a rectangle when its class can't fit). */
    for &i in live.iter().filter(|_| !p.growth.enabled) {
        let b = blocks[i];
        let bx = box_of(&b);
        let mut c = CLASSES[cls[&i]];
        let cells = b.w * b.h;
        let shape = match fit(c, bx, &mut g.rng, p, cells) {
            Some(s) => s,
            None => {
                g.log.push(format!(
                    "block {}: {} does not fit, rect instead",
                    i,
                    c.name()
                ));
                c = ShapeClass::Rect;
                fit(ShapeClass::Rect, bx, &mut g.rng, p, cells).expect("a rect always fits a block")
            }
        };
        let id = g.rooms.len();
        #[cfg(feature = "frontier-growth")]
        {
            room_of_block[i] = id;
        }
        let bounds = shape_bounds(&shape);
        g.rooms.push(Room {
            id,
            cls: c,
            bounds,
            center: centroid(&shape),
            area: area_of(&shape),
            rect: Rect {
                x0: bx.x0,
                y0: bx.y0,
                x1: bx.x1,
                y1: bx.y1,
            },
            ports: ports_of(id, &shape, p),
            shape,
            used: Vec::new(),
            leaf: false,
            alive: !cfg!(feature = "frontier-growth"),
            role: Role::Plain,
            keys: 0,
            switches: 0,
        });
        if !cfg!(feature = "frontier-growth") {
            report_progress(&mut progress, || {
                GenerationEvent::RoomAdded(g.rooms[id].shape.clone())
            });
        }
    }
    g.nodes = g.rooms.iter().map(|r| Node::Room { room: r.id }).collect();
    g.node_of_room = g.rooms.iter().map(|r| r.id).collect();

    let mut island_of: Vec<usize> = Vec::new();
    if p.growth.enabled {
        let target = if p.growth.rooms > 0 {
            p.growth.rooms as usize
        } else {
            live.len()
        };
        let world = Bx {
            x0: grid_x[0],
            y0: grid_y[0],
            x1: grid_x[cols as usize],
            y1: grid_y[rows as usize],
        };
        island_of = grow_rooms(&mut g, world, target, &mut progress);
    }
    lap("rooms");
    let room_index_cell = [p.cell_w, p.cell_h]
        .into_iter()
        .filter(|v| v.is_finite())
        .map(f64::abs)
        .fold(1.0, f64::max);
    let mut room_index = SpatialHash::new(room_index_cell);
    for room in &g.rooms {
        room_index.insert_bounds(room.id, room.bounds.lo, room.bounds.hi);
    }
    g.room_grid = SpatialHash::new(room_index_cell);
    for room in &g.rooms {
        g.room_grid
            .insert_bounds(room.id, room.bounds.lo, room.bounds.hi);
    }
    g.edge_index = SpatialHash::new(room_index_cell);
    for e in &g.edges {
        g.edge_index.insert_region(e.id, &e.region);
    }
    /* §7.5.1 The spanning tree: Prim from the start, weighted by ω; the proximity graph relaxed when it gets stuck. */
    let start_room = if p.growth.enabled {
        0
    } else {
        (g.rng.next() * g.rooms.len() as f64).floor() as usize
    };
    let order: Vec<Proximity> = match p.proximity {
        Proximity::Rng => vec![Proximity::Rng, Proximity::Gabriel, Proximity::Near],
        Proximity::Gabriel => vec![Proximity::Gabriel, Proximity::Near],
        Proximity::Near => vec![Proximity::Near],
    };
    #[cfg(feature = "frontier-growth")]
    let scaffold_neighbors: Vec<Vec<usize>> = {
        let mut neighbors = vec![Vec::new(); g.rooms.len()];
        for &block in &live {
            let room = room_of_block[block];
            for other_block in touching(block) {
                if alive[other_block] {
                    let other_room = room_of_block[other_block];
                    if room != other_room {
                        neighbors[room].push(other_room);
                    }
                }
            }
        }
        for adjacent in &mut neighbors {
            adjacent.sort_unstable();
            adjacent.dedup();
        }
        neighbors
    };
    #[cfg(feature = "frontier-growth")]
    let mut using_scaffold_frontier = true;
    #[cfg(not(feature = "frontier-growth"))]
    let mut using_scaffold_frontier = false;
    let mut level = 0;
    let mut near = p.near.max(0.0);
    let near_base = near;
    #[cfg(feature = "frontier-growth")]
    let mut pairs: Vec<[usize; 2]> = scaffold_neighbors[start_room]
        .iter()
        .map(|&other| [start_room.min(other), start_room.max(other)])
        .collect();
    #[cfg(not(feature = "frontier-growth"))]
    let mut pairs = g.proximity_pairs(order[0], near);
    let mut in_tree: HashSet<usize> = HashSet::from([start_room]);
    if p.growth.enabled {
        // The growth already joined every room to its parent: the tree phase has nothing left to do.
        in_tree.extend(0..g.rooms.len());
    }
    #[cfg(feature = "frontier-growth")]
    {
        g.rooms[start_room].alive = true;
        report_progress(&mut progress, || {
            GenerationEvent::RoomAdded(g.rooms[start_room].shape.clone())
        });
    }
    let mut flat: Vec<Candidate> = Vec::new();
    let mut state: Vec<u8> = Vec::new(); // 0 = unknown/dirty, 1 = fits, 2 = permanently blocked
    let mut dirty_stamp: Vec<u32> = Vec::new();
    let mut dirty_epoch = 0u32;
    let mut by_room: Vec<Vec<usize>> = Vec::new();
    let mut frontier: BTreeSet<usize> = BTreeSet::new();
    let mut flat_spatial: Option<SpatialHash> = None;
    #[cfg(feature = "diagnostic-profile")]
    let mut tree_diagnostics = TreeDiagnostics::default();
    // Baseline batches its candidate graph. The frontier experiment starts with
    // only links adjacent to the start room and discovers more as rooms join.
    g.ensure(&pairs, &room_index);
    lap("candidates");
    while in_tree.len() < g.rooms.len() {
        if flat.is_empty() && !pairs.is_empty() {
            #[cfg(feature = "diagnostic-profile")]
            let rebuild_started = Instant::now();
            flat = pairs
                .iter()
                .flat_map(|[a, b]| g.cache[&(*a, *b)].iter().cloned())
                .collect();
            state = vec![0; flat.len()];
            dirty_stamp = vec![0; flat.len()];
            by_room = vec![Vec::new(); g.rooms.len()];
            let cell = [p.cell_w, p.cell_h]
                .into_iter()
                .filter(|v| v.is_finite())
                .map(f64::abs)
                .fold(1.0, f64::max);
            let mut index = SpatialHash::new(cell);
            for (ci, c) in flat.iter().enumerate() {
                by_room[c.a].push(ci);
                by_room[c.b].push(ci);
                index.insert_region(ci, &c.region);
                if in_tree.contains(&c.a) != in_tree.contains(&c.b) {
                    frontier.insert(ci);
                }
            }
            flat_spatial = Some(index);
            #[cfg(feature = "diagnostic-profile")]
            {
                tree_diagnostics.graph_rebuild += rebuild_started.elapsed();
            }
        }
        // Only the current cut is traversed. Fit checks are repeated only for candidates
        // touched by a newly used room port or by nearby corridor geometry.
        #[cfg(feature = "diagnostic-profile")]
        {
            tree_diagnostics.iterations += 1;
        }
        #[cfg(feature = "diagnostic-profile")]
        let frontier_scan_started = Instant::now();
        let live: Vec<usize> = frontier.iter().copied().collect();
        let dirty: Vec<usize> = live.iter().copied().filter(|ci| state[*ci] == 0).collect();
        #[cfg(feature = "diagnostic-profile")]
        {
            tree_diagnostics.frontier_items_seen += live.len();
            tree_diagnostics.max_frontier = tree_diagnostics.max_frontier.max(frontier.len());
            tree_diagnostics.frontier_scan += frontier_scan_started.elapsed();
        }
        #[cfg(feature = "diagnostic-profile")]
        let fit_started = Instant::now();
        let fits = pmap_min(&dirty, 128, |ci| g.fits_now(&flat[*ci]));
        #[cfg(feature = "diagnostic-profile")]
        {
            tree_diagnostics.fit_checks += dirty.len();
            tree_diagnostics.fit_time += fit_started.elapsed();
        }
        for (&ci, ok) in dirty.iter().zip(fits) {
            if ok {
                state[ci] = 1;
            } else {
                state[ci] = 2;
                frontier.remove(&ci);
            }
        }
        #[cfg(feature = "diagnostic-profile")]
        let valid_scan_started = Instant::now();
        let open: Vec<usize> = frontier
            .iter()
            .copied()
            .filter(|ci| state[*ci] == 1)
            .collect();
        let ws: Vec<f64> = open.iter().map(|ci| flat[*ci].weight).collect();
        #[cfg(feature = "diagnostic-profile")]
        {
            tree_diagnostics.frontier_items_seen += frontier.len();
            tree_diagnostics.open_candidates_seen += open.len();
            tree_diagnostics.frontier_scan += valid_scan_started.elapsed();
        }
        #[cfg(feature = "diagnostic-profile")]
        let weighted_started = Instant::now();
        let selected = g.weighted(&ws);
        #[cfg(feature = "diagnostic-profile")]
        {
            tree_diagnostics.weighted_pick += weighted_started.elapsed();
        }
        if let Some(k) = selected {
            #[cfg(feature = "diagnostic-profile")]
            let update_started = Instant::now();
            let c = flat[open[k]].clone();
            let newly_added = if in_tree.contains(&c.a) { c.b } else { c.a };
            #[cfg(feature = "frontier-growth")]
            {
                g.rooms[newly_added].alive = true;
                report_progress(&mut progress, || {
                    GenerationEvent::RoomAdded(g.rooms[newly_added].shape.clone())
                });
            }
            let edge = g.add_edge(
                g.node_of_room[c.a],
                g.node_of_room[c.b],
                c.pts,
                [Some(c.ports[0]), Some(c.ports[1])],
            );
            report_progress(&mut progress, || GenerationEvent::EdgeUpdated {
                id: edge,
                pts: g.edges[edge].pts.clone(),
            });
            in_tree.insert(c.a);
            in_tree.insert(c.b);
            for &ci in &by_room[newly_added] {
                if in_tree.contains(&flat[ci].a) != in_tree.contains(&flat[ci].b) && state[ci] != 2
                {
                    frontier.insert(ci);
                } else {
                    frontier.remove(&ci);
                }
            }
            dirty_epoch = dirty_epoch.wrapping_add(1);
            if dirty_epoch == 0 {
                dirty_stamp.fill(0);
                dirty_epoch = 1;
            }
            let mut affected = Vec::new();
            let mut mark_affected = |ci: usize| {
                if dirty_stamp[ci] != dirty_epoch {
                    dirty_stamp[ci] = dirty_epoch;
                    affected.push(ci);
                }
            };
            for room in [c.a, c.b] {
                for &ci in &by_room[room] {
                    mark_affected(ci);
                }
            }
            for ci in flat_spatial
                .as_ref()
                .unwrap()
                .query_region(&g.edges[edge].region, p.clear)
            {
                mark_affected(ci);
            }
            for ci in affected {
                if state[ci] == 1 {
                    state[ci] = 0;
                }
            }
            #[cfg(feature = "frontier-growth")]
            {
                let mut new_pairs: Vec<[usize; 2]> = scaffold_neighbors[newly_added]
                    .iter()
                    .copied()
                    .filter(|room| !in_tree.contains(room))
                    .filter(|room| {
                        !g.cache
                            .contains_key(&(newly_added.min(*room), newly_added.max(*room)))
                    })
                    .map(|room| [newly_added.min(room), newly_added.max(room)])
                    .collect();
                new_pairs.sort_unstable();
                new_pairs.dedup();
                if !new_pairs.is_empty() {
                    #[cfg(feature = "diagnostic-profile")]
                    let expand_started = Instant::now();
                    g.ensure(&new_pairs, &room_index);
                    pairs.extend(new_pairs.iter().copied());
                    pairs.sort_unstable();
                    pairs.dedup();
                    for [a, b] in new_pairs {
                        for candidate in g.cache[&(a, b)].iter().cloned() {
                            let ci = flat.len();
                            by_room[a].push(ci);
                            by_room[b].push(ci);
                            if in_tree.contains(&a) != in_tree.contains(&b) {
                                frontier.insert(ci);
                            }
                            flat.push(candidate);
                            state.push(0);
                            dirty_stamp.push(0);
                            flat_spatial
                                .as_mut()
                                .unwrap()
                                .insert_region(ci, &flat[ci].region);
                        }
                    }
                    #[cfg(feature = "diagnostic-profile")]
                    {
                        tree_diagnostics.frontier_ensure += expand_started.elapsed();
                    }
                }
            }
            #[cfg(feature = "diagnostic-profile")]
            {
                tree_diagnostics.edge_update += update_started.elapsed();
            }
            continue;
        }
        if using_scaffold_frontier {
            using_scaffold_frontier = false;
            g.log
                .push("scaffold frontier stalled; expanding to the proximity graph".into());
            #[cfg(feature = "diagnostic-profile")]
            let graph_started = Instant::now();
            pairs = g.proximity_pairs(order[level], near);
            #[cfg(feature = "diagnostic-profile")]
            let graph_time = graph_started.elapsed();
            #[cfg(feature = "diagnostic-profile")]
            let (old_pair_count, old_candidate_count) =
                (g.cache.len(), g.cache.values().map(Vec::len).sum::<usize>());
            #[cfg(feature = "diagnostic-profile")]
            let ensure_started = Instant::now();
            g.ensure(&pairs, &room_index);
            #[cfg(feature = "diagnostic-profile")]
            {
                let new_pairs = g.cache.len().saturating_sub(old_pair_count);
                let new_candidates = g
                    .cache
                    .values()
                    .map(Vec::len)
                    .sum::<usize>()
                    .saturating_sub(old_candidate_count);
                tree_diagnostics.fallbacks.push(format!(
                    "scaffold-frontier fallback: pairs={} new_pairs={} new_candidates={} graph={:.2}ms ensure={:.2}ms",
                    pairs.len(),
                    new_pairs,
                    new_candidates,
                    graph_time.as_secs_f64() * 1_000.0,
                    ensure_started.elapsed().as_secs_f64() * 1_000.0,
                ));
            }
            flat.clear();
            state.clear();
            dirty_stamp.clear();
            by_room.clear();
            frontier.clear();
            flat_spatial = None;
        } else if level < order.len() - 1 {
            level += 1;
            g.log
                .push(format!("tree stuck: candidates from {:?}", order[level]).to_lowercase());
            #[cfg(feature = "diagnostic-profile")]
            let graph_started = Instant::now();
            pairs = g.proximity_pairs(order[level], near);
            #[cfg(feature = "diagnostic-profile")]
            let graph_time = graph_started.elapsed();
            #[cfg(feature = "diagnostic-profile")]
            let (old_pair_count, old_candidate_count) =
                (g.cache.len(), g.cache.values().map(Vec::len).sum::<usize>());
            #[cfg(feature = "diagnostic-profile")]
            let ensure_started = Instant::now();
            g.ensure(&pairs, &room_index);
            #[cfg(feature = "diagnostic-profile")]
            {
                let new_pairs = g.cache.len().saturating_sub(old_pair_count);
                let new_candidates = g
                    .cache
                    .values()
                    .map(Vec::len)
                    .sum::<usize>()
                    .saturating_sub(old_candidate_count);
                tree_diagnostics.fallbacks.push(format!(
                    "{:?}: pairs={} new_pairs={} new_candidates={} graph={:.2}ms ensure={:.2}ms",
                    order[level],
                    pairs.len(),
                    new_pairs,
                    new_candidates,
                    graph_time.as_secs_f64() * 1_000.0,
                    ensure_started.elapsed().as_secs_f64() * 1_000.0,
                ));
            }
            flat.clear();
            state.clear();
            dirty_stamp.clear();
            by_room.clear();
            frontier.clear();
            flat_spatial = None;
        } else if near < 4.0 * cell_w.max(cell_h) {
            near = if near == 0.0 { 1.0 } else { near * 1.5 };
            g.log.push(format!("tree stuck: near {} px", near.round()));
            #[cfg(feature = "diagnostic-profile")]
            let graph_started = Instant::now();
            // Only pairs touching a room still outside the tree can unstick it; pairs inside it are kept only when already worked out.
            pairs = g.proximity_pairs(Proximity::Near, near);
            pairs.retain(|[a, b]| {
                !in_tree.contains(a) || !in_tree.contains(b) || g.cache.contains_key(&(*a, *b))
            });
            #[cfg(feature = "diagnostic-profile")]
            let graph_time = graph_started.elapsed();
            #[cfg(feature = "diagnostic-profile")]
            let (old_pair_count, old_candidate_count) =
                (g.cache.len(), g.cache.values().map(Vec::len).sum::<usize>());
            #[cfg(feature = "diagnostic-profile")]
            let ensure_started = Instant::now();
            g.ensure(&pairs, &room_index);
            #[cfg(feature = "diagnostic-profile")]
            {
                let new_pairs = g.cache.len().saturating_sub(old_pair_count);
                let new_candidates = g
                    .cache
                    .values()
                    .map(Vec::len)
                    .sum::<usize>()
                    .saturating_sub(old_candidate_count);
                tree_diagnostics.fallbacks.push(format!(
                    "Near({near:.0}px): pairs={} new_pairs={} new_candidates={} graph={:.2}ms ensure={:.2}ms",
                    pairs.len(),
                    new_pairs,
                    new_candidates,
                    graph_time.as_secs_f64() * 1_000.0,
                    ensure_started.elapsed().as_secs_f64() * 1_000.0,
                ));
            }
            flat.clear();
            state.clear();
            dirty_stamp.clear();
            by_room.clear();
            frontier.clear();
            flat_spatial = None;
        } else {
            for r in g.rooms.iter_mut() {
                if !in_tree.contains(&r.id) {
                    r.alive = false;
                }
            }
            g.log.push(format!(
                "{} room(s) left out: no valid corridor reaches them",
                g.rooms.len() - in_tree.len()
            ));
            break;
        }
    }

    #[cfg(feature = "diagnostic-profile")]
    g.log.push(format!(
        "diag tree: iterations={} frontier_items={} max_frontier={} fit_checks={} open_candidates={} frontier_scan={:.2}ms fit_time={:.2}ms weighted_pick={:.2}ms edge_update={:.2}ms frontier_ensure={:.2}ms graph_rebuild={:.2}ms fallbacks=[{}]",
        tree_diagnostics.iterations,
        tree_diagnostics.frontier_items_seen,
        tree_diagnostics.max_frontier,
        tree_diagnostics.fit_checks,
        tree_diagnostics.open_candidates_seen,
        tree_diagnostics.frontier_scan.as_secs_f64() * 1_000.0,
        tree_diagnostics.fit_time.as_secs_f64() * 1_000.0,
        tree_diagnostics.weighted_pick.as_secs_f64() * 1_000.0,
        tree_diagnostics.edge_update.as_secs_f64() * 1_000.0,
        tree_diagnostics.frontier_ensure.as_secs_f64() * 1_000.0,
        tree_diagnostics.graph_rebuild.as_secs_f64() * 1_000.0,
        tree_diagnostics.fallbacks.join("; "),
    ));

    lap("tree");
    drop(flat);
    drop(state);
    drop(dirty_stamp);
    drop(by_room);
    drop(frontier);
    drop(flat_spatial);
    /* §7.5.2 Roles: the start, the stairs at a far leaf. */
    let h0 = room_hops(&g.nodes, &g.edges, g.node_of_room[start_room]);
    let leaves: Vec<usize> = g
        .rooms
        .iter()
        .filter(|r| r.alive && r.id != start_room && g.degree(r.id) == 1)
        .map(|r| r.id)
        .collect();
    let pool: Vec<usize> = if leaves.is_empty() {
        g.rooms
            .iter()
            .filter(|r| r.alive && r.id != start_room)
            .map(|r| r.id)
            .collect()
    } else {
        leaves
    };
    // A start room no corridor can leave leaves nothing to pick: the stairs stay on it, V5 flags the draw and `stage` redraws.
    let mut stairs_room = pool.first().copied().unwrap_or(start_room);
    for &r in &pool {
        if h0[g.node_of_room[r]] > h0[g.node_of_room[stairs_room]] {
            stairs_room = r;
        }
    }
    if p.growth.enabled {
        // The cycles added later shorten walks, so the pick keeps a margin over `minHops` when the tree is deep enough for one.
        let pick = |g: &Gen<'_>, wanted: f64| -> Vec<usize> {
            g.rooms
                .iter()
                .filter(|r| {
                    r.alive
                        && r.id != start_room
                        && h0[g.node_of_room[r.id]] >= wanted
                        && g.degree(r.id) <= p.stairs_doors as usize
                })
                .map(|r| r.id)
                .collect()
        };
        let mut eligible = pick(&g, p.min_hops as f64 + 2.0);
        if eligible.is_empty() {
            eligible = pick(&g, p.min_hops as f64);
        }
        if !eligible.is_empty() {
            stairs_room = eligible[(g.rng.next() * eligible.len() as f64).floor() as usize];
        }
    }
    if h0[g.node_of_room[stairs_room]] < p.min_hops as f64 {
        g.log.push(format!(
            "stairs only {} hops from the start (wanted {})",
            h0[g.node_of_room[stairs_room]], p.min_hops
        ));
    }
    g.rooms[start_room].role = Role::Start;
    g.rooms[stairs_room].role = Role::Stairs;
    g.stairs_room = stairs_room;

    /* §7.4 β_max: greedily add the highest-weight valid cycle action on a copy.
     * This includes direct links, T branches and X crossings; an X contributes
     * two independent cycles, so counting only direct links understates the
     * density reference whenever junction actions are enabled.
     */
    // Cycle candidates come from the base proximity graph, not the radius the tree needed to reach a stray room.
    let mut all_pairs = g.proximity_pairs(order[0], near_base);
    if p.growth.enabled && p.growth.islands > 1 {
        // Cycles stay inside a clump: the bridges are the only way between clumps.
        all_pairs.retain(|[a, b]| island_of.get(*a) == island_of.get(*b));
    }
    let live_pairs: Vec<[usize; 2]> = all_pairs
        .iter()
        .copied()
        .filter(|[a, b]| g.rooms[*a].alive && g.rooms[*b].alive)
        .collect();
    g.ensure(&live_pairs, &room_index);
    g.statics.clear();
    for [a, b] in &live_pairs {
        if let Some(mut candidates) = g.cache.remove(&(*a, *b)) {
            g.statics.append(&mut candidates);
        }
    }
    // Candidate vectors from the earlier, relaxed tree graph are no longer
    // needed; keeping them would duplicate all corridor geometry during the
    // expensive cycle phase.
    g.cache = HashMap::new();
    g.no_fit = g.statics.iter().map(|_| AtomicBool::new(false)).collect();
    g.no_cross = g.statics.iter().map(|_| AtomicBool::new(false)).collect();
    g.rebuild_option_cache();
    let alive_rooms = g.rooms.iter().filter(|r| r.alive).count() as i32;
    let beta_now = |g: &Gen| {
        g.edges.len() as i32
            - (alive_rooms
                + g.nodes
                    .iter()
                    .filter(|n| matches!(n, Node::Junction { .. }))
                    .count() as i32)
            + 1
    };
    let beta_max;
    {
        let save_used: Vec<Vec<Port>> = g.rooms.iter().map(|r| r.used.clone()).collect();
        let save_nodes = g.nodes.clone();
        let save_edges = g.edges.clone();
        loop {
            let mut opts = g.options();
            let best = opts
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.weight.partial_cmp(&b.weight).unwrap())
                .map(|(i, _)| i);
            let Some(best) = best else { break };
            let changes = g.apply(opts.swap_remove(best).act);
            g.refresh_option_cache(&changes);
        }
        beta_max = beta_now(&g);
        g.nodes = save_nodes;
        g.edges = save_edges;
        for (r, u) in g.rooms.iter_mut().zip(save_used) {
            r.used = u;
        }
        // The copy may have marked candidates as permanently blocked only
        // because of additions made during this hypothetical maximal fill.
        for blocked in &g.no_fit {
            blocked.store(false, Ordering::Relaxed);
        }
        for blocked in &g.no_cross {
            blocked.store(false, Ordering::Relaxed);
        }
        g.rebuild_option_cache();
    }
    lap("β_max");
    let topology_req = p.topology.requirements();
    let beta_target = (p.delta * beta_max as f64).round() as i32;

    /* §7.5.3 Add cycles and profile-specific topology features. */
    loop {
        let beta = beta_now(&g);
        let (routes, from_cut, to_cut) = if topology_req.min_exit_routes > 1 {
            crate::topology::edge_connectivity_frontiers(
                g.nodes.len(),
                &g.edges,
                g.node_of_room[start_room],
                g.node_of_room[stairs_room],
            )
        } else {
            (1, vec![false; g.nodes.len()], vec![false; g.nodes.len()])
        };
        let need_routes = routes < topology_req.min_exit_routes;
        let need_hub = topology_req.min_hub_degree > 0
            && crate::topology::max_degree(g.nodes.len(), &g.edges) < topology_req.min_hub_degree;
        let need_loop = topology_req.min_cycle_length > 0
            && crate::topology::cycle_girth(g.nodes.len(), &g.edges).unwrap_or(0)
                < topology_req.min_cycle_length;
        if beta >= beta_target && !need_routes && !need_hub && !need_loop {
            break;
        }
        let mut opts = g.options();
        if need_routes || need_loop {
            opts.retain(|opt| {
                let Action::Add(ci) = opt.act else {
                    return false;
                };
                let c = &g.statics[ci];
                let a = g.node_of_room[c.a];
                let b = g.node_of_room[c.b];
                let crosses_all_cuts = (from_cut[a] && to_cut[b]) || (from_cut[b] && to_cut[a]);
                let makes_long_cycle =
                    crate::topology::shortest_path(g.nodes.len(), &g.edges, a, b)
                        .is_some_and(|(_, path)| path.len() + 1 >= topology_req.min_cycle_length);
                (!need_routes || crosses_all_cuts) && (!need_loop || makes_long_cycle)
            });
        }
        let solving_hub = need_hub && !need_routes && !need_loop;
        if solving_hub {
            opts.retain(|opt| matches!(opt.act, Action::Branch { .. } | Action::Cross { .. }));
            let center = [cols as f64 * cell_w / 2.0, rows as f64 * cell_h / 2.0];
            let scale = cell_w.max(cell_h).max(1.0);
            for opt in &mut opts {
                let h = match opt.act {
                    Action::Branch { h, .. } | Action::Cross { h, .. } => h,
                    _ => continue,
                };
                opt.weight *= (-(dist(center, h) / scale)).exp();
            }
        }
        let ws: Vec<f64> = opts.iter().map(|o| o.weight).collect();
        match g.weighted(&ws) {
            Some(k) => {
                let act = opts.swap_remove(k).act;
                let changes = g.apply(act);
                for edge in changes.edges.iter().copied() {
                    report_progress(&mut progress, || GenerationEvent::EdgeUpdated {
                        id: edge,
                        pts: g.edges[edge].pts.clone(),
                    });
                }
                g.refresh_option_cache(&changes);
            }
            None => {
                let b = beta_now(&g);
                if need_routes {
                    g.log.push(format!("topology profile: could not reach {} independent routes to stairs (λ = {})", topology_req.min_exit_routes, routes));
                }
                if solving_hub {
                    g.log.push(format!(
                        "topology profile: no valid junction can reach degree {}",
                        topology_req.min_hub_degree
                    ));
                }
                if need_loop {
                    g.log.push(format!(
                        "topology profile: no valid cycle of at least {} branches fits",
                        topology_req.min_cycle_length
                    ));
                }
                if b < beta_target {
                    g.log.push(format!(
                        "β* = {} not reached: nothing more fits (β = {})",
                        beta_target, b
                    ));
                }
                break;
            }
        }
    }

    lap("cycles");
    if p.growth.enabled && topology_req.min_exit_routes <= 1 {
        // A grown tree is shallow, so the cycles can bring the stairs under `minHops` of the start: move them to a room that is still far enough.
        let h = room_hops(&g.nodes, &g.edges, g.node_of_room[start_room]);
        if h[g.node_of_room[stairs_room]] < p.min_hops as f64 {
            let far: Vec<usize> = g
                .rooms
                .iter()
                .filter(|r| {
                    r.alive
                        && r.id != start_room
                        && r.role == Role::Plain
                        && h[g.node_of_room[r.id]] >= p.min_hops as f64
                        && g.degree(r.id) <= p.stairs_doors as usize
                })
                .map(|r| r.id)
                .collect();
            if !far.is_empty() {
                g.rooms[stairs_room].role = Role::Plain;
                stairs_room = far[(g.rng.next() * far.len() as f64).floor() as usize];
                g.rooms[stairs_room].role = Role::Stairs;
                g.stairs_room = stairs_room;
            }
        }
    }
    let (main_path, main_path_edges) = crate::topology::shortest_path(
        g.nodes.len(),
        &g.edges,
        g.node_of_room[start_room],
        g.node_of_room[stairs_room],
    )
    .unwrap_or_default();
    let main_path_rooms = path_rooms(&g.nodes, &main_path);
    if p.pacing.enabled {
        assign_pacing(&mut g, &main_path, p.ambush.max(0) as usize);
    }
    /* §7.6 Leaf chambers: small rooms in the dropped cells, each hung by one corridor off a room nearby. */
    let leaf_weights: Vec<f64> = CLASSES
        .iter()
        .map(|c| p.leaf_classes.get(*c).max(0.0))
        .collect();
    let mut spots = dropped.clone();
    for i in (1..spots.len()).rev() {
        let j = (g.rng.next() * (i + 1) as f64).floor() as usize;
        spots.swap(i, j);
    }
    let (existing_branches, existing_branch_depth) =
        crate::topology::side_branch_stats(&g.nodes, &g.edges, &main_path);
    let missing_branches = topology_req.min_branches.saturating_sub(existing_branches);
    let depth_goal = if existing_branch_depth < topology_req.min_branch_depth {
        topology_req.min_branch_depth
    } else {
        0
    };
    let mut branches_added = 0usize;
    let mut branch_tips: Vec<(usize, usize)> = Vec::new(); // (tip room, depth)
    let mut depth_branch: Option<usize> = None;
    let mut made_leaves = 0usize;
    for bi in spots {
        let need_depth_branch = depth_goal > 0 && missing_branches == 0 && depth_branch.is_none();
        let depth_satisfied =
            depth_goal == 0 || depth_branch.is_some_and(|i| branch_tips[i].1 >= depth_goal);
        let profile_satisfied = branches_added >= missing_branches && depth_satisfied;
        if made_leaves >= p.leaves.max(0) as usize && profile_satisfied {
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
        if g.rooms
            .iter()
            .any(|o| o.alive && shape_within(&o.shape, &shape, 2.0 * gap))
            || g.edges
                .iter()
                .any(|e| region_within(std::slice::from_ref(&sb), &e.region, p.clear))
        {
            continue;
        }
        let id = g.rooms.len();
        let bounds = shape_bounds(&shape);
        let room = Room {
            id,
            cls: c,
            bounds,
            center: centroid(&shape),
            area: area_of(&shape),
            rect: Rect {
                x0: bx.x0,
                y0: bx.y0,
                x1: bx.x1,
                y1: bx.y1,
            },
            ports: ports_of(id, &shape, p),
            shape,
            used: Vec::new(),
            leaf: true,
            alive: true,
            role: Role::Empty,
            keys: 0,
            switches: 0,
        };
        let center = room.center;
        g.rooms.push(room);
        g.nodes.push(Node::Room { room: id });
        g.node_of_room.push(g.nodes.len() - 1);
        let create_branch = branches_added < missing_branches || need_depth_branch;
        let extend_depth =
            !create_branch && depth_branch.is_some_and(|i| branch_tips[i].1 < depth_goal);
        let preferred_hosts: Vec<usize> = if create_branch {
            main_path_rooms.clone()
        } else if extend_depth {
            vec![branch_tips[depth_branch.unwrap()].0]
        } else if p.topology.profile == TopologyProfile::Backbone {
            main_path_rooms.clone()
        } else {
            g.rooms
                .iter()
                .filter(|o| o.alive && !o.leaf)
                .map(|o| o.id)
                .collect()
        };
        let near_rooms: Vec<usize> = preferred_hosts
            .into_iter()
            .filter(|id| {
                let room = &g.rooms[*id];
                room.alive
                    && !g.locked_full(*id)
                    && dist(room.center, center) <= 1.8 * cell_w.max(cell_h)
            })
            .collect();
        let mut options: Vec<Candidate> = pflat_min(&near_rooms, 12, |o| {
            candidates_for_indexed(p, &g.rooms, id, *o, 2, &room_index)
                .into_iter()
                .filter(|k| g.fits_now(k))
                .collect()
        });
        options.sort_by(|x, y| x.cost.partial_cmp(&y.cost).unwrap());
        let Some(best) = options.into_iter().next() else {
            g.rooms.pop();
            g.nodes.pop();
            g.node_of_room.pop();
            continue;
        };
        g.add_edge(
            g.node_of_room[best.a],
            g.node_of_room[best.b],
            best.pts,
            [Some(best.ports[0]), Some(best.ports[1])],
        );
        room_index.insert_bounds(id, g.rooms[id].bounds.lo, g.rooms[id].bounds.hi);
        let edge_id = g.edges.len() - 1;
        report_progress(&mut progress, || GenerationEvent::EdgeUpdated {
            id: edge_id,
            pts: g.edges[edge_id].pts.clone(),
        });
        report_progress(&mut progress, || {
            GenerationEvent::RoomAdded(g.rooms[id].shape.clone())
        });
        let rw = p.leaf_roles;
        let weights = if p.pacing.enabled
            && (create_branch || p.topology.profile == TopologyProfile::Backbone)
        {
            [rw.trap * 0.5, rw.prize * 1.5, rw.empty]
        } else {
            [rw.trap, rw.prize, rw.empty]
        };
        let k = g.pick_class(&weights);
        g.rooms[id].role = [Role::Trap, Role::Prize, Role::Empty][k];
        made_leaves += 1;
        if create_branch {
            branches_added += 1;
            branch_tips.push((id, 1));
            if depth_goal > 0 && depth_branch.is_none() {
                depth_branch = Some(branch_tips.len() - 1);
            }
        } else if extend_depth {
            let chain = depth_branch.unwrap();
            branch_tips[chain] = (id, branch_tips[chain].1 + 1);
        }
    }
    let depth_satisfied =
        depth_goal == 0 || depth_branch.is_some_and(|i| branch_tips[i].1 >= depth_goal);
    if made_leaves < p.leaves.max(0) as usize {
        g.log
            .push(format!("{} of {} leaf chambers fit", made_leaves, p.leaves));
    }
    if branches_added < missing_branches || !depth_satisfied {
        g.log.push(format!(
            "topology profile: added {} of {} required side branches",
            branches_added, missing_branches
        ));
    }

    /* §7.7 (part) Ambush rooms: the biggest plain rooms. */
    if !p.pacing.enabled {
        let mut plain: Vec<usize> = g
            .rooms
            .iter()
            .filter(|r| r.alive && !r.leaf && r.role == Role::Plain)
            .map(|r| r.id)
            .collect();
        plain.sort_by(|a, b| g.rooms[*b].area.partial_cmp(&g.rooms[*a].area).unwrap());
        for id in plain.into_iter().take(p.ambush.max(0) as usize) {
            g.rooms[id].role = Role::Ambush;
        }
    }

    assign_progression(&mut g, &main_path, &main_path_edges);

    lap("leaf chambers");
    let dropped_centers: Vec<V> = dropped
        .iter()
        .map(|i| {
            let b = box_of(&blocks[*i]);
            [(b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0]
        })
        .collect();
    let candidates = g
        .statics
        .into_iter()
        .map(|c| CandidateView {
            a: c.a,
            b: c.b,
            pts: c.pts,
            kind: c.kind,
        })
        .collect();
    Dungeon {
        params: p.clone(),
        seed,
        rooms: g.rooms,
        nodes: g.nodes,
        edges: g.edges,
        start: start_room,
        stairs: stairs_room,
        main_path,
        grid_x,
        grid_y,
        beta_max,
        beta_target,
        candidates,
        pairs: all_pairs,
        dropped: dropped_centers,
        log: g.log,
        phases,
    }
}
