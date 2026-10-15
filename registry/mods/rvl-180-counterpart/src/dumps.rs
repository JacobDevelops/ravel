//! The documents this package writes out of its own functions, and the test
//! that holds the written tree to them.
//!
//! Nothing new is decided here. Every value below is read back out of
//! [`crate::Shared::declaration`], [`crate::grouping::Aggregation::resolve`] and
//! [`crate::sprites`], so a document under `generated/` is a rendering of what
//! this package already computes rather than a second statement of it that could
//! disagree. That is the whole reason the module exists: `NOTES.md` claims a
//! thirty-four group contribution resolving through leaves and a twenty-one
//! sprite set, and a reader can now count both without building the crate.
//!
//! The three documents are deterministic. The declaration's own generation
//! orders its files by path and sorts each group's members, [`crate::grouping`]
//! keeps its groups in a [`std::collections::BTreeMap`], and the sprite set has
//! a fixed order the specification counts it in; nothing here reads a clock, an
//! environment variable or a hash order.

use std::fmt::Write as _;

use ravel_sdk::content::ContentDeclaration;
use ravel_sdk::prelude::*;
use ravel_sdk::{JsonValue, json};

use crate::grouping::{self, Aggregation};
use crate::{Shared, sprites};

/// Where the generated pack this package's declaration produces is written.
pub const DECLARATIONS: &str = "generated/declarations.json";

/// Where the resolved groups are written.
pub const GROUPING: &str = "generated/grouping.txt";

/// Where the sprite set is written.
pub const SPRITES: &str = "generated/sprites.txt";

/// Returns every document, each as its path relative to the package root and
/// its exact text.
///
/// # Errors
///
/// Returns [`ModError::Refused`] when a spelling this package holds as a
/// constant is illegal, which would be the same packaging failure
/// [`crate::Shared::declaration`] reports, and when a file the declaration
/// generates is not the JSON document it is generated as.
pub fn documents() -> Result<Vec<(&'static str, String)>, ModError> {
    let aggregation =
        Aggregation::conventional().map_err(|error| ModError::refused(error.to_string()))?;
    let declaration = Shared::declaration(&aggregation)?;
    Ok(vec![
        (DECLARATIONS, declarations_document(&declaration)?),
        (GROUPING, grouping_document(&aggregation)?),
        (SPRITES, sprites_document()?),
    ])
}

/// Returns the pack the declaration generates, path by path.
///
/// Each file is parsed back into a value rather than quoted as a line of text,
/// so the group documents, the language document and the atlas read as what they
/// are. The parse is also the check that they are documents at all.
///
/// # Errors
///
/// Returns [`ModError::Refused`] when a generated file is not UTF-8 JSON.
pub fn declarations_document(declaration: &ContentDeclaration) -> Result<String, ModError> {
    let mut files = Vec::new();
    for file in declaration.generate().files() {
        let path = file.path().as_str();
        let text = std::str::from_utf8(file.bytes())
            .map_err(|source| ModError::refused(format!("{path} is not text: {source}")))?;
        let document: JsonValue = text.parse().map_err(|source| {
            ModError::refused(format!("{path} is not a JSON document: {source}"))
        })?;
        files.push(json!({ "path": path, "document": document }));
    }
    Ok(format!("{}\n", json!({ "files": files })))
}

/// Returns one tab-separated row per member each of the thirty-four groups
/// resolves to.
///
/// An aggregate holds no member of its own, so every row under one is a member
/// the resolution reached through a leaf. That is the §4 property this package
/// is built around, and the document is where it can be counted.
///
/// # Errors
///
/// Returns [`ModError::Refused`] when a group's spelling is illegal.
pub fn grouping_document(aggregation: &Aggregation) -> Result<String, ModError> {
    let mut document = String::from("group\tmember\n");
    for group in grouping::all().map_err(|error| ModError::refused(error.to_string()))? {
        for member in aggregation.resolve(&group).members {
            writeln!(document, "{group}\t{member}").expect("writing to a String cannot fail");
        }
    }
    Ok(document)
}

/// Returns one tab-separated row per interface sprite: its name, the group §8
/// counts it in, whether it is scaled, and the path a drawing of it would land
/// on.
///
/// No image sits behind any of these paths. `NOTES.md` says so under what is not
/// provided, and this document is the list a set drawn later would be drawn
/// against.
///
/// # Errors
///
/// Returns [`ModError::Refused`] when a derived sprite path is illegal.
pub fn sprites_document() -> Result<String, ModError> {
    let mut document = String::from("sprite\tgroup\tscales\tpath\n");
    for name in sprites::names() {
        let group = if sprites::POINTERS.contains(&name) {
            "pointer"
        } else if sprites::WIDGETS.contains(&name) {
            "widget"
        } else {
            "frame"
        };
        let path = sprites::sprite_path(name).map_err(|error| ModError::refused(error.to_string()))?;
        writeln!(
            document,
            "{name}\t{group}\t{}\t{}",
            sprites::scales(name),
            path.as_str()
        )
        .expect("writing to a String cannot fail");
    }
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::documents;
    use std::path::Path;

    /// Rebuilds every document from this package's own functions and holds the
    /// tree under `generated/` to the bytes they produce.
    ///
    /// Setting `RAVEL_REGENERATE_GENERATED` writes the documents instead of
    /// comparing them, which is how the tree is produced; that is the only way
    /// it is produced, so a document nobody can regenerate cannot be committed.
    #[test]
    fn the_generated_documents_regenerate_byte_for_byte() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let regenerate = std::env::var_os("RAVEL_REGENERATE_GENERATED").is_some();
        let rendered = documents().expect("this package's constants are legal");
        assert_eq!(rendered.len(), 3);
        for (relative, document) in rendered {
            assert!(
                document.ends_with('\n'),
                "{relative} does not end with a newline"
            );
            let path = root.join(relative);
            if regenerate {
                let directory = path.parent().expect("the path names a directory");
                std::fs::create_dir_all(directory)
                    .unwrap_or_else(|error| panic!("{relative} is not writable: {error}"));
                std::fs::write(&path, &document)
                    .unwrap_or_else(|error| panic!("{relative} is not writable: {error}"));
            } else {
                let found = std::fs::read_to_string(&path).unwrap_or_else(|error| {
                    panic!(
                        "{relative} is not readable: {error}. Set RAVEL_REGENERATE_GENERATED to \
                         write it."
                    )
                });
                assert_eq!(
                    found, document,
                    "{relative} has drifted from what this package generates"
                );
            }
        }
    }
}
