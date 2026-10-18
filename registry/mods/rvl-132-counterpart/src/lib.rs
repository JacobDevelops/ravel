#![forbid(unsafe_code)]

//! An original Ravel implementation of the behaviour specified in RVL-132.
//!
//! A bridging library with no gameplay of its own: it ships no block, no item,
//! no recipe document, no texture and no screen, and a player can encounter it
//! only indirectly, because it makes vanilla containers, stations and dropped
//! items answer questions that only modded machines ask.
//!
//! Section 1.1 says what survives as counterpart work and this package is that
//! list: the three ingredient kinds and their matching, ordering and hashing
//! ([`kinds`], [`data`]), the serialised forms ([`forms`]), the adapter table
//! between store shapes ([`storage`]), the attachment table ([`capabilities`]),
//! and the recipe views ([`recipes`]).
//!
//! **The interface vocabulary is not here.** Section five's abstraction, its
//! registry, the matcher and serialiser interfaces, the candidate set and the
//! candidate value, and the slot-addressed and slotless storage abstractions are
//! all owned by RVL-210's counterpart by the wave's decision. This package
//! depends on `rvl-210-counterpart` and consumes them from its crate root, and
//! redeclares none of them. `NOTES.md` records that ownership.
//!
//! What a reader reaches for:
//!
//! - [`kinds`] — the three ingredient kinds, their conditions and their order;
//! - [`data`] — the shared data comparison a pack-visible setting changes;
//! - [`forms`] — the tag forms an instance and a condition are stored in;
//! - [`storage`] — the twelve store adapters and what they must not lose;
//! - [`capabilities`] — the two compatibility bundles and 47 attachments;
//! - [`readings`] — the work and heat readings;
//! - [`recipes`] — the three recipe views and the brewing closure;
//! - [`holdings`] — the two item stores that live inside items;
//! - [`cache`] — the three caches and the reload that clears them;
//! - [`settings`] — the settings document and the two it declines;
//! - [`census`] — what the pinned entry counts;
//! - [`dumps`] — the generated documents those tables render to.

pub mod cache;
pub mod capabilities;
pub mod census;
pub mod data;
pub mod dumps;
pub mod forms;
pub mod holdings;
pub mod kinds;
pub mod readings;
pub mod recipes;
pub mod settings;
pub mod storage;

use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration};
use ravel_sdk::content::ContentDeclaration;
use ravel_sdk::events::GameEvent;
use ravel_sdk::prelude::*;

use rvl_210_counterpart::capabilities::Registry;

use crate::cache::{Caches, Point};
use crate::capabilities::Bundle;
use crate::data::Comparator;
use crate::forms::Serialisers;
use crate::kinds::{Fluid, Kinds, Scalar, Stackable};
use crate::settings::Settings;
use crate::storage::Adapter;

/// The identity this package is declared and validated under.
pub const MOD_ID: &str = "rvl_132_counterpart";

/// How far through section two's ordering the entry has got.
///
/// Section two names two orderings that are load-bearing: the adapters are
/// installed after common setup, and the serialisers before registry
/// population. A dependent that asks for a storage adapter during its own setup
/// has no entry to read, and this is what makes that a refusal rather than a
/// silently empty table.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Step {
    /// Nothing has run.
    #[default]
    Constructed,
    /// The two serialisers are installed.
    SerialisersInstalled,
    /// The three kinds are registered.
    KindsRegistered,
    /// Every registered kind is subscribed to capability gathering.
    SetUp,
    /// The storage-adapter table is populated.
    AdaptersInstalled,
}

/// What this entry refuses.
#[derive(Clone, Debug, PartialEq)]
pub enum Refused {
    /// A step ran before the step section two puts before it.
    OutOfOrder {
        /// What was asked for.
        wanted: Step,
        /// How far the entry had got.
        reached: Step,
    },
    /// The kind registry refused.
    Kinds(kinds::NotRegistered),
    /// A compatibility bundle refused.
    Bundle(capabilities::NotInstalled),
    /// A settings resolution refused.
    Settings(settings::Refused),
    /// The serialiser table refused.
    Serialisers(forms::TagError),
}

