//! The Tapiti model's generator (`docs/model.md` §7): valid by construction. Every step only makes choices that keep the
//! constraints already fixed, so nothing is drawn and then thrown away whole. Deterministic: the same seed and Λ give the same stage.

use crate::geom::*;
use crate::par::{pflat_min, pmap_min};
use crate::params::*;
use crate::rng::{Const, Mulberry, Rand};
use crate::spatial::SpatialHash;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::f64::consts::PI;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "diagnostic-profile")]
use std::time::{Duration, Instant};

mod beta;
mod candidates;
mod corridors;
mod cycles;
mod doors;
mod graph;
mod growth;
mod leaves;
mod pipeline;
mod roles;
mod rooms;
mod scaffold;
mod shapes;
mod state;
mod tree;
mod types;

use beta::*;
use candidates::*;
use corridors::*;
use cycles::*;
use doors::*;
use growth::*;
use leaves::*;
use pipeline::*;
use roles::*;
use rooms::*;
use scaffold::*;
use shapes::*;
use state::*;
use tree::*;
use types::*;

pub use candidates::candidates_for;
pub use corridors::outside_region;
pub use graph::room_hops;
pub use pipeline::{generate, generate_with_progress};
pub use types::*;
