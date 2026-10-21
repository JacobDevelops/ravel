#![forbid(unsafe_code)]

//! Host contract for statically composed Rust mods.

mod interception;
mod provision;
mod services;

pub use interception::{InterceptionDeclaration, Occasion, Outcome, Point};
pub use provision::Provided;
pub use services::{
    ServiceCallFailed, ServiceDeclaration, ServiceDeclarationError, ServiceDirectory, ServiceId,
    ServiceRequest, ServiceResponse, ServiceUnavailable,
};

use std::borrow::Cow;
use std::fmt;

use ravel_config::{ConfigurationDeclaration, LayeredConfiguration};
use ravel_core::{
    ContentKind, FailureKind, LifecycleState, ModId, RegistryKey, Side, StructuredFailure,
};
use ravel_data::{ContentBehaviour, ContentDeclaration, PlayRequest};
use ravel_network::PacketDelivery;
use ravel_save::{MigrationChain, SavedDataDeclaration, SavedDataEnvelope};

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

    /// Resolves a published canonical event name.
    #[must_use]
    pub fn published(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|event| event.as_str() == name)
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
    /// The runtime's world side: [`Side::IntegratedServer`] or [`Side::DedicatedServer`].
    /// The loader refuses other sides before invoking [`Mod::run_command`].
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

/// A mod's refusal, before the host attributes it to the hook it called.
///
/// Mods supply a refusal or decoding error; identity and phase are host-owned context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModError {
    /// The mod ran and declined, naming why.
    Refused {
        /// The mod's own words, not yet redacted.
        detail: String,
    },
    /// Untrusted packet delivery does not satisfy the receiving contract.
    InvalidPacket {
        /// The structured delivery refusal, not a failure of the receiving mod.
        source: Box<ravel_network::DeliveryRefused>,
    },
    /// The mod does not implement the hook.
    Unsupported,
}

impl ModError {
    /// Refuses a hook with a mod-authored explanation.
    #[must_use]
    pub fn refused(detail: impl Into<String>) -> Self {
        Self::Refused {
            detail: detail.into(),
        }
    }

    /// Refuses a hook with an error's display text, without host attribution.
    #[must_use]
    pub fn from_display(error: impl fmt::Display) -> Self {
        Self::refused(error.to_string())
    }
}

impl From<ravel_network::DeliveryRefused> for ModError {
    fn from(error: ravel_network::DeliveryRefused) -> Self {
        Self::InvalidPacket {
            source: Box::new(error),
        }
    }
}

impl fmt::Display for ModError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused { detail } => formatter.write_str(detail),
            Self::InvalidPacket { source } => write!(formatter, "invalid packet: {source}"),
            Self::Unsupported => formatter.write_str("this hook is not implemented"),
        }
    }
}

impl std::error::Error for ModError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidPacket { source } => Some(source.as_ref()),
            Self::Refused { .. } | Self::Unsupported => None,
        }
    }
}

/// A host-hook failure, attributed to its owning mod and current lifecycle phase.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostError {
    /// A provider failed before this operation; retain its original diagnostic context.
    PreviouslyFailed {
        /// The provider whose prior failure prevents the operation.
        mod_id: ModId,
        /// The recorded failure, including its original phase and redaction provenance.
        attribution: Box<ravel_core::FailureAttribution>,
    },
    /// The mod declined a lifecycle transition.
    Refused {
        /// The mod that declined.
        mod_id: ModId,
        /// The lifecycle phase the refused operation belongs to.
        state: LifecycleState,
        /// What the mod said, in the mod's own words and so not yet redacted.
        detail: String,
    },
    /// A non-transition operation was refused.
    OperationRefused {
        /// The mod responsible for the operation.
        mod_id: ModId,
        /// The phase during the refused operation.
        state: LifecycleState,
        /// The refusal detail, not yet redacted.
        detail: String,
    },
    /// The receiving mod rejected untrusted packet delivery.
    InvalidPacket {
        /// The mod whose packet contract rejected the delivery.
        mod_id: ModId,
        /// The owner's phase while checking the delivery.
        state: LifecycleState,
        /// The structured delivery refusal.
        source: Box<ravel_network::DeliveryRefused>,
    },
    /// The mod does not implement the step at all.
    Unsupported {
        /// The mod that has no implementation for the step.
        mod_id: ModId,
        /// The lifecycle phase: the requested state for a transition, otherwise
        /// the owner's current state.
        state: LifecycleState,
    },
    /// The callback unwound into the loader's panic boundary.
    Panicked {
        /// The mod whose callback panicked.
        mod_id: ModId,
        /// The callback's lifecycle phase: the requested state for a transition,
        /// otherwise the owner's current state.
        state: LifecycleState,
    },
}

