//! Recipes shipped as data: the census, the conditions, and the two counts that
//! differ.
//!
//! This is the largest part of the entry, and the part a parity run trips over
//! first. Two numbers matter and they are not the same number. The original
//! *ships* one hundred and fifty-seven documents; it *loads* eighty, because
//! seventy-seven of them are of kinds a third-party mod would have registered
//! and that mod is absent, and none of the seventy-seven carries a condition
//! that would suppress it.
//!
//! A counterpart ships neither those documents nor the integration, so the
//! expectation is written against the loaded set.

use rvl_321_counterpart::settings as depended;

/// A group of recipe documents this entry ships, and who owns the kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Group {
    /// What the kind is called.
    pub kind: &'static str,
    /// Who owns it.
    pub owner: Owner,
    /// How many documents this entry ships of it.
    pub shipped: usize,
}

/// Who owns a recipe kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Owner {
    /// The host's own crafting grid.
    Host,
    /// The shared library.
    Shared,
    /// The tiered-crop entry.
    Tiered,
    /// A third party this entry does not depend on and does not carry.
    Absent,
}

/// Every group, in the order the census lists them.
pub const GROUPS: [Group; 8] = [
    Group {
        kind: "shaped",
        owner: Owner::Host,
        shipped: 36,
    },
    Group {
        kind: "shapeless",
        owner: Owner::Host,
        shipped: 22,
    },
    Group {
        kind: "shaped_unmirrored",
        owner: Owner::Shared,
        shipped: 8,
    },
    Group {
        kind: "tilling",
        owner: Owner::Tiered,
        shipped: 1,
    },
    Group {
        kind: "ritual_four_station",
        owner: Owner::Tiered,
        shipped: 1,
    },
    Group {
        kind: "ritual_eight_station",
        owner: Owner::Tiered,
        shipped: 6,
    },
    Group {
        kind: "reprocessing",
        owner: Owner::Tiered,
        shipped: 6,
    },
    Group {
        kind: "forging",
        owner: Owner::Absent,
        shipped: 77,
    },
];

/// Documents the original ships.
#[must_use]
pub fn shipped() -> usize {
    GROUPS.iter().map(|group| group.shipped).sum()
}

/// Documents that become a recipe when the third-party mod is absent.
///
/// A document whose kind nothing registered is discarded, and none of the
/// seventy-seven carries a mod-loaded condition that would have suppressed it
/// first. So the difference between the two counts is exactly that group.
#[must_use]
pub fn loaded() -> usize {
    GROUPS
        .iter()
        .filter(|group| group.owner != Owner::Absent)
        .map(|group| group.shipped)
        .sum()
}

/// Documents shipped alongside the absent kinds that are not recipes at all.
///
/// Material definitions, statistics and traits sit beside the seventy-seven, and
/// six further asset documents describe material appearance. All are inert.
pub const INERT_ALONGSIDE: usize = 18;

/// A condition a shipped document carries.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Condition {
    /// The crop is enabled.
    CropEnabled,
    /// The crop's crafting material resolves to something.
    CropHasMaterial,
    /// The dependency's plain-grid seed setting is on, which it is not by
    /// default.
    GridSeedsEnabled,
    /// A named companion mod is loaded.
    CompanionLoaded,
}

/// The three kinds the crop opt-out ships documents for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CropRecipe {
    /// The fixed three-by-three grid pattern.
    Crafting,
    /// The eight-ingredient alternating infusion.
    Infusion,
    /// The one-to-two reprocessing conversion.
    Reprocessing,
}

/// Every crop recipe kind, in the order the documents are counted.
pub const CROP_RECIPES: [CropRecipe; 3] = [
    CropRecipe::Crafting,
    CropRecipe::Infusion,
    CropRecipe::Reprocessing,
];

