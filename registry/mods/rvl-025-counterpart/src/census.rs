//! What this entry registers, what ships beside it, and what it deliberately
//! does not claim.
//!
//! §4 counts the registrations twice and §5 counts the documents, and both
//! counts are carried here as figures a test can check rather than as prose. Two
//! registration groups are claimed and left empty upstream — sounds and entities
//! — and this counterpart claims neither, because a claimed empty group is a
//! name nobody can use and a conflict somebody else could have had.
//!
//! §5's five shipped-document defects are each a decision rather than a copy,
//! and [`Defect`] states the decision beside the defect.

use ravel_sdk::ModId;
use ravel_sdk::content::{ContentDeclaration, ContentKind, DeclarationError, RegistryKey};

/// The seventeen items, in the order this package registers them.
///
/// The filled carrier is the second appearance of the handheld carrier: a
/// display variant reached only by filling one, deliberately outside the
/// creative grouping so a player cannot obtain it directly.
pub const ITEMS: [&str; 17] = [
    "raw_material",
    "refined_material",
    "material_plate",
    "material_rod",
    "reinforced_frame",
    "blank_circuit",
    "focusing_lens",
    "survey_probe",
    "binding_paste",
    "shaping_die",
    "match_item",
    "pattern_card",
    "carrier",
    "filled_carrier",
    "configuration_tool",
    "selecting_configuration_tool",
    "guidebook",
];

/// The item that is registered but kept out of the creative grouping.
pub const OUTSIDE_GROUPING: &str = "filled_carrier";

/// The five blocks, in the order this package registers them.
pub const BLOCKS: [&str; 5] = [
    "infusing_machine",
    "readout_block",
    "surface_ore",
    "sweltering_ore",
    "outer_ore",
];

/// The two items §16 names as the dependents' crafting intermediates.
pub const INTERMEDIATES: [&str; 2] = ["material_plate", "reinforced_frame"];

/// The items that have no recipe at all.
pub const NO_RECIPE_ITEMS: [&str; 2] = ["selecting_configuration_tool", "filled_carrier"];

/// The blocks that have no recipe, because they are mined.
pub const NO_RECIPE_BLOCKS: [&str; 3] = ["surface_ore", "sweltering_ore", "outer_ore"];

/// How many shaped crafting recipes ship.
pub const RECIPES: usize = 12;

/// How many unlock advancements ship, one per recipe.
pub const ADVANCEMENTS: usize = 12;

/// How many block loot tables ship.
pub const LOOT_TABLES: usize = 5;

/// How many tag documents ship.
pub const TAG_DOCUMENTS: usize = 9;

/// How many block-state documents ship.
pub const BLOCK_STATE_DOCUMENTS: usize = 7;

/// How many model documents ship.
pub const MODEL_DOCUMENTS: usize = 32;

/// How many language entries ship.
pub const LANGUAGE_ENTRIES: usize = 55;

/// How many documents the generator produces.
///
/// The recipes, the advancements, the loot tables and the tag documents, plus
/// the loot-modifier document of the fourth defect.
pub const GENERATED_DOCUMENTS: usize = RECIPES + ADVANCEMENTS + LOOT_TABLES + TAG_DOCUMENTS + 1;

/// How many hand-written data documents ship beside the generated ones.
///
/// Four attach the ore to its biomes, four describe its shape, four place it,
/// and one declares the guidebook. The guidebook declaration is also one of
/// §17's seventeen guidebook documents, so the two figures overlap by exactly
/// one and must not be added together.
pub const HAND_WRITTEN_DOCUMENTS: usize = 13;

/// How many block entity kinds are registered.
pub const BLOCK_ENTITY_KINDS: usize = 2;

/// How many menu kinds are registered.
pub const MENU_KINDS: usize = 4;

/// How many item data component kinds are registered.
pub const COMPONENT_KINDS: usize = 5;

/// How many per-owner data attachments are registered.
pub const ATTACHMENTS: usize = 1;

/// How many world-placement modifier kinds are registered.
pub const PLACEMENT_MODIFIER_KINDS: usize = 2;

/// How many creative groupings are registered.
pub const CREATIVE_GROUPINGS: usize = 1;

/// The two registration groups the original claims and leaves empty.
///
/// This counterpart claims neither.
pub const CLAIMED_AND_EMPTY: [&str; 2] = ["sounds", "entities"];

/// One of §5's five defects in the shipped documents, and what was done about it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Defect {
    /// Six item labels and one block label name content that is not registered.
    SurplusLabels,
    /// Two block-state documents name blocks that are not registered.
    UnregisteredBlockStates,
    /// Two key-binding labels match bindings that exist only as comments.
    DeadBindingLabels,
    /// The loot-modifier document declares no entry and does not replace.
    EmptyLootModifier,
    /// One of the four ore shape documents is named by nothing.
    UnreferencedOreShape,
}

impl Defect {
    /// All five, in the order §5 lists them.
    pub const ALL: [Self; 5] = [
        Self::SurplusLabels,
        Self::UnregisteredBlockStates,
        Self::DeadBindingLabels,
        Self::EmptyLootModifier,
        Self::UnreferencedOreShape,
    ];

    /// Returns whether this counterpart ships the defect.
    ///
    /// Four are simply not shipped: a label for content that does not exist, a
    /// block-state document for a block that is not registered, a label for a
    /// binding that is not registered, and a document with no effect are all
    /// files with nothing to do. The fifth is kept, because it is not a file
    /// this package writes: it is a number the placement table records, and
    /// [`crate::world::Variant::declared_vein_size`] is where the divergence
    /// between the declared and effective figures stays visible.
    #[must_use]
    pub const fn shipped(self) -> bool {
        matches!(self, Self::UnreferencedOreShape)
    }
}

