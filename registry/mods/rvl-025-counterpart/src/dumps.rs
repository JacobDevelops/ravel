//! The tables this package already derives, rendered as documents a reader can
//! diff.
//!
//! Nothing here decides anything. Every figure is read back out of the modules
//! that own it — [`crate::census`] for what is registered and what ships beside
//! it, [`crate::capabilities`] for the eight published groups, and
//! [`crate::network`] for the six packet contracts — so a document that drifts
//! from the crate is the crate having changed, which is what the regeneration
//! test in this module exists to catch.
//!
//! Each document is rendered from a fixed order the owning module already
//! publishes: the registration order of §4, the census order of §15, and §13's
//! table order. Nothing here re-sorts them, because re-sorting would hide the
//! order the specification counts in.

use std::fmt::Write as _;

use ravel_sdk::{JsonValue, json};

use crate::{capabilities, census, network};

/// The relative path of the registration census.
pub const CENSUS_PATH: &str = "generated/census.json";

/// The relative path of the published-group census.
pub const CAPABILITIES_PATH: &str = "generated/capabilities.json";

/// The relative path of the packet contract table.
pub const NETWORK_PATH: &str = "generated/network.txt";

/// Every document this package writes, as a relative path and its bytes.
#[must_use]
pub fn documents() -> Vec<(&'static str, String)> {
    vec![
        (CENSUS_PATH, census_document()),
        (CAPABILITIES_PATH, capabilities_document()),
        (NETWORK_PATH, network_table()),
    ]
}

/// Renders §4's and §5's counts, the registered names and the five defects.
#[must_use]
pub fn census_document() -> String {
    let defects: Vec<JsonValue> = census::Defect::ALL
        .into_iter()
        .map(|defect| json!({ "defect": format!("{defect:?}"), "shipped": defect.shipped() }))
        .collect();
    let document = json!({
        "items": census::ITEMS,
        "blocks": census::BLOCKS,
        "creativeGrouping": census::creative_grouping(),
        "outsideGrouping": census::OUTSIDE_GROUPING,
        "intermediates": census::INTERMEDIATES,
        "withoutRecipes": {
            "items": census::NO_RECIPE_ITEMS,
            "blocks": census::NO_RECIPE_BLOCKS,
        },
        "claimedAndEmpty": census::CLAIMED_AND_EMPTY,
        "counts": {
            "items": census::ITEMS.len(),
            "blocks": census::BLOCKS.len(),
            "recipes": census::RECIPES,
            "advancements": census::ADVANCEMENTS,
            "lootTables": census::LOOT_TABLES,
            "tagDocuments": census::TAG_DOCUMENTS,
            "blockStateDocuments": census::BLOCK_STATE_DOCUMENTS,
            "modelDocuments": census::MODEL_DOCUMENTS,
            "languageEntries": census::LANGUAGE_ENTRIES,
            "generatedDocuments": census::GENERATED_DOCUMENTS,
            "handWrittenDocuments": census::HAND_WRITTEN_DOCUMENTS,
            "blockEntityKinds": census::BLOCK_ENTITY_KINDS,
            "menuKinds": census::MENU_KINDS,
            "componentKinds": census::COMPONENT_KINDS,
            "attachments": census::ATTACHMENTS,
            "placementModifierKinds": census::PLACEMENT_MODIFIER_KINDS,
            "creativeGroupings": census::CREATIVE_GROUPINGS,
        },
        "defects": defects,
    });
    format!("{document}\n")
}

/// Renders §15's published obligations and what each one is here.
#[must_use]
pub fn capabilities_document() -> String {
    let groups: Vec<JsonValue> = capabilities::Group::ALL
        .into_iter()
        .map(|group| {
            json!({
                "group": group.to_string(),
                "implemented": group.implemented(),
                "testableAlone": group.testable_alone(),
            })
        })
        .collect();
    let document = json!({
        "groups": groups,
        "builtInData": capabilities::BUILT_IN_DATA,
    });
    format!("{document}\n")
}

/// Renders §13's six contracts as a tab-separated table.
#[must_use]
pub fn network_table() -> String {
    let mut table = String::from("packet\tdirection\tversion\n");
    for contract in network::contracts() {
        writeln!(
            table,
            "{}\t{}\t{}",
            contract.id(),
            contract.direction(),
            contract.version()
        )
        .expect("writing to a String cannot fail");
    }
    table
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn the_generated_documents_regenerate_byte_for_byte() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let rendered = documents();
        assert!(!rendered.is_empty(), "the package renders documents");
        let regenerating = std::env::var_os("RAVEL_REGENERATE_GENERATED").is_some();
        for (relative, document) in rendered {
            assert!(document.ends_with('\n'), "{relative} must end in a newline");
            let path = root.join(relative);
            if regenerating {
                let parent = path.parent().expect("a document sits in a directory");
                std::fs::create_dir_all(parent).expect("the directory is writable");
                std::fs::write(&path, document.as_bytes()).expect("the document is writable");
                continue;
            }
            let on_disk = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{relative} could not be read: {error}"));
            assert_eq!(
                on_disk, document,
                "{relative} has drifted from what the crate derives"
            );
        }
    }
}
