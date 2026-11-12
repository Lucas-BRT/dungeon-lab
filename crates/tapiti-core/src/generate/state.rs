//! The generator state and the moves that change it.

use super::*;

#[cfg(feature = "diagnostic-profile")]
#[derive(Default)]
pub(super) struct TreeDiagnostics {
    pub(super) iterations: usize,
    pub(super) frontier_items_seen: usize,
    pub(super) max_frontier: usize,
    pub(super) fit_checks: usize,
    pub(super) open_candidates_seen: usize,
    pub(super) frontier_scan: Duration,
    pub(super) fit_time: Duration,
    pub(super) weighted_pick: Duration,
    pub(super) edge_update: Duration,
    pub(super) frontier_ensure: Duration,
    pub(super) graph_rebuild: Duration,
    pub(super) fallbacks: Vec<String>,
}

pub(super) struct Gen<'a> {
    pub(super) p: &'a Params,
    pub(super) rng: Mulberry,
    pub(super) log: Vec<String>,
    pub(super) rooms: Vec<Room>,
    pub(super) nodes: Vec<Node>,
    pub(super) node_of_room: Vec<usize>,
    pub(super) edges: Vec<Edge>,
    pub(super) cache: HashMap<(usize, usize), Vec<Candidate>>,
    pub(super) statics: Vec<Candidate>,
    /// Currently valid direct-link or X-crossing option for each static candidate.
    pub(super) static_opts: Vec<Option<Opt>>,
    /// Currently valid T branches, ordered like the full room/port/edge/segment scan.
    pub(super) branch_opts: BTreeMap<(usize, usize, usize, usize), Opt>,
    /// Static candidates incident to each room, for localized port invalidation.
    pub(super) candidates_by_room: Vec<Vec<usize>>,
    /// `statics[i]` is known not to fit: edges and doors in use only grow while the cycles are added, so a candidate that does not fit never will.
    pub(super) no_fit: Vec<AtomicBool>,
    /// `statics[i]` can never become a crossing: its doors are taken or it already touches two corridors (both only grow).
    pub(super) no_cross: Vec<AtomicBool>,
    pub(super) stairs_room: usize,
    /// Broad phase over the regions of `statics`, to find the candidates near a corridor that changed.
    pub(super) static_index: SpatialHash,
    /// Broad phase over the rooms and, kept up to date by `add_edge`/`split_edge`, over the corridor regions (a superset after a rollback: callers re-test exactly).
    pub(super) room_grid: SpatialHash,
    pub(super) edge_index: SpatialHash,
}

impl<'a> Gen<'a> {
    pub(super) fn pick_class(&mut self, ws: &[f64]) -> usize {
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

    pub(super) fn mix_of(&self, a: usize, b: usize) -> f64 {
        mix_of(self.p, &self.rooms, a, b)
    }

    pub(super) fn t_weight(&self) -> f64 {
        let baseline = if self.p.topology.requirements().min_hub_degree > 0 {
            0.15
        } else {
            0.0
        };
        self.p.p_t.max(baseline)
    }

    pub(super) fn x_weight(&self) -> f64 {
        let baseline = if self.p.topology.requirements().min_hub_degree >= 4 {
            0.12
        } else {
            0.0
        };
        self.p.p_x.max(baseline)
    }

    pub(super) fn proximity_pairs(&self, kind: Proximity, near: f64) -> Vec<[usize; 2]> {
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
    pub(super) fn ensure(&mut self, pairs: &[[usize; 2]], room_index: &SpatialHash) {
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

    pub(super) fn add_edge(
        &mut self,
        u: usize,
        v: usize,
        pts: Vec<V>,
        ends: [Option<Port>; 2],
    ) -> usize {
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
    pub(super) fn port_free(&self, p: &Port) -> bool {
        let pr = self.p;
        let mine = exit_band(p.p, p.k, pr, 0.0);
        self.rooms[p.room]
            .used
            .iter()
            .all(|q| !shape_within(&mine, &exit_band(q.p, q.k, pr, 0.0), pr.clear))
    }

    /// Whether a candidate fits what is already built (§7.4): free doors, and σ from every corridor.
    pub(super) fn fits_now(&self, c: &Candidate) -> bool {
        if !self.port_free(&c.ports[0]) || !self.port_free(&c.ports[1]) {
            return false;
        }
        self.edges
            .iter()
            .all(|e| !region_within(&c.region, &e.region, self.p.clear))
    }

    /// `fits_now` of `statics[ci]`, remembering a no for good.
    pub(super) fn fits_cached(&self, ci: usize) -> bool {
        if self.no_fit[ci].load(Ordering::Relaxed) {
            return false;
        }
        let ok = self.fits_now(&self.statics[ci]);
        if !ok {
            self.no_fit[ci].store(true, Ordering::Relaxed);
        }
        ok
    }

    pub(super) fn degree(&self, r: usize) -> usize {
        self.rooms[r].used.len()
    }

    pub(super) fn locked_full(&self, r: usize) -> bool {
        r == self.stairs_room && self.degree(r) >= self.p.stairs_doors as usize
    }

    pub(super) fn weighted(&mut self, ws: &[f64]) -> Option<usize> {
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
    pub(super) fn split_edge(&mut self, e: usize, seg: usize, h: V) -> usize {
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
    pub(super) fn axes(&self) -> Axes {
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
    pub(super) fn axes_for_edges(&self, edge_ids: &[usize]) -> Axes {
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

    pub(super) fn room_of_node(&self, n: usize) -> Option<usize> {
        match self.nodes[n] {
            Node::Room { room } => Some(room),
            Node::Junction { .. } => None,
        }
    }

    /// T (§4.3): a straight arm from a free door of a room to a perpendicular corridor segment, met away from its ends.
    pub(super) fn branches_of(
        &self,
        r: &Room,
        axes: &Axes,
        edge_filter: Option<&HashSet<usize>>,
    ) -> Vec<Opt> {
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
                                        && own.is_none_or(|q| q.room != ri)
                                        && region_shape_within(
                                            region,
                                            &o.shape,
                                            o.bounds,
                                            pr.clear - 1e-6,
                                        )
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
    pub(super) fn crossing(&self, ci: usize) -> Option<Action> {
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

    pub(super) fn static_option(&self, ci: usize) -> Option<Opt> {
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

    pub(super) fn branch_key(opt: &Opt) -> Option<(usize, usize, usize, usize)> {
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
    pub(super) fn rebuild_option_cache(&mut self) {
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

    pub(super) fn branch_arm_may_be_blocked(&self, port: &Port, h: V, region: &[Bounded]) -> bool {
        let pad = self.p.width / 2.0;
        let lo = [port.p[0].min(h[0]) - pad, port.p[1].min(h[1]) - pad];
        let hi = [port.p[0].max(h[0]) + pad, port.p[1].max(h[1]) + pad];
        region
            .iter()
            .any(|b| box_gap_raw(lo, hi, b.lo, b.hi) <= self.p.clear)
    }

    /// Recheck a cached T option against its target edge and the current whole floor.
    pub(super) fn branch_is_current(&self, opt: &Opt) -> bool {
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
    pub(super) fn refresh_option_cache(&mut self, changes: &Changes) {
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
    pub(super) fn options(&self) -> Vec<Opt> {
        let mut out: Vec<Opt> = self.static_opts.iter().filter_map(|opt| *opt).collect();
        out.extend(self.branch_opts.values().copied());
        out
    }

    pub(super) fn apply(&mut self, act: Action) -> Changes {
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
