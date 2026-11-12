//! Prints a hash of every stage for levels 1 to 5 and seeds 1 to 30 (without the clock readings), to compare two builds:
//! `cargo run --release -p tapiti-core --example dump > a.txt`.
use tapiti_core::generate::generate;
use tapiti_core::params::{params_of_level, Layout};

fn main() {
    // The first 150 lines are the grid; the growth lines come after, so `head -150` compares the grid alone.
    for (level, layout, seeds) in (1..=5)
        .map(|l| (l, Layout::Grid, 30u32))
        .chain([1, 3, 5].map(|l| (l, Layout::Growth, 10u32)))
    {
        let mut p = params_of_level(level);
        p.layout = layout;
        for seed in 1..=seeds {
            let mut v = serde_json::to_value(generate(seed, &p)).unwrap();
            v.as_object_mut().unwrap().remove("phases");
            let text = serde_json::to_string(&v).unwrap();
            // FNV-1a
            let mut h: u64 = 0xcbf29ce484222325;
            for b in text.bytes() {
                h = (h ^ b as u64).wrapping_mul(0x100000001b3);
            }
            let tag = if layout == Layout::Growth {
                " growth"
            } else {
                ""
            };
            println!(
                "L{level}{tag} seed {seed}: {:016x} ({} bytes)",
                h,
                text.len()
            );
        }
    }
}
