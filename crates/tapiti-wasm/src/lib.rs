//! The Tapiti model (`docs/model.md`) as a Rust crate compiled to WebAssembly: rooms of any shape,
//! continuous doors, octilinear corridors (I, L, Z/U, T, X), leaf chambers, a validator and the graph's measures.
//!
//! The browser calls [`stage_json`]: the stage, its violations and its measures come back as one JSON string, so the page only draws.

use tapiti_core::{clock, generate, metrics, par, params, stage};

use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct Answer<'a> {
    floor: &'a generate::Floor,
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

/// A valid stage for `seed` and the Λ in `params_json`, as JSON: `{ floor, violations, draws, metrics, ms }`.
#[wasm_bindgen]
pub fn stage_json(seed: u32, params_json: &str) -> Result<String, JsError> {
    let p: params::Params =
        serde_json::from_str(params_json).map_err(|e| JsError::new(&format!("bad Λ: {e}")))?;
    let t0 = clock::now_ms();
    let s = stage::stage(seed, &p, 8);
    let ms = clock::now_ms() - t0;
    let m = metrics::metrics_of(&s.d);
    serde_json::to_string(&Answer {
        floor: &s.d,
        violations: &s.violations,
        draws: s.draws,
        metrics: m,
        ms,
    })
    .map_err(|e| JsError::new(&e.to_string()))
}

/// How many threads the module computes with (1 for the single-threaded build).
#[wasm_bindgen]
pub fn threads() -> u32 {
    par::threads() as u32
}

/// Times `count` stages of one level from the seeds `from..from + count`, one after another, and returns `{ invalid, ms, threads }`.
/// The page's worker pool gives each worker a slice of the seeds (see `apps/site/assets/lab/lab.js`); natively the stages of a slice also split over rayon's threads.
#[wasm_bindgen]
pub fn bench(level: u32, from: u32, count: u32) -> String {
    let p = params::params_of_level(level as usize);
    let seeds: Vec<u32> = (from..from + count).collect();
    let t0 = clock::now_ms();
    let invalid: usize = par::pmap(&seeds, |seed| {
        usize::from(!stage::stage(*seed, &p, 8).violations.is_empty())
    })
    .iter()
    .sum();
    format!(
        "{{\"invalid\":{},\"ms\":{},\"threads\":{}}}",
        invalid,
        clock::now_ms() - t0,
        par::threads()
    )
}
