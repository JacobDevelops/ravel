#![forbid(unsafe_code)]

//! An original Ravel implementation of the behaviour specified in RVL-146.
//!
//! A shared library with no gameplay of its own. It contributes no block, no
//! item, no recipe, no tag and no loot to any world, and a player never
//! encounters it. Its whole purpose is to be depended on, and the three
//! dependents the pack graph records all require it on both sides.
//!
//! What a dependent reaches for:
//!
//! - [`registration`] — what a dependent may declare, its declarable kinds, its
//!   identifier composition and its capability attachment;
//! - [`channel`] — the payload channel, the four send targets and the running
//!   server a radius send needs;
//! - [`transport`] — the argument wire format, its eight types and its bounds;
//! - [`menus`] — the two generic action payloads and the shape each action
//!   declares;
//! - [`recipes`] — the recipe search and its one-shot grace period;
//! - [`persistence`] — the saved recipe memory, which here survives a restart;
//! - [`quantity`] — the ingredient with an amount and its arithmetic;
//! - [`colour`] — the four channels, the three packings and the equality;
//! - [`highlights`] — the block highlights, their lifetimes and what clears them;
//! - [`shapes`] — the codec helpers a dependent composes with;
//! - [`tools`] — the four small utilities;
//! - [`dumps`] — the generated documents, rendered from the modules above.
//!
//! `NOTES.md` beside this file says which sections are carried here, which are
//! declarable only because Ravel publishes no seam for them, and which are
//! declined outright.
//!
//! # What this package declines outright
//!
//! Two things in the original are available in principle and are not implemented
//! here at any Ravel revision. [`DECLINED`] names them and nothing stubs one:
//!
//! - **A reflection kit.** It locates a method or a field by trying several
//!   candidate names in turn and makes it accessible, stripping the final
//!   modifier from a static final field as it goes. The several-name search
//!   exists to survive naming differences between builds, and Ravel's answer to
//!   that problem is its generated-name mapping, which is a first-class supported
//!   surface. The need is already met.
//! - **Direct memory access.** Underneath the kit sits a helper that obtains the
//!   Java runtime's unsupported low-level memory interface by making a private
//!   field accessible, then reads a private static field at a computed memory
//!   offset, and offers typed reads and writes of arbitrary fields through raw
//!   offsets. Ravel forbids unsafe code in every maintained crate with no
//!   exception, and this is the category that rule exists for. A dependent that
//!   needs to reach into another package's private state is a Ravel feature
//!   request rather than a package's problem.
//!
//! Declining both removes nothing a dependent can observe through a declared API:
//! neither has a caller inside the original's own tree.
//!
//! # One thing the original does that this does not
//!
//! **Single construction is answerable.** The original guards its process-wide
//! slot with a language assertion, and assertions are disabled in a normal
//! launch, so a second construction silently replaces the first and every holder
//! of the first is left pointing at a library nothing else uses.
//! [`Entry::sole`] claims the slot and *answers* [`AlreadyConstructed`] for a
//! second claim, which is the refusal the original swallows.
//!
//! The exported entry builds with [`Entry::new`], which claims nothing. A host
//! is not the only caller of that entry — the generated members function drives
//! it from a test or a diagnostic with no JVM in sight — so a second call in one
//! process is a supported path, and an entry that faulted there would unwind out
//! of the host on the JNI path. Single construction is a question this package
//! answers for a caller that asked it, not a rule it enforces by falling over.

pub mod channel;
pub mod colour;
pub mod dumps;
pub mod highlights;
pub mod menus;
pub mod persistence;
pub mod quantity;
pub mod recipes;
pub mod registration;
pub mod shapes;
pub mod tools;
pub mod transport;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

use ravel_sdk::content::{ContentDeclaration, Registrar, claim_all};
use ravel_sdk::events::GameEvent;
use ravel_sdk::prelude::*;
use ravel_sdk::save::{SavedDataDeclaration, SavedDataEnvelope, SavedDataPlan};
use ravel_sdk::sides::WrongSide;

pub use crate::channel::{Channel, Sends, Target};
pub use crate::colour::Colour;
pub use crate::highlights::{Highlights, Moment};
pub use crate::menus::{Actions, Dispatch, Surface};
pub use crate::persistence::{Memory, ReadFailure, Resolution, Spellings};
pub use crate::quantity::{Flavour, Matcher, Portion, Stack};
pub use crate::recipes::{Catalogue, Outcome, Search};
pub use crate::registration::{ClassId, Collection, ItemForm, Marker, Registered};
pub use crate::transport::{Action, Argument, ArgumentKind, TagNumbering};

