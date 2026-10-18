#![forbid(unsafe_code)]

//! An original Ravel implementation of the behaviour specified in RVL-086.
//!
//! The client-presentation cluster's leaf. Nothing in the wave depends on it, so
//! the useful question about it is what it consumes and what it leaves behind in
//! a world, and the answer to the second is small and completely stated: one
//! boolean per attractor item and ordinary item damage. This package owns no
//! world-level saved data, no block entities and no server state, and
//! [`Entry::saved_data_declaration`] answering nothing is the test of that.
//!
//! What a reader reaches for:
//!
//! - [`settings`] — the twenty-eight settings, the seventeen flags, and the
//!   payload that makes the deciding group the server's to decide;
//! - [`matching`] — the expression three settings share;
//! - [`overrides`] — the repair-material table;
//! - [`repair`] — the crafting-grid repair and its integer arithmetic;
//! - [`refusal`] — the nearly-destroyed refusal and the ineffective list;
//! - [`attractor`] — the one item-level value, the one payload, and the sweep;
//! - [`harvest`] — the right-click crop harvest;
//! - [`effects`] — the food durations and the night-vision upkeep;
//! - [`census`] — what is registered, in what order, and what gates the listing;
//! - [`tooltip`] — what the two informational lines would say;
//! - [`dumps`] — the derived documents written into `generated/`.
//!
//! `NOTES.md` beside this file says which sections are carried here, which are
//! declarable only, and what every declared behaviour change is.

pub mod attractor;
pub mod census;
pub mod dumps;
pub mod effects;
pub mod harvest;
pub mod matching;
pub mod overrides;
pub mod refusal;
pub mod repair;
pub mod settings;
pub mod tooltip;

use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration};
use ravel_sdk::content::{ContentDeclaration, Registrar, claim_all};
use ravel_sdk::events::GameEvent;
use ravel_sdk::interception::InterceptionDeclaration;
use ravel_sdk::network::{PacketDelivery, PacketReader};
use ravel_sdk::prelude::*;
use rvl_042_counterpart::flags::FlagTable;

pub use crate::attractor::Toggle;
pub use crate::census::{Grouping, Tier};
pub use crate::effects::Upkeep;
pub use crate::matching::{Expression, Matcher};
pub use crate::overrides::Table;
pub use crate::refusal::{Playing, Surface};
pub use crate::repair::{Held, Shape};
pub use crate::settings::{Decisive, Settings};

/// The identity this package is declared and validated under.
pub const MOD_ID: &str = "rvl_086_counterpart";

/// The namespace this entry's own flags are published under.
pub const FLAG_NAMESPACE: &str = MOD_ID;

/// How many observed signals are kept for a reader to look at.
pub const OBSERVED_KEPT: usize = 64;

/// The namespace an override selector written without one is completed with.
///
/// The host's own namespace is not this package's to name, so it is a value the
/// loader supplies and this is the shipped placeholder until it does.
pub const HOST_NAMESPACE: &str = "host";

/// What this entry attaches after the registration barrier.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Observer {
    /// The night-vision upkeep, on every side.
    NightVisionUpkeep,
    /// The right-click crop harvest, on every side.
    CropHarvest,
    /// The nearly-destroyed refusal, on every side.
    NearlyDestroyed,
    /// The ineffective-tool rule, on every side.
    Ineffective,
    /// The inventory toggle that sends the one payload, on a client.
    InventoryToggle,
    /// The mining-speed tooltip, on a client.
    MiningSpeedTooltip,
    /// The ammunition tooltip, on a client.
    AmmunitionTooltip,
}

impl Observer {
    /// The four attached on every side, in the order the specification names.
    pub const EVERY_SIDE: [Self; 4] = [
        Self::NightVisionUpkeep,
        Self::CropHarvest,
        Self::NearlyDestroyed,
        Self::Ineffective,
    ];

    /// The three attached on a client only.
    pub const CLIENT_ONLY: [Self; 3] = [
        Self::InventoryToggle,
        Self::MiningSpeedTooltip,
        Self::AmmunitionTooltip,
    ];
}