impl HostError {
    /// Attributes a lifecycle transition refusal to the mod and requested phase.
    #[must_use]
    pub fn from_refusal(mod_id: ModId, state: LifecycleState, refusal: ModError) -> Self {
        match refusal {
            ModError::Refused { detail } => Self::Refused {
                mod_id,
                state,
                detail,
            },
            ModError::InvalidPacket { source } => Self::InvalidPacket {
                mod_id,
                state,
                source,
            },
            ModError::Unsupported => Self::Unsupported { mod_id, state },
        }
    }

    /// Attributes a refusal outside a lifecycle transition.
    #[must_use]
    pub fn from_operation_refusal(mod_id: ModId, state: LifecycleState, refusal: ModError) -> Self {
        match refusal {
            ModError::Refused { detail } => Self::OperationRefused {
                mod_id,
                state,
                detail,
            },
            refusal @ (ModError::InvalidPacket { .. } | ModError::Unsupported) => {
                Self::from_refusal(mod_id, state, refusal)
            }
        }
    }

    /// The mod this failure is attributed to.
    #[must_use]
    pub const fn mod_id(&self) -> &ModId {
        match self {
            Self::PreviouslyFailed { mod_id, .. }
            | Self::Refused { mod_id, .. }
            | Self::OperationRefused { mod_id, .. }
            | Self::InvalidPacket { mod_id, .. }
            | Self::Unsupported { mod_id, .. }
            | Self::Panicked { mod_id, .. } => mod_id,
        }
    }

    /// Reattributes a forwarded callback failure without changing its kind or phase.
    #[must_use]
    pub fn with_owner(mut self, owner: ModId) -> Self {
        match &mut self {
            Self::PreviouslyFailed {
                mod_id,
                attribution,
            } => {
                attribution.mod_id = Some(owner.clone());
                *mod_id = owner;
            }
            Self::Refused { mod_id, .. }
            | Self::OperationRefused { mod_id, .. }
            | Self::InvalidPacket { mod_id, .. }
            | Self::Unsupported { mod_id, .. }
            | Self::Panicked { mod_id, .. } => *mod_id = owner,
        }
        self
    }

    /// The lifecycle phase the failed operation belongs to.
    #[must_use]
    pub const fn state(&self) -> LifecycleState {
        match self {
            Self::PreviouslyFailed { attribution, .. } => attribution.state,
            Self::Refused { state, .. }
            | Self::OperationRefused { state, .. }
            | Self::InvalidPacket { state, .. }
            | Self::Unsupported { state, .. }
            | Self::Panicked { state, .. } => *state,
        }
    }
}

impl fmt::Display for HostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PreviouslyFailed { attribution, .. } => {
                formatter.write_str(attribution.message.text())
            }
            Self::Refused {
                mod_id,
                state,
                detail,
            } => write!(
                formatter,
                "`{mod_id}` refused to enter `{}`: {detail}",
                state.as_str()
            ),
            Self::OperationRefused {
                mod_id,
                state,
                detail,
            } => write!(
                formatter,
                "`{mod_id}` refused while `{}`: {detail}",
                state.as_str()
            ),
            Self::InvalidPacket {
                mod_id,
                state,
                source,
            } => write!(
                formatter,
                "`{mod_id}` rejected a packet while `{}`: {source}",
                state.as_str()
            ),
            Self::Unsupported { mod_id, state } => write!(
                formatter,
                "`{mod_id}` does not support this step while `{}`",
                state.as_str()
            ),
            Self::Panicked { mod_id, state } => {
                write!(formatter, "`{mod_id}` panicked during `{}`", state.as_str())
            }
        }
    }
}

