use crate::generate::{generate, Dungeon};
use crate::params::Params;
use crate::validate::validate;

pub struct Stage {
    pub d: Dungeon,
    pub violations: Vec<String>,
    pub draws: u32,
}

/// A valid stage for `seed` and Λ: the generator keeps every constraint it can by construction; the few it can't promise on every draw
/// (the stairs' distance from the start on a small floor, V5) are checked by the validator, and the draw is redone from a seed derived
/// from the first, up to `attempts` times. Returns the stage, its violations (empty when valid) and how many draws it took.
pub fn stage(seed: u32, p: &Params, attempts: u32) -> Stage {
    let mut last: Option<Stage> = None;
    for i in 0..attempts.max(1) {
        let d = generate(seed.wrapping_add(i.wrapping_mul(7919)), p);
        let violations = validate(&d);
        let done = violations.is_empty();
        last = Some(Stage { d, violations, draws: i + 1 });
        if done {
            break;
        }
    }
    last.unwrap()
}
