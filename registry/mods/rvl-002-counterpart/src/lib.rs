#![forbid(unsafe_code)]

//! An original Ravel implementation of the behaviour specified in RVL-002.
//!
//! A leaf of its wave's content cluster. It extends the tiered-crop entry by one
//! tier above that entry's numbered progression, and it adds the content that
//! tier needs to be reachable: a ground block, a material, six crops, four
//! deposits and four families of consumables and tools that span all six tiers
//! rather than only the new one.
//!
//! Almost everything it does is declared data or a small numeric rule applied to
//! declared data. What survives as counterpart work is exactly that: the fuel
//! derivation, the food effect rule, the tool grade table and dispatch rule, the
//! boss drop rule, the ground behaviour it shares with its dependency, and the
//! three states of absence.
//!
//! What a dependent reaches for:
//!
//! - [`settings`] — the eight settings, the five flags they publish and the two
//!   they only read;
//! - [`tier`] — the sixth tier, its four declared and four adjusted values;
//! - [`crops`] — the six crops and which of the three absence states each
//!   reaches;
//! - [`census`] — the registered content, counted, and the creative listing's
//!   order;
//! - [`ground`] — the sixth-tier ground, its drops and what gates conversion;
//! - [`fuel`] — the two derivations behind all twelve burn times;
//! - [`foods`] — the six rows, the cumulative effects and the additive duration;
//! - [`tools`] — the grade table, unbreakability and modifier dispatch;
//! - [`drops`] — what a defeated boss adds to a drop event;
//! - [`recipes`] — the recipe census, its conditions, and the two counts;
//! - [`world`] — the four deposits and the shaping set;
//! - [`data`] — loot, tags, translations and what survives a reload;
//! - [`dumps`] — the documents those tables render out under `generated/`.
//!
//! `NOTES.md` beside this file says which sections are carried here and which
//! are left declarable only because Ravel publishes no seam for them.

pub mod census;
pub mod crops;
pub mod data;
pub mod drops;
pub mod dumps;
pub mod foods;
pub mod fuel;
pub mod ground;
pub mod recipes;
pub mod settings;
pub mod tier;
pub mod tools;
pub mod world;

use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration};
use ravel_sdk::content::{ContentDeclaration, ContentKind, Registrar, claim_all};
use ravel_sdk::events::GameEvent;
use ravel_sdk::prelude::*;
use ravel_sdk::world_shaping::WorldShaping;
use rvl_042_counterpart::flags::FlagTable;
use rvl_321_counterpart::model::{Crop, Tier};

pub use crate::settings::Settings;

/// The identity this package is declared and validated under.
pub const MOD_ID: &str = "rvl_002_counterpart";

/// The namespace the host's own content and tags are named in.
pub const HOST_NAMESPACE: &str = "host";

/// The namespace conventional cross-mod tags are named in.
///
/// A tag named there has no owner, which is the whole point of the convention:
/// every mod that makes an ingot puts one in `c:ingots`.
pub const CONVENTION_NAMESPACE: &str = "c";

/// The entry runs on both sides and is mandatory on both.
pub const SIDES: [Side; 3] = [Side::Client, Side::IntegratedServer, Side::DedicatedServer];

/// The two entries this one declares a hard dependency on.
///
/// The second is declared *load after*, because this entry's registration window
/// reads a tier registry, an item registry and a tag that entry owns.
pub const DEPENDENCIES: [&str; 2] = [rvl_042_counterpart::MOD_ID, rvl_321_counterpart::MOD_ID];

/// The entry this entry must load after.
pub const LOADS_AFTER: &str = rvl_321_counterpart::MOD_ID;

/// The companion mods section six's four crops name, discovered at runtime.
///
/// No soft dependency is declared on any of them. A counterpart that declared
/// one would change the resolved load order, which is the one thing the original
/// does not do here.
pub const DECLARED_SOFT_DEPENDENCIES: usize = 0;

/// Registry kinds Ravel publishes that this entry's census fits into.
///
/// Two of the six registries of section seven have a key to claim: blocks and
/// items. The fluid, fluid-type and creative-listing registries do not, so the
/// thirty fluid registrations and the listing are carried as data in [`census`]
/// and `NOTES.md` records them as host-blocked. This entry owns no recipe kind
/// at all: every recipe it ships is data against a kind one of its two
/// dependencies claims.
pub const CLAIMED_KINDS: [ContentKind; 2] = [ContentKind::Block, ContentKind::Item];

