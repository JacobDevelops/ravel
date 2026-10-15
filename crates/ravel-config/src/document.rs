use std::collections::BTreeMap;
use std::fmt;

use ravel_core::{FailureKind, ModId, StructuredFailure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::SchemaVersion;
use crate::declaration::SettingKey;

/// Per-setting pack or player overrides for one owner; absent keys inherit lower layers.
///
/// # Examples
///
/// ```
/// use ravel_config::ValueDocument;
///
/// let document = r#"{
///   "schemaVersion": 1,
///   "owner": "example_mod",
///   "values": { "render.distance": 8 }
/// }"#;
///
/// let stated = ValueDocument::parse(document)?;
/// let key = "render.distance".parse()?;
/// assert_eq!(stated.values.get(&key), Some(&serde_json::json!(8)));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ValueDocument {
    /// The configuration contract this document was written against.
    pub schema_version: SchemaVersion,
    /// The mod whose settings these are.
    pub owner: ModId,
    /// The value stated for each named setting.
    pub values: BTreeMap<SettingKey, Value>,
}

impl ValueDocument {
    /// A document stating nothing for `owner`, at the version this crate writes.
    #[must_use]
    pub fn empty(owner: ModId) -> Self {
        Self {
            schema_version: SchemaVersion::CURRENT,
            owner,
            values: BTreeMap::new(),
        }
    }

    /// Parses a pack override document or a player's values file.
    ///
    /// Values are checked against the declaration by [`crate::LayeredConfiguration::resolve`].
    ///
    /// # Errors
    ///
    /// Returns syntax or override-schema errors.
    pub fn parse(document: &str) -> Result<Self, DocumentError> {
        serde_json::from_str(document).map_err(|error| DocumentError::from_json(&error))
    }

    /// Renders the document back into the JSON form it parses from.
    ///
    /// # Panics
    ///
    /// Panics if serialization of the document's JSON-compatible fields fails.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a parsed document always serializes")
    }
}

/// Why a values document could not be read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentError {
    /// The bytes are not a JSON document.
    Syntax(String),
    /// The document is JSON that `configuration-override.schema.json` refuses.
    Schema(String),
}

impl DocumentError {
    fn from_json(error: &serde_json::Error) -> Self {
        match error.classify() {
            serde_json::error::Category::Data => Self::Schema(error.to_string()),
            serde_json::error::Category::Io
            | serde_json::error::Category::Syntax
            | serde_json::error::Category::Eof => Self::Syntax(error.to_string()),
        }
    }
}

impl fmt::Display for DocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(detail) => {
                write!(formatter, "the values document is not JSON: {detail}")
            }
            Self::Schema(detail) => write!(
                formatter,
                "the values document breaks the override schema: {detail}"
            ),
        }
    }
}

impl std::error::Error for DocumentError {}

impl StructuredFailure for DocumentError {
    /// Configuration errors are not dependency-graph failures.
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    /// Deserializer details can quote pack or player input.
    fn quotes_untrusted_text(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{LifecycleState, ModId, StructuredFailure};
    use serde_json::json;

    use super::{DocumentError, ValueDocument};
    use crate::SchemaVersion;
    use crate::declaration::SettingKey;

    fn key(text: &str) -> SettingKey {
        text.parse().expect("a legal setting key")
    }

    fn owner() -> ModId {
        "example_mod".parse().expect("a legal mod id")
    }

    const STATED: &str = r#"{
      "schemaVersion": 1,
      "owner": "example_mod",
      "values": { "render.distance": 8, "audio.volume": 0.5 }
    }"#;

    #[test]
    fn a_document_carries_the_value_stated_for_each_named_setting() {
        let stated = ValueDocument::parse(STATED).expect("a legal document");

        assert_eq!(stated.schema_version, SchemaVersion::CURRENT);
        assert_eq!(stated.owner, owner());
        assert_eq!(stated.values.get(&key("render.distance")), Some(&json!(8)));
        assert_eq!(stated.values.get(&key("audio.volume")), Some(&json!(0.5)));
        assert_eq!(stated.values.get(&key("audio.absent")), None);
    }

    #[test]
    fn an_empty_document_states_nothing_and_still_names_its_owner() {
        let empty = ValueDocument::empty(owner());
        assert_eq!(empty.schema_version, SchemaVersion::CURRENT);
        assert_eq!(empty.owner, owner());
        assert!(empty.values.is_empty());
        assert_eq!(
            ValueDocument::parse(&empty.to_json()).expect("an empty document re-parses"),
            empty
        );
    }

    #[test]
    fn a_document_renders_back_to_the_document_it_parsed_from() {
        let stated = ValueDocument::parse(STATED).expect("a legal document");
        assert_eq!(
            ValueDocument::parse(&stated.to_json()).expect("a rendered document re-parses"),
            stated
        );
    }

    #[test]
    fn a_document_that_is_not_json_and_one_that_breaks_the_schema_are_told_apart() {
        let syntax = ValueDocument::parse("{").expect_err("truncated");
        assert!(matches!(syntax, DocumentError::Syntax(_)), "{syntax}");
        assert!(syntax.to_string().contains("not JSON"));

        for refused in [
            r#"{"schemaVersion": 2, "owner": "example_mod", "values": {}}"#,
            r#"{"schemaVersion": 1, "owner": "Example", "values": {}}"#,
            r#"{"schemaVersion": 1, "owner": "example_mod", "values": {"Render": 1}}"#,
            r#"{"schemaVersion": 1, "owner": "example_mod", "values": {}, "extra": 1}"#,
        ] {
            let error = ValueDocument::parse(refused).expect_err(refused);
            assert!(
                matches!(error, DocumentError::Schema(_)),
                "{refused} gave {error}"
            );
        }
    }

    #[test]
    fn every_document_refusal_reports_the_phase_and_whose_words_it_quotes() {
        for error in [
            ValueDocument::parse("{").expect_err("truncated"),
            ValueDocument::parse(r#"{"schemaVersion": 2}"#).expect_err("wrong version"),
        ] {
            assert_eq!(error.failure_kind(), None);
            assert_eq!(error.lifecycle_state(), LifecycleState::Discovered);
            assert!(error.quotes_untrusted_text());
            assert_ne!(error.diagnostic().text(), "");
        }
    }
}
