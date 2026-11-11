//! Picking on the map: which room, door or corridor lies under the pointer.

use crate::{class_label, role_label};
use tapiti_core::generate::{Floor, Node};
use tapiti_core::geom::{dist, Shape};

fn point_in_shape(shape: &Shape, point: [f64; 2]) -> bool {
    match shape {
        Shape::Disc { c, r } => dist(*c, point) <= *r,
        Shape::Poly { pts } => {
            let mut inside = false;
            let mut j = pts.len().saturating_sub(1);
            for i in 0..pts.len() {
                let [xi, yi] = pts[i];
                let [xj, yj] = pts[j];
                let crosses = (yi > point[1]) != (yj > point[1])
                    && point[0] < (xj - xi) * (point[1] - yi) / (yj - yi) + xi;
                if crosses {
                    inside = !inside;
                }
                j = i;
            }
            inside
        }
    }
}

fn distance_to_segment(point: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let length2 = dx * dx + dy * dy;
    let t = if length2 == 0.0 {
        0.0
    } else {
        (((point[0] - a[0]) * dx + (point[1] - a[1]) * dy) / length2).clamp(0.0, 1.0)
    };
    dist(point, [a[0] + dx * t, a[1] + dy * t])
}

pub(crate) fn node_label(floor: &Floor, node: usize) -> String {
    match floor.nodes.get(node) {
        Some(Node::Junction { .. }) => "junction".into(),
        Some(Node::Room { room }) => format!("room #{id}", id = floor.rooms[*room].id),
        None => "point".into(),
    }
}

pub(crate) fn map_object_at(floor: &Floor, point: [f64; 2], zoom: f64) -> String {
    for room in floor.rooms.iter().rev() {
        if room.alive && point_in_shape(&room.shape, point) {
            let kind = if room.leaf { "leaf chamber" } else { "room" };
            let items = [
                (room.keys != 0).then(|| format!("{n} key(s)", n = room.keys.count_ones())),
                (room.switches != 0)
                    .then(|| format!("{n} switch(es)", n = room.switches.count_ones())),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
            let items = if items.is_empty() {
                String::new()
            } else {
                format!("\nItems: {}", items.join(" · "))
            };
            return format!(
                "{kind} · {role}\nRoom #{id} · {class}\nArea: {area:.0} px²\nDoors used: {used} · possible: {possible}{items}",
                kind = kind,
                role = role_label(room.role),
                id = room.id,
                class = class_label(room.cls.name()),
                area = room.area,
                used = room.used.len(),
                possible = room.ports.len(),
            );
        }
    }
    for (id, node) in floor.nodes.iter().enumerate() {
        if let Node::Junction { p } = node {
            if dist(**p, point) <= (8.0 / zoom.max(0.1)).max(6.0) {
                let degree = floor
                    .edges
                    .iter()
                    .filter(|edge| edge.u == id || edge.v == id)
                    .count();
                return format!(
                    "Corridor junction\n{degree} arms connected\nPosition: {x:.0}, {y:.0} px",
                    degree = degree,
                    x = p[0],
                    y = p[1]
                );
            }
        }
    }
    for edge in &floor.edges {
        if edge.pts.windows(2).any(|segment| {
            distance_to_segment(point, segment[0], segment[1])
                <= floor.params.width / 2.0 + 5.0 / zoom.max(0.1)
        }) {
            let length = edge
                .pts
                .windows(2)
                .map(|segment| dist(segment[0], segment[1]))
                .sum::<f64>();
            let gates = [
                (edge.requires_keys != 0).then(|| format!("key {:08b}", edge.requires_keys)),
                (edge.requires_switches != 0)
                    .then(|| format!("switch {:08b}", edge.requires_switches)),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
            return format!(
                "Corridor segment #{id}\n{from} – {to}\n{n} segment(s) · {length:.0} px{gates}",
                id = edge.id,
                from = node_label(floor, edge.u),
                to = node_label(floor, edge.v),
                n = edge.pts.len().saturating_sub(1),
                length = length,
                gates = if gates.is_empty() {
                    String::new()
                } else {
                    format!("\nLocked: {}", gates.join(" and "))
                }
            );
        }
    }
    String::new()
}
