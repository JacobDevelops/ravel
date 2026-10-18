use std::collections::BTreeMap;
use std::fmt;

use ravel_core::{FailureKind, LifecycleState, ModId, Side, StructuredFailure};
use serde_json::Value;

use crate::declaration::{ConfigurationDeclaration, Setting, SettingKey, SettingSide};
use crate::document::ValueDocument;
use crate::schema::{ValueError, ValueKind};

/// Which layer an effective value came from.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ValueSource {
    /// The author's declared `default`, which every setting always has.
    Default,
    /// The pack's override document.
    Pack,
    /// The player's own value.
    Player,
}

/// One setting as a screen on one side sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedSetting<'a> {
    /// What the author declared.
    pub setting: &'a Setting,
    /// The value in force for this session.
    pub value: &'a Value,
    /// The layer the value came from.
    pub source: ValueSource,
    /// Whether the player may change it.
    ///
    /// A setting reaches a screen only on a side that owns it, so within one
    /// side's own settings the only thing that makes a value uneditable is a
    /// pack that took the last word on it.
    pub editable: bool,
}

/// A player value that did not survive loading.
#[derive(Clone, Debug, PartialEq)]
pub struct Rejection {
    /// The key the player's document named.
    pub key: SettingKey,
    /// Why it was dropped.
    pub reason: RejectionReason,
}

impl fmt::Display for Rejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.reason {
            RejectionReason::Unknown => write!(
                formatter,
                "`{}` is not a setting this mod declares, so its value was dropped",
                self.key
            ),
            RejectionReason::Illegal(reason) => write!(
                formatter,
                "`{}` held a value this mod refuses and fell back to the layer below: {reason}",
                self.key
            ),
        }
    }
}

/// Why a player value was dropped rather than applied.
#[derive(Clone, Debug, PartialEq)]
pub enum RejectionReason {
    /// The owner's declaration has no such setting.
    Unknown,
    /// The value is one the setting's `valueSchema` refuses.
    Illegal(ValueError),
}

/// What recording a player edit did.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EditOutcome {
    /// The value is written and is in force now.
    Now,
    /// The value is written, and the session keeps the old one until the next
    /// launch. The screen reports this at the moment of the edit.
    NextLaunch,
}

/// Defaults layered with pack and player values according to each setting's policy.
///
/// Invalid pack values fail resolution; invalid player values fall back and are reported.
///
/// # Examples
///
/// ```
/// use ravel_config::{ConfigurationDeclaration, LayeredConfiguration, ValueDocument, ValueSource};
///
/// let declaration = ConfigurationDeclaration::parse(r#"{
///   "schemaVersion": 1,
///   "owner": "example_mod",
///   "version": 1,
///   "settings": [
///     {
///       "key": "render.distance",
///       "valueSchema": { "type": "integer" },
///       "default": 12,
///       "side": "client",
///       "restartRequired": false,
///       "packOverride": "locked"
///     }
///   ]
/// }"#)?;
///
/// let pack = ValueDocument::parse(r#"{
///   "schemaVersion": 1,
///   "owner": "example_mod",
///   "values": { "render.distance": 6 }
/// }"#)?;
///
/// let config = LayeredConfiguration::resolve(declaration, Some(&pack), None)?;
/// let key = "render.distance".parse()?;
///
/// assert_eq!(config.value(&key), Some(&serde_json::json!(6)));
/// assert_eq!(config.source(&key), Some(ValueSource::Pack));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct LayeredConfiguration {
    declaration: ConfigurationDeclaration,
    pack: BTreeMap<SettingKey, Value>,
    /// What the player's document says, and so what is written back.
    written: BTreeMap<SettingKey, Value>,
    /// Session values, excluding restart-required edits made since loading.
    active: BTreeMap<SettingKey, Value>,
    rejected: Vec<Rejection>,
}

