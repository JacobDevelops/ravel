#![forbid(unsafe_code)]

//! An original Ravel implementation of the behaviour specified in RVL-120.
//!
//! A shared library with no gameplay of its own. It ships no block, no item and
//! no recipe, tag or loot document, and it contributes exactly two things a
//! player could ever see: one key binding and one interface texture. Everything
//! else is machinery for the packages that depend on it, and five admitted
//! counterparts cannot start until it exists.
//!
//! What a dependent reaches for:
//!
//! - [`commands`] — the command transport, its three shapes and its refusals;
//! - [`network`] — the channel and the twelve payload kinds it ships;
//! - [`values`] — the ordered value map and its positional screen wire form;
//! - [`data`] — the common record every dependent's block and item carries;
//! - [`dumps`] — the documents this package renders out of its own tables;
//! - [`preferences`] — the per-player display record and its push rule;
//! - [`grouping`] — the service that treats touching blocks as one thing;
//! - [`arithmetic`] — the two change checks and the energy clamp;
//! - [`primitives`] — the run-length coder and the smaller primitives;
//! - [`save`] — the global saved-data helper and its client-side refusal;
//! - [`census`] — what this package registers, and what it deliberately does not;
//! - [`recipes`] — the one crafting recipe kind's assembly rule;
//! - [`settings`] — the two configuration documents and the colour rules.
//!
//! `NOTES.md` beside this file says which sections are carried here and which
//! are left declarable only because Ravel publishes no seam for them.

pub mod arithmetic;
pub mod census;
pub mod commands;
pub mod data;
pub mod dumps;
pub mod grouping;
pub mod network;
pub mod preferences;
pub mod primitives;
pub mod recipes;
pub mod save;
pub mod settings;
pub mod values;

use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration};
use ravel_sdk::content::ContentDeclaration;
use ravel_sdk::events::GameEvent;
use ravel_sdk::prelude::*;
use ravel_sdk::save::{SavedDataDeclaration, SavedDataEnvelope};

pub use crate::arithmetic::{
    Check32, Check64, Table64, check32_over, check64_over, clamp_energy, reported_energy,
};
pub use crate::commands::{Argument, CommandKey, Commands, Declaration, ElementType, Shape};
pub use crate::data::{BlockData, Identity, RedstoneMode, Spellings};
pub use crate::grouping::{Grouping, Placement, Position};
pub use crate::preferences::{Corner, Preferences};
pub use crate::primitives::{Face, Interner, Orientation, Runs, code_runs, read_runs};
pub use crate::save::GlobalData;
pub use crate::settings::{Colours, Document, Settings};
pub use crate::values::{Subscription, Value, ValueMap};

/// The identity this package is declared and validated under.
pub const MOD_ID: &str = "rvl_120_counterpart";

/// The one key binding, in this package's own category.
pub const KEY_BINDING: &str = "open_manual";

/// The four optional integrations, by the role each plays.
///
/// **Host-blocked.** Ravel publishes no facility for optional cross-mod
/// integration at the reviewed revision, so these are recorded as pending
/// rather than stubbed. The rule that survives is that absence is never an
/// error.
pub const INTEGRATIONS: [&str; 4] = [
    "in_game_manual",
    "hover_probe",
    "hover_tooltip",
    "recipe_viewer",
];

/// The shared library, as the loader drives it.
///
/// Neither the lifecycle steps nor the observed signals are kept as a log. The
/// loader broadcasts an event per tick, so a vector of everything seen would
/// grow for the whole of a session; what a caller can ask is how many arrived
/// and which one was last.
#[derive(Debug)]
pub struct Entry {
    side: Side,
    declaration: Result<ContentDeclaration, ModError>,
    settings: Result<Settings, ModError>,
    colours: Colours,
    commands: Commands,
    grouping: Grouping,
    global: GlobalData,
    manual_present: Option<bool>,
    manual_held: bool,
    events_observed: usize,
    last_event: Option<GameEvent>,
    states_entered: usize,
    last_state: Option<LifecycleState>,
}

