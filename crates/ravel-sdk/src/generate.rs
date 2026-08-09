//! The pack a declaration produces, and why generating it twice gives the same
//! bytes.
//!
//! A mod declares content; Ravel writes the files that content needs. The paths
//! are derived rather than declared — a tag lands at
//! `data/<namespace>/tags/<registry>/<path>.json` and a locale at
//! `assets/<namespace>/lang/<locale>.json` — so a mod cannot misfile its own
//! generated data, and two of a mod's declarations that would write one file are
//! refused when the declaration is made rather than when the file is written.
//!
//! Determinism is a property of the values rather than a promise in prose.
//! [`crate::content::ContentDeclaration`] keeps every collection it generates
//! from keyed or sorted by content, [`GeneratedData`] orders its files by path,
//! and the JSON is rendered through one function whose object keys come out of a
//! `BTreeMap`. Nothing reads a clock, an environment variable, a filesystem, or a
//! hash map's iteration order. So the same declaration produces the same bytes,
//! and the same content declared in a different order produces the same bytes
//! too — which is what `tests/conformance/tests/content_declarations.rs`
//! compares rather than asserts.
//!
//! What is deliberately *not* generated is anything a Minecraft version decides
//! the shape of: a recipe body, a model, a translation key's spelling. Those are
//! the author's, supplied as bytes through
//! [`crate::content::ContentDeclaration::resource`], and they travel in the same
//! output so a mod's pack is one set rather than a generated half and a
//! hand-written half that can disagree about a path.
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
