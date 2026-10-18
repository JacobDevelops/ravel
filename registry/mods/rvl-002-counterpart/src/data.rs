//! Loot, tags, translations, and what survives a save and a reload.
//!
//! The reload half is short because the answer is short: this entry owns no
//! world-persisted state of its own. No block entity, no saved data, no attached
//! data, no capability, no network payload. Everything it contributes to a world
//! is a block state the host persists, an item stack the host persists, or a
//! registry identifier, so the reload contract is entirely about identity.
//!
//! The one real hazard is inherited rather than introduced: a crop registered
//! *then disabled* keeps its identifiers when its companion mod goes away, and a
//! world saved with that mod present reloads without it, losing recipes and
//! nothing else. Registering conditionally instead would turn a recipe loss into
//! a world-corrupting identifier loss.

use ravel_sdk::ModError;
use ravel_sdk::content::{ContentDeclaration, ContentId, ContentKind, RegistryKey};
use ravel_sdk::tags::TagId;
use rvl_321_counterpart::registries::Role;

use crate::world::{Dimension, Ore};
use crate::{CONVENTION_NAMESPACE, HOST_NAMESPACE, MOD_ID, census, crops, world};

/// Block loot documents this entry ships, for twenty block items.
pub const LOOT_DOCUMENTS: usize = 19;

/// The shapes those documents take.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LootShape {
    /// Self-drop with a survives-explosion condition and nothing else.
    SelfDrop,
    /// Self under silk touch, otherwise a uniform count of another entry's
    /// material with the host's fortune formula and explosion decay applied in
    /// that order.
    OreAlternative,
    /// No document at all: the ground block, whose drops are computed in code.
    CodePath,
}

/// How many blocks each shape covers.
#[must_use]
pub const fn covered(shape: LootShape) -> usize {
    match shape {
        LootShape::SelfDrop => 15,
        LootShape::OreAlternative => 4,
        LootShape::CodePath => 1,
    }
}

/// Tag documents this entry ships.
pub const TAG_DOCUMENTS: usize = 48;

/// Storage-block tag documents: nine specific ones plus one aggregating
/// document, mirrored across the block and item registries.
pub const STORAGE_TAG_FACTS: usize = 10;

/// The documents those ten facts take, which is twice ten.
pub const STORAGE_TAG_DOCUMENTS: usize = STORAGE_TAG_FACTS * 2;

/// A tag document's registry and whether it has a counterpart in the other one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Mirrored {
    /// What the tag groups.
    pub about: &'static str,
    /// Facts it states.
    pub facts: usize,
    /// Whether the facts are mirrored across both registries.
    pub both_registries: bool,
}

/// The conventional tag families this entry contributes to.
///
/// Storage and ore mirror across the block and item registries; the material
/// tags name items only, because there is no block to mirror them onto.
pub const CONVENTIONAL: [Mirrored; 3] = [
    Mirrored {
        about: "storage",
        facts: STORAGE_TAG_FACTS,
        both_registries: true,
    },
    Mirrored {
        about: "material",
        facts: 6,
        both_registries: false,
    },
    Mirrored {
        about: "ore",
        facts: 5,
        both_registries: true,
    },
];

/// The host mineability and tier-split tags, which name blocks only.
pub const MINEABILITY_TAGS: usize = 4;

/// The catalogue tags of the tiered-crop entry this entry appends its crops to.
pub const TIERED_CATALOGUE_TAGS: usize = 3;

/// The one tag of this entry's own: the two-member optional tag.
pub const OWN_TAGS: usize = 1;

/// The four listings the six tools appear in.
pub const TOOL_TAG_LISTINGS: usize = 4;

/// How many blocks are declared mineable with a pickaxe.
pub const PICKAXE_MINEABLE: usize = 19;

/// How many blocks are declared mineable with a shovel, which is the ground.
pub const SHOVEL_MINEABLE: usize = 1;

/// Blocks placed at the iron mining tier.
pub const IRON_TIER: usize = 3;

/// Blocks placed at the stone mining tier.
pub const STONE_TIER: usize = 9;

/// Keys the reference language carries.
pub const REFERENCE_KEYS: usize = 104;

