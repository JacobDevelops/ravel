//! Fixture reconstruction and verification failures.

use std::fmt;

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

#[cfg(test)]
mod tests {
    use super::FixtureError;
    use std::error::Error;

    #[test]
    fn message_variants_preserve_the_original_diagnostic() {
        let constructors: [fn(String) -> FixtureError; 5] = [
            FixtureError::Io,
            FixtureError::Json,
            FixtureError::InvalidDescriptor,
            FixtureError::Archive,
            FixtureError::Download,
        ];
        for constructor in constructors {
            for message in ["", "fixture failed: café\nwith context"] {
                let error = constructor(message.to_owned());
                assert_eq!(error.to_string(), message);
                assert!(error.source().is_none());
            }
        }
    }

    #[test]
    fn mismatch_variants_identify_the_field_and_expected_and_observed_values() {
        let cases = [
            (
                FixtureError::HashMismatch {
                    artifact: "archive",
                    expected: "abc123".to_owned(),
                    actual: "def456".to_owned(),
                },
                "archive SHA-256 mismatch: expected abc123, observed def456",
            ),
            (
                FixtureError::MetadataMismatch {
                    field: "version",
                    expected: "1.0".to_owned(),
                    actual: "2.0".to_owned(),
                },
                "fixture manifest version mismatch: expected 1.0, observed 2.0",
            ),
            (
                FixtureError::ReferenceCountMismatch {
                    expected: 3,
                    actual: 0,
                },
                "fixture manifest reference count mismatch: expected 3, observed 0",
            ),
            (
                FixtureError::DuplicateReference {
                    project_id: 42,
                    file_id: 123,
                },
                "fixture manifest repeats CurseForge project 42 file 123",
            ),
        ];
        for (error, message) in cases {
            assert_eq!(error.to_string(), message);
            assert!(error.source().is_none());
        }
    }
}