/// The identity this package is declared and validated under.
pub const MOD_ID: &str = "rvl_146_counterpart";

/// The two areas this package declines outright, by the role each plays.
///
/// Declined rather than host-blocked: each is available in principle and neither
/// should be implemented at any Ravel revision. Nothing here is a stub.
pub const DECLINED: [&str; 2] = ["reflection_kit", "direct_memory_access"];

/// The areas Ravel publishes no seam for at the reviewed revision.
///
/// Each is declared rather than stubbed, and `NOTES.md` says what is implemented
/// in each case and what is waiting on a seam.
pub const HOST_BLOCKERS: [&str; 5] = [
    "world_space_drawing",
    "block_entity_type_registration",
    "data_component_registration",
    "capability_registration",
    "packet_carriage",
];

/// How long one client tick is, in milliseconds.
///
/// The monotonic clock highlights expire against is advanced by this on every
/// client tick, so a lifetime is a duration in game time rather than a reading of
/// the machine's wall clock. A host with a real monotonic source calls
/// [`Entry::advance_to`] instead and this is never consulted.
pub const TICK_MILLIS: u64 = 50;

/// The most saved recipe memories one world holds through this library.
pub const MAX_MEMORIES: usize = 4096;

/// The most characters a memory's owner name may carry.
pub const MAX_MEMORY_NAME: usize = 128;

/// Whether the process-wide slot is taken.
static CONSTRUCTED: AtomicBool = AtomicBool::new(false);

/// The library was constructed twice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AlreadyConstructed;

impl fmt::Display for AlreadyConstructed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the shared library is already constructed in this process")
    }
}

impl std::error::Error for AlreadyConstructed {}

/// Why a memory was not held.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryRefused {
    /// A memory with no owner name.
    NoName,
    /// An owner name longer than [`MAX_MEMORY_NAME`].
    NameTooLong(usize),
    /// The world already holds [`MAX_MEMORIES`].
    Full,
}

impl fmt::Display for MemoryRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoName => formatter.write_str("a saved memory with no owner"),
            Self::NameTooLong(length) => {
                write!(formatter, "{length} characters is past {MAX_MEMORY_NAME}")
            }
            Self::Full => write!(formatter, "at most {MAX_MEMORIES} memories are held"),
        }
    }
}

impl std::error::Error for MemoryRefused {}

/// What only a client holds.
///
/// Highlights are a client's, so on a server side they are not built at all
/// rather than built and hidden.
#[derive(Clone, Debug, Default)]
pub struct ClientState {
    /// Every highlight being held.
    pub highlights: Highlights,
    /// The monotonic reading highlights expire against.
    pub clock: Moment,
}

/// How many of each event have been seen.
///
/// A count rather than a list, because a list of every tick a session saw is a
/// vector that only grows. Each count saturates rather than wrapping.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EventCounts {
    /// Server ticks.
    pub server_ticks: u64,
    /// Client ticks.
    pub client_ticks: u64,
    /// Pack reloads.
    pub reloads: u64,
}

impl EventCounts {
    fn record(&mut self, event: GameEvent) {
        let counter = match event {
            GameEvent::ServerTick => &mut self.server_ticks,
            GameEvent::ClientTick => &mut self.client_ticks,
            GameEvent::ResourcesReloaded => &mut self.reloads,
        };
        *counter = counter.saturating_add(1);
    }
}

/// The shared library, as the loader drives it.
#[derive(Debug)]
pub struct Entry {
    side: Side,
    collection: Collection,
    sends: Sends,
    spellings: Spellings,
    memories: BTreeMap<String, Memory>,
    recovered: Vec<ReadFailure>,
    client: ClientOnly<ClientState>,
    counts: EventCounts,
    entered: Vec<LifecycleState>,
}

impl Entry {
    /// Builds the library on a side.
    ///
    /// This does not claim the process-wide slot: [`Entry::sole`] does, and it
    /// is what a host calls.
    #[must_use]
    pub fn new(side: Side) -> Self {
        Self {
            side,
            collection: Collection::new(Self::owner()),
            sends: Sends::idle(),
            spellings: Spellings::default(),
            memories: BTreeMap::new(),
            recovered: Vec::new(),
            client: ClientOnly::built_on(side, ClientState::default),
            counts: EventCounts::default(),
            entered: Vec::new(),
        }
    }

