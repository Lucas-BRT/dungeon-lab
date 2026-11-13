//! §7.5.1 The spanning tree: Prim from the start, weighted by ω; the proximity graph relaxed when it gets stuck.

use super::*;

/// Joins every room reachable from `start_room` to the floor with corridors, one cheapest valid corridor at a time.
/// Rooms no corridor reaches are switched off (`alive = false`).
pub(super) fn span_tree(
    g: &mut Gen<'_>,
    order: &[Proximity],
    start_room: usize,
    room_index: &SpatialHash,
    progress: &mut Option<&mut dyn FnMut(GenerationEvent)>,
    lap: &mut dyn FnMut(&str),
) {
    let p = g.p;
    let (cell_w, cell_h) = (p.cell_w, p.cell_h);
    let mut level = 0;
    let mut near = p.near.max(0.0);
    let mut pairs = g.proximity_pairs(order[0], near);
    let mut in_tree: HashSet<usize> = HashSet::from([start_room]);
    if p.layout == Layout::Growth {
        // The growth already joined every room to its parent: the tree phase has nothing left to do.
        in_tree.extend(0..g.rooms.len());
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
    // The candidate graph is built in one batch from the proximity pairs.
    g.ensure(&pairs, room_index);
    lap("candidates");
    while in_tree.len() < g.rooms.len() {
        if g.stopped() {
            break;
        }
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
            let edge = g.add_edge(
                g.node_of_room[c.a],
                g.node_of_room[c.b],
                c.pts,
                [Some(c.ports[0]), Some(c.ports[1])],
            );
            report_progress(progress, || GenerationEvent::EdgeUpdated {
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
            #[cfg(feature = "diagnostic-profile")]
            {
                tree_diagnostics.edge_update += update_started.elapsed();
            }
            continue;
        }
        if level < order.len() - 1 {
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
            g.ensure(&pairs, room_index);
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
            g.ensure(&pairs, room_index);
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
}
