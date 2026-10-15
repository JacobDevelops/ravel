//! Tag memberships shared by multiple mods.
//!
//! Declare memberships in [`crate::content::ContentDeclaration`]; [`TagSet`]
//! merges contributions without exclusive ownership or registry claims.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::content::{ContentDeclaration, ContentId, ContentKind};
//! use ravel_sdk::tags::{TagId, TagSet};
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
//! // The convention namespace `c` is the one mods share on purpose, so a tag
//! // named there is not content the declaring mod owns.
//! let ingots = TagId::new(
//!     ContentKind::Item,
//!     ContentId::new("c".parse()?, "ingots").expect("a legal content path"),
//! )
//! .expect("items are taggable");
//!
//! let mut declaration = ContentDeclaration::new(Example::declared_id().clone());
//! let copper = declaration
//!     .entry(ContentKind::Item, "copper_ingot")
//!     .expect("a legal, undeclared entry");
//! declaration
//!     .tag(&ingots, &copper)
//!     .expect("an item joins an item tag");
//!
//! // A tag holds only entries of the registry it groups.
//! let block = declaration
//!     .entry(ContentKind::Block, "copper_block")
//!     .expect("a legal, undeclared entry");
//! assert!(declaration.tag(&ingots, &block).is_err());
//!
//! // Alone, a mod is the only contributor; with others, the members merge.
//! let set = TagSet::merged([&declaration]);
//! assert_eq!(set.members(&ingots), [copper]);
//! assert_eq!(set.contributors(&ingots), [Example::declared_id().clone()]);
//! # Ok::<(), ravel_sdk::ModIdError>(())
//! ```

pub use ravel_data::{TagError, TagId, TagSet};
