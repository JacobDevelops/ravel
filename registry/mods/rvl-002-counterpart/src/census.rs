//! The registered content, counted and ordered.
//!
//! Six registries. Almost all of this is declared data, and the counts are the
//! part tests observe: `registry-parity` compares membership and, for the block
//! items, order.
//!
//! One thing here is not data. The block items are registered before any other
//! item, so they occupy the earliest positions in this namespace's item
//! registry, and reproducing that is what a parity run comparing order needs.

use crate::crops;

/// Blocks this entry registers.
pub const BLOCKS: usize = 26;

/// Blocks that carry a block item, which is every block but the six fluid ones.
pub const BLOCK_ITEMS: usize = 20;

/// Items this entry registers, the twenty block items included.
pub const ITEMS: usize = 53;

/// Fluid families, each a fluid type, a still and a flowing fluid, a block and a
/// bucket.
pub const FLUID_FAMILIES: usize = 6;

/// Registrations one fluid family makes.
pub const PER_FLUID_FAMILY: usize = 5;

/// Registrations the six fluid families make in total.
pub const FLUID_REGISTRATIONS: usize = FLUID_FAMILIES * PER_FLUID_FAMILY;

/// One group of blocks, with the hardness and blast resistance it declares.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockGroup {
    /// What the group is.
    pub name: &'static str,
    /// How many blocks it holds.
    pub count: usize,
    /// Hardness.
    pub hardness: f64,
    /// Blast resistance.
    pub resistance: f64,
    /// Whether the blocks carry a block item.
    pub block_item: bool,
}

/// Every block group, in the order the block registry receives them.
pub const BLOCK_GROUPS: [BlockGroup; 7] = [
    BlockGroup {
        name: "material_storage",
        count: 1,
        hardness: 4.0,
        resistance: 6.0,
        block_item: true,
    },
    BlockGroup {
        name: "metallic_storage",
        count: 2,
        hardness: 5.0,
        resistance: 6.0,
        block_item: true,
    },
    BlockGroup {
        name: "ground",
        count: 1,
        hardness: 0.6,
        resistance: 0.6,
        block_item: true,
    },
    BlockGroup {
        name: "fuel_storage",
        count: 6,
        hardness: 5.0,
        resistance: 6.0,
        block_item: true,
    },
    BlockGroup {
        name: "ore",
        count: 4,
        hardness: 3.0,
        resistance: 3.0,
        block_item: true,
    },
    BlockGroup {
        name: "anchor",
        count: 6,
        hardness: 5.0,
        resistance: 10.0,
        block_item: true,
    },
    BlockGroup {
        name: "fluid",
        count: 6,
        hardness: 100.0,
        resistance: 100.0,
        block_item: false,
    },
];

/// The thirty-three items that are not block items.
pub const PLAIN_ITEMS: usize = ITEMS - BLOCK_ITEMS;

/// One section of the creative listing, in the fixed hand-written order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Listed {
    /// The block items, other than the anchors.
    Blocks,
    /// The six anchor blocks, in crop order, each guarded by its crop's state.
    Anchors,
    /// The plain items.
    Items,
    /// The six tools.
    Tools,
    /// The six buckets.
    Buckets,
}

impl Listed {
    /// The stable spelling used by generated documents.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blocks => "Blocks",
            Self::Anchors => "Anchors",
            Self::Items => "Items",
            Self::Tools => "Tools",
            Self::Buckets => "Buckets",
        }
    }
}

/// The listing's fixed order.
pub const LISTING: [Listed; 5] = [
    Listed::Blocks,
    Listed::Anchors,
    Listed::Items,
    Listed::Tools,
    Listed::Buckets,
];

/// Whether a listed section is guarded by anything.
///
/// Only the anchors are. A flag-disabled item drops out of the listing without
/// disturbing the rest, which is the shared library's flag-aware generator doing
/// its work rather than a rule of this entry's.
#[must_use]
pub const fn guarded(section: Listed) -> bool {
    matches!(section, Listed::Anchors)
}

/// The tint index a bucket's fluid colour is applied at.
pub const BUCKET_TINT_INDEX: u32 = 1;

/// Every block path, in the order the block registry receives them.
///
/// A group of one takes the group's own name; a group of several numbers its
/// members from one. The anchors take their crops' declared names, so a crop and
/// its anchor cannot drift apart.
#[must_use]
pub fn block_paths() -> Vec<String> {
    let mut paths = Vec::with_capacity(BLOCKS);
    for group in BLOCK_GROUPS {
        if group.name == "anchor" {
            paths.extend(
                crops::DECLARED
                    .iter()
                    .map(|declared| declared.anchor.to_owned()),
            );
        } else if group.count == 1 {
            paths.push(group.name.to_owned());
        } else {
            paths.extend((1..=group.count).map(|index| format!("{}_{index}", group.name)));
        }
    }
    paths
}

/// Which block paths carry a block item, in block registry order.
#[must_use]
pub fn block_item_paths() -> Vec<String> {
    let mut paths = Vec::with_capacity(BLOCK_ITEMS);
    let mut all = block_paths().into_iter();
    for group in BLOCK_GROUPS {
        for path in all.by_ref().take(group.count) {
            if group.block_item {
                paths.push(path);
            }
        }
    }
    paths
}

