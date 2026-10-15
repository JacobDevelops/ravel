//! Shared-library provider declarations and version resolution.
//!
//! [`Provision::resolve`] selects one supplier per library or refuses the set.
//! Libraries remain ordinary Cargo dependencies: this module loads nothing,
//! and production launches do not yet consume these declarations.
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
