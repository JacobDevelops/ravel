//! Server-data declarations with derived paths and duplicate-file detection.
//!
//! Supply version-specific entry bodies as bytes. Ravel derives paths from
//! their kind and name without interpreting the bodies.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::server_data::{ServerDataDeclaration, ServerDataKind};
//! use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod};
//!
//! struct Example;
//!
//! #[declare_mod(id = "example")]
//! impl Mod for Example {
//!     fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
//!         let _ = state;
//!         Ok(())
//!     }
//! }
//!
//! let mut declared = ServerDataDeclaration::new(Example::declared_id().clone());
//!
//! // The path is the declaration's answer, not the author's guess.
//! let plate = declared.crafting("plate_from_ingot", br#"{"result": "example:plate"}"#)?;
//! assert_eq!(plate.path().as_str(), "data/example/recipe/plate_from_ingot.json");
//!
//! // One name under another kind is another entry.
//! declared.entry(ServerDataKind::Loot, "plate_from_ingot", b"{}")?;
//! // The same name under one kind is the second claim the barrier refuses.
//! assert!(declared.crafting("plate_from_ingot", b"{}").is_err());
//!
//! // Generating twice produces the same bytes, not an equal-looking value.
//! let generated = declared.generate();
//! assert_eq!(generated.files().count(), 2);
//! assert_eq!(declared.generate(), generated);
//! # Ok::<(), ravel_sdk::server_data::ServerDataError>(())
//! ```

pub use ravel_data::{
    GeneratedServerData, ServerDataDeclaration, ServerDataEntry, ServerDataError, ServerDataKind,
};
