#![forbid(unsafe_code)]

//! An original Ravel implementation of the behaviour specified in RVL-060.
//!
//! A shared library with no gameplay of its own. It contributes no block, no
//! item, no recipe, no tag and no loot to any world, and a player never
//! encounters it except through one client command. Its whole purpose is to be
//! depended on, and the two dependents the pack graph records both require it on
//! both sides.
//!
//! What a dependent reaches for:
//!
//! - [`declaration`] — declaring content, its nesting and its one refusal;
//! - [`channel`] — the channel and packet identifier derivations, and who a send
//!   reaches;
//! - [`persistence`] — the durable per-package directory and the two saved-data
//!   envelopes;
//! - [`transport`] — the full signed range through a sixteen-bit channel;
//! - [`matching`] — block state matching and the three composite predicates;
//! - [`data`] — the stored-data model and the comparison a pack author writes
//!   against;
//! - [`validators`] — the eleven tag validator kinds in four families;
//! - [`recipes`] — the recipe, ingredient and menu payload shapes;
//! - [`fluids`] — the fluid property vocabulary and the lazy client lookup;
//! - [`presentation`] — the scissor stack, the cursors, the screen memory and
//!   the system facts;
//! - [`highlights`] — declared resource packs and block outline documents;
//! - [`colour`] — the packed layout, the naming table and the two encodings.
//! - [`dumps`] — the derived tables written out beside this source.
//!
//! `NOTES.md` beside this file says which sections are carried here, which are
//! declarable only because Ravel publishes no seam for them, and which are
//! declined outright.
//!
//! # What this package declines outright
//!
//! Three things in the original are available in principle and are not
//! implemented here at any Ravel revision. [`DECLINED`] names them and nothing
//! stubs one:
//!
//! - **Direct memory and reflection access.** A helper wrapping the Java
//!   runtime's unsupported low-level memory interface, obtained by making a
//!   private field accessible, and a second that adds values to an enumeration
//!   at runtime by writing the class's hidden constant array. Ravel forbids
//!   unsafe code in every maintained crate with no exception, and this is the
//!   category the rule exists for. A dependent needing an extensible enumeration
//!   is a Ravel feature request rather than a package's problem.
//! - **Outbound network requests.** A synchronous fetch helper that any
//!   dependent could use to reach an arbitrary address from the game process.
//!   That is a capability decision rather than a utility and it belongs to
//!   Ravel's own permission surface.
//! - **A background timer pool.** Ten threads created at class load, never shut
//!   down, driving colour animation that mutates shared state the drawing thread
//!   reads without synchronisation. [`colour::Animated`] is a function of the
//!   frame instead, which removes the pool and the race together.
//!
//! The client command is a fourth candidate and is kept rather than declined: it
//! is claimed at the registration barrier under [`COMMAND`] and run through
//! [`Mod::run_command`], and it gathers [`presentation::SystemFacts`] into a
//! formatted block for a user to paste into a support channel. It reads only
//! what the game already knows about its own machine, sends nothing, and stays
//! explicitly user-invoked. Ravel publishes no clipboard, so the text is held
//! for the host to place through [`Entry::take_report`].

pub mod channel;
pub mod colour;
pub mod data;
pub mod declaration;
pub mod dumps;
pub mod fluids;
pub mod highlights;
pub mod matching;
pub mod persistence;
pub mod presentation;
pub mod recipes;
pub mod transport;
pub mod validators;

use ravel_sdk::content::{ContentDeclaration, ContentId, ContentKind, RegistryKey};
use ravel_sdk::events::{CommandInvocation, GameEvent};
use ravel_sdk::prelude::*;
use ravel_sdk::sides::{ClientOnly, ClientOnlyError};

