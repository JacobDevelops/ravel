#![forbid(unsafe_code)]

//! Host contract for statically composed Rust mods.

mod interception;
mod provision;
mod services;

pub use interception::{
    InterceptionDeclaration, InterceptionRefused, Occasion, Outcome, Point, Points, Round,
};
pub use provision::{Provided, Provision, ProvisionRefused, Resolved};
pub use services::{
    ServiceCallFailed, ServiceDeclaration, ServiceDeclarationError, ServiceDirectory, ServiceId,
    ServiceRequest, ServiceResponse, ServiceUnavailable,
};

use std::fmt;

use ravel_config::{ConfigurationDeclaration, LayeredConfiguration};
use ravel_core::{
    ContentBehaviour, FailureKind, LifecycleState, ModId, RegistryKey, Side, StructuredFailure,
};
use ravel_data::{ContentDeclaration, PlayRequest};
use ravel_network::PacketDelivery;
use ravel_save::{MigrationChain, SavedDataDeclaration, SavedDataEnvelope, SavedDataPlan};

/// A payload-free game occurrence a mod may observe.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GameEvent {
    /// The integrated or dedicated server finished a tick.
    ServerTick,
    /// The client finished a frame's tick.
    ClientTick,
    /// The game finished reloading resource or data packs.
    ResourcesReloaded,
}

impl GameEvent {
    /// Every event, in the order this enum declares them.
    pub const ALL: [Self; 3] = [Self::ServerTick, Self::ClientTick, Self::ResourcesReloaded];

    /// Returns the event's canonical lowercase name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ServerTick => "server-tick",
            Self::ClientTick => "client-tick",
            Self::ResourcesReloaded => "resources-reloaded",
        }
    }

    /// Returns whether this event can happen on `side`.
    #[must_use]
    pub const fn happens_on(self, side: Side) -> bool {
        match self {
            Self::ServerTick => side.is_server(),
            Self::ClientTick => matches!(side, Side::Client),
            Self::ResourcesReloaded => true,
        }
    }
}

impl fmt::Display for GameEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A command invocation routed to the mod that owns its root literal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandInvocation {
    /// The claimed key whose root literal was typed.
    pub key: RegistryKey,
    /// The side the command ran on.
    pub side: Side,
    /// Untrusted arguments split on whitespace; the mod defines their grammar.
    pub arguments: Vec<String>,
}

impl CommandInvocation {
    /// Records an invocation of `key` on `side` with `arguments`.
    #[must_use]
    pub fn new(key: RegistryKey, side: Side, arguments: Vec<String>) -> Self {
        Self {
            key,
            side,
            arguments,
        }
    }
}

/// A mod's refusal of a lifecycle step, attributed to its identity and phase.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostError {
    /// The mod ran and declined, naming why.
    Refused {
        /// The mod that declined.
        mod_id: ModId,
        /// The lifecycle phase the refused operation belongs to.
        state: LifecycleState,
        /// What the mod said, in the mod's own words and so not yet redacted.
        detail: String,
    },
    /// The mod does not implement the step at all.
    Unsupported {
        /// The mod that has no implementation for the step.
        mod_id: ModId,
        /// The state the mod was asked to enter.
        state: LifecycleState,
    },
}

impl HostError {
    /// The mod this failure is attributed to.
    #[must_use]
    pub const fn mod_id(&self) -> &ModId {
        match self {
            Self::Refused { mod_id, .. } | Self::Unsupported { mod_id, .. } => mod_id,
        }
    }

    /// The lifecycle phase the failed operation belongs to.
    #[must_use]
    pub const fn state(&self) -> LifecycleState {
        match self {
            Self::Refused { state, .. } | Self::Unsupported { state, .. } => *state,
        }
    }
}

impl fmt::Display for HostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused {
                mod_id,
                state,
                detail,
            } => write!(
                formatter,
                "`{mod_id}` refused to enter `{}`: {detail}",
                state.name()
            ),
            Self::Unsupported { mod_id, state } => write!(
                formatter,
                "`{mod_id}` does not implement `{}`",
                state.name()
            ),
        }
    }
}

impl std::error::Error for HostError {}

impl StructuredFailure for HostError {
    /// Lifecycle failures have no dependency-graph classification.
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    fn lifecycle_state(&self) -> LifecycleState {
        self.state()
    }

