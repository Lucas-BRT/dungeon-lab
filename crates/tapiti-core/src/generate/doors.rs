//! Doors (ports) on a room outline.

use super::*;

/// The exit band F(p, θ) of §3.2, starting `from` px out of the wall.
pub(super) fn exit_band(p: V, k: i32, pr: &Params, from: f64) -> Shape {
    band(
        add(p, mul(dir_of(k), from)),
        add(p, mul(dir_of(k), pr.exit)),
        pr.width,
    )
}

/// Π(S) sampled (§3.1): the doors of every edge whose outward normal is in the alphabet, `portStep` apart inside [ι, |e| − ι], whose exit band clears the room.
pub(super) fn ports_of(room: usize, s: &Shape, pr: &Params) -> Vec<Port> {
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
pub(super) fn slide(port: &Port, s: &Shape, at: V, k: i32, pr: &Params) -> Option<Port> {
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
