//! The generation entry points.

use super::*;

pub fn generate(seed: u32, p: &Params) -> Floor {
    generate_inner(seed, p, None)
}

/// Generate one floor and report drawable geometry as it becomes available.
pub fn generate_with_progress(
    seed: u32,
    p: &Params,
    progress: &mut dyn FnMut(GenerationEvent),
) -> Floor {
    generate_inner(seed, p, Some(progress))
}

pub(super) fn report_progress(
    progress: &mut Option<&mut dyn FnMut(GenerationEvent)>,
    event: impl FnOnce() -> GenerationEvent,
) {
    if let Some(callback) = progress.as_mut() {
        (**callback)(event());
    }
}

pub(super) fn generate_inner(
    seed: u32,
    p: &Params,
    mut progress: Option<&mut dyn FnMut(GenerationEvent)>,
) -> Floor {
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
    let (cols, rows, gap) = (p.cols.max(1), p.rows.max(1), p.gap);
    let Scaffold {
        blocks,
        alive,
        grid_x,
        grid_y,
        neighbors,
    } = build_scaffold(&mut g, &mut progress);
    let box_of = |b: &Block| Bx {
        x0: grid_x[b.c as usize] + gap,
        y0: grid_y[b.r as usize] + gap,
        x1: grid_x[(b.c + b.w) as usize] - gap,
        y1: grid_y[(b.r + b.h) as usize] - gap,
    };
    let live: Vec<usize> = (0..blocks.len()).filter(|i| alive[*i]).collect();
    let dropped: Vec<usize> = (0..blocks.len())
        .filter(|i| !alive[*i] && p.layout != Layout::Growth)
        .collect();

    let world = Bx {
        x0: grid_x[0],
        y0: grid_y[0],
        x1: grid_x[cols as usize],
        y1: grid_y[rows as usize],
    };
    let island_of = build_rooms(
        &mut g,
        &blocks,
        &live,
        &alive,
        &neighbors,
        &box_of,
        world,
        &mut progress,
    );
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
    let start_room = if p.layout == Layout::Growth {
        0
    } else {
        (g.rng.next() * g.rooms.len() as f64).floor() as usize
    };
    let order: Vec<Proximity> = match p.proximity {
        Proximity::Rng => vec![Proximity::Rng, Proximity::Gabriel, Proximity::Near],
        Proximity::Gabriel => vec![Proximity::Gabriel, Proximity::Near],
        Proximity::Near => vec![Proximity::Near],
    };
    let near_base = p.near.max(0.0);
    span_tree(
        &mut g,
        &order,
        start_room,
        &room_index,
        &mut progress,
        &mut lap,
    );

    let mut stairs_room = assign_start_and_stairs(&mut g, start_room);

    let (beta_max, all_pairs) =
        measure_beta_max(&mut g, &order, near_base, &island_of, &room_index);
    lap("β_max");
    let topology_req = p.topology.requirements();
    let beta_target = (p.delta * beta_max as f64).round() as i32;

    add_cycles(&mut g, start_room, stairs_room, beta_target, &mut progress);

    lap("cycles");
    if p.layout == Layout::Growth && topology_req.min_exit_routes <= 1 {
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
    let (main_path, _) = crate::topology::shortest_path(
        g.nodes.len(),
        &g.edges,
        g.node_of_room[start_room],
        g.node_of_room[stairs_room],
    )
    .unwrap_or_default();
    let main_path_rooms = path_rooms(&g.nodes, &main_path);
    // The spots for leaf chambers: the grid's dropped cells; a grown layout drops none, so they are the boxes of one cell
    // beyond a free door of a room (two reaches each), inside the world and clear of every room (`add_leaf_chambers`
    // still refuses what overlaps a room or a corridor). At most 160 of them, spread over the list.
    let spot_boxes: Vec<Bx> = if p.layout == Layout::Growth {
        let (cw, ch) = (p.cell_w.max(1.0), p.cell_h.max(1.0));
        let (bw, bh) = (cw - 2.0 * p.gap, ch - 2.0 * p.gap);
        let mut all: Vec<Bx> = Vec::new();
        for room in g.rooms.iter().filter(|r| r.alive && !r.leaf) {
            for port in room.ports.iter().filter(|q| g.port_free(q)) {
                let u = dir_of(port.k);
                for extra in [0.0, 0.5 * cw] {
                    let half = (bw * u[0].abs() + bh * u[1].abs()) / 2.0;
                    let reach = p.min_len + extra + half;
                    let (cx, cy) = (port.p[0] + u[0] * reach, port.p[1] + u[1] * reach);
                    let b = Bx {
                        x0: cx - bw / 2.0,
                        y0: cy - bh / 2.0,
                        x1: cx + bw / 2.0,
                        y1: cy + bh / 2.0,
                    };
                    if b.x0 < world.x0 || b.y0 < world.y0 || b.x1 > world.x1 || b.y1 > world.y1 {
                        continue;
                    }
                    let clear = room_index
                        .query_bounds([b.x0, b.y0], [b.x1, b.y1], 2.0 * p.gap)
                        .into_iter()
                        .all(|o| {
                            let rb = g.rooms[o].bounds;
                            box_gap_raw(rb.lo, rb.hi, [b.x0, b.y0], [b.x1, b.y1]) >= 2.0 * p.gap
                        });
                    if clear {
                        all.push(b);
                    }
                }
            }
        }
        let stride = all.len().div_ceil(160).max(1);
        all.into_iter().step_by(stride).collect()
    } else {
        dropped.iter().map(|i| box_of(&blocks[*i])).collect()
    };
    add_leaf_chambers(
        &mut g,
        &spot_boxes,
        &main_path,
        &main_path_rooms,
        &mut room_index,
        &mut progress,
    );

    /* §7.7 (part) Ambush rooms: the biggest plain rooms. */
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
    Floor {
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
