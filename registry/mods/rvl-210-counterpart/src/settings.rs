//! Section three's two settings and section 5.2's declaration framework.
//!
//! The entry's own configuration surface is small and the framework it publishes
//! is one of its largest products, which is exactly the shape section three
//! describes. Both of its own settings are client-scoped, sit under one grouping,
//! are meaningful only in a development environment and change nothing in a
//! shipped game.
//!
//! What section 5.2 asks a counterpart to preserve is that a setting's scope
//! decides which file it lands in and therefore who may change it, not merely
//! where it is written. [`Scope`] is that vocabulary and [`Declared::document`]
//! is the consequence.

use ravel_sdk::Side;
use ravel_sdk::config::{
    AccessError, ConfigurationDeclaration, DeclarationError, LayeredConfiguration,
};

use core::fmt;

/// The key the scratch-world shortcut is switched under.
pub const DEVELOPMENT_SHORTCUT: &str = "development.scratch_world_shortcut";

/// The key startup music is silenced under.
pub const DEVELOPMENT_SILENCE_MUSIC: &str = "development.silence_music";

/// The one grouping both of this entry's own settings sit under.
pub const OWN_GROUPING: &str = "development";

/// Both of this entry's own settings, in declaration order.
pub const OWN_KEYS: [&str; 2] = [DEVELOPMENT_SHORTCUT, DEVELOPMENT_SILENCE_MUSIC];

/// The packaged settings document.
///
/// Both settings are client-scoped, because both are about what a developer's
/// own client does at startup and neither is a server's to decide.
pub const DOCUMENT: &str = r#"{
  "schemaVersion": 1,
  "owner": "rvl_210_counterpart",
  "version": 1,
  "settings": [
    {
      "key": "development.scratch_world_shortcut",
      "title": "Add a scratch-world shortcut to the main menu",
      "description": "Adds a main-menu shortcut that opens a scratch world. Meaningful only inside a development environment and changes nothing in a shipped game.",
      "valueSchema": { "type": "boolean" },
      "default": false,
      "side": "client",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "development.silence_music",
      "title": "Silence music at startup",
      "description": "Silences music when the game starts. Meaningful only inside a development environment and changes nothing in a shipped game.",
      "valueSchema": { "type": "boolean" },
      "default": false,
      "side": "client",
      "restartRequired": false,
      "packOverride": "allowed"
    }
  ]
}"#;

/// Reads the packaged declaration.
///
/// # Errors
///
/// Returns whatever [`ConfigurationDeclaration::parse`] refuses, which for a
/// compile-time constant is a packaging failure rather than a runtime one.
pub fn declaration() -> Result<ConfigurationDeclaration, DeclarationError> {
    ConfigurationDeclaration::parse(DOCUMENT)
}

/// The two values in force for one launch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Settings {
    /// Whether a main-menu shortcut opens a scratch world.
    pub scratch_world_shortcut: bool,
    /// Whether music is silenced at startup.
    pub silence_music: bool,
}

impl Settings {
    /// Resolves the packaged defaults without configuration layers.
    ///
    /// # Errors
    /// Returns a packaging failure if the declaration or defaults are invalid.
    pub fn shipped() -> Result<Self, ravel_sdk::ModError> {
        ravel_sdk::config::read_shipped(DOCUMENT, Self::read)
    }

    /// Reads the values in force, refusing a missing key or a value of the wrong kind.
    pub fn read(resolved: &LayeredConfiguration) -> Result<Self, AccessError> {
        Ok(Self {
            scratch_world_shortcut: resolved.boolean(DEVELOPMENT_SHORTCUT)?,
            silence_music: resolved.boolean(DEVELOPMENT_SILENCE_MUSIC)?,
        })
    }
}

/// The scope vocabulary of section 5.2.
///
/// Three values, and the scope is what decides which file a setting lands in and
/// therefore who may change it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Scope {
    /// Resolved by a client and changeable by whoever runs it.
    Client,
    /// Resolved by a server and changeable by whoever runs it.
    Server,
    /// Resolved on every side.
    Common,
}

impl Scope {
    /// The document a setting of this scope is written into.
    #[must_use]
    pub const fn document(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Server => "server",
            Self::Common => "common",
        }
    }

    /// Whether a side resolves a setting of this scope.
    #[must_use]
    pub const fn resolved_by(self, side: Side) -> bool {
        match self {
            Self::Client => matches!(side, Side::Client),
            Self::Server => side.is_server(),
            Self::Common => true,
        }
    }
}

/// One setting a dependent declared through the framework.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declared {
    property: String,
    category: String,
    comment: String,
    scope: Scope,
    changeable: bool,
    value: Value,
}

/// The closed vocabulary of setting value kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// A truth value.
    Truth,
    /// A whole number.
    Number,
    /// Text.
    Text,
}

