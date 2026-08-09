use std::fmt;

use serde::{Deserialize, Serialize};

/// The characters an ECMA `.` does not match, and so the ones
/// `schemas/package.schema.json`'s `relativePath` pattern excludes by ending in
/// `.+`.
const LINE_TERMINATORS: [char; 4] = ['\n', '\r', '\u{2028}', '\u{2029}'];

/// A path inside a package or a generated pack.
///
/// The grammar is `schemas/package.schema.json`'s `relativePath` pattern,
/// `^(?!/)(?!.*\\)(?!.*(?:^|/)\.\.(?:/|$)).+$`, implemented here rather than
/// matched so no crate in the graph needs a regex engine, and checked against a
/// real engine by the conformance suite rather than against a second
/// hand-written copy of the pattern.
///
/// The lookaheads are the whole point of the type: a manifest names files the
/// loader will open, so an absolute path would let a package address the host
/// filesystem and a `..` component would let it climb out of its own directory.
/// A backslash is refused outright because it is a separator on Windows and not
/// on Linux, so `..\..\x` is one traversal to a future host and one ordinary
/// file name to this one — the disagreement, not the character, is the danger.
/// The trailing `.+` refuses the four characters an ECMA `.` excludes, which is
/// where a line break in a declared path would otherwise part company with the
/// schema. All of it is refused before anything is read, which is what keeps
/// `ravel-package`'s source and provenance verification package-local.
///
/// It lives here rather than in `ravel-package` because two unrelated things
/// need it and only one of them is a package manifest: `ravel-data` names the
/// path a resource is supplied at and the path a generated file is written to,
/// and reaching into the package crate for the type would put that crate's
/// integrity dependencies — a SHA-256 implementation among them — into the
/// dependency closure of every mod that declares a resource.
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

    /// Returns the last non-empty segment, which is all a recorded message may
    /// say about a file.
    ///
    /// `SPEC.md` §9.1 requires reports to redact unnecessarily identifying
    /// filesystem paths, and a package-relative path still discloses the layout
    /// of a player's installation once it is joined to a package root. Every
    /// integrity failure therefore names a file, never a path.
    ///
    /// The schema permits a trailing slash, and the last segment of `src/`
    /// is the empty string, which would render a failure as being about nothing
    /// at all. The last *non-empty* segment always exists instead: a path that
    /// is empty or starts with `/` never becomes a [`RelativePath`], so the
    /// first segment is never empty and there is always one to find.
    #[must_use]
    pub fn file_name(&self) -> &str {
        self.0
            .rsplit('/')
            .find(|segment| !segment.is_empty())
            .unwrap_or("")
    }

    /// Returns the first segment, which names the subtree the path is in.
    ///
    /// `ravel-data` reserves one subtree for generated output, and reserving a
    /// subtree means comparing first segments rather than string prefixes: a
    /// prefix test on `data/` would leave `database/copper.json` looking like a
    /// path inside the reserved tree.
    #[must_use]
    pub fn first_segment(&self) -> &str {
        self.0.split('/').next().unwrap_or("")
    }

    fn validate(candidate: &str) -> Result<(), RelativePathError> {
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
        if candidate.split('/').any(|segment| segment == "..") {
            return Err(RelativePathError::Traversal);
        }
        Ok(())
    }
}

impl TryFrom<String> for RelativePath {
    type Error = RelativePathError;

    fn try_from(candidate: String) -> Result<Self, Self::Error> {
        Self::validate(&candidate)?;
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

/// Why a candidate string is not a valid [`RelativePath`].
///
/// No variant carries the rejected path. The rejection is reported through a
/// deserializer that already reports the line and column, so echoing the value
/// would add nothing except a filesystem path in a message that may be
/// recorded.
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
            // Every shape the schema pattern permits and a reader expects it to
            // refuse: `..` is refused as a path *component*, not as a substring.
            ("a..b/c", "c"),
            ("...", "..."),
            ("./nested/file", "file"),
            ("deep/.hidden", ".hidden"),
            ("deep/nested/", "nested"),
            ("a//", "a"),
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

    #[test]
    fn a_path_that_names_a_directory_still_names_a_file_to_report() {
        // A trailing slash is legal in the schema, and a message about the empty
        // string would be a message about nothing.
        let path = RelativePath::try_from("src/".to_owned()).expect("a legal path");
        assert_eq!(path.file_name(), "src");
    }
}
