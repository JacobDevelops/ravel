use std::fmt;

use serde::{Deserialize, Serialize};

/// The only `schemaVersion` `schemas/package.schema.json` accepts.
const SUPPORTED_SCHEMA_VERSION: u64 = 1;

/// Length in characters of the `sha256` form the schema declares.
const DIGEST_TEXT_LENGTH: usize = 64;

/// Length in bytes of a SHA-256 digest.
const DIGEST_BYTES: usize = 32;

/// The characters an ECMA `.` does not match, and so the ones the schema's
/// `relativePath` pattern excludes by ending in `.+`.
const LINE_TERMINATORS: [char; 4] = ['\n', '\r', '\u{2028}', '\u{2029}'];

/// The manifest schema a package declares itself against.
///
/// `schemas/package.schema.json` pins `schemaVersion` to the constant `1`. The
/// constant is a version negotiation point rather than decoration: a manifest
/// that declares anything else was written against a contract this loader does
/// not implement, and guessing at it would run code that never agreed to these
/// rules.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(into = "u64", try_from = "u64")]
pub struct SchemaVersion(u64);

impl SchemaVersion {
    /// The schema version this crate implements.
    pub const CURRENT: Self = Self(SUPPORTED_SCHEMA_VERSION);

    /// Returns the declared version as its numeric form.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for SchemaVersion {
    type Error = SchemaVersionError;

    fn try_from(declared: u64) -> Result<Self, Self::Error> {
        if declared == SUPPORTED_SCHEMA_VERSION {
            Ok(Self(declared))
        } else {
            Err(SchemaVersionError { declared })
        }
    }
}

impl From<SchemaVersion> for u64 {
    fn from(version: SchemaVersion) -> Self {
        version.0
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// A manifest declared a schema version this crate does not implement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchemaVersionError {
    /// The version the manifest declared.
    pub declared: u64,
}

impl fmt::Display for SchemaVersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a package manifest declares schemaVersion {SUPPORTED_SCHEMA_VERSION}, not {}",
            self.declared
        )
    }
}

impl std::error::Error for SchemaVersionError {}

/// A string the schema constrains with `minLength: 1`.
///
/// The schema asserts a minimum length on the display name, the licence
/// expression, an author name, a target platform, a target architecture, and
/// every declared Minecraft version. Each one names something, and an empty
/// name is a metadata error rather than a mod that happens to be anonymous.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(into = "String", try_from = "String")]
pub struct NonEmptyText(String);

impl NonEmptyText {
    /// Returns the text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for NonEmptyText {
    type Error = EmptyText;

    fn try_from(candidate: String) -> Result<Self, Self::Error> {
        if candidate.is_empty() {
            return Err(EmptyText);
        }
        Ok(Self(candidate))
    }
}

impl From<NonEmptyText> for String {
    fn from(text: NonEmptyText) -> Self {
        text.0
    }
}

impl fmt::Display for NonEmptyText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A required text value was declared empty.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmptyText;

impl fmt::Display for EmptyText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("this value must not be empty")
    }
}

impl std::error::Error for EmptyText {}

/// A path inside the package directory.
///
/// The grammar is `schemas/package.schema.json`'s `relativePath` pattern,
/// `^(?!/)(?!.*\\)(?!.*(?:^|/)\.\.(?:/|$)).+$`, implemented here rather than
/// matched so the crate needs no regex engine, and checked against a real
/// engine by the conformance suite rather than against a second hand-written
/// copy of the pattern.
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
/// [`crate::verify_artifact`] a package-local operation.
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
    /// [`crate::IntegrityError`] therefore names a file, never a path.
    ///
    /// The schema permits a trailing slash, and the last segment of `artifacts/`
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

/// A SHA-256 digest as `schemas/package.schema.json` records one.
///
/// The schema's `sha256` pattern is `^[0-9a-f]{64}$`, so the text form is
/// exactly 64 lowercase hex characters and nothing else. The digest is kept as
/// bytes rather than as text: that is what a comparison against a freshly
/// hashed file needs, and it makes the canonical lowercase rendering the only
/// one this crate can emit.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(into = "String", try_from = "String")]
pub struct Sha256Digest([u8; DIGEST_BYTES]);

impl Sha256Digest {
    /// Wraps a computed digest.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; DIGEST_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; DIGEST_BYTES] {
        &self.0
    }

    fn parse(candidate: &str) -> Result<Self, Sha256Error> {
        if candidate.len() != DIGEST_TEXT_LENGTH {
            return Err(Sha256Error::Length {
                length: candidate.chars().count(),
            });
        }

        let mut bytes = [0_u8; DIGEST_BYTES];
        for (index, character) in candidate.chars().enumerate() {
            let Some(value) = hex_value(character) else {
                return Err(Sha256Error::Character { character, index });
            };
            let shift = if index % 2 == 0 { 4 } else { 0 };
            bytes[index / 2] |= value << shift;
        }
        Ok(Self(bytes))
    }
}