/// The conditions one crop document carries.
///
/// All three kinds carry the crop-enabled condition. Crafting and infusion carry
/// the material condition on top, which is what implements the third state: an
/// enabled crop whose tag resolves empty loses those two and keeps reprocessing.
/// Crafting carries a third, the dependency's grid-seed setting, which defaults
/// off, so in a default installation the six crafting documents load no recipe
/// at all.
#[must_use]
pub fn conditions(recipe: CropRecipe) -> Vec<Condition> {
    match recipe {
        CropRecipe::Crafting => vec![
            Condition::CropEnabled,
            Condition::CropHasMaterial,
            Condition::GridSeedsEnabled,
        ],
        CropRecipe::Infusion => vec![Condition::CropEnabled, Condition::CropHasMaterial],
        CropRecipe::Reprocessing => vec![Condition::CropEnabled],
    }
}

/// Whether a crop document contributes a recipe.
#[must_use]
pub fn contributes(
    recipe: CropRecipe,
    enabled: bool,
    has_material: bool,
    grid_seeds: bool,
) -> bool {
    conditions(recipe)
        .into_iter()
        .all(|condition| match condition {
            Condition::CropEnabled => enabled,
            Condition::CropHasMaterial => has_material,
            Condition::GridSeedsEnabled => grid_seeds,
            Condition::CompanionLoaded => true,
        })
}

/// The dependency setting the crafting documents are gated on, which ships off.
#[must_use]
pub fn grid_seed_setting_default() -> bool {
    depended::Settings::default().recipes_plain_grid_seeds
}

/// The count a decompression yields, which is nine for every member but one.
///
/// The tier material yields four of a *lower* tier's material instead: the one
/// member of the family that is a progression step rather than a storage
/// conversion, and the recipe by which the sixth tier is entered at all.
#[must_use]
pub const fn decompressed(progression_step: bool) -> u64 {
    if progression_step { 4 } else { 9 }
}

/// The awakening recipe: the grade-six tool from the grade-five one.
///
/// Four elemental essences at forty each and four ingredients, and it declares
/// component transfer, so the input tool's slotted modifiers and its damage
/// carry onto the output. That is the only place in this entry where a recipe
/// preserves item-level data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Awakening {
    /// Essences consumed, by kind.
    pub essences: u32,
    /// How many of each.
    pub each: u64,
    /// Further ingredients.
    pub ingredients: u32,
    /// Whether item-level data carries onto the output.
    pub transfers_components: bool,
}

