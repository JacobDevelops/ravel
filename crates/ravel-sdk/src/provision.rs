//! Per-mod shared-library provider declarations.
//!
//! Libraries remain ordinary Cargo dependencies: this module loads nothing,
//! and production launches do not yet consume these declarations. Set-wide
//! provider selection belongs to the host's `ravel-resolve` layer, not the SDK.
//!
//! ```
//! use ravel_sdk::provision::Provided;
//! use ravel_sdk::Version;
//!
//! let provided = Provided::new("shared_maths", Version::parse("2.1.0")?);
//! assert_eq!(provided.library(), "shared_maths");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub use ravel_host_api::Provided;