    /// Builds the one library this process may hold.
    ///
    /// # Errors
    ///
    /// Returns [`AlreadyConstructed`] for a second construction. The original
    /// guards this with a language assertion, which a normal launch disables, so
    /// the second construction silently replaces the first.
    pub fn sole(side: Side) -> Result<Self, AlreadyConstructed> {
        if CONSTRUCTED.swap(true, Ordering::SeqCst) {
            return Err(AlreadyConstructed);
        }
        Ok(Self::new(side))
    }

    /// Returns the identity this package is declared under.
    ///
    /// # Panics
    ///
    /// Never: [`MOD_ID`] is a legal mod id.
    #[must_use]
    pub fn owner() -> ModId {
        MOD_ID
            .parse()
            .expect("the declared identity is a legal mod id")
    }

    /// Returns the side this was built on.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Returns the collection a dependent declares content into.
    #[must_use]
    pub const fn collection(&self) -> &Collection {
        &self.collection
    }

    /// Returns the collection for declaring into.
    pub const fn collection_mut(&mut self) -> &mut Collection {
        &mut self.collection
    }

    /// Returns the send helpers.
    #[must_use]
    pub const fn sends(&self) -> &Sends {
        &self.sends
    }

    /// Returns the send helpers, which is where a host records that a server
    /// started or stopped.
    ///
    /// **Declarable only in its trigger.** Ravel publishes no server-started
    /// event, so the caller is the host's; what a radius send does with and
    /// without one is here.
    pub const fn sends_mut(&mut self) -> &mut Sends {
        &mut self.sends
    }

    /// Returns the saved spellings this build writes a world with.
    #[must_use]
    pub const fn spellings(&self) -> &Spellings {
        &self.spellings
    }

    /// Holds one processing block's recipe memory.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryRefused::NoName`] for an unnamed owner,
    /// [`MemoryRefused::NameTooLong`] past [`MAX_MEMORY_NAME`], and
    /// [`MemoryRefused::Full`] past [`MAX_MEMORIES`].
    pub fn remember(&mut self, owner: &str, memory: Memory) -> Result<(), MemoryRefused> {
        if owner.is_empty() {
            return Err(MemoryRefused::NoName);
        }
        let length = owner.chars().count();
        if length > MAX_MEMORY_NAME {
            return Err(MemoryRefused::NameTooLong(length));
        }
        if !self.memories.contains_key(owner) && self.memories.len() >= MAX_MEMORIES {
            return Err(MemoryRefused::Full);
        }
        self.memories.insert(owner.to_owned(), memory);
        Ok(())
    }

    /// Returns one processing block's remembered recipes.
    #[must_use]
    pub fn memory(&self, owner: &str) -> Option<&Memory> {
        self.memories.get(owner)
    }

    /// Returns how many memories are held.
    #[must_use]
    pub fn memories(&self) -> usize {
        self.memories.len()
    }

    /// Returns every failure the last load recovered from.
    ///
    /// A world with a malformed entry opens, because refusing would cost a
    /// player their world over one block, and the failure is something a caller
    /// has to look at rather than something only a log holds.
    #[must_use]
    pub fn recovered(&self) -> &[ReadFailure] {
        &self.recovered
    }

    /// Returns how many of a world's memories this build could not hold, and
    /// absent when it held every one.
    ///
    /// A world holding more than [`MAX_MEMORIES`] opens, because refusing would
    /// cost a player their world, but it is not saved: writing what was held
    /// back over what was not would delete the rest permanently. Every dropped
    /// entry is named in [`Entry::recovered`].
    #[must_use]
    pub fn dropped(&self) -> Option<usize> {
        let dropped = self
            .recovered
            .iter()
            .filter(|failure| matches!(failure, ReadFailure::Dropped { .. }))
            .count();
        (dropped > 0).then_some(dropped)
    }

    /// Returns what only a client holds.
    ///
    /// # Errors
    ///
    /// Returns [`WrongSide`] on a server side, where the value was never built.
    pub fn client(&self) -> Result<&ClientState, WrongSide> {
        self.client.get(self.side)
    }

