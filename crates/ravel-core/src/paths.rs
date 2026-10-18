use std::fmt;

use serde::{Deserialize, Serialize};

/// Instance-private root, relative to the game directory.
pub const INSTANCE_DIRECTORY: &str = "ravel";
/// Installed package sources, relative to the instance-private root.
pub const PACKAGES_DIRECTORY: &str = "mods";
/// Pack configuration, relative to the instance-private root.
pub const PACK_CONFIG_DIRECTORY: &str = "config";
/// Generated host crate, relative to the instance-private root.
pub const HOST_DIRECTORY: &str = "host";
/// The library name the generated host produces and the JVM loads.
pub const HOST_LIBRARY: &str = "ravel_bootstrap";
/// Seeded-pack record, relative to the instance-private root.
pub const SEEDED_PACKS_FILE: &str = "seeded-packs";

// ECMA `.` excludes these characters in the schema's `relativePath` pattern.
const LINE_TERMINATORS: [char; 4] = ['\n', '\r', '\u{2028}', '\u{2029}'];

/// A canonical, non-empty relative path without backslashes, line breaks, or dot segments.
///
/// # Examples
///
/// ```
/// use ravel_core::{RelativePath, RelativePathError};
///
/// let path = RelativePath::try_from("assets/example/lang/en_us.json".to_owned())?;
/// assert_eq!(path.file_name(), "en_us.json");
///
/// assert_eq!(
///     RelativePath::try_from("/etc/passwd".to_owned()),
///     Err(RelativePathError::Absolute)
/// );
/// assert_eq!(
///     RelativePath::try_from("nested/../../escape".to_owned()),
///     Err(RelativePathError::Traversal)
/// );
/// # Ok::<(), RelativePathError>(())
/// ```
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(into = "String", try_from = "String")]
pub struct RelativePath(String);

impl RelativePath {
    /// Returns the declared path.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the final path segment.
    #[must_use]
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or("")
    }

    /// Returns the first segment, which names the path's subtree.
    #[must_use]
    pub fn first_segment(&self) -> &str {
        self.0.split('/').next().unwrap_or("")
    }

    fn check(candidate: &str) -> Result<(), RelativePathError> {
        if candidate.is_empty() {
            return Err(RelativePathError::Empty);
        }
        if candidate.starts_with('/') {
            return Err(RelativePathError::Absolute);
        }
        if candidate.contains('\\') {
            return Err(RelativePathError::Backslash);
        }
        if candidate.contains(LINE_TERMINATORS) {
            return Err(RelativePathError::LineBreak);
        }
        if is_traversal(candidate) {
            return Err(RelativePathError::Traversal);
        }
        if candidate
            .split('/')
            .any(|segment| segment.is_empty() || segment == ".")
        {
            return Err(RelativePathError::NonCanonical);
        }
        Ok(())
    }
}

/// Returns whether `candidate` holds a `..` segment.
pub(crate) fn is_traversal(candidate: &str) -> bool {
    candidate.split('/').any(|segment| segment == "..")
}

impl TryFrom<String> for RelativePath {
    type Error = RelativePathError;

    fn try_from(candidate: String) -> Result<Self, Self::Error> {
        Self::check(&candidate)?;
        Ok(Self(candidate))
    }
}

impl From<RelativePath> for String {
    fn from(path: RelativePath) -> Self {
        path.0
    }
}

impl fmt::Display for RelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Why a package-relative path was rejected, without disclosing the path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelativePathError {
    /// The candidate is empty.
    Empty,
    /// The candidate is an absolute path.
    Absolute,
    /// The candidate holds a backslash.
    Backslash,
    /// The candidate holds a line terminator.
    LineBreak,
    /// The candidate holds a `..` component.
    Traversal,
    /// The candidate holds an empty or `.` component.
    NonCanonical,
}

impl fmt::Display for RelativePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("a package path cannot be empty"),
            Self::Absolute => formatter.write_str(
                "a package path is relative to the package, so it cannot start with `/`",
            ),
            Self::Backslash => formatter
                .write_str("a package path is `/`-separated, so it cannot hold a backslash"),
            Self::LineBreak => formatter.write_str("a package path cannot hold a line terminator"),
            Self::Traversal => formatter.write_str(
                "a package path cannot leave the package, so it cannot hold a `..` component",
            ),
            Self::NonCanonical => {
                formatter.write_str("a package path cannot hold an empty or `.` component")
            }
        }
    }
}

