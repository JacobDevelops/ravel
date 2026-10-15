//! The derived tables this package writes out beside its source.
//!
//! Three of this package's answers are arithmetic a dependent has to agree with
//! rather than prose it can read: the decoding shape the validators are written
//! against, the two halves a wide value is stored as, and the packed layout a
//! colour reaches saved data and network payloads through. Each is rendered here
//! from the same functions the rest of the crate calls, so a document beside the
//! source is the crate's own answer rather than a copy of it, and the one test
//! below fails when the two disagree.
//!
//! Every document is deterministic and ends with a trailing newline.

use std::fmt::Write as _;

use ravel_sdk::{JsonValue, json};

use crate::colour::Colour;
use crate::transport::WideValues;
use crate::validators::{Family, KindName, Vocabulary};

/// The relative path and rendered body of every document this module writes.
///
/// Ordered by path, so the set a caller walks does not depend on the order the
/// renderers happen to be listed in.
#[must_use]
pub fn documents() -> Vec<(String, String)> {
    let mut documents = vec![colour_layout(), validator_shape(), wide_value_halves()];
    documents.sort_by(|left, right| left.0.cmp(&right.0));
    documents
}

/// Renders the decoding shape the eleven validator kinds are written against.
///
/// The spellings are [`Vocabulary::placeholder`], which is this package's own
/// vocabulary; the family each kind belongs to is named by the gate the
/// vocabulary recognises that family through, which is the field its default
/// kind reads its argument from.
#[must_use]
pub fn validator_shape() -> (String, String) {
    let vocabulary = Vocabulary::placeholder();
    let (lower, upper) = vocabulary.bounds();
    let kinds: Vec<JsonValue> = KindName::ALL
        .into_iter()
        .map(|kind| {
            json!({
                "name": vocabulary.kind(kind),
                "field": vocabulary.field(kind),
                "gate": vocabulary.field(kind.family().default_kind()),
                "familyDefault": kind.is_family_default(),
            })
        })
        .collect();
    let gates: Vec<JsonValue> = Family::ALL
        .into_iter()
        .map(|family| {
            json!({
                "gate": vocabulary.field(family.default_kind()),
                "default": vocabulary.kind(family.default_kind()),
            })
        })
        .collect();
    let document = json!({
        "discriminator": vocabulary.discriminator(),
        "bounds": { "lower": lower, "upper": upper },
        "families": gates,
        "kinds": kinds,
    });
    (
        "generated/validators.json".to_owned(),
        format!("{document}\n"),
    )
}

/// Renders the two halves each landmark value is stored as.
///
/// The values are every single-bit magnitude and its negation, plus the three
/// values the sign handling turns on, and each row is written and read back
/// through a [`WideValues`] of one so the halves are the ones the store keeps.
///
/// # Panics
///
/// Panics when a store of one value refuses index zero, which would mean the
/// index arithmetic no longer hides the doubling.
#[must_use]
pub fn wide_value_halves() -> (String, String) {
    let mut values = vec![i32::MIN, -1, 0, i32::MAX];
    for bit in 0..31 {
        let magnitude = 1_i32 << bit;
        values.push(magnitude);
        values.push(-magnitude);
    }
    values.sort_unstable();
    values.dedup();

    let mut store = WideValues::new(1).expect("a store of one value is inside the ceiling");
    let mut body = String::from("value\thigh\tlow\trecomposed\n");
    for value in values {
        store
            .set(0, value)
            .expect("index zero exists in a store of one");
        let high = store
            .get_narrow(0)
            .expect("the high half is the first narrow slot");
        let low = store
            .get_narrow(1)
            .expect("the low half is the second narrow slot");
        let recomposed = store.get(0).expect("index zero exists in a store of one");
        let _ = writeln!(body, "{value}\t{high}\t{low}\t{recomposed}");
    }
    ("generated/transport.txt".to_owned(), body)
}

/// Renders the packed colour layout, one bit at a time.
///
/// Each row reads a packed word holding a single set bit back into its quartet
/// and packs the quartet again, so the row says which channel that bit belongs
/// to and that the round trip returns it unchanged.
#[must_use]
pub fn colour_layout() -> (String, String) {
    let mut body = String::from("packed\tred\tgreen\tblue\talpha\trepacked\n");
    for bit in 0..u32::BITS {
        let packed = 1_u32 << bit;
        let colour = Colour::from_packed(packed);
        let _ = writeln!(
            body,
            "{:08x}\t{}\t{}\t{}\t{}\t{:08x}",
            packed,
            colour.red,
            colour.green,
            colour.blue,
            colour.alpha,
            colour.packed()
        );
    }
    ("generated/colour.txt".to_owned(), body)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::documents;

    #[test]
    fn the_generated_documents_regenerate_byte_for_byte() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let documents = documents();
        assert_eq!(
            documents.len(),
            3,
            "every renderer contributes one document"
        );
        for (path, rendered) in documents {
            assert!(
                rendered.ends_with('\n'),
                "`{path}` does not end with a trailing newline"
            );
            let full = root.join(&path);
            if std::env::var_os("RAVEL_REGENERATE_GENERATED").is_some() {
                let parent = full.parent().expect("every document names a directory");
                std::fs::create_dir_all(parent).expect("the directory is this package's own");
                std::fs::write(&full, rendered.as_bytes())
                    .expect("the document is this package's own");
                continue;
            }
            let written = std::fs::read_to_string(&full)
                .unwrap_or_else(|error| panic!("`{path}` cannot be read: {error}"));
            assert_eq!(
                written, rendered,
                "`{path}` no longer holds what this crate derives"
            );
        }
    }
}