pub use crate::channel::{Body, Channel, Kind, Peer, Position, Suffixes, Target};
pub use crate::colour::{Animated, Colour, Names};
pub use crate::data::{DataPredicate, Tag};
pub use crate::declaration::{Handle, Registry, Special};
pub use crate::highlights::{Outlines, PackKind, Spellings};
pub use crate::matching::{BlockPredicate, EntityPredicate, ItemPredicate, StatePredicate};
pub use crate::persistence::{DurableStorage, RawEnvelope, Record};
pub use crate::presentation::{Cursors, ScissorStack, ScreenMemory, SystemFacts};
pub use crate::recipes::{Ingredient, MenuProvider, RecipeShape};
pub use crate::transport::WideValues;
pub use crate::validators::{Validator, Vocabulary};

/// The identity this package is declared and validated under.
pub const MOD_ID: &str = "rvl_060_counterpart";

/// The three areas this package declines outright, by the role each plays.
///
/// Declined rather than host-blocked: each is available in principle and none
/// should be implemented at any Ravel revision. Nothing here is a stub.
pub const DECLINED: [&str; 3] = [
    "direct_memory_access",
    "outbound_requests",
    "background_timer_pool",
];

/// The one command a player can invoke.
pub const COMMAND: &str = "support_report";

/// Where a side-appropriate registry accessor reads from.
///
/// At construction the original installs one of two accessors: on a client
/// build, one that reads the client's connection to a server; otherwise one that
/// reads the running server. Asking either before the respective thing exists is
/// a hard failure rather than an empty answer, which is the part a dependent can
/// observe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrySource {
    /// A client build reads its connection to a server.
    Connection,
    /// Any other build reads the running server.
    RunningServer,
}

impl RegistrySource {
    /// Returns the accessor a side installs.
    #[must_use]
    pub const fn for_side(side: Side) -> Self {
        match side {
            Side::Client => Self::Connection,
            Side::IntegratedServer | Side::DedicatedServer => Self::RunningServer,
        }
    }

    /// Returns what the accessor reads from.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Connection => "connection",
            Self::RunningServer => "running-server",
        }
    }
}

/// The registry accessor installed at construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryAccess {
    source: RegistrySource,
    present: bool,
}

impl RegistryAccess {
    /// Installs the accessor a side calls for, with nothing there yet.
    #[must_use]
    pub const fn installed(side: Side) -> Self {
        Self {
            source: RegistrySource::for_side(side),
            present: false,
        }
    }

    /// Records that what the accessor reads from now exists.
    pub const fn mark_present(&mut self) {
        self.present = true;
    }

    /// Records that what the accessor reads from is gone.
    pub const fn mark_absent(&mut self) {
        self.present = false;
    }

    /// Returns what the accessor reads from.
    #[must_use]
    pub const fn source(&self) -> RegistrySource {
        self.source
    }

    /// Returns the source only when its registry set is present.
    ///
    /// # Errors
    ///
    /// Returns [`AccessUnavailable`] when the thing the accessor reads from does
    /// not exist yet. That is a hard failure rather than an empty answer,
    /// because an empty registry set reads as "the game has nothing" and a
    /// dependent would believe it.
    pub const fn require_source(&self) -> Result<RegistrySource, AccessUnavailable> {
        if self.present {
            Ok(self.source)
        } else {
            Err(AccessUnavailable {
                source: self.source,
            })
        }
    }
}

/// The registry set was asked for before the thing behind it existed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccessUnavailable {
    /// What the accessor would have read from.
    pub source: RegistrySource,
}

impl std::fmt::Display for AccessUnavailable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "the registry set was asked for before the {} existed",
            self.source.as_str()
        )
    }
}

impl std::error::Error for AccessUnavailable {}

/// What only a client holds.
///
/// The memory that restores a screen, the naming table colours resolve through
/// and the loaded block outlines are values a server side has no reason to hold.
/// They go in a [`ClientOnly`] rather than behind a runtime side test, so on a
/// server side they are not built at all.
#[derive(Debug, Default)]
pub struct ClientState {
    /// The memory that restores a screen.
    pub screens: ScreenMemory,
    /// The naming table colours resolve through.
    pub names: Names,
    /// The outlines loaded for block highlights.
    pub outlines: Outlines,
}

