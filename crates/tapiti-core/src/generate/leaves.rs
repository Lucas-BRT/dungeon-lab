//! §7.6 Leaf chambers: small rooms in the dropped cells, each hung by one corridor off a room nearby.

use super::*;

/// Fills the spots (boxes of free space: the dropped grid cells, or the room beyond a free door of a grown layout) with leaf chambers,
/// then adds the side branches the topology profile asks for.
#[allow(clippy::too_many_arguments)]
pub(super) fn add_leaf_chambers(
    g: &mut Gen<'_>,
    spot_boxes: &[Bx],
    main_path: &[usize],
    main_path_rooms: &[usize],
    room_index: &mut SpatialHash,
    progress: &mut Option<&mut dyn FnMut(GenerationEvent)>,
) {
    let p = g.p;
    let topology_req = p.topology.requirements_with(p.entrance && p.exit_room);
    let (cell_w, cell_h, gap) = (p.cell_w, p.cell_h, p.gap);
    let leaf_weights: Vec<f64> = CLASSES
        .iter()
        .map(|c| p.leaf_classes.get(*c).max(0.0))
        .collect();
    let mut spots: Vec<usize> = (0..spot_boxes.len()).collect();
    for i in (1..spots.len()).rev() {
        let j = (g.rng.next() * (i + 1) as f64).floor() as usize;
        spots.swap(i, j);
    }
    let (existing_branches, existing_branch_depth) =
        crate::topology::side_branch_stats(&g.nodes, &g.edges, main_path);
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
        let bx = spot_boxes[bi];
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
            role: Role::Plain,
        };
        let center = room.center;
        g.rooms.push(room);
        g.nodes.push(Node::Room { room: id });
        g.node_of_room.push(g.nodes.len() - 1);
        let create_branch = branches_added < missing_branches || need_depth_branch;
        let extend_depth =
            !create_branch && depth_branch.is_some_and(|i| branch_tips[i].1 < depth_goal);
        let preferred_hosts: Vec<usize> = if create_branch {
            main_path_rooms.to_vec()
        } else if extend_depth {
            vec![branch_tips[depth_branch.unwrap()].0]
        } else if p.topology.profile == TopologyProfile::Backbone {
            main_path_rooms.to_vec()
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
            candidates_for_indexed(p, &g.rooms, id, *o, 2, room_index)
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
        report_progress(progress, || GenerationEvent::EdgeUpdated {
            id: edge_id,
            pts: g.edges[edge_id].pts.clone(),
        });
        report_progress(progress, || {
            GenerationEvent::RoomAdded(g.rooms[id].shape.clone())
        });
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
}