/// The entry, as the loader drives it.
#[derive(Debug)]
pub struct Entry {
    declaration: Result<ContentDeclaration, ModError>,
    configuration: Result<ConfigurationDeclaration, ModError>,
    settings: Settings,
    flags: FlagTable,
    tier: Tier,
    crops: Vec<Crop>,
    adjusted: bool,
    reloads: u32,
    entered: Vec<LifecycleState>,
}

impl Default for Entry {
    fn default() -> Self {
        Self::new()
    }
}

impl Entry {
    /// Builds the entry holding its shipped defaults, its tier and its six
    /// crops, all six registered and none of them adjusted yet.
    #[must_use]
    pub fn new() -> Self {
        let tier = tier::registered(&format!("{MOD_ID}:{}", tier::PATH));
        let crops = crops::DECLARED
            .iter()
            .map(|declared| declared.crop(MOD_ID, &tier.id, &declared.material_id(MOD_ID)))
            .collect();
        Self {
            declaration: Self::declaration(),
            configuration: settings::declaration().map_err(|error| ModError::refused(error.to_string())),
            settings: Settings::default(),
            flags: FlagTable::default(),
            tier,
            crops,
            adjusted: false,
            reloads: 0,
            entered: Vec::new(),
        }
    }

    /// The values in force.
    #[must_use]
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The flag table this entry published into.
    #[must_use]
    pub const fn flags(&self) -> &FlagTable {
        &self.flags
    }

    /// The sixth tier.
    #[must_use]
    pub const fn tier(&self) -> &Tier {
        &self.tier
    }

    /// The six crops, all registered whatever state they are in.
    #[must_use]
    pub fn crops(&self) -> &[Crop] {
        &self.crops
    }

    /// How many reloads this entry has seen.
    #[must_use]
    pub const fn reloads(&self) -> u32 {
        self.reloads
    }

    /// The lifecycle steps this entry has taken.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }

    /// Whether the adjust pass has run.
    #[must_use]
    pub const fn adjusted(&self) -> bool {
        self.adjusted
    }

    /// The adjust pass: point the tier at content, copy the accelerant switch,
    /// and point every crop at its anchor block.
    ///
    /// The pointing is unconditional and happens for all six crops including the
    /// ones no companion mod enables, so an unobtainable crop still has a valid
    /// anchor.
    pub fn adjust(&mut self) {
        tier::adjust(
            &mut self.tier,
            &format!("{MOD_ID}:ground"),
            &format!("{MOD_ID}:material"),
            self.settings.crops_fertilisable,
        );
        for (crop, declared) in self.crops.iter_mut().zip(crops::DECLARED) {
            crop.foundation = Some(format!("{MOD_ID}:{}", declared.anchor));
        }
        self.adjusted = true;
    }

    /// This entry declared into the world-shaping set of a world it deposits in.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when the entry's own identity is not a
    /// legal mod id, which for a constant is a packaging failure.
    pub fn shape_world(shaping: &mut WorldShaping) -> Result<(), ModError> {
        world::shaping(shaping).map_err(|error| ModError::refused(error.to_string()))
    }

    /// What this entry claims at the registration barrier.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when a declared name is not a legal content
    /// path, which for these constants is a packaging failure.
    /// Every block is claimed before any item, and the twenty block items are
    /// claimed before the thirty-three plain ones, so the block items occupy the
    /// earliest positions in this namespace's item registry. The tag
    /// memberships of section sixteen are declared in the same place, because
    /// the declaration is the one statement both the barrier and the generated
    /// pack are derived from.
    pub fn declaration() -> Result<ContentDeclaration, ModError> {
        let mut declaration = ContentDeclaration::new(Self::declared_id().clone());
        let mut blocks = Vec::with_capacity(census::BLOCKS);
        for path in census::block_paths() {
            blocks.push(
                declaration
                    .entry(ContentKind::Block, &path)
                    .map_err(|error| ModError::refused(error.to_string()))?,
            );
        }
        let mut items = Vec::with_capacity(census::ITEMS);
        for path in census::item_paths() {
            items.push(
                declaration
                    .entry(ContentKind::Item, &path)
                    .map_err(|error| ModError::refused(error.to_string()))?,
            );
        }
        let tags = data::declare_tags(&mut declaration, &blocks, &items)?;
        if tags != data::TAG_DOCUMENTS {
            return Err(ModError::refused(format!(
                "{tags} of the {} tag documents were declared",
                data::TAG_DOCUMENTS
            )));
        }
        Ok(declaration)
    }
}