/// Keys each non-reference language carries, in the order they are counted.
pub const LANGUAGE_KEYS: [usize; 11] = [104, 104, 104, 104, 104, 98, 98, 105, 108, 39, 39];

/// Whether this entry owns any world-persisted state of its own.
pub const OWNS_PERSISTED_STATE: bool = false;

/// One tag document, and the members this entry puts in it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Document {
    /// The group the document names.
    pub tag: TagId,
    /// The members this entry contributes to it.
    pub members: Vec<RegistryKey>,
}

/// The one tag this entry owns, whose two members are both optional.
pub const OWN_TAG: &str = "optional_material";

/// Every tag document this entry ships, with the membership each one states.
///
/// Nothing here replaces: a tag has no owner, so every one of these merges with
/// whatever else contributed to the same name. The block and item halves of a
/// mirrored family are derived from one member list rather than written twice,
/// which is what makes the mirroring complete by construction.
///
/// # Errors
///
/// Returns [`ModError::Refused`] when a derived name is not a legal identifier,
/// which for these constants is a packaging failure rather than a runtime one.
pub fn documents(blocks: &[RegistryKey], items: &[RegistryKey]) -> Result<Vec<Document>, ModError> {
    let mut documents = Vec::with_capacity(TAG_DOCUMENTS);

    storage_documents(&mut documents, blocks, items)?;

    for material in census::MATERIAL_ITEMS {
        let one = [material.to_owned()];
        documents.push(document(
            &format!("{CONVENTION_NAMESPACE}:{material}s/{MOD_ID}"),
            ContentKind::Item,
            held(items, &one),
        )?);
        documents.push(document(
            &format!("{CONVENTION_NAMESPACE}:{material}s"),
            ContentKind::Item,
            held(items, &one),
        )?);
    }

    for (path, paths) in ore_facts() {
        let name = format!("{CONVENTION_NAMESPACE}:{path}");
        documents.push(document(&name, ContentKind::Block, held(blocks, &paths))?);
        documents.push(document(&name, ContentKind::Item, held(items, &paths))?);
    }

    for (path, paths) in mineability_facts() {
        documents.push(document(
            &format!("{HOST_NAMESPACE}:{path}"),
            ContentKind::Block,
            held(blocks, &paths),
        )?);
    }

    dependent_documents(&mut documents, items)?;

    Ok(documents)
}

fn storage_documents(
    documents: &mut Vec<Document>,
    blocks: &[RegistryKey],
    items: &[RegistryKey],
) -> Result<(), ModError> {
    let storage = census::storage_block_paths();
    for path in &storage {
        let name = format!("{CONVENTION_NAMESPACE}:storage_blocks/{path}");
        let one = std::slice::from_ref(path);
        documents.push(document(&name, ContentKind::Block, held(blocks, one))?);
        documents.push(document(&name, ContentKind::Item, held(items, one))?);
    }
    let aggregate = format!("{CONVENTION_NAMESPACE}:storage_blocks");
    documents.push(document(
        &aggregate,
        ContentKind::Block,
        held(blocks, &storage),
    )?);
    documents.push(document(
        &aggregate,
        ContentKind::Item,
        held(items, &storage),
    )?);
    Ok(())
}

fn dependent_documents(
    documents: &mut Vec<Document>,
    items: &[RegistryKey],
) -> Result<(), ModError> {
    let tools = held(items, &census::tool_item_paths());
    for name in [
        format!("{CONVENTION_NAMESPACE}:tools"),
        format!("{CONVENTION_NAMESPACE}:melee_weapons"),
        format!("{}:multi_tools", rvl_042_counterpart::MOD_ID),
        format!("{HOST_NAMESPACE}:cluster_harvest"),
    ] {
        documents.push(document(&name, ContentKind::Item, tools.clone())?);
    }
    for (path, role, kind) in [
        ("crop_blocks", Role::Block, ContentKind::Block),
        ("crop_materials", Role::Material, ContentKind::Item),
        ("crop_seeds", Role::Seed, ContentKind::Item),
    ] {
        let ids: Vec<String> = crops::DECLARED
            .iter()
            .map(|declared| {
                format!(
                    "{}:{}{}",
                    rvl_321_counterpart::MOD_ID,
                    declared.path,
                    role.suffix()
                )
            })
            .collect();
        documents.push(document(
            &format!("{}:{path}", rvl_321_counterpart::MOD_ID),
            kind,
            named(kind, &ids)?,
        )?);
    }
    documents.push(document(
        &format!("{MOD_ID}:{OWN_TAG}"),
        ContentKind::Item,
        named(ContentKind::Item, &crops::optional_tag_members())?,
    )?);
    Ok(())
}

