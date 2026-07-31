#![forbid(unsafe_code)]

//! Configuration schemas, validation, and layered override contracts.
//!
//! ADG-6 declares configuration in data and draws the screen in Java, so this
//! crate owns the values and holds no widget and no Minecraft version detail. A
//! setting resolves through three layers — the author's `default`, then the
//! pack's override document, then the player's value — and the author's
//! `packOverride` decides which of the last two has the final word.
//!
//! The three documents are [`ConfigurationDeclaration`], which is what a mod
//! author writes, and two [`ValueDocument`]s, which are what a pack and a player
//! state. [`LayeredConfiguration`] is where they meet.

mod declaration;
mod document;
pub mod file;
mod layered;
mod schema;

use std::fmt;

use serde::{Deserialize, Serialize};

pub use declaration::{
    ConfigurationDeclaration, DeclarationError, PackOverride, Setting, SettingKey, SettingKeyError,
    SettingSide,
};
pub use document::{DocumentError, ValueDocument};
pub use layered::{
    EditError, EditOutcome, LayerError, LayeredConfiguration, Rejection, RejectionReason,
    ResolvedSetting, ValueSource,
};
pub use schema::{ValueError, ValueKind, ValueSchema};

/// The only `schemaVersion` the configuration documents accept.
const SUPPORTED_SCHEMA_VERSION: u64 = 1;

/// The configuration contract a document was written against.
///
/// `configuration.schema.json` and `configuration-override.schema.json` both pin
/// `schemaVersion` to the constant 1, and it is a version negotiation point
/// rather than decoration: a document declaring anything else was written
/// against rules this crate does not implement, and reading it anyway would
/// apply this version's rules to a document that never agreed to them.
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

/// A configuration document declared a schema version this crate does not
/// implement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchemaVersionError {
    /// The version the document declared.
    pub declared: u64,
}

impl fmt::Display for SchemaVersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a configuration document declares schemaVersion {SUPPORTED_SCHEMA_VERSION}, not {}",
            self.declared
        )
    }
}

impl std::error::Error for SchemaVersionError {}

#[cfg(test)]
mod tests {
    use super::{SchemaVersion, SchemaVersionError};

    #[test]
    fn the_only_accepted_version_is_the_one_this_crate_implements() {
        assert_eq!(SchemaVersion::CURRENT.get(), 1);
        assert_eq!(SchemaVersion::try_from(1), Ok(SchemaVersion::CURRENT));
        assert_eq!(u64::from(SchemaVersion::CURRENT), 1);
        assert_eq!(SchemaVersion::CURRENT.to_string(), "1");
    }

    #[test]
    fn any_other_version_is_refused_rather_than_read_under_these_rules() {
        for declared in [0, 2, 99] {
            let error = SchemaVersion::try_from(declared).expect_err("not this contract");
            assert_eq!(error, SchemaVersionError { declared });
            assert!(error.to_string().contains(&format!("not {declared}")));
        }
    }

    #[test]
    fn a_version_round_trips_through_the_number_it_serializes_as() {
        let rendered = serde_json::to_string(&SchemaVersion::CURRENT).expect("serializes");
        assert_eq!(rendered, "1");
        assert_eq!(
            serde_json::from_str::<SchemaVersion>(&rendered).expect("deserializes"),
            SchemaVersion::CURRENT
        );
    }
}
