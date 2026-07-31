#![forbid(unsafe_code)]

//! Structured diagnostics, redaction, metrics, and opt-in profiling contracts.

mod redaction;
mod report;

pub use redaction::{Redactor, redact};
pub use report::{PreWorldReport, Refusal, Subject, WorldAccessRefused};
