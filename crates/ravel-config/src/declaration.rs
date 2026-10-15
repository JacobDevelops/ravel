use std::collections::BTreeSet;
use std::fmt;
use std::num::NonZeroU32;
use std::str::FromStr;

use ravel_core::{FailureKind, ModId, Side, StructuredFailure};
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::SchemaVersion;
use crate::schema::{ValueError, ValueSchema};

/// The separators `configuration.schema.json`'s `key` pattern admits.
const SEPARATORS: [char; 3] = ['.', '_', '-'];

/// One setting's key within its owner's declaration.
///
/// The grammar is `configuration.schema.json`'s `key` pattern,
/// `^[a-z][a-z0-9]*(?:[._-][a-z0-9]+)*$`, implemented here rather than matched
/// so the crate needs no regex engine, exactly as [`ravel_core::ModId`] does for
/// the pattern it carries. The two accept the same strings and are still
/// separate types: a key names a setting inside one owner's document and an id
/// names a mod across the whole installation, so a value that is one is never
/// the other and a refusal has to say which it was reading.
///
/// # Examples
///
/// ```
/// use ravel_config::{SettingKey, SettingKeyError};
///
/// let key: SettingKey = "render.distance".parse()?;
/// assert_eq!(key.as_str(), "render.distance");
///
/// assert!("Render".parse::<SettingKey>().is_err());
/// assert!("render.".parse::<SettingKey>().is_err());
/// # Ok::<(), SettingKeyError>(())
/// ```
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(into = "String", try_from = "String")]
pub struct SettingKey(String);

impl SettingKey {
    /// Returns the key as its canonical string form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(candidate: &str) -> Result<(), SettingKeyError> {
        if candidate.is_empty() {
            return Err(SettingKeyError::Empty);
        }

        let mut pending_separator = None;
        for (index, character) in candidate.chars().enumerate() {
            if index == 0 {
                if !character.is_ascii_lowercase() {
                    return Err(SettingKeyError::Leading { character });
                }
            } else if character.is_ascii_lowercase() || character.is_ascii_digit() {
                pending_separator = None;
            } else if SEPARATORS.contains(&character) {
                if pending_separator.is_some() {
                    return Err(SettingKeyError::Separator { character, index });
                }
                pending_separator = Some((character, index));
            } else {
                return Err(SettingKeyError::Character { character, index });
            }
        }

        if let Some((character, index)) = pending_separator {
            return Err(SettingKeyError::Separator { character, index });
        }

        Ok(())
    }
}

impl FromStr for SettingKey {
    type Err = SettingKeyError;

    fn from_str(candidate: &str) -> Result<Self, Self::Err> {
        Self::validate(candidate)?;
        Ok(Self(candidate.to_owned()))
    }
}

impl TryFrom<String> for SettingKey {
    type Error = SettingKeyError;

    fn try_from(candidate: String) -> Result<Self, Self::Error> {
        Self::validate(&candidate)?;
        Ok(Self(candidate))
    }
}

impl From<SettingKey> for String {
    fn from(key: SettingKey) -> Self {
        key.0
    }
}

impl fmt::Display for SettingKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Why a candidate string is not a valid [`SettingKey`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingKeyError {
    /// The candidate is empty.
    Empty,
    /// The first character is not an ASCII lowercase letter.
    Leading {
        /// The rejected first character.
        character: char,
    },
    /// A later character is neither `[a-z0-9]` nor a separator.
    Character {
        /// The rejected character.
        character: char,
        /// Zero-based character index of the rejection.
        index: usize,
    },
    /// A separator ends the key or immediately follows another separator.
    Separator {
        /// The misplaced separator.
        character: char,
        /// Zero-based character index of the rejection.
        index: usize,
    },
}

impl fmt::Display for SettingKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("a setting key cannot be empty"),
            Self::Leading { character } => write!(
                formatter,
                "a setting key starts with an ASCII lowercase letter, not `{character}`"
            ),
            Self::Character { character, index } => write!(
                formatter,
                "a setting key holds only `a`-`z`, `0`-`9`, `.`, `_`, and `-`, but character \
                 {index} is `{character}`"
            ),
            Self::Separator { character, index } => write!(
                formatter,
                "a setting key separator is always followed by a letter or a digit, but \
                 `{character}` at character {index} is not"
            ),
        }
    }
}

impl std::error::Error for SettingKeyError {}