impl std::error::Error for HostError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidPacket { source, .. } => Some(source.as_ref()),
            Self::PreviouslyFailed { .. }
            | Self::Refused { .. }
            | Self::OperationRefused { .. }
            | Self::Unsupported { .. }
            | Self::Panicked { .. } => None,
        }
    }
}

impl StructuredFailure for HostError {
    fn failure_kind(&self) -> Option<FailureKind> {
        match self {
            Self::PreviouslyFailed { attribution, .. } => attribution.kind,
            Self::Refused { .. }
            | Self::OperationRefused { .. }
            | Self::InvalidPacket { .. }
            | Self::Unsupported { .. }
            | Self::Panicked { .. } => None,
        }
    }

    fn lifecycle_state(&self) -> LifecycleState {
        self.state()
    }

    fn diagnostic(&self) -> ravel_core::DiagnosticMessage {
        match self {
            Self::PreviouslyFailed { attribution, .. } => attribution.message.clone(),
            Self::Refused { .. }
            | Self::OperationRefused { .. }
            | Self::InvalidPacket { .. }
            | Self::Unsupported { .. }
            | Self::Panicked { .. } => ravel_core::DiagnosticMessage::raw(self.to_string()),
        }
    }

    /// Mod refusal details and decoded packet fields may contain untrusted text.
    fn quotes_untrusted_text(&self) -> bool {
        match self {
            Self::PreviouslyFailed { attribution, .. } => !attribution.message.is_publishable(),
            Self::Refused { .. } | Self::OperationRefused { .. } | Self::InvalidPacket { .. } => {
                true
            }
            Self::Unsupported { .. } | Self::Panicked { .. } => false,
        }
    }
}

/// The safe Rust contract the loader drives for a statically composed mod.
pub trait Mod: Send {
    /// The canonical identity this mod was validated under.
    ///
    /// This identity must remain unchanged for the lifetime of the mod.
    fn id(&self) -> &ModId;

    /// Takes the mod into `state`.
    ///
    /// The loader calls this once per state, in lifecycle order, and stops at
    /// the first refusal.
    ///
    /// # Errors
    ///
    /// Returns an error when the mod declines the step or does not implement it.
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError>;

