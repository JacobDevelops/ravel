#![forbid(unsafe_code)]

//! Dependency resolution and deterministic load planning.

mod error;
mod graph;
mod plan;

pub use error::ResolutionError;
pub use graph::resolve;
pub use plan::{Discouraged, LoadPlan};
