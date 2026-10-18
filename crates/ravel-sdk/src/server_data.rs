//! Server-data files declared through [`crate::content::ContentDeclaration`].
//!
//! The declaration derives canonical paths for recipes, loot tables, advancements,
//! and functions. Return it from [`crate::Mod::content_declaration`] so the loader
//! includes those bytes in the world's data pack alongside other supplied resources.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::content::{ContentDeclaration, ContentDeclarationError};
//! use ravel_sdk::server_data::ServerDataKind;
//!
//! let mut declared = ContentDeclaration::new("example".parse().expect("a legal mod id"));
//! let plate = declared.server_entry(
//!     ServerDataKind::Crafting,
//!     "plate_from_ingot",
//!     br#"{"result": "example:plate"}"#,
//! )?;
//! assert_eq!(plate.as_str(), "data/example/recipe/plate_from_ingot.json");
//! let generated = declared.generate();
//! assert!(generated.file(&plate).is_some());
//! assert!(declared.server_entry(ServerDataKind::Crafting, "plate_from_ingot", b"{}").is_err());
//! # Ok::<(), ContentDeclarationError>(())
//! ```

pub use ravel_data::ServerDataKind;