/// What is attached on one side.
///
/// The original guards its client-only registrations with a build-time
/// environment test. Ravel declares a side instead, which is the safer form and
/// is observably the same.
#[must_use]
pub fn observers(side: Side) -> Vec<Observer> {
    let mut attached = Observer::EVERY_SIDE.to_vec();
    if side == Side::Client {
        attached.extend(Observer::CLIENT_ONLY);
    }
    attached
}

/// The entry, as the loader drives it.
#[derive(Debug)]
pub struct Entry {
    declaration: Result<ContentDeclaration, ModError>,
    settings: Settings,
    flags: FlagTable,
    grouping: Grouping,
    upkeep: Upkeep,
    entered: Vec<LifecycleState>,
    observed: Vec<GameEvent>,
    seen: u64,
    configured: bool,
}

impl Default for Entry {
    fn default() -> Self {
        Self::new()
    }
}

impl Entry {
    /// Builds the entry holding its shipped defaults and nothing resolved.
    #[must_use]
    pub fn new() -> Self {
        Self {
            declaration: Self::declaration(),
            settings: Settings::default(),
            flags: FlagTable::default(),
            grouping: Grouping::default(),
            upkeep: Upkeep::new(),
            entered: Vec::new(),
            observed: Vec::new(),
            seen: 0,
            configured: false,
        }
    }

    /// Returns the values in force.
    #[must_use]
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Returns the flags this entry published.
    #[must_use]
    pub const fn flags(&self) -> &FlagTable {
        &self.flags
    }

    /// Returns the creative grouping, which is empty until configuration.
    #[must_use]
    pub const fn grouping(&self) -> &Grouping {
        &self.grouping
    }

    /// Returns the transient night-vision upkeep.
    #[must_use]
    pub const fn upkeep(&self) -> &Upkeep {
        &self.upkeep
    }

    /// Returns the transient upkeep for a tick.
    pub const fn upkeep_mut(&mut self) -> &mut Upkeep {
        &mut self.upkeep
    }

    /// Returns the lifecycle steps this entry has taken.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }

    /// Returns the most recent signals this entry observed, oldest first.
    ///
    /// At most [`OBSERVED_KEPT`] of them: a server observes one signal a tick for
    /// as long as it runs, so keeping every one would be a list that only grows.
    /// The count is kept whole in [`Entry::seen`].
    #[must_use]
    pub fn observed(&self) -> &[GameEvent] {
        &self.observed
    }

    /// How many signals this entry has observed, kept or dropped.
    #[must_use]
    pub const fn seen(&self) -> u64 {
        self.seen
    }

    /// Whether the deciding settings have been resolved yet.
    #[must_use]
    pub const fn configured(&self) -> bool {
        self.configured
    }

    /// Adopts a server's deciding settings over whatever this side resolved.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when the creative grouping cannot be built
    /// over the flag names this package declares, which is a packaging failure.
    pub fn adopt(&mut self, decisive: &Decisive) -> Result<(), ModError> {
        self.settings.adopt(decisive, HOST_NAMESPACE);
        self.rebuild()
    }

    /// Returns what this entry claims at the registration barrier.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when a declared path is not a legal content
    /// path, which for these generated names would be a packaging failure.
    pub fn declaration() -> Result<ContentDeclaration, ModError> {
        let mut declaration = ContentDeclaration::new(Self::declared_id().clone());
        let mut items = Vec::with_capacity(census::BLOCKS + census::ITEMS);
        for (kind, name) in census::claims() {
            let key = declaration
                .entry(kind, &name)
                .map_err(|error| ModError::refused(error.to_string()))?;
            if kind == ContentKind::Item {
                items.push(key);
            }
        }
        declaration
            .entry(ContentKind::Recipe, repair::KIND)
            .map_err(|error| ModError::refused(error.to_string()))?;
        census::declare_tags(&mut declaration, &items)
            .map_err(|error| ModError::refused(error.to_string()))?;
        declaration
            .translation(
                census::LOCALE,
                census::GROUPING_TITLE_KEY,
                census::GROUPING_TITLE,
            )
            .map_err(|error| ModError::refused(error.to_string()))?;
        Ok(declaration)
    }

    fn rebuild(&mut self) -> Result<(), ModError> {
        settings::publish(&mut self.flags, &self.settings, FLAG_NAMESPACE)
            .map_err(|error| ModError::refused(error.to_string()))?;
        self.grouping = census::grouping(FLAG_NAMESPACE)
            .map_err(|error| ModError::refused(error.to_string()))?;
        self.configured = true;
        Ok(())
    }
}

