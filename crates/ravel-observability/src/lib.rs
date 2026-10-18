#![forbid(unsafe_code)]

//! Structured diagnostics, redaction, metrics, and opt-in profiling contracts.

mod profiling;
pub mod redaction;
mod report;

pub use profiling::{
    CallKind, DEFAULT_SLOW_CALLBACK, Metric, ModCost, ModReport, Profiler, ProfilingReport, Support,
};
pub use redaction::{Redactor, contains_credential, redact};
pub use report::{PreWorldReport, Refusal, Subject, WorldAccessRefused};
