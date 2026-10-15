//! Binding generation failures.

use std::fmt;

/// Anything that stops a binding from being generated.
#[derive(Debug)]
pub enum GenerateError {
    /// An input is missing or unreadable.
    Input(String),
    /// An input's content does not match what is tracked.
    Drift(String),
    /// A hook entry does not resolve against the jar.
    Unresolved(String),
    /// An extractor could not read the jar.
    Extraction(String),
    /// A hook resolves, but its shape cannot be generated for.
    Unsupported(String),
}

impl fmt::Display for GenerateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(detail) => write!(formatter, "an input is unusable: {detail}"),
            Self::Drift(detail) => write!(formatter, "an input drifted: {detail}"),
            Self::Unresolved(detail) => write!(formatter, "a hook does not resolve: {detail}"),
            Self::Extraction(detail) => write!(formatter, "extraction failed: {detail}"),
            Self::Unsupported(detail) => write!(formatter, "a hook cannot be generated: {detail}"),
        }
    }
}

impl std::error::Error for GenerateError {}
