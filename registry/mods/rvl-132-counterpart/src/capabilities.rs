//! Section nine's attachment table, and section nine's delegation rule.
//!
//! Two compatibility bundles, both enabled by default and both individually
//! disableable. The vanilla bundle makes 46 attachments and the third-party one
//! makes a further attachment that is present only when that mod is.
//!
//! Every attachment is declared through RVL-210's capability-constructor
//! registry, which section 16 calls this entry's largest use of it. The registry,
//! [`Address`] and [`TargetClass`] are all consumed from RVL-210 and none is
//! rebuilt here.
//!
//! A Ravel package names no other package's content, so the one third-party
//! attachment names its target through [`Bundle::third_party`] rather than
//! carrying a spelling this package would have had to read somewhere.

use core::fmt;

use rvl_210_counterpart::capabilities::{Address, Refused, Registry, TargetClass};
use rvl_210_counterpart::quantity::Amount;
use rvl_210_counterpart::storage::{Attempt, Store};

use crate::kinds::Ingredient;
use crate::storage::Pool;

/// The work indicator, which comes from the vocabulary RVL-210 owns.
pub const WORK: &str = "work_indicator";

/// The heat reading, which comes from the vocabulary RVL-210 owns.
pub const HEAT: &str = "heat_reading";

/// The recipe view, which comes from the vocabulary RVL-210 owns.
pub const RECIPES: &str = "recipe_view";

/// The item store, which is the host platform's own.
pub const ITEMS: &str = "item_store";

/// The fluid store, which is the host platform's own.
pub const FLUIDS: &str = "fluid_store";

/// The charge store, which is the host platform's own.
pub const CHARGE: &str = "charge_store";

/// The three capability kinds that come from RVL-210's vocabulary.
pub const FROM_VOCABULARY: [&str; 3] = [WORK, HEAT, RECIPES];

/// The three capability kinds that are the host platform's own.
pub const FROM_PLATFORM: [&str; 3] = [ITEMS, FLUIDS, CHARGE];

/// The three smelting stations.
pub const SMELTING: [&str; 3] = [
    "minecraft:furnace",
    "minecraft:blast_furnace",
    "minecraft:smoker",
];

/// The brewing station.
pub const BREWING: &str = "minecraft:brewing_stand";

/// The campfire.
pub const CAMPFIRE: &str = "minecraft:campfire";

/// Every bucket-like item, addressed by class rather than one at a time.
pub const BUCKETS: &str = "minecraft:bucket";

/// Every crafting-table block, addressed by class.
pub const CRAFTING: &str = "minecraft:crafting_table";

/// The cutting block.
pub const CUTTING: &str = "minecraft:stonecutter";

/// The upgrade block.
pub const UPGRADE: &str = "minecraft:smithing_table";

/// The composting block.
pub const COMPOSTING: &str = "minecraft:composter";

/// The carrying pouch item.
pub const POUCH: &str = "minecraft:bundle";

/// The dropped-item entity.
pub const DROPPED: &str = "minecraft:item";

/// The two picture-frame entities.
pub const FRAMES: [&str; 2] = ["minecraft:item_frame", "minecraft:glow_item_frame"];

/// The seventeen coloured and uncoloured portable container items.
pub const CONTAINERS: [&str; 17] = [
    "minecraft:shulker_box",
    "minecraft:white_shulker_box",
    "minecraft:orange_shulker_box",
    "minecraft:magenta_shulker_box",
    "minecraft:light_blue_shulker_box",
    "minecraft:yellow_shulker_box",
    "minecraft:lime_shulker_box",
    "minecraft:pink_shulker_box",
    "minecraft:gray_shulker_box",
    "minecraft:light_gray_shulker_box",
    "minecraft:cyan_shulker_box",
    "minecraft:purple_shulker_box",
    "minecraft:blue_shulker_box",
    "minecraft:brown_shulker_box",
    "minecraft:green_shulker_box",
    "minecraft:red_shulker_box",
    "minecraft:black_shulker_box",
];

