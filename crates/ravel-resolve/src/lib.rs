#![forbid(unsafe_code)]

//! Dependency resolution and deterministic load planning.

mod graph;
mod plan;

pub use graph::{ResolutionError, resolve};
pub use plan::{Discouraged, LoadPlan};
