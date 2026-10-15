#![forbid(unsafe_code)]

//! An original Ravel implementation of the behaviour specified in RVL-042.
//!
//! This is a shared library with no gameplay of its own. Its whole purpose is to
//! be depended on: it publishes the decision rules, the arithmetic and the saved
//! shapes that the counterparts around it build on. `NOTES.md` beside this file
//! says which sections of the specification are carried here and which are left
//! declarable only because Ravel publishes no seam for them.
//!
//! What a dependent reaches for:
//!
//! - [`flags`] — named booleans over resolved configuration, bound once;
//! - [`selection`] — the entry a tag stands for, and the record that keeps it;
//! - [`inventory`] — the saved shape of a store, with counts above the normal
//!   maximum;
//! - [`reusable`] — the crafting remainder of an entry declared reusable;
//! - [`arithmetic`] — the sweeping, radial, stack and colour arithmetic;
//! - [`settings`] — what a player and a pack may change about all of it;
//! - [`dumps`] — the documents those rules render, kept beside the source.

pub mod arithmetic;
pub mod dumps;
pub mod flags;
pub mod inventory;
pub mod reusable;
pub mod selection;
pub mod settings;

use ravel_sdk::audio::{AudioDeclaration, AudioError, PlayRequest};
use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration};
use ravel_sdk::content::{ContentDeclaration, ContentKind, Registrar, claim_all};
use ravel_sdk::events::GameEvent;
use ravel_sdk::prelude::*;

pub use crate::arithmetic::{Colour, Mature, Position, SweepReport};
pub use crate::flags::{FlagId, FlagReference, FlagTable, Predicate};
pub use crate::inventory::{Access, Facing, LoadReport, Stack, Store, StoreRefused};
pub use crate::reusable::{Reusable, UsesLeft};
pub use crate::selection::{Preference, Recorded, Selection, Unification};
pub use crate::settings::Settings;

/// The identity this package is declared and validated under.
pub const MOD_ID: &str = "rvl_042_counterpart";

/// The recipe kinds this entry contributes, in the order the specification
/// numbers them.
///
/// Ravel claims a recipe kind as a registry entry and decides nothing about what
/// one does, so these are names a dependent may declare against. The matching,
/// the component and damage transfer, and the wire format are all host work and
/// are left declarable only.
pub const RECIPE_KINDS: [&str; 5] = [
    "shaped_unmirrored",
    "shaped_transferring_data",
    "shaped_transferring_damage",
    "shaped_tag_result",
    "shapeless_tag_result",
];

/// The command this entry roots at its own name.
pub const COMMAND: &str = "rvl_042_counterpart";

/// The one sound this entry owns, played while a refillable tool is in use and
/// once at the end of a sweep that harvested anything.
pub const SOUND: &str = "tool_in_use";

/// The bytes the declaration carries for that sound.
///
/// Not a sound. What a sound file holds is a format's decision, nothing in this
/// package reads one, and the specification says nothing about the recording, so
/// the package supplies bytes it can name and the declaration carries them
/// unread.
const SOUND_BYTES: &[u8] = b"the supplied bytes";

/// The shared library, as the loader drives it.
#[derive(Debug)]
pub struct Shared {
    declaration: Result<ContentDeclaration, ModError>,
    flags: FlagTable,
    settings: Settings,
    selection: Option<Selection>,
    observed: Vec<GameEvent>,
    entered: Vec<LifecycleState>,
    asked: Vec<PlayRequest>,
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

impl Shared {
    /// Builds the library holding its shipped defaults and no recorded answer.
    ///
    /// The content declaration is built here and held, so the barrier and the
    /// registration answer one question once rather than asking it twice.
    #[must_use]
    pub fn new() -> Self {
        Self {
            declaration: Self::declaration(),
            flags: FlagTable::default(),
            settings: Settings::default(),
            selection: None,
            observed: Vec::new(),
            entered: Vec::new(),
            asked: Vec::new(),
        }
    }

    /// Returns the flag table dependents publish into and reference out of.
    #[must_use]
    pub const fn flags(&self) -> &FlagTable {
        &self.flags
    }

    /// Returns the flag table for publication.
    ///
    /// A dependent publishes its own flags here before anything asks for them.
    /// Ravel gives no mod a way to force another mod's declarations to exist, so
    /// the ordering guarantee is met by declaration order rather than by
    /// discovery, and a dependent that publishes nothing disables only itself.
    pub const fn flags_mut(&mut self) -> &mut FlagTable {
        &mut self.flags
    }