/// The three entities section nine's delegation rule covers.
///
/// A dropped item and a picture frame each hold an item, and each of the three
/// stores is attached to the entity only if the item it holds publishes the same
/// store.
pub const CARRIERS: [&str; 3] = [DROPPED, FRAMES[0], FRAMES[1]];

/// How many attachments the vanilla bundle makes.
pub const VANILLA_ATTACHMENTS: usize = 46;

/// One attachment, before it reaches RVL-210's registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attachment {
    /// The capability attached.
    pub capability: &'static str,
    /// The class the target sits in.
    pub class: TargetClass,
    /// The target, specific or by class.
    pub address: Address,
}

fn specific(capability: &'static str, class: TargetClass, name: &str) -> Attachment {
    Attachment {
        capability,
        class,
        address: Address::Specific(name.to_owned()),
    }
}

fn broad(capability: &'static str, class: TargetClass, name: &str) -> Attachment {
    Attachment {
        capability,
        class,
        address: Address::Broad(name.to_owned()),
    }
}

/// Every attachment the vanilla bundle makes, in the order section nine tables
/// them.
#[must_use]
pub fn vanilla() -> Vec<Attachment> {
    let mut attachments = Vec::with_capacity(VANILLA_ATTACHMENTS);

    // Work indicator: the three smelting stations, the brewing station, the
    // campfire.
    for station in SMELTING {
        attachments.push(specific(WORK, TargetClass::BlockEntity, station));
    }
    attachments.push(specific(WORK, TargetClass::BlockEntity, BREWING));
    attachments.push(specific(WORK, TargetClass::BlockEntity, CAMPFIRE));

    // Heat reading: the three smelting stations, the campfire, and every
    // bucket-like item by class.
    for station in SMELTING {
        attachments.push(specific(HEAT, TargetClass::BlockEntity, station));
    }
    attachments.push(specific(HEAT, TargetClass::BlockEntity, CAMPFIRE));
    attachments.push(broad(HEAT, TargetClass::Item, BUCKETS));

    // Item store: seventeen container items, the carrying pouch, the dropped
    // item, the two picture frames and the composting block.
    for container in CONTAINERS {
        attachments.push(specific(ITEMS, TargetClass::Item, container));
    }
    attachments.push(specific(ITEMS, TargetClass::Item, POUCH));
    attachments.push(specific(ITEMS, TargetClass::Entity, DROPPED));
    for frame in FRAMES {
        attachments.push(specific(ITEMS, TargetClass::Entity, frame));
    }
    attachments.push(specific(ITEMS, TargetClass::Block, COMPOSTING));

    // Fluid store and charge store: the dropped item and the two picture frames.
    for capability in [FLUIDS, CHARGE] {
        for carrier in CARRIERS {
            attachments.push(specific(capability, TargetClass::Entity, carrier));
        }
    }

    // Recipe view: the brewing station, the three smelting stations, the
    // campfire, every crafting-table block by class, the cutting block and the
    // upgrade block.
    attachments.push(specific(RECIPES, TargetClass::BlockEntity, BREWING));
    for station in SMELTING {
        attachments.push(specific(RECIPES, TargetClass::BlockEntity, station));
    }
    attachments.push(specific(RECIPES, TargetClass::BlockEntity, CAMPFIRE));
    attachments.push(broad(RECIPES, TargetClass::Block, CRAFTING));
    attachments.push(specific(RECIPES, TargetClass::Block, CUTTING));
    attachments.push(specific(RECIPES, TargetClass::Block, UPGRADE));

    attachments
}

/// What a bundle refuses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotInstalled {
    /// RVL-210's registry refused an attachment.
    Registry {
        /// The registry's typed refusal.
        error: Refused,
    },
    /// The third-party bundle names no target.
    NoTarget,
}