/// The shared library, as the loader drives it.
#[derive(Debug)]
pub struct Entry {
    side: Side,
    declaration: Result<ContentDeclaration, ModError>,
    registries: RegistryAccess,
    storage: DurableStorage,
    facts: SystemFacts,
    client: ClientOnly<ClientState>,
    channels: Vec<Channel>,
    observed: Vec<GameEvent>,
    entered: Vec<LifecycleState>,
    report: Option<String>,
}

impl Entry {
    /// Builds the library on a side, with an explicitly chosen durable root.
    ///
    /// Construction is where §2's two construction-time steps land: the durable
    /// directory is named, and the side-appropriate registry accessor is
    /// installed. Neither creates anything and neither reads a file, which is
    /// the difference between this and the original's construction.
    #[must_use]
    pub fn new(side: Side, durable_root: &str) -> Self {
        let mut declaration = ContentDeclaration::new(Self::declared_id().clone());
        let declared = declaration
            .entry(ContentKind::Command, COMMAND)
            .map(|_| ())
            .map_err(ModError::from_display);
        let declaration = declared.map(|()| declaration);
        Self {
            side,
            declaration,
            registries: RegistryAccess::installed(side),
            storage: DurableStorage::under(durable_root),
            facts: SystemFacts::new(),
            client: ClientOnly::built_on(side, ClientState::default),
            channels: Vec::new(),
            observed: Vec::new(),
            entered: Vec::new(),
            report: None,
        }
    }

    /// Returns the key the registration barrier claims for the one command.
    ///
    /// # Panics
    ///
    /// Panics when [`COMMAND`] is not a legal content path, which for a
    /// constant would be a packaging failure rather than a runtime one.
    #[must_use]
    pub fn command_key() -> RegistryKey {
        RegistryKey::new(
            ContentKind::Command,
            ContentId::new(Self::declared_id().clone(), COMMAND)
                .expect("the one command names a legal content path"),
        )
    }

    /// Returns the side this was built on.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Returns the registry accessor installed at construction.
    #[must_use]
    pub const fn registries(&self) -> &RegistryAccess {
        &self.registries
    }

    /// Returns the registry accessor for driving.
    pub const fn registries_mut(&mut self) -> &mut RegistryAccess {
        &mut self.registries
    }

    /// Returns the durable storage under the root chosen at construction.
    #[must_use]
    pub const fn storage(&self) -> &DurableStorage {
        &self.storage
    }

    /// Returns the system facts the one client command formats.
    #[must_use]
    pub const fn facts(&self) -> &SystemFacts {
        &self.facts
    }

    /// Returns the system facts for a dependent to add its own to.
    pub const fn facts_mut(&mut self) -> &mut SystemFacts {
        &mut self.facts
    }

    /// Declares a channel.
    ///
    /// **Host-blocked in one narrow place.** The original collects channels into
    /// a process-wide list so they can all be registered at one loader event.
    /// Ravel has no such event and needs none: each contract is declared through
    /// [`ravel_sdk::network`], so what a dependent hands over is kept beside the
    /// package that owns it rather than in a static.
    pub fn declare_channel(&mut self, channel: Channel) {
        self.channels.push(channel);
    }

    /// Returns every declared channel, in declaration order.
    #[must_use]
    pub fn channels(&self) -> &[Channel] {
        &self.channels
    }

    /// Returns what only a client holds.
    ///
    /// # Errors
    ///
    /// Returns [`ClientOnlyError::WrongSide`] on a server side, where the value was never built.
    pub fn client(&self) -> Result<&ClientState, ClientOnlyError> {
        self.client.get(self.side)
    }

    /// Returns what only a client holds, for changing.
    ///
    /// # Errors
    ///
    /// Returns [`ClientOnlyError::WrongSide`] on a server side.
    pub fn client_mut(&mut self) -> Result<&mut ClientState, ClientOnlyError> {
        self.client.get_mut(self.side)
    }

    /// Returns whether the client-only work exists on this side at all.
    #[must_use]
    pub fn has_client_state(&self) -> bool {
        self.client.is_present()
    }