    /// Claims identifiers while constructed, before the registration barrier closes.
    ///
    /// The default claims every entry from [`Self::content_declaration`] in declaration
    /// order, stopping at the first refusal. An absent declaration claims nothing.
    /// An override replaces this default; use [`claim_all`] to keep claiming the
    /// mod's own declaration before additional registration work.
    ///
    /// # Errors
    ///
    /// Returns the original declaration refusal or the first claim's display text.
    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
        match self.content_declaration()? {
            Some(Cow::Borrowed(declaration)) => claim_all(declaration, registrar),
            Some(Cow::Owned(declaration)) => claim_entries(declaration.into_entries(), registrar),
            None => Ok(()),
        }
    }

    /// Observes an event in resolved load order while running; defaults to ignoring it.
    ///
    /// # Errors
    ///
    /// Returns a mod's failure and stops the broadcast before later mods are called.
    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        let _ = event;
        Ok(())
    }

    /// Runs a command owned by this mod.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Unsupported`] by default, or an implementation's refusal.
    /// Explicit refusals decline the invocation; unsupported commands and panics fail the owner.
    fn run_command(&mut self, invocation: &CommandInvocation) -> Result<(), ModError> {
        let _ = invocation;
        Err(ModError::Unsupported)
    }

    /// Receives a packet routed by contract ownership, including its arrival side.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Unsupported`] by default, or an implementation's decoding or handling refusal.
    /// Decoding, unsupported-contract, and explicit refusals decline delivery; panics fail the owner.
    fn receive_packet(&mut self, delivery: &PacketDelivery) -> Result<(), ModError> {
        let _ = delivery;
        Err(ModError::Unsupported)
    }

    /// Declares content and resources layered in resolved load order; defaults to none.
    ///
    /// Borrow cached declarations to avoid copying their content and resource bytes;
    /// return an owned declaration when constructing one for the request.
    ///
    /// # Errors
    /// Returns the mod's declaration refusal.
    fn content_declaration(&self) -> Result<Option<Cow<'_, ContentDeclaration>>, ModError> {
        Ok(None)
    }

    /// Declares the versioned world data this mod owns, when it owns any.
    ///
    /// # Errors
    /// Returns the mod's declaration refusal.
    fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, ModError> {
        Ok(None)
    }

    /// Supplies migrations checked for completeness before a world payload is read.
    ///
    /// # Errors
    /// Returns the mod's migration refusal.
    fn saved_data_migrations(&self) -> Result<Option<MigrationChain>, ModError> {
        Ok(None)
    }

    /// Applies saved data already migrated by the loader to the current declared version.
    ///
    /// Mods deserialize the current payload; the loader has already applied migrations.
    /// The default accepts only absence for a mod with no saved-data declaration.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Unsupported`] by default for other inputs, or a mod's refusal.
    /// A required mod's failure withholds the world; an optional mod's failure fails
    /// that mod and keeps its stored envelope unread. After world mutation, any mod
    /// failure halts further mod execution and withholds further worlds; it cannot
    /// retroactively withhold the world already opened.
    fn open_saved_data(&mut self, envelope: Option<&SavedDataEnvelope>) -> Result<(), ModError> {
        if envelope.is_none() && self.saved_data_declaration()?.is_none() {
            return Ok(());
        }
        Err(ModError::Unsupported)
    }

    /// Produces a complete checkpoint envelope; defaults to none only without a declaration.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Unsupported`] by default when saved data is declared,
    /// or a mod's refusal. Saves run after world mutation, so any failure
    /// refuses this Ravel write (the previous durable aggregate is kept and the
    /// game's own save continues), halts every mod, and refuses later saves for
    /// the session.
    fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, ModError> {
        if self.saved_data_declaration()?.is_none() {
            return Ok(None);
        }
        Err(ModError::Unsupported)
    }

    /// Drains sound requests for later host execution, without delivery confirmation.
    fn take_play_requests(&mut self) -> Vec<PlayRequest> {
        Vec::new()
    }

    /// Declares settings layered by the host and returned through [`Mod::configure`].
    ///
    /// # Errors
    /// Returns the mod's declaration refusal.
    fn configuration_declaration(&self) -> Result<Option<ConfigurationDeclaration>, ModError> {
        Ok(None)
    }

    /// Declares interception points for host ordering and admission; defaults to none.
    ///
    /// # Errors
    /// Returns the mod's declaration refusal.
    fn interception_declaration(&self) -> Result<Option<InterceptionDeclaration>, ModError> {
        Ok(None)
    }

    /// Answers an interception; [`Outcome::Refuse`] ends the round before later mods.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Unsupported`] by default if interceptions were declared; otherwise proceeds.
    fn intercept(&mut self, occasion: Occasion) -> Result<Outcome, ModError> {
        let _ = occasion;
        if self.interception_declaration()?.is_none() {
            return Ok(Outcome::Proceed);
        }
        Err(ModError::Unsupported)
    }

    /// Applies validated, layered settings for this launch.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Unsupported`] by default if settings were declared, or an implementation's refusal.
    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
        let _ = resolved;
        if self.configuration_declaration()?.is_none() {
            return Ok(());
        }
        Err(ModError::Unsupported)
    }

    /// Declares the services this mod answers and the ones it needs answered.
    ///
    /// # Errors
    /// Returns the mod's declaration refusal.
    fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
        Ok(None)
    }

    /// Answers one call to a service this mod declared it provides.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Unsupported`] by default, or an implementation's refusal.
    /// Returning an error fails the providing mod. Refuse malformed caller input
    /// inside a normal [`ServiceResponse`] rather than returning [`ModError`].
    fn answer_service(&mut self, request: &ServiceRequest) -> Result<ServiceResponse, ModError> {
        let _ = request;
        Err(ModError::Unsupported)
    }

    /// Uses declared services through already-started providers; defaults to no calls.
    ///
    /// # Errors
    ///
    /// Returns a mod's refusal, preventing it from starting.
    fn use_services(&mut self, directory: &mut dyn ServiceDirectory) -> Result<(), ModError> {
        let _ = directory;
        Ok(())
    }
}