    /// Returns the values in force.
    #[must_use]
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Returns the tag selection, once configuration has been resolved.
    #[must_use]
    pub const fn selection(&self) -> Option<&Selection> {
        self.selection.as_ref()
    }

    /// Returns the tag selection for asking, once configuration is resolved.
    pub const fn selection_mut(&mut self) -> Option<&mut Selection> {
        self.selection.as_mut()
    }

    /// Returns the lifecycle steps this library has taken.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }

    /// Returns the signals this library has observed.
    #[must_use]
    pub fn observed(&self) -> &[GameEvent] {
        &self.observed
    }

    /// Returns what this library claims at the registration barrier.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when a declared path is not a legal content
    /// path, which for these constants would be a packaging failure.
    pub fn declaration() -> Result<ContentDeclaration, ModError> {
        let mut declaration = ContentDeclaration::new(Self::declared_id().clone());
        for kind in RECIPE_KINDS {
            declaration
                .entry(ContentKind::Recipe, kind)
                .map_err(|detail| Self::packaging(&detail))?;
        }
        declaration
            .entry(ContentKind::Command, COMMAND)
            .map_err(|detail| Self::packaging(&detail))?;
        Ok(declaration)
    }

    /// Builds the sound declaration: one entry and the event that plays it.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError`] when [`SOUND`] is not a legal entry name, which
    /// for a constant would be a packaging failure.
    pub fn audio_declaration() -> Result<AudioDeclaration, AudioError> {
        let mut declared = AudioDeclaration::new(Self::declared_id().clone());
        declared.entry(SOUND, SOUND_BYTES)?;
        declared.event(SOUND)?;
        Ok(declared)
    }

    /// Asks for the sound a sweep earned, holding the request for the host.
    ///
    /// A sweep that harvested nothing asks for nothing. The request is held
    /// until [`Mod::take_play_requests`] hands it over, which is what makes the
    /// sound happen after the call that asked for it.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] on a server side, where the declaration
    /// refuses to play rather than skipping. A skipped call is what would make
    /// this package audible in single player and silent for everybody else.
    pub fn announce_sweep(&mut self, report: &SweepReport, side: Side) -> Result<bool, ModError> {
        if !report.sound {
            return Ok(false);
        }
        let request = Self::audio_declaration()
            .and_then(|declared| declared.play(SOUND, side))
            .map_err(|detail| Self::refused_while_running(&detail))?;
        self.asked.push(request);
        Ok(true)
    }

    /// Returns what this library has asked for and not yet handed over.
    #[must_use]
    pub fn asked(&self) -> &[PlayRequest] {
        &self.asked
    }

    fn packaging(detail: &(impl ToString + ?Sized)) -> ModError {
        ModError::Refused {
            mod_id: Self::declared_id().clone(),
            state: LifecycleState::Constructed,
            detail: detail.to_string(),
        }
    }

    fn refused_while_running(detail: &(impl ToString + ?Sized)) -> ModError {
        ModError::Refused {
            mod_id: Self::declared_id().clone(),
            state: LifecycleState::Running,
            detail: detail.to_string(),
        }
    }
}

#[declare_mod(id = "rvl_042_counterpart")]
impl Mod for Shared {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        self.entered.push(state);
        Ok(())
    }

    fn content_declaration(&self) -> Result<Option<ContentDeclaration>, ModError> {
        self.declaration.clone().map(Some)
    }

    fn take_play_requests(&mut self) -> Vec<PlayRequest> {
        std::mem::take(&mut self.asked)
    }

    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
        let declaration = self.declaration.as_ref().map_err(Clone::clone)?;
        claim_all(declaration, registrar)
    }

    fn configuration_declaration(&self) -> Result<Option<ConfigurationDeclaration>, ModError> {
        settings::declaration()
            .map(Some)
            .map_err(|error| Self::packaging(&error))
    }

    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
        self.settings = Settings::read(resolved);
        self.selection = Some(Selection::new(
            self.settings.preference.clone(),
            self.settings.self_heal,
        ));
        Ok(())
    }

    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        self.observed.push(event);
        Ok(())
    }
}

export_mod!(Shared::new());

