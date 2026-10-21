//! The generated documents this package writes into `generated/`.
//!
//! Nothing here holds data of its own. Each document is rendered from the
//! functions the rest of the crate already exposes, so a change to the wire
//! format or to what a slot admits shows up as a changed document rather than
//! as a stale one. [`documents`] returns every document as a package-relative
//! path and its rendered text, and the inline test below rebuilds the whole set
//! and compares it against the bytes on disk.
//!
//! Every document is tab-separated, is ordered by the crate's own listing order
//! rather than by anything the renderer chooses, and ends with a newline.

use std::fmt::Write as _;

use crate::menus::Slot;
use crate::transport::{Action, Argument, ArgumentKind, TagNumbering, write};

/// The one argument value each kind is tabulated with.
///
/// The value is immaterial to both tables — a slot reads the kind alone, and the
/// encoding table is about the shape a kind occupies rather than about a
/// payload — so each is the smallest value that exercises its field.
fn sample(kind: ArgumentKind) -> Argument {
    match kind {
        ArgumentKind::Absent => Argument::Absent,
        ArgumentKind::Integer => Argument::Integer(-1),
        ArgumentKind::Long => Argument::Long(-1),
        ArgumentKind::Short => Argument::Short(-1),
        ArgumentKind::Boolean => Argument::Boolean(true),
        ArgumentKind::Text => Argument::Text("ab".to_owned()),
        ArgumentKind::Stack => Argument::Stack(vec![0x0a, 0x00]),
        ArgumentKind::Compound => Argument::Compound(vec![0x0a, 0x00]),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(rendered, "{byte:02x}").expect("writing into a String cannot fail");
    }
    rendered
}

/// Renders the wire table for the eight argument kinds.
///
/// Shows what each kind is called, the tag it travels as under the placeholder
/// numbering, whether that numbering reads the tag back as the same kind, and
/// the body a one-argument action carrying it occupies.
///
/// # Panics
///
/// Panics if formatting into a string fails.
#[must_use]
pub fn wire_table() -> String {
    let numbering = TagNumbering::placeholder();
    let mut rendered = String::from("kind\ttag\treverse\tbytes\tbody\n");
    for kind in ArgumentKind::ALL {
        let tag = numbering.number(kind);
        let reverse = numbering.kind(tag).map_or("-", ArgumentKind::as_str);
        let action = Action::carrying("a", vec![sample(kind)]);
        let body = write(&action, &numbering);
        let (length, encoded) = body.as_ref().map_or_else(
            |refused| (0, format!("refused: {refused}")),
            |bytes| (bytes.len(), hex(bytes)),
        );
        writeln!(
            rendered,
            "{}\t{tag}\t{reverse}\t{length}\t{encoded}",
            kind.as_str()
        )
        .expect("writing into a String cannot fail");
    }
    rendered
}

/// Renders the slot table: every slot this package can express, against every
/// argument kind, saying which arguments the slot admits.
///
/// Both axes are the crate's own exhaustive listings, so the table is the whole
/// of the admittance relation and not a sample of it.
///
/// # Panics
///
/// Panics if formatting into a string fails.
#[must_use]
pub fn slot_table() -> String {
    let mut rendered = String::from("slot");
    for kind in ArgumentKind::ALL {
        rendered.push('\t');
        rendered.push_str(kind.as_str());
    }
    rendered.push('\n');
    for kind in ArgumentKind::ALL {
        for (form, slot) in [
            ("exactly", Slot::Exactly(kind)),
            ("optional", Slot::Optional(kind)),
        ] {
            write!(rendered, "{form}:{}", kind.as_str())
                .expect("writing into a String cannot fail");
            for offered in ArgumentKind::ALL {
                rendered.push('\t');
                rendered.push_str(if slot.admits(&sample(offered)) {
                    "yes"
                } else {
                    "no"
                });
            }
            rendered.push('\n');
        }
    }
    rendered
}

/// Every generated document, as a package-relative path and its rendered text.
#[must_use]
pub fn documents() -> Vec<(&'static str, String)> {
    vec![
        ("generated/slots.txt", slot_table()),
        ("generated/transport.txt", wire_table()),
    ]
}

#[cfg(test)]
mod tests {
    use super::documents as render_documents;

    #[test]
    fn the_generated_documents_regenerate_byte_for_byte() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let rendered = render_documents();
        assert!(!rendered.is_empty(), "the package renders documents");
        let regenerating = std::env::var_os("RAVEL_REGENERATE_GENERATED").is_some();
        for (relative, document) in rendered {
            assert!(document.ends_with('\n'), "{relative} must end in a newline");
            let path = root.join(relative);
            if regenerating {
                let parent = path.parent().expect("a document sits in a directory");
                std::fs::create_dir_all(parent).expect("the generated directory is writable");
                std::fs::write(&path, document.as_bytes()).expect("the document is writable");
                continue;
            }
            let on_disk = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{relative} is not on disk: {error}"));
            assert_eq!(
                on_disk, document,
                "{relative} has drifted from the functions that render it"
            );
        }
    }
}