    /// Returns what only a client holds, for changing.
    ///
    /// # Errors
    ///
    /// Returns [`WrongSide`] on a server side.
    pub fn client_mut(&mut self) -> Result<&mut ClientState, WrongSide> {
        self.client.get_mut(self.side)
    }

    /// Returns whether the client-only work exists on this side at all.
    #[must_use]
    pub fn has_client_state(&self) -> bool {
        self.client.is_present()
    }

    /// Moves the monotonic clock to a host's own reading and expires whatever
    /// its time has passed.
    ///
    /// Returns how many highlights went, and none at all on a server side.
    pub fn advance_to(&mut self, now: Moment) -> usize {
        let Ok(client) = self.client_mut() else {
            return 0;
        };
        client.clock = now;
        client.highlights.expire(now)
    }

    /// Forgets every highlight, which is what leaving a world does.
    ///
    /// **Declarable only in its trigger.** Ravel publishes no disconnect event,
    /// so nothing here calls it on its own. A server side has nothing to forget
    /// and this costs it nothing.
    pub fn on_disconnect(&mut self) {
        if let Ok(client) = self.client_mut() {
            client.highlights.on_disconnect();
        }
    }

    /// Returns how many of each event have been seen.
    #[must_use]
    pub const fn counts(&self) -> EventCounts {
        self.counts
    }

    /// Returns the lifecycle steps taken, in order.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }

    fn payload(&self) -> ravel_sdk::JsonValue {
        let mut document = ravel_sdk::json!({});
        for (owner, memory) in &self.memories {
            document[owner.as_str()] = memory.write(&self.spellings);
        }
        document
    }
}

#[declare_mod(id = "rvl_146_counterpart")]
impl Mod for Entry {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        self.entered.push(state);
        Ok(())
    }

    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
        claim_all(self.collection.run().declaration(), registrar)
    }

    fn content_declaration(&self) -> Result<Option<ContentDeclaration>, ModError> {
        Ok(Some(self.collection.run().declaration().clone()))
    }

    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        self.counts.record(event);
        if event == GameEvent::ClientTick
            && let Ok(client) = self.client_mut()
        {
            let now = client.clock.after(TICK_MILLIS);
            client.clock = now;
            client.highlights.expire(now);
        }
        Ok(())
    }

    fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, ModError> {
        Memory::declaration(Self::owner())
            .map(Some)
            .map_err(|error| ModError::refused(error.to_string()))
    }

    fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, ModError> {
        if let Some(dropped) = self.dropped() {
            return Err(ModError::refused(format!(
                "{dropped} saved memories were not held on the way in, \
                 so writing this world would delete them"
            )));
        }
        Ok(Some(SavedDataEnvelope::current(
            Self::owner(),
            Memory::version(),
            self.payload(),
        )))
    }

    fn open_saved_data(
        &mut self,
        envelope: Option<&SavedDataEnvelope>,
        plan: &SavedDataPlan,
    ) -> Result<(), ModError> {
        let _ = plan;
        self.memories.clear();
        self.recovered.clear();
        let Some(envelope) = envelope else {
            return Ok(());
        };
        let Some(entries) = envelope.payload().as_object() else {
            return Ok(());
        };
        for (owner, document) in entries {
            if !self.memories.contains_key(owner) && self.memories.len() >= MAX_MEMORIES {
                self.recovered.push(ReadFailure::Dropped {
                    owner: owner.clone(),
                });
                continue;
            }
            match Memory::read(document, &self.spellings, &|_| true) {
                Err(failure) => self.recovered.push(failure),
                Ok((current, remembered)) => {
                    let memory = Memory {
                        current: current.recipe().cloned(),
                        remembered: remembered.recipe().cloned(),
                    };
                    self.memories.insert(owner.clone(), memory);
                }
            }
        }
        Ok(())
    }
}

export_mod!(|side| Entry::new(side));

#[cfg(test)]
mod tests {
    use ravel_sdk::content::{Registrar, RegistrationRefused, RegistryKey};
    use ravel_sdk::events::GameEvent;
    use ravel_sdk::prelude::*;
    use ravel_sdk::save::SavedDataPlan;