impl core::fmt::Display for Refused {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OutOfOrder { wanted, reached } => write!(
                formatter,
                "{wanted:?} was asked for at {reached:?}, which is out of the order section two states"
            ),
            Self::Kinds(refused) => refused.fmt(formatter),
            Self::Bundle(refused) => refused.fmt(formatter),
            Self::Settings(refused) => refused.fmt(formatter),
            Self::Serialisers(refused) => refused.fmt(formatter),
        }
    }
}

impl std::error::Error for Refused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Kinds(error) => Some(error),
            Self::Bundle(error) => Some(error),
            Self::Settings(error) => Some(error),
            Self::Serialisers(error) => Some(error),
            Self::OutOfOrder { .. } => None,
        }
    }
}

/// The entry, as the loader drives it.
#[derive(Debug)]
pub struct Entry {
    declaration: ContentDeclaration,
    settings: Settings,
    comparator: Comparator,
    serialisers: Serialisers,
    kinds: Kinds,
    adapters: Vec<Adapter>,
    attachments: Registry,
    bundle_failures: Vec<capabilities::NotInstalled>,
    caches: Caches,
    step: Step,
    entered: Vec<LifecycleState>,
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
        let settings = Settings::shipped();
        // The shipped list is empty, so the only comparator that can refuse is
        // one built from a list a caller assigned.
        let comparator = settings.comparator().unwrap_or_default();
        Self {
            declaration: Self::declaration(),
            settings,
            comparator,
            serialisers: Serialisers::new(),
            kinds: Kinds::new(),
            adapters: Vec::new(),
            attachments: Registry::new(),
            bundle_failures: Vec::new(),
            caches: Caches::new(),
            step: Step::Constructed,
            entered: Vec::new(),
        }
    }

    /// What this entry claims at the registration barrier.
    ///
    /// Nothing. Section four is explicit: three ingredient kinds and nothing
    /// else, and no block, item, entity, menu, particle, effect, trigger,
    /// argument kind, data component kind or loot modifier is registered
    /// anywhere in the tree. The three kinds go into RVL-210's ingredient-kind
    /// registry, which is not one of Ravel's four registry kinds, so the content
    /// declaration is empty rather than absent.
    #[must_use]
    pub fn declaration() -> ContentDeclaration {
        ContentDeclaration::new(Self::declared_id().clone())
    }

    /// The values in force.
    #[must_use]
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The data comparator in force, which the two data-carrying kinds are
    /// built against.
    #[must_use]
    pub const fn comparator(&self) -> &Comparator {
        &self.comparator
    }

    /// The stackable-item kind, built against the comparator in force.
    #[must_use]
    pub fn stackable(&self) -> Stackable {
        Stackable::with(self.comparator.clone())
    }

    /// The fluid-portion kind, built against the comparator in force.
    #[must_use]
    pub fn fluid(&self) -> Fluid {
        Fluid::with(self.comparator.clone())
    }

    /// The scalar-charge kind, which carries no data and so no comparator.
    #[must_use]
    pub const fn scalar(&self) -> Scalar {
        Scalar
    }

    /// The shared ingredient-kind registry, which a dependent registers into.
    #[must_use]
    pub const fn kinds(&self) -> &Kinds {
        &self.kinds
    }

    /// The shared ingredient-kind registry, for a dependent's own kind.
    pub const fn kinds_mut(&mut self) -> &mut Kinds {
        &mut self.kinds
    }

    /// The alternative-set serialiser table.
    #[must_use]
    pub const fn serialisers(&self) -> &Serialisers {
        &self.serialisers
    }

    /// The store-adapter table of section eight.
    #[must_use]
    pub fn adapters(&self) -> &[Adapter] {
        &self.adapters
    }

    /// The capability-attachment registry the two bundles install into.
    #[must_use]
    pub const fn attachments(&self) -> &Registry {
        &self.attachments
    }

    /// The three caches of section fourteen.
    #[must_use]
    pub const fn caches(&self) -> &Caches {
        &self.caches
    }

    /// The three caches, for a dependent filing its own converted collection.
    pub const fn caches_mut(&mut self) -> &mut Caches {
        &mut self.caches
    }

    /// How far through section two's ordering the entry has got.
    #[must_use]
    pub const fn step(&self) -> Step {
        self.step
    }

    /// The lifecycle steps this entry has taken.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }

    /// Registry construction: installs the two alternative-set serialisers.
    ///
    /// This is the only thing the entry contributes to the shared serialiser
    /// table.
    ///
    /// # Errors
    ///
    /// Returns [`Refused::Serialisers`] when the table already holds one of the
    /// two, which would mean the late-priority contribution ran twice.
    pub fn install_serialisers(&mut self) -> Result<(), Refused> {
        self.serialisers
            .install_own()
            .map_err(Refused::Serialisers)?;
        self.step = Step::SerialisersInstalled;
        Ok(())
    }

    /// Registry population: registers the three ingredient kinds.
    ///
    /// # Errors
    ///
    /// Returns [`Refused::OutOfOrder`] before the serialisers are installed,
    /// which section two states as load-bearing ordering, and
    /// [`Refused::Kinds`] for whatever the registry refuses.
    pub fn register_kinds(&mut self) -> Result<(), Refused> {
        if self.step < Step::SerialisersInstalled {
            return Err(Refused::OutOfOrder {
                wanted: Step::KindsRegistered,
                reached: self.step,
            });
        }
        self.kinds.register_own().map_err(Refused::Kinds)?;
        self.step = Step::KindsRegistered;
        Ok(())
    }

    /// Common setup: asserts all three kinds resolved and subscribes every
    /// registered kind to capability gathering.
    ///
    /// A dependent's kind gets its capability gathering wired by this entry
    /// rather than by the dependent, which section two states is a contract
    /// rather than an implementation detail: an entry that wired only its own
    /// three would silently break a dependent's kind.
    ///
    /// # Errors
    ///
    /// Returns [`Refused::Kinds`] naming the first of the three that did not
    /// resolve. Section two says this fails loudly.
    pub fn common_setup(&mut self) -> Result<usize, Refused> {
        let wired = self.kinds.wire().map_err(Refused::Kinds)?;
        self.step = Step::SetUp;
        Ok(wired)
    }

    /// The cross-mod message point: populates the storage-adapter table.
    ///
    /// This is late on purpose. An adapter names a capability and capabilities
    /// are not all present earlier, which is why a dependent asking for a
    /// storage adapter during its own setup is refused here rather than handed
    /// an empty table.
    ///
    /// # Errors
    ///
    /// Returns [`Refused::OutOfOrder`] before common setup.
    pub fn install_adapters(&mut self) -> Result<usize, Refused> {
        if self.step < Step::SetUp {
            return Err(Refused::OutOfOrder {
                wanted: Step::AdaptersInstalled,
                reached: self.step,
            });
        }
        self.adapters = storage::table();
        self.step = Step::AdaptersInstalled;
        Ok(self.adapters.len())
    }

    /// The compatibility-loading point: installs the two bundles.
    ///
    /// One is for vanilla and always enabled; the other is for a third-party
    /// crafting-station package and is enabled when that package is present,
    /// which the caller answers by naming its target or not.
    ///
    /// Section three's `compatibility.abort_on_failure` decides what a bundle
    /// failure does. At its shipped default of `false` the failure is recorded
    /// and loading carries on with whatever the other bundle installed;
    /// switched on, the first failure aborts loading. Ravel's SDK publishes no
    /// logger, so recording is [`Entry::bundle_failures`] rather than a log
    /// line, and a failure is therefore readable rather than merely written.
    ///
    /// # Errors
    ///
    /// Returns [`Refused::Bundle`] for whatever RVL-210's registry refuses when
    /// the abort setting is on, and for an enabled third-party bundle naming
    /// nothing.
    pub fn load_compatibility(&mut self, third_party: Option<&str>) -> Result<usize, Refused> {
        let mut installed = 0;
        match Bundle::vanilla(true).install(&mut self.attachments) {
            Ok(count) => installed += count,
            Err(refused) => self.bundle_failed(refused)?,
        }
        if let Some(target) = third_party {
            match Bundle::third_party(true, Some(target)).install(&mut self.attachments) {
                Ok(count) => installed += count,
                Err(refused) => self.bundle_failed(refused)?,
            }
        }
        Ok(installed)
    }

    /// The eight recipe-view attachments, each bound to the view behind it.
    ///
    /// The brewing view is built at the cap the settings in force resolved, so a
    /// pack that raised `recipes.brewing_closure_cap` reaches the attachment
    /// rather than stopping at the setting.
    #[must_use]
    pub fn recipe_views(&self) -> Vec<recipes::Bound> {
        recipes::bound(self.settings.closure())
    }

    /// Every bundle failure this launch carried on past.
    #[must_use]
    pub fn bundle_failures(&self) -> &[capabilities::NotInstalled] {
        &self.bundle_failures
    }

    fn bundle_failed(&mut self, refused: capabilities::NotInstalled) -> Result<(), Refused> {
        if self.settings.abort_on_bundle_failure {
            return Err(Refused::Bundle(refused));
        }
        self.bundle_failures.push(refused);
        Ok(())
    }

    /// Invalidates all three caches.
    ///
    /// Section fourteen's single most useful divergence: the original clears two
    /// of the three when a server stops and the third never, so a reload that
    /// changes a recipe leaves every station view answering from the old
    /// collection until the world is left.
    pub fn invalidate(&mut self, at: Point) {
        self.caches.invalidate(at);
    }
}

