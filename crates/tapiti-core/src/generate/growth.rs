//! Growth layout: rooms grown one from another.

use super::*;

/// A grown layout: from a first room, each room spawns 1..=`branch` children off its doors down to generation `depth`,
/// every child placed in free space and joined by a corridor on the spot, until `target` rooms stand. Replaces the grid rooms and the tree phase.
pub(super) fn grow_rooms(
    g: &mut Gen<'_>,
    world: Bx,
    target: usize,
    sizes: &[(i32, i32)],
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
    // A box the size of one of the grid's blocks, drawn at random (1x1 when the grid has none), so the rooms take as much room as the grid's.
    let pick_box = |rng: &mut Mulberry| -> (f64, f64, i32) {
        let (kw, kh) = if sizes.is_empty() {
            (1, 1)
        } else {
            sizes[(rng.next() * sizes.len() as f64).floor() as usize % sizes.len()]
        };
        (
            kw as f64 * cw - 2.0 * p.gap,
            kh as f64 * ch - 2.0 * p.gap,
            kw * kh,
        )
    };
    let fit_room = |g: &mut Gen<'_>, bx: Bx, cells: i32| -> Option<(ShapeClass, Shape)> {
        let ws: Vec<f64> = CLASSES.iter().map(|c| weight_of(*c, cells)).collect();
        let mut c = CLASSES[g.pick_class(&ws)];
        let shape = match fit(c, bx, &mut g.rng, p, cells) {
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
        }
    };

    // The first room, in the middle of the world or against the border `growth.start` names (a `gap` in from it).
    let (w0, h0, cells0) = pick_box(&mut g.rng);
    let (mut mx, mut my) = ((world.x0 + world.x1) / 2.0, (world.y0 + world.y1) / 2.0);
    match p.growth.start {
        GrowthStart::Center => {}
        GrowthStart::North => my = world.y0 + p.gap + h0 / 2.0,
        GrowthStart::South => my = world.y1 - p.gap - h0 / 2.0,
        GrowthStart::West => mx = world.x0 + p.gap + w0 / 2.0,
        GrowthStart::East => mx = world.x1 - p.gap - w0 / 2.0,
    }
    let bx0 = Bx {
        x0: mx - w0 / 2.0,
        y0: my - h0 / 2.0,
        x1: mx + w0 / 2.0,
        y1: my + h0 / 2.0,
    };
    let Some((c0, s0)) = fit_room(g, bx0, cells0) else {
        return Vec::new();
    };
    let r0 = make_room(0, c0, s0, &bx0);
    g.room_grid.insert_bounds(0, r0.bounds.lo, r0.bounds.hi);
    report_progress(progress, || GenerationEvent::RoomAdded(r0.shape.clone()));
    g.rooms.push(r0);
    g.nodes.push(Node::Room { room: 0 });
    g.node_of_room.push(0);
    let mut depth = vec![0i32];
    // Failed rounds of each parent: it leaves the pool after three, not after one.
    let mut strikes = vec![0u8];
    let mut island_of = vec![0usize];
    // Rooms within 2.5 cells of each room: the local density the clump bias works on.
    let near_radius = 2.5 * cell;
    let mut near: Vec<f64> = vec![0.0];
    let mut open: Vec<usize> = vec![0];
    let max_depth = p.growth.depth.max(1);
    let branch = p.growth.branch.max(1);
    let mut budget = target.saturating_mul(60).max(200);
    // Diagnostics only (they never touch the RNG): why proposals were refused and why the loop ended.
    // [out of the world, no shape fits, crowded, on a corridor, outside the clump radius, no corridor to the parent]
    let mut refused = [0u32; 6];
    let mut evicted = 0u32;
    let mut stop = "target";
    // When the pool empties short of the target, rooms that still have a free door come back (twice at most), in any clump.
    let (mut refills, mut refilling) = (0, false);
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
        if !need_bridge && open.is_empty() && island_idx + 1 >= islands && refills < 2 {
            refills += 1;
            refilling = true;
            strikes.iter_mut().for_each(|x| *x = 0);
            open = (0..g.rooms.len())
                .filter(|&i| {
                    depth[i] < max_depth && g.rooms[i].ports.iter().any(|q| g.port_free(q))
                })
                .collect();
        }
        if !need_bridge && (open.is_empty() || (island_idx + 1 < islands && island_size >= quota)) {
            if island_idx + 1 < islands {
                need_bridge = true;
                bridge_tries = 0;
            } else {
                stop = "open-empty";
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
                stop = "no-bridge-port";
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
                stop = "weighted-none";
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
                // Only doors with room for a corridor and the smallest room before the edge of the world.
                let min_box = (cw.min(ch) - 2.0 * p.gap).max(1.0);
                let free: Vec<Port> = free
                    .into_iter()
                    .filter(|q| {
                        let u = dir_of(q.k);
                        let to = |pos: f64, lo: f64, hi: f64, d: f64| {
                            if d > 0.0 {
                                (hi - pos) / d
                            } else if d < 0.0 {
                                (lo - pos) / d
                            } else {
                                f64::INFINITY
                            }
                        };
                        let ahead = to(q.p[0], world.x0, world.x1, u[0])
                            .min(to(q.p[1], world.y0, world.y1, u[1]));
                        ahead >= p.min_len + min_box
                    })
                    .collect();
                if free.is_empty() {
                    refused[0] += 1;
                    continue;
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
                        refused[0] += 1;
                        continue;
                    }
                    let Some((c, shape)) = fit_room(g, bx, cells) else {
                        refused[1] += 1;
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
                    let outside = !bridge
                        && !refilling
                        && islands > 1
                        && island_idx + 1 < islands
                        && dist(room.center, root_c) > radius;
                    if crowded || on_corridor || outside {
                        refused[if crowded {
                            2
                        } else if on_corridor {
                            3
                        } else {
                            4
                        }] += 1;
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
                // The proposals are tried in weight order until one has a corridor to the parent.
                let mut chosen: Option<(usize, Candidate)> = None;
                while !proposals.is_empty() {
                    let ws: Vec<f64> = proposals.iter().map(|(_, w)| *w).collect();
                    let Some(k) = g.weighted(&ws) else {
                        break;
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
                        refused[5] += 1;
                        continue;
                    };
                    chosen = Some((id, found.into_iter().nth(k).unwrap()));
                    break;
                }
                let Some((id, c)) = chosen else {
                    continue;
                };
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
                strikes.push(0);
                island_of.push(if bridge { island_idx + 1 } else { island_idx });
                let centre = g.rooms[id].center;
                let mut count = 0.0;
                for (room, slot) in g.rooms.iter().zip(near.iter_mut()).take(id) {
                    let d = dist(room.center, centre);
                    if d <= near_radius {
                        *slot += 1.0;
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
        if !bridge && failed {
            strikes[parent] = strikes[parent].saturating_add(1);
        }
        if !bridge && (strikes[parent] >= 3 || depth[parent] >= max_depth) {
            evicted += u32::from(strikes[parent] >= 3);
            open.swap_remove(k);
        }
    }
    if g.rooms.len() < target {
        g.log.push(format!(
            "growth stopped at {} of {} rooms",
            g.rooms.len(),
            target
        ));
        if budget == 0 {
            stop = "budget";
        }
        let free_ports = (0..g.rooms.len())
            .filter(|&i| g.rooms[i].ports.iter().any(|q| g.port_free(q)))
            .count();
        let area: f64 = g.rooms.iter().map(|r| r.area).sum();
        let world_area = ((world.x1 - world.x0) * (world.y1 - world.y0)).max(1.0);
        g.log.push(format!(
            "growth stop: reason={stop} refused world={} fit={} crowded={} corridor={} radius={} no-link={} evicted={evicted} free-port-rooms={free_ports} max-depth={} area={:.0}%",
            refused[0],
            refused[1],
            refused[2],
            refused[3],
            refused[4],
            refused[5],
            depth.iter().max().copied().unwrap_or(0),
            100.0 * area / world_area
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
