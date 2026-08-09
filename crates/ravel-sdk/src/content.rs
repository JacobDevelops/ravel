//! Naming content, declaring it, and claiming the names during the registration
//! window.
//!
//! `SPEC.md` §14 makes an identifier immutable once the load has claimed it, and
//! the claim happens in one window: the loader calls [`crate::Mod::register`]
//! once, hands it a [`Registrar`], and closes the barrier afterwards. A mod that
//! claims nothing there claims nothing ever, and a mod that holds the registrar
//! past the call is refused with [`RegistrationRefused::Closed`] rather than
//! quietly accepted.
//!
//! A mod that registers more than one thing writes a [`ContentDeclaration`]
//! rather than a sequence of [`RegistryKey`]s. The declaration is the single
//! place its content is named, so the tags in [`crate::tags`] and the files in
//! [`crate::generate`] are derived from the same statement the barrier claims
//! from, and a mod cannot register a block it forgot to generate a name for.
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
//!
//! // A declaration names both in one place, and names them the same way.
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
/// This is the whole of what a mod writes in [`crate::Mod::register`] when it
/// declared its content up front, and it is a function here rather than a method
/// on [`ContentDeclaration`] because the conversion it performs belongs to the
/// author-facing boundary: the declaration model knows nothing about the
/// lifecycle, and the host contract knows nothing about declarations.
///
/// A conflict is fatal. Content a mod could not claim is content it cannot then
/// use, so carrying on would leave the mod running against a registry it does
/// not own — which is the same reason `ravel-test-mod-registries` stops at its
/// first refusal. A mod that has an alternative does not call this: it claims
/// key by key through [`Registrar::claim`] and decides each refusal for itself.
///
/// # Errors
///
/// Returns [`ModError::Refused`], attributed to the declaring mod and to
/// [`LifecycleState::Constructed`], carrying the refusal's own words.
///
/// # Examples
///
/// ```
/// use ravel_sdk::content::{
///     ContentDeclaration, ContentKind, Registrar, RegistrationRefused, RegistryKey, claim_all,
/// };
/// use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod};
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
