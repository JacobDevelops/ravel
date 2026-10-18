mod error;

pub use error::LifecycleError;

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use ravel_core::{
    DiagnosticMessage, FailureAttribution, FailureEnvironment, FailureSurface, LifecycleState,
    ModId, RegistryKey, Side, SideSet, StructuredFailure,
};
use ravel_data::ContentBehaviour;

/// Whether a lifecycle state can run mod code.
#[must_use]
pub const fn executes_mod_code(state: LifecycleState) -> bool {
    matches!(
        state,
        LifecycleState::Loaded
            | LifecycleState::Constructed
            | LifecycleState::Registered
            | LifecycleState::Running
            | LifecycleState::Stopping
    )
}

/// Whether a state is at or past the registration barrier.
const fn has_registered(state: LifecycleState) -> bool {
    matches!(
        state,
        LifecycleState::Registered
            | LifecycleState::Running
            | LifecycleState::Stopping
            | LifecycleState::Stopped
    )
}

/// Whether the load plan can proceed without a mod.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Requirement {
    /// The set fails closed without it.
    Required,
    /// The set proceeds without it.
    Optional,
}

/// One mod of a resolved load plan, as the lifecycle needs it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleEntry {
    /// The canonical identity that owns this mod's registrations and saved data.
    pub mod_id: ModId,
    /// The sides the mod declares.
    pub sides: SideSet,
    /// Whether the set fails closed without this mod.
    pub requirement: Requirement,
    /// The nonzero saved-data versions this identity owns.
    pub saved_data_versions: BTreeSet<NonZeroU32>,
}

/// One recorded lifecycle state change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleEvent {
    /// The mod that changed state.
    pub mod_id: ModId,
    /// The state it entered.
    pub state: LifecycleState,
}

#[derive(Clone, Debug)]
struct Tracked {
    entry: LifecycleEntry,
    state: LifecycleState,
    failure: Option<FailureAttribution>,
}

/// Set-wide facts a per-mod transition is checked against.
struct TransitionContext {
    side: Side,
    halted: bool,
    unregistered: Option<ModId>,
}

/// The per-mod lifecycle progression of one resolved package set.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeSet;
/// use std::num::NonZeroU32;
///
/// use ravel_core::{LifecycleState, ModId, Side, SideSet};
/// use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement};
///
/// let entry = LifecycleEntry {
///     mod_id: "example_mod".parse()?,
///     sides: SideSet::new([Side::Client])?,
///     requirement: Requirement::Required,
///     saved_data_versions: BTreeSet::from([NonZeroU32::MIN]),
/// };
/// let mut machine = LifecycleMachine::new(Side::Client, vec![entry])?;
/// let id: ModId = "example_mod".parse()?;
/// let environment = ravel_core::FailureEnvironment {
///     ravel: "0.1.0".parse()?,
///     minecraft: "example-version".to_owned(),
///     platform: "linux".to_owned(),
///     architecture: "x86_64".to_owned(),
/// };
///
/// assert!(machine.advance(&id, LifecycleState::Running).is_err());
/// for state in [
///     LifecycleState::Validated,
///     LifecycleState::Resolved,
///     LifecycleState::Loaded,
///     LifecycleState::Constructed,
///     LifecycleState::Registered,
///     LifecycleState::Running,
/// ] {
///     machine.advance_all(state)?;
/// }
/// machine.world_access(&ravel_observability::PreWorldReport::new(environment))?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct LifecycleMachine {
    side: Side,
    entries: Vec<Tracked>,
    transcript: Vec<LifecycleEvent>,
    claims: BTreeMap<RegistryKey, ModId>,
    /// Descriptions exist only for identifiers present in `claims`.
    behaviours: BTreeMap<RegistryKey, ContentBehaviour>,
    world_mutated: bool,
    halted: Option<ModId>,
}

impl LifecycleMachine {
    /// Admits a resolved load plan, in the order the resolver produced.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::DuplicateIdentity`] for repeated mod ids.
    pub fn new(side: Side, order: Vec<LifecycleEntry>) -> Result<Self, LifecycleError> {
        let mut entries: Vec<Tracked> = Vec::with_capacity(order.len());
        for entry in order {
            if entries
                .iter()
                .any(|tracked| tracked.entry.mod_id == entry.mod_id)
            {
                return Err(LifecycleError::DuplicateIdentity {
                    mod_id: entry.mod_id,
                });
            }
            entries.push(Tracked {
                entry,
                state: LifecycleState::Discovered,
                failure: None,
            });
        }

        Ok(Self {
            side,
            entries,
            transcript: Vec::new(),
            claims: BTreeMap::new(),
            behaviours: BTreeMap::new(),
            world_mutated: false,
            halted: None,
        })
    }

