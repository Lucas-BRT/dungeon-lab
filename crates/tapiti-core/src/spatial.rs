//! A conservative uniform-grid broad phase for bounded geometric objects.
//! The exact distance predicates remain at each call site.

use crate::geom::{Bounded, V};
use std::collections::HashMap;

pub(crate) struct SpatialHash {
    cell: f64,
    buckets: HashMap<(i64, i64), Vec<usize>>,
    wide: Vec<usize>,
    all: Vec<usize>,
}

impl SpatialHash {
    pub(crate) fn new(cell: f64) -> Self {
        Self {
            cell: if cell.is_finite() && cell > 0.0 {
                cell
            } else {
                256.0
            },
            buckets: HashMap::new(),
            wide: Vec::new(),
            all: Vec::new(),
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
        let Some(range) = self.range(lo, hi, 0.0) else {
            self.wide.push(id);
            return;
        };
        if Self::cell_count(range).is_none_or(|n| n > 4096) {
            self.wide.push(id);
            return;
        }
        let (x0, x1, y0, y1) = range;
        for x in x0..=x1 {
            for y in y0..=y1 {
                self.buckets.entry((x, y)).or_default().push(id);
            }
        }
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
        let mut out = Vec::with_capacity(self.wide.len() + 32);
        if !self.push_query(lo, hi, margin, &mut out) {
            return self.all_sorted();
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    pub(crate) fn query_region(&self, region: &[Bounded], margin: f64) -> Vec<usize> {
        let mut out = Vec::with_capacity(self.wide.len() + 32);
        for bound in region {
            if !self.push_query(bound.lo, bound.hi, margin, &mut out) {
                return self.all_sorted();
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    fn push_query(&self, lo: V, hi: V, margin: f64, out: &mut Vec<usize>) -> bool {
        let Some(range) = self.range(lo, hi, margin) else {
            return false;
        };
        if Self::cell_count(range).is_none_or(|n| n > 100_000) {
            return false;
        }
        out.extend_from_slice(&self.wide);
        let (x0, x1, y0, y1) = range;
        for x in x0..=x1 {
            for y in y0..=y1 {
                if let Some(ids) = self.buckets.get(&(x, y)) {
                    out.extend_from_slice(ids);
                }
            }
        }
        true
    }

    fn all_sorted(&self) -> Vec<usize> {
        let mut out = self.all.clone();
        out.sort_unstable();
        out.dedup();
        out
    }
}