    /// Only a mod's refusal detail contains untrusted text.
    fn quotes_untrusted_text(&self) -> bool {
        matches!(self, Self::Refused { .. })
    }
}

/// The safe Rust contract the loader drives for a statically composed mod.
pub trait HostMod: Send {
    /// The canonical identity this mod was validated under.
    fn id(&self) -> &ModId;

    /// Takes the mod into `state`.
    ///
    /// The loader calls this once per state, in lifecycle order, and stops at
    /// the first refusal.
    ///
    /// # Errors
    ///
    /// Returns an error when the mod declines the step or does not implement it.
    fn enter(&mut self, state: LifecycleState) -> Result<(), HostError>;

    /// Claims identifiers while constructed, before the registration barrier closes.
    ///
    /// The default claims nothing. Mods decide whether a refused claim is fatal.
    ///
    /// # Errors
    ///
    /// Returns a mod's refusal to register.
    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), HostError> {
        let _ = registrar;
        Ok(())
    }

    /// Observes an event in resolved load order while running; defaults to ignoring it.
    ///
    /// # Errors
    ///
    /// Returns a mod's failure and stops the broadcast before later mods are called.
    fn observe(&mut self, event: GameEvent) -> Result<(), HostError> {
        let _ = event;
        Ok(())
    }

    /// Runs a command owned by this mod.
    ///
    /// # Errors
    ///
    /// Returns [`HostError::Unsupported`] by default, or an implementation's refusal.
    fn run_command(&mut self, invocation: &CommandInvocation) -> Result<(), HostError> {
        let _ = invocation;
        Err(HostError::Unsupported {
            mod_id: self.id().clone(),
            state: LifecycleState::Running,
        })
    }

    /// Receives a packet routed by contract ownership, including its arrival side.
    ///
    /// # Errors
    ///
    /// Returns [`HostError::Unsupported`] by default, or an implementation's decoding or handling refusal.
    fn receive_packet(&mut self, delivery: &PacketDelivery) -> Result<(), HostError> {
        let _ = delivery;
        Err(HostError::Unsupported {
            mod_id: self.id().clone(),
            state: LifecycleState::Running,
        })
    }

    /// Declares content and resources layered in resolved load order; defaults to none.
    ///
    /// # Errors
    /// Returns the mod's declaration refusal.
    fn content_declaration(&self) -> Result<Option<ContentDeclaration>, HostError> {
        Ok(None)
    }

    /// Declares the versioned world data this mod owns, when it owns any.
    ///
    /// # Errors
    /// Returns the mod's declaration refusal.
    fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, HostError> {
        Ok(None)
    }

    /// Supplies migrations checked for completeness before a world payload is read.
    ///
    /// # Errors
    /// Returns the mod's migration refusal.
    fn saved_data_migrations(&self) -> Result<Option<MigrationChain>, HostError> {
        Ok(None)
    }

    /// Applies admitted saved data, migrating through [`SavedDataPlan::steps`] first.
    ///
    /// The envelope may still have its stored version, not the current version.
    /// The default accepts only absence for a mod with no saved-data declaration.
    ///
    /// # Errors
    ///
    /// Returns [`HostError::Unsupported`] by default for other inputs, or a mod's refusal; failure withholds the world.
    fn open_saved_data(
        &mut self,
        envelope: Option<&SavedDataEnvelope>,
        plan: &SavedDataPlan,
    ) -> Result<(), HostError> {
        if envelope.is_none() && self.saved_data_declaration()?.is_none() {
            return Ok(());
        }
        Err(HostError::Unsupported {
            mod_id: plan.owner().clone(),
            state: LifecycleState::Registered,
        })
    }

    /// Produces a complete checkpoint envelope; defaults to none.
    ///
    /// # Errors
    ///
    /// Returns a mod's refusal; failure preserves the previous aggregate without aborting the game save.
    fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, HostError> {
        Ok(None)
    }

    /// Drains sound requests for later host execution, without delivery confirmation.
    fn take_play_requests(&mut self) -> Vec<PlayRequest> {
        Vec::new()
    }

    /// Declares settings layered by the host and returned through [`HostMod::configure`].
    ///
    /// # Errors
    /// Returns the mod's declaration refusal.
    fn configuration_declaration(&self) -> Result<Option<ConfigurationDeclaration>, HostError> {
        Ok(None)
    }

    /// Declares interception points for host ordering and admission; defaults to none.
    ///
    /// # Errors
    /// Returns the mod's declaration refusal.
    fn interception_declaration(&self) -> Result<Option<InterceptionDeclaration>, HostError> {
        Ok(None)
    }

    /// Answers an interception; [`Outcome::Refuse`] ends the round before later mods.
    ///
    /// # Errors
    ///
    /// Returns [`HostError::Unsupported`] by default if interceptions were declared; otherwise proceeds.
    fn intercept(&mut self, occasion: Occasion) -> Result<Outcome, HostError> {
        let _ = occasion;
        if self.interception_declaration()?.is_none() {
            return Ok(Outcome::Proceed);
        }
        Err(HostError::Unsupported {
            mod_id: self.id().clone(),
            state: LifecycleState::Running,
        })
    }

    /// Applies validated, layered settings for this launch.
    ///
    /// # Errors
    ///
    /// Returns [`HostError::Unsupported`] by default if settings were declared, or an implementation's refusal.
    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), HostError> {
        let _ = resolved;
        if self.configuration_declaration()?.is_none() {
            return Ok(());
        }
        Err(HostError::Unsupported {
            mod_id: self.id().clone(),
            state: LifecycleState::Constructed,
        })
    }

    /// Declares the services this mod answers and the ones it needs answered.
    ///
    /// # Errors
    /// Returns the mod's declaration refusal.
    fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, HostError> {
        Ok(None)
    }

    /// Answers one call to a service this mod declared it provides.
    ///
    /// # Errors
    ///
    /// Returns [`HostError::Unsupported`] by default, or an implementation's refusal.
    fn answer_service(&mut self, request: &ServiceRequest) -> Result<ServiceResponse, HostError> {
        let _ = request;
        Err(HostError::Unsupported {
            mod_id: self.id().clone(),
            state: LifecycleState::Running,
        })
    }

    /// Uses declared services through already-started providers; defaults to no calls.
    ///
    /// # Errors
    ///
    /// Returns a mod's refusal, preventing it from starting.
    fn use_services(&mut self, directory: &mut dyn ServiceDirectory) -> Result<(), HostError> {
        let _ = directory;
        Ok(())
    }
}

