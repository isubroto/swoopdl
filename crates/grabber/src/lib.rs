//! Bounded website capture projects, independent of the desktop interface.
pub mod model;
pub mod policy;

pub use model::*;
pub use policy::{Decision, PolicyError, evaluate, normalize_url};
