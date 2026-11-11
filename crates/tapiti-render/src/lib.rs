//! SVG rendering of a generated floor. The output is a plain `String`, so the same floor can be inlined in a page, saved as a file
//! (README, docs, social cards) or compared in a test: the same seed and parameters always give the same bytes.
//!
//! Shapes carry CSS classes (`room c-rect`, `corridor`, …) and a `--d` custom property with the room's distance in rooms from the entrance,
//! so a page can colour them with its own tokens and reveal the floor outwards from the entrance with a staggered animation.

use std::fmt::Write;
use tapiti_core::generate::{room_hops, Floor, Node, Role};
use tapiti_core::geom::{dir_of, Shape, V};

/// How colours reach the SVG.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    /// Classes only: the surrounding page styles the shapes (see [`CLASS_NAMES`]). Meant for inlining.
    Inherit,
    /// A `<style>` for a dark background, so the file renders on its own.
    Dark,
    /// A `<style>` for a light background, so the file renders on its own.
    Light,
}

#[derive(Clone, Debug)]
pub struct Style {
    pub theme: Theme,
    /// Draw the grid lines of the scaffold.
    pub grid: bool,
    /// Write `#id` on each room.
    pub labels: bool,
    /// Margin around the floor, in world units, as a fraction of the larger side.
    pub margin: f64,
    /// Accessible name of the figure; defaults to an English description with the seed.
    pub label: Option<String>,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            theme: Theme::Inherit,
            grid: false,
            labels: false,
            margin: 0.03,
            label: None,
        }
    }
}

/// Every class the renderer can emit, for pages that style an inlined SVG.
pub const CLASS_NAMES: &[&str] = &[
    "tapiti-floor",
    "grid",
    "corridor",
    "gated",
    "gate",
    "room",
    "leaf",
    "c-rect",
    "c-disc",
    "c-oct",
    "c-L",
    "c-star",
    "r-start",
    "r-stairs",
    "r-ambush",
    "r-boss",
    "r-rest",
    "r-key",
    "r-switch",
    "r-trap",
    "r-prize",
    "r-empty",
    "r-plain",
    "junction",
    "door",
    "label",
];

fn n(v: f64) -> String {
    let s = format!("{v:.1}");
    s.strip_suffix(".0").map(str::to_owned).unwrap_or(s)
}

fn role_class(role: Role) -> &'static str {
    match role {
        Role::Start => "r-start",
        Role::Stairs => "r-stairs",
        Role::Ambush => "r-ambush",
        Role::Boss => "r-boss",
        Role::Rest => "r-rest",
        Role::Key => "r-key",
        Role::Switch => "r-switch",
        Role::Trap => "r-trap",
        Role::Prize => "r-prize",
        Role::Empty => "r-empty",
        Role::Plain => "r-plain",
    }
}

fn class_class(name: &str) -> &'static str {
    match name {
        "rect" => "c-rect",
        "disc" => "c-disc",
        "oct" => "c-oct",
        "L" => "c-L",
        "star" => "c-star",
        _ => "c-rect",
    }
}

fn points(pts: &[V]) -> String {
    let mut out = String::new();
    for (i, p) in pts.iter().enumerate() {
        let _ = write!(
            out,
            "{}{} {}",
            if i == 0 { "M" } else { "L" },
            n(p[0]),
            n(p[1])
        );
    }
    out
}

