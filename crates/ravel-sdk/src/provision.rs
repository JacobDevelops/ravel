//! Which mod supplies a shared library, and at which version.
//!
//! [`crate::services`] orders named calls between two mods that are both
//! installed. This is the other half of "two mods that know about each other":
//! one mod supplies a library several others build on, and a launch has to agree
//! on exactly one supplier of it. A mod declares what it provides, a mod that
//! uses it declares the range it needs, and [`Provision::resolve`] either names
//! the one provider or refuses the set — naming the library and the mods on both
//! sides.
//!
//! # This is not jar-in-jar, and it does not need to be
//!
//! A mod is distributed as source the player's machine compiles. A library two
//! mods both build on is therefore
//! an ordinary Cargo dependency, resolved once for the build: there is no second
//! copy to load, no shading, and nothing here bundles, fetches, or unpacks
//! anything. **This module ships no loading mechanism and no library.**
//!
//! What Cargo cannot see is the mod *set*. That two installed mods each claim to
//! be the one supplying `shared_maths` to the others, or that one of them needs
//! a major version its supplier does not offer, is a fact about a launch rather
//! than about any one package's manifest — and it is the fact this module
//! decides.
//!
//! # What this does not give a mod
//!
//! **Nothing reads a declaration in a running game.** No launch path builds a
//! [`Provision`] from the packages it composed, exactly as none carries a packet;
//! `docs/guide/limits.md` says so in the same terms. What ships is the
//! declaration, the resolution, and the four refusals, all decidable without a
//! game — so a mod that declares what it provides today is stating the version
//! it will be held to.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::provision::{Provided, Provision};
//! use ravel_sdk::{ModId, VersionRange};
//!
//! let provider: ModId = "maths_mod".parse()?;
//! let consumer: ModId = "shop_mod".parse()?;
//! let older: ModId = "legacy_mod".parse()?;
//!
//! let mut provision = Provision::default();
//! provision.provides(&provider, Provided::new("shared_maths", "2.1.0"));
//! provision.requires(&consumer, "shared_maths", VersionRange::parse("^2")?);
//!
//! // The resolution names the provider rather than only succeeding.
//! let resolved = provision.resolve().expect("one provider answers one requirement");
//! assert_eq!(resolved.provider_of("shared_maths"), Some(&provider));
//! assert_eq!(resolved.version_of("shared_maths"), Some("2.1.0"));
//!
//! // A mod that needs a version this provider does not supply is named, and the
//! // whole set is refused rather than started on a library half of it cannot use.
//! provision.requires(&older, "shared_maths", VersionRange::parse("^1")?);
//! let refusal = provision.resolve().expect_err("no one version answers both");
//! assert_eq!(refusal.library(), "shared_maths");
//! assert_eq!(refusal.unsatisfied(), [older]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub use ravel_host_api::{Provided, Provision, ProvisionRefused, Resolved};