/// The five conventional ore facts, each mirrored across both registries.
///
/// The first two are the split by material, the next two the split by the host
/// stone each deposit sits in, and the last the singular drop-rate tag.
fn ore_facts() -> Vec<(String, Vec<String>)> {
    let of_ore = |wanted: Ore| ore_paths(&|deposit: &world::Deposit| deposit.ore == wanted);
    let of_dimension =
        |wanted: Dimension| ore_paths(&|deposit: &world::Deposit| deposit.dimension == wanted);
    vec![
        ("ores/shard".to_owned(), of_ore(Ore::Shard)),
        ("ores/essence".to_owned(), of_ore(Ore::Essence)),
        (
            "ores_in_ground/lower_stone".to_owned(),
            of_dimension(Dimension::Lower),
        ),
        (
            "ores_in_ground/outer_stone".to_owned(),
            of_dimension(Dimension::Outer),
        ),
        ("ore_rates/singular".to_owned(), census::ore_block_paths()),
    ]
}

/// The four host mineability facts, which name blocks only.
fn mineability_facts() -> Vec<(&'static str, Vec<String>)> {
    let pickaxe: Vec<String> = census::block_item_paths()
        .into_iter()
        .filter(|path| path != census::GROUND_BLOCK)
        .collect();
    let mut iron = ore_paths(&|deposit: &world::Deposit| deposit.ore == Ore::Shard);
    iron.push(census::GEMSTONE_BLOCK.to_owned());
    let mut stone = ore_paths(&|deposit: &world::Deposit| deposit.ore == Ore::Essence);
    stone.push(census::INGOT_BLOCK.to_owned());
    stone.extend(census::anchor_block_paths());
    vec![
        ("mineable/pickaxe", pickaxe),
        ("mineable/shovel", vec![census::GROUND_BLOCK.to_owned()]),
        ("needs_iron_tool", iron),
        ("needs_stone_tool", stone),
    ]
}

/// The ore blocks whose deposit answers `wanted`, paired by declaration order.
fn ore_paths(wanted: &dyn Fn(&world::Deposit) -> bool) -> Vec<String> {
    census::ore_block_paths()
        .into_iter()
        .zip(world::DEPOSITS)
        .filter(|(_, deposit)| wanted(deposit))
        .map(|(path, _)| path)
        .collect()
}

/// The keys in `pool` whose paths are named in `paths`, in registry order.
fn held(pool: &[RegistryKey], paths: &[String]) -> Vec<RegistryKey> {
    pool.iter()
        .filter(|key| paths.iter().any(|path| path == key.id().path()))
        .cloned()
        .collect()
}

/// Keys for members another namespace registers.
fn named(kind: ContentKind, ids: &[String]) -> Result<Vec<RegistryKey>, ModError> {
    ids.iter()
        .map(|id| {
            id.parse::<ContentId>()
                .map(|id| RegistryKey::new(kind, id))
                .map_err(ModError::from_display)
        })
        .collect()
}

fn document(id: &str, kind: ContentKind, members: Vec<RegistryKey>) -> Result<Document, ModError> {
    let id = id.parse::<ContentId>().map_err(ModError::from_display)?;
    let tag = TagId::new(kind, id).map_err(ModError::from_display)?;
    Ok(Document { tag, members })
}

