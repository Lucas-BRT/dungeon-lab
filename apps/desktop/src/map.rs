//! The map view: world-to-screen transform and egui painting of rooms, corridors and overlays.

use crate::{class_label, point_along, role_label, view_box, BuildingMap, Overlays};
use eframe::egui::{
    epaint::PathShape, Align2, Color32, FontId, Mesh, Painter, Pos2, Rect, Shape as Draw, Stroke,
    Vec2,
};
use tapiti_core::generate::{Dungeon, Node, Role};
use tapiti_core::geom::{dir_of, Shape};

/// World (generator pixels) to screen.
pub struct Xf {
    rect: Rect,
    origin: Pos2,
    pub scale: f32,
    ox: f32,
    oy: f32,
    vx: f64,
    vy: f64,
}

impl Xf {
    pub fn new(rect: Rect, min: [f64; 2], max: [f64; 2], zoom: f64, pan: [f64; 2]) -> Self {
        let (vx, vy, width, height) = view_box(min, max, zoom, pan);
        let scale = (f64::from(rect.width()) / width).min(f64::from(rect.height()) / height);
        Self {
            rect,
            origin: rect.min,
            scale: scale as f32,
            ox: ((f64::from(rect.width()) - width * scale) / 2.0) as f32,
            oy: ((f64::from(rect.height()) - height * scale) / 2.0) as f32,
            vx,
            vy,
        }
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    pub fn pt(&self, p: [f64; 2]) -> Pos2 {
        Pos2::new(
            self.origin.x + self.ox + ((p[0] - self.vx) * f64::from(self.scale)) as f32,
            self.origin.y + self.oy + ((p[1] - self.vy) * f64::from(self.scale)) as f32,
        )
    }

    pub fn to_world(&self, s: Pos2) -> [f64; 2] {
        let scale = f64::from(self.scale).max(1e-6);
        [
            self.vx + f64::from(s.x - self.origin.x - self.ox) / scale,
            self.vy + f64::from(s.y - self.origin.y - self.oy) / scale,
        ]
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Color32,
    pub corridor: Color32,
    pub grid: Color32,
    pub ink: Color32,
    pub accent: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub dark: bool,
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                background: Color32::from_rgb(0x15, 0x18, 0x1d),
                corridor: Color32::from_rgb(0x4a, 0x52, 0x60),
                grid: Color32::from_rgb(0x26, 0x2b, 0x33),
                ink: Color32::from_rgb(0xee, 0xf0, 0xf3),
                accent: Color32::from_rgb(0x5b, 0x9b, 0xf5),
                success: Color32::from_rgb(0x4a, 0xc2, 0x6b),
                warning: Color32::from_rgb(0xe8, 0xb3, 0x39),
                danger: Color32::from_rgb(0xef, 0x5d, 0x5d),
                dark,
            }
        } else {
            Self {
                background: Color32::from_rgb(0xf6, 0xf7, 0xf9),
                corridor: Color32::from_rgb(0xa9, 0xb1, 0xbd),
                grid: Color32::from_rgb(0xe3, 0xe6, 0xeb),
                ink: Color32::from_rgb(0x1b, 0x1f, 0x26),
                accent: Color32::from_rgb(0x1f, 0x6f, 0xe0),
                success: Color32::from_rgb(0x1d, 0x9a, 0x44),
                warning: Color32::from_rgb(0xb7, 0x7d, 0x00),
                danger: Color32::from_rgb(0xd1, 0x2f, 0x2f),
                dark,
            }
        }
    }

    pub fn shape(&self, class: &str) -> Color32 {
        let (r, g, b) = match (class, self.dark) {
            ("rect", true) => (0x49, 0x6c, 0x9a),
            ("disc", true) => (0x29, 0xa8, 0x9a),
            ("oct", true) => (0x9b, 0x78, 0xd1),
            ("L", true) => (0xce, 0x84, 0x3c),
            ("star", true) => (0xd0, 0xae, 0x38),
            ("rect", false) => (0x31, 0x5f, 0x96),
            ("disc", false) => (0x08, 0x7f, 0x76),
            ("oct", false) => (0x72, 0x53, 0xaf),
            ("L", false) => (0xa9, 0x58, 0x12),
            ("star", false) => (0x93, 0x6c, 0x00),
            _ => (0x59, 0x67, 0x7a),
        };
        Color32::from_rgb(r, g, b)
    }

    /// Text on top of a room fill: white reads on every shape colour.
    fn on_room(&self) -> Color32 {
        Color32::WHITE
    }

    fn role(&self, role: Role) -> Color32 {
        match role {
            Role::Start | Role::Rest => self.success,
            Role::Stairs | Role::Switch => self.accent,
            Role::Ambush | Role::Boss | Role::Trap => self.danger,
            Role::Key | Role::Prize => self.warning,
            Role::Empty | Role::Plain => self.grid,
        }
    }
}