#[declare_mod(id = "rvl_002_counterpart")]
impl Mod for Entry {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        if state == LifecycleState::Constructed {
            self.declaration.as_ref().map_err(Clone::clone)?;
            self.configuration.as_ref().map_err(Clone::clone)?;
        }
        self.entered.push(state);
        Ok(())
    }

    fn content_declaration(&self) -> Result<Option<ContentDeclaration>, ModError> {
        self.declaration.clone().map(Some)
    }

    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
        let declaration = self.declaration.as_ref().map_err(Clone::clone)?;
        claim_all(declaration, registrar)?;
        self.adjust();
        Ok(())
    }

    fn configuration_declaration(&self) -> Result<Option<ConfigurationDeclaration>, ModError> {
        self.configuration.clone().map(Some)
    }

    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
        self.settings = Settings::read(resolved);
        let published = settings::publish(&mut self.flags, &self.settings);
        let declared = settings::published_flags().len();
        if published != declared {
            return Err(ModError::refused(format!(
                "{published} of the {declared} flags this entry publishes reached the table"
            )));
        }
        Ok(())
    }

    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        match event {
            GameEvent::ResourcesReloaded => self.reloads = self.reloads.saturating_add(1),
            GameEvent::ServerTick | GameEvent::ClientTick => {}
        }
        Ok(())
    }
}

export_mod!(Entry::new());

/// What this entry needs from the shared library.
///
/// Precisely: the non-mirrored shaped recipe kind for eight documents, the
/// feature-flag mechanism for the five it publishes and the flag-aware creative
/// listing generator, and the crafting-remainder behaviour for the six buckets.
///
/// That package's own section fourteen records this entry as needing its
/// persistence as well. It does not, and [`needs_shared_persistence`] is that
/// correction: this entry persists nothing of its own and touches none of that
/// package's persistence envelope directly.
#[must_use]
pub fn needs_from_shared() -> Vec<&'static str> {
    vec!["shaped_unmirrored", "feature_flags", "crafting_remainder"]
}

/// Whether this entry needs the shared library's persistence. It does not.
#[must_use]
pub const fn needs_shared_persistence() -> bool {
    false
}

/// What this entry needs from the tiered-crop entry.
///
/// Precisely: the registration windows and their two-pass shape, the tier and
/// crop model with its exact defaults, the second-drop rule this tier disables,
/// the ground behaviour this entry extends by one member, the tilling, awakening
/// and seed recipe kinds and their conditions, the dynamic derivation this entry
/// switches *off*, and the slotted-modifier format the tools carry.
///
/// It does not need that entry's machines, rituals, charge model, networking or
/// world generation.
#[must_use]
pub fn needs_from_tiered() -> Vec<&'static str> {
    vec![
        "registration_windows",
        "tier_and_crop_model",
        "second_drop_rule",
        "ground_behaviour",
        "recipe_kinds_and_conditions",
        "dynamic_derivation",
        "slotted_modifiers",
    ]
}

/// What the sibling entry in the same cluster needs from this one, and this one
/// from it: nothing, in both directions.
#[must_use]
pub const fn needs_from_sibling() -> usize {
    0
}

