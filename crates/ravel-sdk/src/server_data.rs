//! Server-resolved entries a mod owns, declared beside the rest of its content.
//!
//! A mod's [`crate::content::ContentDeclaration`] holds its registry keys, its
//! tag memberships, its translations and any bytes Ravel does not generate for
//! it. A conversion between items, a table of what something drops, a step in a
//! player's progression and a named sequence of commands are none of those: no
//! mod claims them at the barrier, no mod merges into them, and the server reads
//! them when a world loads. Before this module the only way to ship one was as
//! supplied bytes at a path the author had to spell, which is the escape hatch
//! [`crate::content::ContentDeclaration::resource`] documents as a last resort.
//!
//! What this adds is the path and the barrier. The path is derived from the
//! kind and the name, so a mod cannot misfile its own data; the barrier is the
//! one table every entry goes through, so two of a mod's declarations that would
//! write one file are refused when the second is made.
//!
//! What it deliberately does not add is the *shape* of an entry. What a table
//! means is a Minecraft version's decision, and modelling it here would put a
//! version's shape into the version-neutral surface `docs/architecture/dependency-rules.md`
//! keeps clear of one. The bytes are the author's, carried unread.
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