fn theme_css(theme: Theme) -> &'static str {
    match theme {
        Theme::Inherit => "",
        Theme::Dark => concat!(
            ".tapiti-floor{background:#15110e}.grid{stroke:#2b231d}.corridor{stroke:#5a4e43}.gated{stroke:#e8b339}.gate{fill:#15110e;stroke:#e8b339}",
            ".room{stroke-width:4}.c-rect{fill:#496c9a}.c-disc{fill:#29a89a}.c-oct{fill:#9b78d1}.c-L{fill:#ce843c}.c-star{fill:#d0ae38}",
            ".r-plain,.r-empty{stroke:#2b231d}.r-start,.r-rest{stroke:#4ac26b}.r-stairs,.r-switch{stroke:#f08a3c}.r-ambush,.r-boss,.r-trap{stroke:#ef5d5d}.r-key,.r-prize{stroke:#e8b339}",
            ".junction{fill:#f2e9dc;stroke:#15110e}.door{stroke:#f2e9dc}.label{fill:#fff}"
        ),
        Theme::Light => concat!(
            ".tapiti-floor{background:#f7f1e6}.grid{stroke:#e6dccb}.corridor{stroke:#b3a590}.gated{stroke:#b77d00}.gate{fill:#f7f1e6;stroke:#b77d00}",
            ".room{stroke-width:4}.c-rect{fill:#315f96}.c-disc{fill:#087f76}.c-oct{fill:#7253af}.c-L{fill:#a95812}.c-star{fill:#936c00}",
            ".r-plain,.r-empty{stroke:#e6dccb}.r-start,.r-rest{stroke:#1d9a44}.r-stairs,.r-switch{stroke:#d96b13}.r-ambush,.r-boss,.r-trap{stroke:#d12f2f}.r-key,.r-prize{stroke:#b77d00}",
            ".junction{fill:#2a2118;stroke:#f7f1e6}.door{stroke:#2a2118}.label{fill:#fff}"
        ),
    }
}