/// The registration barrier exposed to a mod without a loader dependency.
pub trait Registrar {
    /// Claims `key` for the mod currently registering.
    ///
    /// # Errors
    ///
    /// Returns [`RegistrationRefused`] for an owned identifier or a closed barrier.
    fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused>;

    /// Claims `key` with behaviour; the default discards the description.
    ///
    /// # Errors
    ///
    /// Returns claim refusals and, for registrars retaining behaviour, a refusal for a mismatched kind.
    fn claim_describing(
        &mut self,
        key: RegistryKey,
        behaviour: ContentBehaviour,
    ) -> Result<(), RegistrationRefused> {
        let _ = behaviour;
        self.claim(key)
    }

    /// Returns the identifiers this mod has already claimed, in claim order.
    fn claimed(&self) -> &[RegistryKey];

    /// Returns whether `mod_id` is running; absent, wrong-side, and failed mods return false.
    fn loads(&self, mod_id: &ModId) -> bool;
}

/// A registration refusal exposed to the claiming mod.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistrationRefused {
    /// Another mod already owns the identifier.
    Conflict {
        /// The contested identifier.
        key: RegistryKey,
        /// The mod that claimed it first.
        owner: ModId,
    },
    /// The registration window is closed.
    Closed {
        /// The identifier that arrived too late.
        key: RegistryKey,
    },
}

impl RegistrationRefused {
    /// The identifier the refusal is about.
    #[must_use]
    pub const fn key(&self) -> &RegistryKey {
        match self {
            Self::Conflict { key, .. } | Self::Closed { key } => key,
        }
    }
}

impl fmt::Display for RegistrationRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict { key, owner } => {
                write!(formatter, "`{key}` is already registered by `{owner}`")
            }
            Self::Closed { key } => write!(
                formatter,
                "`{key}` arrived after the registration barrier closed"
            ),
        }
    }
}

impl std::error::Error for RegistrationRefused {}

