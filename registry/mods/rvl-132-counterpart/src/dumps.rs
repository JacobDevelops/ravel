//! The derived documents this package writes into `generated/`.
//!
//! Three tables the crate already computes are rendered here rather than
//! transcribed: section nine's attachment table as [`capabilities`], section
//! eight's store-adapter table as [`storage`], and the counts of [`crate::census`]
//! with the arithmetic that ties them together as [`census`]. Nothing here is a
//! new fact; every value comes out of the function that already decides it, so a
//! change to the table is a change to the document.
//!
//! [`all`] is the whole set, and the test beside it holds the bytes on disk to
//! what these functions render.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ravel_sdk::{JsonValue, json};
use rvl_210_counterpart::capabilities::Address;

use crate::capabilities::{VANILLA_ATTACHMENTS, vanilla};
use crate::census as counted;
use crate::settings::{DECLINED, KEYS};
use crate::storage::{Class, Shape, table};

/// Where the attachment table is written.
pub const ATTACHMENT_TABLE: &str = "generated/capabilities.txt";

/// Where the store-adapter table is written.
pub const ADAPTER_TABLE: &str = "generated/storage.txt";

/// Where the counts and their arithmetic are written.
pub const CENSUS_DOCUMENT: &str = "generated/census.json";

/// Every generated document, as a package-relative path and its content.
#[must_use]
pub fn all() -> Vec<(&'static str, String)> {
    vec![
        (ATTACHMENT_TABLE, capabilities()),
        (ADAPTER_TABLE, storage()),
        (CENSUS_DOCUMENT, census()),
    ]
}

/// The vanilla bundle's attachments, one row each, in the order section nine
/// tables them.
///
/// # Panics
///
/// Panics if formatting into a string fails.
#[must_use]
pub fn capabilities() -> String {
    let mut rendered = String::from("capability\tclass\tform\ttarget\n");
    for attachment in vanilla() {
        let form = match attachment.address {
            Address::Specific(_) => "specific",
            Address::Broad(_) => "broad",
        };
        writeln!(
            rendered,
            "{}\t{:?}\t{form}\t{}",
            attachment.capability,
            attachment.class,
            attachment.address.written()
        )
        .expect("writing into a String cannot fail");
    }
    rendered
}

/// The store-adapter table, one row per installed adapter.
///
/// # Panics
///
/// Panics if formatting into a string fails.
#[must_use]
pub fn storage() -> String {
    let mut rendered = String::from("shape\tclass\tkind\tslotted\n");
    for adapter in table() {
        writeln!(
            rendered,
            "{:?}\t{}\t{}\t{}",
            adapter.shape,
            adapter.class.name(),
            adapter.shape.kind(),
            adapter.shape.slotted()
        )
        .expect("writing into a String cannot fail");
    }
    rendered
}

/// The counts of [`crate::census`] together with the sums and products this
/// package derives from them.
#[must_use]
pub fn census() -> String {
    let mut by_capability: BTreeMap<&'static str, usize> = BTreeMap::new();
    for attachment in vanilla() {
        *by_capability.entry(attachment.capability).or_default() += 1;
    }
    let groups: Vec<JsonValue> = counted::WIDENING_GROUPS
        .iter()
        .map(|(group, entries)| json!({ "group": group, "entries": entries }))
        .collect();
    let document = json!({
        "adapters": {
            "shapes": Shape::ALL.len(),
            "classes": Class::ALL.len(),
            "installed": table().len(),
        },
        "attachments": {
            "vanilla": VANILLA_ATTACHMENTS,
            "tabled": vanilla().len(),
            "perCapability": by_capability,
        },
        "settings": {
            "declared": KEYS.len(),
            "declined": DECLINED.len(),
            "specified": KEYS.len() - 1 + DECLINED.len(),
        },
        "sources": {
            "shipped": counted::SHIPPED_SOURCES,
            "harness": counted::HARNESS_SOURCES,
            "withoutHarness": counted::SHIPPED_SOURCES - counted::HARNESS_SOURCES,
            "unitTests": counted::UNIT_TESTS,
            "companions": counted::COMPANION_MODS,
        },
        "resources": {
            "translations": counted::TRANSLATIONS,
            "templates": counted::TEMPLATES,
            "logos": counted::LOGOS,
            "shipped": counted::SHIPPED_RESOURCES,
        },
        "widening": {
            "documents": counted::WIDENING_DOCUMENTS,
            "entries": counted::WIDENING_ENTRIES,
            "groups": groups,
        },
    });
    format!("{document}\n")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn the_generated_documents_regenerate_byte_for_byte() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let regenerating = std::env::var_os("RAVEL_REGENERATE_GENERATED").is_some();
        for (relative, rendered) in all() {
            let path = root.join(relative);
            if regenerating {
                std::fs::create_dir_all(path.parent().expect("a package-relative path"))
                    .expect("the generated directory");
                std::fs::write(&path, rendered.as_bytes()).expect("the document");
            } else {
                let on_disk = std::fs::read_to_string(&path)
                    .unwrap_or_else(|_| panic!("{relative} is not in the package"));
                assert_eq!(
                    on_disk, rendered,
                    "{relative} has drifted from what the crate derives"
                );
            }
        }
    }
}