impl LayeredConfiguration {
    /// Layers one owner's declaration, a pack's overrides, and a player's file.
    ///
    /// # Errors
    ///
    /// Returns owner mismatches or unknown, invalid, or missing required pack values.
    /// Required values are checked only when a pack layer is present.
    pub fn resolve(
        declaration: ConfigurationDeclaration,
        pack: Option<&ValueDocument>,
        player: Option<&ValueDocument>,
    ) -> Result<Self, LayerError> {
        let mut layered = Self {
            pack: BTreeMap::new(),
            written: BTreeMap::new(),
            active: BTreeMap::new(),
            rejected: Vec::new(),
            declaration,
        };

        if let Some(stated) = pack {
            layered.check_owner(stated)?;
            for (key, value) in &stated.values {
                let setting = layered
                    .declaration
                    .setting(key)
                    .ok_or_else(|| LayerError::PackUnknown { key: key.clone() })?;
                setting
                    .value_schema
                    .check(value)
                    .map_err(|reason| LayerError::PackValue {
                        key: key.clone(),
                        reason,
                    })?;
                layered.pack.insert(key.clone(), value.clone());
            }
            for setting in &layered.declaration.settings {
                if setting.pack_override.obliges_pack() && !layered.pack.contains_key(&setting.key)
                {
                    return Err(LayerError::PackMissing {
                        key: setting.key.clone(),
                    });
                }
            }
        }

        if let Some(chosen) = player {
            layered.check_owner(chosen)?;
            for (key, value) in &chosen.values {
                let reason = match layered.declaration.setting(key) {
                    None => RejectionReason::Unknown,
                    Some(setting) => match setting.value_schema.check(value) {
                        Ok(()) => {
                            layered.written.insert(key.clone(), value.clone());
                            layered.active.insert(key.clone(), value.clone());
                            continue;
                        }
                        Err(refusal) => RejectionReason::Illegal(refusal),
                    },
                };
                layered.rejected.push(Rejection {
                    key: key.clone(),
                    reason,
                });
            }
        }

        Ok(layered)
    }

    /// Refuses a document written for a different mod.
    fn check_owner(&self, document: &ValueDocument) -> Result<(), LayerError> {
        if document.owner == self.declaration.owner {
            Ok(())
        } else {
            Err(LayerError::Owner {
                expected: self.declaration.owner.clone(),
                found: document.owner.clone(),
            })
        }
    }

