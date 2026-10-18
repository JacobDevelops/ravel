#![forbid(unsafe_code)]

//! An original Ravel implementation of the behaviour specified in RVL-025.
//!
//! The entry is two things at once. It is a playable content pack — seventeen
//! items, five blocks, one machine and one ore that generates in three
//! environments — and it is the published integration surface a family of larger
//! mods builds against. The two halves are unequal in size and inverted in
//! importance, and §16 is clear which one the dependents actually need: a
//! counterpart that ships the content and not the surface satisfies no
//! dependent.
//!
//! What a dependent reaches for:
//!
//! - [`capabilities`] — the eight published groups, their obligations and their
//!   registration window;
//! - [`ordering`] — the cross-block ordering service and its five ranks;
//! - [`machines`] — the upgrading machine, its cycle and its energy discount;
//! - [`world`] — the ore, its drops and the four placements;
//! - [`shapes`] — the pattern card, its grid and its evaluation;
//! - [`matching`] — the stack-match decision and its store;
//! - [`storage`] — the handheld carrier and its item swap;
//! - [`tools`] — the two-mode configuration tool;
//! - [`values`] — the readout block and the split between the two sides;
//! - [`network`] — the six payloads and what each one is put through;
//! - [`commands`] — the two commands, the two key bindings and the one reply
//!   encoding;
//! - [`data`] — the five persistent records everything above is written in;
//! - [`census`] — what is registered, what ships beside it, and what is not;
//! - [`settings`] — the four values a pack may change.
//!
//! `NOTES.md` beside this file says which sections are carried here and which
//! are declarable only because Ravel publishes no seam for them.

pub mod capabilities;
pub mod census;
pub mod commands;
pub mod data;
pub mod dumps;
pub mod machines;
pub mod matching;
pub mod network;
pub mod ordering;
mod services;
pub mod settings;
pub mod shapes;
pub mod storage;
pub mod tools;
pub mod values;
pub mod world;

use ravel_sdk::JsonValue;
use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration};
use ravel_sdk::content::{ContentDeclaration, ContentId};
use ravel_sdk::events::GameEvent;
use ravel_sdk::network::{Packet, PacketDelivery};
use ravel_sdk::prelude::*;
use ravel_sdk::save::{SavedDataDeclaration, SavedDataEnvelope, SavedDataVersion};
use ravel_sdk::services::{ServiceDeclaration, ServiceId, ServiceRequest, ServiceResponse};
use ravel_sdk::sides::ClientOnly;

pub use crate::capabilities::{Group, Surface, SurfaceRefused};
pub use crate::data::{Placement, Record, RecordRefused, Stack, Store};
pub use crate::machines::{Machine, Ticked};
pub use crate::matching::{MatchRule, Modes, VerdictMode};
pub use crate::ordering::{Ordering, OrderingEntry, OrderingRefused};
pub use crate::settings::Settings;
pub use crate::shapes::{Pattern, ShapedRecipe};
pub use crate::storage::{Carrier, Hand, Step};
pub use crate::tools::{Mode, Tool};
pub use crate::values::{Readings, Readout, Screen};
pub use crate::world::{Modifier, OreBlock, Variant};

/// The identity this package is declared and validated under.
pub const MOD_ID: &str = "rvl_025_counterpart";

/// The dimension a caller names when it names none of its own.
///
/// Not the one dimension the tick drains: §14's queue is per dimension and a
/// server tick drains every dimension that holds anything, because Ravel's
/// server-tick signal carries no dimension and draining one would leave every
/// other queue to grow unseen.
pub const DEFAULT_DIMENSION: &str = "overworld";

/// The one saved-data version this build writes and reads.
pub const DATA_VERSION: SavedDataVersion = SavedDataVersion::MIN;

/// The service a dependent registers into the published surface of §15 through.
pub const SURFACE_SERVICE: &str = "interface_surface";

/// The service a dependent enqueues into the ordering of §14 through.
pub const ORDERING_SERVICE: &str = "block_ordering";

/// The state a client holds and a server has no reason to.
///
/// The readout's last answer is the whole of it: only a client receives one, and
/// only a client renders it. Building it through [`ClientOnly`] is what makes
/// that not merely hidden on a server side but never built there.
#[derive(Debug, Default)]
pub struct ClientState {
    /// What the readout was last told.
    pub screen: Screen,
}

/// What the server believes the acting player is holding.
///
/// §13's five server-bound payloads and §14's two commands all act on the held
/// item, so the held item is state the entry has to have before either can do
/// anything. The host sets it; nothing here reads a player.
#[derive(Debug, Default)]
pub struct Holding {
    /// Which hand is active, or none.
    pub hand: Option<Hand>,
    /// Which of this entry's items is held.
    pub kind: network::Held,
    /// Where the player is, for the readout's distance bound.
    pub at: (i32, i32, i32),
    /// Whether the chunk a readout request names is loaded.
    pub chunk_loaded: bool,
}

