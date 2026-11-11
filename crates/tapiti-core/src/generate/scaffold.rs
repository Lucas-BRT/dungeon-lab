//! §7.1 Scaffold: blocks on a grid, the big shapes first, then a share of the 1×1 cells dropped (the rest kept in one piece).

use super::*;

/// The grid the rooms are laid out on.
pub(super) struct Scaffold {
    pub(super) blocks: Vec<Block>,
    /// Per block: kept (`true`) or dropped to a leaf chamber's cell.
    pub(super) alive: Vec<bool>,
    pub(super) grid_x: Vec<f64>,
    pub(super) grid_y: Vec<f64>,
    /// Per block: the other blocks it shares an edge with, sorted.
    pub(super) neighbors: Vec<Vec<usize>>,
}

pub(super) fn build_scaffold(
    g: &mut Gen<'_>,
    progress: &mut Option<&mut dyn FnMut(GenerationEvent)>,
) -> Scaffold {
    let p = g.p;
    let (cols, rows, cell_w, cell_h, gap) =
        (p.cols.max(1), p.rows.max(1), p.cell_w, p.cell_h, p.gap);
    let total = (cols * rows) as usize;
    let mut blocks: Vec<Block> = Vec::new();
    let mut owner = vec![-1i32; total];
    let cells_of = |b: &Block| -> Vec<usize> {
        (0..b.w * b.h)
            .map(|i| ((b.r + i / b.w) * cols + b.c + i % b.w) as usize)
            .collect()
    };
    let shapes: [(i32, i32); 5] = [(2, 1), (1, 2), (2, 2), (3, 2), (2, 3)];
    let (mut tries, mut made, mut used) = (0, 0, 0);
    while tries < p.big * 12 && made < p.big {
        tries += 1;
        let (w, h) = shapes[(g.rng.next() * shapes.len() as f64).floor() as usize];
        let b = Block {
            c: (g.rng.next() * cols as f64).floor() as i32,
            r: (g.rng.next() * rows as f64).floor() as i32,
            w,
            h,
        };
        let fits =
            b.c + b.w <= cols && b.r + b.h <= rows && cells_of(&b).iter().all(|c| owner[*c] < 0);
        if !fits || (used + w * h) as f64 > total as f64 * 0.45 {
            continue;
        }
        blocks.push(b);
        for c in cells_of(&b) {
            owner[c] = (blocks.len() - 1) as i32;
        }
        made += 1;
        used += w * h;
    }
    for (c, slot) in owner.iter_mut().enumerate().take(total) {
        if *slot < 0 {
            let b = Block {
                c: c as i32 % cols,
                r: c as i32 / cols,
                w: 1,
                h: 1,
            };
            blocks.push(b);
            *slot = (blocks.len() - 1) as i32;
        }
    }
    let touching = |i: usize| -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        for cell in cells_of(&blocks[i]) {
            let (c, r) = (cell as i32 % cols, cell as i32 / cols);
            for (dc, dr) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                if c + dc < 0 || r + dr < 0 || c + dc >= cols || r + dr >= rows {
                    continue;
                }
                let o = owner[((r + dr) * cols + c + dc) as usize] as usize;
                if o != i && !out.contains(&o) {
                    out.push(o);
                }
            }
        }
        out.sort();
        out
    };
    let mut alive = vec![true; blocks.len()];
    let whole = |alive: &[bool]| -> bool {
        let first = alive.iter().position(|a| *a).unwrap();
        let mut seen = HashSet::from([first]);
        let mut todo = vec![first];
        let mut i = 0;
        while i < todo.len() {
            for n in touching(todo[i]) {
                if alive[n] && seen.insert(n) {
                    todo.push(n);
                }
            }
            i += 1;
        }
        seen.len() == alive.iter().filter(|a| **a).count()
    };
    let mut smalls: Vec<usize> = (0..blocks.len())
        .filter(|i| blocks[*i].w * blocks[*i].h == 1)
        .collect();
    for i in (1..smalls.len()).rev() {
        let j = (g.rng.next() * (i + 1) as f64).floor() as usize;
        smalls.swap(i, j);
    }
    let mut to_drop = (total as f64 * p.drop).floor() as i32;
    for &i in &smalls {
        if to_drop <= 0 {
            break;
        }
        alive[i] = false;
        if whole(&alive) {
            to_drop -= 1;
        } else {
            alive[i] = true;
        }
    }
    let mut grid_x: Vec<f64> = (0..=cols).map(|i| i as f64 * cell_w).collect();
    let mut grid_y: Vec<f64> = (0..=rows).map(|i| i as f64 * cell_h).collect();
    let requested_jitter = p.layout_jitter.clamp(0.0, 0.45);
    let minimum_cell_span = 2.0 * gap + 2.0 * (p.width / 2.0 + p.clear) + p.width;
    let jitter_limit = [cell_w, cell_h].into_iter().fold(0.45f64, |limit, span| {
        if span > 0.0 {
            limit.min(((span - minimum_cell_span) / (2.0 * span)).max(0.0))
        } else {
            0.0
        }
    });
    let jitter = requested_jitter.min(jitter_limit);
    if jitter + 1e-9 < requested_jitter {
        g.log.push(format!(
            "layout jitter limited to {:.3} (requested {:.3}) to preserve minimum room size",
            jitter, requested_jitter
        ));
    }
    if jitter > 0.0 {
        for x in grid_x.iter_mut().take(cols as usize).skip(1) {
            *x += (g.rng.next() * 2.0 - 1.0) * cell_w * jitter;
        }
        for y in grid_y.iter_mut().take(rows as usize).skip(1) {
            *y += (g.rng.next() * 2.0 - 1.0) * cell_h * jitter;
        }
    }
    report_progress(progress, || GenerationEvent::Layout {
        grid_x: grid_x.clone(),
        grid_y: grid_y.clone(),
        corridor_width: p.width,
    });
    let neighbors = (0..blocks.len()).map(touching).collect();
    Scaffold {
        blocks,
        alive,
        grid_x,
        grid_y,
        neighbors,
    }
}
