//! Layout regularity of the grid scaffold across seeds, grid sizes and `layout_jitter`.
//! Run with `cargo run --release -p tapiti-core --example layout_study`.

use tapiti_core::{generate, layout::layout_stats, params};

fn mean_sd(xs: &[f64]) -> (f64, f64) {
    let m = xs.iter().sum::<f64>() / xs.len() as f64;
    (m, (xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / xs.len() as f64).sqrt())
}

fn main() {
    println!("reference: alignment ≈ 1/√n for unrelated points; Clark-Evans 1 random, 2.15 hex lattice; dispersion 1 Poisson");
    println!("{:<12} {:>6} {:>6} {:>14} {:>14} {:>14} {:>12}", "grid", "jitter", "rooms", "alignment", "clark-evans", "dispersion", "area-cv");
    for (cols, rows, scale) in [(10, 8, 1.0), (16, 12, 1.0), (10, 8, 2.0)] {
        for jitter in [0.0, 0.15, 0.3, 0.45] {
            let mut p = params::params_of_level(5);
            p.cols = cols;
            p.rows = rows;
            p.layout_jitter = jitter;
            p.cell_w *= scale;
            p.cell_h *= scale;
            let all: Vec<_> = (0..100u32).map(|s| layout_stats(&generate::generate(0xA000 + s, &p))).collect();
            let col = |f: fn(&tapiti_core::layout::LayoutStats) -> f64| {
                mean_sd(&all.iter().map(f).collect::<Vec<_>>())
            };
            let (r, a, c, d, v) = (
                col(|s| s.rooms as f64),
                col(|s| s.alignment),
                col(|s| s.clark_evans),
                col(|s| s.dispersion),
                col(|s| s.area_cv),
            );
            println!(
                "{:<12} {:>6.2} {:>6.0} {:>8.3}±{:<5.3} {:>8.3}±{:<5.3} {:>8.3}±{:<5.3} {:>6.3}±{:<5.3}",
                format!("{cols}x{rows}{}", if scale > 1.0 { format!(" c×{scale}") } else { String::new() }), jitter, r.0, a.0, a.1, c.0, c.1, d.0, d.1, v.0, v.1
            );
        }
    }
}