impl Kind {
    /// The canonical spelling used in diagnostics and generated documents.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Truth => "truth",
            Self::Number => "number",
            Self::Text => "text",
        }
    }
}

/// A declared setting's value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    /// A truth value.
    Truth(bool),
    /// A whole number.
    Number(i64),
    /// Text.
    Text(String),
}

impl Value {
    /// Which kind of value this is, so a write of another kind can be refused.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        match self {
            Self::Truth(_) => Kind::Truth,
            Self::Number(_) => Kind::Number,
            Self::Text(_) => Kind::Text,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truth(truth) => write!(formatter, "{truth}"),
            Self::Number(number) => write!(formatter, "{number}"),
            Self::Text(text) => formatter.write_str(text),
        }
    }
}

impl Declared {
    /// Declares one setting: its property name, its category, its comment, the
    /// document it belongs in, and whether it can be changed at runtime.
    #[must_use]
    pub fn new(
        property: &str,
        category: &str,
        comment: &str,
        scope: Scope,
        changeable: bool,
        value: Value,
    ) -> Self {
        Self {
            property: property.to_owned(),
            category: category.to_owned(),
            comment: comment.to_owned(),
            scope,
            changeable,
            value,
        }
    }

    /// The name the command tree addresses it by.
    #[must_use]
    pub fn property(&self) -> &str {
        &self.property
    }

    /// Its category.
    #[must_use]
    pub fn category(&self) -> &str {
        &self.category
    }

    /// Its comment.
    #[must_use]
    pub fn comment(&self) -> &str {
        &self.comment
    }

    /// Its scope.
    #[must_use]
    pub const fn scope(&self) -> Scope {
        self.scope
    }

    /// The document it lands in, which follows from its scope alone.
    #[must_use]
    pub const fn document(&self) -> &'static str {
        self.scope.document()
    }

    /// Whether the command tree may change it while the game runs.
    #[must_use]
    pub const fn changeable(&self) -> bool {
        self.changeable
    }

    /// The value in force.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

/// What the framework refuses when a setting is read or written at runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChangeRefused {
    /// Nothing is declared under that property name.
    NoSuchProperty {
        /// The name asked for.
        property: String,
    },
    /// The caller's side does not resolve the setting's scope.
    WrongSide {
        /// The property asked for.
        property: String,
        /// The declared scope.
        scope: Scope,
        /// The side attempting the write.
        side: Side,
    },
    /// The setting is not marked changeable at runtime.
    NotChangeable {
        /// The name asked for.
        property: String,
    },
    /// The value written is not of the declared kind.
    WrongKind {
        /// What was declared.
        declared: Kind,
        /// What was written.
        written: Kind,
    },
}

impl fmt::Display for ChangeRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSuchProperty { property } => {
                write!(formatter, "no setting is declared as {property}")
            }
            Self::WrongSide {
                property,
                scope,
                side,
            } => {
                write!(
                    formatter,
                    "{property} has scope {}, not writable on {}",
                    scope.document(),
                    side.as_str(),
                )
            }
            Self::NotChangeable { property } => {
                write!(
                    formatter,
                    "{property} is not changeable while the game runs"
                )
            }
            Self::WrongKind { declared, written } => {
                write!(
                    formatter,
                    "{} was written as {}",
                    declared.as_str(),
                    written.as_str()
                )
            }
        }
    }
}

impl std::error::Error for ChangeRefused {}

/// The settings a dependent declared, collected per document.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Framework {
    declared: Vec<Declared>,
    resolved: bool,
}