pub const SHAPE_CLASSES: [&str; 5] = ["rect", "disc", "oct", "L", "star"];

/// Ear clipping, for the concave rooms (L, truncated star): egui fills only convex paths.
pub fn triangulate(pts: &[[f64; 2]]) -> Vec<[usize; 3]> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let area: f64 = (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };
    let cross = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) * sign
    };
    let mut ring: Vec<usize> = (0..n).collect();
    let mut out = Vec::with_capacity(n - 2);
    'clip: while ring.len() > 3 {
        let m = ring.len();
        for i in 0..m {
            let (p, c, q) = (ring[(i + m - 1) % m], ring[i], ring[(i + 1) % m]);
            if cross(pts[p], pts[c], pts[q]) <= 1e-9 {
                continue;
            }
            let blocked = ring.iter().any(|&o| {
                if [p, c, q].iter().any(|&v| pts[v] == pts[o]) {
                    return false;
                }
                cross(pts[p], pts[c], pts[o]) >= 0.0
                    && cross(pts[c], pts[q], pts[o]) >= 0.0
                    && cross(pts[q], pts[p], pts[o]) >= 0.0
            });
            if !blocked {
                out.push([p, c, q]);
                ring.remove(i);
                continue 'clip;
            }
        }
        break; // degenerate ring: fan the rest below
    }
    for i in 1..ring.len().saturating_sub(1) {
        out.push([ring[0], ring[i], ring[i + 1]]);
    }
    out
}

fn polyline(
    painter: &Painter,
    xf: &Xf,
    points: &[[f64; 2]],
    color: Color32,
    width: f32,
    dashed: bool,
) {
    if points.len() < 2 {
        return;
    }
    let width = (width * xf.scale).max(1.0);
    let screen: Vec<Pos2> = points.iter().map(|p| xf.pt(*p)).collect();
    if dashed {
        painter.extend(Draw::dashed_line(
            &screen,
            Stroke::new(width, color),
            width * 3.0,
            width * 2.0,
        ));
        return;
    }
    // Round joins and caps: a disc at every vertex hides the gaps egui leaves at bends.
    painter.add(PathShape::line(screen.clone(), Stroke::new(width, color)));
    if width > 2.5 {
        for p in screen {
            painter.circle_filled(p, width / 2.0, color);
        }
    }
}

fn room_shape(
    painter: &Painter,
    xf: &Xf,
    shape: &Shape,
    fill: Option<Color32>,
    stroke: Color32,
    width: f32,
    dashed: bool,
) {
    let width = (width * xf.scale).max(1.0);
    match shape {
        Shape::Disc { c, r } => {
            let (center, radius) = (xf.pt(*c), *r as f32 * xf.scale);
            if let Some(fill) = fill {
                painter.circle_filled(center, radius, fill);
            }
            if dashed {
                let ring: Vec<Pos2> = (0..=48)
                    .map(|i| {
                        let t = i as f32 / 48.0 * std::f32::consts::TAU;
                        center + Vec2::angled(t) * radius
                    })
                    .collect();
                painter.extend(Draw::dashed_line(
                    &ring,
                    Stroke::new(width, stroke),
                    width * 3.0,
                    width * 2.0,
                ));
            } else {
                painter.circle_stroke(center, radius, Stroke::new(width, stroke));
            }
        }
        Shape::Poly { pts } => {
            let screen: Vec<Pos2> = pts.iter().map(|p| xf.pt(*p)).collect();
            if let Some(fill) = fill {
                let mut mesh = Mesh::default();
                for p in &screen {
                    mesh.colored_vertex(*p, fill);
                }
                for [a, b, c] in triangulate(pts) {
                    mesh.add_triangle(a as u32, b as u32, c as u32);
                }
                painter.add(Draw::mesh(mesh));
            }
            if dashed {
                let mut closed = screen.clone();
                closed.extend(screen.first().copied());
                painter.extend(Draw::dashed_line(
                    &closed,
                    Stroke::new(width, stroke),
                    width * 3.0,
                    width * 2.0,
                ));
            } else {
                painter.add(PathShape::closed_line(screen, Stroke::new(width, stroke)));
            }
        }
    }
}

