//! Samples where heap allocations come from (every 20th, by the first `tapiti_core` frame outside the allocator), for the medium size.
//! `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only cargo run --release -p tapiti-core --no-default-features --example alloc_sites`
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::Mutex;
use tapiti_core::{
    generate::generate,
    params::{rich_of_size, Size},
};

struct Sampler;
static N: AtomicU64 = AtomicU64::new(0);
static SITES: Mutex<Option<HashMap<String, u64>>> = Mutex::new(None);
thread_local!(static BUSY: Cell<bool> = const { Cell::new(false) });

fn sample() {
    if !N.fetch_add(1, Relaxed).is_multiple_of(20) || BUSY.with(|b| b.replace(true)) {
        return;
    }
    let bt = std::backtrace::Backtrace::force_capture().to_string();
    let site = bt
        .lines()
        .filter(|l| {
            l.contains("at ./crates/tapiti-core/src") || l.contains("at crates/tapiti-core/src")
        })
        .map(|l| l.trim().trim_start_matches("at ").to_string())
        .next()
        .unwrap_or_else(|| "other".into());
    *SITES
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .entry(site)
        .or_default() += 1;
    BUSY.with(|b| b.set(false));
}
unsafe impl GlobalAlloc for Sampler {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        sample();
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        sample();
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static A: Sampler = Sampler;

fn main() {
    let p = rich_of_size(Size::Medium);
    for seed in 1..=10 {
        std::hint::black_box(generate(seed, &p));
    }
    let map = SITES.lock().unwrap().take().unwrap();
    let total: u64 = map.values().sum();
    let mut v: Vec<_> = map.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    for (s, n) in v.iter().take(30) {
        println!("{:5.1}%  {s}", *n as f64 * 100.0 / total as f64);
    }
}