/// The block the ground group registers.
pub const GROUND_BLOCK: &str = "ground";

/// The block the ingot's storage form is registered at.
pub const INGOT_BLOCK: &str = "metallic_storage_1";

/// The block the gemstone's storage form is registered at.
pub const GEMSTONE_BLOCK: &str = "metallic_storage_2";

/// The three items the conventional material tags name, in registry order.
pub const MATERIAL_ITEMS: [&str; 3] = ["ingot", "nugget", "gemstone"];

fn paths_in(groups: &[&str]) -> Vec<String> {
    let mut paths = Vec::new();
    let mut all = block_paths().into_iter();
    for group in BLOCK_GROUPS {
        for path in all.by_ref().take(group.count) {
            if groups.contains(&group.name) {
                paths.push(path);
            }
        }
    }
    paths
}

/// The nine storage blocks, in block registry order.
#[must_use]
pub fn storage_block_paths() -> Vec<String> {
    paths_in(&["material_storage", "metallic_storage", "fuel_storage"])
}

/// The four ore blocks, in the order [`crate::world::DEPOSITS`] declares them.
#[must_use]
pub fn ore_block_paths() -> Vec<String> {
    paths_in(&["ore"])
}

/// The six anchor blocks, in crop order.
#[must_use]
pub fn anchor_block_paths() -> Vec<String> {
    paths_in(&["anchor"])
}

/// The six tool items, in item registry order.
#[must_use]
pub fn tool_item_paths() -> Vec<String> {
    plain_item_paths()
        .into_iter()
        .filter(|path| path.starts_with("tool_"))
        .collect()
}

/// The thirty-three item paths that are not block items, in registry order.
#[must_use]
pub fn plain_item_paths() -> Vec<String> {
    let mut paths = vec![
        "material".to_owned(),
        "ingot".to_owned(),
        "nugget".to_owned(),
        "gemstone".to_owned(),
        "shard".to_owned(),
        "soul".to_owned(),
        "fragment".to_owned(),
        "scale".to_owned(),
        "decorative".to_owned(),
    ];
    paths.extend((1..=crate::fuel::TIERS).map(|tier| format!("fuel_{tier}")));
    paths.extend((1..=crate::fuel::TIERS).map(|tier| format!("food_{tier}")));
    paths.extend((1..=crate::fuel::TIERS).map(|grade| format!("tool_{grade}")));
    paths.extend((1..=FLUID_FAMILIES).map(|index| format!("bucket_{index}")));
    paths
}

/// Every item path, in the order the item registry receives them.
///
/// The block items come first, in the block registry's own insertion order, so
/// they occupy the earliest positions in this namespace's item registry — which
/// a parity run observes only if it compares order and not just membership.
#[must_use]
pub fn item_paths() -> Vec<String> {
    let mut paths = block_item_paths();
    paths.extend(plain_item_paths());
    paths
}