fn grid(painter: &Painter, xf: &Xf, grid_x: &[f64], grid_y: &[f64], color: Color32) {
    let (Some(&y0), Some(&y1), Some(&x0), Some(&x1)) =
        (grid_y.first(), grid_y.last(), grid_x.first(), grid_x.last())
    else {
        return;
    };
    let stroke = Stroke::new(1.0, color);
    for &x in grid_x {
        painter.line_segment([xf.pt([x, y0]), xf.pt([x, y1])], stroke);
    }
    for &y in grid_y {
        painter.line_segment([xf.pt([x0, y]), xf.pt([x1, y])], stroke);
    }
}

fn label(painter: &Painter, xf: &Xf, text: &str, at: [f64; 2], color: Color32, world_size: f32) {
    let size = (world_size * xf.scale).round();
    if size < 6.0 {
        return;
    }
    // Small rooms get only their number: the full label would spill over the neighbours.
    let text = if size < 12.0 {
        text.split(' ').next().unwrap_or(text)
    } else {
        text
    };
    painter.text(
        xf.pt(at),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(size.clamp(9.0, 40.0)),
        color,
    );
}

pub fn draw_building(
    painter: &Painter,
    xf: &Xf,
    building: &BuildingMap,
    overlays: Overlays,
    pal: &Palette,
) {
    if overlays.cells {
        grid(painter, xf, &building.grid_x, &building.grid_y, pal.grid);
    }
    for points in &building.edges {
        polyline(
            painter,
            xf,
            points,
            pal.corridor,
            building.corridor_width.max(2.0) as f32,
            false,
        );
    }
    for shape in &building.rooms {
        room_shape(
            painter,
            xf,
            shape,
            Some(pal.shape("rect").gamma_multiply(0.9)),
            pal.grid,
            4.0,
            false,
        );
    }
}

