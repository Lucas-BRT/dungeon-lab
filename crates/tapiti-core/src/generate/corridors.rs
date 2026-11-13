//! Corridors between two doors.

use super::*;

pub(super) fn turn_ok(a: i32, b: i32, pr: &Params) -> bool {
    let t = turn_of(a, b);
    t == 0 || t == 2 || (t == 1 && pr.m == 8)
}

/// The ways (§4.3) to join door a to door b with at most `maxBends` bends: I, L and Z, each a polyline from p_a to p_b.
pub(super) fn emit_route(
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

pub(super) fn for_each_route(
    pa: &Port,
    pb: &Port,
    pr: &Params,
    mut visit: impl FnMut(Vec<V>, RouteType),
) {
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
pub(super) fn region_of(pts: &[V], ends: &[Option<Port>; 2], pr: &Params) -> Vec<Bounded> {
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