/// Decodes one lowercase hex character, refusing the uppercase form the schema
/// pattern excludes so a digest has exactly one text spelling.
fn hex_value(character: char) -> Option<u8> {
    let value = match character {
        '0'..='9' => u32::from(character) - u32::from('0'),
        'a'..='f' => u32::from(character) - u32::from('a') + 10,
        _ => return None,
    };
    u8::try_from(value).ok()
}

impl TryFrom<String> for Sha256Digest {
    type Error = Sha256Error;

    fn try_from(candidate: String) -> Result<Self, Self::Error> {
        Self::parse(&candidate)
    }
}

impl From<Sha256Digest> for String {
    fn from(digest: Sha256Digest) -> Self {
        digest.to_string()
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Why a candidate string is not a valid [`Sha256Digest`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sha256Error {
    /// The candidate is not 64 characters long.
    Length {
        /// The candidate length in characters.
        length: usize,
    },
    /// A character is not a lowercase hex digit.
    Character {
        /// The rejected character.
        character: char,
        /// Zero-based character index of the rejection.
        index: usize,
    },
}

impl fmt::Display for Sha256Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { length } => write!(
                formatter,
                "a sha256 digest is {DIGEST_TEXT_LENGTH} characters long, not {length}"
            ),
            Self::Character { character, index } => write!(
                formatter,
                "a sha256 digest holds only `0`-`9` and `a`-`f`, but character {index} is \
                 `{character}`"
            ),
        }
    }
}

impl std::error::Error for Sha256Error {}

#[cfg(test)]
mod tests {
    use super::{
        EmptyText, NonEmptyText, RelativePath, RelativePathError, SchemaVersion,
        SchemaVersionError, Sha256Digest, Sha256Error,
    };

    const ZERO_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";
    const MIXED_DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn the_only_accepted_schema_version_is_the_one_this_crate_implements() {
        let version = SchemaVersion::try_from(1).expect("the current schema version");
        assert_eq!(version, SchemaVersion::CURRENT);
        assert_eq!(version.get(), 1);
        assert_eq!(version.to_string(), "1");
        assert_eq!(u64::from(version), 1);

        for declared in [0, 2, 99] {
            assert_eq!(
                SchemaVersion::try_from(declared),
                Err(SchemaVersionError { declared })
            );
        }
        assert_eq!(
            SchemaVersionError { declared: 2 }.to_string(),
            "a package manifest declares schemaVersion 1, not 2"
        );
    }

    #[test]
    fn a_schema_version_round_trips_as_the_bare_number() {
        let encoded = serde_json::to_string(&SchemaVersion::CURRENT).expect("should serialize");
        assert_eq!(encoded, "1");
        assert_eq!(
            serde_json::from_str::<SchemaVersion>(&encoded).expect("should deserialize"),
            SchemaVersion::CURRENT
        );

        let error = serde_json::from_str::<SchemaVersion>("2").expect_err("unsupported version");
        assert!(error.to_string().contains("not 2"), "{error}");
    }

    #[test]
    fn constrained_text_refuses_only_the_empty_value() {
        let text = NonEmptyText::try_from("MIT".to_owned()).expect("a licence expression");
        assert_eq!(text.as_str(), "MIT");
        assert_eq!(text.to_string(), "MIT");
        assert_eq!(String::from(text), "MIT");

        assert_eq!(NonEmptyText::try_from(String::new()), Err(EmptyText));
        assert_eq!(EmptyText.to_string(), "this value must not be empty");

        let error = serde_json::from_str::<NonEmptyText>("\"\"").expect_err("empty text");
        assert!(error.to_string().contains("must not be empty"), "{error}");
    }

    #[test]
    fn a_package_path_accepts_every_shape_the_schema_pattern_does() {
        for candidate in [
            "mod.so",
            "artifacts/linux-x86_64/mod.so",
            "resources/lang/en.json",
            "a..b/c",
            "...",
            "./nested/file",
            "deep/.hidden",
        ] {
            let path = RelativePath::try_from(candidate.to_owned()).expect(candidate);
            assert_eq!(path.as_str(), candidate);
            assert_eq!(path.to_string(), candidate);
            assert_eq!(String::from(path), candidate);
        }
    }