    /// Returns the mod these settings belong to.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        &self.declaration.owner
    }

    /// Returns the declaration the layers were resolved against.
    #[must_use]
    pub const fn declaration(&self) -> &ConfigurationDeclaration {
        &self.declaration
    }

    /// Returns every player value dropped when this loaded, in key order.
    #[must_use]
    pub fn rejected(&self) -> &[Rejection] {
        &self.rejected
    }

    /// Returns the value in force for `key`, or `None` for an undeclared one.
    #[must_use]
    pub fn value(&self, key: &SettingKey) -> Option<&Value> {
        Some(self.layer(self.declaration.setting(key)?).0)
    }

    /// Returns a boolean from a setting declared as boolean.
    pub fn boolean(&self, key: &str) -> Result<bool, ValueError> {
        self.typed_value(key, ValueKind::Boolean)?
            .as_bool()
            .ok_or(ValueError::Kind {
                expected: ValueKind::Boolean,
                found: "non-boolean",
            })
    }

    /// Returns a signed integer from a setting declared as integer.
    pub fn integer(&self, key: &str) -> Result<i64, ValueError> {
        self.typed_value(key, ValueKind::Integer)?
            .as_i64()
            .ok_or(ValueError::IntegerRange)
    }

    /// Returns a number from a setting declared as number.
    pub fn number(&self, key: &str) -> Result<f64, ValueError> {
        self.typed_value(key, ValueKind::Number)?
            .as_f64()
            .ok_or(ValueError::Kind {
                expected: ValueKind::Number,
                found: "non-number",
            })
    }

    /// Borrows text from a setting explicitly declared as string.
    ///
    /// Closed choices also require `"type": "string"`; an enum alone does not
    /// establish a scalar kind.
    pub fn text(&self, key: &str) -> Result<&str, ValueError> {
        self.typed_value(key, ValueKind::String)?
            .as_str()
            .ok_or(ValueError::Kind {
                expected: ValueKind::String,
                found: "non-string",
            })
    }

    fn typed_value(&self, key: &str, expected: ValueKind) -> Result<&Value, ValueError> {
        let setting = self
            .declaration
            .settings()
            .iter()
            .find(|setting| setting.key.as_str() == key)
            .ok_or_else(|| ValueError::Unknown {
                key: key.to_owned(),
            })?;
        let declared = setting.value_schema.kind;
        if declared != Some(expected) {
            return Err(ValueError::AccessKind { expected, declared });
        }
        Ok(self.layer(setting).0)
    }

    /// Returns the layer the value in force for `key` came from.
    #[must_use]
    pub fn source(&self, key: &SettingKey) -> Option<ValueSource> {
        Some(self.layer(self.declaration.setting(key)?).1)
    }

    /// Resolves one setting through the three layers.
    fn layer<'a>(&'a self, setting: &'a Setting) -> (&'a Value, ValueSource) {
        let stated = self.pack.get(&setting.key);
        if let Some(value) = stated
            && setting.pack_override.overrules_player()
        {
            return (value, ValueSource::Pack);
        }
        if let Some(value) = self.active.get(&setting.key) {
            return (value, ValueSource::Player);
        }
        if let Some(value) = stated {
            return (value, ValueSource::Pack);
        }
        (&setting.default, ValueSource::Default)
    }

    /// Returns whether the player may change `setting` from a screen on `side`.
    fn editable(&self, setting: &Setting, side: Side) -> bool {
        setting.side.applies_to(side)
            && !(setting.pack_override.overrules_player() && self.pack.contains_key(&setting.key))
    }

    /// Returns the settings a process running as `side` owns, in declared order.
    #[must_use]
    pub fn settings_on(&self, side: Side) -> Vec<ResolvedSetting<'_>> {
        self.declaration
            .settings
            .iter()
            .filter(|setting| setting.side.applies_to(side))
            .map(|setting| {
                let (value, source) = self.layer(setting);
                ResolvedSetting {
                    setting,
                    value,
                    source,
                    editable: self.editable(setting, side),
                }
            })
            .collect()
    }

    /// Records a player edit made from a screen on `side`.
    ///
    /// Restart-required edits are persisted but leave [`Self::value`] unchanged until reload.
    ///
    /// # Errors
    ///
    /// Returns [`EditError::Unknown`] for a key the owner does not declare,
    /// [`EditError::OtherSide`] for a setting this side does not own,
    /// [`EditError::FixedByPack`] where the pack took the last word, and
    /// [`EditError::Illegal`] for a value the setting's `valueSchema` refuses.
    pub fn set(
        &mut self,
        key: &SettingKey,
        value: Value,
        side: Side,
    ) -> Result<EditOutcome, EditError> {
        let setting = self
            .declaration
            .setting(key)
            .ok_or_else(|| EditError::Unknown { key: key.clone() })?;

        if !setting.side.applies_to(side) {
            return Err(EditError::OtherSide {
                key: key.clone(),
                declared: setting.side,
                side,
            });
        }
        if setting.pack_override.overrules_player() && self.pack.contains_key(key) {
            return Err(EditError::FixedByPack { key: key.clone() });
        }
        setting
            .value_schema
            .check(&value)
            .map_err(|reason| EditError::Illegal {
                key: key.clone(),
                reason,
            })?;
        let restart_required = setting.restart_required;

        self.written.insert(key.clone(), value.clone());
        if restart_required {
            return Ok(EditOutcome::NextLaunch);
        }
        self.active.insert(key.clone(), value);
        Ok(EditOutcome::Now)
    }

    /// Returns whether a written edit is waiting for the next launch.
    #[must_use]
    pub fn restart_pending(&self) -> bool {
        self.written != self.active
    }

    /// Returns the player's layer as the document it is written back as.
    #[must_use]
    pub fn player_document(&self) -> ValueDocument {
        ValueDocument {
            schema_version: crate::SchemaVersion::CURRENT,
            owner: self.declaration.owner.clone(),
            values: self.written.clone(),
        }
    }
}

/// Pack-layer or owner errors that prevent resolution; player-value errors are rejections.
#[derive(Clone, Debug, PartialEq)]
pub enum LayerError {
    /// A document belongs to a different mod than the declaration.
    Owner {
        /// The mod the declaration is for.
        expected: ModId,
        /// The mod the document names.
        found: ModId,
    },
    /// The pack states a value for a setting the owner does not declare.
    PackUnknown {
        /// The key the pack named.
        key: SettingKey,
    },
    /// The pack states a value the setting's `valueSchema` refuses.
    PackValue {
        /// The setting the pack stated a value for.
        key: SettingKey,
        /// What the value schema said about it.
        reason: ValueError,
    },
    /// The pack omits a setting the author marked `required`.
    PackMissing {
        /// The setting the pack had to state and did not.
        key: SettingKey,
    },
}