/// Declares every tag membership this entry contributes, and answers with the
/// number of documents it declared.
///
/// A document with no members is refused rather than declared, because every one
/// of the forty-eight states a membership and an empty one would reach the
/// barrier as a tag nobody can see the effect of.
///
/// # Errors
///
/// Returns [`ModError::Refused`] when a derived name is not a legal identifier
/// or when a membership is refused, both of which are packaging failures.
pub fn declare_tags(
    declaration: &mut ContentDeclaration,
    blocks: &[RegistryKey],
    items: &[RegistryKey],
) -> Result<usize, ModError> {
    let mut declared = 0;
    for document in documents(blocks, items)? {
        if document.members.is_empty() {
            return Err(ModError::refused(format!(
                "the tag {} would be declared with no members",
                document.tag
            )));
        }
        for member in &document.members {
            declaration
                .tag(&document.tag, member)
                .map_err(ModError::from_display)?;
        }
        declared += 1;
    }
    Ok(declared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Entry;
    use ravel_sdk::Declared;

    fn declared() -> (ContentDeclaration, usize) {
        let owner = Entry::declared_id().clone();
        let mut declaration = ContentDeclaration::new(owner);
        let blocks: Vec<RegistryKey> = census::block_paths()
            .iter()
            .map(|path| {
                declaration
                    .entry(ContentKind::Block, path)
                    .expect("a legal, undeclared block")
            })
            .collect();
        let items: Vec<RegistryKey> = census::item_paths()
            .iter()
            .map(|path| {
                declaration
                    .entry(ContentKind::Item, path)
                    .expect("a legal, undeclared item")
            })
            .collect();
        let tags = declare_tags(&mut declaration, &blocks, &items).expect("legal tag names");
        (declaration, tags)
    }

    fn members(declaration: &ContentDeclaration, kind: ContentKind, id: &str) -> usize {
        declaration
            .tags()
            .filter(|tag| tag.kind() == kind && tag.id().to_string() == id)
            .map(|tag| declaration.tag_members(tag).len())
            .sum()
    }

    #[test]
    fn the_declared_tags_are_the_forty_eight_the_census_counts() {
        let (declaration, tags) = declared();
        assert_eq!(tags, TAG_DOCUMENTS);
        assert_eq!(declaration.tags().count(), TAG_DOCUMENTS);
    }

    #[test]
    fn no_document_is_declared_with_a_placeholder_membership() {
        let (declaration, _) = declared();
        for tag in declaration.tags() {
            let held = declaration.tag_members(tag);
            assert_ne!(held.len(), 0, "{tag} states a membership");
            assert!(
                held.iter().all(|member| member.kind() == tag.kind()),
                "{tag} groups one registry"
            );
        }
    }

    #[test]
    fn every_mirrored_family_states_the_same_membership_in_both_registries() {
        let (declaration, _) = declared();
        let mut mirrored = 0;
        for tag in declaration.tags() {
            if tag.kind() != ContentKind::Block
                || tag.id().namespace().as_str() != CONVENTION_NAMESPACE
            {
                continue;
            }
            let id = tag.id().to_string();
            assert_eq!(
                members(&declaration, ContentKind::Item, &id),
                declaration.tag_members(tag).len(),
                "{id} is mirrored member for member"
            );
            mirrored += 1;
        }
        assert_eq!(
            mirrored,
            STORAGE_TAG_FACTS + 5,
            "the storage and ore families are the mirrored ones"
        );
    }

    #[test]
    fn the_storage_family_states_nine_specific_facts_and_one_aggregate() {
        let (declaration, _) = declared();
        let storage = census::storage_block_paths();
        assert_eq!(storage.len(), STORAGE_TAG_FACTS - 1);
        for kind in [ContentKind::Block, ContentKind::Item] {
            for path in &storage {
                let id = format!("{CONVENTION_NAMESPACE}:storage_blocks/{path}");
                assert_eq!(members(&declaration, kind, &id), 1, "{id}");
            }
            let id = format!("{CONVENTION_NAMESPACE}:storage_blocks");
            assert_eq!(members(&declaration, kind, &id), storage.len());
        }
    }

    #[test]
    fn the_material_family_names_items_only_and_states_one_member_each() {
        let (declaration, _) = declared();
        for material in census::MATERIAL_ITEMS {
            for id in [
                format!("{CONVENTION_NAMESPACE}:{material}s/{MOD_ID}"),
                format!("{CONVENTION_NAMESPACE}:{material}s"),
            ] {
                assert_eq!(members(&declaration, ContentKind::Item, &id), 1, "{id}");
                assert_eq!(members(&declaration, ContentKind::Block, &id), 0, "{id}");
            }
        }
    }

    #[test]
    fn the_ore_family_splits_by_material_by_host_stone_and_by_drop_rate() {
        let (declaration, _) = declared();
        let facts = ore_facts();
        assert_eq!(facts.len(), 5);
        for (path, paths) in facts {
            let id = format!("{CONVENTION_NAMESPACE}:{path}");
            let expected = if path == "ore_rates/singular" { 4 } else { 2 };
            assert_eq!(paths.len(), expected, "{id}");
            for kind in [ContentKind::Block, ContentKind::Item] {
                assert_eq!(members(&declaration, kind, &id), expected, "{id}");
            }
        }
    }

    #[test]
    fn the_mineability_family_states_the_nineteen_the_one_and_the_two_tiers() {
        let (declaration, _) = declared();
        for (path, expected) in [
            ("mineable/pickaxe", PICKAXE_MINEABLE),
            ("mineable/shovel", SHOVEL_MINEABLE),
            ("needs_iron_tool", IRON_TIER),
            ("needs_stone_tool", STONE_TIER),
        ] {
            let id = format!("{HOST_NAMESPACE}:{path}");
            assert_eq!(
                members(&declaration, ContentKind::Block, &id),
                expected,
                "{id}"
            );
            assert_eq!(members(&declaration, ContentKind::Item, &id), 0, "{id}");
        }
    }

    #[test]
    fn the_four_tool_listings_hold_the_same_six_items() {
        let (declaration, _) = declared();
        let listings = [
            format!("{CONVENTION_NAMESPACE}:tools"),
            format!("{CONVENTION_NAMESPACE}:melee_weapons"),
            format!("{}:multi_tools", rvl_042_counterpart::MOD_ID),
            format!("{HOST_NAMESPACE}:cluster_harvest"),
        ];
        for id in &listings {
            assert_eq!(
                members(&declaration, ContentKind::Item, id),
                crate::tools::COUNT,
                "{id}"
            );
        }
    }

    #[test]
    fn the_three_catalogue_tags_carry_the_six_crops_into_the_dependencys_rules() {
        let (declaration, _) = declared();
        for (path, kind) in [
            ("crop_blocks", ContentKind::Block),
            ("crop_materials", ContentKind::Item),
            ("crop_seeds", ContentKind::Item),
        ] {
            let id = format!("{}:{path}", rvl_321_counterpart::MOD_ID);
            assert_eq!(members(&declaration, kind, &id), crops::COUNT, "{id}");
        }
    }

    #[test]
    fn the_one_tag_this_entry_owns_carries_both_optional_spellings() {
        let (declaration, _) = declared();
        let id = format!("{MOD_ID}:{OWN_TAG}");
        assert_eq!(
            members(&declaration, ContentKind::Item, &id),
            crops::OPTIONAL_TAG_MEMBERS
        );
    }

    #[test]
    fn the_tag_counts_add_up_the_way_the_census_says_they_do() {
        let mirrored: usize = CONVENTIONAL
            .iter()
            .map(|family| family.facts * usize::from(family.both_registries) + family.facts)
            .sum();
        assert_eq!(
            mirrored + MINEABILITY_TAGS + TOOL_TAG_LISTINGS + TIERED_CATALOGUE_TAGS + OWN_TAGS,
            TAG_DOCUMENTS
        );
    }

    #[test]
    fn nineteen_documents_and_one_code_path_cover_the_twenty_block_items() {
        let documents = covered(LootShape::SelfDrop) + covered(LootShape::OreAlternative);
        assert_eq!(documents, LOOT_DOCUMENTS);
        assert_eq!(
            documents + covered(LootShape::CodePath),
            census::BLOCK_ITEMS
        );
    }

    #[test]
    fn the_fifteen_self_drops_are_the_storage_fuel_and_anchor_blocks() {
        let storage = 3;
        let fuel = usize::try_from(crate::fuel::TIERS).expect("the six fuel tiers fit usize");
        let anchors = crops::COUNT;
        assert_eq!(storage + fuel + anchors, covered(LootShape::SelfDrop));
    }

    #[test]
    fn the_four_ore_alternatives_match_the_deposits_and_their_ranges() {
        assert_eq!(covered(LootShape::OreAlternative), world::DEPOSITS.len());
        for deposit in &world::DEPOSITS {
            match deposit.ore {
                Ore::Shard => {
                    assert_eq!((deposit.lowest_drop, deposit.highest_drop), (2, 5));
                }
                Ore::Essence => {
                    assert_eq!((deposit.lowest_drop, deposit.highest_drop), (3, 5));
                }
            }
        }
    }

    #[test]
    fn forty_eight_tag_documents_all_append_and_twenty_of_them_state_ten_facts() {
        assert_eq!(STORAGE_TAG_DOCUMENTS.div_euclid(2), STORAGE_TAG_FACTS);
    }

    #[test]
    fn the_two_mirrored_families_state_each_fact_twice_and_the_third_states_it_once() {
        let mirrored = CONVENTIONAL
            .iter()
            .filter(|family| family.both_registries)
            .count();
        assert_eq!(mirrored, 2);
        assert!(CONVENTIONAL.iter().all(|family| family.facts > 0));

        let documents: usize = CONVENTIONAL
            .iter()
            .map(|family| family.facts * if family.both_registries { 2 } else { 1 })
            .sum();
        assert_eq!(documents, 36);
        assert!(documents < TAG_DOCUMENTS);
    }

    #[test]
    fn the_mineability_split_covers_every_block_item_once() {
        assert_eq!(PICKAXE_MINEABLE + SHOVEL_MINEABLE, census::BLOCK_ITEMS);
        assert_eq!(IRON_TIER + STONE_TIER, 12);
        assert_eq!(STONE_TIER, 2 + 1 + crops::COUNT);
    }

    #[test]
    fn the_language_counts_are_the_measured_ones_and_not_an_assumption_of_parity() {
        let matching = LANGUAGE_KEYS
            .iter()
            .filter(|keys| **keys == REFERENCE_KEYS)
            .count();
        assert_eq!(matching, 5, "five of eleven match the reference exactly");

        let stale = LANGUAGE_KEYS.iter().filter(|keys| **keys == 39).count();
        assert_eq!(stale, 2);
        let short = LANGUAGE_KEYS.iter().filter(|keys| **keys == 98).count();
        assert_eq!(short, 2);
        let over = LANGUAGE_KEYS
            .iter()
            .filter(|keys| **keys > REFERENCE_KEYS)
            .count();
        assert_eq!(over, 2, "the comparison is not a subset test either way");
    }

    #[test]
    fn the_grounds_moisture_round_trips_inside_the_dependencys_range() {
        use rvl_042_counterpart::flags::FlagTable;
        use rvl_321_counterpart::settings as depended;

        let highest = rvl_321_counterpart::ground::MAXIMUM_MOISTURE;

        let mut flags = FlagTable::default();
        let flag = depended::flag_id(depended::GROUND_CONVERT_BY_MATERIAL)
            .expect("the dependency names its conversion flag");
        flags.publish_constant(flag, true);
        let held = crate::tier::declared(
            "test:6".parse().expect("a legal tier identity"),
            "test:ground".parse().expect("a legal ground identity"),
            "test:material".parse().expect("a legal material identity"),
            false,
        );

        for moisture in 0..=highest {
            let converted = crate::ground::convert(&flags, None, true, &held, moisture, 0);
            assert_eq!(converted.ground.as_deref(), Some("test:ground"));
            assert_eq!(
                converted.moisture, moisture,
                "the one persisted block state carries across unchanged"
            );
        }
    }

    #[test]
    fn a_world_reloaded_without_a_companion_mod_loses_recipes_and_not_identifiers() {
        for declared in crops::DECLARED
            .iter()
            .filter(|held| held.declares_companion())
        {
            let crop = declared.crop(
                format!("test:{}", declared.path)
                    .parse()
                    .expect("a legal crop identity"),
                &"test:6".parse().expect("a legal tier identity"),
                declared
                    .material_id("test")
                    .parse()
                    .expect("a legal material identity"),
            );
            let state = crops::state(declared, &crop, false, &|_| false, &|_| 1);
            assert_eq!(state, rvl_321_counterpart::Availability::Unobtainable);
            assert_ne!(
                crop.path().len(),
                0,
                "the identifier is still there to resolve on reload"
            );
        }
    }
}
