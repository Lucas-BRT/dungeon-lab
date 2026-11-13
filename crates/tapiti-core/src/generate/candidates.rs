//! Corridor candidates and the options the search chooses from.

use super::*;

#[derive(Clone, Copy)]
pub(super) struct Block {
    pub(super) c: i32,
    pub(super) r: i32,
    pub(super) w: i32,
    pub(super) h: i32,
}

#[derive(Clone, Copy)]
pub(super) enum Action {
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
pub(super) struct SegRef {
    pub(super) edge: usize,
    pub(super) seg: usize,
    pub(super) a: V,
    pub(super) ds: V,
    pub(super) sl: f64,
}

pub(super) type Axes = [Vec<SegRef>; 4];

#[derive(Clone, Copy)]
pub(super) struct Opt {
    pub(super) weight: f64,
    pub(super) act: Action,
}

pub(super) struct Changes {
    pub(super) edges: Vec<usize>,
    pub(super) rooms: Vec<usize>,
}

pub(super) fn box_gap(a: &Bounded, b: &Bounded) -> f64 {
    let dx = (a.lo[0] - b.hi[0]).max(b.lo[0] - a.hi[0]).max(0.0);
    let dy = (a.lo[1] - b.hi[1]).max(b.lo[1] - a.hi[1]).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

pub(super) fn box_gap_raw(a_lo: V, a_hi: V, b_lo: V, b_hi: V) -> f64 {
    let dx = (a_lo[0] - b_hi[0]).max(b_lo[0] - a_hi[0]).max(0.0);
    let dy = (a_lo[1] - b_hi[1]).max(b_lo[1] - a_hi[1]).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

/// A cheap conservative test: false guarantees the two regions cannot be within `limit`.
pub(super) fn regions_may_be_within(a: &[Bounded], b: &[Bounded], limit: f64) -> bool {
    a.iter().any(|x| b.iter().any(|y| box_gap(x, y) <= limit))
}

pub(super) fn mix_of(p: &Params, rooms: &[Room], a: usize, b: usize) -> f64 {
    let (x, y) = (rooms[a].cls, rooms[b].cls);
    let (x, y) = if x.index() <= y.index() {
        (x, y)
    } else {
        (y, x)
    };
    // The key is at most "rect-rect" (9 bytes): built on the stack, no String per lookup.
    let mut buf = [0u8; 16];
    let (nx, ny) = (x.name().as_bytes(), y.name().as_bytes());
    buf[..nx.len()].copy_from_slice(nx);
    buf[nx.len()] = b'-';
    buf[nx.len() + 1..nx.len() + 1 + ny.len()].copy_from_slice(ny);
    let key = std::str::from_utf8(&buf[..nx.len() + 1 + ny.len()]).unwrap();
    *p.mix.get(key).unwrap_or(&1.0)
}

pub(super) fn type_weight(p: &Params, t: RouteType) -> f64 {
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

pub(super) fn candidates_for_indexed(
    pr: &Params,
    rooms: &[Room],
    a: usize,
    b: usize,
    keep: usize,
    room_index: &SpatialHash,
) -> Vec<Candidate> {
    candidates_for_with_index(pr, rooms, a, b, keep, Some(room_index))
}

pub(super) fn candidates_for_with_index(
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