impl Default for Entry {
    fn default() -> Self {
        Self::new(Side::Client)
    }
}

impl Entry {
    /// Builds the library for one side, holding its shipped defaults.
    ///
    /// Declaration and default-resolution failures are retained for `Constructed` refusal.
    #[must_use]
    pub fn new(side: Side) -> Self {
        Self {
            side,
            declaration: census::declaration().map_err(ModError::from_display),
            settings: Settings::shipped(),
            colours: Colours::shipped(),
            commands: Commands::new(),
            grouping: Grouping::new(),
            global: GlobalData::new(),
            manual_present: None,
            manual_held: false,
            events_observed: 0,
            last_event: None,
            states_entered: 0,
            last_state: None,
        }
    }

    /// Returns the side this instance was built for.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Returns the three server values in force.
    ///
    /// # Errors
    /// Returns the retained packaged-default failure.
    pub fn settings(&self) -> Result<&Settings, ModError> {
        self.settings.as_ref().map_err(Clone::clone)
    }

    /// Returns the seventy colours in force.
    #[must_use]
    pub const fn colours(&self) -> &Colours {
        &self.colours
    }

    /// Returns the command tables dependents declare into.
    #[must_use]
    pub const fn commands(&self) -> &Commands {
        &self.commands
    }

    /// Returns the command tables for declaring.
    ///
    /// A dependent declares its commands here at its own declaration point.
    /// Ravel publishes no way for one package to discover another's statically
    /// declared commands, so the guarantee that a command exists before it is
    /// called comes from declaration order rather than from a scan.
    pub const fn commands_mut(&mut self) -> &mut Commands {
        &mut self.commands
    }

    /// Returns the grouping service.
    #[must_use]
    pub const fn grouping(&self) -> &Grouping {
        &self.grouping
    }

    /// Returns the grouping service for placing and removing.
    pub const fn grouping_mut(&mut self) -> &mut Grouping {
        &mut self.grouping
    }

    /// Returns the global saved-data store.
    #[must_use]
    pub const fn global(&self) -> &GlobalData {
        &self.global
    }

    /// Returns the global saved-data store for writing.
    pub const fn global_mut(&mut self) -> &mut GlobalData {
        &mut self.global
    }

    /// Returns how many lifecycle steps this library has taken.
    #[must_use]
    pub const fn states_entered(&self) -> usize {
        self.states_entered
    }

    /// Returns the last lifecycle step this library took.
    #[must_use]
    pub const fn last_state(&self) -> Option<LifecycleState> {
        self.last_state
    }

    /// Returns how many signals this library has observed.
    #[must_use]
    pub const fn events_observed(&self) -> usize {
        self.events_observed
    }

    /// Returns the last signal this library observed.
    #[must_use]
    pub const fn last_event(&self) -> Option<GameEvent> {
        self.last_event
    }

    /// Latches whether the optional manual is installed.
    ///
    /// The original latches this once at common setup and never re-checks. Here
    /// it may be re-latched freely, because nothing depends on the latch being
    /// stable and absence is not an error either way.
    pub const fn latch_manual(&mut self, present: bool) {
        self.manual_present = Some(present);
    }

    /// Returns what the latch holds, or nothing before it has been set.
    #[must_use]
    pub const fn manual_present(&self) -> Option<bool> {
        self.manual_present
    }

    /// Answers the key binding as a press rather than as a held key.
    ///
    /// The original polls the binding on every client tick with a direct device
    /// query and no edge detection, so holding the key re-triggers the open on
    /// every tick. This edge-detects: `down` is the device's answer this tick
    /// and the result is whether the manual should open now.
    ///
    /// A binding that is not held while one of this package's screens is open
    /// does nothing, which is the original's own gating.
    pub const fn poll_manual_key(&mut self, down: bool, screen_open: bool) -> bool {
        let pressed = down && !self.manual_held && screen_open;
        self.manual_held = down;
        pressed
    }
}

