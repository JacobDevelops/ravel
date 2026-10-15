//! CLI fixture and performance failures.

use std::fmt;

/// A performance-collector parsing or normalization failure.
#[derive(Debug, Eq, PartialEq)]
pub enum PerformanceError {
    /// Reading raw collector output failed.
    Io(String),
    /// A pinned Tracy collector or exporter operation failed.
    TracyTool(String),
    /// Tracy frame CSV violated the pinned collector contract.
    InvalidTracyFrameCsv(String),
    /// Linux procfs memory counters violated the pinned collector contract.
    InvalidLinuxProc(String),
    /// Linux normalized memory CSV violated the pinned collector contract.
    InvalidLinuxMemoryCsv(String),
    /// Lifecycle marker CSV violated the initial profile contract.
    InvalidPerformanceMarkerCsv(String),
}

impl fmt::Display for PerformanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message)
            | Self::TracyTool(message)
            | Self::InvalidTracyFrameCsv(message)
            | Self::InvalidLinuxProc(message)
            | Self::InvalidLinuxMemoryCsv(message)
            | Self::InvalidPerformanceMarkerCsv(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for PerformanceError {}

/// A fixture reconstruction or verification failure.
#[derive(Debug, Eq, PartialEq)]
pub enum FixtureError {
    /// A local file operation failed.
    Io(String),
    /// JSON parsing failed.
    Json(String),
    /// A descriptor field violates the public fixture policy.
    InvalidDescriptor(String),
    /// An artifact did not match its recorded digest.
    HashMismatch {
        /// Artifact being checked.
        artifact: &'static str,
        /// Recorded SHA-256.
        expected: String,
        /// Observed SHA-256.
        actual: String,
    },
    /// The archive could not provide its manifest.
    Archive(String),
    /// A pinned manifest field drifted.
    MetadataMismatch {
        /// Field being checked.
        field: &'static str,
        /// Recorded value.
        expected: String,
        /// Observed value.
        actual: String,
    },
    /// The `CurseForge` reference total drifted.
    ReferenceCountMismatch {
        /// Recorded reference total.
        expected: usize,
        /// Observed reference total.
        actual: usize,
    },
    /// The manifest contains the same project/file pair more than once.
    DuplicateReference {
        /// `CurseForge` project identifier.
        project_id: u64,
        /// `CurseForge` file identifier.
        file_id: u64,
    },
    /// The download process failed.
    Download(String),
}

impl fmt::Display for FixtureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message)
            | Self::Json(message)
            | Self::InvalidDescriptor(message)
            | Self::Archive(message)
            | Self::Download(message) => formatter.write_str(message),
            Self::HashMismatch {
                artifact,
                expected,
                actual,
            } => write!(
                formatter,
                "{artifact} SHA-256 mismatch: expected {expected}, observed {actual}"
            ),
            Self::MetadataMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "fixture manifest {field} mismatch: expected {expected}, observed {actual}"
            ),
            Self::ReferenceCountMismatch { expected, actual } => write!(
                formatter,
                "fixture manifest reference count mismatch: expected {expected}, observed {actual}"
            ),
            Self::DuplicateReference {
                project_id,
                file_id,
            } => write!(
                formatter,
                "fixture manifest repeats CurseForge project {project_id} file {file_id}"
            ),
        }
    }
}

impl std::error::Error for FixtureError {}
