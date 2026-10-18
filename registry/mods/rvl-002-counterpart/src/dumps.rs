//! The documents this entry renders out of its own tables.
//!
//! Nothing here decides anything. Each function walks a table or a derivation
//! that already exists elsewhere in the crate and writes what it finds, so a
//! document that disagrees with the crate is a document that has drifted rather
//! than a second statement of the same rule. The census walk is
//! [`crate::census`]'s registry order, the food rows are [`crate::foods`]'s
//! cumulative effect rule applied at every declared tier, and the fuel table is
//! [`crate::fuel`]'s two derivations rather than the twelve constants they
//! reproduce.
//!
//! The order of every list is the order the crate already fixes, so no sort is
//! needed and none is applied: sorting here would hide the very orderings
//! sections 7.2 and 10 make observable.

use std::fmt::Write as _;

use ravel_sdk::{JsonValue, json};

use crate::{census, foods, fuel};

/// Where the census document is written, relative to the package root.
pub const CENSUS_PATH: &str = "generated/census.json";

/// Where the food document is written, relative to the package root.
pub const FOODS_PATH: &str = "generated/foods.json";

/// Where the fuel table is written, relative to the package root.
pub const FUEL_PATH: &str = "generated/fuel.txt";

/// Every document, each as its relative path and its rendered bytes.
#[must_use]
pub fn documents() -> Vec<(&'static str, String)> {
    vec![
        (CENSUS_PATH, census_document()),
        (FOODS_PATH, foods_document()),
        (FUEL_PATH, fuel_table()),
    ]
}

/// The registered content, counted and named in registry order.
///
/// The counts are the ones [`census`] derives rather than the ones it declares:
/// a group table that stopped adding to twenty-six would be visible here.
#[must_use]
pub fn census_document() -> String {
    let groups: Vec<JsonValue> = census::BLOCK_GROUPS
        .iter()
        .map(|group| {
            json!({
                "name": group.name,
                "count": group.count,
                "hardness": group.hardness,
                "resistance": group.resistance,
                "blockItem": group.block_item,
            })
        })
        .collect();
    let listing: Vec<JsonValue> = census::LISTING
        .iter()
        .map(|section| {
            json!({
                "section": format!("{section:?}"),
                "guarded": census::guarded(*section),
            })
        })
        .collect();
    let document = json!({
        "blocks": census::block_paths().len(),
        "blockItems": census::block_item_paths().len(),
        "items": census::item_paths().len(),
        "plainItems": census::plain_item_paths().len(),
        "fluidRegistrations": census::FLUID_REGISTRATIONS,
        "groups": groups,
        "blockPaths": census::block_paths(),
        "blockItemPaths": census::block_item_paths(),
        "itemPaths": census::item_paths(),
        "storageBlockPaths": census::storage_block_paths(),
        "oreBlockPaths": census::ore_block_paths(),
        "anchorBlockPaths": census::anchor_block_paths(),
        "toolItemPaths": census::tool_item_paths(),
        "listing": listing,
    });
    format!("{document}\n")
}

/// The six food rows with the effects each tier grants.
///
/// The effect list per row is [`foods::effects`] rather than a written-out
/// prefix, so the cumulative rule is what produces the document.
#[must_use]
pub fn foods_document() -> String {
    let rows: Vec<JsonValue> = foods::DECLARED
        .iter()
        .map(|food| {
            let effects: Vec<String> = foods::effects(food.tier)
                .iter()
                .map(|effect| format!("{effect:?}"))
                .collect();
            json!({
                "tier": food.tier,
                "nutrition": food.nutrition,
                "saturation": food.saturation,
                "effects": effects,
            })
        })
        .collect();
    let document = json!({
        "amplifier": foods::AMPLIFIER,
        "alwaysEdible": foods::ALWAYS_EDIBLE,
        "ticksPerSecond": foods::TICKS_PER_SECOND,
        "order": foods::ORDER
            .iter()
            .map(|effect| format!("{effect:?}"))
            .collect::<Vec<String>>(),
        "rows": rows,
    });
    format!("{document}\n")
}

/// The twelve burn times as a tab-separated table, one row per tier.
///
/// Both columns come out of the derivations, so the factor of nine between an
/// item and its block is a property of the table rather than a claim beside it.
#[must_use]
pub fn fuel_table() -> String {
    let mut rendered = String::from("tier\titem\tblock\n");
    for (tier, (item, block)) in
        (1..=fuel::TIERS).zip(fuel::item_series().into_iter().zip(fuel::block_series()))
    {
        writeln!(rendered, "{tier}\t{item}\t{block}").expect("writing to a String cannot fail");
    }
    rendered
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::documents;

    #[test]
    fn the_generated_documents_regenerate_byte_for_byte() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let regenerating = std::env::var_os("RAVEL_REGENERATE_GENERATED").is_some();
        for (relative, rendered) in documents() {
            assert!(
                rendered.ends_with('\n'),
                "{relative} does not end with a newline"
            );
            let path = root.join(relative);
            if regenerating {
                let directory = path.parent().expect("a document sits in a directory");
                fs::create_dir_all(directory).expect("the generated directory is writable");
                fs::write(&path, rendered.as_bytes()).expect("the document is writable");
                continue;
            }
            let found = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{relative} is not on disk: {error}"));
            assert_eq!(
                found, rendered,
                "{relative} has drifted from the functions that render it"
            );
        }
    }
}
