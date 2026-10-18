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
        let tier = tier::registered(
            format!("{MOD_ID}:{}", tier::PATH)
                .parse()
                .expect("the declared tier has a legal identity"),
        );
        let crops = crops::DECLARED
            .iter()
            .map(|declared| {
                declared.crop(
                    ContentId::new(Self::declared_id().clone(), declared.path)
                        .expect("the declared crop has a legal path"),
                    &tier.id,
                    declared
                        .material_id(MOD_ID)
                        .parse()
                        .expect("a legal material identity"),
                )
            })
            .collect();
        Self {
            declaration: Self::declaration(),
            configuration: settings::declaration().map_err(ModError::from_display),
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
            ContentId::new(Self::declared_id().clone(), "ground").expect("a legal ground path"),
            ContentId::new(Self::declared_id().clone(), "material").expect("a legal material path"),
            self.settings.crops_fertilisable,
        );
        for (crop, declared) in self.crops.iter_mut().zip(crops::DECLARED) {
            crop.foundation = Some(
                ContentId::new(Self::declared_id().clone(), declared.anchor)
                    .expect("a legal foundation path"),
            );
        }
        self.adjusted = true;
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
                    .map_err(ModError::from_display)?,
            );
        }
        let mut items = Vec::with_capacity(census::ITEMS);
        for path in census::item_paths() {
            items.push(
                declaration
                    .entry(ContentKind::Item, &path)
                    .map_err(ModError::from_display)?,
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

    fn content_declaration(
        &self,
    ) -> Result<Option<std::borrow::Cow<'_, ContentDeclaration>>, ModError> {
        self.declaration
            .as_ref()
            .map(std::borrow::Cow::Borrowed)
            .map(Some)
            .map_err(Clone::clone)
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
        self.settings = Settings::read(resolved).map_err(ModError::from_display)?;
        settings::publish(&mut self.flags, &self.settings).map_err(ModError::from_display)?;
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
                crop.foundation,
                Some(
                    format!("{MOD_ID}:{}", declared.anchor)
                        .parse()
                        .expect("the declared anchor is an identifier")
                )
            );
        }
    }

    #[test]
    fn the_registration_barrier_runs_the_adjust_pass() {
        let mut entry = Entry::new();
        let mut registrar = Accepting::default();
        entry
            .register(&mut registrar)
            .expect("the accepting registrar refuses nothing");
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
                Crafted::Entry { id, .. } | Crafted::Tag(id) => id.to_string(),
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
            if id.namespace() == Entry::declared_id() {
                let tag = declaration
                    .tags()
                    .find(|tag| tag.id() == id)
                    .expect("a tag this entry declares itself");
                assert_eq!(
                    declaration.tag_members(tag).len(),
                    crops::OPTIONAL_TAG_MEMBERS,
                    "the two optional spellings are the members"
                );
            } else {
                assert!(
                    id.namespace().as_str() == CONVENTION_NAMESPACE,
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
    fn a_world_this_entry_deposited_into_carries_its_name() {
        let mut shaping = WorldShaping::default();
        world::shaping(&mut shaping);
        assert_eq!(shaping.mods().len(), 1);
        assert_eq!(shaping.mods()[0].as_str(), MOD_ID);
    }
}
