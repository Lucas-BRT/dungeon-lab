use std::collections::BTreeMap;
use tapiti_core::{
    generate::generate,
    params::{self, Layout, TopologyProfile},
    stage::stage,
    validate::validate,
};
fn run(label: &str, p: &params::Params) {
    let n = 100u32;
    let (mut raw_ok, mut st_ok, mut draws, mut rooms, mut grid_rooms) =
        (0, 0, 0u32, 0usize, 0usize);
    let mut logs: BTreeMap<String, u32> = BTreeMap::new();
    let mut viol: BTreeMap<String, u32> = BTreeMap::new();
    let mut gp = p.clone();
    gp.layout = Layout::Grid;
    for seed in 1..=n {
        let d = generate(seed, p);
        let v = validate(&d);
        if v.is_empty() {
            raw_ok += 1
        } else {
            for m in v.iter().take(1) {
                *viol.entry(m.chars().take(60).collect()).or_default() += 1
            }
        }
        rooms += d.rooms.iter().filter(|r| r.alive).count();
        grid_rooms += generate(seed, &gp).rooms.iter().filter(|r| r.alive).count();
        for m in &d.log {
            let k: String = m.chars().filter(|c| !c.is_ascii_digit()).take(55).collect();
            *logs.entry(k).or_default() += 1
        }
        let s = stage(seed, p, 8);
        draws += s.draws;
        if s.violations.is_empty() {
            st_ok += 1
        }
    }
    println!("{label:<34} raw valid {raw_ok:>3}/100  stage(8) valid {st_ok:>3}/100  draws {:.2}  rooms {:.0}% of grid", draws as f64 / n as f64, 100.0 * rooms as f64 / grid_rooms as f64);
    for (k, c) in &viol {
        println!("      violation {c:>3}x {k}")
    }
    for (k, c) in logs.iter().filter(|(k, _)| !k.contains("does not fit")) {
        println!("      log {c:>3}x {k}")
    }
}
fn main() {
    for level in [1, 3, 5] {
        let mut p = params::params_of_level(level);
        p.layout = Layout::Growth;
        run(&format!("level {level} default grid"), &p);
    }
    for (name, prof) in [
        ("classic", TopologyProfile::Classic),
        ("backbone", TopologyProfile::Backbone),
        ("hub", TopologyProfile::Hub),
        ("redundantExit", TopologyProfile::RedundantExit),
        ("longLoop", TopologyProfile::LongLoop),
    ] {
        let mut p = params::params_of_level(5);
        p.layout = Layout::Growth;
        p.topology.profile = prof;
        run(&format!("level 5 profile {name}"), &p);
    }
    let mut p = params::params_of_level(5);
    p.layout = Layout::Growth;
    p.leaves = 4;
    let d = generate(1, &p);
    println!(
        "leaf rooms with growth, leaves=4: {}",
        d.rooms.iter().filter(|r| r.leaf).count()
    );
    let mut p = params::params_of_level(5);
    p.layout = Layout::Growth;
    p.cols = 16;
    p.rows = 12;
    run("level 5 16x12", &p);
}
