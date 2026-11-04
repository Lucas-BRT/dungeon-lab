//! The dungeon model (`docs/dungeon-model.md` of the game's repository) as a Rust crate compiled to WebAssembly: rooms of any shape,
//! continuous doors, octilinear corridors (I, L, Z/U, T, X), leaf chambers, a validator and the graph's measures.
//!
//! The browser calls [`stage_json`]: the stage, its violations and its measures come back as one JSON string, so the page only draws.

pub mod generate;
pub mod geom;
pub mod metrics;
pub mod params;
pub mod rng;
pub mod stage;
pub mod validate;

use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct Answer<'a> {
    dungeon: &'a generate::Dungeon,
    violations: &'a [String],
    draws: u32,
    metrics: metrics::Metrics,
    /// Milliseconds the generation took inside the module (when the host gave a clock), for the page's panel.
    ms: f64,
}

/// Λ of a level (1 to 5) as JSON.
#[wasm_bindgen]
pub fn default_params(level: u32) -> String {
    serde_json::to_string(&params::params_of_level(level as usize)).unwrap()
}

/// A valid stage for `seed` and the Λ in `params_json`, as JSON: `{ dungeon, violations, draws, metrics, ms }`.
#[wasm_bindgen]
pub fn stage_json(seed: u32, params_json: &str) -> Result<String, JsError> {
    let p: params::Params = serde_json::from_str(params_json).map_err(|e| JsError::new(&format!("bad Λ: {e}")))?;
    let t0 = now_ms();
    let s = stage::stage(seed, &p, 8);
    let ms = now_ms() - t0;
    let m = metrics::metrics_of(&s.d);
    serde_json::to_string(&Answer { dungeon: &s.d, violations: &s.violations, draws: s.draws, metrics: m, ms }).map_err(|e| JsError::new(&e.to_string()))
}

/// Generates `n` stages of one level from consecutive seeds and returns `{ invalid, ms }` (a benchmark that measures the module alone).
#[wasm_bindgen]
pub fn bench(level: u32, n: u32) -> String {
    let p = params::params_of_level(level as usize);
    let t0 = now_ms();
    let invalid = (1..=n).filter(|seed| !stage::stage(*seed, &p, 8).violations.is_empty()).count();
    format!("{{\"invalid\":{},\"ms\":{}}}", invalid, now_ms() - t0)
}

#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    js_sys_now()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = "export function js_sys_now() { return performance.now() }")]
extern "C" {
    fn js_sys_now() -> f64;
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}