#[cfg(test)]
mod tests {
    use super::*;
    use ravel_sdk::audio::AudioEvent;
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
        assert_eq!(Shared::declared_id().as_str(), MOD_ID);
        assert_eq!(Shared::new().id().as_str(), MOD_ID);
    }

    #[test]
    fn every_declared_kind_is_claimed_at_the_barrier_before_anything_else() {
        let mut shared = Shared::new();
        let mut registrar = Accepting::default();
        shared
            .register(&mut registrar)
            .expect("the accepting registrar refuses nothing");

        assert_eq!(registrar.claimed().len(), RECIPE_KINDS.len() + 1);
        assert!(
            registrar
                .claimed()
                .iter()
                .any(|key| key.kind() == ContentKind::Command)
        );
        assert_eq!(
            registrar
                .claimed()
                .iter()
                .filter(|key| key.kind() == ContentKind::Recipe)
                .count(),
            5
        );
    }

    #[test]
    fn the_five_recipe_kinds_are_named_once_each() {
        let mut sorted = RECIPE_KINDS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), RECIPE_KINDS.len());
    }

    #[test]
    fn the_library_declares_both_its_content_and_its_settings() {
        let shared = Shared::new();
        assert!(
            shared
                .content_declaration()
                .expect("valid content")
                .is_some()
        );
        assert!(
            shared
                .configuration_declaration()
                .expect("valid configuration")
                .is_some()
        );
    }

    #[test]
    fn a_library_that_has_not_been_configured_holds_no_selection_yet() {
        let shared = Shared::new();
        assert!(shared.selection().is_none());
        assert_eq!(shared.settings(), &Settings::default());
    }

    #[test]
    fn an_observed_signal_is_recorded_and_the_lifecycle_runs_through() {
        let mut shared = Shared::new();
        for state in LifecycleState::ALL {
            shared.enter(state).expect("the library accepts every step");
        }
        shared
            .observe(GameEvent::ServerTick)
            .expect("the library records what it observes");

        assert_eq!(shared.entered(), LifecycleState::ALL);
        assert_eq!(shared.observed(), [GameEvent::ServerTick]);
    }

    #[test]
    fn a_dependent_publishes_its_flags_into_the_one_table() {
        let mut shared = Shared::new();
        let id = FlagId::parse("dependent:optional").expect("a legal identity");
        shared.flags_mut().publish_constant(id.clone(), true);
        assert!(shared.flags().enabled(&id));
    }

    #[test]
    fn the_declared_event_names_the_one_sound_this_package_owns() {
        let declared = Shared::audio_declaration().expect("the declaration is well formed");
        assert_eq!(declared.entries().collect::<Vec<_>>(), [SOUND]);
        assert_eq!(
            declared.events().map(AudioEvent::entry).collect::<Vec<_>>(),
            [SOUND]
        );

        let mut declared = Shared::audio_declaration().expect("the declaration is well formed");
        assert!(
            declared.event("nothing_owns_this").is_err(),
            "an event with nothing to play is refused where it is written"
        );
    }

    #[test]
    fn a_sweep_that_harvested_something_asks_for_the_sound_once_and_the_host_takes_it() {
        let mut shared = Shared::new();
        let harvested = SweepReport {
            sound: true,
            ..SweepReport::default()
        };

        assert!(
            shared
                .announce_sweep(&harvested, Side::Client)
                .expect("a client plays what the package declared")
        );
        assert_eq!(shared.asked().len(), 1);

        let taken = shared.take_play_requests();
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].entry(), SOUND);
        assert_eq!(taken[0].owner(), Shared::declared_id());
        assert_eq!(shared.take_play_requests(), Vec::<PlayRequest>::new());
    }

    #[test]
    fn a_sweep_that_harvested_nothing_asks_for_nothing_and_a_server_is_refused() {
        let mut shared = Shared::new();
        assert!(
            !shared
                .announce_sweep(&SweepReport::default(), Side::Client)
                .expect("nothing harvested asks for nothing")
        );
        assert_eq!(shared.asked(), []);

        let harvested = SweepReport {
            sound: true,
            ..SweepReport::default()
        };
        for side in [Side::IntegratedServer, Side::DedicatedServer] {
            assert!(
                shared.announce_sweep(&harvested, side).is_err(),
                "a server side is refused rather than quietly skipped"
            );
        }
        assert_eq!(shared.asked(), []);
    }

    #[test]
    fn the_host_builds_this_package_under_its_declared_identity() {
        assert_eq!(__ravel_entry(Side::DedicatedServer).id().as_str(), MOD_ID);
    }
}