impl fmt::Display for LayerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Owner { expected, found } => write!(
                formatter,
                "a values document for `{found}` was layered onto `{expected}`'s declaration"
            ),
            Self::PackUnknown { key } => write!(
                formatter,
                "the pack states `{key}`, which this mod does not declare"
            ),
            Self::PackValue { key, reason } => write!(
                formatter,
                "the pack states a value for `{key}` that the mod refuses: {reason}"
            ),
            Self::PackMissing { key } => write!(
                formatter,
                "`{key}` is required of the pack and the pack states no value for it"
            ),
        }
    }
}

impl std::error::Error for LayerError {}

impl StructuredFailure for LayerError {
    /// Configuration errors are not dependency-graph failures.
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    /// A pack's overrides are layered once its mod set is accepted and before
    /// anything constructs a mod, which is [`LifecycleState::Resolved`].
    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Resolved
    }

    /// Messages contain validated identifiers and schema descriptions, not raw values.
    fn quotes_untrusted_text(&self) -> bool {
        false
    }
}

/// A player edit rejected without changing the active value.
#[derive(Clone, Debug, PartialEq)]
pub enum EditError {
    /// The owner declares no such setting.
    Unknown {
        /// The key the edit named.
        key: SettingKey,
    },
    /// The setting belongs to a side this process is not running as.
    OtherSide {
        /// The setting the edit named.
        key: SettingKey,
        /// The side the author declared it for.
        declared: SettingSide,
        /// The side the edit came from.
        side: Side,
    },
    /// The pack took the last word on this setting.
    FixedByPack {
        /// The setting the pack fixed.
        key: SettingKey,
    },
    /// The value is one the setting's `valueSchema` refuses.
    Illegal {
        /// The setting the edit named.
        key: SettingKey,
        /// What the value schema said about the value.
        reason: ValueError,
    },
}

impl EditError {
    /// The setting the refused edit named.
    #[must_use]
    pub const fn key(&self) -> &SettingKey {
        match self {
            Self::Unknown { key }
            | Self::OtherSide { key, .. }
            | Self::FixedByPack { key }
            | Self::Illegal { key, .. } => key,
        }
    }
}

impl fmt::Display for EditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown { key } => {
                write!(formatter, "`{key}` is not a setting this mod declares")
            }
            Self::OtherSide { key, declared, .. } => write!(
                formatter,
                "`{key}` is a {} setting, which this side does not own",
                declared.as_str()
            ),
            Self::FixedByPack { key } => write!(
                formatter,
                "`{key}` is fixed by the pack and cannot be changed here"
            ),
            Self::Illegal { key, reason } => {
                write!(formatter, "`{key}` was left unchanged: {reason}")
            }
        }
    }
}

impl std::error::Error for EditError {}

#[cfg(test)]
mod tests {
    use ravel_core::{LifecycleState, ModId, Side, StructuredFailure};
    use serde_json::{Value, json};

    use super::{
        EditError, EditOutcome, LayerError, LayeredConfiguration, Rejection, RejectionReason,
        ValueSource,
    };
    use crate::declaration::{ConfigurationDeclaration, SettingKey, SettingSide};
    use crate::document::ValueDocument;
    use crate::schema::{ValueError, ValueKind};

    fn key(text: &str) -> SettingKey {
        text.parse().expect("a legal setting key")
    }

    fn owner() -> ModId {
        "example_mod".parse().expect("a legal mod id")
    }