#[declare_mod(id = "rvl_120_counterpart")]
impl Mod for Entry {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        self.states_entered += 1;
        self.last_state = Some(state);
        if state == LifecycleState::Constructed {
            self.declaration.as_ref().map_err(Clone::clone)?;
            self.settings()?;
        }
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

    fn configuration_declaration(&self) -> Result<Option<ConfigurationDeclaration>, ModError> {
        settings::declaration()
            .map(Some)
            .map_err(ModError::from_display)
    }

    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
        self.settings = Ok(Settings::read(resolved).map_err(ModError::from_display)?);
        self.colours
            .resolve(Document::of(self.side), resolved)
            .map_err(ModError::from_display)?;
        Ok(())
    }

    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        self.events_observed += 1;
        self.last_event = Some(event);
        Ok(())
    }

    fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, ModError> {
        GlobalData::declaration()
            .map(Some)
            .map_err(ModError::from_display)
    }

    fn open_saved_data(&mut self, envelope: Option<&SavedDataEnvelope>) -> Result<(), ModError> {
        let Some(envelope) = envelope else {
            self.global = GlobalData::new();
            return Ok(());
        };
        self.global =
            GlobalData::from_payload(envelope.payload()).map_err(ModError::from_display)?;
        Ok(())
    }

    fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, ModError> {
        if self.global.is_empty() {
            return Ok(None);
        }
        self.global
            .envelope()
            .map(Some)
            .map_err(ModError::from_display)
    }
}