pub fn draw_dungeon(
    painter: &Painter,
    xf: &Xf,
    dungeon: &Dungeon,
    overlays: Overlays,
    pal: &Palette,
) {
    if overlays.cells {
        grid(painter, xf, &dungeon.grid_x, &dungeon.grid_y, pal.grid);
    }
    if overlays.pairs {
        for [a, b] in &dungeon.pairs {
            if let (Some(first), Some(second)) = (dungeon.rooms.get(*a), dungeon.rooms.get(*b)) {
                painter.add(Draw::dashed_line(
                    &[xf.pt(first.center), xf.pt(second.center)],
                    Stroke::new(2.0, pal.accent.gamma_multiply(0.6)),
                    8.0,
                    7.0,
                ));
            }
        }
    }
    if overlays.candidates {
        for candidate in &dungeon.candidates {
            polyline(
                painter,
                xf,
                &candidate.pts,
                pal.accent.gamma_multiply(0.45),
                5.0,
                false,
            );
        }
    }
    for edge in &dungeon.edges {
        let gated = edge.requires_keys > 0 || edge.requires_switches > 0;
        polyline(
            painter,
            xf,
            &edge.pts,
            if gated { pal.warning } else { pal.corridor },
            dungeon.params.width.max(2.0) as f32,
            gated,
        );
        if gated {
            let middle = xf.pt(point_along(&edge.pts, 0.5));
            let radius = (13.0 * xf.scale).max(7.0);
            painter.circle(
                middle,
                radius,
                pal.background,
                Stroke::new(2.0, pal.warning),
            );
            painter.text(
                middle,
                Align2::CENTER_CENTER,
                if edge.requires_keys > 0 { "K" } else { "S" },
                FontId::proportional((radius * 1.2).clamp(9.0, 30.0)),
                pal.ink,
            );
        }
    }
    for room in &dungeon.rooms {
        if room.alive {
            let alpha = if room.leaf { 0.65 } else { 0.92 };
            room_shape(
                painter,
                xf,
                &room.shape,
                Some(pal.shape(room.cls.name()).gamma_multiply(alpha)),
                pal.role(room.role),
                4.0,
                false,
            );
        } else {
            room_shape(painter, xf, &room.shape, None, pal.danger, 3.0, true);
        }
    }
    if overlays.ports {
        for room in &dungeon.rooms {
            for port in &room.ports {
                let half = (4.0 * xf.scale).max(1.5);
                painter.rect_filled(
                    Rect::from_center_size(xf.pt(port.p), Vec2::splat(half * 2.0)),
                    0.0,
                    pal.ink.gamma_multiply(0.5),
                );
            }
        }
    }
    for room in &dungeon.rooms {
        for port in &room.used {
            let d = dir_of(port.k);
            polyline(
                painter,
                xf,
                &[port.p, [port.p[0] + d[0] * 14.0, port.p[1] + d[1] * 14.0]],
                pal.ink,
                5.0,
                false,
            );
        }
    }
    for node in &dungeon.nodes {
        if let Node::Junction { p } = node {
            painter.circle(
                xf.pt(**p),
                (7.0 * xf.scale).max(2.5),
                pal.ink,
                Stroke::new(1.5, pal.background),
            );
        }
    }
    if overlays.labels {
        for room in dungeon.rooms.iter().filter(|room| room.alive) {
            let inventory = [
                (room.keys != 0).then(|| format!("K{}", room.keys.count_ones())),
                (room.switches != 0).then(|| format!("S{}", room.switches.count_ones())),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
            let role = if room.role == Role::Plain {
                String::new()
            } else {
                format!(" · {}", role_label(room.role))
            };
            let text = format!(
                "#{} {}{}{}",
                room.id,
                class_label(room.cls.name()),
                role,
                if inventory.is_empty() {
                    String::new()
                } else {
                    format!(" · {inventory}")
                }
            );
            label(painter, xf, &text, room.center, pal.on_room(), 18.0);
        }
    }
}

/// Small key over the map: what the fills and outlines mean.
pub fn legend(painter: &Painter, rect: Rect, pal: &Palette, text_color: Color32) {
    let font = FontId::proportional(12.0);
    let row = 18.0;
    let items: Vec<(Color32, String)> = SHAPE_CLASSES
        .iter()
        .map(|c| (pal.shape(c), class_label(c).to_string()))
        .collect();
    let height = row * (items.len() as f32 + 2.0) + 14.0;
    let panel = Rect::from_min_size(
        Pos2::new(rect.left() + 10.0, rect.bottom() - height - 10.0),
        Vec2::new(200.0, height),
    );
    let bg = if pal.dark {
        Color32::from_black_alpha(150)
    } else {
        Color32::from_white_alpha(200)
    };
    painter.rect_filled(panel, 8.0, bg);
    let mut y = panel.top() + 12.0;
    for (color, name) in items {
        painter.rect_filled(
            Rect::from_min_size(Pos2::new(panel.left() + 10.0, y - 6.0), Vec2::splat(12.0)),
            3.0,
            color,
        );
        painter.text(
            Pos2::new(panel.left() + 30.0, y),
            Align2::LEFT_CENTER,
            name,
            font.clone(),
            text_color,
        );
        y += row;
    }
    painter.line_segment(
        [
            Pos2::new(panel.left() + 10.0, y),
            Pos2::new(panel.left() + 24.0, y),
        ],
        Stroke::new(3.0, pal.warning),
    );
    painter.text(
        Pos2::new(panel.left() + 30.0, y),
        Align2::LEFT_CENTER,
        t!("gate (key/switch)"),
        font.clone(),
        text_color,
    );
    y += row;
    painter.circle_filled(Pos2::new(panel.left() + 16.0, y), 4.0, pal.ink);
    painter.text(
        Pos2::new(panel.left() + 30.0, y),
        Align2::LEFT_CENTER,
        t!("junction · outline = role"),
        font,
        text_color,
    );
}

#[cfg(test)]
mod tests {
    use super::triangulate;

    fn area(pts: &[[f64; 2]]) -> f64 {
        (0..pts.len())
            .map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                a[0] * b[1] - b[0] * a[1]
            })
            .sum::<f64>()
            .abs()
            / 2.0
    }

    fn tri_area(pts: &[[f64; 2]], t: [usize; 3]) -> f64 {
        area(&[pts[t[0]], pts[t[1]], pts[t[2]]])
    }

    #[test]
    fn concave_l_and_star_are_covered_exactly() {
        let l = [
            [0.0, 0.0],
            [4.0, 0.0],
            [4.0, 2.0],
            [2.0, 2.0],
            [2.0, 4.0],
            [0.0, 4.0],
        ];
        let star = [
            [0.0, -4.0],
            [1.0, -1.0],
            [4.0, 0.0],
            [1.0, 1.0],
            [0.0, 4.0],
            [-1.0, 1.0],
            [-4.0, 0.0],
            [-1.0, -1.0],
        ];
        for pts in [&l[..], &star[..], &[l[5], l[4], l[3], l[2], l[1], l[0]][..]] {
            let tris = triangulate(pts);
            assert_eq!(tris.len(), pts.len() - 2);
            let sum: f64 = tris.iter().map(|t| tri_area(pts, *t)).sum();
            assert!((sum - area(pts)).abs() < 1e-9, "{sum} vs {}", area(pts));
        }
    }
}