impl Framework {
    /// A framework with nothing declared.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            declared: Vec::new(),
            resolved: false,
        }
    }

    /// Declares one setting, keeping declaration order.
    pub fn declare(&mut self, declared: Declared) {
        self.declared.push(declared);
    }

    /// Everything declared, in declaration order.
    #[must_use]
    pub fn declared(&self) -> &[Declared] {
        &self.declared
    }

    /// The settings collected into one document, in declaration order.
    #[must_use]
    pub fn document(&self, scope: Scope) -> Vec<&Declared> {
        self.declared
            .iter()
            .filter(|declared| declared.scope == scope)
            .collect()
    }

    /// Marks the framework as having resolved, which happens before a
    /// dependent's own setup runs.
    pub const fn mark_resolved(&mut self) {
        self.resolved = true;
    }

    /// Whether resolution has happened yet.
    #[must_use]
    pub const fn resolved(&self) -> bool {
        self.resolved
    }

    /// Reads one setting through the command tree.
    ///
    /// # Errors
    ///
    /// Returns [`ChangeRefused::NoSuchProperty`] when nothing carries the name.
    pub fn read(&self, property: &str) -> Result<&Value, ChangeRefused> {
        self.declared
            .iter()
            .find(|declared| declared.property == property)
            .map(Declared::value)
            .ok_or_else(|| ChangeRefused::NoSuchProperty {
                property: property.to_owned(),
            })
    }

    /// Writes one setting through the command tree.
    ///
    /// The write takes effect immediately against the in-memory value. It is not
    /// written back to the file: the specification records that path as unread,
    /// and a counterpart that guessed would be writing a file on a guess.
    ///
    /// # Errors
    ///
    /// Returns [`ChangeRefused`] for an unknown property, a side outside its scope,
    /// a setting not changeable at runtime, or a value of another kind.
    pub fn write(&mut self, property: &str, value: Value, side: Side) -> Result<(), ChangeRefused> {
        let Some(declared) = self
            .declared
            .iter_mut()
            .find(|declared| declared.property == property)
        else {
            return Err(ChangeRefused::NoSuchProperty {
                property: property.to_owned(),
            });
        };
        if !declared.scope.resolved_by(side) {
            return Err(ChangeRefused::WrongSide {
                property: property.to_owned(),
                scope: declared.scope,
                side,
            });
        }
        if !declared.changeable {
            return Err(ChangeRefused::NotChangeable {
                property: property.to_owned(),
            });
        }
        if declared.value.kind() != value.kind() {
            return Err(ChangeRefused::WrongKind {
                declared: declared.value.kind(),
                written: value.kind(),
            });
        }
        declared.value = value;
        Ok(())
    }

    /// The property names the settings-property argument kind offers.
    #[must_use]
    pub fn properties(&self) -> Vec<&str> {
        self.declared.iter().map(Declared::property).collect()
    }
}