impl std::error::Error for RelativePathError {}

#[cfg(test)]
mod tests {
    use super::{RelativePath, RelativePathError};

    #[test]
    fn a_legal_relative_path_keeps_its_spelling_and_names_its_file() {
        for (candidate, file_name) in [
            ("mod.so", "mod.so"),
            ("src/lib.rs", "lib.rs"),
            ("resources/lang/en.json", "en.json"),
            ("a/b/c/provenance.json", "provenance.json"),
            ("a..b/c", "c"),
            ("...", "..."),
            ("deep/.hidden", ".hidden"),
            ("single", "single"),
        ] {
            let path = RelativePath::try_from(candidate.to_owned()).expect(candidate);
            assert_eq!(path.as_str(), candidate);
            assert_eq!(path.to_string(), candidate);
            assert_eq!(path.file_name(), file_name);
            assert_eq!(String::from(path.clone()), candidate);

            let encoded = serde_json::to_string(&path).expect("should serialize");
            assert_eq!(
                serde_json::from_str::<RelativePath>(&encoded).expect("should deserialize"),
                path
            );
        }
    }

    #[test]
    fn the_first_segment_names_a_subtree_rather_than_a_string_prefix() {
        for (candidate, first) in [
            ("data/example/tags/item/ingots.json", "data"),
            ("database/copper.json", "database"),
            ("single", "single"),
        ] {
            let path = RelativePath::try_from(candidate.to_owned()).expect(candidate);
            assert_eq!(path.first_segment(), first);
        }
    }

    #[test]
    fn every_shape_the_schema_pattern_excludes_is_refused_with_its_reason() {
        for (candidate, expected) in [
            ("", RelativePathError::Empty),
            ("/etc/passwd", RelativePathError::Absolute),
            ("/", RelativePathError::Absolute),
            ("..", RelativePathError::Traversal),
            ("../sibling", RelativePathError::Traversal),
            ("nested/../../escape", RelativePathError::Traversal),
            ("nested/..", RelativePathError::Traversal),
            (".", RelativePathError::NonCanonical),
            ("./nested/file", RelativePathError::NonCanonical),
            ("nested/./file", RelativePathError::NonCanonical),
            ("deep/nested/", RelativePathError::NonCanonical),
            ("a//", RelativePathError::NonCanonical),
            ("a//b", RelativePathError::NonCanonical),
            ("..\\..\\escape", RelativePathError::Backslash),
            ("\\\\server\\share", RelativePathError::Backslash),
            ("nested\\file", RelativePathError::Backslash),
            ("a\nb", RelativePathError::LineBreak),
            ("a\rb", RelativePathError::LineBreak),
            ("trailing\n", RelativePathError::LineBreak),
            ("a\u{2028}b", RelativePathError::LineBreak),
            ("a\u{2029}b", RelativePathError::LineBreak),
        ] {
            assert_eq!(
                RelativePath::try_from(candidate.to_owned()),
                Err(expected),
                "{candidate:?}"
            );
            let encoded = serde_json::to_string(candidate).expect("candidate string");
            assert!(
                serde_json::from_str::<RelativePath>(&encoded).is_err(),
                "{candidate:?}"
            );
        }
    }

    #[test]
    fn a_refusal_says_which_rule_the_path_broke_and_quotes_no_path() {
        for (error, expected) in [
            (RelativePathError::Empty, "a package path cannot be empty"),
            (
                RelativePathError::Absolute,
                "a package path is relative to the package, so it cannot start with `/`",
            ),
            (
                RelativePathError::Backslash,
                "a package path is `/`-separated, so it cannot hold a backslash",
            ),
            (
                RelativePathError::LineBreak,
                "a package path cannot hold a line terminator",
            ),
            (
                RelativePathError::Traversal,
                "a package path cannot leave the package, so it cannot hold a `..` component",
            ),
            (
                RelativePathError::NonCanonical,
                "a package path cannot hold an empty or `.` component",
            ),
        ] {
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn a_path_rejection_never_echoes_the_path_it_refused() {
        for candidate in ["/etc/passwd", "../../home/player/.ssh/id_ed25519"] {
            let error = RelativePath::try_from(candidate.to_owned()).expect_err(candidate);
            let rendered = error.to_string();
            assert!(!rendered.contains("passwd"), "{rendered}");
            assert!(!rendered.contains("player"), "{rendered}");
        }
    }
}
