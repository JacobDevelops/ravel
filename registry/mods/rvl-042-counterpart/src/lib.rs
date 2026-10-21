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
use ravel_sdk::content::{ContentDeclaration, ContentKind};
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
pub struct Entry {
    declaration: Result<ContentDeclaration, ModError>,
    settings: Result<Settings, ModError>,
    selection: Option<Selection>,
    observed: Vec<GameEvent>,
    entered: Vec<LifecycleState>,
    asked: Vec<PlayRequest>,
}

impl Default for Entry {
    fn default() -> Self {
        Self::new()
    }
}

impl Entry {
    /// Builds the library holding its shipped defaults and no recorded answer.
    ///
    /// The content declaration is built here and held, so the barrier and the
    /// registration answer one question once rather than asking it twice.
    #[must_use]
    pub fn new() -> Self {
        Self {
            declaration: Self::declaration(),
            settings: Settings::shipped(),
            selection: None,
            observed: Vec::new(),
            entered: Vec::new(),
            asked: Vec::new(),
        }
    }

    /// Returns the values in force.
    ///
    /// # Errors
    /// Returns the retained shipped-settings refusal.
    pub fn settings(&self) -> Result<&Settings, ModError> {
        self.settings.as_ref().map_err(Clone::clone)
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
                .map_err(ModError::from_display)?;
        }
        declaration
            .entry(ContentKind::Command, COMMAND)
            .map_err(ModError::from_display)?;
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
        declared.entry(SOUND, SOUND_BYTES.to_vec())?;
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
            .and_then(|declared| declared.play(declared.declared_event(SOUND)?, side))
            .map_err(ModError::from_display)?;
        self.asked.push(request);
        Ok(true)
    }

    /// Returns what this library has asked for and not yet handed over.
    #[must_use]
    pub fn asked(&self) -> &[PlayRequest] {
        &self.asked
    }
}

#[declare_mod(id = "rvl_042_counterpart")]
impl Mod for Entry {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        if state == LifecycleState::Constructed {
            self.declaration.as_ref().map_err(Clone::clone)?;
            self.settings()?;
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

    fn take_play_requests(&mut self) -> Vec<PlayRequest> {
        std::mem::take(&mut self.asked)
    }

    fn configuration_declaration(&self) -> Result<Option<ConfigurationDeclaration>, ModError> {
        settings::declaration()
            .map(Some)
            .map_err(ModError::from_display)
    }

    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
        let settings = Settings::read(resolved).map_err(ModError::from_display)?;
        self.selection = Some(Selection::new(
            settings.preference.clone(),
            settings.self_heal,
        ));
        self.settings = Ok(settings);
        Ok(())
    }

    fn run_command(&mut self, _: &CommandInvocation) -> Result<(), ModError> {
        Err(ModError::refused(
            "the energy store this command fills is host-blocked",
        ))
    }

    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        self.observed.push(event);
        Ok(())
    }
}

export_mod!(Entry::new());

#[cfg(test)]
mod tests {
    use super::*;
    use ravel_sdk::audio::AudioEvent;

    #[test]
    fn shipped_settings_failure_is_refused_at_constructed() {
        let original = ModError::refused("invalid shipped settings");
        let mut entry = Entry::new();
        entry.settings = Err(original.clone());
        assert_eq!(entry.enter(LifecycleState::Constructed), Err(original));
        assert_eq!(entry.entered(), []);
    }

    #[test]
    fn the_declared_identity_is_the_one_the_package_carries() {
        assert_eq!(Entry::declared_id().as_str(), MOD_ID);
        assert_eq!(Entry::new().id().as_str(), MOD_ID);
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
        let shared = Entry::new();
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
        let shared = Entry::new();
        assert!(shared.selection().is_none());
        assert_eq!(
            shared.settings(),
            Settings::shipped().as_ref().map_err(Clone::clone)
        );
    }

    #[test]
    fn an_observed_signal_is_recorded_and_the_lifecycle_runs_through() {
        let mut shared = Entry::new();
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
    fn the_declared_event_names_the_one_sound_this_package_owns() {
        let declared = Entry::audio_declaration().expect("the declaration is well formed");
        assert_eq!(declared.entries().collect::<Vec<_>>(), [SOUND]);
        assert_eq!(
            declared.events().map(AudioEvent::entry).collect::<Vec<_>>(),
            [SOUND]
        );

        let mut declared = Entry::audio_declaration().expect("the declaration is well formed");
        assert!(
            declared.event("nothing_owns_this").is_err(),
            "an event with nothing to play is refused where it is written"
        );
    }

    #[test]
    fn a_sweep_that_harvested_something_asks_for_the_sound_once_and_the_host_takes_it() {
        let mut shared = Entry::new();
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
        assert_eq!(taken[0].owner(), Entry::declared_id());
        assert_eq!(shared.take_play_requests(), Vec::<PlayRequest>::new());
    }

    #[test]
    fn a_sweep_that_harvested_nothing_asks_for_nothing_and_a_server_is_refused() {
        let mut shared = Entry::new();
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
    fn the_claimed_command_refuses_the_host_blocked_energy_store() {
        let mut entry = Entry::new();
        let key = RegistryKey::new(
            ContentKind::Command,
            ContentId::new(Entry::declared_id().clone(), COMMAND)
                .expect("the command identity is valid"),
        );
        let invocation = CommandInvocation::new(key, Side::DedicatedServer, Vec::new());
        assert_eq!(
            entry.run_command(&invocation),
            Err(ModError::refused(
                "the energy store this command fills is host-blocked",
            ))
        );
    }

    #[test]
    fn the_host_builds_this_package_under_its_declared_identity() {
        assert_eq!(__ravel_entry(Side::DedicatedServer).id().as_str(), MOD_ID);
    }
}
