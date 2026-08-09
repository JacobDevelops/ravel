//! Naming content, and claiming the names during the registration window.
//!
//! `SPEC.md` §14 makes an identifier immutable once the load has claimed it, and
//! the claim happens in one window: the loader calls [`crate::Mod::register`]
//! once, hands it a [`Registrar`], and closes the barrier afterwards. A mod that
//! claims nothing there claims nothing ever, and a mod that holds the registrar
//! past the call is refused with [`RegistrationRefused::Closed`] rather than
//! quietly accepted.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::content::{ContentId, ContentKind, RegistryKey};
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
//! // A key is built from the mod's own declared identity, so a mod cannot name
//! // content in a namespace it does not own without saying so.
//! let copper = RegistryKey::new(
//!     ContentKind::Block,
//!     ContentId::new(Example::declared_id().clone(), "copper")?,
//! );
//! assert_eq!(copper.to_string(), "block:example:copper");
//!
//! // The same name in two registries is two different claims.
//! let item = RegistryKey::new(ContentKind::Item, copper.id().clone());
//! assert_ne!(copper, item);
//! # Ok::<(), ravel_sdk::content::ContentIdError>(())
//! ```

pub use ravel_core::{
    ContentId, ContentIdError, ContentKind, ContentKindError, RegistryKey, RegistryKeyError,
};
pub use ravel_host_api::{Registrar, RegistrationRefused};
