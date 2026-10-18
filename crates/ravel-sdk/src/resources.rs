//! Supplied resource files and load-order overrides.
//!
//! Declare files in [`crate::content::ContentDeclaration`]. At launch the later
//! supplier wins, and the loader records which suppliers were overridden.
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
