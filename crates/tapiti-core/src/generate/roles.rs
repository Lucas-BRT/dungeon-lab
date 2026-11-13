//! §7.5.2 The entrance, and the exit at a far leaf.

use super::*;

/// Marks `start_room` as the entrance and, when `with_exit`, picks the exit room (a far leaf when there is one) and returns it.
pub(super) fn assign_entrance_and_exit(
    g: &mut Gen<'_>,
    start_room: usize,
    with_exit: bool,
) -> Option<usize> {
    let p = g.p;
    g.rooms[start_room].role = Role::Entrance;
    if !with_exit {
        return None;
    }
    let h0 = room_hops(&g.nodes, &g.edges, g.node_of_room[start_room]);
    let leaves: Vec<usize> = g
        .rooms
        .iter()
        .filter(|r| r.alive && r.id != start_room && g.degree(r.id) == 1)
        .map(|r| r.id)
        .collect();
    let pool: Vec<usize> = if leaves.is_empty() {
        g.rooms
            .iter()
            .filter(|r| r.alive && r.id != start_room)
            .map(|r| r.id)
            .collect()
    } else {
        leaves
    };
    // An entrance room no corridor can leave leaves nothing to pick: the exit stays on it, V5 flags the draw and `stage` redraws.
    let mut exit_room = pool.first().copied().unwrap_or(start_room);
    for &r in &pool {
        if h0[g.node_of_room[r]] > h0[g.node_of_room[exit_room]] {
            exit_room = r;
        }
    }
    if p.layout == Layout::Growth {
        // The cycles added later shorten walks, so the pick keeps a margin over `exitMinHops` when the tree is deep enough for one.
        let pick = |g: &Gen<'_>, wanted: f64| -> Vec<usize> {
            g.rooms
                .iter()
                .filter(|r| {
                    r.alive
                        && r.id != start_room
                        && h0[g.node_of_room[r.id]] >= wanted
                        && g.degree(r.id) <= p.exit_doors as usize
                })
                .map(|r| r.id)
                .collect()
        };
        let mut eligible = pick(g, p.exit_min_hops as f64 + 2.0);
        if eligible.is_empty() {
            eligible = pick(g, p.exit_min_hops as f64);
        }
        if !eligible.is_empty() {
            exit_room = eligible[(g.rng.next() * eligible.len() as f64).floor() as usize];
        }
    }
    if h0[g.node_of_room[exit_room]] < p.exit_min_hops as f64 {
        g.log.push(format!(
            "exit only {} hops from the entrance (wanted {})",
            h0[g.node_of_room[exit_room]], p.exit_min_hops
        ));
    }
    g.rooms[exit_room].role = Role::Exit;
    g.exit_room = exit_room;
    Some(exit_room)
}

/// The rooms along a path of nodes of H, in order (junctions are skipped).
pub(super) fn path_rooms(nodes: &[Node], path: &[usize]) -> Vec<usize> {
    path.iter()
        .filter_map(|&node| match nodes.get(node) {
            Some(Node::Room { room }) => Some(*room),
            _ => None,
        })
        .collect()
}
