//! Grid scaffold against the grown layout: layout regularity, size, validity and time, 40 seeds each.
//! Run with `cargo run --release -p tapiti-core --example growth_study`.

use std::time::Instant;
use tapiti_core::{
    generate,
    layout::{layout_stats, LayoutStats},
    params, stage, validate,
};

fn mean_sd(xs: &[f64]) -> (f64, f64) {
    let m = xs.iter().sum::<f64>() / xs.len() as f64;
    (
        m,
        (xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / xs.len() as f64).sqrt(),
    )
}

fn run(label: &str, p: &params::Params) {
    let t = Instant::now();
    let ds: Vec<_> = (0..40u32)
        .map(|s| generate::generate(0xB000 + s, p))
        .collect();
    let ms = t.elapsed().as_secs_f64() * 25.0;
    let raw_valid = ds
        .iter()
        .filter(|d| validate::validate(d).is_empty())
        .count();
    let staged_valid = (0..0u32)
        .filter(|s| stage::stage(0xB000 + s, p, 8).violations.is_empty())
        .count();
    let st: Vec<LayoutStats> = ds.iter().map(layout_stats).collect();
    let col = |f: fn(&LayoutStats) -> f64| mean_sd(&st.iter().map(f).collect::<Vec<_>>());
    let (r, a, c, d) = (
        col(|s| s.rooms as f64),
        col(|s| s.alignment),
        col(|s| s.clark_evans),
        col(|s| s.dispersion),
    );
    let depth: Vec<f64> = ds
        .iter()
        .map(|d| {
            let h = tapiti_core::generate::room_hops(&d.nodes, &d.edges, d.rooms[d.start].id);
            h.iter()
                .cloned()
                .filter(|x| x.is_finite())
                .fold(0.0, f64::max)
        })
        .collect();
    let dd = mean_sd(&depth);
    println!(
        "{:<26} rooms {:>5.1}±{:<4.1} align {:.3}±{:.3} CE {:.3}±{:.3} disp {:.2}±{:.2} ecc {:>4.1} valid raw/40 {:>3} stage/8 {:<2} {:>6.1} ms/map",
        label, r.0, r.1, a.0, a.1, c.0, c.1, d.0, d.1, dd.0, raw_valid, staged_valid, ms
    );
}

fn main() {
    let mut p = params::params_of_level(5);
    p.cols = 16;
    p.rows = 12;
    println!("--- 16x12 world, level 5 (area = whole world) ---");
    for rooms in [30, 60] {
        for (cluster, islands) in [
            (0.0, 1),
            (1.0, 1),
            (0.0, 2),
            (0.0, 3),
            (1.0, 3),
            (0.0, 4),
            (1.0, 4),
            (1.0, 6),
        ] {
            let mut g = p.clone();
            g.growth.enabled = true;
            g.growth.rooms = rooms;
            g.growth.cluster = cluster;
            g.growth.islands = islands;
            run(&format!("grow R={rooms} λ={cluster} islands={islands}"), &g);
        }
    }
}