/// The percentage a hover line renders a fraction as.
///
/// The original multiplies the stored fraction by a hundred and concatenates a
/// percent sign, so a default of 0.35 renders as a seventeen-digit expansion.
/// That is a display defect rather than a rounding choice, so this formats the
/// value instead, and a pixel-level presentation test should expect the
/// disagreement.
#[must_use]
pub fn chance_text(fraction: f64) -> String {
    let percent = fraction * 100.0;
    let rounded = (percent * 100.0).round() / 100.0;
    if (rounded - rounded.trunc()).abs() < f64::EPSILON {
        format!("{}%", rounded.trunc())
    } else {
        format!("{rounded}%")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_named_block_groups_are_the_ones_the_tag_census_counts() {
        assert_eq!(storage_block_paths().len(), 9);
        assert_eq!(ore_block_paths().len(), crate::world::DEPOSITS.len());
        assert_eq!(anchor_block_paths().len(), crops::COUNT);
        assert_eq!(tool_item_paths().len(), crate::tools::COUNT);

        let blocks = block_paths();
        for path in storage_block_paths()
            .into_iter()
            .chain(ore_block_paths())
            .chain(anchor_block_paths())
            .chain([
                GROUND_BLOCK.to_owned(),
                INGOT_BLOCK.to_owned(),
                GEMSTONE_BLOCK.to_owned(),
            ])
        {
            assert!(blocks.contains(&path), "{path} is a registered block");
        }
        for path in MATERIAL_ITEMS {
            assert!(plain_item_paths().contains(&path.to_owned()));
        }
        assert_ne!(INGOT_BLOCK, GEMSTONE_BLOCK);
    }

    #[test]
    fn the_block_groups_add_up_to_the_declared_block_count() {
        let total: usize = BLOCK_GROUPS.iter().map(|group| group.count).sum();
        assert_eq!(total, BLOCKS);

        let with_items: usize = BLOCK_GROUPS
            .iter()
            .filter(|group| group.block_item)
            .map(|group| group.count)
            .sum();
        assert_eq!(with_items, BLOCK_ITEMS);
        assert_eq!(BLOCKS - BLOCK_ITEMS, FLUID_FAMILIES);
    }

    #[test]
    fn the_declared_hardness_and_resistance_of_each_group_is_what_the_table_states() {
        let by_name = |name: &str| {
            BLOCK_GROUPS
                .iter()
                .find(|group| group.name == name)
                .copied()
                .expect("a declared group")
        };
        let material = by_name("material_storage");
        assert_eq!((material.hardness, material.resistance), (4.0, 6.0));
        let metallic = by_name("metallic_storage");
        assert_eq!((metallic.hardness, metallic.resistance), (5.0, 6.0));
        assert_eq!(metallic.count, 2);
        let fuel = by_name("fuel_storage");
        assert_eq!((fuel.hardness, fuel.resistance), (5.0, 6.0));
        assert_eq!(
            fuel.count,
            usize::try_from(crate::fuel::TIERS).expect("the six fuel tiers fit usize")
        );
        let ore = by_name("ore");
        assert_eq!((ore.hardness, ore.resistance), (3.0, 3.0));
        assert_eq!(ore.count, crate::world::DEPOSITS.len());
        let anchor = by_name("anchor");
        assert_eq!((anchor.hardness, anchor.resistance), (5.0, 10.0));
        assert_eq!(anchor.count, crops::COUNT);
    }

    #[test]
    fn the_three_storage_blocks_are_one_material_block_and_two_metallic_ones() {
        let storage: usize = BLOCK_GROUPS
            .iter()
            .filter(|group| group.name.ends_with("_storage") && group.name != "fuel_storage")
            .map(|group| group.count)
            .sum();
        assert_eq!(storage, 3);
    }

    #[test]
    fn the_item_count_is_twenty_block_items_and_thirty_three_others() {
        assert_eq!(BLOCK_ITEMS + PLAIN_ITEMS, ITEMS);

        // The thirty-three, named by group: the tier material, an ingot, its
        // nugget, a gemstone, a shard, the soul, a fragment, a scale, a
        // decorative item, six fuels, six foods, six tools and six buckets.
        let named = 9
            + usize::try_from(crate::fuel::TIERS).expect("the six fuel tiers fit usize")
            + crate::foods::COUNT
            + crate::tools::COUNT
            + FLUID_FAMILIES;
        assert_eq!(named, PLAIN_ITEMS);
    }

    #[test]
    fn the_six_fluid_families_make_thirty_registrations_across_four_registries() {
        // Fluid type, block and bucket are one each; the still and the flowing
        // members share the fluid registry.
        let per_family = 1 + 2 + 1 + 1;
        assert_eq!(per_family, PER_FLUID_FAMILY);
        assert_eq!(FLUID_FAMILIES * 2, 12, "still and flowing, twelve fluids");
    }

    #[test]
    fn only_the_anchors_in_the_listing_are_guarded() {
        for section in LISTING {
            assert_eq!(guarded(section), section == Listed::Anchors);
        }
    }

    #[test]
    fn the_paths_are_as_many_as_the_census_counts_and_every_one_is_distinct() {
        let blocks = block_paths();
        assert_eq!(blocks.len(), BLOCKS);
        let mut sorted = blocks;
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), BLOCKS);

        let items = item_paths();
        assert_eq!(items.len(), ITEMS);
        let mut sorted = items;
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ITEMS);
    }

    #[test]
    fn the_block_items_occupy_the_earliest_positions_in_the_item_registry() {
        let block_items = block_item_paths();
        assert_eq!(block_items.len(), BLOCK_ITEMS);
        let items = item_paths();
        assert_eq!(&items[..BLOCK_ITEMS], block_items.as_slice());
        assert_eq!(&items[BLOCK_ITEMS..], plain_item_paths().as_slice());
    }

    #[test]
    fn the_block_items_follow_the_block_registrys_own_insertion_order() {
        let blocks = block_paths();
        let block_items = block_item_paths();
        let mut positions: Vec<usize> = block_items
            .iter()
            .map(|path| {
                blocks
                    .iter()
                    .position(|held| held == path)
                    .expect("a block item names a block")
            })
            .collect();
        let ordered = positions.clone();
        positions.sort_unstable();
        assert_eq!(positions, ordered, "insertion order, not sorted order");
    }

    #[test]
    fn the_six_fluid_blocks_carry_no_block_item() {
        let block_items = block_item_paths();
        for index in 1..=FLUID_FAMILIES {
            assert!(!block_items.contains(&format!("fluid_{index}")));
        }
    }

    #[test]
    fn every_anchor_block_is_named_by_the_crop_it_belongs_to() {
        let blocks = block_paths();
        for declared in crops::DECLARED {
            assert!(blocks.contains(&declared.anchor.to_owned()));
        }
    }

    #[test]
    fn a_percentage_is_formatted_rather_than_concatenated() {
        assert_eq!(chance_text(0.35), "35%");
        assert_eq!(chance_text(0.0), "0%");
        assert_eq!(chance_text(1.0), "100%");
        assert_eq!(chance_text(0.125), "12.5%");
        assert!(
            !chance_text(0.35).contains("000000"),
            "the original renders a seventeen-digit expansion here"
        );
    }
}
