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
//!             .map_err(|error| ModError::refused(error.to_string()))
//!     }
//!
//!     fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
//!         let key = "lanterns".parse().expect("the key this mod declared");
//!         self.lanterns = u64::try_from(
//!             resolved.integer(&key).map_err(|error| ModError::refused(error.to_string()))?,
//!         )
//!         .map_err(|error| ModError::refused(error.to_string()))?;
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
//! let key = "lanterns".parse()?;
//! assert_eq!(resolved.source(&key), Some(ValueSource::Default));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub use ravel_config::{
    ConfigurationDeclaration, DeclarationError, DocumentError, EditError, EditOutcome, LayerError,
    LayeredConfiguration, PackOverride, Rejection, RejectionReason, ResolvedSetting, SchemaVersion,
    SchemaVersionError, Setting, SettingKey, SettingKeyError, SettingSide, ValueDocument,
    ValueError, ValueKind, ValueSchema, ValueSource,
};