impl fmt::Display for NotInstalled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Registry { error } => write!(formatter, "{error}"),
            Self::NoTarget => {
                formatter.write_str("the third-party bundle was enabled without naming a target")
            }
        }
    }
}

impl std::error::Error for NotInstalled {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Registry { error } => Some(error),
            Self::NoTarget => None,
        }
    }
}

impl From<Refused> for NotInstalled {
    fn from(refused: Refused) -> Self {
        Self::Registry { error: refused }
    }
}

/// The name the vanilla bundle is switched under.
pub const VANILLA_BUNDLE: &str = "vanilla";

/// The name the third-party bundle is switched under.
pub const THIRD_PARTY_BUNDLE: &str = "third_party_station";

/// One compatibility bundle.
///
/// Section nine: both are enabled by default and both are individually
/// disableable through RVL-210's settings framework, so a bundle carries its
/// switch rather than being installed unconditionally.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bundle {
    name: &'static str,
    enabled: bool,
    attachments: Vec<Attachment>,
}

impl Bundle {
    /// The vanilla bundle, which is always available.
    #[must_use]
    pub fn vanilla(enabled: bool) -> Self {
        Self {
            name: VANILLA_BUNDLE,
            enabled,
            attachments: vanilla(),
        }
    }

    /// The third-party bundle, whose one attachment is a recipe view over
    /// another package's crafting-station block entity.
    ///
    /// The target is named by the caller because a Ravel package declares its own
    /// names and never another's, and because section nine makes the bundle
    /// present only when that package is.
    #[must_use]
    pub fn third_party(enabled: bool, target: Option<&str>) -> Self {
        let attachments = target.map_or_else(Vec::new, |target| {
            vec![specific(RECIPES, TargetClass::BlockEntity, target)]
        });
        Self {
            name: THIRD_PARTY_BUNDLE,
            enabled,
            attachments,
        }
    }

    /// What the bundle is switched under.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Whether the settings framework has it switched on.
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// The attachments it would make.
    pub fn attachments(&self) -> impl Iterator<Item = &Attachment> + '_ {
        self.attachments.iter()
    }

    /// How many attachments it makes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.attachments.len()
    }

    /// Whether it makes none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.attachments.is_empty()
    }

    /// Installs the bundle into RVL-210's registry, answering how many
    /// attachments it made.
    ///
    /// A disabled bundle installs nothing and is not an error.
    ///
    /// # Errors
    ///
    /// Returns [`NotInstalled::Registry`] when the registry refuses, which it
    /// does after it is baked and past its own published limit, and
    /// [`NotInstalled::NoTarget`] for an enabled third-party bundle naming
    /// nothing.
    pub fn install(&self, registry: &mut Registry) -> Result<usize, NotInstalled> {
        if !self.enabled {
            return Ok(0);
        }
        if self.name == THIRD_PARTY_BUNDLE && self.attachments.is_empty() {
            return Err(NotInstalled::NoTarget);
        }
        for attachment in &self.attachments {
            registry.register(
                attachment.class,
                attachment.address.clone(),
                attachment.capability,
            )?;
        }
        Ok(self.attachments.len())
    }
}

/// What one entity holds, and the store that item publishes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Holding<K: Ingredient>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    /// The held item's identity.
    pub identity: ravel_sdk::content::ContentId,
    /// The store it publishes, absent when it publishes none of this kind.
    pub store: Option<Pool<K>>,
}

/// A dropped item or a picture frame, and the store it delegates to.
///
/// Section nine's delegation rule has two consequences a counterpart must keep,
/// and both are properties of this type rather than of a caller's discipline:
/// the entity's store appears and disappears as the held item changes, and a
/// caller holding the entity's store across a change sees the new item rather
/// than a stale one. Every operation here re-reads the held item, so there is no
/// handle to go stale.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Carrier<K: Ingredient>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    held: Option<Holding<K>>,
}