/// Renders `d` as a standalone `<svg>` element.
pub fn floor_svg(d: &Floor, style: &Style) -> String {
    let (x0, x1) = (
        d.grid_x.first().copied().unwrap_or(0.0),
        d.grid_x.last().copied().unwrap_or(1.0),
    );
    let (y0, y1) = (
        d.grid_y.first().copied().unwrap_or(0.0),
        d.grid_y.last().copied().unwrap_or(1.0),
    );
    let (w, h) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
    let m = w.max(h) * style.margin;
    let hops = room_hops(&d.nodes, &d.edges, d.rooms[d.start].id);
    let hop_of_node = |node: usize| -> usize {
        let v = hops.get(node).copied().unwrap_or(0.0);
        if v.is_finite() {
            v as usize
        } else {
            0
        }
    };

    let mut out = String::with_capacity(32 * 1024);
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" class="tapiti-floor" viewBox="{} {} {} {}" role="img" aria-label="{}">"#,
        n(x0 - m),
        n(y0 - m),
        n(w + 2.0 * m),
        n(h + 2.0 * m),
        style
            .label
            .clone()
            .unwrap_or_else(|| format!("Tapiti floor, seed {}", d.seed))
            .replace('&', "&amp;")
            .replace('"', "&quot;")
            .replace('<', "&lt;")
    );
    let css = theme_css(style.theme);
    if !css.is_empty() {
        let _ = write!(out, "<style>{css}</style>");
    }
    if style.grid {
        out.push_str(r#"<path class="grid" fill="none" stroke-width="2" d=""#);
        for &x in &d.grid_x {
            let _ = write!(out, "M{} {}L{} {}", n(x), n(y0), n(x), n(y1));
        }
        for &y in &d.grid_y {
            let _ = write!(out, "M{} {}L{} {}", n(x0), n(y), n(x1), n(y));
        }
        out.push_str(r#""/>"#);
    }
    let width = d.params.width.max(2.0);
    for e in &d.edges {
        let gated = e.requires_keys > 0 || e.requires_switches > 0;
        let _ = write!(
            out,
            r#"<path class="corridor{}" style="--d:{}" fill="none" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"{} d="{}"/>"#,
            if gated { " gated" } else { "" },
            hop_of_node(e.u).max(hop_of_node(e.v)),
            n(width),
            if gated {
                r#" stroke-dasharray="14 9""#
            } else {
                ""
            },
            points(&e.pts)
        );
    }
    for r in d.rooms.iter().filter(|r| r.alive) {
        let class = format!(
            "room {} {}{}",
            class_class(r.cls.name()),
            role_class(r.role),
            if r.leaf { " leaf" } else { "" }
        );
        let d_attr = hop_of_node(
            d.nodes
                .iter()
                .position(|nd| matches!(nd, Node::Room { room } if *room == r.id))
                .unwrap_or(0),
        );
        let opacity = if r.leaf { "0.65" } else { "0.92" };
        match &r.shape {
            Shape::Disc { c, r: radius } => {
                let _ = write!(
                    out,
                    r#"<circle class="{class}" style="--d:{d_attr}" fill-opacity="{opacity}" cx="{}" cy="{}" r="{}"/>"#,
                    n(c[0]),
                    n(c[1]),
                    n(*radius)
                );
            }
            Shape::Poly { pts } => {
                let _ = write!(
                    out,
                    r#"<path class="{class}" style="--d:{d_attr}" fill-opacity="{opacity}" d="{}Z"/>"#,
                    points(pts)
                );
            }
        }
    }
    for r in d.rooms.iter().filter(|r| r.alive) {
        for port in &r.used {
            let k = dir_of(port.k);
            let _ = write!(
                out,
                r#"<path class="door" fill="none" stroke-width="5" d="M{} {}L{} {}"/>"#,
                n(port.p[0]),
                n(port.p[1]),
                n(port.p[0] + k[0] * 14.0),
                n(port.p[1] + k[1] * 14.0)
            );
        }
    }
    for node in &d.nodes {
        if let Node::Junction { p } = node {
            let _ = write!(
                out,
                r#"<circle class="junction" stroke-width="2" cx="{}" cy="{}" r="7"/>"#,
                n(p[0]),
                n(p[1])
            );
        }
    }
    if style.labels {
        let size = (w.max(h) / 60.0).max(10.0);
        for r in d.rooms.iter().filter(|r| r.alive) {
            let _ = write!(
                out,
                r#"<text class="label" text-anchor="middle" dominant-baseline="central" font-size="{}" x="{}" y="{}">#{}</text>"#,
                n(size),
                n(r.center[0]),
                n(r.center[1]),
                r.id
            );
        }
    }
    out.push_str("</svg>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tapiti_core::{generate::generate, params::params_of_level};

    fn svg(seed: u32, level: usize, style: &Style) -> String {
        floor_svg(&generate(seed, &params_of_level(level)), style)
    }

    #[test]
    fn same_seed_gives_the_same_bytes() {
        let style = Style {
            theme: Theme::Dark,
            grid: true,
            labels: true,
            ..Style::default()
        };
        assert_eq!(svg(7, 5, &style), svg(7, 5, &style));
        assert_ne!(svg(7, 5, &style), svg(8, 5, &style));
        let named = Style {
            label: Some("A \"floor\"".into()),
            ..Style::default()
        };
        assert!(svg(7, 5, &named).contains("aria-label=\"A &quot;floor&quot;\""));
    }

    #[test]
    fn draws_rooms_corridors_and_a_viewbox() {
        let s = svg(3, 5, &Style::default());
        assert!(s.starts_with("<svg ") && s.ends_with("</svg>"));
        assert!(s.contains("viewBox=\""));
        assert!(s.matches("class=\"room ").count() >= 5, "rooms");
        assert!(s.contains("class=\"corridor"), "corridors");
        assert!(
            s.contains("r-start") && s.contains("r-stairs"),
            "entrance and stairs"
        );
        assert!(
            !s.contains("<style>"),
            "inherit theme leaves colours to the page"
        );
    }

    #[test]
    fn standalone_themes_embed_their_colours() {
        assert!(svg(
            3,
            2,
            &Style {
                theme: Theme::Dark,
                ..Style::default()
            }
        )
        .contains("<style>"));
        assert!(svg(
            3,
            2,
            &Style {
                theme: Theme::Light,
                ..Style::default()
            }
        )
        .contains("#f7f1e6"));
    }

    #[test]
    fn every_emitted_class_is_documented() {
        let s = svg(
            11,
            5,
            &Style {
                grid: true,
                labels: true,
                ..Style::default()
            },
        );
        for part in s.split("class=\"").skip(1) {
            let classes = part.split('"').next().unwrap();
            for c in classes.split(' ') {
                assert!(CLASS_NAMES.contains(&c), "undocumented class {c}");
            }
        }
    }
}
