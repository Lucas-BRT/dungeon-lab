//! How regular a floor's room layout looks: statistics of the room centers that are blind to how the layout was produced,
//! so a grid scaffold and a grown layout can be compared on the same scale.

use crate::generate::Dungeon;
use std::f64::consts::TAU;

#[derive(Clone, Copy, Debug, Default)]
pub struct LayoutStats {
    pub rooms: usize,
    /// Rayleigh concentration R ∈ [0, 1] of the centers' phase on the cell lattice (x then y, averaged). 1 = every center on the same lattice offset; ≈ 1/√n for unrelated points.
    pub alignment: f64,
    /// Clark–Evans index: mean nearest-neighbour distance over its Poisson expectation 0.5/√density. < 1 clustered, 1 random, > 1 regular (≤ 2.15).
    pub clark_evans: f64,
    /// Index of dispersion (variance / mean) of room counts in quadrats two cells wide. < 1 regular, 1 Poisson, > 1 clustered.
    pub dispersion: f64,
    /// Coefficient of variation of room areas.
    pub area_cv: f64,
}

pub fn layout_stats(d: &Dungeon) -> LayoutStats {
    let rooms: Vec<_> = d.rooms.iter().filter(|r| r.alive).collect();
    let n = rooms.len();
    if n < 3 {
        return LayoutStats { rooms: n, ..Default::default() };
    }
    let (cw, ch) = (d.params.cell_w.max(1.0), d.params.cell_h.max(1.0));
    let c: Vec<[f64; 2]> = rooms.iter().map(|r| r.center).collect();
    let nf = n as f64;

    let phase = |pitch: f64, axis: usize| {
        let (mut s, mut co) = (0.0, 0.0);
        for p in &c {
            let a = TAU * (p[axis] / pitch).rem_euclid(1.0);
            s += a.sin();
            co += a.cos();
        }
        (s * s + co * co).sqrt() / nf
    };
    let alignment = (phase(cw, 0) + phase(ch, 1)) / 2.0;

    // The world the rooms were laid out in, so a floor that huddles in one corner reads as clustered.
    let (lo, hi) = match (d.grid_x.as_slice(), d.grid_y.as_slice()) {
        ([x0, .., x1], [y0, .., y1]) => ([*x0, *y0], [*x1, *y1]),
        _ => {
            let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
            for r in &rooms {
                for k in 0..2 {
                    lo[k] = lo[k].min(r.bounds.lo[k]);
                    hi[k] = hi[k].max(r.bounds.hi[k]);
                }
            }
            (lo, hi)
        }
    };
    let area = ((hi[0] - lo[0]) * (hi[1] - lo[1])).max(1.0);
    let nn_sum: f64 = (0..n)
        .map(|i| {
            (0..n)
                .filter(|&j| j != i)
                .map(|j| (c[i][0] - c[j][0]).hypot(c[i][1] - c[j][1]))
                .fold(f64::MAX, f64::min)
        })
        .sum();
    let clark_evans = (nn_sum / nf) / (0.5 / (nf / area).sqrt());

    let (qw, qh) = (2.0 * cw, 2.0 * ch);
    let (nx, ny) = (
        (((hi[0] - lo[0]) / qw).ceil() as usize).max(1),
        (((hi[1] - lo[1]) / qh).ceil() as usize).max(1),
    );
    let mut counts = vec![0.0f64; nx * ny];
    for p in &c {
        let ix = (((p[0] - lo[0]) / qw) as usize).min(nx - 1);
        let iy = (((p[1] - lo[1]) / qh) as usize).min(ny - 1);
        counts[iy * nx + ix] += 1.0;
    }
    let (mean, var) = mean_var(&counts);
    let dispersion = if mean > 0.0 { var / mean } else { 0.0 };

    let areas: Vec<f64> = rooms.iter().map(|r| r.area).collect();
    let (am, av) = mean_var(&areas);
    let area_cv = if am > 0.0 { av.sqrt() / am } else { 0.0 };

    LayoutStats { rooms: n, alignment, clark_evans, dispersion, area_cv }
}

fn mean_var(xs: &[f64]) -> (f64, f64) {
    let m = xs.iter().sum::<f64>() / xs.len() as f64;
    (m, xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / xs.len() as f64)
}