impl<K: Ingredient> Carrier<K>
where
    K::Instance: Clone + fmt::Debug + Eq,
{
    /// An entity holding nothing.
    #[must_use]
    pub const fn empty() -> Self {
        Self { held: None }
    }

    /// An entity holding one item.
    #[must_use]
    pub const fn holding(held: Holding<K>) -> Self {
        Self { held: Some(held) }
    }

    /// Puts a different item in the entity, answering what it held.
    pub fn put(&mut self, held: Option<Holding<K>>) -> Option<Holding<K>> {
        core::mem::replace(&mut self.held, held)
    }

    /// What the entity holds.
    #[must_use]
    pub const fn held(&self) -> Option<&Holding<K>> {
        self.held.as_ref()
    }

    /// Whether the entity publishes this store at all.
    ///
    /// The store is attached only if the item the entity holds publishes the
    /// same store, so this answers false for an empty entity and for one holding
    /// an item that publishes none.
    #[must_use]
    pub const fn publishes(&self) -> bool {
        matches!(&self.held, Some(Holding { store: Some(_), .. }))
    }

    /// Everything the delegated store holds, read through the held item.
    #[must_use]
    pub fn contents(&self) -> Vec<K::Instance> {
        self.held
            .as_ref()
            .and_then(|held| held.store.as_ref())
            .map_or_else(Vec::new, |store| store.contents().cloned().collect())
    }

    /// Forwards an insertion to the held item's store and writes it back.
    pub fn insert(&mut self, value: &K::Instance, amount: Amount, attempt: Attempt) -> Amount {
        let Some(Holding {
            store: Some(store), ..
        }) = &mut self.held
        else {
            return 0;
        };
        store.insert(value, amount, attempt)
    }

    /// Forwards an extraction to the held item's store and writes it back.
    pub fn extract(&mut self, value: &K::Instance, amount: Amount, attempt: Attempt) -> Amount {
        let Some(Holding {
            store: Some(store), ..
        }) = &mut self.held
        else {
            return 0;
        };
        store.extract(value, amount, attempt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinds::{Stack, Stackable};
    use std::collections::BTreeSet;

    fn counted(capability: &str) -> usize {
        vanilla()
            .into_iter()
            .filter(|attachment| attachment.capability == capability)
            .count()
    }

    #[test]
    fn the_vanilla_bundle_makes_forty_six_attachments() {
        assert_eq!(vanilla().len(), VANILLA_ATTACHMENTS);
        assert_eq!(counted(WORK), 5);
        assert_eq!(counted(HEAT), 5);
        assert_eq!(counted(ITEMS), 22);
        assert_eq!(counted(FLUIDS), 3);
        assert_eq!(counted(CHARGE), 3);
        assert_eq!(counted(RECIPES), 8);
    }

    #[test]
    fn the_table_names_six_capability_kinds_and_no_others() {
        let named: BTreeSet<&str> = vanilla()
            .iter()
            .map(|attachment| attachment.capability)
            .collect();
        let expected: BTreeSet<&str> = FROM_VOCABULARY.into_iter().chain(FROM_PLATFORM).collect();
        assert_eq!(named, expected);
    }

    #[test]
    fn three_capability_kinds_come_from_the_vocabulary_and_three_from_the_platform() {
        for capability in FROM_VOCABULARY {
            assert!(!FROM_PLATFORM.contains(&capability), "{capability}");
        }
    }

    #[test]
    fn no_attachment_is_made_twice() {
        let attachments = vanilla();
        let distinct: BTreeSet<(&str, TargetClass, String)> = attachments
            .iter()
            .map(|attachment| {
                (
                    attachment.capability,
                    attachment.class,
                    attachment.address.written().to_owned(),
                )
            })
            .collect();
        assert_eq!(distinct.len(), attachments.len());
    }

    #[test]
    fn the_two_by_class_attachments_are_the_only_broad_ones() {
        let broad: Vec<String> = vanilla()
            .into_iter()
            .filter(|attachment| matches!(attachment.address, Address::Broad(_)))
            .map(|attachment| attachment.address.written().to_owned())
            .collect();
        assert_eq!(broad, vec![BUCKETS.to_owned(), CRAFTING.to_owned()]);
    }

    #[test]
    fn seventeen_container_items_and_the_pouch_carry_an_item_store() {
        let mut distinct: BTreeSet<&str> = CONTAINERS.into_iter().collect();
        assert_eq!(distinct.len(), 17);
        distinct.insert(POUCH);
        assert_eq!(distinct.len(), 18);
        let attached: BTreeSet<String> = vanilla()
            .into_iter()
            .filter(|attachment| {
                attachment.capability == ITEMS && attachment.class == TargetClass::Item
            })
            .map(|attachment| attachment.address.written().to_owned())
            .collect();
        assert_eq!(attached.len(), 18);
    }

    #[test]
    fn the_three_carriers_take_all_three_stores_and_the_composting_block_takes_one() {
        for capability in [ITEMS, FLUIDS, CHARGE] {
            for carrier in CARRIERS {
                assert!(
                    vanilla().contains(&specific(capability, TargetClass::Entity, carrier)),
                    "{capability} on {carrier}"
                );
            }
        }
        assert!(vanilla().contains(&specific(ITEMS, TargetClass::Block, COMPOSTING)));
        assert!(!vanilla().contains(&specific(FLUIDS, TargetClass::Block, COMPOSTING)));
    }

    #[test]
    fn the_vanilla_bundle_installs_every_attachment_into_the_registry() {
        let mut registry = Registry::new();
        let installed = Bundle::vanilla(true)
            .install(&mut registry)
            .expect("a fresh registry");
        assert_eq!(installed, VANILLA_ATTACHMENTS);
    }

    #[test]
    fn a_disabled_bundle_installs_nothing_and_is_not_an_error() {
        let mut registry = Registry::new();
        assert_eq!(Bundle::vanilla(false).install(&mut registry), Ok(0));
        assert_eq!(
            Bundle::third_party(false, None).install(&mut registry),
            Ok(0)
        );
    }

    #[test]
    fn the_third_party_bundle_makes_one_attachment_when_that_package_is_present() {
        let mut registry = Registry::new();
        let bundle = Bundle::third_party(true, Some("other:station"));
        assert_eq!(bundle.len(), 1);
        assert_eq!(bundle.install(&mut registry), Ok(1));
        assert_eq!(bundle.name(), THIRD_PARTY_BUNDLE);
    }

    #[test]
    fn an_enabled_third_party_bundle_naming_nothing_is_refused_rather_than_silent() {
        let mut registry = Registry::new();
        let bundle = Bundle::third_party(true, None);
        assert!(bundle.is_empty());
        assert_eq!(bundle.install(&mut registry), Err(NotInstalled::NoTarget));
    }

    #[test]
    fn a_bundle_installed_after_the_registry_is_baked_is_refused() {
        let mut registry = Registry::new();
        registry.bake();
        let refusal = Bundle::vanilla(true)
            .install(&mut registry)
            .expect_err("a baked registry takes nothing");
        assert!(matches!(refusal, NotInstalled::Registry { .. }));
        assert!(refusal.to_string().contains("baked"));
        assert!(
            std::error::Error::source(&refusal)
                .expect("the registry refusal is retained")
                .downcast_ref::<Refused>()
                .is_some()
        );
    }

    #[test]
    fn both_bundles_reach_one_registry_together() {
        let mut registry = Registry::new();
        let vanilla = Bundle::vanilla(true)
            .install(&mut registry)
            .expect("a fresh registry");
        let third = Bundle::third_party(true, Some("other:station"))
            .install(&mut registry)
            .expect("room for one more");
        assert_eq!(vanilla + third, VANILLA_ATTACHMENTS + 1);
    }

    #[test]
    fn an_entity_holding_an_item_that_publishes_no_store_publishes_none_either() {
        let carrier: Carrier<Stackable> = Carrier::holding(Holding {
            identity: "minecraft:stone".parse().expect("a content identity"),
            store: None,
        });
        assert!(!carrier.publishes());
        assert_eq!(carrier.contents().len(), 0);
    }

    #[test]
    fn an_entity_holding_nothing_publishes_no_store() {
        let carrier: Carrier<Stackable> = Carrier::empty();
        assert!(!carrier.publishes());
        assert_eq!(carrier.held(), None);
    }

    #[test]
    fn the_stores_operations_forward_to_the_held_items_store() {
        let mut carrier = Carrier::holding(Holding {
            identity: "minecraft:shulker_box".parse().expect("a content identity"),
            store: Some(Pool::new(Stackable::default(), 1_000)),
        });
        assert!(carrier.publishes());
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        assert_eq!(carrier.insert(&stone, 30, Attempt::Real), 30);
        assert_eq!(carrier.contents().len(), 1);
        assert_eq!(carrier.extract(&stone, 10, Attempt::Real), 10);
        assert_eq!(carrier.contents()[0].count, 20);
    }

    #[test]
    fn an_operation_on_an_entity_publishing_nothing_answers_the_empty_answer() {
        let mut carrier: Carrier<Stackable> = Carrier::empty();
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        assert_eq!(carrier.insert(&stone, 30, Attempt::Real), 0);
        assert_eq!(carrier.extract(&stone, 30, Attempt::Real), 0);
    }

    #[test]
    fn the_store_appears_and_disappears_as_the_held_item_changes() {
        let mut carrier: Carrier<Stackable> = Carrier::empty();
        assert!(!carrier.publishes());
        carrier.put(Some(Holding {
            identity: "minecraft:shulker_box".parse().expect("a content identity"),
            store: Some(Pool::new(Stackable::default(), 1_000)),
        }));
        assert!(carrier.publishes());
        carrier.put(Some(Holding {
            identity: "minecraft:stone".parse().expect("a content identity"),
            store: None,
        }));
        assert!(!carrier.publishes());
        assert_eq!(carrier.contents().len(), 0);
    }

    #[test]
    fn a_caller_reading_across_a_change_sees_the_new_item_rather_than_a_stale_one() {
        let mut first = Pool::new(Stackable::default(), 1_000);
        first.insert(
            &Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                1,
            ),
            10,
            Attempt::Real,
        );
        let mut second = Pool::new(Stackable::default(), 1_000);
        second.insert(
            &Stack::of(
                "minecraft:dirt".parse().expect("a valid content identity"),
                1,
            ),
            5,
            Attempt::Real,
        );
        let mut carrier = Carrier::holding(Holding {
            identity: "minecraft:shulker_box".parse().expect("a content identity"),
            store: Some(first),
        });
        assert_eq!(
            carrier.contents()[0].identity.to_string(),
            "minecraft:stone"
        );
        let was = carrier.put(Some(Holding {
            identity: "minecraft:white_shulker_box"
                .parse()
                .expect("a content identity"),
            store: Some(second),
        }));
        assert_eq!(
            was.map(|held| held.identity),
            Some("minecraft:shulker_box".parse().expect("a content identity"))
        );
        assert_eq!(carrier.contents()[0].identity.to_string(), "minecraft:dirt");
        assert_eq!(carrier.contents()[0].count, 5);
    }

    #[test]
    fn refusals_say_what_was_wrong() {
        assert!(
            NotInstalled::NoTarget
                .to_string()
                .contains("naming a target")
        );
        assert!(
            NotInstalled::Registry {
                error: Refused::Full { limit: 4 }
            }
            .to_string()
            .contains("at most 4 attachments")
        );
    }
}
