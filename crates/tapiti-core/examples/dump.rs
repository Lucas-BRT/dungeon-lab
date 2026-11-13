//! Prints a hash of every stage for the four sizes (all options on) and seeds 1 to 30 (without the clock readings), to compare two builds:
//! `cargo run --release -p tapiti-core --example dump > a.txt`.
use tapiti_core::generate::generate;
use tapiti_core::params::{rich_of_size, Layout, Size};

fn main() {
    // The first 120 lines are the grid; the growth lines come after, so `head -120` compares the grid alone.
    for (size, layout, seeds) in Size::ALL
        .map(|s| (s, Layout::Grid, 30u32))
        .into_iter()
        .chain([Size::Small, Size::Medium, Size::Large].map(|s| (s, Layout::Growth, 10u32)))
    {
        let mut p = rich_of_size(size);
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
                "{}{tag} seed {seed}: {:016x} ({} bytes)",
                size.name(),
                h,
                text.len()
            );
        }
    }
}
