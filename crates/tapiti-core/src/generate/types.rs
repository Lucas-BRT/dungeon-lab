//! Doors, rooms, edges, candidates and the finished `Floor`.

use super::*;

/// A door (§3): where on the room's outline a corridor meets it and the direction (0..7, ×45°) it leaves in.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Port {
    pub room: usize,
    pub p: V,
    pub k: i32,
    pub edge: i32,
    pub embed: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Start,
    Stairs,
    Ambush,
    Boss,
    Rest,
    Key,
    Switch,
    Plain,
    Trap,
    Prize,
    Empty,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Room {
    pub id: usize,
    pub cls: ShapeClass,
    pub shape: Shape,
    #[serde(skip)]
    pub bounds: Bounds,
    pub center: V,
    pub area: f64,
    #[serde(rename = "box")]
    pub rect: Rect,
    /// Admissible doors, sampled `portStep` px apart along each edge (the set is continuous; these are the positions tried).
    pub ports: Vec<Port>,
    pub used: Vec<Port>,
    pub leaf: bool,
    pub alive: bool,
    pub role: Role,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Node {
    Room { room: usize },
    Junction { p: Box<V> },
}

/// A branch of a corridor between two nodes of H (§5.1): a room or a junction at each end.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub id: usize,
    pub u: usize,
    pub v: usize,
    pub pts: Vec<V>,
    pub ends: [Option<Port>; 2],
    /// Collision geometry is kept in Rust for generation/validation but is not
    /// needed by the viewer's map representation.
    #[serde(skip)]
    pub region: Vec<Bounded>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum RouteType {
    I,
    L,
    Z,
    W,
}

#[derive(Clone, Debug, Serialize)]
pub struct Candidate {
    pub a: usize,
    pub b: usize,
    pub ports: [Port; 2],
    pub pts: Vec<V>,
    #[serde(rename = "type")]
    pub kind: RouteType,
    pub cost: f64,
    pub weight: f64,
    pub region: Vec<Bounded>,
}

/// The compact candidate geometry sent to the viewer. Search-only ports,
/// weights and collision regions are released after generation.
#[derive(Clone, Debug, Serialize)]
pub struct CandidateView {
    pub a: usize,
    pub b: usize,
    pub pts: Vec<V>,
    #[serde(rename = "type")]
    pub kind: RouteType,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Floor {
    pub params: Params,
    pub seed: u32,
    pub rooms: Vec<Room>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub start: usize,
    pub stairs: usize,
    /// Node IDs on the designated start-to-stairs path in H, before leaf rooms are added.
    pub main_path: Vec<usize>,
    /// Grid line coordinates; interior lines may be jittered while the outer bounds stay fixed.
    pub grid_x: Vec<f64>,
    pub grid_y: Vec<f64>,
    pub beta_max: i32,
    pub beta_target: i32,
    pub candidates: Vec<CandidateView>,
    pub pairs: Vec<[usize; 2]>,
    pub dropped: Vec<V>,
    pub log: Vec<String>,
    /// Milliseconds each phase of the generator took, in order (rooms, candidates, tree, β_max, cycles, leaf chambers).
    pub phases: Vec<(String, f64)>,
}

/// A lightweight event stream for drawing a floor while it is being built.
#[derive(Clone, Debug)]
pub enum GenerationEvent {
    AttemptStarted {
        draw: u32,
        seed: u32,
    },
    Layout {
        grid_x: Vec<f64>,
        grid_y: Vec<f64>,
        corridor_width: f64,
    },
    RoomAdded(Shape),
    EdgeUpdated {
        id: usize,
        pts: Vec<V>,
    },
}
