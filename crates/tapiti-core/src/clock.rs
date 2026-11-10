//! A millisecond clock that works natively and in the browser (where `std::time` has none): `performance.now()` through a tiny JS import.

#[cfg(target_arch = "wasm32")]
pub fn now_ms() -> f64 {
    js_now()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(
    inline_js = "export function js_now() { return performance.now() }"
)]
extern "C" {
    fn js_now() -> f64;
}

#[cfg(not(target_arch = "wasm32"))]
pub fn now_ms() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}
