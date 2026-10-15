#![forbid(unsafe_code)]

//! An original Ravel implementation of the behaviour specified in RVL-210.
//!
//! The wave's foundation seed: a shared library with almost no gameplay of its
//! own, whose whole product is machinery the mods that declare it are built on.
//! Section 1.1 says what survives as counterpart work, and this package is that
//! list: the wire contracts of sections six and seven, the collection and
//! storage semantics of section ten, the query grammar of section eleven, the
//! shape detector of section twelve, and the registration vocabulary of section
//! five that dependents declare against.
//!
//! The wave decision makes this package the **owner** of the interface
//! vocabulary section ten rests on, which the original ships inside its own
//! artifact from a sub-repository and which neither plan row names. Section
//! eighteen raised it as blocking both counterparts; it lives in [`quantity`]
//! and [`storage`], and RVL-132's counterpart consumes it from here.
//!
//! What a reader reaches for:
//!
//! - [`quantity`] — the quantity kind, its conditions, and the collections;
//! - [`storage`] — the stores, the move semantics and the difference tracker;
//! - [`transport`] — the wire format's field ordering and the codec table;
//! - [`channel`] — the screen value channel's seven readers;
//! - [`query`] — the twenty-two-handler grammar;
//! - [`shapes`] — the multiblock detector and its failure detail;
//! - [`declarations`] — the registration vocabulary and this entry's own census;
//! - [`settings`] — the two settings and the declaration framework;
//! - [`menus`] — the slot ranges, the indexed store and the change counter;
//! - [`persistence`] — the persistable fields and the per-world counters;
//! - [`dumps`] — the seven registry-dumping producers;
//! - [`commands`] — the seven subcommands;
//! - [`capabilities`] — the attachment registry section seventeen leans on;
//! - [`helpers`] — block-entity lookup, store and fluid helpers, side questions.
//!
//! `NOTES.md` beside this file says which sections are carried here, which are
//! declarable only, and what every declared behaviour change is. Section nine's
//! outbound fetch and its hard-coded account list are reproduced by neither
//! half, which the specification states as a deliberate divergence.

pub mod capabilities;
pub mod channel;
pub mod commands;
pub mod declarations;
pub mod dumps;
pub mod helpers;
pub mod menus;
pub mod persistence;
pub mod quantity;
pub mod query;
pub mod settings;
pub mod shapes;
pub mod storage;
pub mod transport;

use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration};
use ravel_sdk::content::{ContentDeclaration, Registrar, claim_all};
use ravel_sdk::events::GameEvent;
use ravel_sdk::network::PacketDelivery;
use ravel_sdk::prelude::*;
use ravel_sdk::save::{SavedDataDeclaration, SavedDataEnvelope, SavedDataPlan, SavedDataVersion};
use ravel_sdk::services::{ServiceDeclaration, ServiceId, ServiceRequest, ServiceResponse};
use ravel_sdk::{JsonValue, json};

pub use crate::capabilities::{Address, Registry, TargetClass};
pub use crate::channel::Channel;
pub use crate::declarations::{Declaration, Declarations};
pub use crate::helpers::Sides;
pub use crate::menus::{Ranges, Role, Stack, Store};
pub use crate::persistence::{Counters, FieldTypes, Marked};
pub use crate::quantity::{
    Amount, Chain, Classified, Collapsed, Conditions, Held, KindError, List, Partitioned,
    Partitioning, QuantityKind, Summing,
};
pub use crate::query::{ParseError, Query, Segment};
pub use crate::settings::{Framework, Scope, Settings};
pub use crate::shapes::{Constraint, Failure, Reason, Region};
pub use crate::storage::{
    Attempt, Cell, Difference, Inconsistent, Slotted, SlottedStore, Store as QuantityStore, Tank,
    difference, move_quantity,
};
pub use crate::transport::{Codecs, Level, Shape, Target, TransportError};

/// The identity this package is declared and validated under.
pub const MOD_ID: &str = "rvl_210_counterpart";

/// The service a dependent contributes a registry-dumping producer through.
pub const PRODUCER_SERVICE: &str = "dump_producers";

/// The version the version subcommand reports.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The one saved-data version this build writes and reads.
pub const DATA_VERSION: SavedDataVersion = SavedDataVersion::MIN;