export_mod!(|side| Entry::new(side));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_settings_failure_is_retained_until_configuration_succeeds() {
        let original = ModError::refused("invalid packaged settings");
        let mut shared = Entry::new(Side::Client);
        shared.settings = Err(original.clone());
        assert_eq!(shared.settings(), Err(original.clone()));
        assert_eq!(
            shared.enter(LifecycleState::Constructed),
            Err(original.clone())
        );
        assert_eq!(shared.settings(), Err(original));

        let resolved = LayeredConfiguration::resolve(
            settings::declaration().expect("valid packaged declaration"),
            None,
            None,
        )
        .expect("valid packaged defaults");
        shared.configure(&resolved).expect("configuration recovers");
        assert_eq!(
            shared.settings().expect("configured settings"),
            &Settings::shipped().expect("packaged defaults")
        );
        shared
            .enter(LifecycleState::Constructed)
            .expect("configured settings permit construction");
    }

    #[test]
    fn deferred_declarations_preserve_the_original_refusal() {
        let original = ModError::refused("the original packaged declaration failure");
        let mut shared = Entry::new(Side::Client);
        shared.declaration = Err(original.clone());
        assert_eq!(
            shared
                .content_declaration()
                .expect_err("the declaration refused"),
            original
        );
        assert_eq!(shared.enter(LifecycleState::Constructed), Err(original));
    }

    #[test]
    fn the_declared_identity_is_the_one_the_package_carries() {
        assert_eq!(Entry::declared_id().as_str(), MOD_ID);
        assert_eq!(Entry::new(Side::Client).id().as_str(), MOD_ID);
        assert_eq!(__ravel_entry(Side::DedicatedServer).id().as_str(), MOD_ID);
    }

    #[test]
    fn the_library_declares_both_its_content_and_its_settings() {
        let shared = Entry::new(Side::Client);
        assert!(
            shared
                .content_declaration()
                .expect("valid content")
                .is_some()
        );
        assert!(
            shared
                .configuration_declaration()
                .expect("valid settings")
                .is_some()
        );
    }

    #[test]
    fn the_lifecycle_runs_through_and_what_is_kept_is_a_count_and_the_last_one() {
        let mut shared = Entry::new(Side::Client);
        for state in LifecycleState::ALL {
            shared.enter(state).expect("the library accepts every step");
        }
        for event in GameEvent::ALL {
            shared
                .observe(event)
                .expect("the library records what it sees");
        }
        assert_eq!(shared.states_entered(), LifecycleState::ALL.len());
        assert_eq!(shared.last_state(), LifecycleState::ALL.last().copied());
        assert_eq!(shared.events_observed(), GameEvent::ALL.len());
        assert_eq!(shared.last_event(), GameEvent::ALL.last().copied());
    }

    #[test]
    fn a_signal_every_tick_does_not_make_the_library_grow() {
        let mut shared = Entry::new(Side::Client);
        let event = GameEvent::ALL[0];
        for _ in 0..10_000 {
            shared
                .observe(event)
                .expect("the library records what it sees");
            shared
                .enter(LifecycleState::Running)
                .expect("the library accepts the step");
        }
        assert_eq!(shared.events_observed(), 10_000);
        assert_eq!(shared.states_entered(), 10_000);
        assert_eq!(shared.last_event(), Some(event));
    }

    #[test]
    fn a_dedicated_server_does_not_re_resolve_the_seventy_client_colours() {
        let declaration = settings::declaration().expect("the packaged declaration is legal");
        let resolved = LayeredConfiguration::resolve(declaration, None, None)
            .expect("the author's defaults resolve");

        let mut server = Entry::new(Side::DedicatedServer);
        assert_eq!(server.side(), Side::DedicatedServer);
        let before = server.colours().values().to_vec();
        server.configure(&resolved).expect("the defaults are read");
        assert_eq!(
            server.colours().values(),
            before.as_slice(),
            "the client document has no backing on a server, so nothing is read from it"
        );

        let mut client = Entry::new(Side::Client);
        client.configure(&resolved).expect("the defaults are read");
        assert_eq!(client.colours().values().len(), settings::COLOUR_COUNT);
    }

    #[test]
    fn a_library_that_owns_no_global_record_produces_no_envelope() {
        let shared = Entry::new(Side::Client);
        assert!(
            shared
                .save_saved_data()
                .expect("an empty store produces nothing")
                .is_none()
        );
        assert!(
            shared
                .saved_data_declaration()
                .expect("valid saved data")
                .is_some()
        );
    }

    #[test]
    fn a_global_record_written_on_a_server_reaches_the_envelope() {
        let mut shared = Entry::new(Side::Client);
        shared
            .global_mut()
            .write(
                Side::DedicatedServer,
                "counter",
                ravel_sdk::json!({ "count": 1 }),
            )
            .expect("a server reaches it");
        let envelope = shared
            .save_saved_data()
            .expect("the envelope is well formed")
            .expect("a store holding something produces one");
        assert_eq!(envelope.payload()["scope"], save::SCOPE);
    }

    #[test]
    fn a_stored_global_record_reaches_the_next_visit_through_the_envelope() {
        let mut first = Entry::new(Side::Client);
        first
            .global_mut()
            .write(
                Side::DedicatedServer,
                "counter",
                ravel_sdk::json!({ "count": 4 }),
            )
            .expect("a server reaches it");
        let envelope = first
            .save_saved_data()
            .expect("the envelope is well formed")
            .expect("a store holding something produces one");

        let mut second = Entry::new(Side::Client);
        second
            .open_saved_data(Some(&envelope))
            .expect("what was written reads back");
        assert_eq!(
            second
                .global()
                .read(Side::DedicatedServer, "counter")
                .expect("a server reaches it"),
            Some(&ravel_sdk::json!({ "count": 4 })),
            "a second visit holds what the first one saved"
        );
    }

    #[test]
    fn a_world_holding_nothing_opens_the_store_empty_rather_than_refusing() {
        let mut shared = Entry::new(Side::Client);
        shared
            .global_mut()
            .write(Side::DedicatedServer, "left_over", ravel_sdk::json!({}))
            .expect("a server reaches it");
        shared.open_saved_data(None).expect("absence initializes");
        assert!(
            shared.global().is_empty(),
            "a fresh world leaves nothing of the last one behind"
        );
    }

    #[test]
    fn a_payload_pinned_to_another_scope_withholds_the_world() {
        let declaration = GlobalData::declaration().expect("the declared version is legal");
        let envelope = SavedDataEnvelope::current(
            Entry::declared_id().clone(),
            declaration.current(),
            ravel_sdk::json!({ "scope": "the_nether", "records": {} }),
        );
        let mut shared = Entry::new(Side::Client);
        assert!(
            shared.open_saved_data(Some(&envelope)).is_err(),
            "a record stored against another scope is refused rather than adopted"
        );
    }

    #[test]
    fn the_manual_latch_starts_unset_and_absence_is_not_an_error() {
        let mut shared = Entry::new(Side::Client);
        assert_eq!(shared.manual_present(), None);
        shared.latch_manual(false);
        assert_eq!(shared.manual_present(), Some(false));
        shared.latch_manual(true);
        assert_eq!(
            shared.manual_present(),
            Some(true),
            "the latch may be re-read, unlike the original's one-time one"
        );
    }

    #[test]
    fn the_key_binding_is_a_press_rather_than_a_poll_of_a_held_key() {
        let mut shared = Entry::new(Side::Client);
        assert!(
            shared.poll_manual_key(true, true),
            "the press opens the manual"
        );
        assert!(
            !shared.poll_manual_key(true, true),
            "holding the key does not re-open it on every tick"
        );
        shared.poll_manual_key(false, true);
        assert!(
            shared.poll_manual_key(true, true),
            "releasing arms the next press"
        );
    }

    #[test]
    fn the_binding_does_nothing_while_no_screen_of_this_package_is_open() {
        let mut shared = Entry::new(Side::Client);
        assert!(!shared.poll_manual_key(true, false));
        assert!(
            !shared.poll_manual_key(true, true),
            "a key already down when a screen opens is not a fresh press"
        );
    }

    #[test]
    fn the_package_names_four_optional_integrations_and_stubs_none() {
        let mut named = INTEGRATIONS.to_vec();
        named.sort_unstable();
        named.dedup();
        assert_eq!(named.len(), 4);
    }

    #[test]
    fn what_dependents_need_is_reachable_from_the_crate_root() {
        let _: fn(&[i32]) -> i32 = check32_over;
        let _: fn(&Table64, &[u32]) -> u64 = check64_over;
        let _: fn(&[u8]) -> Vec<u8> = code_runs;
        let _: fn(&[u8]) -> Result<Vec<u8>, primitives::RunError> = read_runs;

        assert_eq!(clamp_energy(i64::MAX), i32::MAX);
        assert_eq!(reported_energy(5, 0), 5);
    }

    #[test]
    fn a_dependent_declares_its_commands_into_the_one_table() {
        let mut shared = Entry::new(Side::Client);
        let key = CommandKey::new("dependent", "act").expect("a legal key");
        shared
            .commands_mut()
            .declare(
                commands::Where::Server,
                key.clone(),
                Declaration::fire_and_forget("act"),
                Box::new(|_| commands::Answer::Nothing),
            )
            .expect("a legal declaration");
        assert_eq!(
            shared
                .commands()
                .dispatch(commands::Where::Server, &key, &[]),
            Ok(Some(commands::Answer::Nothing)),
            "what a dependent declared is what a call runs"
        );
    }

    #[test]
    fn a_dependent_places_and_removes_through_the_one_grouping_service() {
        let mut shared = Entry::new(Side::Client);
        let first = Position::new(0, 0, 0);
        shared
            .grouping_mut()
            .place(first, &|_, _| true)
            .expect("a legal placement");
        assert_eq!(shared.grouping().group_of(first), Some(1));
    }

    #[test]
    fn a_configured_library_holds_the_settings_and_the_colours_it_resolved() {
        let declaration = settings::declaration().expect("the packaged declaration is legal");
        let resolved = LayeredConfiguration::resolve(declaration, None, None)
            .expect("the author's defaults resolve");
        let mut shared = Entry::new(Side::Client);
        shared.configure(&resolved).expect("the defaults are read");

        assert_eq!(
            shared.settings().expect("configured settings"),
            &Settings::shipped().expect("packaged defaults")
        );
        assert_eq!(shared.colours().values().len(), settings::COLOUR_COUNT);
        assert!(
            shared
                .colours()
                .values()
                .iter()
                .all(|value| value >> 24 == 0xFF)
        );
    }
}
