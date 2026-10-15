//! Content declarations and registration-window claims.
//!
//! Declare entries once in [`ContentDeclaration`], then claim them from
//! [`crate::Mod::register`]. Claims after the barrier closes are refused.
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

use ravel_core::LifecycleState;

pub use ravel_core::{
    ContentId, ContentIdError, ContentKind, ContentKindError, RegistryKey, RegistryKeyError,
};
pub use ravel_data::{ContentDeclaration, DeclarationError};
pub use ravel_host_api::{Registrar, RegistrationRefused};

use crate::ModError;

/// Claims every entry a declaration holds, in declaration order.
///
/// Stops at the first refusal; use [`Registrar::claim`] directly to handle
/// individual conflicts.
///
/// # Errors
///
/// Returns [`ModError::Refused`] for the owner in [`LifecycleState::Constructed`].
///
/// # Examples
///
/// ```
/// use ravel_sdk::content::{
///     ContentDeclaration, ContentKind, Registrar, RegistrationRefused, RegistryKey, claim_all,
/// };
/// use ravel_sdk::{Declared, LifecycleState, Mod, ModError, ModId, declare_mod};
///
/// struct Example;
///
/// #[declare_mod(id = "example")]
/// impl Mod for Example {
///     fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
///         let _ = state;
///         Ok(())
///     }
/// }
///
/// #[derive(Default)]
/// struct Accepting {
///     claimed: Vec<RegistryKey>,
/// }
///
/// impl Registrar for Accepting {
///     fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
///         self.claimed.push(key);
///         Ok(())
///     }
///
///     fn claimed(&self) -> &[RegistryKey] {
///         &self.claimed
///     }
///
///     fn loads(&self, mod_id: &ModId) -> bool {
///         let _ = mod_id;
///         false
///     }
/// }
///
/// let mut declaration = ContentDeclaration::new(Example::declared_id().clone());
/// declaration.entry(ContentKind::Block, "copper").expect("a legal entry");
/// declaration.entry(ContentKind::Item, "copper").expect("a legal entry");
///
/// let mut registrar = Accepting::default();
/// claim_all(&declaration, &mut registrar)?;
/// assert_eq!(registrar.claimed(), declaration.entries());
/// # Ok::<(), ModError>(())
/// ```
pub fn claim_all(
    declaration: &ContentDeclaration,
    registrar: &mut dyn Registrar,
) -> Result<(), ModError> {
    for key in declaration.entries() {
        registrar
            .claim(key.clone())
            .map_err(|refusal| ModError::Refused {
                mod_id: declaration.owner().clone(),
                state: LifecycleState::Constructed,
                detail: refusal.to_string(),
            })?;
    }
    Ok(())
}
