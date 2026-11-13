//! A conservative uniform-grid broad phase for bounded geometric objects.
//! The exact distance predicates remain at each call site.

use crate::geom::{Bounded, V};

pub(crate) struct SpatialHash {
    cell: f64,
    /// Dense grid of buckets over the cells ever filled: cell (x, y) is `buckets[(y - y0) * nx + (x - x0)]`.
    buckets: Vec<Vec<usize>>,
    x0: i64,
    y0: i64,
    nx: i64,
    ny: i64,
    wide: Vec<usize>,
    all: Vec<usize>,
    max_id: usize,
}

impl SpatialHash {
    pub(crate) fn new(cell: f64) -> Self {
        Self {
            cell: if cell.is_finite() && cell > 0.0 {
                cell
            } else {
                256.0
            },
            buckets: Vec::new(),
            x0: 0,
            y0: 0,
            nx: 0,
            ny: 0,
            wide: Vec::new(),
            all: Vec::new(),
            max_id: 0,
        }
    }

    fn range(&self, lo: V, hi: V, margin: f64) -> Option<(i64, i64, i64, i64)> {
        if lo.iter().chain(hi.iter()).any(|v| !v.is_finite()) || !margin.is_finite() {
            return None;
        }
        let margin = margin.max(0.0);
        let index = |v: f64| (v / self.cell).floor() as i64;
        let x0 = index(lo[0] - margin).saturating_sub(1);
        let y0 = index(lo[1] - margin).saturating_sub(1);
        let x1 = index(hi[0] + margin).saturating_add(1);
        let y1 = index(hi[1] + margin).saturating_add(1);
        Some((x0, x1, y0, y1))
    }

    fn cell_count(range: (i64, i64, i64, i64)) -> Option<i128> {
        let (x0, x1, y0, y1) = range;
        let nx = (x1 as i128) - (x0 as i128) + 1;
        let ny = (y1 as i128) - (y0 as i128) + 1;
        if nx <= 0 || ny <= 0 {
            None
        } else {
            nx.checked_mul(ny)
        }
    }

    pub(crate) fn insert(&mut self, id: usize, lo: V, hi: V) {
        self.all.push(id);
        self.max_id = self.max_id.max(id);
        let Some(range) = self.range(lo, hi, 0.0) else {
            self.wide.push(id);
            return;
        };
        if Self::cell_count(range).is_none_or(|n| n > 4096) {
            self.wide.push(id);
            return;
        }
        let (x0, x1, y0, y1) = range;
        if !self.grow(x0, x1, y0, y1) {
            self.wide.push(id);
            return;
        }
        for y in y0..=y1 {
            let row = ((y - self.y0) * self.nx - self.x0) as isize;
            for x in x0..=x1 {
                self.buckets[(row + x as isize) as usize].push(id);
            }
        }
    }

    /// Makes the dense grid cover the cells x0..=x1, y0..=y1; false when that would take too many cells (the item goes in `wide`).
    fn grow(&mut self, x0: i64, x1: i64, y0: i64, y1: i64) -> bool {
        let empty = self.buckets.is_empty();
        let (nx0, ny0) = if empty {
            (x0, y0)
        } else {
            (self.x0.min(x0), self.y0.min(y0))
        };
        let (nx1, ny1) = if empty {
            (x1, y1)
        } else {
            (
                (self.x0 + self.nx - 1).max(x1),
                (self.y0 + self.ny - 1).max(y1),
            )
        };
        let (nnx, nny) = (nx1 - nx0 + 1, ny1 - ny0 + 1);
        if nnx.saturating_mul(nny) > 1 << 20 {
            return false;
        }
        if empty || nx0 != self.x0 || ny0 != self.y0 || nnx != self.nx || nny != self.ny {
            let mut fresh = vec![Vec::new(); (nnx * nny) as usize];
            for (i, b) in std::mem::take(&mut self.buckets).into_iter().enumerate() {
                let (x, y) = (self.x0 + i as i64 % self.nx, self.y0 + i as i64 / self.nx);
                fresh[((y - ny0) * nnx + (x - nx0)) as usize] = b;
            }
            self.buckets = fresh;
            (self.x0, self.y0, self.nx, self.ny) = (nx0, ny0, nnx, nny);
        }
        true
    }

    pub(crate) fn insert_bound(&mut self, id: usize, bound: &Bounded) {
        self.insert(id, bound.lo, bound.hi);
    }

    pub(crate) fn insert_bounds(&mut self, id: usize, lo: V, hi: V) {
        self.insert(id, lo, hi);
    }

    pub(crate) fn insert_region(&mut self, id: usize, region: &[Bounded]) {
        for bound in region {
            self.insert_bound(id, bound);
        }
    }

    pub(crate) fn query_bounds(&self, lo: V, hi: V, margin: f64) -> Vec<usize> {
        self.query_parts(std::iter::once((lo, hi)), margin)
    }

    pub(crate) fn query_region(&self, region: &[Bounded], margin: f64) -> Vec<usize> {
        self.query_parts(region.iter().map(|b| (b.lo, b.hi)), margin)
    }

    /// Ids whose cells meet any of the boxes (grown by `margin`), ascending and unique.
    fn query_parts(&self, parts: impl Iterator<Item = (V, V)>, margin: f64) -> Vec<usize> {
        // With small ids the ids are marked in a bitset on the stack and read back in order: no list of duplicates, no sort.
        let small = self.max_id < 1024;
        let mut bits = [0u64; 16];
        let mut out = Vec::new();
        for (lo, hi) in parts {
            let Some((x0, x1, y0, y1)) = self.cells(lo, hi, margin) else {
                return self.all_sorted();
            };
            let mut take = |ids: &[usize]| {
                if small {
                    for &id in ids {
                        bits[id >> 6] |= 1 << (id & 63);
                    }
                } else {
                    out.extend_from_slice(ids);
                }
            };
            take(&self.wide);
            if self.buckets.is_empty() {
                continue;
            }
            let (x0, x1) = (x0.max(self.x0), x1.min(self.x0 + self.nx - 1));
            let (y0, y1) = (y0.max(self.y0), y1.min(self.y0 + self.ny - 1));
            for y in y0..=y1 {
                let row = (y - self.y0) * self.nx - self.x0;
                for x in x0..=x1 {
                    take(&self.buckets[(row + x) as usize]);
                }
            }
        }
        if !small {
            out.sort_unstable();
            out.dedup();
            return out;
        }
        for (w, &word) in bits[..=self.max_id >> 6].iter().enumerate() {
            let mut word = word;
            while word != 0 {
                out.push(w * 64 + word.trailing_zeros() as usize);
                word &= word - 1;
            }
        }
        out
    }

    /// The cells a box grown by `margin` covers, or None when it is not finite or covers too many.
    fn cells(&self, lo: V, hi: V, margin: f64) -> Option<(i64, i64, i64, i64)> {
        let range = self.range(lo, hi, margin)?;
        if Self::cell_count(range).is_none_or(|n| n > 100_000) {
            return None;
        }
        Some(range)
    }

    fn all_sorted(&self) -> Vec<usize> {
        let mut out = self.all.clone();
        out.sort_unstable();
        out.dedup();
        out
    }
}
