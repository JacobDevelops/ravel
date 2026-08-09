//! Files a mod supplies, and what happens when two mods supply the same one.
//!
//! A resource is neither a registry claim nor a tag. Two mods claiming
//! `block:example:copper` is a conflict, two mods contributing to `c:ingots` is a
//! merge, and two mods supplying `assets/minecraft/lang/en_us.json` is neither:
//! it is an override, which is how every resource system in modded Minecraft
//! works. The mod later in the resolved load order supplies the path, and the
//! ones it took the path from are recorded — because a player whose translation
//! vanished is owed the name of the mod that took it.
//!
//! A mod declares the files it supplies in its
//! [`crate::content::ContentDeclaration`], either derived — a tag, a locale's
//! translations — or as bytes it hands over for content Ravel does not model.
//! [`ResourceSet`] is the layered view, and the loader owns the real one, because
//! the resolved order is the loader's to decide and a mod cannot see it.
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