/// This entry's own two settings as the framework sees them.
///
/// Both are marked changeable, which section three says is a property of the
/// framework rather than of these two settings.
#[must_use]
pub fn own_framework() -> Framework {
    let mut framework = Framework::new();
    framework.declare(Declared::new(
        DEVELOPMENT_SHORTCUT,
        OWN_GROUPING,
        "Adds a main-menu shortcut that opens a scratch world.",
        Scope::Client,
        true,
        Value::Truth(false),
    ));
    framework.declare(Declared::new(
        DEVELOPMENT_SILENCE_MUSIC,
        OWN_GROUPING,
        "Silences music when the game starts.",
        Scope::Client,
        true,
        Value::Truth(false),
    ));
    framework
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravel_sdk::Side;
    use ravel_sdk::config::ValueError;

    #[test]
    fn the_packaged_defaults_resolve_to_the_launch_settings() {
        let resolved = LayeredConfiguration::resolve(
            declaration().expect("the packaged declaration is valid"),
            None,
            None,
        )
        .expect("the packaged defaults resolve");
        assert_eq!(
            Settings::read(&resolved).expect("defaults read"),
            Settings::shipped().expect("defaults resolve")
        );
    }

    #[test]
    fn a_missing_declared_key_is_returned_instead_of_defaulted() {
        for name in OWN_KEYS {
            let changed = DOCUMENT.replace(name, "development.renamed");
            let declared = ConfigurationDeclaration::parse(&changed).expect("a legal declaration");
            let resolved = LayeredConfiguration::resolve(declared, None, None)
                .expect("the changed declaration resolves");
            assert_eq!(
                Settings::read(&resolved),
                Err(AccessError {
                    key: name.to_owned(),
                    reason: ValueError::Unknown {
                        key: name.to_owned()
                    },
                })
            );
        }
    }

    #[test]
    fn a_wrong_declared_kind_is_returned_instead_of_defaulted() {
        let changed = DOCUMENT
            .replace(r#""type": "boolean""#, r#""type": "integer""#)
            .replace(r#""default": false"#, r#""default": 0"#);
        let declared = ConfigurationDeclaration::parse(&changed).expect("a legal declaration");
        let resolved = LayeredConfiguration::resolve(declared, None, None)
            .expect("the changed declaration resolves");
        assert!(matches!(
            Settings::read(&resolved),
            Err(AccessError {
                reason: ValueError::AccessKind { .. },
                ..
            })
        ));
    }

    #[test]
    fn runtime_writes_enforce_scope_for_every_side_and_preserve_refused_values() {
        for scope in [Scope::Client, Scope::Server, Scope::Common] {
            for side in [Side::Client, Side::IntegratedServer, Side::DedicatedServer] {
                let mut framework = Framework::new();
                framework.declare(Declared::new(
                    "scoped.one",
                    "scoped",
                    "scope guard",
                    scope,
                    true,
                    Value::Number(3),
                ));
                let result = framework.write("scoped.one", Value::Number(9), side);
                if scope.resolved_by(side) {
                    assert_eq!(result, Ok(()));
                    assert_eq!(framework.read("scoped.one"), Ok(&Value::Number(9)));
                } else {
                    assert_eq!(
                        result,
                        Err(ChangeRefused::WrongSide {
                            property: "scoped.one".to_owned(),
                            scope,
                            side,
                        })
                    );
                    assert_eq!(framework.read("scoped.one"), Ok(&Value::Number(3)));
                }
            }
        }
    }

    #[test]
    fn the_packaged_document_parses_and_declares_both_settings() {
        let declaration = declaration().expect("the packaged document is well formed");
        for key in OWN_KEYS {
            assert!(
                declaration.setting(key).is_some(),
                "the document declares it"
            );
        }
    }

    #[test]
    fn both_of_this_entry_s_own_settings_are_client_scoped_and_under_one_grouping() {
        let framework = own_framework();
        assert_eq!(framework.declared().len(), 2);
        for declared in framework.declared() {
            assert_eq!(declared.scope(), Scope::Client);
            assert_eq!(declared.document(), "client");
            assert_eq!(declared.category(), OWN_GROUPING);
            assert!(declared.changeable());
        }
        assert_eq!(framework.document(Scope::Client).len(), 2);
        assert_eq!(framework.document(Scope::Server).len(), 0);
        assert_eq!(framework.document(Scope::Common).len(), 0);
    }

    #[test]
    fn both_shipped_defaults_are_off_so_a_shipped_game_sees_nothing() {
        let settings = Settings::shipped().expect("the packaged defaults resolve");
        assert!(!settings.scratch_world_shortcut);
        assert!(!settings.silence_music);
    }

    #[test]
    fn a_scope_decides_which_side_resolves_the_setting_at_all() {
        assert!(Scope::Client.resolved_by(Side::Client));
        assert!(!Scope::Client.resolved_by(Side::DedicatedServer));
        assert!(!Scope::Server.resolved_by(Side::Client));
        assert!(Scope::Server.resolved_by(Side::DedicatedServer));
        assert!(Scope::Server.resolved_by(Side::IntegratedServer));
        for side in [Side::Client, Side::IntegratedServer, Side::DedicatedServer] {
            assert!(Scope::Common.resolved_by(side));
        }
    }

    #[test]
    fn a_runtime_write_takes_effect_immediately_against_the_in_memory_value() {
        let mut framework = own_framework();
        assert_eq!(
            framework.read(DEVELOPMENT_SHORTCUT),
            Ok(&Value::Truth(false))
        );
        framework
            .write(DEVELOPMENT_SHORTCUT, Value::Truth(true), Side::Client)
            .expect("the setting is changeable");
        assert_eq!(
            framework.read(DEVELOPMENT_SHORTCUT),
            Ok(&Value::Truth(true))
        );
    }

    #[test]
    fn a_setting_that_is_not_changeable_refuses_a_runtime_write_and_keeps_its_value() {
        let mut framework = Framework::new();
        framework.declare(Declared::new(
            "fixed.one",
            "fixed",
            "not changeable",
            Scope::Common,
            false,
            Value::Number(3),
        ));
        assert_eq!(
            framework.write("fixed.one", Value::Number(9), Side::Client),
            Err(ChangeRefused::NotChangeable {
                property: "fixed.one".to_owned()
            })
        );
        assert_eq!(framework.read("fixed.one"), Ok(&Value::Number(3)));
    }

    #[test]
    fn a_write_of_another_kind_is_refused_rather_than_replacing_the_declared_one() {
        let mut framework = own_framework();
        assert_eq!(
            framework.write(DEVELOPMENT_SHORTCUT, Value::Number(1), Side::Client),
            Err(ChangeRefused::WrongKind {
                declared: Kind::Truth,
                written: Kind::Number,
            })
        );
        assert_eq!(
            framework.read(DEVELOPMENT_SHORTCUT),
            Ok(&Value::Truth(false))
        );
    }

    #[test]
    fn a_property_nothing_declares_is_refused_by_both_the_read_and_the_write() {
        let mut framework = own_framework();
        assert_eq!(
            framework.read("never.declared"),
            Err(ChangeRefused::NoSuchProperty {
                property: "never.declared".to_owned()
            })
        );
        assert_eq!(
            framework.write("never.declared", Value::Truth(true), Side::Client),
            Err(ChangeRefused::NoSuchProperty {
                property: "never.declared".to_owned()
            })
        );
    }

    #[test]
    fn the_argument_kind_offers_every_declared_property_in_declaration_order() {
        let framework = own_framework();
        assert_eq!(framework.properties(), OWN_KEYS.to_vec());
    }

    #[test]
    fn resolution_happens_before_a_dependent_s_own_setup_and_is_visible() {
        let mut framework = own_framework();
        assert!(!framework.resolved());
        framework.mark_resolved();
        assert!(framework.resolved());
    }
}
