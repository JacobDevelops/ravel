#![forbid(unsafe_code)]

//! Deterministic test support for Ravel mods, packages, and loader processes.

mod pipeline;

pub use pipeline::{
    Host, PipelineError, PipelineRun, TestPackage, executed_states, run_order, run_pipeline,
};