/// Claims every entry a declaration holds, in declaration order.
///
/// Stops at the first refusal; use [`Registrar::claim`] directly to handle
/// individual conflicts.
///
/// An override replaces the default [`Mod::register`]. Use this helper to
/// claim the mod's own declaration before additional registration work, or to
/// claim other declarations.
///
/// # Errors
///
/// Returns the claim refusal detail; the host attributes the registration hook.
pub fn claim_all(
    declaration: &ContentDeclaration,
    registrar: &mut dyn Registrar,
) -> Result<(), ModError> {
    claim_entries(declaration.entries().iter().cloned(), registrar)
}

fn claim_entries(
    entries: impl IntoIterator<Item = RegistryKey>,
    registrar: &mut dyn Registrar,
) -> Result<(), ModError> {
    for key in entries {
        registrar.claim(key).map_err(ModError::from_display)?;
    }
    Ok(())
}

/// The registration barrier exposed to a mod without a loader dependency.
pub trait Registrar {
    /// Claims `key` for the mod currently registering.
    ///
    /// # Errors
    ///
    /// Returns [`RegistrationRefused`] for an owned identifier, a foreign namespace, or a closed barrier.
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

    /// Returns whether `mod_id` is in this set, loads on this side, and has not failed so far.
    /// This is provisional: the mod may still fail before running.
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
    /// The mod tried to declare an identifier in another mod's namespace.
    ForeignDeclaration {
        /// The foreign identifier.
        key: RegistryKey,
        /// The mod that attempted to declare it.
        mod_id: ModId,
    },
    /// The behaviour describes a different content kind than the key.
    MismatchedBehaviour {
        /// The identifier being described.
        key: RegistryKey,
        /// The kind the behaviour describes.
        described: ContentKind,
    },
}

