//! Deterministic pack generation from content declarations.
//!
//! Ravel derives tag and locale paths and orders files by path. Supply
//! version-specific recipe bodies and models as resource bytes through
//! [`crate::content::ContentDeclaration::resource`].
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::content::{ContentDeclaration, ContentId, ContentKind};
//! use ravel_sdk::tags::TagId;
//! use ravel_sdk::ModId;
//!
//! let owner: ModId = "example".parse()?;
//! let ingots = TagId::new(
//!     ContentKind::Item,
//!     ContentId::new("c".parse()?, "ingots").expect("a legal content path"),
//! )
//! .expect("items are taggable");
//!
//! let mut declaration = ContentDeclaration::new(owner);
//! let copper = declaration
//!     .entry(ContentKind::Item, "copper_ingot")
//!     .expect("a legal, undeclared entry");
//! declaration.tag(&ingots, &copper).expect("an item joins an item tag");
//!
//! let generated = declaration.generate();
//! let tag_file = generated
//!     .file(&ingots.generated_path())
//!     .expect("the tag was declared");
//! assert_eq!(
//!     std::str::from_utf8(tag_file.bytes()).expect("generated JSON is UTF-8"),
//!     "{\n  \"replace\": false,\n  \"values\": [\n    \"example:copper_ingot\"\n  ]\n}\n"
//! );
//!
//! // Generating again produces the same bytes, not an equal-looking value.
//! assert_eq!(declaration.generate(), generated);
//! # Ok::<(), ravel_sdk::ModIdError>(())
//! ```

pub use ravel_data::{GeneratedData, GeneratedFile};
