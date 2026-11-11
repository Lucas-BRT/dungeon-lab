//! Pacing and progression: keys, switches, the path to the stairs.

use super::*;

pub(super) fn path_rooms(nodes: &[Node], path: &[usize]) -> Vec<usize> {
    path.iter()
        .filter_map(|&node| match nodes.get(node) {
            Some(Node::Room { room }) => Some(*room),
            _ => None,
        })
        .collect()
}

pub(super) fn assign_pacing(g: &mut Gen<'_>, path: &[usize], ambushes: usize) {
    let rooms = path_rooms(&g.nodes, path);
    if rooms.len() < 3 {
        return;
    }
    let choose_at = |fraction: f64, rooms: &[usize], data: &[Room]| -> Option<usize> {
        let lo = 1;
        let hi = rooms.len() - 1;
        (lo..hi)
            .filter_map(|i| {
                let id = rooms[i];
                (data[id].role == Role::Plain)
                    .then_some((id, (i as f64 / (rooms.len() - 1) as f64 - fraction).abs()))
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(id, _)| id)
    };
    if let Some(rest) = choose_at(0.45, &rooms, &g.rooms) {
        g.rooms[rest].role = Role::Rest;
    }
    if let Some(boss) = choose_at(0.85, &rooms, &g.rooms) {
        g.rooms[boss].role = Role::Boss;
    }
    for i in 0..ambushes {
        let fraction = 0.2 + 0.55 * ((i + 1) as f64 / (ambushes + 1) as f64);
        if let Some(room) = choose_at(fraction, &rooms, &g.rooms) {
            g.rooms[room].role = Role::Ambush;
        }
    }
}

pub(super) fn assign_progression(g: &mut Gen<'_>, path: &[usize], path_edges: &[usize]) {
    let requested_keys = g.p.progression.key_locks.min(8);
    let requested_switches = g.p.progression.switch_locks.min(8);
    if g.p.progression.key_locks > 8 || g.p.progression.switch_locks > 8 {
        g.log
            .push("progression: each lock type is limited to eight bits".to_string());
    }
    let requested = requested_keys + requested_switches;
    if requested == 0 || path_edges.is_empty() {
        return;
    }
    let count = requested.min(path_edges.len());
    if count < requested {
        g.log.push(format!(
            "progression: only {} of {} requested locks fit on the main path",
            count, requested
        ));
    }
    let key_count = requested_keys.min(count);

    // Assign every node to the closest node on the designated path. Each lock
    // then gates the complete cut between two path positions, so a loop cannot
    // bypass the corresponding key or switch.
    let mut path_position = vec![usize::MAX; g.nodes.len()];
    let mut queue = VecDeque::new();
    for (position, &node) in path.iter().enumerate() {
        path_position[node] = position;
        queue.push_back(node);
    }
    let mut adjacency = vec![Vec::new(); g.nodes.len()];
    for edge in &g.edges {
        adjacency[edge.u].push(edge.v);
        adjacency[edge.v].push(edge.u);
    }
    while let Some(node) = queue.pop_front() {
        for &next in &adjacency[node] {
            if path_position[next] == usize::MAX {
                path_position[next] = path_position[node];
                queue.push_back(next);
            }
        }
    }

    for index in 0..count {
        let edge_pos = (((index + 1) * path_edges.len()) / (count + 1)).min(path_edges.len() - 1);
        let bit = if index < key_count {
            1u8 << index
        } else {
            1u8 << (index - key_count)
        };
        let is_key = index < key_count;
        for edge in &mut g.edges {
            let (a, b) = (path_position[edge.u], path_position[edge.v]);
            if a.min(b) <= edge_pos && a.max(b) > edge_pos {
                if is_key {
                    edge.requires_keys |= bit;
                } else {
                    edge.requires_switches |= bit;
                }
            }
        }

        let candidate_rooms = path[..=edge_pos.min(path.len() - 1)]
            .iter()
            .rev()
            .filter_map(|node| match g.nodes[*node] {
                Node::Room { room }
                    if matches!(g.rooms[room].role, Role::Plain | Role::Key | Role::Switch) =>
                {
                    Some(room)
                }
                _ => None,
            });
        let room = candidate_rooms
            .into_iter()
            .find(|room| g.rooms[*room].role == Role::Plain)
            .or_else(|| {
                path[..=edge_pos.min(path.len() - 1)]
                    .iter()
                    .rev()
                    .find_map(|node| match g.nodes[*node] {
                        Node::Room { room }
                            if matches!(g.rooms[room].role, Role::Key | Role::Switch) =>
                        {
                            Some(room)
                        }
                        _ => None,
                    })
            })
            .or_else(|| path_rooms(&g.nodes, path).first().copied());
        let Some(room) = room else { continue };
        if is_key {
            g.rooms[room].keys |= bit;
            if !matches!(
                g.rooms[room].role,
                Role::Start | Role::Stairs | Role::Boss | Role::Rest | Role::Ambush
            ) {
                g.rooms[room].role = Role::Key;
            }
        } else {
            g.rooms[room].switches |= bit;
            if !matches!(
                g.rooms[room].role,
                Role::Start | Role::Stairs | Role::Boss | Role::Rest | Role::Ambush | Role::Key
            ) {
                g.rooms[room].role = Role::Switch;
            }
        }
    }
}