impl RegistrationRefused {
    /// The identifier the refusal is about.
    #[must_use]
    pub const fn key(&self) -> &RegistryKey {
        match self {
            Self::Conflict { key, .. }
            | Self::Closed { key }
            | Self::MismatchedBehaviour { key, .. }
            | Self::ForeignDeclaration { key, .. } => key,
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
            Self::ForeignDeclaration { key, mod_id } => write!(
                formatter,
                "`{mod_id}` cannot register `{key}` in another mod's namespace"
            ),
            Self::MismatchedBehaviour { key, described } => write!(
                formatter,
                "`{key}` cannot use behaviour describing `{}`",
                described.as_str()
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
            Self::Closed { .. }
            | Self::ForeignDeclaration { .. }
            | Self::MismatchedBehaviour { .. } => None,
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
    use std::borrow::Cow;

    use ravel_core::{
        ContentKind, FailureKind, LifecycleState, ModId, Redaction, RegistryKey, Side,
        StructuredFailure,
    };

    use super::{HostError, Mod, ModError, Registrar, RegistrationRefused};
    use ravel_data::ContentDeclaration;

    #[test]
    fn published_events_use_only_the_canonical_vocabulary() {
        for event in super::GameEvent::ALL {
            assert_eq!(super::GameEvent::published(event.as_str()), Some(event));
        }
        for name in ["resource-reloaded", "atlas-reloaded", "", "CLIENT-TICK"] {
            assert_eq!(super::GameEvent::published(name), None);
        }
    }

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
        declaration: Result<Option<ContentDeclaration>, ModError>,
    }

    impl Mod for Registering {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn content_declaration(&self) -> Result<Option<Cow<'_, ContentDeclaration>>, ModError> {
            self.declaration
                .as_ref()
                .map(|declaration| declaration.as_ref().map(Cow::Borrowed))
                .map_err(Clone::clone)
        }
    }

    fn registering() -> Registering {
        let id: ModId = "example_mod".parse().expect("valid id");
        let mut declaration = ContentDeclaration::new(id.clone());
        for kind in [ContentKind::Block, ContentKind::Item] {
            declaration.entry(kind, "copper").expect("valid entry");
        }
        Registering {
            id,
            declaration: Ok(Some(declaration)),
        }
    }

    #[test]
    fn default_writer_refuses_declared_data_and_preserves_declaration_errors() {
        struct Saving {
            id: ModId,
            declaration: Result<Option<ravel_save::SavedDataDeclaration>, ModError>,
        }

        impl Mod for Saving {
            fn id(&self) -> &ModId {
                &self.id
            }

            fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
                Ok(())
            }

            fn saved_data_declaration(
                &self,
            ) -> Result<Option<ravel_save::SavedDataDeclaration>, ModError> {
                self.declaration.clone()
            }
        }

        let mut saving = Saving {
            id: "saving".parse().expect("valid owner"),
            declaration: Ok(None),
        };
        assert_eq!(saving.save_saved_data(), Ok(None));
        saving.declaration = Ok(Some(
            ravel_save::SavedDataDeclaration::new(
                saving.id.clone(),
                [std::num::NonZeroU32::new(1).expect("nonzero version")],
            )
            .expect("one declared version"),
        ));
        assert_eq!(saving.save_saved_data(), Err(ModError::Unsupported));
        saving.declaration = Err(ModError::refused("declaration failed"));
        assert_eq!(
            saving.save_saved_data(),
            Err(ModError::refused("declaration failed"))
        );
    }

    #[test]
    fn malformed_packets_retain_typed_source_and_host_attribution() {
        let source = ravel_network::DeliveryRefused::Malformed {
            id: ravel_network::PacketId::new("example_mod:ping".parse().expect("valid id")),
            source: ravel_network::WireError::NotABool { byte: 3 },
        };
        let refusal = ModError::InvalidPacket {
            source: Box::new(source.clone()),
        };
        assert_eq!(refusal.to_string(), format!("invalid packet: {source}"));
        assert_eq!(
            std::error::Error::source(&refusal)
                .and_then(|error| error.downcast_ref::<ravel_network::DeliveryRefused>()),
            Some(&source)
        );
        let owner = "example_mod".parse().expect("valid owner");
        let failure = HostError::from_operation_refusal(owner, LifecycleState::Running, refusal);
        assert!(failure.quotes_untrusted_text());
        assert_eq!(failure.state(), LifecycleState::Running);
        assert_eq!(failure.mod_id().as_str(), "example_mod");
        let delivery = std::error::Error::source(&failure).expect("typed delivery source");
        assert_eq!(
            delivery.downcast_ref::<ravel_network::DeliveryRefused>(),
            Some(&source)
        );
        assert_eq!(
            delivery
                .source()
                .and_then(|error| error.downcast_ref::<ravel_network::WireError>()),
            Some(&ravel_network::WireError::NotABool { byte: 3 })
        );
        assert!(matches!(
            failure,
            HostError::InvalidPacket { source: actual, .. } if actual.as_ref() == &source
        ));
    }