    /// Forgets what the memory restores, which is what leaving a world does.
    ///
    /// **Declarable only in its trigger.** Ravel publishes no disconnect event,
    /// so nothing here calls this on its own; the rule is implemented and the
    /// caller is the host's. A server side has nothing to forget and this costs
    /// it nothing.
    pub fn on_disconnect(&mut self) {
        if let Ok(client) = self.client_mut() {
            client.screens.on_disconnect();
        }
    }

    /// Returns the lifecycle steps taken, in order.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }

    /// Returns the events observed, in order.
    #[must_use]
    pub fn observed(&self) -> &[GameEvent] {
        &self.observed
    }

    /// Returns the formatted support report the one client command produces.
    ///
    /// Explicitly user-invoked, reads only what the game already knows about its
    /// own machine, and sends nothing.
    #[must_use]
    pub fn support_report(&self) -> String {
        self.facts.report()
    }

    /// Takes the report the last invocation produced, leaving nothing behind.
    ///
    /// **Declarable only in its last step.** Ravel publishes no clipboard and no
    /// chat surface, so the text the command produced is held for the host to
    /// place rather than placed here. Taking it is what makes a second
    /// invocation produce a second report rather than read the first one.
    pub fn take_report(&mut self) -> Option<String> {
        self.report.take()
    }
}

#[declare_mod(id = "rvl_060_counterpart")]
impl Mod for Entry {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        if state == LifecycleState::Constructed {
            self.declaration.as_ref().map_err(Clone::clone)?;
        }
        self.entered.push(state);
        Ok(())
    }

    fn content_declaration(
        &self,
    ) -> Result<Option<std::borrow::Cow<'_, ContentDeclaration>>, ModError> {
        self.declaration
            .as_ref()
            .map(|declaration| Some(std::borrow::Cow::Borrowed(declaration)))
            .map_err(Clone::clone)
    }

    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        self.observed.push(event);
        Ok(())
    }

    fn run_command(&mut self, invocation: &CommandInvocation) -> Result<(), ModError> {
        if invocation.key != Self::command_key() {
            return Err(ModError::refused(format!(
                "`{}` is not this package's command",
                invocation.key
            )));
        }
        self.report = Some(self.support_report());
        Ok(())
    }
}

/// The shipped durable paths are relative to the process working directory.
const DURABLE_ROOT: &str = ".";

fn shipped_entry(side: Side) -> Entry {
    Entry::new(side, DURABLE_ROOT)
}

export_mod!(|side| shipped_entry(side));

/// This package's own sources, so a test can assert on what they do not hold.
///
/// `#![forbid(unsafe_code)]` proves the first decline for the compiler; the
/// other two are the absence of a capability rather than a rule the compiler
/// knows, so the proof is that nothing here names one.
#[cfg(test)]
const SOURCES: [(&str, &str); 16] = [
    ("lib.rs", include_str!("lib.rs")),
    ("channel.rs", include_str!("channel.rs")),
    ("colour.rs", include_str!("colour.rs")),
    ("data.rs", include_str!("data.rs")),
    ("declaration.rs", include_str!("declaration.rs")),
    ("dumps.rs", include_str!("dumps.rs")),
    ("fluids.rs", include_str!("fluids.rs")),
    ("highlights.rs", include_str!("highlights.rs")),
    (
        "highlights/declarations.rs",
        include_str!("highlights/declarations.rs"),
    ),
    ("matching.rs", include_str!("matching.rs")),
    ("presentation.rs", include_str!("presentation.rs")),
    ("recipes.rs", include_str!("recipes.rs")),
    ("persistence.rs", include_str!("persistence.rs")),
    ("transport.rs", include_str!("transport.rs")),
    ("validators.rs", include_str!("validators.rs")),
    (
        "validators/conditions.rs",
        include_str!("validators/conditions.rs"),
    ),
];

#[cfg(test)]
mod tests {
    use ravel_sdk::content::{ContentId, ContentKind, RegistryKey};
    use ravel_sdk::events::{CommandInvocation, GameEvent};
    use ravel_sdk::prelude::*;

