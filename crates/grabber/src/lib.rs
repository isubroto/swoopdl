//! Bounded website capture projects, independent of the desktop interface.
pub mod catalog;
pub mod classify;
pub mod crawl;
pub mod destinations;
pub mod discover;
pub mod explore;
pub mod jobs;
pub mod model;
pub mod paths;
pub mod policy;
pub mod report;
pub mod rewrite;
pub mod robots;
pub mod store;
pub mod sync;

pub use model::*;
pub use policy::{Decision, PolicyError, evaluate, normalize_url};
pub(crate) fn media_type(value: &str) -> String {
    value
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}