/// This platform's additions to the wire codec table, loaded at common setup.
pub const PLATFORM_CODECS: [(&str, &str); 2] = [
    ("position", transport::NUMBER),
    ("identifier", transport::TEXT),
];

/// This platform's additions to the persistable field-type table.
pub const PLATFORM_FIELD_TYPES: [&str; 2] = ["position", "identifier"];

/// The three advancement-trigger event hooks installed at common setup.
pub const TRIGGER_HOOKS: [&str; 3] = ["screen_opened", "item_crafted", "first_item_obtained"];

/// The entry, as the loader drives it.
#[derive(Debug)]
pub struct Entry {
    declaration: Option<ContentDeclaration>,
    settings: Settings,
    framework: Framework,
    declarations: Declarations,
    codecs: Codecs,
    field_types: FieldTypes,
    capabilities: Registry,
    producers: dumps::Producers,
    counters: Counters,
    channel: Channel,
    entered: Vec<LifecycleState>,
    crafting_helpers: bool,
    load_complete: bool,
    carried: u64,
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
            declaration: Self::declaration().ok(),
            settings: Settings::default(),
            framework: settings::own_framework(),
            declarations: declarations::own_declarations(),
            codecs: Codecs::new(),
            field_types: FieldTypes::new(),
            capabilities: Registry::new(),
            producers: dumps::Producers::new(),
            counters: Counters::new(),
            channel: Channel::new(),
            entered: Vec::new(),
            crafting_helpers: false,
            load_complete: false,
            carried: 0,
        }
    }

    /// The values in force.
    #[must_use]
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The settings framework a dependent declares through.
    #[must_use]
    pub const fn framework(&self) -> &Framework {
        &self.framework
    }

    /// The settings framework, for a runtime change through the command tree.
    pub const fn framework_mut(&mut self) -> &mut Framework {
        &mut self.framework
    }

    /// What this entry declares of its own.
    #[must_use]
    pub const fn declarations(&self) -> &Declarations {
        &self.declarations
    }

    /// The wire codec table, which a dependent extends for its own types.
    #[must_use]
    pub const fn codecs(&self) -> &Codecs {
        &self.codecs
    }

    /// The persistable field-type table.
    #[must_use]
    pub const fn field_types(&self) -> &FieldTypes {
        &self.field_types
    }

    /// The capability-attachment registry of section seventeen.
    #[must_use]
    pub const fn capabilities(&self) -> &Registry {
        &self.capabilities
    }

    /// The capability-attachment registry, for a dependent's registration.
    pub const fn capabilities_mut(&mut self) -> &mut Registry {
        &mut self.capabilities
    }

    /// The registry-dumping producers collected so far.
    #[must_use]
    pub const fn producers(&self) -> &dumps::Producers {
        &self.producers
    }

    /// The per-world counters this entry owns.
    #[must_use]
    pub const fn counters(&self) -> &Counters {
        &self.counters
    }

    /// The per-world counters, so a dependent can allocate a durable identifier.
    pub const fn counters_mut(&mut self) -> &mut Counters {
        &mut self.counters
    }

    /// The screen value channel.
    #[must_use]
    pub const fn channel(&self) -> &Channel {
        &self.channel
    }

    /// The screen value channel, for the server pushing a change.
    pub const fn channel_mut(&mut self) -> &mut Channel {
        &mut self.channel
    }

    /// The lifecycle steps this entry has taken.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }

    /// Whether the crafting helper tables were built, which happens on a client
    /// only. On a dedicated server they are never built.
    #[must_use]
    pub const fn crafting_helpers(&self) -> bool {
        self.crafting_helpers
    }

    /// How many of this entry's own thirteen messages it has carried.
    #[must_use]
    pub const fn carried(&self) -> u64 {
        self.carried
    }

    /// The latched flag a dependent reads to ask whether loading finished.
    #[must_use]
    pub const fn load_complete(&self) -> bool {
        self.load_complete
    }

    /// The service a dependent contributes a dumping producer through.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when the declared name is not a legal
    /// content path, which for a constant is a packaging failure.
    pub fn producer_service() -> Result<ServiceId, ModError> {
        Ok(ServiceId::new(
            ContentId::new(Self::declared_id().clone(), PRODUCER_SERVICE)
                .map_err(|error| Self::packaging(&error))?,
        ))
    }

    /// What this entry claims at the registration barrier.
    ///
    /// The command tree of section fifteen is the whole of it: nothing this
    /// entry declares is a block, an item or a recipe, and Ravel's registry has
    /// no kind for a particle, a data component or a trigger.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when a declared path is not a legal content
    /// path, which for these names would be a packaging failure.
    pub fn declaration() -> Result<ContentDeclaration, ModError> {
        let mut declaration = ContentDeclaration::new(Self::declared_id().clone());
        for (name, _) in commands::SUBCOMMANDS {
            declaration
                .entry(ContentKind::Command, name)
                .map_err(|error| Self::packaging(&error))?;
        }
        Ok(declaration)
    }

    /// Loads what common setup loads, on the side it is running on.
    ///
    /// The producer set, the two table halves and the trigger hooks land on
    /// every side. The crafting helper tables are a client-only build, which is
    /// a side test at runtime rather than a client-only declaration, so on a
    /// dedicated server that table is never built.
    pub fn common_setup(&mut self, side: Side) {
        for name in dumps::SHIPPED {
            self.producers.collect(name);
        }
        self.codecs.load_platform(&PLATFORM_CODECS);
        self.field_types.load_platform(&PLATFORM_FIELD_TYPES);
        if side == Side::Client {
            self.crafting_helpers = true;
        }
    }

    fn contribute(&mut self, arguments: &JsonValue) -> JsonValue {
        let Some(producer) = arguments.get("producer").and_then(JsonValue::as_str) else {
            return json!({ "collected": false, "refused": "a contribution names no producer" });
        };
        if self.producers.collect(producer) {
            json!({ "collected": true })
        } else {
            json!({ "collected": false, "refused": "that producer is already collected" })
        }
    }

    /// Reads or writes one setting, which is what the settings subcommand does.
    ///
    /// A written value is read as a truth value or a whole number when it is
    /// one and as text otherwise, so a value of the wrong kind reaches
    /// [`Framework::write`] and is refused there rather than coerced.
    fn run_settings(&mut self, arguments: &[String]) -> Result<(), ModError> {
        let mut arguments = arguments.iter();
        let Some(property) = arguments.next() else {
            return Ok(());
        };
        match arguments.next() {
            Some(written) => {
                let value = match written.as_str() {
                    "true" => settings::Value::Truth(true),
                    "false" => settings::Value::Truth(false),
                    other => other.parse::<i64>().map_or_else(
                        |_| settings::Value::Text(other.to_owned()),
                        settings::Value::Number,
                    ),
                };
                self.framework
                    .write(property, value)
                    .map_err(|error| Self::refused(&error))?;
            }
            None => {
                self.framework
                    .read(property)
                    .map_err(|error| Self::refused(&error))?;
            }
        }
        Ok(())
    }

    fn refused(detail: &(impl ToString + ?Sized)) -> ModError {
        ModError::Refused {
            mod_id: Self::declared_id().clone(),
            state: LifecycleState::Running,
            detail: detail.to_string(),
        }
    }

    fn packaging(detail: &(impl ToString + ?Sized)) -> ModError {
        ModError::Refused {
            mod_id: Self::declared_id().clone(),
            state: LifecycleState::Constructed,
            detail: detail.to_string(),
        }
    }
}