/// The single awakening this entry ships.
pub const AWAKENING: Awakening = Awakening {
    essences: 4,
    each: 40,
    ingredients: 4,
    transfers_components: true,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crops;

    #[test]
    fn the_shipped_count_and_the_loaded_count_differ_by_exactly_the_absent_group() {
        assert_eq!(shipped(), 157);
        assert_eq!(loaded(), 80);
        assert_eq!(shipped() - loaded(), 77);
        let absent: usize = GROUPS
            .iter()
            .filter(|group| group.owner == Owner::Absent)
            .map(|group| group.shipped)
            .sum();
        assert_eq!(absent, 77);
    }

    #[test]
    fn this_counterpart_carries_the_loaded_set_and_none_of_the_absent_one() {
        assert_eq!(loaded(), 80);
        assert_ne!(loaded(), shipped());
        assert!(
            GROUPS
                .iter()
                .filter(|group| group.owner == Owner::Absent)
                .all(|group| group.shipped > 0),
            "the absent group is declared rather than dropped from the census"
        );
    }

    #[test]
    fn the_shared_librarys_kind_is_the_only_one_of_its_this_entry_uses() {
        let shared: Vec<&Group> = GROUPS
            .iter()
            .filter(|group| group.owner == Owner::Shared)
            .collect();
        assert_eq!(shared.len(), 1);
        assert_eq!(shared[0].shipped, 8);
        assert!(
            rvl_042_counterpart::RECIPE_KINDS.contains(&shared[0].kind),
            "the kind is one the shared library names"
        );
    }

    #[test]
    fn every_tiered_kind_this_entry_ships_data_for_is_one_that_entry_declares() {
        for group in GROUPS.iter().filter(|held| held.owner == Owner::Tiered) {
            assert!(
                rvl_321_counterpart::RECIPE_KINDS.contains(&group.kind),
                "{} is a kind the dependency claims",
                group.kind
            );
        }
    }

    #[test]
    fn crop_documents_replace_the_derivation_the_extension_switched_off() {
        for declared in crops::DECLARED {
            let derived = declared
                .crop(
                    format!("test:{}", declared.path)
                        .parse()
                        .expect("a legal crop identity"),
                    &"test:6".parse().expect("a legal tier identity"),
                    "test:material".parse().expect("a legal material identity"),
                )
                .derived;
            assert_eq!(derived, crops::OPT_OUT);
            assert!(!derived.ritual_seed);
            assert!(!derived.grid_seed);
            assert!(!derived.conversion);
        }
    }

    #[test]
    fn every_crop_document_carries_the_enabled_condition_and_only_two_carry_the_material_one() {
        for recipe in CROP_RECIPES {
            assert!(conditions(recipe).contains(&Condition::CropEnabled));
        }
        let with_material: Vec<CropRecipe> = CROP_RECIPES
            .into_iter()
            .filter(|recipe| conditions(*recipe).contains(&Condition::CropHasMaterial))
            .collect();
        assert_eq!(with_material, [CropRecipe::Crafting, CropRecipe::Infusion]);
    }

    #[test]
    fn only_the_crafting_documents_carry_the_third_condition_and_it_ships_off() {
        let gated: Vec<CropRecipe> = CROP_RECIPES
            .into_iter()
            .filter(|recipe| conditions(*recipe).contains(&Condition::GridSeedsEnabled))
            .collect();
        assert_eq!(gated, [CropRecipe::Crafting]);
        assert!(
            !grid_seed_setting_default(),
            "a default installation loads no crafting recipe of the six"
        );
    }

    #[test]
    fn an_enabled_crop_with_an_empty_material_keeps_reprocessing_and_loses_the_other_two() {
        assert!(!contributes(CropRecipe::Crafting, true, false, true));
        assert!(!contributes(CropRecipe::Infusion, true, false, true));
        assert!(contributes(CropRecipe::Reprocessing, true, false, true));
    }

    #[test]
    fn an_unobtainable_crop_contributes_no_recipe_of_any_kind() {
        for recipe in CROP_RECIPES {
            assert!(!contributes(recipe, false, true, true));
        }
    }

    #[test]
    fn a_default_installation_loads_infusion_and_reprocessing_and_no_crafting() {
        let grid_seeds = grid_seed_setting_default();
        assert!(!contributes(CropRecipe::Crafting, true, true, grid_seeds));
        assert!(contributes(CropRecipe::Infusion, true, true, grid_seeds));
        assert!(contributes(
            CropRecipe::Reprocessing,
            true,
            true,
            grid_seeds
        ));
    }

    #[test]
    fn the_compression_family_agrees_with_the_fuel_arithmetic() {
        assert_eq!(decompressed(false), crate::fuel::COMPRESSION);
        assert_eq!(
            decompressed(true),
            4,
            "the tier material is a progression step, not a storage conversion"
        );
    }

    #[test]
    fn the_four_station_ritual_group_contains_one_awakening_recipe() {
        let awakenings: usize = GROUPS
            .iter()
            .filter(|group| group.kind == "ritual_four_station")
            .map(|group| group.shipped)
            .sum();
        assert_eq!(awakenings, 1);
    }

    #[test]
    fn the_inert_documents_beside_the_absent_kinds_are_counted_and_not_loaded() {
        assert_eq!(
            shipped() - loaded() + INERT_ALONGSIDE,
            95,
            "the ninety-five that would have fed an integration this entry does not carry"
        );
    }
}