/// The entry, as the loader drives it.
#[derive(Debug)]
pub struct Entry {
    side: Side,
    declaration: Result<ContentDeclaration, ModError>,
    settings: Settings,
    surface: Surface,
    ordering: Ordering,
    store: Store,
    card: Pattern,
    carrier: Carrier,
    rule: MatchRule,
    recipes: Vec<ShapedRecipe>,
    holding: Holding,
    answered: Option<network::ReadoutAnswer>,
    evaluated: Option<commands::Evaluated>,
    cycled: Option<commands::Cycled>,
    client: ClientOnly<ClientState>,
    entered: Vec<LifecycleState>,
    observed: Vec<GameEvent>,
    deferred: bool,
}

impl Entry {
    /// Builds the entry for one side, holding its shipped defaults.
    ///
    /// The client-only state is built only on a client, which is why the side is
    /// taken here rather than tested later.
    #[must_use]
    pub fn new(side: Side) -> Self {
        Self {
            side,
            declaration: Self::content(),
            settings: Settings::default(),
            surface: Surface::new(),
            ordering: Ordering::new(),
            store: Store::new(),
            card: Pattern::new(),
            carrier: Carrier::new(),
            rule: MatchRule::new(),
            recipes: Vec::new(),
            holding: Holding::default(),
            answered: None,
            evaluated: None,
            cycled: None,
            client: ClientOnly::built_on(side, ClientState::default),
            entered: Vec::new(),
            observed: Vec::new(),
            deferred: false,
        }
    }

    /// Returns the values in force.
    #[must_use]
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Returns the world data this entry owns.
    #[must_use]
    pub const fn store(&self) -> &Store {
        &self.store
    }