#[declare_mod(id = "rvl_210_counterpart")]
impl Mod for Entry {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        match state {
            LifecycleState::Constructed => {
                settings::declaration().map_err(|error| Self::packaging(&error))?;
                self.declaration.as_ref().ok_or_else(|| {
                    Self::packaging("the packaged content declaration is not well formed")
                })?;
            }
            LifecycleState::Running => {
                self.framework.resolve();
                self.load_complete = true;
            }
            _ => {}
        }
        self.entered.push(state);
        Ok(())
    }

    fn content_declaration(&self) -> Option<ContentDeclaration> {
        self.declaration.clone()
    }

    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
        let declaration = self.declaration.as_ref().ok_or_else(|| {
            Self::packaging("the packaged content declaration is not well formed")
        })?;
        claim_all(declaration, registrar)?;
        self.capabilities.bake();
        Ok(())
    }

    fn configuration_declaration(&self) -> Option<ConfigurationDeclaration> {
        settings::declaration().ok()
    }

    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
        self.settings = Settings::read(resolved);
        Ok(())
    }

    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        // Section six's resource reload request is one of the thirteen and this
        // is the signal it exists to cause; nothing else here is driven by a
        // tick, so the other two are observed and add nothing.
        if event == GameEvent::ResourcesReloaded {
            self.channel = Channel::new();
        }
        Ok(())
    }

    fn run_command(&mut self, invocation: &CommandInvocation) -> Result<(), ModError> {
        let name = invocation.key.id().path();
        let resolved = commands::resolve(name, &invocation.arguments)
            .map_err(|error| Self::refused(&error))?;
        match resolved {
            commands::SETTINGS => self.run_settings(&invocation.arguments)?,
            // The same emptying section seven's reload causes, because this is
            // the subcommand that causes it.
            commands::RELOAD => self.channel = Channel::new(),
            // The other five change nothing this entry holds: what each answers
            // is a chat surface Ravel does not publish, which `NOTES.md` records
            // as declarable only.
            _ => {}
        }
        Ok(())
    }

    fn receive_packet(&mut self, delivery: &PacketDelivery) -> Result<(), ModError> {
        // A delivery that is not one of the thirteen is not this entry's and is
        // left alone rather than refused: the transport carries a dependent's
        // own messages too, and refusing them here would break the dependent.
        if transport::MESSAGE_KINDS
            .iter()
            .any(|kind| delivery.id.id().path() == *kind)
        {
            self.carried = self.carried.saturating_add(1);
        }
        Ok(())
    }

    fn service_declaration(&self) -> Option<ServiceDeclaration> {
        let service = Self::producer_service().ok()?;
        ServiceDeclaration::new(Self::declared_id().clone(), [service], []).ok()
    }

    fn answer_service(&mut self, request: &ServiceRequest) -> Result<ServiceResponse, ModError> {
        let service = Self::producer_service()?;
        if request.id != service {
            return Err(Self::packaging("this entry answers one service"));
        }
        Ok(ServiceResponse::new(self.contribute(&request.arguments)))
    }

    fn saved_data_declaration(&self) -> Option<SavedDataDeclaration> {
        SavedDataDeclaration::new(Self::declared_id().clone(), [DATA_VERSION]).ok()
    }

    fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, ModError> {
        Ok(Some(SavedDataEnvelope::current(
            Self::declared_id().clone(),
            DATA_VERSION,
            self.counters.save(),
        )))
    }

    fn open_saved_data(
        &mut self,
        envelope: Option<&SavedDataEnvelope>,
        plan: &SavedDataPlan,
    ) -> Result<(), ModError> {
        // Section thirteen scopes the counters to a world, so a world that
        // stored nothing opens on nothing rather than on the last one's counts.
        self.counters = Counters::new();
        if let Some(envelope) = envelope {
            if !plan.steps().is_empty() {
                return Err(Self::packaging(
                    "this build declares one saved-data version and so crosses no step",
                ));
            }
            if envelope.data_version() != DATA_VERSION {
                return Err(Self::packaging(
                    "the world stored a version this build does not read",
                ));
            }
            self.counters
                .load(envelope.payload())
                .map_err(|refused| ModError::Refused {
                    mod_id: Self::declared_id().clone(),
                    state: LifecycleState::Running,
                    detail: refused.to_string(),
                })?;
        }
        // Last, so a delivery this entry refused above allocates nothing and
        // leaves the counts it was holding alone.
        self.counters
            .next(persistence::OPENINGS)
            .map_err(|refused| ModError::Refused {
                mod_id: Self::declared_id().clone(),
                state: LifecycleState::Running,
                detail: refused.to_string(),
            })?;
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
    fn the_command_tree_is_the_whole_of_what_reaches_the_barrier() {
        let mut entry = Entry::new();
        let mut registrar = Accepting::default();
        entry.register(&mut registrar).expect("nothing is refused");
        assert_eq!(registrar.claimed().len(), commands::SUBCOMMANDS.len());
        assert!(
            registrar
                .claimed()
                .iter()
                .all(|key| key.kind() == ContentKind::Command)
        );
    }

    #[test]
    fn the_capability_registry_is_baked_at_the_barrier_and_refuses_afterwards() {
        let mut entry = Entry::new();
        entry
            .capabilities_mut()
            .register(
                TargetClass::Item,
                Address::Specific("one".to_owned()),
                "store",
            )
            .expect("the registry is open before the barrier");
        let mut registrar = Accepting::default();
        entry.register(&mut registrar).expect("nothing is refused");
        assert!(entry.capabilities().baked());
        assert!(
            entry
                .capabilities_mut()
                .register(
                    TargetClass::Item,
                    Address::Specific("two".to_owned()),
                    "store"
                )
                .is_err()
        );
    }

    #[test]
    fn the_entry_declares_its_content_its_settings_its_service_and_its_saved_data() {
        let entry = Entry::new();
        assert!(entry.content_declaration().is_some());
        assert!(entry.configuration_declaration().is_some());
        assert_eq!(
            entry
                .service_declaration()
                .expect("the producer service")
                .provides()
                .count(),
            1
        );
        assert!(entry.saved_data_declaration().is_some());
    }

    #[test]
    fn common_setup_builds_the_crafting_helpers_on_a_client_and_on_no_server() {
        for side in [Side::DedicatedServer, Side::IntegratedServer] {
            let mut entry = Entry::new();
            entry.common_setup(side);
            assert!(
                !entry.crafting_helpers(),
                "a server built the client-only table"
            );
        }
        let mut entry = Entry::new();
        entry.common_setup(Side::Client);
        assert!(entry.crafting_helpers());
    }

    #[test]
    fn common_setup_fills_both_tables_and_the_producer_set_on_every_side() {
        for side in [Side::Client, Side::IntegratedServer, Side::DedicatedServer] {
            let mut entry = Entry::new();
            assert!(!entry.codecs().platform_loaded());
            assert!(!entry.field_types().platform_loaded());
            assert!(entry.producers().is_empty());
            entry.common_setup(side);
            assert!(entry.codecs().platform_loaded());
            assert!(entry.field_types().platform_loaded());
            assert_eq!(entry.producers().len(), dumps::SHIPPED.len());
        }
    }

    #[test]
    fn the_portable_table_entries_are_there_before_common_setup_runs() {
        let entry = Entry::new();
        for name in transport::PORTABLE_CODECS {
            assert!(entry.codecs().get(name).is_some());
        }
        for name in persistence::PORTABLE_FIELD_TYPES {
            assert!(entry.field_types().holds(name));
        }
        assert!(
            entry.codecs().get("position").is_none(),
            "a platform entry before common setup has nothing to read"
        );
    }

    #[test]
    fn three_trigger_hooks_are_installed_and_each_names_a_declared_trigger() {
        assert_eq!(TRIGGER_HOOKS.len(), 3);
        let declared = declarations::own_declarations();
        for hook in TRIGGER_HOOKS {
            assert!(
                declared
                    .declared()
                    .iter()
                    .any(|declaration| declaration.name() == hook
                        && declaration.kind() == "advancement_trigger_kind"),
                "{hook} is hooked and not declared"
            );
        }
    }

    #[test]
    fn the_load_complete_flag_latches_when_the_entry_starts_running() {
        let mut entry = Entry::new();
        assert!(!entry.load_complete());
        entry.enter(LifecycleState::Constructed).expect("declared");
        assert!(!entry.load_complete());
        entry.enter(LifecycleState::Running).expect("running");
        assert!(entry.load_complete());
        assert!(entry.framework().resolved());
    }

    #[test]
    fn a_dependent_contributes_a_producer_through_the_service_and_a_repeat_is_named() {
        let mut entry = Entry::new();
        let service = Entry::producer_service().expect("a legal service name");
        let answer = entry
            .answer_service(&ServiceRequest::new(
                service.clone(),
                json!({ "producer": "mine" }),
            ))
            .expect("the entry answers its own service");
        assert_eq!(answer.value().get("collected"), Some(&json!(true)));
        let again = entry
            .answer_service(&ServiceRequest::new(
                service.clone(),
                json!({ "producer": "mine" }),
            ))
            .expect("answered");
        assert_eq!(again.value().get("collected"), Some(&json!(false)));
        let nameless = entry
            .answer_service(&ServiceRequest::new(service, json!({})))
            .expect("answered");
        assert_eq!(nameless.value().get("collected"), Some(&json!(false)));
    }

    #[test]
    fn the_counters_survive_a_save_and_a_reopen() {
        let mut entry = Entry::new();
        for _ in 0..3 {
            entry
                .counters_mut()
                .next("identifiers")
                .expect("inside the limit");
        }
        let saved = entry
            .save_saved_data()
            .expect("the entry writes its counters")
            .expect("a declared owner writes an envelope");
        let mut reopened = Entry::new();
        let plan = SavedDataPlan::open(
            &reopened
                .saved_data_declaration()
                .expect("the entry declares its saved data"),
            Some(saved.data_version()),
        )
        .expect("the world stored the version this build writes");
        reopened
            .open_saved_data(Some(&saved), &plan)
            .expect("the world opens");
        assert_eq!(reopened.counters().last("identifiers"), 3);
    }

    #[test]
    fn every_opening_of_a_world_allocates_the_next_value_of_the_entrys_own_counter() {
        let mut first = Entry::new();
        let fresh = SavedDataPlan::open(
            &first
                .saved_data_declaration()
                .expect("the entry declares its saved data"),
            None,
        )
        .expect("absence initializes");
        first
            .open_saved_data(None, &fresh)
            .expect("a fresh world opens");
        assert_eq!(first.counters().last(persistence::OPENINGS), 1);
        let stored = first
            .save_saved_data()
            .expect("the entry writes its counters")
            .expect("a declared owner writes an envelope");

        let mut second = Entry::new();
        let again = SavedDataPlan::open(
            &second
                .saved_data_declaration()
                .expect("the entry declares its saved data"),
            Some(stored.data_version()),
        )
        .expect("the world stored the version this build writes");
        second
            .open_saved_data(Some(&stored), &again)
            .expect("the world opens");
        assert_eq!(
            second.counters().last(persistence::OPENINGS),
            2,
            "the sequence survives the restart rather than starting again"
        );
        let round = second
            .save_saved_data()
            .expect("the entry writes its counters")
            .expect("a declared owner writes an envelope");
        assert_ne!(
            round.payload(),
            stored.payload(),
            "a visit that read the record writes a successor to it"
        );
    }

    #[test]
    fn a_resource_reload_empties_the_screen_value_channel() {
        let mut entry = Entry::new();
        entry.channel_mut().send(1, json!(7));
        assert!(!entry.channel().is_empty());
        entry
            .observe(GameEvent::ResourcesReloaded)
            .expect("observed");
        assert!(entry.channel().is_empty());
        entry.channel_mut().send(1, json!(7));
        entry.observe(GameEvent::ServerTick).expect("observed");
        assert!(!entry.channel().is_empty(), "a tick empties nothing");
    }

    fn invoke(entry: &mut Entry, name: &str, arguments: &[&str]) -> Result<(), ModError> {
        let key = RegistryKey::new(
            ContentKind::Command,
            ContentId::new(Entry::declared_id().clone(), name).expect("a legal path"),
        );
        let arguments = arguments.iter().map(|held| (*held).to_owned()).collect();
        entry.run_command(&CommandInvocation::new(
            key,
            Side::DedicatedServer,
            arguments,
        ))
    }

    #[test]
    fn the_settings_subcommand_writes_the_setting_it_names() {
        let mut entry = Entry::new();
        assert_eq!(
            entry.framework().read(settings::DEVELOPMENT_SHORTCUT),
            Ok(&settings::Value::Truth(false))
        );
        invoke(
            &mut entry,
            commands::SETTINGS,
            &[settings::DEVELOPMENT_SHORTCUT, "true"],
        )
        .expect("a changeable setting written with a truth value");
        assert_eq!(
            entry.framework().read(settings::DEVELOPMENT_SHORTCUT),
            Ok(&settings::Value::Truth(true)),
            "the tree resolved the name and then wrote through it"
        );
    }

    #[test]
    fn the_settings_subcommand_refuses_a_name_the_framework_lacks_and_a_value_of_another_kind() {
        let mut entry = Entry::new();
        assert!(
            invoke(&mut entry, commands::SETTINGS, &["never_declared"]).is_err(),
            "a read of an undeclared property is refused"
        );
        assert!(
            invoke(
                &mut entry,
                commands::SETTINGS,
                &[settings::DEVELOPMENT_SILENCE_MUSIC, "seventeen"],
            )
            .is_err(),
            "text written over a truth value is refused rather than coerced"
        );
        assert_eq!(
            entry.framework().read(settings::DEVELOPMENT_SILENCE_MUSIC),
            Ok(&settings::Value::Truth(false)),
            "a refused write changes nothing"
        );
    }

    #[test]
    fn the_reload_subcommand_empties_the_screen_value_channel() {
        let mut entry = Entry::new();
        entry.channel_mut().send(1, json!(7));
        invoke(&mut entry, commands::RELOAD, &[]).expect("the reload subcommand runs");
        assert!(entry.channel().is_empty());
    }

    #[test]
    fn a_world_that_stored_nothing_opens_on_no_counters_rather_than_the_last_world_s() {
        let mut entry = Entry::new();
        for _ in 0..3 {
            entry
                .counters_mut()
                .next("identifiers")
                .expect("inside the limit");
        }
        let plan = SavedDataPlan::open(
            &entry
                .saved_data_declaration()
                .expect("the entry declares its saved data"),
            None,
        )
        .expect("a world that stored nothing");
        entry.open_saved_data(None, &plan).expect("the world opens");
        assert_eq!(
            entry.counters().last("identifiers"),
            0,
            "section thirteen scopes the counters to a world"
        );
    }

    #[test]
    fn a_world_holding_a_counter_that_is_not_a_number_refuses_to_open() {
        let mut entry = Entry::new();
        let saved = SavedDataEnvelope::current(
            Entry::declared_id().clone(),
            DATA_VERSION,
            json!({ "counters": { "identifiers": "not a number" } }),
        );
        let plan = SavedDataPlan::open(
            &entry
                .saved_data_declaration()
                .expect("the entry declares its saved data"),
            Some(saved.data_version()),
        )
        .expect("the world stored the version this build writes");
        assert!(
            entry.open_saved_data(Some(&saved), &plan).is_err(),
            "opening on zero re-issues identifiers the world already stored"
        );
        assert_eq!(entry.counters().last("identifiers"), 0);
    }

    #[test]
    fn every_platform_codec_names_a_kind_the_table_really_has() {
        // The names used to be remapped through a match that sent anything it
        // did not recognise to the text codec, so a kind that does not exist
        // loaded as text rather than being caught. The constants are the table
        // now, and this is what catches a bad one.
        for (name, kind) in PLATFORM_CODECS {
            assert!(
                transport::PORTABLE_CODECS.contains(&kind),
                "{name} names {kind}, which is not a codec"
            );
        }
        let mut entry = Entry::new();
        entry.common_setup(Side::DedicatedServer);
        assert!(
            entry.codecs().get("position").is_some(),
            "the platform additions reach the table"
        );
    }

    #[test]
    fn a_command_the_tree_does_not_carry_is_refused() {
        let mut entry = Entry::new();
        let key = RegistryKey::new(
            ContentKind::Command,
            ContentId::new(Entry::declared_id().clone(), "never").expect("a legal path"),
        );
        let invocation = CommandInvocation::new(key, Side::DedicatedServer, Vec::new());
        assert!(entry.run_command(&invocation).is_err());
    }

    #[test]
    fn this_package_owns_the_vocabulary_the_dependent_needs_and_names_it_at_the_root() {
        // Section eighteen raised where the shared vocabulary lives as the thing
        // blocking both counterparts. It lives here, and this is the test that
        // says so: every name RVL-132 consumes is reachable from the crate root.
        let kind: fn(&Conditions) -> u32 = |conditions| conditions.bits();
        assert_eq!(kind(&Conditions::NONE), Conditions::NONE.bits());
        let _list = List::new(TokenKind);
        let _chain = Chain::new(TokenKind);
        let _cell = Cell::new(0u32);
        let _tank: Tank<String> = Tank::new(10);
        let _query = Query::parse("$").expect("the parse error type is public too");
        let _sides = Sides::on(Side::Client);
        let _registry = Registry::new();
    }

    #[derive(Clone, Copy, Debug)]
    struct TokenKind;

    impl QuantityKind for TokenKind {
        type Instance = u32;

        fn conditions(&self) -> Conditions {
            Conditions::NONE
        }

        fn matches(&self, left: &u32, right: &u32, _under: Conditions) -> bool {
            left == right
        }

        fn digest(&self, value: &u32, _under: Conditions) -> u64 {
            u64::from(*value)
        }

        fn write(&self, value: &u32) -> JsonValue {
            json!(value)
        }

        fn read(&self, stored: &JsonValue) -> Result<u32, KindError> {
            stored
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| KindError::new("not a number"))
        }
    }
}
