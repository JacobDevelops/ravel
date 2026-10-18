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
//!         self.lanterns = u64::try_from(
//!             resolved.integer("lanterns").map_err(ModError::from_display)?,
//!         )
//!         .map_err(ModError::from_display)?;
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
    ConfigurationDeclaration, DeclarationError, DocumentError, EditError, EditOutcome, LayerError,
    LayeredConfiguration, PackOverride, Rejection, RejectionReason, ResolvedSetting, SchemaVersion,
    SchemaVersionError, Setting, SettingKey, SettingKeyError, SettingSide, ValueDocument,
    ValueError, ValueKind, ValueSchema, ValueSource,
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

/// Reads an integer setting as `u32`, preserving access and range refusals.
pub fn unsigned(resolved: &LayeredConfiguration, key: &str) -> Result<u32, ValueError> {
    let value = resolved.integer(key)?;
    u32::try_from(value).map_err(|_| {
        if value < 0 {
            ValueError::Below { minimum: 0.0 }
        } else {
            ValueError::Above {
                maximum: f64::from(u32::MAX),
            }
        }
    })
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
    fn shipped_unsigned_reads_preserve_bounds() {
        for value in [0, u32::MAX] {
            assert_eq!(
                read_shipped(&document(&crate::json!(value)), |resolved| {
                    unsigned(resolved, "count")
                })
                .expect("a representable unsigned default"),
                value
            );
        }
        for (value, expected) in [
            (-1_i64, ValueError::Below { minimum: 0.0 }),
            (
                i64::from(u32::MAX) + 1,
                ValueError::Above {
                    maximum: f64::from(u32::MAX),
                },
            ),
        ] {
            let declaration = ConfigurationDeclaration::parse(&document(&crate::json!(value)))
                .expect("a legal declaration");
            let resolved = LayeredConfiguration::resolve(declaration, None, None)
                .expect("unbounded integers resolve");
            assert_eq!(unsigned(&resolved, "count"), Err(expected));
        }
    }

    #[test]
    fn unsigned_reads_preserve_unknown_key_and_wrong_accessor_errors() {
        let document =
            document(&crate::json!(true)).replace("\"type\":\"integer\"", "\"type\":\"boolean\"");
        let declared = ConfigurationDeclaration::parse(&document).expect("a boolean declaration");
        let resolved =
            LayeredConfiguration::resolve(declared, None, None).expect("valid defaults resolve");
        assert_eq!(
            unsigned(&resolved, "missing"),
            Err(ValueError::Unknown {
                key: "missing".to_owned(),
            })
        );
        assert_eq!(
            unsigned(&resolved, "count"),
            Err(ValueError::AccessKind {
                expected: ValueKind::Integer,
                declared: Some(ValueKind::Boolean),
            })
        );
    }

    #[test]
    fn shipped_reads_distinguish_declaration_and_typed_reader_refusals() {
        let malformed = read_shipped("{", |resolved| unsigned(resolved, "count"))
            .expect_err("malformed JSON is refused before reading");
        assert_eq!(
            malformed.to_string(),
            "the declaration is not JSON: EOF while parsing an object at line 1 column 1"
        );
        let invalid_default = read_shipped(&document(&crate::json!("wrong kind")), |resolved| {
            unsigned(resolved, "count")
        })
        .expect_err("a default contradicting its schema is refused before reading");
        assert_eq!(
            invalid_default.to_string(),
            "`count` declares a default its own value schema refuses: the setting holds a integer, not a string"
        );
        let missing = read_shipped(&document(&crate::json!(1)), |resolved| {
            unsigned(resolved, "missing")
        })
        .expect_err("the typed reader refuses an undeclared key");
        assert_eq!(missing.to_string(), "`missing` is not a declared setting");
    }
}
