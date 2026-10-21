//! Declarative settings resolved from author defaults, pack overrides, and player values.
//!
//! [`crate::Mod::configure`] receives validated [`LayeredConfiguration`].
//! [`ValueSchema`] supports booleans, integer literals, numbers, and strings,
//! with enum, numeric-range, and string-length constraints.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration, ValueSource};
//! use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod};
//!
//! #[derive(Default)]
//! struct Example {
//!     lanterns: u64,
//! }
//!
//! const SETTINGS: &str = r#"{
//!   "schemaVersion": 1,
//!   "owner": "example",
//!   "version": 1,
//!   "settings": [{
//!     "key": "lanterns",
//!     "valueSchema": { "type": "integer", "minimum": 0.0, "maximum": 64.0 },
//!     "default": 3,
//!     "side": "common",
//!     "restartRequired": false
//!   }]
//! }"#;
//!
//! #[declare_mod(id = "example")]
//! impl Mod for Example {
//!     fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
//!         Ok(())
//!     }
//!
//!     fn configuration_declaration(&self) -> Result<Option<ConfigurationDeclaration>, ModError> {
//!         ConfigurationDeclaration::parse(SETTINGS)
//!             .map(Some)
//!             .map_err(ModError::from_display)
//!     }
//!
//!     fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
//!         self.lanterns = resolved.integer_in::<u64>("lanterns")
//!             .map_err(ModError::from_display)?;
//!         Ok(())
//!     }
//! }
//!
//! // With no pack and no player file, every setting is the author's default.
//! let mut example = Example::default();
//! example.enter(LifecycleState::Loaded)?;
//! let declaration = example
//!     .configuration_declaration()?
//!     .expect("the mod declares settings");
//! let resolved = LayeredConfiguration::resolve(declaration, None, None)?;
//! example.configure(&resolved)?;
//!
//! assert_eq!(example.lanterns, 3);
//! assert_eq!(resolved.source("lanterns"), Ok(ValueSource::Default));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub use ravel_config::{
    AccessError, ConfigurationDeclaration, DeclarationError, DocumentError, EditError, EditOutcome,
    LayerError, LayeredConfiguration, PackOverride, Rejection, RejectionReason, ResolvedSetting,
    SchemaVersion, SchemaVersionError, Setting, SettingKey, SettingKeyError, SettingSide,
    ValueDocument, ValueError, ValueKind, ValueSchema, ValueSource,
};

/// Resolves a packaged declaration's defaults and reads the mod's typed settings.
///
/// Declaration and typed-reader failures are returned as mod errors.
/// No pack or player overrides participate in this pre-configuration value.
pub fn read_shipped<T, E: std::fmt::Display>(
    document: &str,
    read: impl FnOnce(&LayeredConfiguration) -> Result<T, E>,
) -> Result<T, crate::ModError> {
    let declared =
        ConfigurationDeclaration::parse(document).map_err(crate::ModError::from_display)?;
    let resolved = LayeredConfiguration::resolve(declared, None, None)
        .map_err(crate::ModError::from_display)?;
    read(&resolved).map_err(crate::ModError::from_display)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(default: &crate::JsonValue) -> String {
        crate::json!({
            "schemaVersion": 1,
            "owner": "example",
            "version": 1,
            "settings": [{
                "key": "count",
                "valueSchema": { "type": "integer" },
                "default": default,
                "side": "common",
                "restartRequired": false
            }]
        })
        .to_string()
    }

    #[test]
    fn read_shipped_returns_representable_defaults() {
        for value in [0, u32::MAX] {
            assert_eq!(
                read_shipped(&document(&crate::json!(value)), |resolved| {
                    resolved.integer_in::<u32>("count")
                })
                .expect("a representable unsigned default"),
                value
            );
        }
    }

    #[test]
    fn shipped_reads_distinguish_declaration_and_typed_reader_refusals() {
        let malformed = read_shipped("{", |resolved| resolved.integer_in::<u32>("count"))
            .expect_err("malformed JSON is refused before reading");
        assert_eq!(
            malformed.to_string(),
            "the declaration is not JSON: EOF while parsing an object at line 1 column 1"
        );
        let invalid_default = read_shipped(&document(&crate::json!("wrong kind")), |resolved| {
            resolved.integer_in::<u32>("count")
        })
        .expect_err("a default contradicting its schema is refused before reading");
        assert_eq!(
            invalid_default.to_string(),
            "`count` declares a default its own value schema refuses: the setting holds a integer, not a string"
        );
        let missing = read_shipped(&document(&crate::json!(1)), |resolved| {
            resolved.integer_in::<u32>("missing")
        })
        .expect_err("the typed reader refuses an undeclared key");
        assert_eq!(missing.to_string(), "`missing` is not a declared setting");
    }
}
