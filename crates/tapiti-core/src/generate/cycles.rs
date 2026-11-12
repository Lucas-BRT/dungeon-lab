//! §7.5.3 Add cycles and profile-specific topology features.

use super::*;

/// Adds corridors that close cycles until β reaches `beta_target` and the topology profile's requirements hold, or nothing more fits.
pub(super) fn add_cycles(
    g: &mut Gen<'_>,
    anchors: Option<(usize, usize)>,
    beta_target: i32,
    progress: &mut Option<&mut dyn FnMut(GenerationEvent)>,
) {
    let p = g.p;
    let topology_req = p.topology.requirements_with(p.entrance && p.exit_room);
    let (cols, rows, cell_w, cell_h) = (p.cols.max(1), p.rows.max(1), p.cell_w, p.cell_h);
    loop {
        let beta = beta_now(g);
        let (routes, from_cut, to_cut) = if let (true, Some((start_room, exit_room))) =
            (topology_req.min_exit_routes > 1, anchors)
        {
            crate::topology::edge_connectivity_frontiers(
                g.nodes.len(),
                &g.edges,
                g.node_of_room[start_room],
                g.node_of_room[exit_room],
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
                    report_progress(progress, || GenerationEvent::EdgeUpdated {
                        id: edge,
                        pts: g.edges[edge].pts.clone(),
                    });
                }
                g.refresh_option_cache(&changes);
            }
            None => {
                let b = beta_now(g);
                if need_routes {
                    g.log.push(format!("topology profile: could not reach {} independent routes to the exit (λ = {})", topology_req.min_exit_routes, routes));
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
}
