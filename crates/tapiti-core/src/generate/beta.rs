//! §7.4 β_max: the densest the floor could get, found by greedily filling cycles on a copy.

use super::*;

/// Loads the cycle candidates into `g` and returns `(β_max, proximity pairs of the base graph)`.
/// The floor itself is left as it was: the fill is undone.
pub(super) fn measure_beta_max(
    g: &mut Gen<'_>,
    order: &[Proximity],
    near_base: f64,
    island_of: &[usize],
    room_index: &SpatialHash,
) -> (i32, Vec<[usize; 2]>) {
    let p = g.p;
    /* §7.4 β_max: greedily add the highest-weight valid cycle action on a copy.
     * This includes direct links, T branches and X crossings; an X contributes
     * two independent cycles, so counting only direct links understates the
     * density reference whenever junction actions are enabled.
     */
    // Cycle candidates come from the base proximity graph, not the radius the tree needed to reach a stray room.
    let mut all_pairs = g.proximity_pairs(order[0], near_base);
    if p.layout == Layout::Growth && p.growth.islands > 1 {
        // Cycles stay inside a clump: the bridges are the only way between clumps.
        all_pairs.retain(|[a, b]| island_of.get(*a) == island_of.get(*b));
    }
    let live_pairs: Vec<[usize; 2]> = all_pairs
        .iter()
        .copied()
        .filter(|[a, b]| g.rooms[*a].alive && g.rooms[*b].alive)
        .collect();
    g.ensure(&live_pairs, room_index);
    g.statics.clear();
    for [a, b] in &live_pairs {
        if let Some(mut candidates) = g.cache.remove(&(*a, *b)) {
            g.statics.append(&mut candidates);
        }
    }
    // Candidate vectors from the earlier, relaxed tree graph are no longer
    // needed; keeping them would duplicate all corridor geometry during the
    // expensive cycle phase.
    g.cache = HashMap::new();
    g.no_fit = g.statics.iter().map(|_| AtomicBool::new(false)).collect();
    g.no_cross = g.statics.iter().map(|_| AtomicBool::new(false)).collect();
    g.rebuild_option_cache();
    let beta_max;
    {
        let save_used: Vec<Vec<Port>> = g.rooms.iter().map(|r| r.used.clone()).collect();
        let save_nodes = g.nodes.clone();
        let save_edges = g.edges.clone();
        loop {
            if g.stopped() {
                break;
            }
            let mut opts = g.options();
            let best = opts
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.weight.partial_cmp(&b.weight).unwrap())
                .map(|(i, _)| i);
            let Some(best) = best else { break };
            let changes = g.apply(opts.swap_remove(best).act);
            g.refresh_option_cache(&changes);
        }
        beta_max = beta_now(g);
        g.nodes = save_nodes;
        g.edges = save_edges;
        for (r, u) in g.rooms.iter_mut().zip(save_used) {
            r.used = u;
        }
        // The copy may have marked candidates as permanently blocked only
        // because of additions made during this hypothetical maximal fill.
        for blocked in &g.no_fit {
            blocked.store(false, Ordering::Relaxed);
        }
        for blocked in &g.no_cross {
            blocked.store(false, Ordering::Relaxed);
        }
        g.rebuild_option_cache();
    }
    (beta_max, all_pairs)
}

/// Independent cycles of the floor: edges minus vertices (alive rooms and junctions) plus one.
pub(super) fn beta_now(g: &Gen<'_>) -> i32 {
    let alive_rooms = g.rooms.iter().filter(|r| r.alive).count() as i32;
    g.edges.len() as i32
        - (alive_rooms
            + g.nodes
                .iter()
                .filter(|n| matches!(n, Node::Junction { .. }))
                .count() as i32)
        + 1
}
