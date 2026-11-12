//! The parameter vector Λ of the Tapiti model (§8): one generator, one Λ that a size preset or the user fills in. The JSON field names are camelCase,
//! so a Λ copied from one frontend (desktop or web) loads in the other.

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

pub const CLASSES: [ShapeClass; 5] = [
    ShapeClass::Rect,
    ShapeClass::Disc,
    ShapeClass::Oct,
    ShapeClass::L,
    ShapeClass::Star,
];

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
        Self {
            rect,
            disc,
            oct,
            l,
            star,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Proximity {
    Rng,
    Gabriel,
    Near,
}

fn yes() -> bool {
    true
}

/// How the rooms are laid out: one per grid block, or grown one from another off a parent's door (prototype).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Layout {
    #[default]
    Grid,
    Growth,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TopologyProfile {
    #[default]
    Classic,
    Backbone,
    Hub,
    RedundantExit,
    LongLoop,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TopologySpec {
    pub profile: TopologyProfile,
    pub min_branches: usize,
    pub min_branch_depth: usize,
    pub min_exit_routes: usize,
    pub min_cycle_length: usize,
    pub min_hub_degree: usize,
}

impl Default for TopologySpec {
    fn default() -> Self {
        Self {
            profile: TopologyProfile::Classic,
            min_branches: 0,
            min_branch_depth: 0,
            min_exit_routes: 1,
            min_cycle_length: 0,
            min_hub_degree: 0,
        }
    }
}

impl TopologySpec {
    /// The requirements of the profile and the explicit minimums. Side branches and independent routes are measured from the
    /// path between the entrance and the exit, so without both they cannot hold and are left out.
    pub fn requirements_with(self, anchors: bool) -> Self {
        let mut out = self.requirements();
        if !anchors {
            out.min_branches = 0;
            out.min_branch_depth = 0;
            out.min_exit_routes = 1;
        }
        out
    }

    pub fn requirements(self) -> Self {
        let mut out = self;
        if out.min_branch_depth > 0 {
            out.min_branches = out.min_branches.max(1);
        }
        match self.profile {
            TopologyProfile::Classic => {}
            TopologyProfile::Backbone => {
                out.min_branches = out.min_branches.max(2);
                out.min_branch_depth = out.min_branch_depth.max(1);
            }
            TopologyProfile::Hub => out.min_hub_degree = out.min_hub_degree.max(3),
            TopologyProfile::RedundantExit => out.min_exit_routes = out.min_exit_routes.max(2),
            TopologyProfile::LongLoop => out.min_cycle_length = out.min_cycle_length.max(4),
        }
        out
    }
}

/// Where the first room of the grown layout stands: the middle of the world, or against one of its borders (north is the top of the map).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GrowthStart {
    #[default]
    Center,
    North,
    South,
    East,
    West,
}

/// Grown layout (prototype): rooms are placed one after another off a parent's door, instead of one per grid block.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GrowthSpec {
    /// Where the first room (the entrance, when there is one) stands.
    pub start: GrowthStart,
    /// Rooms to grow; 0 takes the number of live blocks the grid scaffold would have given.
    pub rooms: i32,
    /// Each room spawns 1..=branch children.
    pub branch: i32,
    /// Deepest generation a room may be in (the first room is 0).
    pub depth: i32,
    /// Placement bias: weight e^(cluster·n) with n the rooms near the spot. > 0 clumps, < 0 spreads out.
    pub cluster: f64,
    /// Mean corridor reach beyond the minimum, in cells.
    pub reach: f64,
    /// Clumps to split the rooms into; a bridge corridor three to six cells long starts each one after the first.
    pub islands: i32,
}