    #[test]
    fn a_package_path_refuses_escape_in_every_position() {
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
                "{candidate}"
            );
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
    fn every_path_rejection_renders_the_rule_it_breaks() {
        for (error, expected) in [
            (RelativePathError::Empty, "a package path cannot be empty"),
            (
                RelativePathError::Absolute,
                "a package path is relative to the package, so it cannot start with `/`",
            ),
            (
                RelativePathError::Traversal,
                "a package path cannot leave the package, so it cannot hold a `..` component",
            ),
            (
                RelativePathError::Backslash,
                "a package path is `/`-separated, so it cannot hold a backslash",
            ),
            (
                RelativePathError::LineBreak,
                "a package path cannot hold a line terminator",
            ),
        ] {
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn a_file_name_is_the_last_segment_of_a_declared_path() {
        for (candidate, expected) in [
            ("mod.so", "mod.so"),
            ("artifacts/linux-x86_64/mod.so", "mod.so"),
            ("a/b/c/provenance.json", "provenance.json"),
        ] {
            let path = RelativePath::try_from(candidate.to_owned()).expect(candidate);
            assert_eq!(path.file_name(), expected);
        }
    }

    #[test]
    fn a_trailing_slash_still_names_a_file_rather_than_nothing() {
        for (candidate, expected) in [
            ("artifacts/", "artifacts"),
            ("a/b/", "b"),
            ("a//", "a"),
            ("a", "a"),
        ] {
            let path = RelativePath::try_from(candidate.to_owned()).expect(candidate);
            assert!(!path.file_name().is_empty(), "{candidate}");
            assert_eq!(path.file_name(), expected, "{candidate}");
        }
    }

    #[test]
    fn a_digest_round_trips_through_its_lowercase_hex_form() {
        for candidate in [ZERO_DIGEST, MIXED_DIGEST, &"f".repeat(64)] {
            let digest = Sha256Digest::try_from(candidate.to_owned()).expect(candidate);
            assert_eq!(digest.to_string(), candidate);
            assert_eq!(String::from(digest), candidate);

            let encoded = serde_json::to_string(&digest).expect("should serialize");
            assert_eq!(encoded, format!("\"{candidate}\""));
            assert_eq!(
                serde_json::from_str::<Sha256Digest>(&encoded).expect("should deserialize"),
                digest
            );
        }
    }

    #[test]
    fn a_digest_decodes_every_nibble_into_the_byte_it_names() {
        let digest = Sha256Digest::try_from(MIXED_DIGEST.to_owned()).expect("valid digest");
        assert_eq!(digest.as_bytes()[0], 0x01);
        assert_eq!(digest.as_bytes()[7], 0xef);
        assert_eq!(
            digest.as_bytes(),
            &Sha256Digest::from_bytes(*digest.as_bytes()).0
        );
    }

    #[test]
    fn a_digest_refuses_every_shape_the_schema_pattern_refuses() {
        for (candidate, expected) in [
            ("", Sha256Error::Length { length: 0 }),
            (&"a".repeat(63), Sha256Error::Length { length: 63 }),
            (&"a".repeat(65), Sha256Error::Length { length: 65 }),
            (
                &format!("{}A", &MIXED_DIGEST[..63]),
                Sha256Error::Character {
                    character: 'A',
                    index: 63,
                },
            ),
            (
                &format!("g{}", &MIXED_DIGEST[1..]),
                Sha256Error::Character {
                    character: 'g',
                    index: 0,
                },
            ),
            (
                &format!(" {}", &MIXED_DIGEST[1..]),
                Sha256Error::Character {
                    character: ' ',
                    index: 0,
                },
            ),
        ] {
            assert_eq!(
                Sha256Digest::try_from(candidate.to_owned()),
                Err(expected),
                "{candidate}"
            );
        }
    }

    #[test]
    fn a_multibyte_candidate_is_measured_in_characters_rather_than_bytes() {
        let candidate = format!("{}\u{e9}", &MIXED_DIGEST[..62]);
        assert_eq!(candidate.len(), 64);
        assert_eq!(
            Sha256Digest::try_from(candidate),
            Err(Sha256Error::Character {
                character: '\u{e9}',
                index: 62,
            })
        );

        assert_eq!(
            Sha256Digest::try_from("\u{e9}".to_owned()),
            Err(Sha256Error::Length { length: 1 })
        );
    }

    #[test]
    fn every_digest_rejection_renders_the_rule_it_breaks() {
        for (error, expected) in [
            (
                Sha256Error::Length { length: 3 },
                "a sha256 digest is 64 characters long, not 3",
            ),
            (
                Sha256Error::Character {
                    character: 'A',
                    index: 63,
                },
                "a sha256 digest holds only `0`-`9` and `a`-`f`, but character 63 is `A`",
            ),
        ] {
            assert_eq!(error.to_string(), expected);
        }
    }
}