    #[test]
    fn packet_conversion_keeps_every_peer_selected_contract_refusal_nonfatal() {
        use ravel_network::{DeliveryRefused, Direction, PacketId, WireError};

        let expected = PacketId::new("example_mod:ping".parse().expect("valid id"));
        let found = PacketId::new("example_mod:other".parse().expect("valid id"));
        assert_eq!(
            ModError::from(DeliveryRefused::Malformed {
                id: expected.clone(),
                source: WireError::NotUtf8,
            }),
            ModError::InvalidPacket {
                source: Box::new(DeliveryRefused::Malformed {
                    id: expected.clone(),
                    source: WireError::NotUtf8,
                })
            }
        );
        assert_eq!(
            ModError::from(DeliveryRefused::WrongContract {
                expected: expected.clone(),
                found: found.clone(),
            }),
            ModError::InvalidPacket {
                source: Box::new(DeliveryRefused::WrongContract {
                    expected: expected.clone(),
                    found,
                }),
            }
        );
        for error in [
            DeliveryRefused::WrongSide {
                id: expected.clone(),
                direction: Direction::ToServer,
                side: Side::Client,
            },
            DeliveryRefused::WrongDirection {
                id: expected,
                expected: Direction::ToServer,
                found: Direction::ToClient,
            },
        ] {
            assert_eq!(
                ModError::from(error.clone()),
                ModError::InvalidPacket {
                    source: Box::new(error),
                }
            );
        }
    }

    #[test]
    fn default_registration_preserves_declaration_refusal_before_claiming() {
        let original = ModError::refused("declaration is unavailable");
        let mut host = registering();
        host.declaration = Err(original.clone());
        let mut table = Table::default();
        assert_eq!(host.register(&mut table), Err(original));
        assert_eq!(table.claimed(), []);
    }

    #[test]
    fn printable_refusals_preserve_display_text_without_debug_formatting() {
        let error = RegistrationRefused::Closed {
            key: key(ContentKind::Item, "example_mod:copper"),
        };
        let result: Result<(), RegistrationRefused> = Err(error.clone());
        assert_eq!(
            result.map_err(ModError::from_display),
            Err(ModError::Refused {
                detail: error.to_string(),
            })
        );
    }

    #[test]
    fn mods_refuse_with_detail_and_hosts_supply_the_context() {
        let refusal = ModError::refused("not today");
        assert_eq!(refusal.to_string(), "not today");
        let owner: ModId = "actual".parse().expect("valid id");
        let error =
            HostError::from_operation_refusal(owner.clone(), LifecycleState::Running, refusal);
        assert_eq!(error.mod_id(), &owner);
        assert_eq!(error.state(), LifecycleState::Running);
        assert!(error.quotes_untrusted_text());
    }

    #[test]
    fn unsupported_mod_hooks_carry_no_invented_identity_or_phase() {
        let owner: ModId = "actual".parse().expect("valid id");
        let error = HostError::from_refusal(
            owner.clone(),
            LifecycleState::Running,
            ModError::Unsupported,
        );
        assert_eq!(error.mod_id(), &owner);
        assert_eq!(error.state(), LifecycleState::Running);
        assert!(!error.quotes_untrusted_text());
    }

    #[test]
    fn refusal_construction_and_host_attribution_preserve_the_detail() {
        let actual: ModId = "actual".parse().expect("valid id");
        let error = HostError::from_operation_refusal(
            actual.clone(),
            LifecycleState::Running,
            ModError::refused("not today"),
        );
        assert_eq!(error.mod_id(), &actual);
        assert_eq!(error.state(), LifecycleState::Running);
        assert_eq!(
            error.to_string(),
            "`actual` refused while `running`: not today"
        );
    }

