//! Loader-driven entry, setup ordering, and refusal reporting.

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

use crate::{capabilities, forms, kinds, recipes, settings, storage};

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
        let wired = self.kinds.subscribe_all().map_err(Refused::Kinds)?;
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

#[cfg(test)]
mod conditions;
