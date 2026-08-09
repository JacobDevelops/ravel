//! Grouping content several mods contribute to.
//!
//! A tag is the one thing a mod declares that has no owner. A registry key has
//! exactly one — the second claim on `example:copper` is a conflict — and a
//! resource path has exactly one supplier. `c:ingots` has neither: every mod
//! that makes an ingot puts it in, and a recipe that accepts the tag accepts
//! ingots the recipe's author never saw. Refusing the second contribution would
//! break the only thing tags are for.
//!
//! That is why nothing here goes through the registration barrier. A mod
//! declares its memberships in its [`crate::content::ContentDeclaration`], and
//! [`TagSet`] is what several declarations add up to once the loader knows the
//! resolved load order.
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