/// Where a setting applies (`configuration.schema.json`'s `side`).
///
/// Deliberately not [`ravel_core::Side`]. The schema admits a fourth value,
/// `common`, which is not a role a process runs as but a statement that the
/// setting belongs to every role there is, so mapping the four onto the three
/// running sides is what [`SettingSide::applies_to`] exists for.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SettingSide {
    /// The rendering and input side of the client.
    Client,
    /// The server hosted inside a single-player client process.
    IntegratedServer,
    /// A standalone server process.
    DedicatedServer,
    /// Every side.
    Common,
}

impl SettingSide {
    /// Every value, in the order the schema lists them.
    pub const ALL: [Self; 4] = [
        Self::Client,
        Self::IntegratedServer,
        Self::DedicatedServer,
        Self::Common,
    ];

    /// The name this value is written under, matching its serialized form.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::IntegratedServer => "integrated-server",
            Self::DedicatedServer => "dedicated-server",
            Self::Common => "common",
        }
    }

    /// Returns whether a process running as `side` owns this setting.
    ///
    /// Screen contents follow ownership: a
    /// `dedicated-server` setting has no client screen, so a client asking for
    /// its own settings never sees one.
    #[must_use]
    pub const fn applies_to(self, side: Side) -> bool {
        match self {
            Self::Common => true,
            Self::Client => matches!(side, Side::Client),
            Self::IntegratedServer => matches!(side, Side::IntegratedServer),
            Self::DedicatedServer => matches!(side, Side::DedicatedServer),
        }
    }
}

/// Who has the last word on a setting when a pack states a value for it.
///
/// Pack overrides apply per setting, and the three values carry the policy: `allowed`
/// lets the player change what the pack stated, `locked` does not where the pack
/// stated anything, and `required` obliges the pack to state a value at all.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PackOverride {
    /// A pack may state a value; the player still has the last word.
    #[default]
    Allowed,
    /// A pack may state a value; where it did, the pack has the last word.
    Locked,
    /// A pack must state a value, and the pack has the last word.
    Required,
}

impl PackOverride {
    /// Every policy, in the order the schema lists them.
    pub const ALL: [Self; 3] = [Self::Allowed, Self::Locked, Self::Required];

    /// Returns whether a pack that stated a value overrules the player's.
    ///
    /// This rule reads `packOverride` alone and never looks at `side`, so it
    /// permits a pack to lock a client-side setting.
    #[must_use]
    pub const fn overrules_player(self) -> bool {
        matches!(self, Self::Locked | Self::Required)
    }

    /// Returns whether a pack that states any value for this owner must state
    /// one for this setting.
    #[must_use]
    pub const fn obliges_pack(self) -> bool {
        matches!(self, Self::Required)
    }
}

/// One setting a mod declares (`configuration.schema.json`'s `setting`).
///
/// Every field is the schema's, including the camel-case names and the refusal
/// of anything else: a setting that parses here has to be one the schema
/// validates. Nothing about presentation beyond `title` and `description` lives
/// here, because widgets live in Java and this crate supplies values.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Setting {
    /// The key this setting is named by inside its owner's documents.
    pub key: SettingKey,
    /// A human-readable name for the screen, defaulting to nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Free-form explanatory text for the screen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// What a legal value for this setting looks like.
    pub value_schema: ValueSchema,
    /// The author's value, and the bottom layer of every resolution.
    pub default: Value,
    /// Where this setting applies.
    pub side: SettingSide,
    /// Whether a change takes effect only on the next launch.
    pub restart_required: bool,
    /// Who has the last word when a pack states a value.
    #[serde(default)]
    pub pack_override: PackOverride,
}

/// Everything `configuration.schema.json` declares about one mod's settings.
///
/// # Examples
///
/// ```
/// use ravel_config::{ConfigurationDeclaration, SettingSide};
///
/// let document = r#"{
///   "schemaVersion": 1,
///   "owner": "example_mod",
///   "version": 1,
///   "settings": [
///     {
///       "key": "render.distance",
///       "valueSchema": { "type": "integer", "minimum": 2.0, "maximum": 32.0 },
///       "default": 12,
///       "side": "client",
///       "restartRequired": false
///     }
///   ]
/// }"#;
///
/// let declaration = ConfigurationDeclaration::parse(document)?;
/// let key = "render.distance".parse()?;
/// let setting = declaration.setting(&key).expect("the only setting");
///
/// assert_eq!(setting.side, SettingSide::Client);
/// // `packOverride` is absent above, and the schema defaults it to `allowed`.
/// assert!(!setting.pack_override.overrules_player());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ConfigurationDeclaration {
    /// The configuration contract this document was written against.
    pub schema_version: SchemaVersion,
    /// The mod whose settings these are.
    pub owner: ModId,
    /// The declaration's own version, which the author bumps as settings change.
    pub version: NonZeroU32,
    /// The settings, in the order the author declared them.
    #[serde(deserialize_with = "deserialize_unique_keys")]
    pub settings: Vec<Setting>,
}

