//! Supplied resource files and load-order overrides.
//!
//! Declare files in [`crate::content::ContentDeclaration`]. In [`ResourceSet`],
//! the later supplier wins and overridden suppliers remain recorded.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::resources::{RelativePath, ResourceSet};
//! use ravel_sdk::ModId;
//!
//! let base: ModId = "base_mod".parse()?;
//! let skin: ModId = "skin_mod".parse()?;
//! let shared = RelativePath::try_from("assets/base_mod/lang/en_us.json".to_owned())
//!     .expect("a legal package path");
//!
//! // `skin_mod` loads later, so its copy is the one the game reads, and the mod
//! // it shadowed is named rather than silently replaced.
//! let set = ResourceSet::layered([(&base, [&shared]), (&skin, [&shared])]);
//! assert_eq!(set.supplier(&shared), Some(&skin));
//! assert_eq!(set.overrides()[0].shadowed, [base]);
//! # Ok::<(), ravel_sdk::ModIdError>(())
//! ```

pub use ravel_core::{RelativePath, RelativePathError};
pub use ravel_data::{Override, Resource, ResourceSet};
