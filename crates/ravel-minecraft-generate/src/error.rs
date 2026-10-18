//! Binding generation failures.

use std::fmt;

/// Anything that stops a binding from being generated.
#[derive(Debug)]
pub enum GenerateError {
    /// An I/O operation failed, retaining its original error.
    Io {
        /// Privacy-safe diagnostic label for the file, directory, or command.
        file: String,
        /// Original I/O failure.
        source: std::io::Error,
    },
    /// An input or generation request is semantically unusable.
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
            Self::Io { file, source } => write!(formatter, "cannot access {file}: {source}"),
            Self::Input(detail) => write!(formatter, "an input is unusable: {detail}"),
            Self::Drift(detail) => write!(formatter, "an input drifted: {detail}"),
            Self::Unresolved(detail) => write!(formatter, "a hook does not resolve: {detail}"),
            Self::Extraction(detail) => write!(formatter, "extraction failed: {detail}"),
            Self::Unsupported(detail) => write!(formatter, "a hook cannot be generated: {detail}"),
        }
    }
}

impl std::error::Error for GenerateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Input(_)
            | Self::Drift(_)
            | Self::Unresolved(_)
            | Self::Extraction(_)
            | Self::Unsupported(_) => None,
        }
    }
}

impl GenerateError {
    pub(crate) fn io(file: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            file: file.into(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    #[test]
    fn io_errors_retain_the_original_source() {
        let error = super::GenerateError::io(
            "adapter.toml",
            std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        );
        let source = error
            .source()
            .expect("I/O generation errors must retain their source")
            .downcast_ref::<std::io::Error>()
            .expect("I/O generation error source must be an std::io::Error");
        assert_eq!(source.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("adapter.toml"));
        assert!(
            super::GenerateError::Input("invalid".to_owned())
                .source()
                .is_none()
        );
    }
}