impl ConfigurationDeclaration {
    /// Parses a mod's configuration declaration.
    ///
    /// # Errors
    ///
    /// Returns [`DeclarationError::Syntax`] when the bytes are not JSON and
    /// [`DeclarationError::Schema`] when they are JSON the configuration schema
    /// refuses, which includes a settings array that names one key twice: the
    /// schema cannot express uniqueness over a field of an item, and two
    /// settings under one key give every layer two answers.
    ///
    /// Returns [`DeclarationError::Default`] for a setting whose own `default`
    /// its own `valueSchema` refuses. The schema cannot express that either —
    /// the two are siblings and neither describes the other — and an author
    /// whose default is illegal has declared a setting with no legal bottom
    /// layer for anything above it to fall back to.
    pub fn parse(document: &str) -> Result<Self, DeclarationError> {
        let declaration: Self =
            serde_json::from_str(document).map_err(|error| DeclarationError::from_json(&error))?;
        for setting in &declaration.settings {
            if let Err(reason) = setting.value_schema.check(&setting.default) {
                return Err(DeclarationError::Default {
                    key: setting.key.clone(),
                    reason,
                });
            }
        }
        Ok(declaration)
    }

    /// Renders the declaration back into the JSON document form it parses from.
    ///
    /// # Panics
    ///
    /// Never. Every field is a string, a number, a sequence, or a struct with
    /// string keys, so the only way this fires is a serializer that has stopped
    /// being one.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a parsed declaration always serializes")
    }

    /// Returns the setting `key` names, if the author declared one.
    #[must_use]
    pub fn setting(&self, key: &SettingKey) -> Option<&Setting> {
        self.settings.iter().find(|setting| &setting.key == key)
    }
}

/// Refuses a settings array that names one key twice.
fn deserialize_unique_keys<'de, D>(deserializer: D) -> Result<Vec<Setting>, D::Error>
where
    D: Deserializer<'de>,
{
    let settings = Vec::<Setting>::deserialize(deserializer)?;
    let mut seen = BTreeSet::new();
    for setting in &settings {
        if !seen.insert(&setting.key) {
            return Err(de::Error::custom(format_args!(
                "`{}` is declared more than once",
                setting.key
            )));
        }
    }
    Ok(settings)
}

/// Why a configuration declaration could not be read.
#[derive(Clone, Debug, PartialEq)]
pub enum DeclarationError {
    /// The bytes are not a JSON document.
    Syntax(String),
    /// The document is JSON that `configuration.schema.json` refuses.
    Schema(String),
    /// A setting's own `default` is a value its own `valueSchema` refuses.
    Default {
        /// The setting that contradicts itself.
        key: SettingKey,
        /// What the value schema said about the default.
        reason: ValueError,
    },
}

impl DeclarationError {
    fn from_json(error: &serde_json::Error) -> Self {
        match error.classify() {
            serde_json::error::Category::Data => Self::Schema(error.to_string()),
            _ => Self::Syntax(error.to_string()),
        }
    }
}

impl fmt::Display for DeclarationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(detail) => {
                write!(formatter, "the declaration is not JSON: {detail}")
            }
            Self::Schema(detail) => write!(
                formatter,
                "the declaration breaks the configuration schema: {detail}"
            ),
            Self::Default { key, reason } => write!(
                formatter,
                "`{key}` declares a default its own value schema refuses: {reason}"
            ),
        }
    }
}

impl std::error::Error for DeclarationError {}

