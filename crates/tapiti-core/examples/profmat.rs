//! `profmat <grid|growth> <profile> [seeds]`: validity of a topology profile, first draw and with eight draws, seeds in parallel.
use rayon::prelude::*;
use tapiti_core::{
    generate::generate,
    params::{params_of_size, rich_of_size, Layout, Size, TopologyProfile},
    stage::stage,
    validate::validate,
};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let layout = if a[1] == "grid" {
        Layout::Grid
    } else {
        Layout::Growth
    };
    let prof = match a[2].as_str() {
        "backbone" => TopologyProfile::Backbone,
        "hub" => TopologyProfile::Hub,
        "redundantExit" => TopologyProfile::RedundantExit,
        "longLoop" => TopologyProfile::LongLoop,
        _ => TopologyProfile::Classic,
    };
    let seeds: u32 = a.get(3).map_or(100, |s| s.parse().unwrap());
    let mut p = rich_of_size(Size::Medium);
    p.exit_min_hops = params_of_size(Size::Medium).exit_min_hops;
    p.layout = layout;
    p.topology.profile = prof;
    let t = std::time::Instant::now();
    let raw = (1..=seeds)
        .into_par_iter()
        .filter(|s| validate(&generate(*s, &p)).is_empty())
        .count();
    let st = (1..=seeds)
        .into_par_iter()
        .filter(|s| stage(*s, &p, 8).violations.is_empty())
        .count();
    println!(
        "{:?} {:14} raw {raw:3}/{seeds} stage(8) {st:3}/{seeds}  ({:.1}s)",
        layout,
        a[2],
        t.elapsed().as_secs_f64()
    );
}