    #[test]
    fn previously_failed_preserves_diagnostic_context_and_reattributes_only_owner() {
        let owner: ModId = "provider".parse().expect("valid id");
        let mut attribution = ravel_core::FailureAttribution {
            mod_id: Some(owner.clone()),
            version: Some("1.0.0".parse().expect("valid version")),
            dependency_chain: vec![owner.clone()],
            state: LifecycleState::Constructed,
            kind: Some(FailureKind::MissingRequiredDependency),
            surface: None,
            environment: ravel_core::FailureEnvironment {
                ravel: "0.1.0".parse().expect("valid version"),
                minecraft: "test".to_owned(),
                platform: "linux".to_owned(),
                architecture: "x64".to_owned(),
            },
            message: ravel_core::DiagnosticMessage::redacted("original failure"),
        };
        for raw in [false, true] {
            if raw {
                attribution.message = ravel_core::DiagnosticMessage::raw("original failure");
            }
            let failure = HostError::PreviouslyFailed {
                mod_id: owner.clone(),
                attribution: Box::new(attribution.clone()),
            };
            assert_eq!(failure.mod_id(), &owner);
            assert_eq!(failure.state(), attribution.state);
            assert_eq!(failure.failure_kind(), attribution.kind);
            assert_eq!(failure.to_string(), attribution.message.text());
            assert_eq!(failure.diagnostic(), attribution.message);
            assert_eq!(failure.quotes_untrusted_text(), raw);
            let actual: ModId = "actual".parse().expect("valid id");
            let normalized = failure.with_owner(actual.clone());
            let HostError::PreviouslyFailed {
                mod_id,
                attribution: prior,
            } = normalized
            else {
                panic!("reattribution preserves failure variant");
            };
            let mut expected = attribution.clone();
            expected.mod_id = Some(actual.clone());
            assert_eq!(mod_id, actual);
            assert_eq!(*prior, expected);
        }
    }

    #[test]
    fn operation_refusals_do_not_describe_a_state_transition() {
        let owner: ModId = "actual".parse().expect("valid id");
        let error = HostError::from_operation_refusal(
            owner.clone(),
            LifecycleState::Running,
            ModError::refused("command declined"),
        );
        assert_eq!(error.mod_id(), &owner);
        assert_eq!(error.state(), LifecycleState::Running);
        assert!(error.quotes_untrusted_text());
        assert_eq!(
            error.to_string(),
            "`actual` refused while `running`: command declined"
        );
        assert!(matches!(error, HostError::OperationRefused { .. }));
    }

    #[test]
    fn unsupported_steps_receive_the_actual_hook_context() {
        let actual: ModId = "actual".parse().expect("valid id");
        let error = HostError::from_operation_refusal(
            actual.clone(),
            LifecycleState::Running,
            ModError::Unsupported,
        );
        assert_eq!(error.mod_id(), &actual);
        assert_eq!(error.state(), LifecycleState::Running);
        assert_eq!(
            error.to_string(),
            "`actual` does not support this step while `running`"
        );
    }

    #[test]
    fn foreign_registration_names_the_claimant_and_validated_key() {
        let key = key(ContentKind::Block, "other_mod:copper");
        let error = RegistrationRefused::ForeignDeclaration {
            key: key.clone(),
            mod_id: "example_mod".parse().expect("valid id"),
        };
        assert_eq!(error.key(), &key);
        assert_eq!(error.failure_kind(), None);
        assert!(!error.quotes_untrusted_text());
        assert_eq!(
            error.to_string(),
            "`example_mod` cannot register `block:other_mod:copper` in another mod's namespace"
        );
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
        assert_eq!(error, ModError::Unsupported);
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
            owned: vec![(key(ContentKind::Block, "example_mod:copper"), owner.clone())],
            ..Table::default()
        };

        let error = registering()
            .register(&mut table)
            .expect_err("the block is taken");
        assert!(error.to_string().contains("other_mod"), "{error}");
        assert_eq!(
            error,
            ModError::from_display(RegistrationRefused::Conflict {
                key: key(ContentKind::Block, "example_mod:copper"),
                owner,
            })
        );

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

    impl Mod for Counting {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
            if state == LifecycleState::Registered {
                return Err(ModError::refused("the registry barrier had already closed"));
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
        let mut host: Box<dyn Mod> = Box::new(counting());

        assert_eq!(host.id().as_str(), "example_mod");
        assert_eq!(host.enter(LifecycleState::Loaded), Ok(()));
        assert_eq!(
            host.enter(LifecycleState::Registered),
            Err(ModError::refused("the registry barrier had already closed"))
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
        assert!(unsupported.to_string().contains("does not support"));
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
