//! Supplied resource files and load-order overrides.
//!
//! Declare files in [`crate::content::ContentDeclaration`]. At launch ordinary
//! resource paths use the later supplier, and the loader records overridden suppliers.
//! Tag documents at `data/<namespace>/tags/<registry>/*.json` instead merge members
//! in load order; `replace: true` discards earlier members. Unknown document fields
//! come from the last contributor rather than merging.
//! Launch-wide layering belongs to the loader, not individual mods.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::content::{ContentDeclaration, ContentDeclarationError};
//! use ravel_sdk::resources::RelativePath;
//!
//! let mut declared = ContentDeclaration::new("example".parse().expect("a legal mod id"));
//! let path = RelativePath::try_from("assets/example/lang/en_us.json".to_owned())
//!     .expect("a legal package path");
//! declared.resource(path.clone(), br#"{"item.example.plate":"Plate"}"#.to_vec())?;
//! assert!(declared.generate().file(&path).is_some());
//! # Ok::<(), ContentDeclarationError>(())
//! ```

pub use ravel_core::{RelativePath, RelativePathError};