    /// Returns the side this set is running on.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Returns the mod ids in resolved order.
    pub fn order(&self) -> impl Iterator<Item = &ModId> + '_ {
        self.entries.iter().map(|tracked| &tracked.entry.mod_id)
    }

    /// Returns admitted entries in resolved order.
    pub fn entries(&self) -> impl Iterator<Item = &LifecycleEntry> {
        self.entries.iter().map(|tracked| &tracked.entry)
    }

    /// Iterates admitted entries and their states in resolved order.
    pub(crate) fn tracked_entries(
        &self,
    ) -> impl Iterator<Item = (&LifecycleEntry, LifecycleState)> {
        self.entries
            .iter()
            .map(|tracked| (&tracked.entry, tracked.state))
    }

    /// Iterates eligible members without re-querying tracked state.
    pub(crate) fn eligible(&self) -> impl Iterator<Item = (&LifecycleEntry, LifecycleState)> {
        self.tracked_entries()
            .filter(|(entry, _)| self.loads(&entry.mod_id))
    }

    pub(crate) fn tracked_entries_with_failures(
        &self,
    ) -> impl Iterator<Item = (&LifecycleEntry, LifecycleState, Option<&FailureAttribution>)> {
        self.entries
            .iter()
            .map(|tracked| (&tracked.entry, tracked.state, tracked.failure.as_ref()))
    }

    /// Returns every state change so far, in the order it happened.
    #[must_use]
    pub fn transcript(&self) -> &[LifecycleEvent] {
        &self.transcript
    }

    /// Returns one mod's current state.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::UnknownMod`] when the id is not in the plan.
    pub fn state(&self, mod_id: &ModId) -> Result<LifecycleState, LifecycleError> {
        self.tracked(mod_id).map(|tracked| tracked.state)
    }

    /// Returns the attribution recorded when a mod failed.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::UnknownMod`] when the id is not in the plan.
    pub fn failure(&self, mod_id: &ModId) -> Result<Option<&FailureAttribution>, LifecycleError> {
        self.tracked(mod_id).map(|tracked| tracked.failure.as_ref())
    }

    /// Returns the saved-data versions a canonical identity owns.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::UnknownMod`] when the id is not in the plan.
    pub fn saved_data_versions(
        &self,
        mod_id: &ModId,
    ) -> Result<&BTreeSet<NonZeroU32>, LifecycleError> {
        self.tracked(mod_id)
            .map(|tracked| &tracked.entry.saved_data_versions)
    }

    /// Returns whether the registration barrier has closed.
    ///
    /// Failed and side-parked mods do not hold the barrier open.
    #[must_use]
    pub fn registration_barrier_closed(&self) -> bool {
        self.unregistered().is_none()
    }

    /// Returns the plan entry one mod was admitted under.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::UnknownMod`] for an id outside the plan.
    pub fn entry(&self, mod_id: &ModId) -> Result<&LifecycleEntry, LifecycleError> {
        Ok(&self.tracked(mod_id)?.entry)
    }

    /// Returns whether the mod loads on this side and has not failed.
    #[must_use]
    pub fn loads(&self, mod_id: &ModId) -> bool {
        self.tracked(mod_id).is_ok_and(|tracked| {
            tracked.entry.sides.loads_on(self.side) && tracked.state != LifecycleState::Failed
        })
    }

    /// Returns the identity that owns a registry identifier.
    #[must_use]
    pub fn registry_owner(&self, identifier: &RegistryKey) -> Option<&ModId> {
        self.claims.get(identifier)
    }

    /// Returns how many registry identifiers one mod owns.
    #[must_use]
    pub fn registry_claim_count(&self, mod_id: &ModId) -> usize {
        self.claims
            .values()
            .filter(|owner| *owner == mod_id)
            .count()
    }

    /// Returns every admitted registry key in deterministic registry-key order.
    pub fn claimed_keys(&self) -> impl Iterator<Item = &RegistryKey> {
        self.claims.keys()
    }

    /// Returns admitted content in registry-key order when its kind has a behaviour.
    pub fn claimed_content(&self) -> impl Iterator<Item = (RegistryKey, &ModId, ContentBehaviour)> {
        self.claims.iter().filter_map(|(key, owner)| {
            // Omitted descriptions use the kind's default, if one exists.
            let behaviour = self
                .behaviours
                .get(key)
                .copied()
                .or_else(|| ContentBehaviour::default_for(key.kind()))?;
            Some((key.clone(), owner, behaviour))
        })
    }

    /// Moves one mod to its next state.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::UnknownMod`] for an id outside the plan,
    /// [`LifecycleError::AttributionRequired`] for a bare move to
    /// [`LifecycleState::Failed`], [`LifecycleError::IllegalTransition`] for any
    /// pair the `SPEC.md` §14 table refuses,
    /// [`LifecycleError::RegistrationBarrierOpen`] for starting ahead of a mod
    /// that has not registered, [`LifecycleError::Halted`] for forward progress
    /// after a failure that followed world mutation, and
    /// [`LifecycleError::SideMismatch`] when mod code would run on a side the
    /// mod does not declare.
    pub fn advance(&mut self, mod_id: &ModId, next: LifecycleState) -> Result<(), LifecycleError> {
        if next == LifecycleState::Failed {
            return Err(LifecycleError::AttributionRequired);
        }
        let context = self.context();
        let tracked = self.tracked_mut(mod_id)?;
        Self::check(&context, tracked, next)?;

        tracked.state = next;
        let mod_id = tracked.entry.mod_id.clone();
        self.transcript.push(LifecycleEvent {
            mod_id,
            state: next,
        });
        Ok(())
    }

    /// Moves the whole set to its next state, in resolved order.
    ///
    /// Checks every active mod before committing any transition.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::AttributionRequired`] for
    /// [`LifecycleState::Failed`], which is per-mod by definition, and
    /// otherwise the same refusals as [`LifecycleMachine::advance`], naming the
    /// first mod in resolved order that cannot take the step.
    pub fn advance_all(&mut self, next: LifecycleState) -> Result<(), LifecycleError> {
        if next == LifecycleState::Failed {
            return Err(LifecycleError::AttributionRequired);
        }

        let context = self.context();
        for tracked in &self.entries {
            if !Self::is_settled(self.side, tracked) {
                Self::check(&context, tracked, next)?;
            }
        }

        for tracked in &mut self.entries {
            if !Self::is_settled(self.side, tracked) {
                tracked.state = next;
                self.transcript.push(LifecycleEvent {
                    mod_id: tracked.entry.mod_id.clone(),
                    state: next,
                });
            }
        }
        Ok(())
    }

    /// Ends one mod's lifecycle with structured attribution.
    ///
    /// A failure recorded after the world was mutated halts the set: the run
    /// cannot silently continue and cannot save, and the only remaining
    /// progression is quiescing to [`LifecycleState::Stopped`].
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::UnknownMod`] for an id outside the plan and
    /// [`LifecycleError::IllegalTransition`] when the mod has already ended.
    pub fn fail(
        &mut self,
        mod_id: &ModId,
        attribution: FailureAttribution,
    ) -> Result<(), LifecycleError> {
        let tracked = self.tracked_mut(mod_id)?;
        let from = tracked.state;
        if !from.can_transition_to(LifecycleState::Failed) {
            return Err(LifecycleError::IllegalTransition {
                mod_id: tracked.entry.mod_id.clone(),
                from,
                to: LifecycleState::Failed,
            });
        }

        tracked.state = LifecycleState::Failed;
        tracked.failure = Some(attribution);
        let mod_id = tracked.entry.mod_id.clone();
        self.transcript.push(LifecycleEvent {
            mod_id: mod_id.clone(),
            state: LifecycleState::Failed,
        });
        if self.world_mutated && self.halted.is_none() {
            self.halted = Some(mod_id);
        }
        Ok(())
    }

    /// Records a callback failure once and returns the owner's participation requirement.
    ///
    /// # Errors
    ///
    /// Returns a lifecycle refusal for an unknown owner or an ended lifecycle.
    pub fn fail_callback(
        &mut self,
        error: &ravel_host_api::HostError,
        environment: &FailureEnvironment,
        surface: Option<FailureSurface>,
    ) -> Result<Requirement, LifecycleError> {
        let mod_id = error.mod_id();
        let requirement = self.entry(mod_id)?.requirement;
        if self.state(mod_id)? != LifecycleState::Failed {
            self.fail(
                mod_id,
                FailureAttribution {
                    mod_id: Some(mod_id.clone()),
                    version: None,
                    dependency_chain: Vec::new(),
                    state: error.state(),
                    kind: error.failure_kind(),
                    surface,
                    environment: environment.clone(),
                    message: DiagnosticMessage::raw(error.to_string()),
                },
            )?;
        }
        Ok(requirement)
    }

    /// Records a mod callback failure and reports whether the caller must stop.
    pub(crate) fn fail_setup_callback(
        &mut self,
        error: &ravel_host_api::HostError,
        environment: &FailureEnvironment,
    ) -> Result<bool, LifecycleError> {
        let requirement = self.fail_callback(error, environment, None)?;
        Ok(requirement == Requirement::Required || self.halted_by().is_some())
    }

    pub(crate) fn fail_callback_attributed(
        &mut self,
        error: &ravel_host_api::HostError,
        environment: &FailureEnvironment,
        surface: Option<FailureSurface>,
    ) -> Result<(Requirement, &FailureAttribution), LifecycleError> {
        let requirement = self.fail_callback(error, environment, surface)?;
        let attribution = self
            .tracked(error.mod_id())?
            .failure
            .as_ref()
            .expect("fail_callback records attribution before returning");
        Ok((requirement, attribution))
    }

    /// Claims a registry identifier for a mod before the registration barrier.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::UnknownMod`] for an id outside the plan,
    /// [`LifecycleError::RegistrationBarrierClosed`] once the set has
    /// registered, [`LifecycleError::RegistrationWindow`] while the set is
    /// still registering but this mod is not constructed,
    /// [`LifecycleError::ForeignRegistryClaim`] for another mod's namespace,
    /// [`LifecycleError::ConflictingRegistryClaim`] when already owned, or
    /// [`LifecycleError::MismatchedContentBehaviour`] when the description does
    /// not describe the kind the identifier names.
    pub fn claim_registry_identifier(
        &mut self,
        mod_id: &ModId,
        identifier: &RegistryKey,
        behaviour: Option<ContentBehaviour>,
    ) -> Result<(), LifecycleError> {
        let closed = self.registration_barrier_closed();
        let tracked = self.tracked(mod_id)?;
        if closed {
            return Err(LifecycleError::RegistrationBarrierClosed {
                mod_id: tracked.entry.mod_id.clone(),
                identifier: identifier.clone(),
            });
        }
        if tracked.state != LifecycleState::Constructed {
            return Err(LifecycleError::RegistrationWindow {
                mod_id: tracked.entry.mod_id.clone(),
                identifier: identifier.clone(),
                state: tracked.state,
            });
        }
        if identifier.id().namespace() != mod_id {
            return Err(LifecycleError::ForeignRegistryClaim {
                key: identifier.clone(),
                mod_id: mod_id.clone(),
            });
        }
        if let Some(owner) = self.claims.get(identifier) {
            return Err(LifecycleError::ConflictingRegistryClaim {
                identifier: identifier.clone(),
                owner: owner.clone(),
                claimant: tracked.entry.mod_id.clone(),
            });
        }

        // Validate before writing either table so a refused claim owns nothing.
        if let Some(behaviour) = behaviour
            && !behaviour.describes(identifier.kind())
        {
            return Err(LifecycleError::MismatchedContentBehaviour {
                identifier: identifier.clone(),
                described: behaviour.kind(),
            });
        }

        let owner = tracked.entry.mod_id.clone();
        if let Some(behaviour) = behaviour {
            self.behaviours.insert(identifier.clone(), behaviour);
        }
        self.claims.insert(identifier.clone(), owner);
        Ok(())
    }

    /// Decides whether the set may open a world.
    ///
    /// All non-parked mods must run, except optional mods that failed.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::WorldAccessHalted`] when a failure followed
    /// world mutation, and [`LifecycleError::WorldAccessBlocked`] naming the
    /// first mod in resolved order that is not running.
    pub(crate) fn lifecycle_world_access(&self) -> Result<(), LifecycleError> {
        if let Some(mod_id) = &self.halted {
            return Err(LifecycleError::WorldAccessHalted {
                mod_id: mod_id.clone(),
            });
        }
        for tracked in &self.entries {
            if Self::is_parked(self.side, tracked) || tracked.state == LifecycleState::Running {
                continue;
            }
            if tracked.entry.requirement == Requirement::Optional
                && tracked.state == LifecycleState::Failed
            {
                continue;
            }
            return Err(LifecycleError::WorldAccessBlocked {
                mod_id: tracked.entry.mod_id.clone(),
                state: tracked.state,
            });
        }
        Ok(())
    }

    /// Records that the world has been mutated.
    ///
    /// # Errors
    ///
    /// Returns whatever [`LifecycleMachine::world_access`] refuses: a set that
    /// cannot open a world cannot have mutated one.
    pub fn record_world_mutation(&mut self) -> Result<(), LifecycleError> {
        self.lifecycle_world_access()?;
        self.world_mutated = true;
        Ok(())
    }

    /// Decides whether saved state may be written.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::PartialStateRefused`] when a failure followed
    /// world mutation.
    pub fn save_access(&self) -> Result<(), LifecycleError> {
        self.halted.as_ref().map_or(Ok(()), |mod_id| {
            Err(LifecycleError::PartialStateRefused {
                mod_id: mod_id.clone(),
            })
        })
    }

    pub(crate) const fn halted_by(&self) -> Option<&ModId> {
        self.halted.as_ref()
    }

    /// Snapshots set-wide facts before borrowing an individual mod.
    fn context(&self) -> TransitionContext {
        TransitionContext {
            side: self.side,
            halted: self.halted.is_some(),
            unregistered: self.unregistered().cloned(),
        }
    }

    fn unregistered(&self) -> Option<&ModId> {
        self.entries
            .iter()
            .find(|tracked| !Self::is_settled(self.side, tracked) && !has_registered(tracked.state))
            .map(|tracked| &tracked.entry.mod_id)
    }

    fn check(
        context: &TransitionContext,
        tracked: &Tracked,
        next: LifecycleState,
    ) -> Result<(), LifecycleError> {
        let from = tracked.state;
        if !from.can_transition_to(next) {
            return Err(LifecycleError::IllegalTransition {
                mod_id: tracked.entry.mod_id.clone(),
                from,
                to: next,
            });
        }
        if context.halted && !matches!(next, LifecycleState::Stopping | LifecycleState::Stopped) {
            return Err(LifecycleError::Halted {
                mod_id: tracked.entry.mod_id.clone(),
                from,
                to: next,
            });
        }
        if executes_mod_code(next) && !tracked.entry.sides.loads_on(context.side) {
            return Err(LifecycleError::SideMismatch {
                mod_id: tracked.entry.mod_id.clone(),
                from,
                side: context.side,
            });
        }
        if next == LifecycleState::Running
            && let Some(blocking) = &context.unregistered
        {
            return Err(LifecycleError::RegistrationBarrierOpen {
                mod_id: tracked.entry.mod_id.clone(),
                blocking: blocking.clone(),
            });
        }
        Ok(())
    }

    fn is_parked(side: Side, tracked: &Tracked) -> bool {
        tracked.state == LifecycleState::Resolved && !tracked.entry.sides.loads_on(side)
    }

    fn is_settled(side: Side, tracked: &Tracked) -> bool {
        tracked.state.is_terminal() || Self::is_parked(side, tracked)
    }

    fn tracked(&self, mod_id: &ModId) -> Result<&Tracked, LifecycleError> {
        self.entries
            .iter()
            .find(|tracked| tracked.entry.mod_id == *mod_id)
            .ok_or_else(|| LifecycleError::UnknownMod {
                mod_id: mod_id.clone(),
            })
    }

    fn tracked_mut(&mut self, mod_id: &ModId) -> Result<&mut Tracked, LifecycleError> {
        self.entries
            .iter_mut()
            .find(|tracked| tracked.entry.mod_id == *mod_id)
            .ok_or_else(|| LifecycleError::UnknownMod {
                mod_id: mod_id.clone(),
            })
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use ravel_core::{
        ContentKind, DiagnosticMessage, FailureAttribution, FailureEnvironment, FailureKind,
        FailureSurface, LifecycleState, ModId, Redaction, Side, StructuredFailure,
    };
    use ravel_data::{ContentBehaviour, ContentDeclaration};
    use ravel_host_api::{HostError, Mod, ModError};

    use super::{LifecycleError, LifecycleMachine, Requirement, executes_mod_code, has_registered};
    use crate::tests::entry;

    const SUCCESS_PATH: [LifecycleState; 8] = [
        LifecycleState::Validated,
        LifecycleState::Resolved,
        LifecycleState::Loaded,
        LifecycleState::Constructed,
        LifecycleState::Registered,
        LifecycleState::Running,
        LifecycleState::Stopping,
        LifecycleState::Stopped,
    ];

    fn id(name: &str) -> ModId {
        name.parse().expect("valid mod id")
    }

    fn attribution(name: &str, state: LifecycleState) -> FailureAttribution {
        FailureAttribution {
            mod_id: Some(id(name)),
            version: None,
            dependency_chain: Vec::new(),
            state,
            kind: Some(FailureKind::MissingRequiredDependency),
            surface: None,
            environment: FailureEnvironment {
                ravel: "0.1.0".parse().expect("valid version"),
                minecraft: "26.0.0".to_owned(),
                platform: "linux".to_owned(),
                architecture: "x86_64".to_owned(),
            },
            message: DiagnosticMessage::redacted("the dependency is absent"),
        }
    }

    #[test]
    fn loading_is_answered_for_every_way_a_mod_can_be_absent() {
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![
                entry("example_mod", &[Side::Client]),
                entry("server_side_mod", &[Side::DedicatedServer]),
                entry("failing_mod", &[Side::Client]),
            ],
        )
        .expect("three mods are a valid plan");

        assert!(
            machine.loads(&id("example_mod")),
            "a mod this side loads is one an integration can build on"
        );
        assert!(
            !machine.loads(&id("server_side_mod")),
            "a mod parked by its own sides declaration does not run here"
        );
        assert!(
            !machine.loads(&id("never_installed")),
            "an id outside the plan names nothing"
        );

        machine
            .fail(
                &id("failing_mod"),
                attribution("failing_mod", LifecycleState::Failed),
            )
            .expect("failure is reachable from discovered");
        assert!(
            !machine.loads(&id("failing_mod")),
            "a mod that failed is not one an integration can build on"
        );
    }

    fn machine_at(target: LifecycleState) -> LifecycleMachine {
        let mut machine =
            LifecycleMachine::new(Side::Client, vec![entry("example_mod", &[Side::Client])])
                .expect("one mod is a valid plan");
        if target == LifecycleState::Failed {
            machine
                .fail(&id("example_mod"), attribution("example_mod", target))
                .expect("failure is reachable from discovered");
            return machine;
        }
        for state in SUCCESS_PATH {
            if machine.state(&id("example_mod")).expect("known mod") == target {
                break;
            }
            machine.advance(&id("example_mod"), state).expect("legal");
        }
        assert_eq!(
            machine.state(&id("example_mod")).expect("known mod"),
            target
        );
        machine
    }

    #[test]
    fn every_ordered_pair_of_states_is_accepted_or_refused_by_the_spec_table() {
        let mut accepted = 0_usize;
        for from in LifecycleState::ALL {
            for to in LifecycleState::ALL {
                let mut machine = machine_at(from);
                let mod_id = id("example_mod");
                let result = if to == LifecycleState::Failed {
                    machine.fail(&mod_id, attribution("example_mod", from))
                } else {
                    machine.advance(&mod_id, to)
                };

                if from.can_transition_to(to) {
                    result.expect("the spec table accepts this pair");
                    assert_eq!(machine.state(&mod_id).expect("known mod"), to);
                    accepted += 1;
                } else {
                    assert_eq!(
                        result,
                        Err(LifecycleError::IllegalTransition {
                            mod_id: mod_id.clone(),
                            from,
                            to,
                        }),
                        "{from:?} -> {to:?}"
                    );
                    assert_eq!(
                        machine.state(&mod_id).expect("known mod"),
                        from,
                        "a refused transition changes nothing"
                    );
                }
            }
        }
        assert_eq!(
            accepted, 16,
            "eight successful steps and eight failure edges"
        );
    }

    #[test]
    fn a_bare_move_to_failed_is_refused_because_attribution_is_structural() {
        let mut machine = machine_at(LifecycleState::Running);
        assert_eq!(
            machine.advance(&id("example_mod"), LifecycleState::Failed),
            Err(LifecycleError::AttributionRequired)
        );
        assert_eq!(
            machine.advance_all(LifecycleState::Failed),
            Err(LifecycleError::AttributionRequired)
        );
        assert_eq!(
            machine.state(&id("example_mod")).expect("known mod"),
            LifecycleState::Running
        );
    }

    #[test]
    fn a_recorded_failure_keeps_the_attribution_it_was_given() {
        let mut machine = machine_at(LifecycleState::Resolved);
        let mod_id = id("example_mod");
        let recorded = attribution("example_mod", LifecycleState::Resolved);
        machine
            .fail(&mod_id, recorded.clone())
            .expect("failure is legal from resolved");

        assert_eq!(
            machine.failure(&mod_id).expect("known mod"),
            Some(&recorded)
        );
        assert_eq!(
            machine.failure(&id("other_mod")),
            Err(LifecycleError::UnknownMod {
                mod_id: id("other_mod")
            })
        );
    }

    #[test]
    fn callback_failure_retains_its_phase_context_and_first_attribution() {
        let owner = id("example_mod");
        let mut machine = machine_at(LifecycleState::Running);
        let error = HostError::Refused {
            mod_id: owner.clone(),
            state: LifecycleState::Running,
            detail: "callback refused".to_owned(),
        };
        let environment = attribution("example_mod", LifecycleState::Running).environment;
        let surface = FailureSurface::Hook("world_load".to_owned());
        let requirement = machine.entry(&owner).expect("known owner").requirement;
        assert_eq!(
            machine.fail_callback(&error, &environment, Some(surface.clone())),
            Ok(requirement)
        );
        let recorded = machine
            .failure(&owner)
            .expect("known owner")
            .expect("failure retained")
            .clone();
        assert_eq!(recorded.mod_id, Some(owner.clone()));
        assert_eq!(recorded.state, LifecycleState::Running);
        assert_eq!(recorded.kind, error.failure_kind());
        assert_eq!(recorded.environment, environment);
        assert_eq!(recorded.surface, Some(surface));
        let event_count = machine.transcript().len();
        let mut other_environment = environment.clone();
        other_environment.platform = "different".to_owned();
        assert_eq!(
            machine.fail_callback(&error, &other_environment, None),
            Ok(requirement)
        );
        assert_eq!(
            machine.failure(&owner).expect("known owner"),
            Some(&recorded)
        );
        assert_eq!(machine.transcript().len(), event_count);
    }

    #[test]
    fn advancing_the_whole_set_moves_nothing_when_one_mod_cannot_take_the_step() {
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![
                entry("first_mod", &[Side::Client]),
                entry("second_mod", &[Side::Client]),
            ],
        )
        .expect("two mods are a valid plan");
        machine
            .advance(&id("first_mod"), LifecycleState::Validated)
            .expect("legal");

        assert_eq!(
            machine.advance_all(LifecycleState::Validated),
            Err(LifecycleError::IllegalTransition {
                mod_id: id("first_mod"),
                from: LifecycleState::Validated,
                to: LifecycleState::Validated,
            })
        );
        assert_eq!(
            machine.state(&id("second_mod")).expect("known mod"),
            LifecycleState::Discovered,
            "a refused set transition leaves every mod alone"
        );
    }

    #[test]
    fn advancing_the_whole_set_skips_mods_that_have_already_ended() {
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![
                entry("first_mod", &[Side::Client]),
                entry("second_mod", &[Side::Client]),
            ],
        )
        .expect("two mods are a valid plan");
        machine
            .fail(
                &id("first_mod"),
                attribution("first_mod", LifecycleState::Discovered),
            )
            .expect("failure is legal from discovered");

        machine
            .advance_all(LifecycleState::Validated)
            .expect("the surviving mod advances alone");
        assert_eq!(
            machine.state(&id("first_mod")).expect("known mod"),
            LifecycleState::Failed
        );
        assert_eq!(
            machine.state(&id("second_mod")).expect("known mod"),
            LifecycleState::Validated
        );
    }

    #[test]
    fn a_registry_identifier_is_claimed_only_while_the_owner_is_constructed() {
        let mut machine = machine_at(LifecycleState::Loaded);
        let mod_id = id("example_mod");
        assert_eq!(
            machine.claim_registry_identifier(
                &mod_id,
                &"block:example_mod:block".parse().expect("valid key"),
                None,
            ),
            Err(LifecycleError::RegistrationWindow {
                mod_id: mod_id.clone(),
                identifier: "block:example_mod:block".parse().expect("valid key"),
                state: LifecycleState::Loaded,
            })
        );

        machine
            .advance(&mod_id, LifecycleState::Constructed)
            .expect("legal");
        machine
            .claim_registry_identifier(
                &mod_id,
                &"block:example_mod:block".parse().expect("valid key"),
                None,
            )
            .expect("the window is open while constructed");
        assert_eq!(
            machine.registry_owner(&"block:example_mod:block".parse().expect("valid key")),
            Some(&mod_id)
        );
        assert_eq!(
            machine.registry_owner(&"item:example_mod:item".parse().expect("valid key")),
            None
        );
    }

    #[test]
    fn every_admitted_claim_can_be_read_back_as_a_typed_key() {
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![
                entry("first_mod", &[Side::Client]),
                entry("second_mod", &[Side::Client]),
            ],
        )
        .expect("two mods are a valid plan");
        for state in [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
        ] {
            machine.advance_all(state).expect("legal");
        }

        // Claim order differs from identifier order.
        machine
            .claim_registry_identifier(
                &id("second_mod"),
                &"item:second_mod:ingot".parse().expect("valid key"),
                None,
            )
            .expect("accepted");
        machine
            .claim_registry_identifier(
                &id("first_mod"),
                &"block:first_mod:copper".parse().expect("valid key"),
                None,
            )
            .expect("accepted");

        let claimed: Vec<(String, ModId)> = machine
            .claimed_content()
            .map(|(key, owner, _)| (key.to_string(), owner.clone()))
            .collect();

        assert_eq!(
            claimed,
            vec![
                ("block:first_mod:copper".to_owned(), id("first_mod")),
                ("item:second_mod:ingot".to_owned(), id("second_mod")),
            ],
            "the order is the table's own, so what a game registers cannot \
             depend on the order mods happened to claim in"
        );

        let kinds: Vec<ContentKind> = machine
            .claimed_content()
            .map(|(key, _, _)| key.kind())
            .collect();
        assert_eq!(
            kinds,
            vec![ContentKind::Block, ContentKind::Item],
            "a claim carries which registry it belongs in, not just its name"
        );
    }

    #[test]
    fn complete_claim_inventory_includes_keys_without_content_behaviour() {
        let mut machine = machine_at(LifecycleState::Constructed);
        let expected = [
            "block:example_mod:copper",
            "item:example_mod:ingot",
            "command:example_mod:inspect",
        ]
        .map(|key| key.parse().expect("valid key"));
        for key in expected.iter().rev() {
            machine
                .claim_registry_identifier(&id("example_mod"), key, None)
                .expect("accepted claim");
        }
        assert_eq!(
            machine.claimed_keys().cloned().collect::<Vec<_>>(),
            expected
        );
        assert_eq!(machine.claimed_content().count(), 2);
    }

    #[test]
    fn a_claim_carries_what_the_mod_described_and_its_kind_default_otherwise() {
        let mut machine = machine_at(LifecycleState::Constructed);
        let mod_id = id("example_mod");
        machine
            .claim_registry_identifier(
                &mod_id,
                &"block:example_mod:obsidian".parse().expect("valid key"),
                Some(ContentBehaviour::Block(ravel_data::BlockBehaviour {
                    hardness: 50.0,
                    explosion_resistance: 1200.0,
                    requires_correct_tool: true,
                    random_ticks: false,
                })),
            )
            .expect("accepted");
        machine
            .claim_registry_identifier(
                &mod_id,
                &"item:example_mod:ingot".parse().expect("valid key"),
                None,
            )
            .expect("accepted");

        let described: Vec<ContentBehaviour> = machine
            .claimed_content()
            .map(|(_, _, behaviour)| behaviour)
            .collect();

        assert_eq!(
            described,
            vec![
                ContentBehaviour::Block(ravel_data::BlockBehaviour {
                    hardness: 50.0,
                    explosion_resistance: 1200.0,
                    requires_correct_tool: true,
                    random_ticks: false,
                }),
                ContentBehaviour::Item(ravel_data::ItemBehaviour::default()),
            ],
            "a claim that described nothing gets its kind's default rather than \
             arriving undescribed"
        );
    }

    #[test]
    fn content_described_as_a_kind_its_name_does_not_carry_is_refused_whole() {
        let mut machine = machine_at(LifecycleState::Constructed);
        let mod_id = id("example_mod");

        let error = machine
            .claim_registry_identifier(
                &mod_id,
                &"item:example_mod:copper".parse().expect("valid key"),
                Some(ContentBehaviour::Block(
                    ravel_data::BlockBehaviour::default(),
                )),
            )
            .expect_err("an item described as a block is a value no registry builds");

        assert!(
            matches!(error, LifecycleError::MismatchedContentBehaviour { .. }),
            "{error}"
        );
        assert_eq!(
            machine.registry_claim_count(&mod_id),
            0,
            "a refused description leaves no identifier owned, or the mod holds a \
             name it can neither re-claim nor use"
        );
    }

    #[test]
    fn foreign_namespace_claims_never_acquire_ownership() {
        let mut machine = machine_at(LifecycleState::Constructed);
        let foreign = "block:other_mod:copper".parse().expect("valid key");
        let mod_id = id("example_mod");
        assert_eq!(
            machine.claim_registry_identifier(&mod_id, &foreign, None),
            Err(LifecycleError::ForeignRegistryClaim {
                key: foreign.clone(),
                mod_id: mod_id.clone(),
            })
        );
        let refusal = LifecycleError::ForeignRegistryClaim {
            key: foreign.clone(),
            mod_id: mod_id.clone(),
        };
        assert_eq!(
            refusal.to_string(),
            "`example_mod` cannot claim `block:other_mod:copper` in another mod's namespace"
        );
        assert_eq!(refusal.lifecycle_state(), LifecycleState::Constructed);
        assert_eq!(refusal.failure_kind(), None);
        assert_eq!(machine.registry_owner(&foreign), None);
        assert_eq!(machine.registry_claim_count(&mod_id), 0);
        assert_eq!(machine.claimed_content().count(), 0);
    }

    #[test]
    fn default_registration_refuses_a_foreign_declaration_through_the_loader() {
        struct ForeignDeclaration {
            id: ModId,
            declaration: ContentDeclaration,
        }

        impl Mod for ForeignDeclaration {
            fn id(&self) -> &ModId {
                &self.id
            }

            fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
                assert!(matches!(
                    state,
                    LifecycleState::Loaded | LifecycleState::Constructed
                ));
                Ok(())
            }

            fn content_declaration(&self) -> Result<Option<Cow<'_, ContentDeclaration>>, ModError> {
                Ok(Some(Cow::Borrowed(&self.declaration)))
            }
        }

        let mut declaration = ContentDeclaration::new(id("other_mod"));
        declaration
            .entry(ContentKind::Block, "copper")
            .expect("valid foreign entry");
        let mut host = ForeignDeclaration {
            id: id("example_mod"),
            declaration,
        };
        let mut machine =
            LifecycleMachine::new(Side::Client, vec![entry("example_mod", &[Side::Client])])
                .expect("valid plan");
        let error = crate::drive_loaded(
            &mut crate::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                .expect("complete host set"),
            &attribution("example_mod", LifecycleState::Constructed).environment,
        )
        .expect_err("default registration must reject foreign content");
        let crate::DriveError::Host {
            source,
            attribution,
        } = error
        else {
            panic!("expected an attributed registration refusal");
        };
        assert_eq!(
            source,
            HostError::OperationRefused {
                mod_id: id("example_mod"),
                state: LifecycleState::Constructed,
                detail: "`example_mod` cannot register `block:other_mod:copper` in another mod's namespace"
                    .to_owned(),
            }
        );
        assert_eq!(attribution.mod_id, Some(id("example_mod")));
        assert_eq!(attribution.state, LifecycleState::Constructed);
        assert_eq!(
            machine.state(&id("example_mod")),
            Ok(LifecycleState::Failed)
        );
        assert_eq!(machine.claimed_content().count(), 0);
    }

    #[test]
    fn optional_mismatched_registration_fails_only_its_owner() {
        struct Mismatched {
            owner: ModId,
        }
        impl Mod for Mismatched {
            fn id(&self) -> &ModId {
                &self.owner
            }

            fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
                Ok(())
            }

            fn register(
                &mut self,
                registrar: &mut dyn ravel_host_api::Registrar,
            ) -> Result<(), ModError> {
                registrar
                    .claim_describing(
                        "item:example_mod:copper".parse().expect("valid key"),
                        ContentBehaviour::Block(ravel_data::BlockBehaviour::default()),
                    )
                    .map_err(ModError::from_display)?;
                Ok(())
            }
        }
        let mut optional = entry("example_mod", &[Side::Client]);
        optional.requirement = Requirement::Optional;
        let mut machine = LifecycleMachine::new(Side::Client, vec![optional]).expect("valid plan");
        let mut host = Mismatched {
            owner: id("example_mod"),
        };
        crate::drive_loaded(
            &mut crate::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                .expect("complete host set"),
            &attribution("example_mod", LifecycleState::Constructed).environment,
        )
        .expect("an optional registration refusal does not abort the set");
        assert_eq!(
            machine.state(&id("example_mod")),
            Ok(LifecycleState::Failed)
        );
        let failure = machine
            .failure(&id("example_mod"))
            .expect("known owner")
            .expect("registration failure retained");
        assert_eq!(failure.state, LifecycleState::Constructed);
        assert_eq!(machine.claimed_content().count(), 0);
        machine
            .lifecycle_world_access()
            .expect("optional failure permits world access");
    }

    #[test]
    fn an_owner_cannot_claim_one_registry_identifier_twice() {
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![
                entry("first_mod", &[Side::Client]),
                entry("second_mod", &[Side::Client]),
            ],
        )
        .expect("two mods are a valid plan");
        for state in [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
        ] {
            machine.advance_all(state).expect("legal");
        }

        machine
            .claim_registry_identifier(
                &id("first_mod"),
                &"block:first_mod:block".parse().expect("valid key"),
                None,
            )
            .expect("the first claim is accepted");
        assert_eq!(
            machine.claim_registry_identifier(
                &id("first_mod"),
                &"block:first_mod:block".parse().expect("valid key"),
                None,
            ),
            Err(LifecycleError::ConflictingRegistryClaim {
                identifier: "block:first_mod:block".parse().expect("valid key"),
                owner: id("first_mod"),
                claimant: id("first_mod"),
            })
        );
        assert_eq!(
            machine.registry_owner(&"block:first_mod:block".parse().expect("valid key")),
            Some(&id("first_mod"))
        );
    }

    #[test]
    fn a_failed_or_parked_mod_does_not_hold_the_registration_barrier_open() {
        let mut machine = LifecycleMachine::new(
            Side::DedicatedServer,
            vec![
                entry("server_mod", &[Side::DedicatedServer]),
                entry("client_mod", &[Side::Client]),
                entry("doomed_mod", &[Side::DedicatedServer]),
            ],
        )
        .expect("three mods are a valid plan");
        for state in [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
        ] {
            machine.advance_all(state).expect("legal");
        }
        machine
            .fail(
                &id("doomed_mod"),
                attribution("doomed_mod", LifecycleState::Constructed),
            )
            .expect("failure is legal from constructed");
        assert!(!machine.registration_barrier_closed());

        machine
            .advance(&id("server_mod"), LifecycleState::Registered)
            .expect("legal");
        assert!(
            machine.registration_barrier_closed(),
            "a parked mod and a failed mod are past caring"
        );
        machine
            .advance(&id("server_mod"), LifecycleState::Running)
            .expect("the only live mod has registered");
    }

    #[test]
    fn a_plan_is_refused_before_anything_runs_when_it_cannot_name_an_owner() {
        assert_eq!(
            LifecycleMachine::new(
                Side::Client,
                vec![
                    entry("example_mod", &[Side::Client]),
                    entry("example_mod", &[Side::Client]),
                ],
            )
            .map(|_| ()),
            Err(LifecycleError::DuplicateIdentity {
                mod_id: id("example_mod")
            })
        );
    }

    #[test]
    fn an_unknown_mod_is_refused_by_every_operation_that_names_one() {
        let mut machine = machine_at(LifecycleState::Discovered);
        let unknown = id("other_mod");
        let refusal = || LifecycleError::UnknownMod {
            mod_id: id("other_mod"),
        };

        assert_eq!(machine.state(&unknown), Err(refusal()));
        assert_eq!(machine.failure(&unknown), Err(refusal()));
        assert_eq!(machine.saved_data_versions(&unknown), Err(refusal()));
        assert_eq!(
            machine.advance(&unknown, LifecycleState::Validated),
            Err(refusal())
        );
        assert_eq!(
            machine.fail(
                &unknown,
                attribution("other_mod", LifecycleState::Discovered)
            ),
            Err(refusal())
        );
        assert_eq!(
            machine.claim_registry_identifier(
                &unknown,
                &"block:other_mod:block".parse().expect("valid key"),
                None,
            ),
            Err(refusal())
        );
    }

    #[test]
    fn an_empty_plan_carries_no_order_and_blocks_nothing() {
        let mut machine =
            LifecycleMachine::new(Side::DedicatedServer, Vec::new()).expect("empty plan");
        assert_eq!(machine.side(), Side::DedicatedServer);
        assert_eq!(machine.order().count(), 0);
        assert_eq!(machine.transcript(), []);
        assert!(machine.registration_barrier_closed());
        machine
            .advance_all(LifecycleState::Validated)
            .expect("legal");
        assert_eq!(machine.transcript(), []);
        machine
            .lifecycle_world_access()
            .expect("nothing blocks an empty set");
        machine.save_access().expect("nothing halted an empty set");
    }

    #[test]
    fn the_code_and_registration_predicates_name_exactly_the_states_they_claim() {
        for state in LifecycleState::ALL {
            assert_eq!(
                executes_mod_code(state),
                matches!(
                    state,
                    LifecycleState::Loaded
                        | LifecycleState::Constructed
                        | LifecycleState::Registered
                        | LifecycleState::Running
                        | LifecycleState::Stopping
                ),
                "{state:?}"
            );
            assert_eq!(
                has_registered(state),
                matches!(
                    state,
                    LifecycleState::Registered
                        | LifecycleState::Running
                        | LifecycleState::Stopping
                        | LifecycleState::Stopped
                ),
                "{state:?}"
            );
        }
    }

    fn every_refusal() -> impl Iterator<Item = (LifecycleError, &'static str)> {
        transition_refusals()
            .into_iter()
            .chain(registration_and_world_refusals())
            .chain(registry_description_refusals())
    }

    fn transition_refusals() -> [(LifecycleError, &'static str); 6] {
        [
            (
                LifecycleError::DuplicateIdentity {
                    mod_id: id("example_mod"),
                },
                "two packages claim the mod id `example_mod`, so nothing can own its \
                 registrations or saved data",
            ),
            (
                LifecycleError::UnknownMod {
                    mod_id: id("example_mod"),
                },
                "the load plan holds no mod `example_mod`",
            ),
            (
                LifecycleError::IllegalTransition {
                    mod_id: id("example_mod"),
                    from: LifecycleState::Discovered,
                    to: LifecycleState::Running,
                },
                "`example_mod` cannot move from `discovered` to `running`",
            ),
            (
                LifecycleError::AttributionRequired,
                "a move to `failed` carries structured attribution, so it is recorded by `fail`",
            ),
            (
                LifecycleError::Halted {
                    mod_id: id("example_mod"),
                    from: LifecycleState::Registered,
                    to: LifecycleState::Running,
                },
                "the set failed after mutating the world, so `example_mod` cannot move from \
                 `registered` to `running`; only stopping remains",
            ),
            (
                LifecycleError::SideMismatch {
                    mod_id: id("example_mod"),
                    from: LifecycleState::Resolved,
                    side: Side::DedicatedServer,
                },
                "`example_mod` does not declare the `dedicated-server` side, so its code never \
                 loads here and it stays `resolved`",
            ),
        ]
    }

    fn registration_and_world_refusals() -> [(LifecycleError, &'static str); 7] {
        [
            (
                LifecycleError::RegistrationBarrierOpen {
                    mod_id: id("first_mod"),
                    blocking: id("second_mod"),
                },
                "`first_mod` cannot start running until the set has registered, and `second_mod` \
                 has not",
            ),
            (
                LifecycleError::RegistrationBarrierClosed {
                    mod_id: id("example_mod"),
                    identifier: "block:example_mod:block".parse().expect("valid key"),
                },
                "the set closed the registration barrier, so `example_mod` cannot claim \
                 `block:example_mod:block`",
            ),
            (
                LifecycleError::RegistrationWindow {
                    mod_id: id("example_mod"),
                    identifier: "block:example_mod:block".parse().expect("valid key"),
                    state: LifecycleState::Loaded,
                },
                "`example_mod` can claim `block:example_mod:block` only while constructed, but it is \
                 `loaded`",
            ),
            (
                LifecycleError::ConflictingRegistryClaim {
                    identifier: "block:shared:block".parse().expect("valid key"),
                    owner: id("first_mod"),
                    claimant: id("second_mod"),
                },
                "`second_mod` claimed the registry identifier `block:shared:block`, which `first_mod` \
                 already owns",
            ),
            (
                LifecycleError::WorldAccessBlocked {
                    mod_id: id("example_mod"),
                    state: LifecycleState::Registered,
                },
                "`example_mod` is `registered` rather than running, so the set cannot open a \
                 world",
            ),
            (
                LifecycleError::WorldAccessHalted {
                    mod_id: id("example_mod"),
                },
                "`example_mod` failed after the world was mutated, so the set cannot open a world",
            ),
            (
                LifecycleError::PartialStateRefused {
                    mod_id: id("example_mod"),
                },
                "`example_mod` failed after the world was mutated, so partial state is not saved",
            ),
        ]
    }

    fn registry_description_refusals() -> [(LifecycleError, &'static str); 2] {
        [
            (
                LifecycleError::ForeignRegistryClaim {
                    key: match "block:other_mod:copper".parse() {
                        Ok(key) => key,
                        Err(error) => panic!("invalid registry key fixture: {error}"),
                    },
                    mod_id: id("example_mod"),
                },
                "`example_mod` cannot claim `block:other_mod:copper` in another mod's namespace",
            ),
            (
                LifecycleError::MismatchedContentBehaviour {
                    identifier: "item:example_mod:copper".parse().expect("valid key"),
                    described: ContentKind::Block,
                },
                "`item:example_mod:copper` was described as block content, which is not the kind \
                 it names",
            ),
        ]
    }

    #[test]
    fn registry_description_refusals_report_the_constructed_phase() {
        for (error, _) in registry_description_refusals() {
            assert_eq!(error.lifecycle_state(), LifecycleState::Constructed);
        }
    }

    #[test]
    fn every_refusal_reports_itself_through_the_shared_failure_trait() {
        for (error, expected) in every_refusal() {
            let diagnostic = error.diagnostic();
            assert_eq!(diagnostic.text(), expected);
            assert_eq!(
                diagnostic.redaction(),
                Redaction::Raw,
                "no failure vouches for its own text; only a policy does"
            );
            assert!(
                !error.quotes_untrusted_text(),
                "every lifecycle message is written here: {error:?}"
            );
            assert_eq!(
                DiagnosticMessage::redacted(diagnostic.text()).text(),
                expected,
                "a lifecycle message names mods, so the policy has nothing to take"
            );

            let expected_kind = match error {
                LifecycleError::DuplicateIdentity { .. } => Some(FailureKind::DuplicateIdentity),
                LifecycleError::SideMismatch { .. } => Some(FailureKind::SideMismatch),
                LifecycleError::ConflictingRegistryClaim { .. } => {
                    Some(FailureKind::ConflictingRegistryClaim)
                }
                LifecycleError::UnknownMod { .. }
                | LifecycleError::IllegalTransition { .. }
                | LifecycleError::AttributionRequired
                | LifecycleError::Halted { .. }
                | LifecycleError::RegistrationBarrierOpen { .. }
                | LifecycleError::RegistrationBarrierClosed { .. }
                | LifecycleError::RegistrationWindow { .. }
                | LifecycleError::ForeignRegistryClaim { .. }
                | LifecycleError::MismatchedContentBehaviour { .. }
                | LifecycleError::WorldAccessBlocked { .. }
                | LifecycleError::WorldAccessHalted { .. }
                | LifecycleError::PartialStateRefused { .. } => None,
            };
            assert_eq!(
                error.failure_kind(),
                expected_kind,
                "only a §9.2 outcome reports a kind: {error:?}"
            );
        }
    }

    #[test]
    fn a_transition_refusal_reports_the_phase_it_names_rather_than_a_default() {
        for (error, expected) in [
            (
                LifecycleError::IllegalTransition {
                    mod_id: id("example_mod"),
                    from: LifecycleState::Loaded,
                    to: LifecycleState::Running,
                },
                LifecycleState::Loaded,
            ),
            (
                LifecycleError::Halted {
                    mod_id: id("example_mod"),
                    from: LifecycleState::Registered,
                    to: LifecycleState::Running,
                },
                LifecycleState::Registered,
            ),
            (
                LifecycleError::SideMismatch {
                    mod_id: id("example_mod"),
                    from: LifecycleState::Resolved,
                    side: Side::DedicatedServer,
                },
                LifecycleState::Resolved,
            ),
            (
                LifecycleError::RegistrationWindow {
                    mod_id: id("example_mod"),
                    identifier: "block:example_mod:block".parse().expect("valid key"),
                    state: LifecycleState::Loaded,
                },
                LifecycleState::Loaded,
            ),
            (
                LifecycleError::WorldAccessBlocked {
                    mod_id: id("example_mod"),
                    state: LifecycleState::Stopping,
                },
                LifecycleState::Stopping,
            ),
        ] {
            assert_eq!(error.lifecycle_state(), expected, "{error:?}");
        }
    }

    #[test]
    fn a_set_refusal_reports_the_phase_it_names_rather_than_a_default() {
        for (error, expected) in [
            (
                LifecycleError::ConflictingRegistryClaim {
                    identifier: "block:shared:block".parse().expect("valid key"),
                    owner: id("first_mod"),
                    claimant: id("second_mod"),
                },
                LifecycleState::Constructed,
            ),
            (
                LifecycleError::RegistrationBarrierOpen {
                    mod_id: id("first_mod"),
                    blocking: id("second_mod"),
                },
                LifecycleState::Registered,
            ),
            (
                LifecycleError::RegistrationBarrierClosed {
                    mod_id: id("example_mod"),
                    identifier: "block:example_mod:block".parse().expect("valid key"),
                },
                LifecycleState::Registered,
            ),
            (
                LifecycleError::WorldAccessHalted {
                    mod_id: id("example_mod"),
                },
                LifecycleState::Failed,
            ),
            (
                LifecycleError::PartialStateRefused {
                    mod_id: id("example_mod"),
                },
                LifecycleState::Failed,
            ),
            (
                LifecycleError::DuplicateIdentity {
                    mod_id: id("example_mod"),
                },
                LifecycleState::Discovered,
            ),
            (
                LifecycleError::UnknownMod {
                    mod_id: id("example_mod"),
                },
                LifecycleState::Discovered,
            ),
            (
                LifecycleError::AttributionRequired,
                LifecycleState::Discovered,
            ),
        ] {
            assert_eq!(error.lifecycle_state(), expected, "{error:?}");
        }
    }
}
