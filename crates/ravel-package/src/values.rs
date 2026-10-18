use std::fmt;

use serde::{Deserialize, Serialize};

/// Checked paths and SHA-256 digests shared with other Ravel contracts.
pub use ravel_core::{RelativePath, RelativePathError, Sha256Digest, Sha256Error};

const SUPPORTED_SCHEMA_VERSION: u64 = 1;

/// The manifest schema a package declares itself against.
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

#[cfg(test)]
mod tests {
    use super::{EmptyText, NonEmptyText, SchemaVersion, SchemaVersionError};

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
}