    /// Returns that data for writing.
    pub const fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }

    /// Returns the pattern card the acting player is holding.
    #[must_use]
    pub const fn card(&self) -> &Pattern {
        &self.card
    }

    /// Returns the carrier the acting player is holding.
    #[must_use]
    pub const fn carrier(&self) -> &Carrier {
        &self.carrier
    }

    /// Returns that carrier for writing, which is how a host fills one.
    pub const fn carrier_mut(&mut self) -> &mut Carrier {
        &mut self.carrier
    }

    /// Returns the match rule the acting player is holding.
    #[must_use]
    pub const fn rule(&self) -> &MatchRule {
        &self.rule
    }

    /// Returns what the server believes the acting player is holding.
    #[must_use]
    pub const fn holding(&self) -> &Holding {
        &self.holding
    }

    /// Returns that for writing, which is how a host sets it.
    pub const fn holding_mut(&mut self) -> &mut Holding {
        &mut self.holding
    }

    /// Returns the recipes the evaluate command scans.
    pub fn set_recipes(&mut self, recipes: Vec<ShapedRecipe>) {
        self.recipes = recipes;
    }

    /// Returns the last readout answer this entry produced, on the server.
    #[must_use]
    pub const fn answered(&self) -> Option<&network::ReadoutAnswer> {
        self.answered.as_ref()
    }

    /// Returns what the evaluate command last did.
    #[must_use]
    pub const fn evaluated(&self) -> Option<commands::Evaluated> {
        self.evaluated
    }

    /// Returns what the cycle command last did.
    #[must_use]
    pub const fn cycled(&self) -> Option<commands::Cycled> {
        self.cycled
    }

    /// Names the service a dependent registers into the surface of §15 through.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when the declared name is not a legal
    /// content path, which for a constant is a packaging failure.
    pub fn surface_service() -> Result<ServiceId, ModError> {
        Ok(ServiceId::new(
            ContentId::new(Self::declared_id().clone(), SURFACE_SERVICE)
                .map_err(ModError::from_display)?,
        ))
    }

    /// Names the service a dependent enqueues into the ordering of §14 through.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when the declared name is not a legal
    /// content path.
    pub fn ordering_service() -> Result<ServiceId, ModError> {
        Ok(ServiceId::new(
            ContentId::new(Self::declared_id().clone(), ORDERING_SERVICE)
                .map_err(ModError::from_display)?,
        ))
    }

    /// Returns the published interface surface dependents register into.
    #[must_use]
    pub const fn surface(&self) -> &Surface {
        &self.surface
    }

    /// Returns that surface for registration.
    pub const fn surface_mut(&mut self) -> &mut Surface {
        &mut self.surface
    }

    /// Returns the cross-block ordering service.
    #[must_use]
    pub const fn ordering(&self) -> &Ordering {
        &self.ordering
    }

    /// Returns that service for enqueuing and draining.
    pub const fn ordering_mut(&mut self) -> &mut Ordering {
        &mut self.ordering
    }

    /// Returns the client-only state, on a client.
    ///
    /// # Errors
    ///
    /// Returns [`ravel_sdk::sides::ClientOnlyError`] when this is a server or state was never built.
    pub fn client(&self) -> Result<&ClientState, ravel_sdk::sides::ClientOnlyError> {
        self.client.get(self.side)
    }

    /// Returns the client-only state for writing, on a client.
    ///
    /// # Errors
    ///
    /// Returns [`ravel_sdk::sides::ClientOnlyError`] when this is a server or state was never built.
    pub fn client_mut(&mut self) -> Result<&mut ClientState, ravel_sdk::sides::ClientOnlyError> {
        self.client.get_mut(self.side)
    }

    /// Returns the lifecycle steps this entry has taken.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }

    /// Returns the signals this entry has observed.
    #[must_use]
    pub fn observed(&self) -> &[GameEvent] {
        &self.observed
    }

    /// Returns whether the deferred work of §2 has run.
    #[must_use]
    pub const fn deferred_work_done(&self) -> bool {
        self.deferred
    }

    /// Runs the work §2 defers to after registration and after configuration.
    ///
    /// Two things happen here and nowhere earlier: the two commands and the one
    /// reply encoding are filed with the shared transport, and the published
    /// interface surface stops accepting registrations. Both need every registry
    /// populated, and running either earlier gets a partially-populated view.
    pub fn run_deferred(&mut self) -> &'static [commands::Registration] {
        self.surface.close();
        self.deferred = true;
        &commands::REGISTRATIONS
    }

    /// Builds the content declaration this package claims at the barrier.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when a name this package spells is not a
    /// legal content path, which for constants is a packaging failure.
    pub fn content() -> Result<ContentDeclaration, ModError> {
        census::declaration(Self::declared_id().clone()).map_err(ModError::from_display)
    }

    fn route_packet(&mut self, delivery: &PacketDelivery) -> Result<(), ModError> {
        let path = delivery.id.id().path().to_owned();
        let held = self.holding.kind;
        if path == network::HeldComponents::contract().id().id().path() {
            let payload: network::HeldComponents =
                delivery.decode().map_err(ModError::from_display)?;
            if network::apply_components(held, &payload, Record::PatternContents)
                == network::Applied::Accepted
                && let Some(written) = payload
                    .offered
                    .components
                    .get(Record::PatternContents.as_str())
                && let Ok(document) = written.parse::<JsonValue>()
                && let Ok(card) = Pattern::from_saved(&document)
            {
                self.card = card;
            }
            return Ok(());
        }
        if path == network::CardArguments::contract().id().id().path() {
            let payload: network::CardArguments =
                delivery.decode().map_err(ModError::from_display)?;
            // This packet never updates card state, whether accepted or refused.
            match network::apply_card_arguments(held, &payload) {
                network::Applied::Accepted
                | network::Applied::Discarded
                | network::Applied::Refused(_) => {}
            }
            return Ok(());
        }
        if path == network::BrowserPattern::contract().id().id().path() {
            let payload: network::BrowserPattern =
                delivery.decode().map_err(ModError::from_display)?;
            if network::apply_browser_pattern(held, self.card_menu_open(), &payload)
                == network::Applied::Accepted
                && let Ok(pushed) = self.surface.push_browser_grid(&payload.slots)
            {
                self.card = pushed.pattern;
            }
            return Ok(());
        }
        if path == network::ReadoutRequest::contract().id().id().path() {
            let payload: network::ReadoutRequest =
                delivery.decode().map_err(ModError::from_display)?;
            if network::answer_readout(&payload, self.holding.at, self.holding.chunk_loaded)
                == network::Applied::Accepted
            {
                let readout = Readout::new();
                let readings = readout.collect(None);
                self.answered = Some(
                    network::readout_answer(payload.at, &readings)
                        .map_err(ModError::from_display)?,
                );
            } else {
                self.answered = None;
            }
            return Ok(());
        }
        if path == network::HeldItem::contract().id().id().path() {
            self.route_held_item(delivery, held)?;
            return Ok(());
        }
        if path == network::ReadoutAnswer::contract().id().id().path() {
            let payload: network::ReadoutAnswer =
                delivery.decode().map_err(ModError::from_display)?;
            return self.accept_readout(&payload);
        }
        Err(ModError::refused(
            "this entry declares no contract under that name",
        ))
    }

    fn accept_readout(&mut self, payload: &network::ReadoutAnswer) -> Result<(), ModError> {
        let mut readings = Readings::new();
        for (name, reading) in payload.values() {
            readings.declare(name, reading.clone());
        }
        let stamp = self.ordering.stamp();
        self.client_mut()
            .map_err(ModError::from_display)?
            .screen
            .accept(readings, stamp);
        Ok(())
    }

    fn route_held_item(
        &mut self,
        delivery: &PacketDelivery,
        held: network::Held,
    ) -> Result<(), ModError> {
        let payload: network::HeldItem = delivery.decode().map_err(ModError::from_display)?;
        if network::apply_held_item(held, held, &payload) != network::Applied::Accepted {
            return Ok(());
        }
        let record = match held {
            network::Held::Carrier => Record::CarrierContents,
            network::Held::PatternCard | network::Held::MatchItem | network::Held::Other => {
                Record::MatchRule
            }
        };
        if let Some(written) = payload.sent.components.get(record.as_str())
            && let Ok(document) = written.parse::<JsonValue>()
        {
            match record {
                Record::CarrierContents => {
                    if let Ok(carrier) = Carrier::from_saved(&document) {
                        self.carrier = carrier;
                    }
                }
                Record::PatternContents
                | Record::MatchRule
                | Record::InfusionProgress
                | Record::SelectedPosition => {
                    if let Ok(rule) = MatchRule::from_saved(&document) {
                        self.rule = rule;
                    }
                }
            }
        }
        Ok(())
    }

    const fn card_menu_open(&self) -> bool {
        matches!(self.holding.kind, network::Held::PatternCard)
    }
}