#[declare_mod(id = "rvl_086_counterpart")]
impl Mod for Entry {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        if state == LifecycleState::Constructed {
            // The two declarations are parsed from packaged text, so a malformed
            // one is a packaging failure. Refusing it here is what stops the mod
            // launching with no settings and every deciding value at its default.
            settings::declaration().map_err(|error| ModError::refused(error.to_string()))?;
            self.declaration.as_ref().map_err(Clone::clone)?;
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
        settings::declaration()
            .map(Some)
            .map_err(|error| ModError::refused(error.to_string()))
    }

    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
        self.settings = Settings::read(resolved, HOST_NAMESPACE);
        self.rebuild()
    }

    fn interception_declaration(&self) -> Result<Option<InterceptionDeclaration>, ModError> {
        let mut declared = InterceptionDeclaration::new(Self::declared_id().clone());
        declared
            .answer_at(Point::BlockIsBroken.as_str())
            .answer_at(Point::ItemIsUsed.as_str());
        Ok(Some(declared))
    }

    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        self.seen = self.seen.saturating_add(1);
        if self.observed.len() == OBSERVED_KEPT {
            self.observed.remove(0);
        }
        self.observed.push(event);
        Ok(())
    }

    fn receive_packet(&mut self, delivery: &PacketDelivery) -> Result<(), ModError> {
        let contract =
            Decisive::contract().map_err(|error| ModError::refused(error.to_string()))?;
        if &delivery.id != contract.id()
            || delivery.direction != contract.direction()
            || delivery.side != Side::Client
        {
            return Ok(());
        }
        let mut reader = PacketReader::new(&delivery.payload);
        let decisive =
            Decisive::read(&mut reader).map_err(|error| ModError::refused(error.to_string()))?;
        reader
            .finish()
            .map_err(|error| ModError::refused(error.to_string()))?;
        self.adopt(&decisive)
    }

    fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, ModError> {
        // Section nine: this entry owns nothing world-level. Everything it stores
        // is item-level, and a counterpart that declared saved data here would
        // have added state the original does not have.
        Ok(None)
    }
}