/// Builds this entry's content declaration.
///
/// Ravel's registry has four kinds and this entry claims all four: the blocks,
/// the items, the recipes and the two commands of §14. Claiming the command
/// names is what routes a typed command to this entry, so a command declared and
/// not claimed is one the player can never run. Block entity kinds, menus,
/// component kinds, attachments, placement modifiers and the creative grouping
/// have no key to claim and are carried as the counts above.
///
/// # Errors
///
/// Returns [`DeclarationError`] when a name this package spells is not a legal
/// content path or is declared twice, both of which are packaging failures for
/// constants.
pub fn declaration(owner: ModId) -> Result<ContentDeclaration, DeclarationError> {
    let mut declaration = ContentDeclaration::new(owner);
    for block in BLOCKS {
        declaration.entry(ContentKind::Block, block)?;
    }
    for item in ITEMS {
        declaration.entry(ContentKind::Item, item)?;
    }
    for index in 0..RECIPES {
        declaration.entry(ContentKind::Recipe, &format!("recipe_{index}"))?;
    }
    for command in crate::commands::NAMES {
        declaration.entry(ContentKind::Command, command)?;
    }
    Ok(declaration)
}

/// Returns the entries the creative grouping holds.
///
/// Sixteen of the seventeen items, which is every one except the filled carrier.
#[must_use]
pub fn creative_grouping() -> Vec<&'static str> {
    ITEMS
        .into_iter()
        .filter(|item| *item != OUTSIDE_GROUPING)
        .collect()
}

/// Returns the keys the declaration claims, for a caller that wants the count.
///
/// # Errors
///
/// Returns whatever [`declaration`] refuses.
pub fn claimed(owner: ModId) -> Result<Vec<RegistryKey>, DeclarationError> {
    Ok(declaration(owner)?.entries().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> ModId {
        crate::MOD_ID
            .parse()
            .expect("the packaged identity is legal")
    }

    #[test]
    fn the_registration_census_is_the_one_the_specification_counts() {
        assert_eq!(ITEMS.len(), 17);
        assert_eq!(BLOCKS.len(), 5);
    }

    #[test]
    fn every_item_and_every_block_is_named_once() {
        let mut names: Vec<&str> = ITEMS.into_iter().chain(BLOCKS).collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), before);
    }

    #[test]
    fn the_creative_grouping_holds_sixteen_of_the_seventeen_items() {
        let grouped = creative_grouping();
        assert_eq!(grouped.len(), 16);
        assert!(
            !grouped.contains(&OUTSIDE_GROUPING),
            "the display variant is deliberately unobtainable"
        );
        assert!(ITEMS.contains(&OUTSIDE_GROUPING));
    }

    #[test]
    fn the_two_empty_registration_groups_are_claimed_by_nothing_here() {
        assert_eq!(CLAIMED_AND_EMPTY.len(), 2);
        let keys = claimed(owner()).expect("the packaged names are legal");
        for empty in CLAIMED_AND_EMPTY {
            assert!(
                !keys.iter().any(|key| key.id().path() == empty),
                "a claimed empty group is a name nobody can use"
            );
        }
    }

    #[test]
    fn the_declaration_claims_every_block_item_recipe_and_command() {
        let keys = claimed(owner()).expect("the packaged names are legal");
        assert_eq!(
            keys.len(),
            BLOCKS.len() + ITEMS.len() + RECIPES + crate::commands::NAMES.len()
        );

        let of = |kind: ContentKind| keys.iter().filter(|key| key.kind() == kind).count();
        assert_eq!(of(ContentKind::Block), 5);
        assert_eq!(of(ContentKind::Item), 17);
        assert_eq!(of(ContentKind::Recipe), 12);
        assert_eq!(
            of(ContentKind::Command),
            2,
            "a command name nothing claims is one the loader routes to nobody"
        );
    }

    #[test]
    fn every_recipe_has_one_unlock_advancement() {
        assert_eq!(RECIPES, ADVANCEMENTS);
    }

    #[test]
    fn the_generated_document_count_is_the_four_kinds_plus_the_loot_modifier() {
        assert_eq!(
            GENERATED_DOCUMENTS - 1,
            RECIPES + ADVANCEMENTS + LOOT_TABLES + TAG_DOCUMENTS
        );
    }

    #[test]
    fn the_recipe_less_registrations_are_the_ones_the_specification_names() {
        for item in NO_RECIPE_ITEMS {
            assert!(ITEMS.contains(&item));
        }
        for block in NO_RECIPE_BLOCKS {
            assert!(BLOCKS.contains(&block));
        }
        assert!(
            RECIPES < ITEMS.len() - NO_RECIPE_ITEMS.len() + BLOCKS.len() - NO_RECIPE_BLOCKS.len(),
            "the twelve recipes do not cover every remaining registration, and \
             the specification names only the five that certainly have none"
        );
    }

    #[test]
    fn four_of_the_five_document_defects_are_not_shipped_and_one_is_kept() {
        let shipped: Vec<Defect> = Defect::ALL
            .into_iter()
            .filter(|defect| defect.shipped())
            .collect();
        assert_eq!(
            shipped,
            [Defect::UnreferencedOreShape],
            "the one that is kept is a number in a table rather than a file"
        );
        assert_eq!(Defect::ALL.len(), 5);
    }

    #[test]
    fn both_crafting_intermediates_are_registered_items() {
        assert_eq!(INTERMEDIATES.len(), 2);
        for intermediate in INTERMEDIATES {
            assert!(ITEMS.contains(&intermediate));
        }
    }
}
