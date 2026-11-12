//! Counts heap allocations of one generated stage per level (and a time median), to compare allocation strategies.
//! Run single-threaded so counts are stable: `cargo run --release -p tapiti-core --no-default-features --example alloc_profile`.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Instant;
use tapiti_core::{generate::generate, params::params_of_level};

struct Counting;
static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(l.size() as u64, Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(n as u64, Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static A: Counting = Counting;

fn main() {
    const SEEDS: u32 = 30;
    for level in 1..=5 {
        let p = params_of_level(level);
        generate(1, &p); // warm up
        let (a0, b0) = (ALLOCS.load(Relaxed), BYTES.load(Relaxed));
        let mut ms = Vec::new();
        for seed in 1..=SEEDS {
            let t = Instant::now();
            std::hint::black_box(generate(seed, &p));
            ms.push(t.elapsed().as_secs_f64() * 1e3);
        }
        ms.sort_by(|a, b| a.total_cmp(b));
        let (a, b) = (ALLOCS.load(Relaxed) - a0, BYTES.load(Relaxed) - b0);
        println!(
            "L{level}: allocs/stage {:>8} KiB/stage {:>8} median {:.2} ms p90 {:.2} ms",
            a / SEEDS as u64,
            b / SEEDS as u64 / 1024,
            ms[ms.len() / 2],
            ms[ms.len() * 9 / 10]
        );
    }
}
