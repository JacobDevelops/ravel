//! The settings a mod declares, and the values in force for one launch.
//!
//! ADG-6 decided that configuration is declared in data and that every screen
//! Ravel owns is drawn in Java. Both halves matter to an author: what you write
//! is a document, and you never write a widget. A [`ConfigurationDeclaration`]
//! names each [`Setting`] — its key, what a legal value looks like, the author's
//! `default`, which [`SettingSide`] it applies on, whether a change waits for the
//! next launch, and how much say a pack has through [`PackOverride`].
//!
//! Three layers resolve into one answer, in this order: the author's `default`,
//! then a pack's override document, then the player's file.
//! [`LayeredConfiguration`] is the result, and it is what
//! [`crate::Mod::configure`] is handed. A mod reads values off it and never
//! reads a file: a player's document that states an unknown key or an illegal
//! value has already been dropped into [`LayeredConfiguration::rejected`], so what
//! a mod receives has been validated against the declaration the mod itself
//! wrote.
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
//!     fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
//!         let _ = state;
//!         Ok(())
//!     }
//!
//!     fn configuration_declaration(&self) -> Option<ConfigurationDeclaration> {
//!         ConfigurationDeclaration::parse(SETTINGS).ok()
//!     }
//!
//!     fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
//!         let key = "lanterns".parse().expect("the key this mod declared");
//!         self.lanterns = resolved
//!             .value(&key)
//!             .and_then(serde_json::Value::as_u64)
//!             .ok_or_else(|| ModError::Refused {
//!                 mod_id: Self::declared_id().clone(),
//!                 state: LifecycleState::Constructed,
//!                 detail: "`lanterns` resolved to something that is not a count".to_owned(),
//!             })?;
//!         Ok(())
//!     }
//! }
//!
//! // With no pack and no player file, every setting is the author's default.
//! let mut example = Example::default();
//! let declaration = example
//!     .configuration_declaration()
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