impl StructuredFailure for RegistrationRefused {
    /// Only a contested identifier is a conflicting registry claim.
    fn failure_kind(&self) -> Option<FailureKind> {
        match self {
            Self::Conflict { .. } => Some(FailureKind::ConflictingRegistryClaim),
            Self::Closed { .. } => None,
        }
    }

    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Constructed
    }

    /// Messages contain only validated identifiers.
    fn quotes_untrusted_text(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{
        ContentKind, FailureKind, LifecycleState, ModId, Redaction, RegistryKey, StructuredFailure,
    };

    use super::{HostError, HostMod, Registrar, RegistrationRefused};

    /// An in-memory registrar for exercising the mod-facing refusal contract.
    #[derive(Default)]
    struct Table {
        claimed: Vec<RegistryKey>,
        owned: Vec<(RegistryKey, ModId)>,
        closed: bool,
    }

    impl Registrar for Table {
        fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
            if self.closed {
                return Err(RegistrationRefused::Closed { key });
            }
            if let Some((_, owner)) = self.owned.iter().find(|(owned, _)| owned == &key) {
                return Err(RegistrationRefused::Conflict {
                    key,
                    owner: owner.clone(),
                });
            }
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

    fn key(kind: ContentKind, id: &str) -> RegistryKey {
        RegistryKey::new(kind, id.parse().expect("a legal content id"))
    }

    /// A mod that claims one block and one item, and gives up on a conflict.
    struct Registering {
        id: ModId,
    }

    impl HostMod for Registering {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), HostError> {
            for kind in [ContentKind::Block, ContentKind::Item] {
                registrar
                    .claim(key(kind, "example_mod:copper"))
                    .map_err(|refused| HostError::Refused {
                        mod_id: self.id.clone(),
                        state: LifecycleState::Constructed,
                        detail: refused.to_string(),
                    })?;
            }
            Ok(())
        }
    }

    fn registering() -> Registering {
        Registering {
            id: "example_mod".parse().expect("valid id"),
        }
    }

    #[test]
    fn a_mod_that_declared_no_packet_contract_refuses_a_delivery_rather_than_ignoring_it() {
        use ravel_core::Side;
        use ravel_network::{Direction, PacketDelivery, PacketId};

        let delivery = PacketDelivery::new(
            PacketId::new("example_mod:ping".parse().expect("a legal content id")),
            Direction::ToServer,
            Side::DedicatedServer,
            Vec::new(),
        );

        let error = counting()
            .receive_packet(&delivery)
            .expect_err("the default reads no packet");
        assert_eq!(
            error,
            HostError::Unsupported {
                mod_id: "example_mod".parse().expect("valid id"),
                state: LifecycleState::Running,
            }
        );
    }

    #[test]
    fn a_mod_that_registers_nothing_needs_no_implementation() {
        let mut table = Table::default();
        let mut host = counting();

        host.register(&mut table)
            .expect("the default claims nothing");
        assert_eq!(table.claimed(), []);
    }

    #[test]
    fn one_mod_claims_the_same_name_in_two_registries_without_conflicting() {
        let mut table = Table::default();

        registering()
            .register(&mut table)
            .expect("a block and an item are different claims");
        assert_eq!(
            table.claimed(),
            [
                key(ContentKind::Block, "example_mod:copper"),
                key(ContentKind::Item, "example_mod:copper"),
            ]
        );
    }

    #[test]
    fn a_contested_identifier_reaches_the_mod_naming_who_owns_it() {
        let owner: ModId = "other_mod".parse().expect("valid id");
        let mut table = Table {
            owned: vec![(key(ContentKind::Block, "example_mod:copper"), owner)],
            ..Table::default()
        };

        let error = registering()
            .register(&mut table)
            .expect_err("the block is taken");
        assert_eq!(error.mod_id().as_str(), "example_mod");
        assert_eq!(error.state(), LifecycleState::Constructed);
        assert!(error.to_string().contains("other_mod"), "{error}");

        assert_eq!(table.claimed(), []);
    }

    #[test]
    fn a_claim_after_the_barrier_is_refused_as_late_rather_than_as_contested() {
        let mut table = Table {
            closed: true,
            ..Table::default()
        };
        let contested = key(ContentKind::Block, "example_mod:copper");

        let refused = table
            .claim(contested.clone())
            .expect_err("the barrier shut");
        assert_eq!(refused, RegistrationRefused::Closed { key: contested });
        assert!(
            refused
                .to_string()
                .contains("after the registration barrier")
        );
    }

    #[test]
    fn only_a_contested_claim_reports_a_dependency_graph_outcome() {
        let contested = key(ContentKind::Recipe, "example_mod:smelting/copper");
        let conflict = RegistrationRefused::Conflict {
            key: contested.clone(),
            owner: "other_mod".parse().expect("valid id"),
        };
        let closed = RegistrationRefused::Closed {
            key: contested.clone(),
        };

        assert_eq!(
            conflict.failure_kind(),
            Some(FailureKind::ConflictingRegistryClaim)
        );
        assert_eq!(closed.failure_kind(), None);
        assert_eq!(conflict.key(), &contested);
        assert_eq!(closed.key(), &contested);
        assert_eq!(conflict.lifecycle_state(), LifecycleState::Constructed);

        assert!(!conflict.quotes_untrusted_text());
        assert!(!closed.quotes_untrusted_text());
        assert_eq!(conflict.diagnostic().redaction(), Redaction::Raw);
        assert_ne!(conflict, closed);
    }

    struct Counting {
        id: ModId,
        entered: Vec<LifecycleState>,
    }

    impl HostMod for Counting {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, state: LifecycleState) -> Result<(), HostError> {
            if state == LifecycleState::Registered {
                return Err(HostError::Refused {
                    mod_id: self.id.clone(),
                    state,
                    detail: "the registry barrier had already closed".to_owned(),
                });
            }
            self.entered.push(state);
            Ok(())
        }
    }

    fn counting() -> Counting {
        Counting {
            id: "example_mod".parse().expect("valid id"),
            entered: Vec::new(),
        }
    }

    #[test]
    fn the_loader_drives_a_mod_through_states_and_stops_at_the_first_refusal() {
        let mut host: Box<dyn HostMod> = Box::new(counting());

        let id: ModId = "example_mod".parse().expect("valid id");

        assert_eq!(host.id().as_str(), "example_mod");
        assert_eq!(host.enter(LifecycleState::Loaded), Ok(()));
        assert_eq!(
            host.enter(LifecycleState::Registered),
            Err(HostError::Refused {
                mod_id: id,
                state: LifecycleState::Registered,
                detail: "the registry barrier had already closed".to_owned(),
            })
        );
    }

    #[test]
    fn every_refusal_names_the_mod_and_the_phase_it_happened_in() {
        let id: ModId = "example_mod".parse().expect("valid id");
        let refused = HostError::Refused {
            mod_id: id.clone(),
            state: LifecycleState::Constructed,
            detail: "no service was declared".to_owned(),
        };
        let unsupported = HostError::Unsupported {
            mod_id: id.clone(),
            state: LifecycleState::Stopping,
        };

        assert!(refused.to_string().contains("no service was declared"));
        assert!(refused.to_string().contains("example_mod"));
        assert!(unsupported.to_string().contains("does not implement"));
        assert_eq!(refused.mod_id(), &id);
        assert_eq!(unsupported.state(), LifecycleState::Stopping);
        assert_ne!(refused, unsupported);
    }

    #[test]
    fn a_refusal_reports_the_phase_it_reached_and_no_dependency_kind() {
        let refused = HostError::Refused {
            mod_id: "example_mod".parse().expect("valid id"),
            state: LifecycleState::Constructed,
            detail: "no service was declared".to_owned(),
        };

        assert_eq!(refused.failure_kind(), None);
        assert_eq!(refused.lifecycle_state(), LifecycleState::Constructed);
        assert!(refused.diagnostic().text().contains("constructed"));
    }

    #[test]
    fn only_the_message_a_mod_wrote_is_reported_as_quoting_untrusted_text() {
        let refused = HostError::Refused {
            mod_id: "example_mod".parse().expect("valid id"),
            state: LifecycleState::Loaded,
            detail: "could not open the world directory".to_owned(),
        };
        let unsupported = HostError::Unsupported {
            mod_id: "example_mod".parse().expect("valid id"),
            state: LifecycleState::Loaded,
        };

        assert!(refused.quotes_untrusted_text());
        assert!(!unsupported.quotes_untrusted_text());

        assert_eq!(refused.diagnostic().redaction(), Redaction::Raw);
        assert_eq!(unsupported.diagnostic().redaction(), Redaction::Raw);
        assert_eq!(unsupported.diagnostic().text(), unsupported.to_string());
    }
}
