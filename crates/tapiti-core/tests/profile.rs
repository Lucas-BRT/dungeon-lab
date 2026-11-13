use std::collections::BTreeMap;
use tapiti_core::generate::generate;
use tapiti_core::params::{rich_of_size, Size};

/// Where a stage's time goes at each size (`cargo test --release -p tapiti-core profile -- --nocapture`).
#[test]
fn profile() {
    for size in [Size::Small, Size::Medium] {
        let p = rich_of_size(size);
        let n = 30u32;
        let mut sum: BTreeMap<String, f64> = BTreeMap::new();
        let mut order: Vec<String> = Vec::new();
        for seed in 1..=n {
            for (name, ms) in generate(seed, &p).phases {
                if !order.contains(&name) {
                    order.push(name.clone());
                }
                *sum.entry(name).or_insert(0.0) += ms / n as f64;
            }
        }
        let line: Vec<String> = order.iter().map(|k| format!("{k} {:.2}", sum[k])).collect();
        println!(
            "{}: {} (total {:.1} ms, {} threads)",
            size.name(),
            line.join(" · "),
            sum.values().sum::<f64>(),
            tapiti_core::par::threads()
        );
    }
}