#[declare_mod(id = "rvl_025_counterpart")]
impl Mod for Entry {
    /// Takes one lifecycle step, and empties the ordering queues on the one
    /// that stands in for a server starting.
    ///
    /// §14 says the whole structure is cleared when a server starts. Ravel's
    /// event set is closed and has no server-started member, so the nearest
    /// published point is the step at which runtime hooks become active, and
    /// that is where the clear happens. On a client the queues are empty and the
    /// clear is a no-op, so the substitution costs nothing there.
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        self.entered.push(state);
        if state == LifecycleState::Running {
            self.ordering.clear();
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
        self.settings = Settings::read(resolved).map_err(ModError::from_display)?;
        Ok(())
    }

    /// Observes the post-tick signal and drains every dimension that ticked.
    ///
    /// The drain happens on a server tick alone. Ravel delivers a client tick
    /// only to a client and a server tick only to a server, so the observer is
    /// declared by which event it answers rather than by a runtime test on the
    /// level, which is what §2 asks a counterpart to do with an observer the
    /// original guards by hand.
    ///
    /// §14 puts the drain after a server-side *level* tick and names the
    /// dimension that ticked. Ravel's signal names no dimension, so every
    /// dimension holding anything is drained. Draining one named dimension
    /// instead would leave a dependent's block entity in any other dimension
    /// queued forever, and the loss would be silent, which is the one thing §14
    /// says a counterpart must not do to this service.
    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        self.observed.push(event);
        if event == GameEvent::ServerTick {
            self.ordering.drain_all();
        }
        Ok(())
    }

    /// Runs one of the two commands of §14 against what the player holds.
    fn run_command(&mut self, invocation: &CommandInvocation) -> Result<(), ModError> {
        let name = invocation.key.id().path();
        let filed = commands::resolve(name)
            .ok_or_else(|| ModError::refused("this entry files no command under that name"))?;
        if filed.on_server && !invocation.side.is_server() {
            return Err(ModError::refused("this command runs only on a server"));
        }
        match filed.name {
            commands::EVALUATE => {
                let held = self.card_menu_open().then_some(&mut self.card);
                self.evaluated = Some(commands::evaluate(held, &self.recipes));
            }
            commands::CYCLE => {
                let step =
                    commands::direction(&invocation.arguments).map_err(ModError::from_display)?;
                let holding = match self.holding.kind {
                    network::Held::Carrier => Some(commands::Cyclable::Carrier(&mut self.carrier)),
                    network::Held::PatternCard
                    | network::Held::MatchItem
                    | network::Held::Other => None,
                };
                self.cycled = Some(commands::cycle(self.holding.hand, holding, step));
            }
            // The registration list also carries the reply encoding, which is
            // not a command and does not resolve above.
            _ => {}
        }
        Ok(())
    }

    /// Reads one of the six payloads of §13 and puts it through its validation.
    ///
    /// The contract the delivery names decides which validation runs, and the
    /// validation decides whether anything changes. A delivery naming a contract
    /// this entry does not declare is refused rather than ignored, because the
    /// loader routes by namespace and one that reached here is addressed to a
    /// name this entry was supposed to own.
    fn receive_packet(&mut self, delivery: &PacketDelivery) -> Result<(), ModError> {
        self.route_packet(delivery)
    }

    fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
        let surface = Self::surface_service()?;
        let ordering = Self::ordering_service()?;
        ServiceDeclaration::new(Self::declared_id().clone(), [surface, ordering], [])
            .map(Some)
            .map_err(ModError::from_display)
    }

    /// Answers §16's first two entries, which are the two a dependent needs.
    ///
    /// §16 says the published surface and the ordering service are what the
    /// three admitted dependents consume, and Ravel's service seam is the one
    /// thing that crosses from one mod to another while both run. Without this
    /// both are reachable only by whoever owns the entry value, which is the
    /// loader, and no dependent is that.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] for a call to a service this entry does not
    /// answer.
    fn answer_service(&mut self, request: &ServiceRequest) -> Result<ServiceResponse, ModError> {
        let surface = Self::surface_service()?;
        let ordering = Self::ordering_service()?;
        if request.id == surface {
            return Ok(ServiceResponse::new(services::answer_surface(
                &mut self.surface,
                &request.arguments,
            )));
        }
        if request.id == ordering {
            return Ok(ServiceResponse::new(services::answer_ordering(
                &mut self.ordering,
                &request.arguments,
            )));
        }
        Err(ModError::refused("this entry answers two services"))
    }

    fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, ModError> {
        SavedDataDeclaration::new(Self::declared_id().clone(), [DATA_VERSION])
            .map(Some)
            .map_err(ModError::from_display)
    }

    /// Writes the five records of §4 out to the world.
    fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, ModError> {
        Ok(Some(SavedDataEnvelope::current(
            Self::declared_id().clone(),
            DATA_VERSION,
            self.store.to_saved(),
        )))
    }

    /// Reads them back, or opens on nothing where the world stored nothing.
    ///
    /// §4 scopes the records to what carries them and every one of them lives in
    /// a world, so a world that stored nothing opens on an empty store rather
    /// than on the last world's records.
    fn open_saved_data(&mut self, envelope: Option<&SavedDataEnvelope>) -> Result<(), ModError> {
        self.store = Store::new();
        let Some(envelope) = envelope else {
            return Ok(());
        };
        self.store = Store::from_saved(envelope.payload()).map_err(ModError::from_display)?;
        Ok(())
    }
}

