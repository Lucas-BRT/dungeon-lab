//! The parameter vector Λ of the dungeon model (§8): one generator, a different Λ per level. The JSON field names are the ones the
//! TypeScript prototype used (camelCase), so a Λ copied from either lab loads in the other.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeClass {
    #[serde(rename = "rect")]
    Rect,
    #[serde(rename = "disc")]
    Disc,
    #[serde(rename = "oct")]
    Oct,
    L,
    #[serde(rename = "star")]
    Star,
}

pub const CLASSES: [ShapeClass; 5] = [ShapeClass::Rect, ShapeClass::Disc, ShapeClass::Oct, ShapeClass::L, ShapeClass::Star];

impl ShapeClass {
    pub fn index(self) -> usize {
        CLASSES.iter().position(|c| *c == self).unwrap()
    }
    pub fn name(self) -> &'static str {
        ["rect", "disc", "oct", "L", "star"][self.index()]
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ByClass {
    pub rect: f64,
    pub disc: f64,
    pub oct: f64,
    #[serde(rename = "L")]
    pub l: f64,
    pub star: f64,
}

impl ByClass {
    pub fn get(&self, c: ShapeClass) -> f64 {
        [self.rect, self.disc, self.oct, self.l, self.star][c.index()]
    }
    const fn new(rect: f64, disc: f64, oct: f64, l: f64, star: f64) -> Self {
        Self { rect, disc, oct, l, star }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Proximity {
    Rng,
    Gabriel,
    Near,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct LeafRoles {
    pub trap: f64,
    pub prize: f64,
    pub empty: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Params {
    // Scaffold (§7.1)
    pub cols: i32,
    pub rows: i32,
    pub cell_w: f64,
    pub cell_h: f64,
    pub gap: f64,
    pub big: i32,
    pub drop: f64,
    // Shape classes (§7.2)
    pub weights: ByClass,
    pub max_of: ByClass,
    pub star_cells: i32,
    pub homophily: f64,
    // Directions (§1, §3.1)
    pub m: i32,
    pub disc_ports: i32,
    pub port_step: f64,
    // Corridors (§4)
    pub width: f64,
    pub clear: f64,
    pub exit: f64,
    pub seg: f64,
    pub min_len: f64,
    pub max_bends: i32,
    pub p_i: f64,
    pub p_l: f64,
    pub p_z: f64,
    pub p_t: f64,
    pub p_x: f64,
    pub cost_bend: f64,
    pub cost_len: f64,
    pub cost45: f64,
    pub cost_facing: f64,
    /// The mixing matrix M (§7.3.1), keys `a-b` in CLASSES order; 0 forbids, missing is 1.
    pub mix: BTreeMap<String, f64>,
    // Candidate pairs (§7.3), density (§6.1), roles (V5)
    pub proximity: Proximity,
    pub near: f64,
    pub delta: f64,
    pub stairs_doors: i32,
    pub min_hops: i32,
    pub ambush: i32,
    // Leaf chambers (§7.6)
    pub leaves: i32,
    pub leaf_classes: ByClass,
    pub leaf_roles: LeafRoles,
    /// Junction conductance ω_h (§4.2.1): null merges the arms into one space. Shown, not used by the generator.
    pub junction: Option<f64>,
}

/// The pair key of the mixing matrix, the same whatever the order.
pub fn mix_key(a: ShapeClass, b: ShapeClass) -> String {
    let (x, y) = if a.index() <= b.index() { (a, b) } else { (b, a) };
    format!("{}-{}", x.name(), y.name())
}

fn full_mix() -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    for a in CLASSES {
        for b in CLASSES {
            out.insert(mix_key(a, b), 1.0);
        }
    }
    out
}

fn base() -> Params {
    Params {
        cols: 4,
        rows: 3,
        cell_w: 270.0,
        cell_h: 230.0,
        gap: 56.0,
        big: 2,
        drop: 0.15,
        weights: ByClass::new(1.0, 0.0, 0.0, 0.0, 0.0),
        max_of: ByClass::new(99.0, 99.0, 99.0, 99.0, 99.0),
        star_cells: 2,
        homophily: 0.0,
        m: 4,
        disc_ports: 8,
        port_step: 12.0,
        width: 44.0,
        clear: 12.0,
        exit: 24.0,
        seg: 24.0,
        min_len: 112.0,
        max_bends: 2,
        p_i: 1.0,
        p_l: 1.0,
        p_z: 0.6,
        p_t: 0.0,
        p_x: 0.0,
        cost_bend: 1.0,
        cost_len: 0.004,
        cost45: 0.5,
        cost_facing: 0.5,
        mix: full_mix(),
        proximity: Proximity::Rng,
        near: 520.0,
        delta: 0.15,
        stairs_doors: 1,
        min_hops: 3,
        ambush: 1,
        leaves: 0,
        leaf_classes: ByClass::new(1.0, 0.0, 0.0, 0.0, 0.0),
        leaf_roles: LeafRoles { trap: 1.0, prize: 1.0, empty: 1.0 },
        junction: None,
    }
}

/// Λ by level (the doc's §8 table, to be tuned here).
pub fn params_of_level(level: usize) -> Params {
    let mut p = base();
    match level.clamp(1, 5) {
        1 => {}
        2 => {
            p.cols = 5;
            p.weights = ByClass::new(0.8, 0.0, 0.0, 0.2, 0.0);
            p.delta = 0.25;
            p.p_t = 0.1;
            p.leaves = 1;
        }
        3 => {
            p.cols = 5;
            p.rows = 4;
            p.m = 8;
            p.weights = ByClass::new(0.6, 0.2, 0.0, 0.2, 0.0);
            p.proximity = Proximity::Gabriel;
            p.delta = 0.35;
            p.p_t = 0.15;
            p.p_x = 0.05;
            p.leaves = 2;
            p.leaf_classes = ByClass::new(1.0, 1.0, 0.0, 0.0, 0.0);
            p.big = 3;
        }
        4 => {
            p.cols = 6;
            p.rows = 4;
            p.m = 8;
            p.weights = ByClass::new(0.4, 0.2, 0.1, 0.2, 0.1);
            p.proximity = Proximity::Gabriel;
            p.delta = 0.45;
            p.p_t = 0.2;
            p.p_x = 0.1;
            p.stairs_doors = 2;
            p.leaves = 3;
            p.leaf_classes = ByClass::new(1.0, 1.0, 1.0, 0.0, 0.0);
            p.big = 3;
        }
        _ => {
            p.cols = 6;
            p.rows = 5;
            p.m = 8;
            p.weights = ByClass::new(0.3, 0.2, 0.15, 0.2, 0.15);
            p.proximity = Proximity::Near;
            p.delta = 0.55;
            p.p_t = 0.25;
            p.p_x = 0.15;
            p.stairs_doors = 2;
            p.min_hops = 4;
            p.ambush = 2;
            p.leaves = 4;
            p.leaf_classes = ByClass::new(1.0, 1.0, 1.0, 0.0, 0.0);
            p.big = 4;
        }
    }
    p
}