#[declare_mod(id = "rvl_132_counterpart")]
impl Mod for Entry {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        match state {
            LifecycleState::Constructed => {
                settings::declaration().map_err(ModError::from_display)?;
                // Registry construction sits inside this state, and the
                // serialisers go in before registry population.
                self.install_serialisers().map_err(ModError::from_display)?;
            }
            LifecycleState::Registered => {
                self.register_kinds().map_err(ModError::from_display)?;
                self.common_setup().map_err(ModError::from_display)?;
                self.install_adapters().map_err(ModError::from_display)?;
                self.load_compatibility(None)
                    .map_err(ModError::from_display)?;
                self.attachments.bake();
            }
            // A server stopping is one of the two points section fourteen
            // clears at, and the reload is the other.
            LifecycleState::Stopping => self.invalidate(Point::Stopped),
            LifecycleState::Discovered
            | LifecycleState::Validated
            | LifecycleState::Resolved
            | LifecycleState::Loaded
            | LifecycleState::Running
            | LifecycleState::Stopped
            | LifecycleState::Failed => {}
        }
        self.entered.push(state);
        Ok(())
    }

    fn content_declaration(
        &self,
    ) -> Result<Option<std::borrow::Cow<'_, ContentDeclaration>>, ModError> {
        Ok(Some(std::borrow::Cow::Borrowed(&self.declaration)))
    }

    fn configuration_declaration(&self) -> Result<Option<ConfigurationDeclaration>, ModError> {
        settings::declaration()
            .map(Some)
            .map_err(ModError::from_display)
    }

    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
        // Section three: the list is re-read on every load and reload and the
        // comparator is rebuilt from it each time, so the setting takes effect
        // without a restart.
        self.settings = Settings::read(resolved).map_err(ModError::from_display)?;
        self.comparator = self.settings.comparator().map_err(ModError::from_display)?;
        Ok(())
    }

    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        if event == GameEvent::ResourcesReloaded {
            self.invalidate(Point::Reloaded);
        }
        Ok(())
    }
}