impl Default for GrowthSpec {
    fn default() -> Self {
        Self {
            start: GrowthStart::Center,
            rooms: 0,
            branch: 3,
            depth: 8,
            cluster: 0.0,
            reach: 1.0,
            islands: 1,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Params {
    /// The layout mode; the `growth` settings apply only to `growth`.
    #[serde(default)]
    pub layout: Layout,
    // Scaffold (§7.1)
    pub cols: i32,
    pub rows: i32,
    pub cell_w: f64,
    pub cell_h: f64,
    pub gap: f64,
    pub big: i32,
    pub drop: f64,
    /// Fractional displacement of interior grid lines; 0 preserves a uniform grid.
    #[serde(default)]
    pub layout_jitter: f64,
    /// Rotate polygonal rooms by a direction from the active direction alphabet.
    #[serde(default)]
    pub rotate_rooms: bool,
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
    #[serde(default)]
    pub p_w: f64,
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
    /// Whether the floor has an entrance room.
    #[serde(default = "yes")]
    pub entrance: bool,
    /// Whether the floor has an exit room (only with an entrance).
    #[serde(default = "yes")]
    pub exit_room: bool,
    pub exit_doors: i32,
    pub exit_min_hops: i32,
    // Leaf chambers (§7.6)
    pub leaves: i32,
    pub leaf_classes: ByClass,
    /// Junction conductance ω_h (§4.2.1): null merges the arms into one space. Shown, not used by the generator.
    pub junction: Option<f64>,
    #[serde(default)]
    pub topology: TopologySpec,
    #[serde(default)]
    pub growth: GrowthSpec,
}

/// The pair key of the mixing matrix, the same whatever the order.
pub fn mix_key(a: ShapeClass, b: ShapeClass) -> String {
    let (x, y) = if a.index() <= b.index() {
        (a, b)
    } else {
        (b, a)
    };
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
        layout: Layout::Grid,
        cols: 4,
        rows: 3,
        cell_w: 270.0,
        cell_h: 230.0,
        gap: 56.0,
        big: 2,
        drop: 0.15,
        layout_jitter: 0.0,
        rotate_rooms: false,
        weights: ByClass::new(1.0, 0.0, 0.0, 0.0, 0.0),
        max_of: ByClass::new(10000.0, 10000.0, 10000.0, 10000.0, 10000.0),
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
        p_w: 0.0,
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
        entrance: true,
        exit_room: true,
        exit_doors: 1,
        exit_min_hops: 3,
        leaves: 0,
        leaf_classes: ByClass::new(1.0, 0.0, 0.0, 0.0, 0.0),
        junction: None,
        topology: TopologySpec::default(),
        growth: GrowthSpec::default(),
    }
}

impl Params {
    /// The topology requirements this Λ can meet: those that need the path between an entrance and an exit are left out without
    /// both, and independent routes to the exit are left out with several islands, whose bridges are single corridors.
    pub fn topology_requirements(&self) -> TopologySpec {
        let mut out = self
            .topology
            .requirements_with(self.entrance && self.exit_room);
        if self.layout == Layout::Growth && self.growth.islands > 1 {
            out.min_exit_routes = 1;
        }
        out
    }
}

/// A starting size for the floor: the grid and how many large blocks and leaf chambers go with it. A quick way to begin;
/// every other parameter stays at its default and any value can be changed afterwards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Small,
    Medium,
    Large,
    Huge,
}

impl Size {
    pub const ALL: [Size; 4] = [Size::Small, Size::Medium, Size::Large, Size::Huge];

    pub fn name(self) -> &'static str {
        ["small", "medium", "large", "huge"][self as usize]
    }

    /// The size at position `n` (1 to 4) of `ALL`, clamped.
    pub fn of_index(n: usize) -> Size {
        Size::ALL[n.clamp(1, 4) - 1]
    }

    /// Columns by rows of the grid.
    pub fn grid(self) -> (i32, i32) {
        [(4, 3), (6, 5), (10, 8), (16, 12)][self as usize]
    }
}

/// Λ of a size: the grid, about one large block in seven cells and one leaf chamber in eight.
pub fn params_of_size(size: Size) -> Params {
    let mut p = base();
    let (cols, rows) = size.grid();
    let cells = cols * rows;
    p.cols = cols;
    p.rows = rows;
    p.big = (cells + 3) / 7;
    p.leaves = [1, 4, 10, 24][size as usize];
    p
}

/// `params_of_size` with every shape, junction and connection option switched on, for tests, benchmarks and demos: a floor that
/// uses the whole generator. It is not a preset of the apps, which start from `params_of_size`.
pub fn rich_of_size(size: Size) -> Params {
    let mut p = params_of_size(size);
    p.m = 8;
    p.weights = ByClass::new(0.3, 0.2, 0.15, 0.2, 0.15);
    p.proximity = Proximity::Near;
    p.delta = 0.55;
    p.p_t = 0.25;
    p.p_x = 0.15;
    p.exit_doors = 2;
    p.exit_min_hops = 4;
    p.leaf_classes = ByClass::new(1.0, 1.0, 1.0, 0.0, 0.0);
    p
}
