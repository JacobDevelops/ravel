//! Content declarations and registration-window claims.
//!
//! Declare entries once in [`ContentDeclaration`] and return them from
//! [`crate::Mod::content_declaration`]; the default [`crate::Mod::register`] claims
//! them in order, stopping at the first refusal. Claims after the barrier closes
//! are refused.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::content::{ContentDeclaration, ContentId, ContentKind, RegistryKey};
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
//! let copper = RegistryKey::new(
//!     ContentKind::Block,
//!     ContentId::new(Example::declared_id().clone(), "copper")?,
//! );
//! assert_eq!(copper.to_string(), "block:example:copper");
//!
//! let item = RegistryKey::new(ContentKind::Item, copper.id().clone());
//! assert_ne!(copper, item);
//!
//! let mut declaration = ContentDeclaration::new(Example::declared_id().clone());
//! let declared_block = declaration
//!     .entry(ContentKind::Block, "copper")
//!     .expect("a legal, undeclared entry");
//! assert_eq!(declared_block, copper);
//! # Ok::<(), ravel_sdk::content::ContentIdError>(())
//! ```

pub use ravel_core::{
    BlockBehaviour, ContentBehaviour, ContentId, ContentIdError, ContentKind, ContentKindError,
    ItemBehaviour, RegistryKey, RegistryKeyError,
};
pub use ravel_data::{ContentDeclaration, DeclarationError};
pub use ravel_host_api::{Registrar, RegistrationRefused};

pub use ravel_host_api::claim_all;
