//! The documents this entry derives from its own tables and writes to disk.
//!
//! Nothing here holds data of its own. Each function renders what another
//! module already computes: [`crate::recipes::derive`] over the tiers and kinds
//! [`crate::model`] ships, the upgrade arithmetic of [`crate::machines`], and
//! the deposit table and substitution predicate of [`crate::world`]. The
//! rendered bytes live under `generated/` beside the crate and the test in this
//! module rebuilds them, so a change to any of those tables that is not
//! reflected on disk fails rather than drifts.
//!
//! Every document ends with a trailing newline and every collection whose order
//! is not already fixed by the table it came from is sorted here.

use std::fmt::Write as _;

use ravel_sdk::{JsonValue, json};
use rvl_042_counterpart::Stack;

use crate::machines::{Bases, Facing, Sweep, Upgrade};
use crate::model::{Crafted, Crop, CropKind, Tier};
use crate::recipes::{DerivedRecipe, Ingredient};
use crate::world::{Placed, Stage, Where};

/// Where the derived recipe document is written.
pub const RECIPES_PATH: &str = "generated/recipes.json";
/// Where the machine arithmetic table is written.
pub const MACHINES_PATH: &str = "generated/machines.txt";
/// Where the deposit table is written.
pub const WORLD_PATH: &str = "generated/world.txt";

/// Every document, as a relative path and the bytes that belong at it.
#[must_use]
pub fn documents() -> Vec<(&'static str, String)> {
    vec![
        (RECIPES_PATH, recipe_document()),
        (MACHINES_PATH, machine_document()),
        (WORLD_PATH, world_document()),
    ]
}

/// The crops the recipe document is derived over.
///
/// One crop per shipped tier and shipped kind, named from the two identifiers
/// it joins, put through the adjust pass so that a tier has a ground and a
/// material and a kind has a base seed. Nothing outside this crate names any of
/// it: a real crop catalogue belongs to whoever declares it.
fn subjects() -> Vec<(Crop, Tier, CropKind)> {
    let mut subjects = Vec::new();
    for tier in crate::model::shipped_tiers() {
        let mut tier = tier;
        let tier_path = tier.id.path().to_owned();
        tier.adjust(
            format!("{}:{tier_path}_ground", crate::MOD_ID)
                .parse()
                .expect("a legal ground identity"),
            format!("{}:{tier_path}_material", crate::MOD_ID)
                .parse()
                .expect("a legal material identity"),
        );
        for kind in crate::model::shipped_kinds() {
            let mut kind = kind;
            let kind_path = kind.id.path().to_owned();
            kind.adjust(
                format!("{}:{kind_path}_seed", crate::MOD_ID)
                    .parse()
                    .expect("a legal seed identity"),
            );
            let crop = Crop::new(
                format!("{}:{tier_path}_{kind_path}", crate::MOD_ID)
                    .parse()
                    .expect("a legal crop identity"),
                tier.id.clone(),
                kind.id.clone(),
                Crafted::Entry {
                    id: format!("{}:{tier_path}_crafted", crate::MOD_ID)
                        .parse()
                        .expect("a legal material identity"),
                    data: None,
                },
            );
            subjects.push((crop, tier.clone(), kind));
        }
    }
    subjects
}

/// The three recipes each subject crop derives, rendered as one document.
///
/// The plain-grid seed recipe is switched off in the shipped settings, so it is
/// derived here with the setting on: the document states what the derivation
/// produces, not what one configuration selects. Which of the three a running
/// entry contributes stays [`crate::recipes::derive`]'s answer.
#[must_use]
pub fn recipe_document() -> String {
    let resolves = |_: &ravel_sdk::content::ContentId| 1usize;
    let mut rendered: Vec<(String, JsonValue)> = Vec::new();
    for (crop, tier, kind) in subjects() {
        for recipe in crate::recipes::derive(&crop, &tier, &kind, true, &resolves) {
            rendered.push((recipe.id().to_owned(), recipe_value(&crop, &recipe)));
        }
    }
    rendered.sort_by(|left, right| left.0.cmp(&right.0));
    let document = json!({
        "schemaVersion": 1,
        "owner": crate::MOD_ID,
        "recipes": rendered.into_iter().map(|(_, value)| value).collect::<Vec<_>>(),
    });
    format!("{document:#}\n")
}

fn recipe_value(crop: &Crop, recipe: &DerivedRecipe) -> JsonValue {
    match recipe {
        DerivedRecipe::RitualSeed(ritual) => json!({
            "id": ritual.id,
            "kind": crate::RECIPE_KINDS[0],
            "crop": crop.id,
            "altar": ingredient_value(&ritual.altar),
            "ring": ritual.ring.iter().map(ingredient_value).collect::<Vec<_>>(),
            "result": stack_value(&ritual.result),
            "copiesData": ritual.copies_data,
        }),
        DerivedRecipe::GridSeed { id, grid, result } => json!({
            "id": id,
            "kind": "plain_grid_seed",
            "crop": crop.id,
            "grid": grid
                .iter()
                .map(|slot| slot.as_ref().map_or(JsonValue::Null, ingredient_value))
                .collect::<Vec<_>>(),
            "result": stack_value(result),
        }),
        DerivedRecipe::Conversion { id, input, result } => json!({
            "id": id,
            "kind": "seed_conversion",
            "crop": crop.id,
            "input": ingredient_value(input),
            "result": stack_value(result),
        }),
    }
}

