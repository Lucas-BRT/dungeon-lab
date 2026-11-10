//! Prints a hash of every stage for levels 1 to 5 and seeds 1 to 30 (without the clock readings), to compare two builds:
//! `cargo run --release -p tapiti-core --example dump > a.txt`.
use tapiti_core::generate::generate;
use tapiti_core::params::params_of_level;

fn main() {
    for level in 1..=5 {
        let p = params_of_level(level);
        for seed in 1..=30u32 {
            let mut v = serde_json::to_value(generate(seed, &p)).unwrap();
            v.as_object_mut().unwrap().remove("phases");
            let text = serde_json::to_string(&v).unwrap();
            // FNV-1a
            let mut h: u64 = 0xcbf29ce484222325;
            for b in text.bytes() {
                h = (h ^ b as u64).wrapping_mul(0x100000001b3);
            }
            println!("L{level} seed {seed}: {:016x} ({} bytes)", h, text.len());
        }
    }
}