/// Whether anything in the wave lists this entry as a dependency.
///
/// Nothing does. It is a leaf, so the traffic is one-way.
#[must_use]
pub const fn is_leaf() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravel_sdk::content::{RegistrationRefused, RegistryKey};
    use rvl_321_counterpart::model::Crafted;

    #[derive(Default)]
    struct Accepting {
        claimed: Vec<RegistryKey>,
    }

    impl Registrar for Accepting {
        fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
            self.claimed.push(key);
            Ok(())
        }

        fn claimed(&self) -> &[RegistryKey] {
            &self.claimed
        }

        fn loads(&self, _mod_id: &ModId) -> bool {
            false
        }
    }

    #[test]
    fn the_declared_identity_is_the_one_the_package_carries() {
        assert_eq!(Entry::declared_id().as_str(), MOD_ID);
        assert_eq!(Entry::new().id().as_str(), MOD_ID);
        assert_eq!(__ravel_entry(Side::DedicatedServer).id().as_str(), MOD_ID);
    }

    #[test]
    fn the_entry_runs_on_both_sides_and_declares_two_hard_dependencies() {
        assert_eq!(SIDES.len(), 3);
        assert_eq!(DEPENDENCIES.len(), 2);
        assert_eq!(LOADS_AFTER, rvl_321_counterpart::MOD_ID);
        assert!(DEPENDENCIES.contains(&LOADS_AFTER));
        assert_eq!(
            DECLARED_SOFT_DEPENDENCIES, 0,
            "companion mods are discovered at runtime, never declared"
        );
    }

    #[test]
    fn a_fresh_entry_registers_all_six_crops_and_has_not_adjusted_them() {
        let entry = Entry::new();
        assert_eq!(entry.crops().len(), crops::COUNT);
        assert!(entry.crops().iter().all(|crop| crop.enabled));
        assert!(entry.crops().iter().all(|crop| crop.foundation.is_none()));
        assert!(!entry.adjusted());
        assert_eq!(entry.tier().value, tier::VALUE);
        assert_eq!(entry.tier().ground, None);
        assert_eq!(entry.settings(), &Settings::default());
    }

    #[test]
    fn the_adjust_pass_points_the_tier_at_content_and_every_crop_at_an_anchor() {
        let mut entry = Entry::new();
        entry.adjust();
        assert!(entry.adjusted());
        assert!(entry.tier().ground.is_some());
        assert!(entry.tier().material.is_some());
        assert!(!entry.tier().second_drop);
        for (crop, declared) in entry.crops().iter().zip(crops::DECLARED) {
            assert_eq!(
                crop.foundation.as_deref(),
                Some(format!("{MOD_ID}:{}", declared.anchor).as_str())
            );
        }
    }

    #[test]
    fn the_barrier_claims_every_block_and_item_the_census_counts_and_no_recipe() {
        let mut entry = Entry::new();
        let mut registrar = Accepting::default();
        entry
            .register(&mut registrar)
            .expect("the accepting registrar refuses nothing");
        assert_eq!(registrar.claimed().len(), census::BLOCKS + census::ITEMS);
        for (kind, expected) in [
            (ContentKind::Block, census::BLOCKS),
            (ContentKind::Item, census::ITEMS),
        ] {
            let claimed = registrar
                .claimed()
                .iter()
                .filter(|key| key.kind() == kind)
                .count();
            assert_eq!(claimed, expected);
        }
        assert!(
            registrar
                .claimed()
                .iter()
                .all(|key| CLAIMED_KINDS.contains(&key.kind())),
            "this entry owns no recipe kind of its own"
        );
        assert!(
            entry.adjusted(),
            "the adjust pass follows the register pass"
        );
    }

    #[test]
    fn every_block_is_claimed_before_any_item_and_block_items_come_first_among_items() {
        let declaration = Entry::declaration().expect("the packaged declaration is well formed");
        let entries = declaration.entries();
        let first_item = entries
            .iter()
            .position(|key| key.kind() == ContentKind::Item)
            .expect("the entry claims items");
        assert_eq!(first_item, census::BLOCKS);
        let item_paths: Vec<&str> = entries[first_item..]
            .iter()
            .map(|key| key.id().path())
            .collect();
        assert_eq!(item_paths, census::item_paths());
        assert_eq!(
            &item_paths[..census::BLOCK_ITEMS],
            census::block_item_paths().as_slice()
        );
    }

    #[test]
    fn the_declaration_carries_the_forty_eight_tag_documents_too() {
        let declaration = Entry::declaration().expect("the packaged declaration is well formed");
        assert_eq!(declaration.tags().count(), data::TAG_DOCUMENTS);
    }

    #[test]
    fn each_of_the_six_crops_is_crafted_from_a_material_of_its_own() {
        let entry = Entry::new();
        let mut ids: Vec<String> = entry
            .crops()
            .iter()
            .map(|crop| match &crop.crafted {
                Crafted::Entry { id, .. } | Crafted::Tag(id) => id.clone(),
                Crafted::Nothing => String::new(),
            })
            .collect();
        assert_eq!(ids.len(), crops::COUNT);
        assert!(
            ids.iter().all(|id| !id.is_empty()),
            "every one of the six names a material"
        );
        ids.sort();
        ids.dedup();
        assert_eq!(
            ids.len(),
            crops::COUNT,
            "the five shapes do not collapse to one name"
        );
    }

    #[test]
    fn a_tag_shaped_crop_names_a_tag_this_entry_declares_or_one_nobody_owns() {
        let entry = Entry::new();
        let declaration = Entry::declaration().expect("the packaged declaration is well formed");
        let mut shaped = 0;
        for crop in entry.crops() {
            let Crafted::Tag(id) = &crop.crafted else {
                continue;
            };
            shaped += 1;
            if id.starts_with(&format!("{MOD_ID}:")) {
                let tag = declaration
                    .tags()
                    .find(|tag| tag.id().to_string() == *id)
                    .expect("a tag this entry declares itself");
                assert_eq!(
                    declaration.tag_members(tag).len(),
                    crops::OPTIONAL_TAG_MEMBERS,
                    "the two optional spellings are the members"
                );
            } else {
                assert!(
                    id.starts_with(&format!("{CONVENTION_NAMESPACE}:")),
                    "a cross-mod tag is named where no mod owns the name"
                );
            }
        }
        assert_eq!(shaped, 2, "the convention tag and this entry's own");
    }

    #[test]
    fn a_short_tag_declaration_is_refused_rather_than_carried_to_the_barrier() {
        let declaration = Entry::declaration().expect("the packaged declaration is well formed");
        let owner = Entry::declared_id().clone();
        let blocks: Vec<RegistryKey> = declaration
            .entries()
            .iter()
            .filter(|key| key.kind() == ContentKind::Block)
            .cloned()
            .collect();
        let mut short = ContentDeclaration::new(owner);
        let refusal = data::declare_tags(&mut short, &blocks, &[]);
        assert!(
            refusal.is_err(),
            "a family with no members to state is a packaging failure"
        );
    }

    #[test]
    fn the_entry_declares_both_its_content_and_its_settings() {
        let entry = Entry::new();
        assert!(
            entry
                .content_declaration()
                .expect("valid content")
                .is_some()
        );
        assert!(
            entry
                .configuration_declaration()
                .expect("valid configuration")
                .is_some()
        );
    }

    #[test]
    fn the_lifecycle_runs_through_and_a_reload_is_the_one_signal_it_counts() {
        let mut entry = Entry::new();
        for state in LifecycleState::ALL {
            entry.enter(state).expect("the entry accepts every step");
        }
        assert_eq!(entry.entered(), LifecycleState::ALL);

        entry
            .observe(GameEvent::ResourcesReloaded)
            .expect("a reload is a signal this entry sees");
        assert_eq!(entry.reloads(), 1);
    }

    #[test]
    fn a_thousand_ticks_leave_the_entry_exactly_as_they_found_it() {
        let mut entry = Entry::new();
        entry.adjust();
        let before = format!("{entry:?}");
        for _ in 0..1000 {
            entry
                .observe(GameEvent::ServerTick)
                .expect("a tick is a signal this entry has nothing to do about");
            entry
                .observe(GameEvent::ClientTick)
                .expect("a tick is a signal this entry has nothing to do about");
        }
        assert_eq!(format!("{entry:?}"), before);
        assert_eq!(entry.reloads(), 0);
    }

    #[test]
    fn a_reload_changes_nothing_because_this_entry_owns_nothing_a_reload_rebuilds() {
        let mut entry = Entry::new();
        entry.adjust();
        let tier = entry.tier().clone();
        let crops = entry.crops().to_vec();
        for _ in 0..5 {
            entry
                .observe(GameEvent::ResourcesReloaded)
                .expect("a reload is a signal this entry sees");
        }
        assert_eq!(entry.reloads(), 5);
        assert_eq!(entry.tier(), &tier);
        assert_eq!(entry.crops(), crops.as_slice());
        assert_eq!(
            data::OWNS_PERSISTED_STATE,
            entry
                .saved_data_declaration()
                .expect("valid saved data declaration")
                .is_some(),
            "an entry that owns no world data declares none"
        );
    }

    #[test]
    fn this_entry_owns_no_saved_data_and_writes_no_envelope() {
        let entry = Entry::new();
        assert!(
            entry
                .saved_data_declaration()
                .expect("valid saved data declaration")
                .is_none()
        );
        assert!(
            entry
                .save_saved_data()
                .expect("an entry that owns nothing writes nothing")
                .is_none()
        );
    }

    #[test]
    fn what_this_entry_needs_is_narrower_than_the_shared_library_assumed() {
        assert_eq!(needs_from_shared().len(), 3);
        assert!(!needs_shared_persistence());
        assert_eq!(needs_from_tiered().len(), 7);
        assert_eq!(needs_from_sibling(), 0);
        assert!(is_leaf());
    }

    #[test]
    fn a_world_this_entry_deposited_into_carries_its_name() {
        let mut shaping = WorldShaping::default();
        Entry::shape_world(&mut shaping).expect("a legal mod id");
        assert_eq!(shaping.mods().len(), 1);
        assert_eq!(shaping.mods()[0].as_str(), MOD_ID);
    }
}