impl StructuredFailure for DeclarationError {
    /// A declaration that will not read has no `SPEC.md` §9.2 kind. That list
    /// names dependency-graph outcomes, and the graph is not what failed.
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    /// The detail is the deserializer's sentence or a key the author wrote, so
    /// a report attributes the words to the mod rather than to Ravel.
    fn quotes_untrusted_text(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{LifecycleState, Side, StructuredFailure};
    use serde_json::json;

    use super::{
        ConfigurationDeclaration, DeclarationError, PackOverride, SettingKey, SettingKeyError,
        SettingSide,
    };
    use crate::SchemaVersion;
    use crate::schema::{ValueError, ValueKind};

    /// A declaration holding one client integer setting, with `extra` spliced
    /// into the settings array.
    fn document(extra: &str) -> String {
        format!(
            r#"{{
              "schemaVersion": 1,
              "owner": "example_mod",
              "version": 1,
              "settings": [
                {{
                  "key": "render.distance",
                  "title": "Render distance",
                  "valueSchema": {{ "type": "integer", "minimum": 2.0 }},
                  "default": 12,
                  "side": "client",
                  "restartRequired": false
                }}{extra}
              ]
            }}"#
        )
    }

    #[test]
    fn a_declaration_carries_every_field_the_schema_names() {
        let declaration =
            ConfigurationDeclaration::parse(&document("")).expect("a legal declaration");

        assert_eq!(declaration.schema_version, SchemaVersion::CURRENT);
        assert_eq!(declaration.owner.as_str(), "example_mod");
        assert_eq!(declaration.version.get(), 1);

        let key: SettingKey = "render.distance".parse().expect("a legal key");
        let setting = declaration.setting(&key).expect("the declared setting");
        assert_eq!(setting.title.as_deref(), Some("Render distance"));
        assert_eq!(setting.description, None);
        assert_eq!(setting.default, json!(12));
        assert_eq!(setting.side, SettingSide::Client);
        assert!(!setting.restart_required);
        // Absent in the document, and the schema defaults it.
        assert_eq!(setting.pack_override, PackOverride::Allowed);
    }

    #[test]
    fn a_key_nobody_declared_has_no_setting() {
        let declaration =
            ConfigurationDeclaration::parse(&document("")).expect("a legal declaration");
        let absent: SettingKey = "render.absent".parse().expect("a legal key");
        assert_eq!(declaration.setting(&absent), None);
    }

    #[test]
    fn a_declaration_renders_back_to_the_document_it_parsed_from() {
        let declaration =
            ConfigurationDeclaration::parse(&document("")).expect("a legal declaration");
        let rendered = declaration.to_json();

        assert_eq!(
            ConfigurationDeclaration::parse(&rendered).expect("a rendered declaration re-parses"),
            declaration
        );
        // An absent optional field stays absent rather than becoming a null.
        assert!(!rendered.contains("description"), "{rendered}");
    }

    #[test]
    fn two_settings_under_one_key_are_refused_rather_than_shadowed() {
        let repeated = r#",
                {
                  "key": "render.distance",
                  "valueSchema": { "type": "integer" },
                  "default": 4,
                  "side": "client",
                  "restartRequired": false
                }"#;
        let error =
            ConfigurationDeclaration::parse(&document(repeated)).expect_err("one key, twice");
        assert!(
            matches!(&error, DeclarationError::Schema(detail) if detail.contains("more than once")),
            "{error}"
        );
    }

    #[test]
    fn a_default_the_authors_own_schema_refuses_is_refused() {
        let contradictory = r#"{
          "schemaVersion": 1,
          "owner": "example_mod",
          "version": 1,
          "settings": [
            {
              "key": "render.distance",
              "valueSchema": { "type": "integer" },
              "default": "twelve",
              "side": "client",
              "restartRequired": false
            }
          ]
        }"#;

        let error =
            ConfigurationDeclaration::parse(contradictory).expect_err("the default is a string");
        assert_eq!(
            error,
            DeclarationError::Default {
                key: "render.distance".parse().expect("a legal key"),
                reason: ValueError::Kind {
                    expected: ValueKind::Integer,
                    found: "string",
                },
            }
        );
        assert!(error.to_string().contains("its own value schema refuses"));
    }

    #[test]
    fn a_document_that_is_not_json_and_one_that_breaks_the_schema_are_told_apart() {
        let syntax = ConfigurationDeclaration::parse("{").expect_err("truncated");
        assert!(matches!(syntax, DeclarationError::Syntax(_)), "{syntax}");
        assert!(syntax.to_string().contains("not JSON"));

        for refused in [
            r#"{"schemaVersion": 2, "owner": "example_mod", "version": 1, "settings": []}"#,
            r#"{"schemaVersion": 1, "owner": "Example", "version": 1, "settings": []}"#,
            r#"{"schemaVersion": 1, "owner": "example_mod", "version": 0, "settings": []}"#,
            r#"{"schemaVersion": 1, "owner": "example_mod", "version": 1, "settings": [], "extra": 1}"#,
        ] {
            let error = ConfigurationDeclaration::parse(refused).expect_err("{refused}");
            assert!(
                matches!(error, DeclarationError::Schema(_)),
                "{refused} gave {error}"
            );
        }
    }

    #[test]
    fn every_declaration_refusal_reports_the_phase_and_whose_words_it_quotes() {
        for error in [
            ConfigurationDeclaration::parse("{").expect_err("truncated"),
            ConfigurationDeclaration::parse(r#"{"schemaVersion": 2}"#).expect_err("wrong version"),
            DeclarationError::Default {
                key: "render.distance".parse().expect("a legal key"),
                reason: ValueError::Below { minimum: 2.0 },
            },
        ] {
            assert_eq!(error.failure_kind(), None);
            // Nothing has been admitted yet, which is where a metadata refusal
            // leaves the package.
            assert_eq!(error.lifecycle_state(), LifecycleState::Discovered);
            assert!(error.quotes_untrusted_text());
            assert_ne!(error.diagnostic().text(), "");
        }
    }

    #[test]
    fn common_applies_everywhere_and_every_other_value_to_its_own_side_only() {
        for side in Side::ALL {
            assert!(SettingSide::Common.applies_to(side));
        }
        assert!(SettingSide::Client.applies_to(Side::Client));
        assert!(!SettingSide::Client.applies_to(Side::DedicatedServer));
        assert!(SettingSide::IntegratedServer.applies_to(Side::IntegratedServer));
        assert!(!SettingSide::IntegratedServer.applies_to(Side::Client));
        assert!(SettingSide::DedicatedServer.applies_to(Side::DedicatedServer));
        assert!(!SettingSide::DedicatedServer.applies_to(Side::IntegratedServer));
    }

    #[test]
    fn every_side_and_policy_round_trips_under_the_name_the_schema_uses() {
        for side in SettingSide::ALL {
            let rendered = serde_json::to_string(&side).expect("a side serializes");
            assert_eq!(
                serde_json::from_str::<SettingSide>(&rendered).expect("a side deserializes"),
                side
            );
        }
        assert_eq!(
            serde_json::to_string(&SettingSide::DedicatedServer).expect("serializes"),
            "\"dedicated-server\""
        );

        for policy in PackOverride::ALL {
            let rendered = serde_json::to_string(&policy).expect("a policy serializes");
            assert_eq!(
                serde_json::from_str::<PackOverride>(&rendered).expect("a policy deserializes"),
                policy
            );
        }
        assert_eq!(
            serde_json::to_string(&PackOverride::Required).expect("serializes"),
            "\"required\""
        );
    }

    #[test]
    fn only_locked_and_required_take_the_last_word_and_only_required_obliges_a_pack() {
        assert!(!PackOverride::Allowed.overrules_player());
        assert!(PackOverride::Locked.overrules_player());
        assert!(PackOverride::Required.overrules_player());

        assert!(!PackOverride::Allowed.obliges_pack());
        assert!(!PackOverride::Locked.obliges_pack());
        assert!(PackOverride::Required.obliges_pack());
    }

    #[test]
    fn accepted_keys_cover_every_position_of_the_grammar() {
        for candidate in ["a", "ab", "key9", "render.distance", "a-b_c.d9"] {
            let key: SettingKey = candidate.parse().expect("a legal key");
            assert_eq!(key.as_str(), candidate);
            assert_eq!(key.to_string(), candidate);
            assert_eq!(String::from(key), candidate);
        }
    }

    #[test]
    fn rejected_keys_report_the_rule_they_break() {
        for (candidate, expected) in [
            ("", SettingKeyError::Empty),
            ("Render", SettingKeyError::Leading { character: 'R' }),
            ("9lives", SettingKeyError::Leading { character: '9' }),
            (
                "render distance",
                SettingKeyError::Character {
                    character: ' ',
                    index: 6,
                },
            ),
            (
                "render.",
                SettingKeyError::Separator {
                    character: '.',
                    index: 6,
                },
            ),
            (
                "render..distance",
                SettingKeyError::Separator {
                    character: '.',
                    index: 7,
                },
            ),
        ] {
            assert_eq!(
                candidate.parse::<SettingKey>().expect_err(candidate),
                expected
            );
            assert!(expected.to_string().contains("a setting key"));
            assert!(SettingKey::try_from(candidate.to_owned()).is_err());
        }
    }
}