fn ingredient_value(ingredient: &Ingredient) -> JsonValue {
    match ingredient {
        Ingredient::Entry(entry) => json!({ "kind": "entry", "entry": entry }),
        Ingredient::Tag(tag) => json!({ "kind": "tag", "tag": tag }),
        Ingredient::TillingTool => json!({ "kind": "tilling_tool" }),
        Ingredient::ChargedContainer => json!({ "kind": "charged_container" }),
        Ingredient::CropComponent { crop, role } => json!({
            "kind": "crop_component",
            "crop": crop,
            "role": format!("{role:?}"),
        }),
    }
}

fn stack_value(stack: &Stack) -> JsonValue {
    json!({ "entry": stack.entry, "count": stack.count })
}

/// The machine bases, one row per machine and upgrade.
const MACHINE_BASES: [(&str, Bases); 5] = [
    ("smelting", crate::machines::SMELTING),
    ("conversion", crate::machines::CONVERSION),
    ("extraction", crate::machines::EXTRACTION),
    ("harvesting", crate::machines::HARVESTING),
    ("summoning", crate::machines::SUMMONING),
];

/// What every machine runs at with no upgrade and with each of the six.
///
/// The operation time, the fuel usage and the capacity are the upgrade's own
/// arithmetic; the reach, the swept square's side and its area come from
/// [`Sweep`], which reads the upgrade's range and nothing else. The machine
/// rows are stated in the order the bases are declared and the upgrade rows in
/// the order the table states them, so nothing here needs sorting.
#[must_use]
pub fn machine_document() -> String {
    let mut document = String::new();
    document.push_str(
        "machine\tupgrade\toperation_ticks\tusage_per_tick\tcapacity\treach\tside\tarea\n",
    );
    for (machine, bases) in MACHINE_BASES {
        for upgrade in std::iter::once(None).chain(crate::machines::UPGRADES.map(Some)) {
            let sweep = Sweep {
                origin: (0, 0, 0),
                facing: Facing::North,
                range: Sweep::range(upgrade),
            };
            let position =
                upgrade.map_or_else(|| "none".to_owned(), |it| it.position().to_string());
            let operation =
                upgrade.map_or(bases.operation, |it| it.operation_ticks(bases.operation));
            let usage = upgrade.map_or(bases.usage, |it| it.usage_per_tick(bases.usage));
            let capacity = upgrade.map_or(crate::machines::BASE_CAPACITY, Upgrade::capacity);
            writeln!(
                document,
                "{machine}\t{position}\t{operation}\t{usage}\t{capacity}\t{}\t{}\t{}",
                sweep.range,
                sweep.side(),
                sweep.area()
            )
            .expect("writing to a String cannot fail");
        }
    }
    document
}

/// The chances the substitution readings are taken at.
const SUBSTITUTION_DRAWS: [f64; 5] = [0.0, 0.01, 0.05, 0.5, 0.99];

/// The three deposits and what the third places at one position.
///
/// The first table is the deposit parameters as the table states them. The
/// second is [`crate::world::place`] read at every combination of a bound entry
/// and the draws above against the shipped substitution chance, which is what
/// makes the predicate's two conditions visible as data rather than as prose.
#[must_use]
pub fn world_document() -> String {
    let mut document = String::new();
    document.push_str(
        "deposit\tsize\ttargets\tcount\tlowest\thighest\tbiomes\tstage\town_feature\tair_discard\n",
    );
    for deposit in crate::world::DEPOSITS {
        let biomes = match deposit.biomes {
            Where::Overworld => "overworld",
            Where::Nether => "nether",
        };
        let stage = match deposit.stage {
            Stage::Raw => "raw",
            Stage::UndergroundOres => "underground_ores",
        };
        writeln!(
            document,
            "{}\t{}\t{}\t{}\t{}\t{}\t{biomes}\t{stage}\t{}\t{}",
            deposit.name,
            deposit.size,
            deposit.targets,
            deposit.count,
            deposit.lowest,
            deposit.highest,
            deposit.own_feature,
            crate::world::AIR_DISCARD,
        )
        .expect("writing to a String cannot fail");
    }
    document.push('\n');
    document.push_str("bound\tchance\tdraw\tplaced\n");
    let chance = 0.05;
    for bound in [false, true] {
        for draw in SUBSTITUTION_DRAWS {
            let placed = match crate::world::place(bound, chance, draw) {
                Placed::Target => "target",
                Placed::Rare => "rare",
            };
            writeln!(document, "{bound}\t{chance}\t{draw}\t{placed}")
                .expect("writing to a String cannot fail");
        }
    }
    document
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_generated_documents_regenerate_byte_for_byte() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let regenerating = std::env::var_os("RAVEL_REGENERATE_GENERATED").is_some();
        for (relative, rendered) in documents() {
            assert!(
                rendered.ends_with('\n'),
                "{relative} does not end with a newline"
            );
            let path = root.join(relative);
            if regenerating {
                let parent = path.parent().expect("a document sits inside a directory");
                std::fs::create_dir_all(parent).expect("the directory is writable");
                std::fs::write(&path, rendered.as_bytes()).expect("the document is writable");
                continue;
            }
            let on_disk = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{relative} is missing from the package: {error}"));
            assert_eq!(
                on_disk, rendered,
                "{relative} on disk is not what the crate derives"
            );
        }
    }
}
