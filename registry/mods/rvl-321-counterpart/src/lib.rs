#![forbid(unsafe_code)]

//! An original Ravel implementation of the behaviour specified in RVL-321.
//!
//! One loop: a player plants a crop, the crop matures on tiered ground, and
//! harvesting it yields a material that would otherwise have to be mined,
//! killed for or traded for. Around that loop sit tiers of material and ground,
//! powered machines, two rituals, slotted modifiers and a catalogue of crops.
//! The catalogue is not carried here — every one of its names belongs to
//! somebody else — and the machinery that carries a catalogue is.
//!
//! What a dependent reaches for:
//!
//! - [`settings`] — the twenty settings and the flags fourteen of them publish;
//! - [`registries`] — the three tables, their refusal rules and the ordering;
//! - [`model`] — the crop, tier and crop-kind model with its defaults;
//! - [`growth`] — the four gates and the four ways a crop grows;
//! - [`harvest`] — what a harvest yields, and the second-drop arithmetic;
//! - [`ground`] — moisture, drops and conversion of the tiered ground;
//! - [`recipes`] — the recipe and ingredient kinds, and the derivation per crop;
//! - [`machines`] — the upgrade table, the energy model and every machine;
//! - [`cache`] — item-level data, the four payloads and the ingredient index;
//! - [`world`] — the three deposits and the substitution inside the third.
//!
//! `NOTES.md` beside this file says which sections are carried here and which
//! are left declarable only because Ravel publishes no seam for them.

pub mod cache;
pub mod dumps;
pub mod ground;
pub mod growth;
pub mod harvest;
pub mod machines;
pub mod model;
pub mod recipes;
pub mod registries;
pub mod settings;
pub mod world;

use std::collections::BTreeMap;

use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration};
use ravel_sdk::content::{ContentDeclaration, ContentId, ContentKind, Registrar, claim_all};
use ravel_sdk::events::GameEvent;
use ravel_sdk::prelude::*;
use ravel_sdk::save::{SavedDataDeclaration, SavedDataEnvelope, SavedDataPlan};
use ravel_sdk::services::{ServiceDeclaration, ServiceId, ServiceRequest, ServiceResponse};
use ravel_sdk::world_shaping::WorldShaping;
use ravel_sdk::{JsonValue, json};
use rvl_042_counterpart::flags::FlagTable;

pub use crate::model::{ChargeType, Crafted, Crop, CropKind, Tier};
pub use crate::registries::{Extension, Registries, Step};
pub use crate::settings::Settings;

/// The identity this package is declared and validated under.
pub const MOD_ID: &str = "rvl_321_counterpart";

/// The eight recipe kinds this entry contributes.
///
/// Six are its own machine and ritual kinds and two are crafting-grid kinds.
/// Ravel claims a recipe kind as a registry entry and decides nothing about
/// what one does, so these are names; the matching lives in [`recipes`].
pub const RECIPE_KINDS: [&str; 8] = [
    "ritual_four_station",
    "ritual_eight_station",
    "enchanting",
    "reprocessing",
    "charge_extraction",
    "creature_summoning",
    "tilling",
    "emptying",
];

/// The three states a crop can be in, which are deliberately not two.
///
/// "Disabled" means unobtainable rather than absent: the identifiers still
/// exist, so a world holding them loads. A crop whose crafting material
/// resolves to nothing is a third state again, enabled and registered with only
/// its seed recipes missing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Availability {
    /// Registered, listed, and contributing every recipe.
    Whole,
    /// Registered and enabled, with no seed recipe because nothing resolves.
    WithoutSeedRecipes,
    /// Registered, hidden from the listing, contributing no recipe.
    Unobtainable,
}

/// Which state a crop is in.
///
/// `companions` is how many of the crop's declared companion mods are loaded. A
/// development environment enables every crop regardless, which is why that is
/// a parameter rather than a lookup.
#[must_use]
pub fn availability(
    crop: &Crop,
    companions_loaded: usize,
    declares_companions: bool,
    development: bool,
    resolves: &dyn Fn(&str) -> usize,
) -> Availability {
    let enabled = crop.enabled && (development || !declares_companions || companions_loaded > 0);
    if !enabled {
        return Availability::Unobtainable;
    }
    if crop.crafted.resolves(resolves) {
        Availability::Whole
    } else {
        Availability::WithoutSeedRecipes
    }
}