    use super::presentation::Category;
    use super::{
        Channel, DECLINED, Entry, MOD_ID, RegistryAccess, RegistrySource, SOURCES, Suffixes,
    };

    #[test]
    fn the_package_is_declared_under_the_identity_its_manifest_names() {
        assert_eq!(Entry::declared_id().as_str(), MOD_ID);
    }

    #[test]
    fn the_host_can_discover_the_command_before_registration() {
        let shared = Entry::new(Side::Client, "/root");
        let declaration = shared
            .content_declaration()
            .expect("a valid declaration")
            .expect("the command is declared");
        assert_eq!(declaration.owner(), Entry::declared_id());
        assert_eq!(declaration.entries(), &[Entry::command_key()]);
    }

    #[test]
    fn the_claimed_command_runs_and_produces_the_report_for_the_host_to_place() {
        let mut shared = Entry::new(Side::Client, "/root");
        shared
            .facts_mut()
            .add(Category::Game, "version", "1.21.1")
            .expect("inside the ceiling");
        assert_eq!(shared.take_report(), None, "nothing has been invoked yet");
        shared
            .run_command(&CommandInvocation::new(
                Entry::command_key(),
                Side::Client,
                Vec::new(),
            ))
            .expect("the package claimed this command, so it runs it");
        assert_eq!(
            shared.take_report().as_deref(),
            Some("[game]\nversion: 1.21.1")
        );
        assert_eq!(
            shared.take_report(),
            None,
            "a report is handed over once, not read twice"
        );
    }

    #[test]
    fn a_command_this_package_did_not_claim_is_refused_rather_than_answered() {
        let mut shared = Entry::new(Side::Client, "/root");
        let other = RegistryKey::new(
            ContentKind::Command,
            ContentId::new(Entry::declared_id().clone(), "something_else")
                .expect("a legal content path"),
        );
        shared
            .run_command(&CommandInvocation::new(other, Side::Client, Vec::new()))
            .expect_err("nothing else was claimed");
        assert_eq!(shared.take_report(), None);
    }

    #[test]
    fn a_client_build_reads_its_connection_and_any_other_reads_the_running_server() {
        assert_eq!(
            RegistrySource::for_side(Side::Client),
            RegistrySource::Connection
        );
        for side in [Side::IntegratedServer, Side::DedicatedServer] {
            assert_eq!(
                RegistrySource::for_side(side),
                RegistrySource::RunningServer
            );
        }
        assert_eq!(RegistrySource::Connection.as_str(), "connection");
        assert_eq!(RegistrySource::RunningServer.as_str(), "running-server");
    }

    #[test]
    fn asking_for_the_registry_set_before_it_exists_is_a_hard_failure() {
        let mut access = RegistryAccess::installed(Side::Client);
        assert_eq!(access.source(), RegistrySource::Connection);
        let refusal = access
            .require_source()
            .expect_err("nothing has connected yet");
        assert_eq!(refusal.source, RegistrySource::Connection);
        assert_ne!(
            refusal.to_string(),
            "",
            "an empty answer would read as `the game has nothing`"
        );
        access.mark_present();
        assert_eq!(access.require_source(), Ok(RegistrySource::Connection));
        access.mark_absent();
        assert!(access.require_source().is_err());
    }

    #[test]
    fn the_accessor_a_side_installs_is_the_one_it_keeps() {
        let shared = Entry::new(Side::DedicatedServer, "/root");
        assert_eq!(shared.registries().source(), RegistrySource::RunningServer);
        assert!(shared.registries().require_source().is_err());
    }

    #[test]
    fn the_shipped_entry_names_its_working_directory_relative_storage() {
        let entry = super::shipped_entry(Side::DedicatedServer);
        assert_eq!(entry.storage().root(), ".");
        assert_eq!(
            entry.storage().for_package("example"),
            Ok("./example".to_owned())
        );
    }

    #[test]
    fn the_durable_directory_is_named_at_construction_and_creates_nothing() {
        let shared = Entry::new(Side::DedicatedServer, "/wherever/ravel/chose");
        assert_eq!(
            shared.storage().for_package("example"),
            Ok("/wherever/ravel/chose/example".to_owned())
        );
        assert_eq!(shared.storage().root(), "/wherever/ravel/chose");
    }

