//! Mulberry32, the same generator the game uses (`engine/rng.ts`), so a seed means the same stream here and there.

pub trait Rand {
    fn next(&mut self) -> f64;
    fn range(&mut self, min: f64, max: f64) -> f64 {
        min + self.next() * (max - min)
    }
}

pub struct Mulberry(pub u32);

impl Rand for Mulberry {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let a = self.0;
        let mut t = a;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        ((t ^ (t >> 14)) as f64) / 4294967296.0
    }
}

/// A source that always answers the same value (the star's fallback fit).
pub struct Const(pub f64);

impl Rand for Const {
    fn next(&mut self) -> f64 {
        self.0
    }
}
