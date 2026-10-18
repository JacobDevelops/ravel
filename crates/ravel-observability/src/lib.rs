#![forbid(unsafe_code)]

//! Structured diagnostics, metrics, and opt-in profiling contracts.

mod profiling;
mod report;

pub use profiling::{
    CallKind, DEFAULT_SLOW_CALLBACK, Metric, ModCost, ModReport, Profiler, ProfilingReport, Support,
};
pub use report::{PreWorldReport, Refusal, Subject, WorldAccessRefused};