export_mod!(Entry::new());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deferred_content_refusal_is_not_replaced_by_a_generic_message() {
        let original = ModError::refused("the original packaged content failure");
        let mut entry = Entry::new();
        entry.declaration = Err(original.clone());
        assert_eq!(
            entry
                .content_declaration()
                .expect_err("the declaration refused"),
            original
        );
        assert_eq!(entry.enter(LifecycleState::Constructed), Err(original));
        assert_eq!(entry.entered(), []);
    }

    #[test]
    fn the_declared_identity_is_the_one_the_package_carries() {
        assert_eq!(Entry::declared_id().as_str(), MOD_ID);
        assert_eq!(Entry::new().id().as_str(), MOD_ID);
        assert_eq!(__ravel_entry(Side::DedicatedServer).id().as_str(), MOD_ID);
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
                .expect("valid settings")
                .is_some()
        );
    }

    #[test]
    fn this_entry_owns_no_world_level_saved_data() {
        assert!(
            Entry::new()
                .saved_data_declaration()
                .expect("valid declaration")
                .is_none()
        );
        assert!(
            Entry::new()
                .saved_data_migrations()
                .expect("valid migrations")
                .is_none()
        );
    }

    #[test]
    fn four_observers_are_attached_on_every_side_and_three_more_on_a_client() {
        assert_eq!(observers(Side::DedicatedServer).len(), 4);
        assert_eq!(observers(Side::IntegratedServer).len(), 4);
        assert_eq!(observers(Side::Client).len(), 7);
        for attached in Observer::CLIENT_ONLY {
            assert!(!observers(Side::DedicatedServer).contains(&attached));
            assert!(observers(Side::Client).contains(&attached));
        }
    }

    #[test]
    fn every_registered_entry_is_listed_and_each_flag_changes_what_is_shown() {
        let mut entry = Entry::new();
        entry
            .adopt(&Settings::default().decisive())
            .expect("the packaged flag names are legal");

        let listed = entry.grouping().declared().len();
        assert_eq!(listed, census::BLOCKS + census::ITEMS);
        assert_eq!(entry.grouping().shown(entry.flags()).len(), listed);

        for (flag, key) in settings::FLAG_BINDINGS {
            let slot = settings::SWITCH_KEYS
                .iter()
                .position(|switch| *switch == key)
                .expect("every bound flag names a switch");
            let mut off = Settings::default();
            off.switches[slot] = false;
            let mut entry = Entry::new();
            entry
                .adopt(&off.decisive())
                .expect("the packaged flag names are legal");
            assert!(
                entry.grouping().shown(entry.flags()).len() < listed,
                "{flag} gates something"
            );
            assert_eq!(
                entry.grouping().declared().len(),
                listed,
                "{flag} unlists rather than unregisters"
            );
        }
    }

    #[test]
    fn a_fresh_entry_holds_the_shipped_defaults_and_has_resolved_nothing() {
        let entry = Entry::new();
        assert_eq!(entry.settings(), &Settings::default());
        assert!(!entry.configured());
        assert!(entry.upkeep().is_empty());
        assert_eq!(entry.grouping().declared().len(), 0);
    }

    #[test]
    fn adopting_a_servers_answers_republishes_the_flags_over_them() {
        let server = Settings {
            switches: [false; 14],
            ..Settings::default()
        };
        let mut entry = Entry::new();
        entry
            .adopt(&server.decisive())
            .expect("the packaged flag names are legal");
        assert!(entry.configured());
        for (flag, _) in settings::FLAG_BINDINGS {
            let id = rvl_042_counterpart::flags::FlagId::new(FLAG_NAMESPACE, flag)
                .expect("a legal identity");
            assert!(!entry.flags().enabled(&id), "{flag} follows its switch");
        }
    }

    #[test]
    fn a_delivery_for_another_contract_is_left_alone_rather_than_decoded() {
        let mut entry = Entry::new();
        let other = Toggle::contract().expect("a legal name");
        let delivery = PacketDelivery::new(
            other.id().clone(),
            other.direction(),
            Side::DedicatedServer,
            Vec::new(),
        );
        entry
            .receive_packet(&delivery)
            .expect("a delivery this entry does not own changes nothing");
        assert!(!entry.configured());
    }

    #[test]
    fn a_decisive_frame_arriving_at_a_server_is_left_alone() {
        let contract = Decisive::contract().expect("a legal name");
        let mut entry = Entry::new();
        let delivery = PacketDelivery::new(
            contract.id().clone(),
            contract.direction(),
            Side::DedicatedServer,
            settings::to_bytes(&|writer| Settings::default().decisive().write(writer)),
        );
        entry
            .receive_packet(&delivery)
            .expect("a frame for the wrong side changes nothing");
        assert!(
            !entry.configured(),
            "a server keeps the settings it decided"
        );
    }

    #[test]
    fn the_observed_signals_are_kept_to_a_ceiling_while_the_count_stays_whole() {
        let mut entry = Entry::new();
        for _ in 0..OBSERVED_KEPT * 2 {
            entry
                .observe(GameEvent::ServerTick)
                .expect("the entry records what it observes");
        }
        assert_eq!(entry.observed().len(), OBSERVED_KEPT);
        assert_eq!(
            entry.seen(),
            u64::try_from(OBSERVED_KEPT * 2).expect("the observation ceiling fits in u64")
        );
    }

    #[test]
    fn an_observed_signal_is_recorded_and_the_lifecycle_runs_through() {
        let mut entry = Entry::new();
        for state in LifecycleState::ALL {
            entry.enter(state).expect("the entry accepts every step");
        }
        entry
            .observe(GameEvent::ServerTick)
            .expect("the entry records what it observes");
        assert_eq!(entry.entered(), LifecycleState::ALL);
        assert_eq!(entry.observed(), [GameEvent::ServerTick]);
    }

    #[test]
    fn the_two_interception_points_ravel_publishes_are_the_two_declared() {
        let declared = Entry::new()
            .interception_declaration()
            .expect("valid interception")
            .expect("the entry declares interception");
        assert!(declared.answers_at(Point::BlockIsBroken));
        assert!(declared.answers_at(Point::ItemIsUsed));
        assert!(!declared.answers_at(Point::WorldIsSaved));
    }
}