/// The identifier the extension-contribution service answers under.
///
/// A service is namespaced by the mod that answers it, so this is the one name
/// an extension calls to reach these registries. That is the seam that stands
/// in for §4's annotation scan: the loader reads both declarations, orders this
/// entry before anything that consumes the service, and refuses a set the
/// wiring does not resolve — so a contribution always arrives after this
/// entry's own, which is the ordering §4 makes observable.
pub const CONTRIBUTION_SERVICE: &str = "registries";

/// The entry, as the loader drives it.
#[derive(Debug)]
pub struct Entry {
    declaration: Result<ContentDeclaration, ModError>,
    configuration: Result<ConfigurationDeclaration, ModError>,
    settings: Settings,
    flags: FlagTable,
    registries: Registries,
    cache: cache::Cache,
    colours: cache::Colours,
    declared_colours: Vec<(String, u32)>,
    machines: BTreeMap<String, machines::Persisted>,
    reloads: u32,
    entered: Vec<LifecycleState>,
}

impl Default for Entry {
    fn default() -> Self {
        Self::new()
    }
}

impl Entry {
    /// Builds the entry holding its shipped defaults and its six tiers.
    #[must_use]
    pub fn new() -> Self {
        let mut registries = Registries::new();
        for tier in model::shipped_tiers() {
            registries.register_tier(tier);
        }
        for kind in model::shipped_kinds() {
            registries.register_kind(kind);
        }
        Self {
            declaration: Self::declaration(),
            configuration: settings::declaration().map_err(|error| ModError::refused(error.to_string())),
            settings: Settings::default(),
            flags: FlagTable::default(),
            registries,
            cache: cache::Cache::new(),
            colours: cache::Colours::default(),
            declared_colours: Vec::new(),
            machines: BTreeMap::new(),
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

    /// The three registries.
    #[must_use]
    pub const fn registries(&self) -> &Registries {
        &self.registries
    }

    /// The three registries, for an extension to contribute to.
    pub const fn registries_mut(&mut self) -> &mut Registries {
        &mut self.registries
    }

    /// The ingredient index everything sided consults.
    #[must_use]
    pub const fn cache(&self) -> &cache::Cache {
        &self.cache
    }

    /// The ingredient index, for a data load to rebuild.
    pub const fn cache_mut(&mut self) -> &mut cache::Cache {
        &mut self.cache
    }

    /// The colours a data pack declared.
    #[must_use]
    pub const fn colours(&self) -> &cache::Colours {
        &self.colours
    }

    /// The colours, for a reload to rebuild.
    pub const fn colours_mut(&mut self) -> &mut cache::Colours {
        &mut self.colours
    }

    /// The lifecycle steps this entry has taken.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }

    /// How many reloads this entry has rebuilt itself for.
    #[must_use]
    pub const fn reloads(&self) -> u32 {
        self.reloads
    }

    /// The colours a data load parsed, held so a reload can rebuild from them.
    ///
    /// §13's colour map is cleared and rebuilt on each reload rather than
    /// accumulated, so what a reload rebuilds *from* is what a data load last
    /// declared and nothing older.
    pub fn declare_colours(&mut self, declared: Vec<(String, u32)>) {
        self.declared_colours = declared;
    }

    /// Rebuilds what a reload rebuilds.
    ///
    /// The colour map comes back from the last declared set, and the ingredient
    /// index is emptied: a client's index is only ever what a payload delivered,
    /// so a reload leaves it waiting for the next one rather than serving the
    /// index a previous load built.
    pub fn reload(&mut self) {
        self.colours.rebuild(self.declared_colours.iter().cloned());
        self.cache = cache::Cache::new();
        self.reloads = self.reloads.saturating_add(1);
    }

    /// One machine's persisted state, by the name the world knows it under.
    #[must_use]
    pub fn machine(&self, name: &str) -> Option<&machines::Persisted> {
        self.machines.get(name)
    }

    /// Records a machine's state, replacing whatever that name held.
    pub fn hold_machine(&mut self, name: &str, held: machines::Persisted) {
        self.machines.insert(name.to_owned(), held);
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

    /// The service an extension contributes to these registries through.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when the constant is not a legal content
    /// path, which for a constant is a packaging failure.
    pub fn contribution_service() -> Result<ServiceId, ModError> {
        Ok(ServiceId::new(
            ContentId::new(Self::declared_id().clone(), CONTRIBUTION_SERVICE)
                .map_err(|error| ModError::refused(error.to_string()))?,
        ))
    }

    fn contribute(&mut self, arguments: &JsonValue) -> JsonValue {
        let before = self.registries.refused().len();
        let id = arguments.get("id").and_then(JsonValue::as_str);
        let Some(id) = id else {
            return json!({"registered": false, "refused": "a contribution names no identifier"});
        };
        let value = arguments
            .get("value")
            .and_then(JsonValue::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(0);
        let strings = |key: &str| -> Vec<String> {
            arguments
                .get(key)
                .and_then(JsonValue::as_array)
                .map_or_else(Vec::new, |held| {
                    held.iter()
                        .filter_map(|entry| entry.as_str().map(str::to_owned))
                        .collect()
                })
        };
        let registered = match arguments.get("kind").and_then(JsonValue::as_str) {
            Some("tier") => self.registries.register_tier(Tier::new(id, value)),
            Some("crop_kind") => {
                let stem = arguments
                    .get("stem")
                    .and_then(JsonValue::as_str)
                    .unwrap_or(id);
                self.registries.register_kind(CropKind::new(id, stem))
            }
            Some("modifier") => {
                let kinds = strings("kinds");
                let borrowed: Vec<&str> = kinds.iter().map(String::as_str).collect();
                self.registries
                    .register_modifier(registries::Modifier::new(id, value, &borrowed))
            }
            Some("charge_type") => {
                let requirement = arguments
                    .get("requirement")
                    .and_then(JsonValue::as_f64)
                    .unwrap_or(0.0);
                let mut charge = ChargeType::new(id, requirement, 255, 255, 255);
                for creature in strings("creatures") {
                    charge.claim(&creature);
                }
                self.registries.register_charge(charge)
            }
            _ => {
                return json!({
                    "registered": false,
                    "refused": "a crop is registered inside the window rather than through the service",
                });
            }
        };
        let refused = self
            .registries
            .refused()
            .get(before)
            .map(ToString::to_string);
        json!({"registered": registered, "refused": refused})
    }

    /// What this entry claims at the registration barrier.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when a declared name is not a legal
    /// content path, which for these constants is a packaging failure.
    pub fn declaration() -> Result<ContentDeclaration, ModError> {
        let mut declaration = ContentDeclaration::new(Self::declared_id().clone());
        for kind in RECIPE_KINDS {
            declaration
                .entry(ContentKind::Recipe, kind)
                .map_err(|error| ModError::refused(error.to_string()))?;
        }
        Ok(declaration)
    }
}

#[declare_mod(id = "rvl_321_counterpart")]
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
        claim_all(declaration, registrar)
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
                "{published} of {declared} flags published, and the creative listing is gated on all of them"
            )));
        }
        Ok(())
    }

    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        match event {
            GameEvent::ResourcesReloaded => self.reload(),
            GameEvent::ServerTick | GameEvent::ClientTick => {}
        }
        Ok(())
    }

    fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
        let service = Self::contribution_service()?;
        ServiceDeclaration::new(Self::declared_id().clone(), [service], [])
            .map(Some)
            .map_err(|error| ModError::refused(error.to_string()))
    }

    fn answer_service(&mut self, request: &ServiceRequest) -> Result<ServiceResponse, ModError> {
        let service = Self::contribution_service()?;
        if request.id != service {
            return Err(ModError::refused("this entry answers one service"));
        }
        Ok(ServiceResponse::new(self.contribute(&request.arguments)))
    }

    fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, ModError> {
        SavedDataDeclaration::new(Self::declared_id().clone(), [machines::DATA_VERSION])
            .map(Some)
            .map_err(|error| ModError::refused(error.to_string()))
    }

    fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, ModError> {
        let machines: JsonValue = self
            .machines
            .iter()
            .map(|(name, held)| (name.clone(), held.save(JsonValue::Null)))
            .collect();
        Ok(Some(SavedDataEnvelope::current(
            Self::declared_id().clone(),
            machines::DATA_VERSION,
            json!({ "machines": machines }),
        )))
    }

    fn open_saved_data(
        &mut self,
        envelope: Option<&SavedDataEnvelope>,
        plan: &SavedDataPlan,
    ) -> Result<(), ModError> {
        let Some(envelope) = envelope else {
            return Ok(());
        };
        if !plan.steps().is_empty() {
            return Err(ModError::refused(
                "this build declares one saved-data version and so crosses no step",
            ));
        }
        if envelope.data_version() != machines::DATA_VERSION {
            return Err(ModError::refused(
                "the world stored a version this build does not read",
            ));
        }
        self.machines.clear();
        let Some(held) = envelope
            .payload()
            .get("machines")
            .and_then(JsonValue::as_object)
        else {
            return Ok(());
        };
        for (name, payload) in held {
            self.machines
                .insert(name.clone(), machines::Persisted::load(payload));
        }
        Ok(())
    }
}