export_mod!(|side| Entry::new(side));

#[cfg(test)]
mod tests {
    use super::*;
    use ravel_sdk::content::{ContentKind, RegistrationRefused, RegistryKey};
    use ravel_sdk::events::CommandInvocation;
    use ravel_sdk::json;

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
        assert_eq!(Entry::new(Side::Client).id().as_str(), MOD_ID);
    }

    #[test]
    fn the_entry_declares_both_its_content_and_its_settings() {
        let entry = Entry::new(Side::Client);
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
    fn the_deferred_work_closes_the_surface_and_files_the_three_registrations() {
        let mut entry = Entry::new(Side::DedicatedServer);
        assert!(entry.surface().is_open());
        assert!(!entry.deferred_work_done());

        let filed = entry.run_deferred();
        assert_eq!(filed.len(), 3);
        assert!(!entry.surface().is_open());
        assert!(entry.deferred_work_done());
    }

    #[test]
    fn a_dependent_registers_into_the_surface_before_the_deferred_work_and_not_after() {
        let mut entry = Entry::new(Side::DedicatedServer);
        entry
            .surface_mut()
            .register_channel(capabilities::ChannelKind {
                identifier: "dependent:power".parse().expect("a legal identifier"),
                owner: "dependent".parse().expect("a legal owner"),
            })
            .expect("the window is open before the deferred work");

        entry.run_deferred();
        assert_eq!(
            entry
                .surface_mut()
                .register_channel(capabilities::ChannelKind {
                    identifier: "dependent:fluid".parse().expect("a legal identifier"),
                    owner: "dependent".parse().expect("a legal owner"),
                }),
            Err(SurfaceRefused::Closed)
        );
    }

    #[test]
    fn a_server_starting_empties_the_ordering_queues() {
        let mut entry = Entry::new(Side::DedicatedServer);
        entry
            .ordering_mut()
            .enqueue(DEFAULT_DIMENSION, 0, "dependent")
            .expect("a legal rank");
        assert_eq!(entry.ordering().queued(DEFAULT_DIMENSION), 1);

        entry
            .enter(LifecycleState::Running)
            .expect("the entry accepts the step");
        assert_eq!(entry.ordering().queued(DEFAULT_DIMENSION), 0);
    }

    #[test]
    fn no_earlier_lifecycle_step_empties_the_queues() {
        let mut entry = Entry::new(Side::DedicatedServer);
        entry
            .ordering_mut()
            .enqueue(DEFAULT_DIMENSION, 0, "dependent")
            .expect("a legal rank");

        for state in [
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
        ] {
            entry.enter(state).expect("the entry accepts every step");
            assert_eq!(entry.ordering().queued(DEFAULT_DIMENSION), 1);
        }
    }

    #[test]
    fn a_server_tick_drains_the_dimension_that_ticked() {
        let mut entry = Entry::new(Side::DedicatedServer);
        entry
            .ordering_mut()
            .enqueue(DEFAULT_DIMENSION, 2, "dependent")
            .expect("a legal rank");

        entry
            .observe(GameEvent::ServerTick)
            .expect("the entry records what it observes");
        assert_eq!(entry.ordering().queued(DEFAULT_DIMENSION), 0);
        assert_eq!(entry.ordering().stamp(), 1);
    }

    #[test]
    fn a_client_tick_drains_nothing() {
        let mut entry = Entry::new(Side::Client);
        entry
            .ordering_mut()
            .enqueue(DEFAULT_DIMENSION, 0, "dependent")
            .expect("a legal rank");

        entry
            .observe(GameEvent::ClientTick)
            .expect("the entry records what it observes");
        assert_eq!(
            entry.ordering().queued(DEFAULT_DIMENSION),
            1,
            "the drain is the server's, and this observer is not it"
        );
    }

    #[test]
    fn the_readouts_client_memory_exists_on_a_client_and_nowhere_else() {
        let mut client = Entry::new(Side::Client);
        client
            .client_mut()
            .expect("a client builds it")
            .screen
            .accept(Readings::new(), 7);
        assert_eq!(
            client.client().expect("a client holds it").screen.stamped(),
            Some(7)
        );

        for side in [Side::IntegratedServer, Side::DedicatedServer] {
            let mut server = Entry::new(side);
            assert_eq!(
                server.client().expect_err("no server state").side(),
                Some(side)
            );
            assert_eq!(
                server.client_mut().expect_err("no server state").side(),
                Some(side)
            );
        }
    }

    #[test]
    fn the_whole_lifecycle_runs_through_and_records_what_it_was_asked() {
        let mut entry = Entry::new(Side::IntegratedServer);
        for state in LifecycleState::ALL {
            entry.enter(state).expect("the entry accepts every step");
        }
        assert_eq!(entry.entered(), LifecycleState::ALL);

        let declaration = entry
            .configuration_declaration()
            .expect("valid configuration")
            .expect("the entry declares settings");
        let resolved = LayeredConfiguration::resolve(declaration, None, None)
            .expect("the shipped defaults resolve");
        entry.configure(&resolved).expect("they read back");
        assert_eq!(entry.settings(), &Settings::default());
    }

    #[test]
    fn configure_propagates_a_missing_key_without_changing_settings() {
        let mut entry = Entry::new(Side::DedicatedServer);
        let before = *entry.settings();
        let changed = settings::DOCUMENT.replace(settings::ENERGY_CAPACITY, "infusing.renamed");
        let declaration = ConfigurationDeclaration::parse(&changed).expect("a legal declaration");
        let resolved = LayeredConfiguration::resolve(declaration, None, None)
            .expect("the changed declaration resolves");

        assert!(entry.configure(&resolved).is_err());
        assert_eq!(entry.settings(), &before);
    }

    #[test]
    fn the_host_builds_this_package_under_its_declared_identity() {
        assert_eq!(__ravel_entry(Side::DedicatedServer).id().as_str(), MOD_ID);
    }

    fn delivery_of<P: Packet>(packet: &P, side: Side) -> PacketDelivery {
        PacketDelivery::of(packet, side).expect("the side may receive it")
    }

    fn card_holder() -> Entry {
        let mut entry = Entry::new(Side::DedicatedServer);
        entry.holding_mut().kind = network::Held::PatternCard;
        entry.holding_mut().hand = Some(Hand::Main);
        entry
    }

    #[test]
    fn a_dependent_reaches_the_published_surface_through_the_service_seam() {
        let mut entry = Entry::new(Side::DedicatedServer);
        let declaration = entry
            .service_declaration()
            .expect("valid service declaration")
            .expect("the entry declares what it answers");
        let surface = Entry::surface_service().expect("a legal service name");
        let ordering = Entry::ordering_service().expect("a legal service name");
        let provided: Vec<&ServiceId> = declaration.provides().collect();
        assert!(provided.contains(&&surface));
        assert!(provided.contains(&&ordering));

        let answer = entry
            .answer_service(&ServiceRequest::new(
                surface,
                json!({
                    "action": "register_channel",
                    "owner": "dependent",
                    "identifier": "dependent:power",
                }),
            ))
            .expect("the entry answers this service");
        assert_eq!(answer.value()["answered"], json!(true));
        assert_eq!(
            entry.surface().channels().len(),
            1,
            "a dependent with no hold on the entry value registered into the surface"
        );
    }

    #[test]
    fn a_dependent_enqueues_into_the_ordering_service_through_the_same_seam() {
        let mut entry = Entry::new(Side::DedicatedServer);
        let ordering = Entry::ordering_service().expect("a legal service name");

        let answer = entry
            .answer_service(&ServiceRequest::new(
                ordering,
                json!({
                    "action": "enqueue",
                    "dimension": "counterpart:the_nether",
                    "rank": 2,
                    "who": "dependent",
                }),
            ))
            .expect("the entry answers this service");
        assert_eq!(answer.value()["answered"], json!(true));
        assert_eq!(entry.ordering().queued("counterpart:the_nether"), 1);

        entry
            .observe(GameEvent::ServerTick)
            .expect("the entry records what it observes");
        assert_eq!(
            entry.ordering().queued("counterpart:the_nether"),
            0,
            "a dependent's block entity outside the overworld is run rather than stranded"
        );
    }

    #[test]
    fn a_dependent_supplies_a_module_an_instruction_and_a_dimension_through_the_seam() {
        let mut entry = Entry::new(Side::DedicatedServer);
        let surface = Entry::surface_service().expect("a legal service name");
        let mut call = |arguments: JsonValue| {
            entry
                .answer_service(&ServiceRequest::new(surface.clone(), arguments))
                .expect("the entry answers this service")
                .value()
                .clone()
        };

        // A module's data must already be known, so the factory comes first and
        // the module names what it registered.
        assert_eq!(
            call(json!({
                "action": "register_data_factory",
                "owner": "dependent",
                "kind": "dependent:throughput",
            }))["answered"],
            json!(true)
        );
        assert_eq!(
            call(json!({
                "action": "register_module",
                "owner": "dependent",
                "name": "throughput",
                "data": "dependent:throughput",
                "energy_per_tick": 4,
                "takes_clicks": true,
            }))["answered"],
            json!(true)
        );
        assert_eq!(
            call(json!({
                "action": "register_instruction",
                "name": "hold",
                "output": "amount",
                "parameters": [{ "kind": "amount", "required": true }],
                "body": [{ "step": "produce", "index": 0 }, { "step": "suspend", "ticks": 3 }],
            }))["answered"],
            json!(true)
        );
        let ran = call(json!({
            "action": "run",
            "name": "hold",
            "arguments": [11],
        }));
        assert_eq!(ran["answered"], json!(true));
        assert_eq!(ran["last"], json!(11), "the body produced its parameter");
        assert_eq!(ran["delay"], json!(3), "and suspended the program");

        assert_eq!(
            call(json!({
                "action": "publish_dimension",
                "owner": "dependent",
                "identifier": "dependent:vault",
                "energy": 500,
                "capacity": 1000,
                "probes": 2,
            }))["answered"],
            json!(true)
        );
        let read = call(json!({ "action": "read_dimension", "identifier": "dependent:vault" }));
        assert_eq!(read["owner"], json!("dependent"));
        assert_eq!(read["energy"], json!(500));

        assert_eq!(
            entry.surface().modules().len(),
            1,
            "a dependent with no hold on the entry value reached every §15 registrar"
        );
    }

    #[test]
    fn an_instruction_whose_body_is_not_this_entrys_is_refused_rather_than_registered() {
        let mut entry = Entry::new(Side::DedicatedServer);
        let surface = Entry::surface_service().expect("a legal service name");
        for arguments in [
            json!({ "action": "register_instruction", "name": "x", "output": "amount",
                    "body": [{ "step": "detonate" }] }),
            json!({ "action": "register_instruction", "name": "x", "output": "amount",
                    "body": [{ "step": "suspend", "ticks": -1 }] }),
            json!({ "action": "register_instruction", "name": "x", "output": "amount",
                    "parameters": [{ "kind": "colour" }] }),
            json!({ "action": "register_instruction", "name": "x", "output": "amount",
                    "body": [{ "step": "produce", "index": 4 }] }),
        ] {
            let answer = entry
                .answer_service(&ServiceRequest::new(surface.clone(), arguments))
                .expect("the entry answers this service");
            assert_eq!(
                answer.value()["answered"],
                json!(false),
                "an unreadable declaration is refused rather than half-registered"
            );
        }
        let ran = entry
            .answer_service(&ServiceRequest::new(
                surface,
                json!({ "action": "run", "name": "x", "arguments": [] }),
            ))
            .expect("the entry answers this service");
        assert_eq!(
            ran.value()["answered"],
            json!(false),
            "nothing was registered under the refused name"
        );
    }

    #[test]
    fn a_service_call_naming_nothing_this_entry_publishes_is_refused_rather_than_guessed() {
        let mut entry = Entry::new(Side::DedicatedServer);
        let surface = Entry::surface_service().expect("a legal service name");

        for arguments in [json!({}), json!({ "action": "drop_everything" })] {
            let answer = entry
                .answer_service(&ServiceRequest::new(surface.clone(), arguments))
                .expect("the entry answers this service");
            assert_eq!(answer.value()["answered"], json!(false));
        }
    }

    #[test]
    fn a_queue_in_any_dimension_is_drained_by_a_server_tick() {
        let mut entry = Entry::new(Side::DedicatedServer);
        for dimension in [
            DEFAULT_DIMENSION,
            "counterpart:the_nether",
            "counterpart:the_end",
        ] {
            entry
                .ordering_mut()
                .enqueue(dimension, 0, "dependent")
                .expect("a legal rank");
        }

        entry
            .observe(GameEvent::ServerTick)
            .expect("the entry records what it observes");
        assert_eq!(entry.ordering().dimensions(), 0);
        assert_eq!(entry.ordering().stamp(), 1);
    }

    #[test]
    fn the_claimed_cycle_command_runs_rather_than_being_declared_and_refused() {
        let mut entry = Entry::new(Side::DedicatedServer);
        let mut registrar = Accepting::default();
        entry.register(&mut registrar).expect("nothing is refused");
        let claimed: Vec<&RegistryKey> = registrar
            .claimed()
            .iter()
            .filter(|key| key.kind() == ContentKind::Command)
            .collect();

        let mut entry = Entry::new(Side::DedicatedServer);
        entry.holding_mut().kind = network::Held::Carrier;
        entry.holding_mut().hand = Some(Hand::Main);
        for slot in [1, 4] {
            let mut device = Stack::of("counterpart:device").expect("a legal identity");
            device.carrier_ready = true;
            entry
                .carrier_mut()
                .insert(slot, device)
                .expect("it declares carrier support");
        }

        let key = claimed
            .iter()
            .find(|key| key.id().path() == commands::CYCLE)
            .expect("the cycle command is claimed");
        entry
            .run_command(&CommandInvocation::new(
                (*key).clone(),
                Side::DedicatedServer,
                vec!["true".to_owned()],
            ))
            .expect("the entry runs a command it claimed");
        assert_eq!(
            entry.carrier().selected(),
            1,
            "the command moved the carrier rather than answering that it had"
        );
    }

    #[test]
    fn a_client_cycle_command_is_refused_before_mutating_the_carrier() {
        let mut entry = Entry::new(Side::DedicatedServer);
        entry.holding_mut().kind = network::Held::Carrier;
        entry.holding_mut().hand = Some(Hand::Main);
        let mut device = Stack::of("counterpart:device").expect("a legal identity");
        device.carrier_ready = true;
        entry
            .carrier_mut()
            .insert(1, device)
            .expect("supported device");
        let before = entry.carrier().clone();
        let key = census::claimed(MOD_ID.parse().expect("a legal identity"))
            .expect("legal names")
            .into_iter()
            .find(|key| key.id().path() == commands::CYCLE)
            .expect("the cycle command is claimed");

        assert!(
            entry
                .run_command(&CommandInvocation::new(
                    key,
                    Side::Client,
                    vec!["true".to_owned()],
                ))
                .is_err()
        );
        assert_eq!(entry.carrier(), &before);
        assert_eq!(entry.cycled, None);
    }

    #[test]
    fn a_cycle_command_whose_direction_is_not_one_of_the_two_words_is_refused() {
        let mut entry = Entry::new(Side::DedicatedServer);
        let key = census::claimed(MOD_ID.parse().expect("the packaged identity is legal"))
            .expect("the packaged names are legal")
            .into_iter()
            .find(|key| key.id().path() == commands::CYCLE)
            .expect("the cycle command is claimed");

        assert!(
            entry
                .run_command(&CommandInvocation::new(
                    key,
                    Side::DedicatedServer,
                    vec!["sideways".to_owned()],
                ))
                .is_err(),
            "text a player typed is refused rather than read as one of the two words"
        );
    }

    #[test]
    fn a_components_payload_is_delivered_decoded_and_written_into_the_card() {
        let mut entry = card_holder();
        let mut card = Pattern::new();
        card.set_input(0, Some(Stack::of("counterpart:beam").expect("legal")))
            .expect("an input position");

        let mut offered = Stack::of("counterpart:card").expect("legal");
        offered.components.insert(
            Record::PatternContents.as_str().to_owned(),
            card.to_saved().to_string(),
        );
        let delivery = delivery_of(&network::HeldComponents { offered }, Side::DedicatedServer);

        entry
            .receive_packet(&delivery)
            .expect("the entry reads a contract it declared");
        assert_eq!(
            entry
                .card()
                .positions()
                .first()
                .and_then(Option::as_ref)
                .map(|stack| stack.identity.to_string()),
            Some("counterpart:beam".to_owned()),
            "the payload reached the card rather than a validation nothing called"
        );
    }

    #[test]
    fn a_readout_request_out_of_range_answers_nothing_and_one_in_range_answers() {
        let mut entry = Entry::new(Side::DedicatedServer);
        entry.holding_mut().chunk_loaded = true;
        entry.holding_mut().at = (0, 0, 0);

        entry
            .receive_packet(&delivery_of(
                &network::ReadoutRequest { at: (0, 0, 4) },
                Side::DedicatedServer,
            ))
            .expect("the entry reads a contract it declared");
        assert!(entry.answered().is_some());

        entry
            .receive_packet(&delivery_of(
                &network::ReadoutRequest { at: (0, 0, 4096) },
                Side::DedicatedServer,
            ))
            .expect("the entry reads a contract it declared");
        assert!(
            entry.answered().is_none(),
            "a position further away than a readout may be read is not answered"
        );
    }

    #[test]
    fn a_wholesale_payload_carrying_an_unsound_record_changes_nothing() {
        let mut entry = Entry::new(Side::DedicatedServer);
        entry.holding_mut().kind = network::Held::Carrier;
        let before = entry.carrier().clone();

        let mut sent = Stack::of("counterpart:carrier").expect("legal");
        sent.components.insert(
            Record::CarrierContents.as_str().to_owned(),
            json!({
                "slots": [Stack::of("counterpart:plain").expect("legal").to_saved(),
                          null, null, null, null, null],
                "selected": 0,
            })
            .to_string(),
        );
        entry
            .receive_packet(&delivery_of(
                &network::HeldItem { sent },
                Side::DedicatedServer,
            ))
            .expect("the entry reads a contract it declared");
        assert_eq!(
            entry.carrier(),
            &before,
            "a record naming an item the carrier does not support is not installed"
        );
    }

    #[test]
    fn a_client_bound_answer_reaches_the_client_only_memory() {
        let mut entry = Entry::new(Side::Client);
        let answer = network::ReadoutAnswer::new(
            (1, 2, 3),
            &[("stored".to_owned(), values::Reading::Amount(9))],
        )
        .expect("bounded values");
        entry
            .receive_packet(&delivery_of(&answer, Side::Client))
            .expect("a client may receive it");
        assert_eq!(
            entry
                .client()
                .expect("a client builds it")
                .screen
                .answer()
                .expect("the answer arrived")
                .value("stored"),
            Some(&values::Reading::Amount(9))
        );
    }

    #[test]
    fn the_five_records_survive_a_world_reload_through_ravels_own_seam() {
        let mut entry = Entry::new(Side::DedicatedServer);
        entry
            .store_mut()
            .write(Record::InfusionProgress, "0,64,0", json!(12))
            .expect("a legal key");
        entry
            .store_mut()
            .write(Record::CarrierContents, "0,64,0", Carrier::new().to_saved())
            .expect("a legal key");

        let envelope = entry
            .save_saved_data()
            .expect("saving does not fail")
            .expect("this entry owns world data");
        let mut reopened = Entry::new(Side::DedicatedServer);
        reopened
            .open_saved_data(Some(&envelope))
            .expect("the world reads back");
        assert_eq!(
            reopened.store().read(Record::InfusionProgress, "0,64,0"),
            Some(&json!(12)),
            "progress a world stored is progress a world opens on"
        );
        assert_eq!(reopened.store().stored(Record::CarrierContents), 1);
    }

    #[test]
    fn a_world_that_stored_nothing_opens_on_nothing_rather_than_the_last_worlds_records() {
        let mut entry = Entry::new(Side::DedicatedServer);
        entry
            .store_mut()
            .write(Record::InfusionProgress, "0,64,0", json!(12))
            .expect("a legal key");

        entry
            .open_saved_data(None)
            .expect("a world that stored nothing opens");
        assert_eq!(entry.store().stored(Record::InfusionProgress), 0);
    }

    #[test]
    fn the_published_surface_is_what_the_dependents_of_the_specification_need() {
        assert!(
            Group::ALL.iter().any(|group| !group.implemented()),
            "two groups are published for consumers of a subsystem this entry does not own"
        );
    }
}
