use dungeon_lab::generate::generate;
use dungeon_lab::params::params_of_level;
use std::collections::BTreeMap;

/// Where a stage's time goes at each level (`cargo test --release profile -- --nocapture`).
#[test]
fn profile() {
    for level in [3usize, 5] {
        let p = params_of_level(level);
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
        println!("L{level}: {} (total {:.1} ms, {} threads)", line.join(" · "), sum.values().sum::<f64>(), dungeon_lab::par::threads());
    }
}
