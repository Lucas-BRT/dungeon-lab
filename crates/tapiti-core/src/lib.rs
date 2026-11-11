//! Core model and generation algorithms for Tapiti.
// `!(a < b)` is deliberate: a NaN must take the negated branch, which `a >= b` would not.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

pub mod clock;
pub mod generate;
pub mod geom;
pub mod layout;
pub mod metrics;
pub mod par;
pub mod params;
pub mod rng;
pub(crate) mod spatial;
pub mod stage;
pub mod topology;
pub mod validate;