export_mod!(Entry::new());

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::{RECIPES, VANILLA_ATTACHMENTS};
    use crate::kinds::REGISTERED;

    #[test]
    fn wrapped_refusals_expose_their_typed_sources() {
        use std::error::Error;
        let kinds = Refused::Kinds(kinds::NotRegistered::Full { limit: 2 });
        assert!(
            kinds
                .source()
                .expect("kind source")
                .is::<kinds::NotRegistered>()
        );
        let bundle = Refused::Bundle(capabilities::NotInstalled::NoTarget);
        assert!(
            bundle
                .source()
                .expect("bundle source")
                .is::<capabilities::NotInstalled>()
        );
        let settings = Refused::Settings(settings::Refused::ListTooLong { limit: 4 });
        assert!(
            settings
                .source()
                .expect("settings source")
                .is::<settings::Refused>()
        );
        let serialisers = Refused::Serialisers(forms::TagError::Shape { expected: "a tag" });
        assert!(
            serialisers
                .source()
                .expect("serialiser source")
                .is::<forms::TagError>()
        );
        assert!(
            Refused::OutOfOrder {
                wanted: Step::SetUp,
                reached: Step::Constructed,
            }
            .source()
            .is_none()
        );
    }

    fn run_to_registered() -> Entry {
        let mut entry = Entry::new();
        entry
            .enter(LifecycleState::Constructed)
            .expect("a fresh entry");
        entry
            .enter(LifecycleState::Registered)
            .expect("a fresh entry");
        entry
    }

    #[test]
    fn the_entry_claims_nothing_at_the_registration_barrier() {
        let declared = Entry::declaration();
        assert_eq!(declared.entries().len(), 0);
        assert_eq!(Entry::declared_id().as_str(), MOD_ID);
    }

    #[test]
    fn construction_installs_the_two_serialisers_and_nothing_else() {
        let mut entry = Entry::new();
        assert!(entry.serialisers().is_empty());
        entry
            .enter(LifecycleState::Constructed)
            .expect("a fresh entry");
        assert_eq!(entry.serialisers().len(), 2);
        assert_eq!(entry.step(), Step::SerialisersInstalled);
        assert!(entry.kinds().is_empty());
        assert_eq!(entry.adapters().len(), 0);
    }

    #[test]
    fn registering_the_kinds_before_the_serialisers_is_refused() {
        let mut entry = Entry::new();
        assert_eq!(
            entry.register_kinds(),
            Err(Refused::OutOfOrder {
                wanted: Step::KindsRegistered,
                reached: Step::Constructed
            })
        );
        assert!(entry.kinds().is_empty());
    }

    #[test]
    fn asking_for_the_adapter_table_before_common_setup_is_refused() {
        let mut entry = Entry::new();
        entry.install_serialisers().expect("a fresh table");
        entry.register_kinds().expect("a fresh registry");
        assert_eq!(
            entry.install_adapters(),
            Err(Refused::OutOfOrder {
                wanted: Step::AdaptersInstalled,
                reached: Step::KindsRegistered
            })
        );
        assert_eq!(entry.adapters().len(), 0);
    }

    #[test]
    fn registration_registers_three_kinds_wires_them_and_installs_twelve_adapters() {
        let entry = run_to_registered();
        assert_eq!(entry.kinds().len(), 3);
        let names: Vec<&str> = entry
            .kinds()
            .entries()
            .map(|held| held.name.as_str())
            .collect();
        assert_eq!(names, REGISTERED);
        assert_eq!(entry.kinds().subscribed(), 3);
        assert_eq!(entry.adapters().len(), 12);
        assert_eq!(entry.step(), Step::AdaptersInstalled);
    }

    #[test]
    fn a_dependents_kind_registered_before_common_setup_is_wired_too() {
        let mut entry = Entry::new();
        entry.install_serialisers().expect("a fresh table");
        entry.register_kinds().expect("a fresh registry");
        entry
            .kinds_mut()
            .register("other:gas", &kinds::Portion::CATEGORIES, false)
            .expect("a dependent's kind");
        assert_eq!(entry.common_setup(), Ok(4));
        assert!(
            entry
                .kinds()
                .entry("other:gas")
                .expect("registered")
                .subscribed
        );
    }

    #[test]
    fn registration_installs_the_vanilla_bundle_and_bakes_the_registry() {
        let entry = run_to_registered();
        assert_eq!(entry.attachments().attachments().len(), VANILLA_ATTACHMENTS);
        assert!(entry.attachments().baked());
    }

    #[test]
    fn every_recipe_view_attachment_the_entry_installs_has_a_view_behind_it() {
        let entry = run_to_registered();
        let views = entry.recipe_views();
        let attached: Vec<&str> = entry
            .attachments()
            .attachments()
            .iter()
            .filter(|attachment| attachment.capability == RECIPES)
            .map(|attachment| attachment.address.written())
            .collect();
        assert_eq!(attached.len(), views.len());
        for bound in &views {
            assert!(attached.contains(&bound.target), "{}", bound.target);
        }
    }

    #[test]
    fn the_entrys_brewing_view_carries_the_resolved_cap() {
        let mut entry = Entry::new();
        entry.settings = Settings {
            closure_cap: 42,
            ..Settings::shipped()
        };
        let views = entry.recipe_views();
        let brewing = views
            .iter()
            .find_map(|bound| match &bound.view {
                recipes::View::Brewing(closure) => Some(closure),
                recipes::View::Station(_) => None,
            })
            .expect("a brewing view");
        assert_eq!(brewing.cap(), 42);
    }

    #[test]
    fn a_bundle_failure_is_recorded_and_loading_carries_on_at_the_shipped_default() {
        // Section three: with `compatibility.abort_on_failure` off, a bundle
        // failure is logged and loading continues. Ravel publishes no logger, so
        // the record is readable instead.
        let mut entry = run_to_registered();
        assert!(entry.attachments().baked());
        assert_eq!(entry.load_compatibility(None), Ok(0));
        assert_eq!(entry.bundle_failures().len(), 1);
    }

    #[test]
    fn the_abort_setting_turns_a_bundle_failure_into_a_refusal() {
        let mut entry = run_to_registered();
        entry.settings = Settings {
            abort_on_bundle_failure: true,
            ..Settings::shipped()
        };
        assert!(matches!(
            entry.load_compatibility(None),
            Err(Refused::Bundle(_))
        ));
        assert_eq!(entry.bundle_failures().len(), 0);
    }

    #[test]
    fn the_third_party_bundle_adds_one_attachment_when_a_target_is_named() {
        let mut entry = Entry::new();
        assert_eq!(entry.load_compatibility(Some("other:station")), Ok(47));
    }

    #[test]
    fn a_resolved_ignore_list_rebuilds_the_comparator_the_kinds_are_built_against() {
        let mut entry = Entry::new();
        assert!(entry.comparator().ignored().next().is_none());
        entry.settings = Settings {
            ignored: vec!["wear".to_owned()],
            ..Settings::shipped()
        };
        entry.comparator = entry.settings.comparator().expect("a one-name list");
        assert_eq!(
            entry.comparator().ignored().collect::<Vec<&str>>(),
            ["wear"]
        );
        assert_eq!(
            entry
                .stackable()
                .comparator()
                .ignored()
                .collect::<Vec<&str>>(),
            ["wear"]
        );
        assert_eq!(
            entry.fluid().comparator().ignored().collect::<Vec<&str>>(),
            ["wear"]
        );
        assert_eq!(entry.scalar(), Scalar);
    }

    #[test]
    fn a_resource_reload_invalidates_every_cache() {
        let mut entry = run_to_registered();
        entry
            .caches_mut()
            .brewing(|| Ok::<Vec<recipes::Converted>, ()>(Vec::new()))
            .expect("a small table");
        assert!(entry.caches().brewed());
        entry
            .observe(GameEvent::ResourcesReloaded)
            .expect("an event");
        assert!(entry.caches().is_empty());
    }

    #[test]
    fn a_tick_invalidates_nothing() {
        let mut entry = run_to_registered();
        entry
            .caches_mut()
            .brewing(|| Ok::<Vec<recipes::Converted>, ()>(Vec::new()))
            .expect("a small table");
        for event in [GameEvent::ServerTick, GameEvent::ClientTick] {
            entry.observe(event).expect("an event");
        }
        assert!(entry.caches().brewed());
    }

    #[test]
    fn a_server_stopping_invalidates_every_cache() {
        let mut entry = run_to_registered();
        entry
            .caches_mut()
            .brewing(|| Ok::<Vec<recipes::Converted>, ()>(Vec::new()))
            .expect("a small table");
        entry
            .enter(LifecycleState::Stopping)
            .expect("a running entry");
        assert!(entry.caches().is_empty());
    }

    #[test]
    fn the_entry_declares_its_settings_document() {
        let entry = Entry::new();
        let declared = entry
            .configuration_declaration()
            .expect("valid settings")
            .expect("a packaged document");
        assert_eq!(declared.settings().len(), settings::KEYS.len());
    }

    #[test]
    fn the_lifecycle_steps_taken_are_kept_in_order() {
        let entry = run_to_registered();
        assert_eq!(
            entry.entered(),
            [LifecycleState::Constructed, LifecycleState::Registered]
        );
    }

    #[test]
    fn refusals_say_what_was_wrong() {
        assert!(
            Refused::OutOfOrder {
                wanted: Step::SetUp,
                reached: Step::Constructed
            }
            .to_string()
            .contains("out of the order")
        );
        assert!(
            Refused::Settings(settings::Refused::ListTooLong { limit: 4 })
                .to_string()
                .contains("longer than 4")
        );
        assert!(
            Refused::Kinds(kinds::NotRegistered::Full { limit: 2 })
                .to_string()
                .contains("at most 2")
        );
        assert!(
            Refused::Bundle(capabilities::NotInstalled::NoTarget)
                .to_string()
                .contains("naming a target")
        );
        assert!(
            Refused::Serialisers(forms::TagError::Shape { expected: "a tag" })
                .to_string()
                .contains("a tag")
        );
    }
}
