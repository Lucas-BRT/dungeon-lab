//! The validator of the Tapiti model (§5.2, §9): V1 to V5 checked from the stage alone, without trusting the generator. Returns the
//! violations found (an empty list: valid). V6 (rocks and crystals) belongs to the furniture, not drawn here.

use crate::generate::*;
use crate::geom::*;
use crate::spatial::SpatialHash;
use std::collections::{HashMap, HashSet};

pub fn validate(d: &Floor) -> Vec<String> {
    let p = &d.params;
    let mut out: Vec<String> = Vec::new();
    let rooms: Vec<&Room> = d.rooms.iter().filter(|r| r.alive).collect();
    let spatial_cell = [p.cell_w, p.cell_h]
        .into_iter()
        .filter(|v| v.is_finite())
        .map(f64::abs)
        .fold(1.0, f64::max);
    let room_spatial = if rooms.len() >= 128 {
        let mut index = SpatialHash::new(spatial_cell);
        for (i, r) in rooms.iter().enumerate() {
            index.insert_bounds(i, r.bounds.lo, r.bounds.hi);
        }
        Some(index)
    } else {
        None
    };
    let mut room_node: HashMap<usize, usize> = HashMap::new();
    for (i, n) in d.nodes.iter().enumerate() {
        if let Node::Room { room } = n {
            room_node.insert(*room, i);
        }
    }
    let eps = 1e-6;

    // V1: every room reached from the start.
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); d.nodes.len()];
    for e in &d.edges {
        adj[e.u].push(e.v);
        adj[e.v].push(e.u);
    }
    let s0 = room_node[&d.origin()];
    let mut seen: HashSet<usize> = HashSet::from([s0]);
    let mut todo = vec![s0];
    let mut i = 0;
    while i < todo.len() {
        for &v in &adj[todo[i]] {
            if seen.insert(v) {
                todo.push(v);
            }
        }
        i += 1;
    }
    for r in &rooms {
        if !seen.contains(&room_node[&r.id]) {
            out.push(format!("V1 room {} not reached from the start", r.id));
        }
    }

    // Corridor groups: the edges joined through junctions.
    let mut group: Vec<usize> = (0..d.edges.len()).collect();
    fn find(group: &mut Vec<usize>, i: usize) -> usize {
        if group[i] == i {
            i
        } else {
            let r = find(group, group[i]);
            group[i] = r;
            r
        }
    }
    let mut at_junction: HashMap<usize, Vec<usize>> = HashMap::new();
    for e in &d.edges {
        for n in [e.u, e.v] {
            if matches!(d.nodes[n], Node::Junction { .. }) {
                at_junction.entry(n).or_default().push(e.id);
            }
        }
    }
    let mut keys: Vec<&usize> = at_junction.keys().collect();
    keys.sort();
    for k in keys {
        let ids = &at_junction[k];
        for id in ids.iter().skip(1) {
            let (a, b) = (find(&mut group, *id), find(&mut group, ids[0]));
            group[a] = b;
        }
    }
    let mut groups: Vec<(usize, Vec<&Edge>)> = Vec::new();
    let mut group_index: HashMap<usize, usize> = HashMap::new();
    for e in &d.edges {
        let g = find(&mut group, e.id);
        if let Some(i) = group_index.get(&g).copied() {
            groups[i].1.push(e);
        } else {
            group_index.insert(g, groups.len());
            groups.push((g, vec![e]));
        }
    }

    // V2 rooms: 2g apart.
    let room_clearance = 2.0 * p.gap - 0.5;
    for i in 0..rooms.len() {
        let candidates = room_spatial.as_ref().map_or_else(
            || ((i + 1)..rooms.len()).collect(),
            |index| {
                index.query_bounds(
                    rooms[i].bounds.lo,
                    rooms[i].bounds.hi,
                    room_clearance.max(0.0),
                )
            },
        );
        for j in candidates.into_iter().filter(|j| *j > i) {
            if shape_dist(&rooms[i].shape, &rooms[j].shape) < room_clearance {
                out.push(format!(
                    "V2 rooms {} and {} closer than 2g",
                    rooms[i].id, rooms[j].id
                ));
            }
        }
    }

    // V2 and V4 per corridor group.
    for (g, es) in &groups {
        let mut ends: Vec<(usize, usize)> = Vec::new();
        for e in es {
            for n in [e.u, e.v] {
                if let Node::Room { room } = d.nodes[n] {
                    if !ends.contains(&(n, room)) {
                        ends.push((n, room));
                    }
                }
            }
        }
        if ends.len() < 2 {
            out.push(format!(
                "V4 corridor {} reaches {} room(s): every corridor joins rooms",
                g,
                ends.len()
            ));
        }
        let mut group_adj: HashMap<usize, Vec<(usize, f64)>> = HashMap::new();
        for e in es {
            let length = polyline_length(&e.pts);
            group_adj.entry(e.u).or_default().push((e.v, length));
            group_adj.entry(e.v).or_default().push((e.u, length));
        }
        for &(n0, r0) in &ends {
            let mut best: HashMap<usize, f64> = HashMap::from([(n0, 0.0)]);
            let mut open: Vec<usize> = vec![n0];
            while !open.is_empty() {
                let k = (0..open.len())
                    .min_by(|a, b| best[&open[*a]].partial_cmp(&best[&open[*b]]).unwrap())
                    .unwrap();
                let u = open.swap_remove(k);
                for &(v, length) in group_adj.get(&u).into_iter().flatten() {
                    let w = best[&u] + length;
                    if w < *best.get(&v).unwrap_or(&f64::INFINITY) {
                        best.insert(v, w);
                        // Walks don't go through rooms: a room ends a corridor.
                        if matches!(d.nodes[v], Node::Junction { .. }) {
                            open.push(v);
                        }
                    }
                }
            }
            for &(n1, r1) in &ends {
                if r1 <= r0 {
                    continue;
                }
                let walk = *best.get(&n1).unwrap_or(&f64::INFINITY);
                if walk < p.min_len - eps {
                    out.push(format!(
                        "V4 rooms {} and {}: corridor {:.0} px < ℓmin",
                        r0, r1, walk
                    ));
                }
                if shape_dist(&d.rooms[r0].shape, &d.rooms[r1].shape) < p.min_len - eps {
                    out.push(format!(
                        "V2 rooms {} and {} joined but closer than ℓmin in a straight line",
                        r0, r1
                    ));
                }
            }
        }
    }
    let trim_for = |e: &Edge, junction_trim: f64| {
        let t0 = e.ends[0].map_or(junction_trim, |q| 1.0 + q.embed);
        let t1 = e.ends[1].map_or(junction_trim, |q| 1.0 + q.embed);
        corridor_region(&trim_ends(&e.pts, t0, t1), p.width)
    };
    for e in &d.edges {
        let mine: HashSet<usize> = e.ends.iter().flatten().map(|q| q.room).collect();
        let outside = trim_for(e, 0.0);
        let mut candidate_rooms = room_spatial.as_ref().map_or_else(
            || (0..rooms.len()).collect(),
            |index| index.query_region(&e.region, p.clear),
        );
        if room_spatial.is_some() {
            candidate_rooms.extend(
                rooms
                    .iter()
                    .enumerate()
                    .filter_map(|(i, r)| mine.contains(&r.id).then_some(i)),
            );
        }
        candidate_rooms.sort_unstable();
        candidate_rooms.dedup();
        for ri in candidate_rooms {
            let r = rooms[ri];
            if mine.contains(&r.id) {
                if region_shape_dist(&outside, &r.shape, r.bounds) <= 0.0 {
                    out.push(format!(
                        "V2 corridor {} runs into room {} past its door",
                        e.id, r.id
                    ));
                }
            } else if region_shape_dist(&e.region, &r.shape, r.bounds) < p.clear - eps {
                out.push(format!(
                    "V2 corridor {} closer than σ to room {}",
                    e.id, r.id
                ));
            }
        }
    }
    // V2 corridors against corridors: σ apart, except where they meet at a junction (cut out round it).
    let edge_spatial = if d.edges.len() >= 128 {
        let mut index = SpatialHash::new(spatial_cell);
        for (i, e) in d.edges.iter().enumerate() {
            index.insert_region(i, &e.region);
        }
        Some(index)
    } else {
        None
    };
    for i in 0..d.edges.len() {
        let candidates = edge_spatial.as_ref().map_or_else(
            || ((i + 1)..d.edges.len()).collect(),
            |index| index.query_region(&d.edges[i].region, p.clear),
        );
        for j in candidates.into_iter().filter(|j| *j > i) {
            let (a, b) = (&d.edges[i], &d.edges[j]);
            let shared = [a.u, a.v]
                .iter()
                .any(|n| (*n == b.u || *n == b.v) && matches!(d.nodes[*n], Node::Junction { .. }));
            let cut = p.width + p.clear;
            // An arm shorter than the cuts at its junction ends lies wholly inside them: nothing of it is left to measure (V2 exempts the junction).
            let inside_cut = |e: &Edge| {
                let ends = e.ends.iter().filter(|q| q.is_none()).count() as f64;
                polyline_length(&e.pts) <= ends * cut
            };
            let (ra, rb) = if shared {
                (
                    if inside_cut(a) {
                        Vec::new()
                    } else {
                        trim_for(a, cut)
                    },
                    if inside_cut(b) {
                        Vec::new()
                    } else {
                        trim_for(b, cut)
                    },
                )
            } else {
                (a.region.clone(), b.region.clone())
            };
            if region_dist(&ra, &rb) < p.clear - eps {
                out.push(format!("V2 corridors {} and {} closer than σ", a.id, b.id));
            }
        }
    }

    // V3 doors: admissible and separated.
    let exit = |q: &Port| band(q.p, add(q.p, mul(dir_of(q.k), p.exit)), p.width);
    for r in &rooms {
        for q in &r.used {
            if p.m == 4 && q.k % 2 == 1 {
                out.push(format!(
                    "V3 room {}: a door at 45° in a retilinear stage",
                    r.id
                ));
            }
            let from_wall = band(
                add(q.p, mul(dir_of(q.k), 0.5)),
                add(q.p, mul(dir_of(q.k), p.exit)),
                p.width,
            );
            if shape_dist(&from_wall, &r.shape) <= 0.0 {
                out.push(format!(
                    "V3 room {}: a door's exit band runs back into the room",
                    r.id
                ));
            }
            if let Shape::Disc { c, r: rad } = &r.shape {
                if (dist(q.p, *c) - rad).abs() > 0.01 {
                    out.push(format!("V3 room {}: a door off the circle", r.id));
                }
            }
        }
        for i in 0..r.used.len() {
            for j in (i + 1)..r.used.len() {
                if shape_dist(&exit(&r.used[i]), &exit(&r.used[j])) < p.clear - eps {
                    out.push(format!(
                        "V3 room {}: doors {} and {} closer than σ",
                        r.id, i, j
                    ));
                }
            }
        }
    }

    // V4 per edge: directions in the alphabet, turns, segment lengths.
    for e in &d.edges {
        let dirs: Vec<i32> = e.pts.windows(2).map(|w| seg_dir(w[0], w[1])).collect();
        if dirs.iter().any(|k| *k < 0 || (p.m == 4 && k % 2 == 1)) {
            out.push(format!("V4 corridor {}: a segment off the alphabet", e.id));
        }
        for w in dirs.windows(2) {
            let t = turn_of(w[0], w[1]);
            if t > 2 || (t == 1 && p.m == 4) {
                out.push(format!("V4 corridor {}: a {}° turn", e.id, t * 45));
            }
        }
        let n = e.pts.len();
        for (i, w) in e.pts.windows(2).enumerate() {
            let l = dist(w[0], w[1]);
            let at_door = (i == 0 && e.ends[0].is_some()) || (i == n - 2 && e.ends[1].is_some());
            if l < (if at_door { p.exit } else { p.seg }) - eps {
                out.push(format!("V4 corridor {}: a segment of {:.0} px", e.id, l));
            }
        }
        for (i, q) in e.ends.iter().enumerate() {
            if let Some(q) = q {
                let k = if i == 0 {
                    dirs[0]
                } else {
                    (dirs[dirs.len() - 1] + 4) % 8
                };
                if k != q.k {
                    out.push(format!("V4 corridor {}: leaves a door at an angle", e.id));
                }
            }
        }
    }

    // V5 the entrance and the exit, when the Λ asks for them.
    if p.entrance {
        let entrance = d.entrance.and_then(|i| d.rooms.get(i)).filter(|r| r.alive);
        if entrance.is_none() {
            out.push("V5 the stage has no entrance room".to_string());
        } else if p.exit_room {
            let exit = d.exit.and_then(|i| d.rooms.get(i)).filter(|r| r.alive);
            match (d.entrance, d.exit, exit) {
                (Some(e), Some(x), Some(t)) if e != x => {
                    if t.used.len() > p.exit_doors as usize {
                        out.push(format!(
                            "V5 the exit has {} doors (most {})",
                            t.used.len(),
                            p.exit_doors
                        ));
                    }
                    let h = room_hops(&d.nodes, &d.edges, room_node[&e]);
                    let hops = h[room_node[&x]];
                    if hops < p.exit_min_hops as f64 {
                        out.push(format!(
                            "V5 the exit is {} hops from the entrance (least {})",
                            hops, p.exit_min_hops
                        ));
                    }
                }
                _ => out.push(
                    "V5 the entrance and the exit must be two rooms of the stage".to_string(),
                ),
            }
        }
    }

    let req = p.topology.requirements_with(p.entrance && p.exit_room);
    if req.min_branches > 0 || req.min_branch_depth > 0 {
        let (branches, branch_depth) =
            crate::topology::side_branch_stats(&d.nodes, &d.edges, &d.main_path);
        if branches < req.min_branches {
            out.push(format!(
                "T1 profile requires {} side branches; found {}",
                req.min_branches, branches
            ));
        }
        if branch_depth < req.min_branch_depth {
            out.push(format!(
                "T2 profile requires branch depth {}; found {}",
                req.min_branch_depth, branch_depth
            ));
        }
    }
    if req.min_exit_routes > 1 {
        let routes = crate::topology::edge_connectivity_frontiers(
            d.nodes.len(),
            &d.edges,
            room_node[&d.entrance.unwrap_or(0)],
            room_node[&d.exit.unwrap_or(0)],
        )
        .0;
        if routes < req.min_exit_routes {
            out.push(format!(
                "T3 profile requires {} independent routes to the exit; found {}",
                req.min_exit_routes, routes
            ));
        }
    }
    if req.min_hub_degree > 0 {
        let degree = crate::topology::max_degree(d.nodes.len(), &d.edges);
        if degree < req.min_hub_degree {
            out.push(format!(
                "T4 profile requires a hub degree of {}; found {}",
                req.min_hub_degree, degree
            ));
        }
    }
    if req.min_cycle_length > 0 {
        let girth = crate::topology::cycle_girth(d.nodes.len(), &d.edges).unwrap_or(0);
        if girth < req.min_cycle_length {
            out.push(format!(
                "T5 profile requires a cycle of at least {} branches; found {}",
                req.min_cycle_length, girth
            ));
        }
    }

    out
}