export_mod!(Entry::new());

#[cfg(test)]
mod tests {
    use super::*;
    use ravel_sdk::content::{RegistrationRefused, RegistryKey};

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
    fn every_recipe_kind_is_claimed_once_at_the_barrier() {
        let mut entry = Entry::new();
        let mut registrar = Accepting::default();
        entry
            .register(&mut registrar)
            .expect("the accepting registrar refuses nothing");
        assert_eq!(registrar.claimed().len(), RECIPE_KINDS.len());
        assert!(
            registrar
                .claimed()
                .iter()
                .all(|key| key.kind() == ContentKind::Recipe)
        );

        let mut sorted = RECIPE_KINDS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 8);
    }

    #[test]
    fn a_fresh_entry_holds_its_six_tiers_two_kinds_and_an_empty_cache() {
        let entry = Entry::new();
        assert_eq!(entry.registries().tiers().len(), 6);
        assert_eq!(entry.registries().kinds().len(), 2);
        assert_eq!(entry.registries().crops().len(), 0);
        assert!(!entry.registries().is_open());
        assert!(entry.cache().is_empty());
        assert!(entry.colours().is_empty());
        assert_eq!(entry.settings(), &Settings::default());
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
    fn the_lifecycle_runs_through_and_a_reload_rebuilds_what_a_reload_rebuilds() {
        let mut entry = Entry::new();
        for state in LifecycleState::ALL {
            entry.enter(state).expect("the entry accepts every step");
        }
        assert_eq!(entry.entered(), LifecycleState::ALL);

        entry.declare_colours(vec![("test:one".to_owned(), 0x00_FF_00)]);
        entry
            .observe(GameEvent::ResourcesReloaded)
            .expect("a reload is a signal this entry acts on");
        assert_eq!(entry.reloads(), 1);
        assert_eq!(entry.colours().of("test:one"), 0x00_FF_00);

        entry.declare_colours(Vec::new());
        entry
            .observe(GameEvent::ResourcesReloaded)
            .expect("a reload is a signal this entry acts on");
        assert!(
            entry.colours().is_empty(),
            "the map is rebuilt from the last load rather than accumulated"
        );
    }

    #[test]
    fn a_thousand_ticks_leave_the_entry_exactly_as_they_found_it() {
        let mut entry = Entry::new();
        entry.declare_colours(vec![("test:one".to_owned(), 1)]);
        entry.reload();
        let before = format!("{entry:?}");
        for _ in 0..1000 {
            entry
                .observe(GameEvent::ServerTick)
                .expect("a tick is a signal this entry has nothing to do about");
            entry
                .observe(GameEvent::ClientTick)
                .expect("a tick is a signal this entry has nothing to do about");
        }
        assert_eq!(
            format!("{entry:?}"),
            before,
            "two thousand ticks add nothing to the entry"
        );
        assert_eq!(entry.reloads(), 1);
    }

    #[test]
    fn a_machine_brings_its_upgrade_and_every_multiplier_back_across_a_reload() {
        let mut entry = Entry::new();
        entry.hold_machine(
            "harvester",
            machines::Persisted {
                progress: 11,
                energy: machines::Energy {
                    amount: 4321,
                    capacity: machines::UPGRADES[3].capacity(),
                    remaining: 40,
                    total: 2000,
                },
                upgrade: Some(4),
                cursor: Some(23),
            },
        );
        let saved = entry
            .save_saved_data()
            .expect("the entry writes its machines")
            .expect("a declared owner writes an envelope");

        let mut reopened = Entry::new();
        let plan = SavedDataPlan::open(
            &reopened
                .saved_data_declaration()
                .expect("the saved data declaration is valid")
                .expect("the entry declares its saved data"),
            Some(saved.data_version()),
        )
        .expect("the world stored the version this build writes");
        reopened
            .open_saved_data(Some(&saved), &plan)
            .expect("the world opens");

        let held = reopened
            .machine("harvester")
            .expect("the machine came back");
        let upgrade = held.upgrade().expect("the position names an upgrade");
        assert_eq!(held.cursor, Some(23), "the sweep resumes where it left off");
        assert_eq!(upgrade.capacity(), 400_000);
        assert_eq!(upgrade.usage_per_tick(machines::HARVESTING.usage), 640);
        assert_eq!(upgrade.operation_ticks(machines::HARVESTING.operation), 12);
    }

    #[test]
    fn an_extension_contributes_through_the_service_and_a_collision_comes_back_named() {
        let mut entry = Entry::new();
        let service = Entry::contribution_service().expect("a legal service name");
        assert_eq!(
            entry
                .service_declaration()
                .expect("the service declaration is valid")
                .expect("the entry declares the service it answers")
                .provides()
                .count(),
            1
        );

        let answer = entry
            .answer_service(&ServiceRequest::new(
                service.clone(),
                json!({"kind": "modifier", "id": "other:reach", "value": 2, "kinds": ["tool"]}),
            ))
            .expect("the entry answers its own service");
        assert_eq!(answer.value().get("registered"), Some(&json!(true)));
        assert_eq!(entry.registries().modifiers().len(), 1);

        let again = entry
            .answer_service(&ServiceRequest::new(
                service,
                json!({"kind": "modifier", "id": "other:reach", "value": 2, "kinds": ["tool"]}),
            ))
            .expect("the entry answers its own service");
        assert_eq!(again.value().get("registered"), Some(&json!(false)));
        assert!(
            again.value()["refused"]
                .as_str()
                .is_some_and(|said| said.contains("other:reach")),
            "the refusal names what was refused"
        );
    }

    #[test]
    fn a_crop_whose_companions_are_absent_is_unobtainable_rather_than_missing() {
        let crop = Crop::new(
            "test:crop",
            "test:tier",
            "test:kind",
            Crafted::Entry {
                id: "other:material".to_owned(),
                data: None,
            },
        );
        assert_eq!(
            availability(&crop, 0, true, false, &|_| 1),
            Availability::Unobtainable
        );
        assert_eq!(
            availability(&crop, 0, true, true, &|_| 1),
            Availability::Whole,
            "a development environment enables every crop"
        );
        assert_eq!(
            availability(&crop, 1, true, false, &|_| 1),
            Availability::Whole
        );
    }

    #[test]
    fn a_material_that_resolves_to_nothing_is_the_third_state_and_not_the_second() {
        let mut crop = Crop::new(
            "test:crop",
            "test:tier",
            "test:kind",
            Crafted::Tag("test:empty".to_owned()),
        );
        assert_eq!(
            availability(&crop, 0, false, false, &|_| 0),
            Availability::WithoutSeedRecipes
        );
        crop.enabled = false;
        assert_eq!(
            availability(&crop, 0, false, false, &|_| 0),
            Availability::Unobtainable
        );
    }
}
