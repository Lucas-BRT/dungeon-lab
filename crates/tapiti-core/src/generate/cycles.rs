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
    let topology_req = p.topology_requirements();
    let (cols, rows, cell_w, cell_h) = (p.cols.max(1), p.rows.max(1), p.cell_w, p.cell_h);
    loop {
        if g.stopped() {
            break;
        }
        let beta = beta_now(g);
        let (routes, _, _) = if let (true, Some((start_room, exit_room))) =
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
        if let (Layout::Growth, Some((start, exit))) = (p.layout, anchors) {
            keep_exit_far(g, &mut opts, start, exit);
        }
        // Every cycle must keep at least `min_cycle_length` edges (the validator checks the girth), so once the profile asks for long
        // loops no link that closes a shorter one is taken, even after a long loop exists.
        let long_only = topology_req.min_cycle_length > 0 && !need_hub;
        // While the exit is short of independent routes, a link is worth taking when it closes a cycle over a corridor that alone
        // keeps the entrance and the exit together: it bypasses that corridor, and the next links bypass the others.
        let bridges = match (need_routes, anchors) {
            (true, Some((start, exit))) => {
                route_bridges(g, g.node_of_room[start], g.node_of_room[exit])
            }
            _ => Vec::new(),
        };
        if need_routes || need_loop || long_only {
            opts.retain(|opt| {
                let Action::Add(ci) = opt.act else {
                    return false;
                };
                let c = &g.statics[ci];
                let a = g.node_of_room[c.a];
                let b = g.node_of_room[c.b];
                let bypasses_a_bridge = !need_routes
                    || crate::topology::shortest_path(g.nodes.len(), &g.edges, a, b)
                        .is_some_and(|(_, path)| path.iter().any(|e| bridges.contains(e)));
                let makes_long_cycle =
                    crate::topology::shortest_path(g.nodes.len(), &g.edges, a, b)
                        .is_some_and(|(_, path)| path.len() + 1 >= topology_req.min_cycle_length);
                bypasses_a_bridge && (!(need_loop || long_only) || makes_long_cycle)
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

/// Drops the actions that would bring the exit closer to the entrance than `exitMinHops` (growth only: a grown tree is shallow, so
/// the cycles shorten it more easily). An action that does not shorten the walk is kept.
fn keep_exit_far(g: &Gen<'_>, opts: &mut Vec<Opt>, start: usize, exit: usize) {
    let (s, t) = (g.node_of_room[start], g.node_of_room[exit]);
    let ds = room_hops(&g.nodes, &g.edges, s);
    let dt = room_hops(&g.nodes, &g.edges, t);
    let floor = (g.p.exit_min_hops as f64).min(ds[t]);
    let arrive = |n: usize| f64::from(g.room_of_node(n).is_some());
    // The shortest walk entrance to exit that goes from node `r` to node `x` over a new link, in room hops.
    let via = |r: usize, x: usize, extra: f64| (ds[r] + extra + dt[x]).min(dt[r] + extra + ds[x]);
    opts.retain(|o| {
        let shortest = match o.act {
            Action::Add(ci) => {
                let c = &g.statics[ci];
                via(g.node_of_room[c.a], g.node_of_room[c.b], 1.0)
            }
            Action::Branch { room, edge, .. } => {
                let r = g.node_of_room[room];
                let e = &g.edges[edge];
                via(r, e.u, arrive(e.u)).min(via(r, e.v, arrive(e.v)))
            }
            Action::Cross { cand, edge, .. } => {
                let c = &g.statics[cand];
                let (a, b) = (g.node_of_room[c.a], g.node_of_room[c.b]);
                let e = &g.edges[edge];
                [a, b]
                    .into_iter()
                    .flat_map(|r| [via(r, e.u, arrive(e.u)), via(r, e.v, arrive(e.v))])
                    .fold(via(a, b, 1.0), f64::min)
            }
        };
        shortest >= floor
    });
}

/// The corridors of the shortest walk from node `s` to node `t` whose removal would cut the two apart.
fn route_bridges(g: &Gen<'_>, s: usize, t: usize) -> Vec<usize> {
    let Some((_, path)) = crate::topology::shortest_path(g.nodes.len(), &g.edges, s, t) else {
        return Vec::new();
    };
    let mut adj: Vec<Vec<(usize, usize)>> = vec![Vec::new(); g.nodes.len()];
    for e in &g.edges {
        adj[e.u].push((e.v, e.id));
        adj[e.v].push((e.u, e.id));
    }
    path.into_iter()
        .filter(|&skip| {
            let mut seen = vec![false; g.nodes.len()];
            let mut todo = vec![s];
            seen[s] = true;
            while let Some(u) = todo.pop() {
                for &(v, id) in &adj[u] {
                    if id != skip && !seen[v] {
                        seen[v] = true;
                        todo.push(v);
                    }
                }
            }
            !seen[t]
        })
        .collect()
}