    use super::{
        __ravel_entry, AlreadyConstructed, DECLINED, Entry, EventCounts, HOST_BLOCKERS, ItemForm,
        MAX_MEMORIES, MAX_MEMORY_NAME, MOD_ID, Memory, MemoryRefused, Moment, ReadFailure,
        TICK_MILLIS,
    };
    use crate::colour::Colour;
    use crate::highlights::{Bounds, Key, Place};
    use crate::persistence::Spellings;
    use crate::recipes::Search;

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

        fn loads(&self, mod_id: &ModId) -> bool {
            let _ = mod_id;
            false
        }
    }

    fn recipe(path: &str) -> ContentId {
        ContentId::new(Entry::owner(), path).expect("a legal content path")
    }

    fn key(x: i32) -> Key {
        Key {
            place: Place {
                level: "overworld".to_owned(),
                position: (x, 64, 0),
            },
            face: None,
        }
    }

    #[test]
    fn the_library_is_constructed_once_and_a_second_construction_is_refused() {
        // The only test that claims the process-wide slot, which is what makes
        // it deterministic beside every other test here. Nothing else claims it,
        // the exported entry included: `sole` is for a caller that can hear the
        // refusal, and the entry has nowhere to put one.
        assert!(Entry::sole(Side::Client).is_ok());
        assert_eq!(Entry::sole(Side::Client).err(), Some(AlreadyConstructed));
        assert_eq!(
            AlreadyConstructed.to_string(),
            "the shared library is already constructed in this process"
        );
    }

    #[test]
    fn the_declared_identity_is_the_one_the_package_is_validated_under() {
        let shared = Entry::new(Side::Client);
        assert_eq!(Entry::declared_id().as_str(), MOD_ID);
        assert_eq!(shared.id().as_str(), MOD_ID);
        assert_eq!(shared.side(), Side::Client);
    }

    #[test]
    fn what_a_dependent_declared_is_what_the_barrier_claims() {
        let mut shared = Entry::new(Side::DedicatedServer);
        shared
            .collection_mut()
            .block("crusher", ItemForm::Default, &[])
            .expect("a legal path");
        let mut registrar = Accepting::default();
        shared
            .register(&mut registrar)
            .expect("an accepting registrar");
        assert_eq!(
            registrar
                .claimed()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            [
                "block:rvl_146_counterpart:crusher".to_owned(),
                "item:rvl_146_counterpart:crusher".to_owned()
            ]
        );
    }

    #[test]
    fn client_only_state_is_not_built_on_a_server_and_costs_it_nothing() {
        let mut server = Entry::new(Side::DedicatedServer);
        assert!(!server.has_client_state());
        assert!(server.client().is_err());
        assert_eq!(server.advance_to(Moment(10_000)), 0);
        server.on_disconnect();

        let client = Entry::new(Side::Client);
        assert!(client.has_client_state());
        assert!(
            client
                .client()
                .expect("a client holds it")
                .highlights
                .is_empty()
        );
    }

    #[test]
    fn a_client_tick_advances_the_clock_and_expires_what_its_time_has_passed() {
        let mut shared = Entry::new(Side::Client);
        shared
            .client_mut()
            .expect("a client")
            .highlights
            .add(
                key(0),
                Moment(0),
                TICK_MILLIS,
                Bounds::BLOCK,
                Colour::WHITE,
                None,
            )
            .expect("a fresh set");
        shared
            .observe(GameEvent::ClientTick)
            .expect("a client tick is observed");
        let client = shared.client().expect("a client");
        assert_eq!(client.clock, Moment(TICK_MILLIS));
        assert!(
            client.highlights.is_empty(),
            "expiry happens on a tick rather than on a frame"
        );
    }

    #[test]
    fn every_event_is_counted_rather_than_collected() {
        let mut shared = Entry::new(Side::Client);
        for _ in 0..3 {
            shared.observe(GameEvent::ServerTick).expect("observed");
        }
        shared
            .observe(GameEvent::ResourcesReloaded)
            .expect("observed");
        assert_eq!(
            shared.counts(),
            EventCounts {
                server_ticks: 3,
                client_ticks: 0,
                reloads: 1,
            }
        );
    }

    #[test]
    fn a_count_saturates_rather_than_wrapping() {
        let mut counts = EventCounts {
            server_ticks: u64::MAX,
            ..EventCounts::default()
        };
        counts.record(GameEvent::ServerTick);
        assert_eq!(counts.server_ticks, u64::MAX);
    }

    #[test]
    fn leaving_a_world_forgets_the_highlights_a_client_held() {
        let mut shared = Entry::new(Side::Client);
        shared
            .client_mut()
            .expect("a client")
            .highlights
            .add(
                key(0),
                Moment(0),
                10_000,
                Bounds::BLOCK,
                Colour::WHITE,
                None,
            )
            .expect("a fresh set");
        shared.on_disconnect();
        assert!(shared.client().expect("a client").highlights.is_empty());
    }

    #[test]
    fn the_whole_lifecycle_runs_through_the_entry_alone() {
        let mut shared = Entry::new(Side::IntegratedServer);
        for state in LifecycleState::ALL {
            shared.enter(state).expect("every step is accepted");
        }
        assert_eq!(shared.entered(), LifecycleState::ALL);
    }

    #[test]
    fn a_memory_written_into_a_world_is_the_memory_read_back_out_of_it() {
        let mut shared = Entry::new(Side::IntegratedServer);
        shared
            .remember(
                "the crusher at 0 64 0",
                Memory {
                    current: Some(recipe("smelting")),
                    remembered: Some(recipe("blasting")),
                },
            )
            .expect("a named memory");
        let envelope = shared
            .save_saved_data()
            .expect("saving produces an envelope")
            .expect("this library owns world data");

        let mut opened = Entry::new(Side::IntegratedServer);
        let declaration = opened
            .saved_data_declaration()
            .expect("valid saved data")
            .expect("this library declares its data");
        let plan = SavedDataPlan::open(&declaration, Some(Memory::version()))
            .expect("a world at the current version");
        opened
            .open_saved_data(Some(&envelope), &plan)
            .expect("a world this package wrote");
        assert_eq!(
            opened.memory("the crusher at 0 64 0"),
            Some(&Memory {
                current: Some(recipe("smelting")),
                remembered: Some(recipe("blasting")),
            }),
            "the saved recipe memory survives a save and a load"
        );
        assert_eq!(opened.recovered().len(), 0);
    }

    #[test]
    fn a_world_with_a_malformed_entry_opens_and_the_failure_is_something_to_look_at() {
        let spellings = Spellings::default();
        let mut inner = ravel_sdk::json!({});
        inner[spellings.current()] = ravel_sdk::json!(7);
        let mut memory = ravel_sdk::json!({});
        memory[spellings.document()] = inner;
        let mut payload = ravel_sdk::json!({});
        payload["the crusher"] = memory;
        let envelope =
            ravel_sdk::save::SavedDataEnvelope::current(Entry::owner(), Memory::version(), payload);

        let mut shared = Entry::new(Side::IntegratedServer);
        let declaration = shared
            .saved_data_declaration()
            .expect("valid saved data")
            .expect("declared");
        let plan = SavedDataPlan::open(&declaration, Some(Memory::version())).expect("a plan");
        shared
            .open_saved_data(Some(&envelope), &plan)
            .expect("a malformed entry does not cost a world");
        assert_eq!(shared.memories(), 0);
        assert_eq!(shared.recovered().len(), 1);
    }

    #[test]
    fn a_fresh_world_holds_no_memory_and_is_not_a_failure() {
        let mut shared = Entry::new(Side::IntegratedServer);
        let declaration = shared
            .saved_data_declaration()
            .expect("valid saved data")
            .expect("declared");
        let plan = SavedDataPlan::open(&declaration, None).expect("a fresh world");
        shared
            .open_saved_data(None, &plan)
            .expect("a fresh world opens");
        assert_eq!(shared.memories(), 0);
    }

    #[test]
    fn the_memories_a_world_holds_are_named_and_bounded() {
        let mut shared = Entry::new(Side::IntegratedServer);
        assert_eq!(
            shared.remember("", Memory::default()),
            Err(MemoryRefused::NoName)
        );
        let long = "x".repeat(MAX_MEMORY_NAME + 1);
        assert_eq!(
            shared.remember(&long, Memory::default()),
            Err(MemoryRefused::NameTooLong(MAX_MEMORY_NAME + 1))
        );
        for index in 0..MAX_MEMORIES {
            shared
                .remember(&format!("block{index}"), Memory::default())
                .expect("inside the ceiling");
        }
        assert_eq!(
            shared.remember("one more", Memory::default()),
            Err(MemoryRefused::Full)
        );
        assert!(
            shared.remember("block0", Memory::default()).is_ok(),
            "replacing a held memory is not a new one"
        );
        assert_eq!(shared.memories(), MAX_MEMORIES);
    }

    #[test]
    fn the_search_a_dependent_drives_is_the_one_this_library_saves() {
        let mut search = Search::idle();
        search.restore(Some(recipe("smelting")), Some(recipe("blasting")));
        let memory = Memory {
            current: search.current().cloned(),
            remembered: search.remembered().cloned(),
        };
        let mut shared = Entry::new(Side::IntegratedServer);
        shared
            .remember("the crusher", memory)
            .expect("a named memory");
        assert_eq!(
            shared
                .memory("the crusher")
                .and_then(|held| held.current.as_ref()),
            Some(&recipe("smelting"))
        );
    }

    #[test]
    fn a_running_server_is_recorded_where_a_radius_send_reads_it() {
        let mut shared = Entry::new(Side::DedicatedServer);
        assert_eq!(shared.sends().server(), None);
        shared.sends_mut().server_started("the running server");
        assert_eq!(shared.sends().server(), Some("the running server"));
    }

    #[test]
    fn no_area_is_both_declined_and_host_blocked() {
        // The two lists are what a reader is pointed at, and an area belongs to
        // exactly one of them: a declined area is one no Ravel revision should
        // carry, and a host-blocked one is waiting on a seam. Naming an area in
        // both would be claiming both at once.
        for declined in DECLINED {
            assert_ne!(declined.len(), 0);
            assert!(
                !HOST_BLOCKERS.contains(&declined),
                "`{declined}` is declined and cannot also be waiting on a seam"
            );
        }
        for blocked in HOST_BLOCKERS {
            assert_ne!(blocked.len(), 0);
            assert_eq!(
                HOST_BLOCKERS
                    .iter()
                    .filter(|held| **held == blocked)
                    .count(),
                1,
                "`{blocked}` is named once"
            );
        }
    }

    #[test]
    fn the_entry_a_host_calls_builds_a_mod_every_time_it_is_called() {
        // `ravel-build` writes a members function over this entry and says a
        // test or a diagnostic may drive it with no JVM, so a second call in one
        // process is a supported path. An entry that claimed a process-wide slot
        // would fault there, and on the JNI path that unwinds out of the host.
        let first = __ravel_entry(Side::Client);
        let second = __ravel_entry(Side::DedicatedServer);
        assert_eq!(first.id().as_str(), MOD_ID);
        assert_eq!(second.id().as_str(), MOD_ID);
    }

    #[test]
    fn a_world_holding_more_memories_than_this_build_can_is_not_written_back() {
        let spellings = Spellings::default();
        let mut payload = ravel_sdk::json!({});
        let held = Memory {
            current: Some(recipe("smelting")),
            remembered: None,
        };
        for index in 0..MAX_MEMORIES + 10 {
            payload[format!("the crusher {index:05}")] = held.write(&spellings);
        }
        let envelope =
            ravel_sdk::save::SavedDataEnvelope::current(Entry::owner(), Memory::version(), payload);

        let mut opened = Entry::new(Side::IntegratedServer);
        let declaration = opened
            .saved_data_declaration()
            .expect("valid saved data")
            .expect("this library declares its data");
        let plan = SavedDataPlan::open(&declaration, Some(Memory::version()))
            .expect("a world at the current version");
        opened
            .open_saved_data(Some(&envelope), &plan)
            .expect("a world too large for this build still opens");

        assert_eq!(opened.memories(), MAX_MEMORIES);
        assert_eq!(
            opened.dropped(),
            Some(10),
            "every memory that was not held is counted"
        );
        assert_eq!(
            opened
                .recovered()
                .iter()
                .filter(|failure| matches!(failure, ReadFailure::Dropped { .. }))
                .count(),
            10,
            "and each one is named, not merely counted"
        );
        assert!(
            opened.save_saved_data().is_err(),
            "saving would write the held memories over the dropped ones"
        );

        // A world this build did hold every memory of saves as it always did.
        let mut ordinary = Entry::new(Side::IntegratedServer);
        ordinary
            .open_saved_data(None, &plan)
            .expect("a fresh world");
        assert_eq!(ordinary.dropped(), None);
        assert!(ordinary.save_saved_data().is_ok());
    }
}
