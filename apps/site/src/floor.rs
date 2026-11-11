//! Floors drawn by the generator itself and inlined in pages as SVG.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tapiti_core::{
    generate::{Floor, generate},
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

fn svg(floor: &Floor, label: String) -> TrustedHtml {
    TrustedHtml(floor_svg(
        floor,
        &Style {
            label: Some(label),
            ..Style::default()
        },
    ))
}

pub fn hero(label: String) -> TrustedHtml {
    static FLOOR: OnceLock<Floor> = OnceLock::new();
    svg(
        FLOOR.get_or_init(|| generate(HERO_SEED, &params_of_level(5))),
        label,
    )
}

pub fn shapes(label: String) -> TrustedHtml {
    static FLOOR: OnceLock<Floor> = OnceLock::new();
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
    static FLOOR: OnceLock<(Floor, Vec<String>)> = OnceLock::new();
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

/// Floors that visitors ask for by seed ("dig again"). Generating one takes about 15 ms, so the site keeps the latest ones and spends at
/// most `BUDGET` new floors per `WINDOW` across all visitors (behind a proxy every visitor shares one address anyway); over budget, the
/// landing page floor is shown instead of working for free.
const BUDGET: u32 = 40;
const WINDOW: Duration = Duration::from_secs(10);
const CACHE_LIMIT: usize = 64;

struct Dug {
    floors: HashMap<u32, Arc<Floor>>,
    window_start: Instant,
    spent: u32,
}

fn dug() -> &'static Mutex<Dug> {
    static DUG: OnceLock<Mutex<Dug>> = OnceLock::new();
    DUG.get_or_init(|| {
        Mutex::new(Dug {
            floors: HashMap::new(),
            window_start: Instant::now(),
            spent: 0,
        })
    })
}

/// The level 5 floor of `seed`, or the landing page's floor when the budget is spent.
pub fn dig(seed: u32, label: String) -> (u32, TrustedHtml) {
    let floor = {
        let mut state = dug()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(floor) = state.floors.get(&seed) {
            Some((seed, floor.clone()))
        } else {
            if state.window_start.elapsed() >= WINDOW {
                state.window_start = Instant::now();
                state.spent = 0;
            }
            if state.spent >= BUDGET {
                None
            } else {
                state.spent += 1;
                // Generated outside the lock below would be nicer, but the budget keeps this short and bounded.
                let floor = Arc::new(stage(seed, &params_of_level(5), 8).d);
                if state.floors.len() >= CACHE_LIMIT {
                    state.floors.clear();
                }
                state.floors.insert(seed, floor.clone());
                Some((seed, floor))
            }
        }
    };
    match floor {
        Some((seed, floor)) => (seed, svg(&floor, label)),
        None => (HERO_SEED, hero(label)),
    }
}

#[cfg(test)]
mod dig_tests {
    use super::*;

    #[test]
    fn digging_is_deterministic_cached_and_bounded() {
        let (seed, first) = dig(4242, String::new());
        let (_, second) = dig(4242, String::new());
        assert_eq!(seed, 4242);
        assert_eq!(first.0, second.0);
        // Far more new seeds than the budget allows: the surplus gets the landing page floor, not a new one.
        let fallbacks = (10_000..10_000 + 3 * BUDGET)
            .filter(|s| dig(*s, String::new()).0 == HERO_SEED)
            .count();
        assert!(fallbacks >= 2 * BUDGET as usize - 5, "{fallbacks}");
    }
}
