//! The settings a mod declares, and the values in force for one launch.
//!
//! Configuration is declared in data and every screen Ravel owns is drawn in
//! Java. Both halves matter to an author: what you write
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
//! A resolved value is a [`crate::JsonValue`], because a setting's shape belongs
//! to the mod that declared it. Reading one is a method call on the value, so the
//! example below names no type at all; a mod that wants to write the type down —
//! in a signature, say — reaches it through the facade rather than through
//! `serde_json`, which a mod's manifest does not list.
//!
//! # What a setting's value may be
//!
//! `schemas/configuration.schema.json` types `valueSchema` as a bare object, so
//! the schema alone does not say. [`ValueSchema`] does, and it is a closed set:
//!
//! - `"type"` is one of [`ValueKind`]'s four names — `"boolean"`, `"integer"`,
//!   `"number"`, `"string"` — or absent, which constrains the type not at all.
//!   **There is no array, object or null setting.** A mod that wants a list makes
//!   it a string it parses itself, or several settings.
//! - `"enum"` is the exact values permitted, when the setting is a closed choice.
//! - `"minimum"` and `"maximum"` bound a number, inclusive.
//! - `"minLength"` and `"maxLength"` bound a string's length.
//!
//! A keyword that does not apply to the value's type constrains nothing, so a
//! `"minimum"` beside a string says nothing about that string; `"type"` is what
//! refuses the string in the first place.
//!
//! `"integer"` here means a JSON integer literal, which is narrower than JSON
//! Schema's rule: `2` is an integer and `2.0` is not. Every document Ravel writes
//! spells an integer as one, and the narrow reading can be widened later without
//! invalidating anything already accepted.
//!
//! `"minimum"` and `"maximum"` are read as floating point whichever way they are
//! written, so `"minimum": 1` and `"minimum": 1.0` are the same bound and both
//! are accepted. The examples here write `1.0` only to make it obvious that the
//! bound is not itself typed by the setting's `"type"`.
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
//!         // `configuration_declaration` can only answer `Some` or `None`, so a
//!         // document that does not parse becomes `None` — which reads as "this
//!         // mod declares no settings". `configure` is then never called and the
//!         // mod runs on its own defaults with nothing said. Refusing here makes
//!         // that loud, and does so whichever order the host settles settings and
//!         // drives the lifecycle in.
//!         ConfigurationDeclaration::parse(SETTINGS).map_err(|error| ModError::Refused {
//!             mod_id: Self::declared_id().clone(),
//!             state,
//!             detail: error.to_string(),
//!         })?;
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
//!             .and_then(|value| value.as_u64())
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
//! example.enter(LifecycleState::Loaded)?;
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