    #[test]
    fn the_client_only_state_is_not_built_on_a_server_side_rather_than_hidden_there() {
        let client = Entry::new(Side::Client, "/root");
        assert!(client.has_client_state());
        assert!(client.client().is_ok());
        assert_eq!(client.side(), Side::Client);
        for side in [Side::IntegratedServer, Side::DedicatedServer] {
            let server = Entry::new(side, "/root");
            assert!(!server.has_client_state());
            assert!(server.client().is_err());
        }
    }

    #[test]
    fn a_disconnect_clears_the_screen_memory_on_a_client_and_costs_a_server_nothing() {
        let mut client = Entry::new(Side::Client, "/root");
        client
            .client_mut()
            .expect("a client holds its own state")
            .screens
            .remember("catalogue", "scroll", ravel_sdk::json!({ "row": 3 }));
        client.on_disconnect();
        assert_eq!(
            client
                .client()
                .expect("a client holds its own state")
                .screens
                .len(),
            0,
            "the original never clears this, so state crosses worlds"
        );

        let mut server = Entry::new(Side::DedicatedServer, "/root");
        server.on_disconnect();
        assert!(server.client().is_err());
    }

    #[test]
    fn the_whole_lifecycle_runs_and_every_step_is_recorded_in_order() {
        let mut shared = Entry::new(Side::IntegratedServer, "/root");
        for state in LifecycleState::ALL {
            shared.enter(state).expect("the library accepts every step");
        }
        assert_eq!(shared.entered(), LifecycleState::ALL);
    }

    #[test]
    fn every_observed_event_is_recorded_in_the_order_it_arrived() {
        let mut shared = Entry::new(Side::Client, "/root");
        for event in GameEvent::ALL {
            shared.observe(event).expect("every event is recorded");
        }
        assert_eq!(shared.observed(), GameEvent::ALL);
    }

    #[test]
    fn a_channel_is_declared_beside_the_package_rather_than_in_a_process_wide_list() {
        let mut shared = Entry::new(Side::DedicatedServer, "/root");
        assert_eq!(shared.channels().len(), 0);
        shared.declare_channel(Channel::new(
            Entry::declared_id().clone(),
            "shared",
            1,
            true,
            Suffixes::placeholder(),
        ));
        assert_eq!(shared.channels().len(), 1);
        assert_eq!(shared.channels()[0].effective_id(), "shared-v1");
        assert!(shared.channels()[0].is_optional());
    }

    #[test]
    fn the_one_command_reads_what_the_game_knows_and_sends_nothing() {
        let mut shared = Entry::new(Side::Client, "/root");
        shared
            .facts_mut()
            .add(Category::Game, "version", "1.21.1")
            .expect("inside the ceiling");
        assert_eq!(shared.support_report(), "[game]\nversion: 1.21.1");
        assert_eq!(shared.facts().count(), 1);
    }

    #[test]
    fn the_three_declined_areas_are_named_once_each() {
        let mut sorted = DECLINED.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), DECLINED.len());
        assert!(DECLINED.iter().all(|area| !area.is_empty()));
    }

    #[test]
    fn nothing_in_this_package_implements_one_of_the_declined_areas() {
        // Each marker is spelled in two halves so that this test's own source,
        // which is one of the sources it reads, does not match it.
        let forbidden = [
            ("std::", "net"),
            ("std::", "thread"),
            ("Tcp", "Stream"),
            ("Udp", "Socket"),
            ("thread::", "spawn"),
        ];
        assert!(
            SOURCES[0].1.starts_with("#![forbid(unsafe_code)]"),
            "the first decline is the compiler's to enforce and this is where it is said"
        );
        for (name, source) in SOURCES {
            for (head, tail) in forbidden {
                let marker = format!("{head}{tail}");
                assert!(
                    !source.contains(&marker),
                    "`{name}` names `{marker}`, which is one of the three declined areas"
                );
            }
        }
    }
}
