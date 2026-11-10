//! Floors drawn by the generator itself and inlined in pages as SVG.

use std::sync::OnceLock;
use std::time::Instant;
use tapiti_core::{
    generate::{Dungeon, generate},
    params::params_of_level,
    stage::stage,
    validate::validate,
};
use tapiti_render::{Style, floor_svg};
use topcoat::{
    context::Cx,
    view::{NodeViewParts, PartsWriter},
};

/// Markup produced by this repository's own code (generator output, repository docs), never from request data, so it is rendered unescaped.
pub struct TrustedHtml(pub String);

impl NodeViewParts for TrustedHtml {
    fn into_view_parts(self, _cx: &Cx, parts: &mut PartsWriter<'_>) {
        parts.push_string_unescaped(self.0);
    }
}

/// Landing page floor: level 5, picked for its mix of shapes, junctions and dead ends.
pub const HERO_SEED: u32 = 26;
/// Shapes figure: level 5 with every shape weighted equally, so all five appear.
pub const SHAPES_SEED: u32 = 16;
/// Validator figure: a level 5 first draw that the validator rejects (the stairs end up too close to the entrance).
pub const REJECTED_SEED: u32 = 17;

fn svg(dungeon: &Dungeon, label: String) -> TrustedHtml {
    TrustedHtml(floor_svg(
        dungeon,
        &Style {
            label: Some(label),
            ..Style::default()
        },
    ))
}

pub fn hero(label: String) -> TrustedHtml {
    static FLOOR: OnceLock<Dungeon> = OnceLock::new();
    svg(
        FLOOR.get_or_init(|| generate(HERO_SEED, &params_of_level(5))),
        label,
    )
}

pub fn shapes(label: String) -> TrustedHtml {
    static FLOOR: OnceLock<Dungeon> = OnceLock::new();
    let floor = FLOOR.get_or_init(|| {
        let mut params = params_of_level(5);
        params.weights.rect = 1.0;
        params.weights.disc = 1.0;
        params.weights.oct = 1.0;
        params.weights.l = 1.0;
        params.weights.star = 1.0;
        params.m = 8;
        stage(SHAPES_SEED, &params, 8).d
    });
    svg(floor, label)
}

/// The rejected first draw and what the validator said about it.
pub fn rejected(label: String) -> (TrustedHtml, Vec<String>) {
    static FLOOR: OnceLock<(Dungeon, Vec<String>)> = OnceLock::new();
    let (floor, violations) = FLOOR.get_or_init(|| {
        let floor = generate(REJECTED_SEED, &params_of_level(5));
        let violations = validate(&floor);
        (floor, violations)
    });
    (svg(floor, label), violations.clone())
}

/// Numbers measured on this machine when the server starts, so the landing page never quotes a figure the code did not produce.
#[derive(Clone, Copy, Debug)]
pub struct Speed {
    /// Median time to dig a validated level 5 floor, in milliseconds.
    pub median_ms: u32,
    pub valid: usize,
    pub total: usize,
}

pub fn speed() -> Speed {
    static SPEED: OnceLock<Speed> = OnceLock::new();
    *SPEED.get_or_init(|| {
        let params = params_of_level(5);
        let total = 60;
        let mut times = Vec::with_capacity(total);
        let mut valid = 0;
        for seed in 1..=total as u32 {
            let started = Instant::now();
            let staged = stage(seed, &params, 8);
            times.push(started.elapsed().as_secs_f64() * 1000.0);
            valid += usize::from(staged.violations.is_empty());
        }
        times.sort_by(f64::total_cmp);
        Speed {
            median_ms: times[times.len() / 2].round().max(1.0) as u32,
            valid,
            total,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_validator_really_rejects_the_figure_floor() {
        let (_, violations) = rejected(String::new());
        assert!(
            violations.iter().any(|v| v.starts_with("V5")),
            "{violations:?}"
        );
    }

    #[test]
    fn the_shapes_figure_shows_all_five_shapes() {
        let svg = shapes(String::new()).0;
        for class in ["c-rect", "c-disc", "c-oct", "c-L", "c-star"] {
            assert!(svg.contains(class), "{class}");
        }
    }
}