    /// A declaration with one setting per `packOverride`, one per side, and one
    /// that needs a restart.
    fn declaration() -> ConfigurationDeclaration {
        ConfigurationDeclaration::parse(
            r#"{
              "schemaVersion": 1,
              "owner": "example_mod",
              "version": 1,
              "settings": [
                {
                  "key": "render.distance",
                  "valueSchema": { "type": "integer", "minimum": 2.0, "maximum": 32.0 },
                  "default": 12,
                  "side": "client",
                  "restartRequired": false
                },
                {
                  "key": "world.difficulty",
                  "valueSchema": { "type": "string", "enum": ["calm", "harsh"] },
                  "default": "calm",
                  "side": "integrated-server",
                  "restartRequired": false,
                  "packOverride": "locked"
                },
                {
                  "key": "server.motd",
                  "valueSchema": { "type": "string", "minLength": 1 },
                  "default": "a server",
                  "side": "dedicated-server",
                  "restartRequired": false,
                  "packOverride": "required"
                },
                {
                  "key": "engine.threads",
                  "valueSchema": { "type": "integer", "minimum": 1.0 },
                  "default": 4,
                  "side": "common",
                  "restartRequired": true
                }
              ]
            }"#,
        )
        .expect("a legal declaration")
    }

    /// A values document stating each `(key, value)` pair for `example_mod`.
    fn document(values: &[(&str, Value)]) -> ValueDocument {
        let mut stated = ValueDocument::empty(owner());
        for (name, value) in values {
            stated.values.insert(key(name), value.clone());
        }
        stated
    }

    /// A pack stating only what `server.motd` obliges it to.
    fn minimal_pack() -> ValueDocument {
        document(&[("server.motd", json!("the pack's server"))])
    }

    fn resolved(
        pack: Option<&ValueDocument>,
        player: Option<&ValueDocument>,
    ) -> LayeredConfiguration {
        LayeredConfiguration::resolve(declaration(), pack, player).expect("legal layers")
    }

    #[test]
    fn typed_accessors_follow_layers_and_enforce_declared_kinds() {
        let mut config = resolved(None, Some(&document(&[("render.distance", json!(20))])));
        assert_eq!(config.integer("render.distance"), Ok(20));
        assert_eq!(config.text("world.difficulty"), Ok("calm"));
        assert_eq!(
            config.boolean("render.distance"),
            Err(ValueError::AccessKind {
                expected: ValueKind::Boolean,
                declared: Some(ValueKind::Integer),
            })
        );
        assert_eq!(
            config.text("absent"),
            Err(ValueError::Unknown {
                key: "absent".to_owned()
            })
        );
        assert_eq!(
            config.integer("Not a setting"),
            Err(ValueError::Unknown {
                key: "Not a setting".to_owned()
            })
        );
        config
            .set(&key("engine.threads"), json!(8), Side::Client)
            .expect("a valid edit");
        assert_eq!(config.integer("engine.threads"), Ok(4));
        let pack = document(&[
            ("world.difficulty", json!("harsh")),
            ("server.motd", json!("pack")),
        ]);
        assert_eq!(
            resolved(Some(&pack), None).text("world.difficulty"),
            Ok("harsh")
        );
    }

    #[test]
    fn typed_accessors_cover_numbers_booleans_untyped_schemas_and_integer_overflow() {
        let declaration = ConfigurationDeclaration::parse(r#"{
          "schemaVersion":1,"owner":"example_mod","version":1,"settings":[
            {"key":"enabled","valueSchema":{"type":"boolean"},"default":true,"side":"common","restartRequired":false},
            {"key":"ratio","valueSchema":{"type":"number"},"default":1.5,"side":"common","restartRequired":false},
            {"key":"large","valueSchema":{"type":"integer"},"default":18446744073709551615,"side":"common","restartRequired":false},
            {"key":"untyped","valueSchema":{},"default":true,"side":"common","restartRequired":false}
          ]}"#).expect("valid defaults");
        let config = LayeredConfiguration::resolve(declaration, None, None).expect("valid layers");
        assert_eq!(config.boolean("enabled"), Ok(true));
        assert_eq!(config.number("ratio"), Ok(1.5));
        assert_eq!(config.integer("large"), Err(ValueError::IntegerRange));
        assert_eq!(
            config.boolean("untyped"),
            Err(ValueError::AccessKind {
                expected: ValueKind::Boolean,
                declared: None,
            })
        );
    }

    #[test]
    fn a_setting_nobody_overrode_resolves_to_the_authors_default() {
        let config = resolved(None, None);

        assert_eq!(config.owner(), &owner());
        assert_eq!(config.value(&key("render.distance")), Some(&json!(12)));
        assert_eq!(
            config.source(&key("render.distance")),
            Some(ValueSource::Default)
        );
        assert_eq!(config.rejected(), []);
        assert!(!config.restart_pending());
    }

    #[test]
    fn an_undeclared_key_resolves_to_nothing_at_all() {
        let config = resolved(None, None);
        assert_eq!(config.value(&key("render.absent")), None);
        assert_eq!(config.source(&key("render.absent")), None);
    }

    #[test]
    fn the_player_takes_the_last_word_from_a_pack_that_only_stated_a_value() {
        let pack = document(&[
            ("render.distance", json!(6)),
            ("server.motd", json!("the pack's server")),
        ]);
        let player = document(&[("render.distance", json!(24))]);

        // The pack alone beats the default, and the player beats the pack.
        let without = resolved(Some(&pack), None);
        assert_eq!(without.value(&key("render.distance")), Some(&json!(6)));
        assert_eq!(
            without.source(&key("render.distance")),
            Some(ValueSource::Pack)
        );

        let with = resolved(Some(&pack), Some(&player));
        assert_eq!(with.value(&key("render.distance")), Some(&json!(24)));
        assert_eq!(
            with.source(&key("render.distance")),
            Some(ValueSource::Player)
        );
    }

    #[test]
    fn a_locked_setting_the_pack_stated_keeps_the_packs_value_over_the_players() {
        let pack = document(&[
            ("world.difficulty", json!("harsh")),
            ("server.motd", json!("the pack's server")),
        ]);
        let player = document(&[("world.difficulty", json!("calm"))]);
        let config = resolved(Some(&pack), Some(&player));

        assert_eq!(
            config.value(&key("world.difficulty")),
            Some(&json!("harsh"))
        );
        assert_eq!(
            config.source(&key("world.difficulty")),
            Some(ValueSource::Pack)
        );
        // The player's value is kept in the file; it is simply not in force.
        assert_eq!(
            config
                .player_document()
                .values
                .get(&key("world.difficulty")),
            Some(&json!("calm"))
        );
    }

    #[test]
    fn a_locked_setting_the_pack_left_alone_is_still_the_players() {
        let player = document(&[("world.difficulty", json!("harsh"))]);
        let config = resolved(Some(&minimal_pack()), Some(&player));

        assert_eq!(
            config.value(&key("world.difficulty")),
            Some(&json!("harsh"))
        );
        assert_eq!(
            config.source(&key("world.difficulty")),
            Some(ValueSource::Player)
        );
        assert!(
            config
                .settings_on(Side::IntegratedServer)
                .iter()
                .all(|resolved| resolved.editable)
        );
    }

    #[test]
    fn a_required_setting_the_pack_states_beats_the_player() {
        let player = document(&[("server.motd", json!("the player's server"))]);
        let config = resolved(Some(&minimal_pack()), Some(&player));

        assert_eq!(
            config.value(&key("server.motd")),
            Some(&json!("the pack's server"))
        );
        assert_eq!(config.source(&key("server.motd")), Some(ValueSource::Pack));
    }

    #[test]
    fn a_pack_that_omits_a_required_setting_is_refused() {
        let error = LayeredConfiguration::resolve(
            declaration(),
            Some(&document(&[("render.distance", json!(6))])),
            None,
        )
        .expect_err("`server.motd` is required of the pack");

        assert_eq!(
            error,
            LayerError::PackMissing {
                key: key("server.motd"),
            }
        );
        assert!(error.to_string().contains("required of the pack"));
    }

    #[test]
    fn an_installation_with_no_pack_has_nobody_to_oblige() {
        // `required` is an obligation on a pack, and there is no pack, so the
        // author's default is what the setting resolves to.
        let config = resolved(None, None);
        assert_eq!(config.value(&key("server.motd")), Some(&json!("a server")));
        assert_eq!(
            config.source(&key("server.motd")),
            Some(ValueSource::Default)
        );
    }

    #[test]
    fn a_pack_that_states_an_illegal_or_undeclared_value_is_refused_outright() {
        let illegal = LayeredConfiguration::resolve(
            declaration(),
            Some(&document(&[
                ("server.motd", json!("the pack's server")),
                ("render.distance", json!(64)),
            ])),
            None,
        )
        .expect_err("64 is above the declared maximum");
        assert_eq!(
            illegal,
            LayerError::PackValue {
                key: key("render.distance"),
                reason: ValueError::Above { maximum: 32.0 },
            }
        );

        let undeclared = LayeredConfiguration::resolve(
            declaration(),
            Some(&document(&[
                ("server.motd", json!("the pack's server")),
                ("render.absent", json!(1)),
            ])),
            None,
        )
        .expect_err("the mod declares no such setting");
        assert_eq!(
            undeclared,
            LayerError::PackUnknown {
                key: key("render.absent"),
            }
        );
    }

    #[test]
    fn a_document_written_for_another_mod_is_refused_from_either_layer() {
        let other: ModId = "other_mod".parse().expect("a legal mod id");
        let stranger = ValueDocument::empty(other.clone());
        let expected = LayerError::Owner {
            expected: owner(),
            found: other,
        };

        assert_eq!(
            LayeredConfiguration::resolve(declaration(), Some(&stranger), None)
                .expect_err("a pack for another mod"),
            expected
        );
        assert_eq!(
            LayeredConfiguration::resolve(declaration(), Some(&minimal_pack()), Some(&stranger))
                .expect_err("a player file for another mod"),
            expected
        );
        assert!(expected.to_string().contains("other_mod"));
    }

    #[test]
    fn an_illegal_player_value_falls_back_and_is_reported_rather_than_failing() {
        let player = document(&[
            ("render.distance", json!("far")),
            ("world.absent", json!(1)),
        ]);
        let config = resolved(None, Some(&player));

        assert_eq!(config.value(&key("render.distance")), Some(&json!(12)));
        assert_eq!(
            config.source(&key("render.distance")),
            Some(ValueSource::Default)
        );
        assert_eq!(
            config.rejected(),
            [
                Rejection {
                    key: key("render.distance"),
                    reason: RejectionReason::Illegal(ValueError::Kind {
                        expected: ValueKind::Integer,
                        found: "string",
                    }),
                },
                Rejection {
                    key: key("world.absent"),
                    reason: RejectionReason::Unknown,
                },
            ]
        );
        assert!(config.player_document().values.is_empty());
        assert!(config.rejected()[0].to_string().contains("fell back"));
        assert!(config.rejected()[1].to_string().contains("not a setting"));
    }

    #[test]
    fn a_screen_holds_the_settings_its_own_side_owns_and_no_others() {
        let config = resolved(None, None);

        let client: Vec<&str> = config
            .settings_on(Side::Client)
            .iter()
            .map(|resolved| resolved.setting.key.as_str())
            .collect();
        assert_eq!(client, ["render.distance", "engine.threads"]);

        let dedicated: Vec<&str> = config
            .settings_on(Side::DedicatedServer)
            .iter()
            .map(|resolved| resolved.setting.key.as_str())
            .collect();
        assert_eq!(dedicated, ["server.motd", "engine.threads"]);

        // Every one of them is the player's to change while no pack has spoken.
        assert!(
            config
                .settings_on(Side::Client)
                .iter()
                .all(|resolved| resolved.editable)
        );
    }

    #[test]
    fn a_setting_the_pack_fixed_is_shown_and_is_not_editable() {
        let pack = document(&[
            ("world.difficulty", json!("harsh")),
            ("server.motd", json!("the pack's server")),
        ]);
        let config = resolved(Some(&pack), None);

        let fixed = config.settings_on(Side::IntegratedServer);
        let difficulty = fixed
            .iter()
            .find(|resolved| resolved.setting.key == key("world.difficulty"))
            .expect("the integrated-server setting");
        assert!(!difficulty.editable);
        assert_eq!(difficulty.source, ValueSource::Pack);
        assert_eq!(difficulty.value, &json!("harsh"));
        // A pack that fixed one setting has not fixed the rest.
        assert!(
            fixed
                .iter()
                .filter(|resolved| resolved.setting.key != key("world.difficulty"))
                .all(|resolved| resolved.editable)
        );
    }

    #[test]
    fn an_edit_the_pack_fixed_is_refused_and_leaves_the_value_alone() {
        let pack = document(&[
            ("world.difficulty", json!("harsh")),
            ("server.motd", json!("the pack's server")),
        ]);
        let mut config = resolved(Some(&pack), None);

        let refused = config
            .set(
                &key("world.difficulty"),
                json!("calm"),
                Side::IntegratedServer,
            )
            .expect_err("the pack has the last word");
        assert_eq!(
            refused,
            EditError::FixedByPack {
                key: key("world.difficulty"),
            }
        );
        assert_eq!(refused.key(), &key("world.difficulty"));
        assert_eq!(
            config.value(&key("world.difficulty")),
            Some(&json!("harsh"))
        );
        assert!(config.player_document().values.is_empty());
    }

    #[test]
    fn an_edit_from_the_wrong_side_or_of_an_unknown_key_is_refused() {
        let mut config = resolved(None, None);

        assert_eq!(
            config
                .set(&key("server.motd"), json!("hello"), Side::Client)
                .expect_err("no dedicated-server setting has a client screen"),
            EditError::OtherSide {
                key: key("server.motd"),
                declared: SettingSide::DedicatedServer,
                side: Side::Client,
            }
        );
        assert_eq!(
            config
                .set(&key("render.absent"), json!(1), Side::Client)
                .expect_err("undeclared"),
            EditError::Unknown {
                key: key("render.absent"),
            }
        );
        assert!(config.player_document().values.is_empty());
    }

    #[test]
    fn an_illegal_edit_is_refused_and_the_previous_value_stays_in_place() {
        let mut config = resolved(None, Some(&document(&[("render.distance", json!(8))])));

        let refused = config
            .set(&key("render.distance"), json!(64), Side::Client)
            .expect_err("above the declared maximum");
        assert_eq!(
            refused,
            EditError::Illegal {
                key: key("render.distance"),
                reason: ValueError::Above { maximum: 32.0 },
            }
        );
        assert!(refused.to_string().contains("left unchanged"));
        assert_eq!(config.value(&key("render.distance")), Some(&json!(8)));
    }

    #[test]
    fn an_ordinary_edit_takes_effect_at_once() {
        let mut config = resolved(None, None);

        assert_eq!(
            config.set(&key("render.distance"), json!(24), Side::Client),
            Ok(EditOutcome::Now)
        );
        assert_eq!(config.value(&key("render.distance")), Some(&json!(24)));
        assert_eq!(
            config.source(&key("render.distance")),
            Some(ValueSource::Player)
        );
        assert!(!config.restart_pending());
    }

    #[test]
    fn a_restart_required_edit_is_written_now_and_takes_effect_on_the_next_launch() {
        let mut config = resolved(None, None);

        assert_eq!(
            config.set(&key("engine.threads"), json!(8), Side::Client),
            Ok(EditOutcome::NextLaunch)
        );
        assert_eq!(config.value(&key("engine.threads")), Some(&json!(4)));
        assert_eq!(
            config.player_document().values.get(&key("engine.threads")),
            Some(&json!(8))
        );
        assert!(config.restart_pending());

        let relaunched = resolved(None, Some(&config.player_document()));
        assert_eq!(relaunched.value(&key("engine.threads")), Some(&json!(8)));
        assert!(!relaunched.restart_pending());
    }

    #[test]
    fn a_configuration_round_trips_through_the_file_it_writes() {
        let pack = minimal_pack();
        let mut config = resolved(Some(&pack), None);
        config
            .set(&key("render.distance"), json!(24), Side::Client)
            .expect("a legal edit");
        config
            .set(&key("engine.threads"), json!(8), Side::Client)
            .expect("a legal edit needing a restart");

        // Out to a document, out to JSON, and back in through both.
        let written = config.player_document().to_json();
        let reread = ValueDocument::parse(&written).expect("the written file re-parses");
        let relaunched = LayeredConfiguration::resolve(declaration(), Some(&pack), Some(&reread))
            .expect("legal layers");

        assert_eq!(relaunched.value(&key("render.distance")), Some(&json!(24)));
        assert_eq!(relaunched.value(&key("engine.threads")), Some(&json!(8)));
        assert_eq!(
            relaunched.value(&key("server.motd")),
            Some(&json!("the pack's server"))
        );
        assert_eq!(relaunched.rejected(), []);
        assert_eq!(relaunched.player_document().to_json(), written);
    }

    #[test]
    fn a_layer_refusal_reports_the_phase_it_failed_in_and_no_dependency_outcome() {
        for error in [
            LayerError::PackMissing {
                key: key("server.motd"),
            },
            LayerError::PackUnknown {
                key: key("render.absent"),
            },
            LayerError::PackValue {
                key: key("render.distance"),
                reason: ValueError::Above { maximum: 32.0 },
            },
        ] {
            assert_eq!(error.failure_kind(), None);
            assert_eq!(error.lifecycle_state(), LifecycleState::Resolved);
            assert!(!error.quotes_untrusted_text());
            assert_ne!(error.diagnostic().text(), "");
        }
    }
}
