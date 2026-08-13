use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::num::NonZeroU32;

use ravel_core::{
    ContentBehaviour, ContentKind, FailureAttribution, FailureKind, LifecycleState, ModId,
    RegistryKey, Side, SideSet, StructuredFailure,
};

/// Whether a lifecycle state can run mod code.
///
/// `SPEC.md` §14 admits mod code from [`LifecycleState::Loaded`], where the
/// code package is made available, through [`LifecycleState::Stopping`], where
/// shutdown callbacks are still quiescing. Nothing runs once the process has
/// stopped, and every earlier state is bookkeeping over metadata the loader
/// read itself, which is what makes the first `SPEC.md` §14 invariant a
/// property of the transition table rather than a runtime check.
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
///
/// `SPEC.md` §9.2 makes Ravel fail closed on an invalid required-mod graph, so
/// a required mod that fails withholds world access from the whole set while an
/// optional one that fails does not. The resolver decides which a mod is; the
/// lifecycle machine only honours the decision.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Requirement {
    /// The set fails closed without it.
    Required,
    /// The set proceeds without it.
    Optional,
}

/// One mod of a resolved load plan, as the lifecycle needs it.
///
/// This is deliberately not a package manifest and not a resolver output. It is
/// the subset of a resolved plan that lifecycle rules read: the canonical
/// identity that owns saved data, the sides the mod loads on, whether the set
/// fails closed without it, and the saved-data versions it owns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleEntry {
    /// The canonical identity that owns this mod's registrations and saved data.
    pub mod_id: ModId,
    /// The sides the mod declares.
    pub sides: SideSet,
    /// Whether the set fails closed without this mod.
    pub requirement: Requirement,
    /// The saved-data versions this identity owns, empty when it persists
    /// nothing.
    ///
    /// The element type is the manifest's, so a version of zero is
    /// unrepresentable rather than rejected: `SPEC.md` §14 requires persistent
    /// data to be versioned, and the type is what says so.
    pub saved_data_versions: BTreeSet<NonZeroU32>,
}

/// One recorded lifecycle state change.
///
/// The transcript of these events is the machine's determinism evidence: the
/// same plan driven the same way produces the same sequence, in resolved order.
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
/// The machine owns sequencing for a whole set rather than a single mod,
/// because the `SPEC.md` §14 invariants are properties of the set: world access
/// depends on every required mod, the registration barrier is one barrier for
/// the load rather than one checkpoint per mod, and lifecycle order has to be
/// deterministic for an identical package set. Determinism is structural — the
/// resolved order is taken as input and every traversal preserves it, so no
/// caller's iteration order can change the outcome.
///
/// Three collaborators this milestone does not have are modelled as barriers
/// here rather than invented. The registration barrier stands in for the
/// registries M3-W1 builds, [`LifecycleMachine::request_save`] stands in for
/// the save system M3-W4 builds, and [`LifecycleMachine::world_access`] stands
/// in for the pre-world gate M2-W7 renders. Each is the smallest rule that
/// makes its invariant testable, and each is replaced rather than extended when
/// its owning work package lands.
///
/// The machine never loads, links, or executes anything. `Loaded` and
/// `Constructed` are state changes a caller drives once it has performed the
/// step, because ADG-2 has not selected a mod execution model.
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
/// machine.world_access()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct LifecycleMachine {
    side: Side,
    entries: Vec<Tracked>,
    transcript: Vec<LifecycleEvent>,
    claims: BTreeMap<String, ModId>,
    /// How each claimed identifier's value behaves.
    ///
    /// Beside the claim table rather than inside it because the two answer
    /// different questions and only one of them is a barrier: `claims` decides
    /// conflicts and is written by every claim, and this is written only by a
    /// claim that described something. A key here is always a key there.
    behaviours: BTreeMap<String, ContentBehaviour>,
    world_mutated: bool,
    halted: Option<ModId>,
}

impl LifecycleMachine {
    /// Admits a resolved load plan, in the order the resolver produced.
    ///
    /// Every mod starts [`LifecycleState::Discovered`]: the package was found
    /// and nothing has run.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::DuplicateIdentity`] when two entries claim one
    /// mod id, because saved data and registrations are owned by identity and
    /// an ambiguous owner cannot be resolved later.
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
    /// Ownership is fixed when the plan is admitted and no lifecycle step can
    /// move it, which is what makes the identity stable in the sense `SPEC.md`
    /// §14 requires.
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
    /// `SPEC.md` §14 names one barrier for the load, not one checkpoint per
    /// mod, so it closes when every mod still in the plan has committed its
    /// registrations. A mod that failed, and a mod that does not load on this
    /// side, are past caring and do not hold it open.
    #[must_use]
    pub fn registration_barrier_closed(&self) -> bool {
        self.unregistered().is_none()
    }

    /// Returns the plan entry one mod was admitted under.
    ///
    /// The state and the failure are answered separately because they change;
    /// this is what the plan declared and does not.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::UnknownMod`] for an id outside the plan.
    pub fn entry(&self, mod_id: &ModId) -> Result<&LifecycleEntry, LifecycleError> {
        Ok(&self.tracked(mod_id)?.entry)
    }

    /// Returns whether a mod is one this launch is actually running.
    ///
    /// This is the question an optional integration asks: not whether a package
    /// is installed, but whether the mod it would integrate with is in this
    /// plan, loads on this side, and has not failed. All three have to hold, and
    /// each is a different way the answer is no — an id outside the plan is not
    /// installed, a mod parked by its `sides` declaration is installed and will
    /// not run here, and a failed optional mod ran and stopped. A mod that
    /// integrated with any of the three would be building on something absent.
    ///
    /// The answer changes as the set advances, which is why it is a question
    /// rather than a list handed out once: a mod asking during registration is
    /// told what is true at the barrier, and a mod that fails afterwards is a
    /// failure the integrating mod sees through its own calls rather than
    /// through this.
    /// The side is read from the entry's own declaration rather than through
    /// [`Self::is_parked`], which is a fact about the *state machine* — a mod is
    /// parked once the set has resolved and left it behind — and would answer
    /// yes for a server-only mod on a client before the set reaches
    /// [`LifecycleState::Resolved`]. What a mod is asking is whether the other
    /// mod will run here, and its `sides` list decides that from the moment the
    /// plan is admitted.
    #[must_use]
    pub fn loads(&self, mod_id: &ModId) -> bool {
        self.tracked(mod_id).is_ok_and(|tracked| {
            tracked.entry.sides.loads_on(self.side) && tracked.state != LifecycleState::Failed
        })
    }

    /// Returns the identity that owns a registry identifier.
    #[must_use]
    pub fn registry_owner(&self, identifier: &str) -> Option<&ModId> {
        self.claims.get(identifier)
    }

    /// Returns how many registry identifiers one mod owns.
    ///
    /// This is `SPEC.md` §9.3's registry footprint, and it is a reading of the
    /// claim table rather than something a collector accumulates: the table is
    /// exact whether or not anything is profiling, and counting it here means
    /// the number cannot drift from what the barrier actually admitted.
    #[must_use]
    pub fn registry_claims(&self, mod_id: &ModId) -> usize {
        self.claims
            .values()
            .filter(|owner| *owner == mod_id)
            .count()
    }

    /// Returns every admitted registry claim, with the identity that owns it.
    ///
    /// This is the claim table read out rather than counted, and it exists
    /// because something outside the loader has to carry these into the game:
    /// the barrier decides *that* a mod owns `block:example:copper`, and a
    /// registry in the running game is where that becomes a block. The order is
    /// the table's own, which is sorted by identifier, so two runs over one set
    /// produce the same sequence and a caller that turns it into registrations
    /// cannot make the game's contents depend on map iteration order.
    ///
    /// A claim whose identifier does not parse as a [`RegistryKey`] is skipped.
    /// The barrier stores identifiers as text, and every claim reaching it came
    /// from a key that was formatted into that text, so the parse cannot fail
    /// for anything this loader admitted; skipping rather than panicking means
    /// a future claimant that writes the table by another route degrades to
    /// carrying less content instead of taking the game down.
    pub fn claimed_content(&self) -> impl Iterator<Item = (RegistryKey, &ModId, ContentBehaviour)> {
        self.claims.iter().filter_map(|(identifier, owner)| {
            let key: RegistryKey = identifier.parse().ok()?;
            // A claim that described nothing gets the kind's default, and a kind
            // with no value to describe is not carried at all. That is what
            // keeps "described plainly" and "not a thing a registry builds"
            // from arriving as the same answer.
            let behaviour = self
                .behaviours
                .get(identifier)
                .copied()
                .or_else(|| ContentBehaviour::default_for(key.kind()))?;
            Some((key, owner, behaviour))
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
    /// A mod that has already ended, and a mod parked at
    /// [`LifecycleState::Resolved`] because it does not load on this side, are
    /// both skipped rather than refused. Every other mod must be able to take
    /// the step: the call is checked in full before anything moves, so a set
    /// that cannot advance together does not advance partway.
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
        let mut selected = Vec::with_capacity(self.entries.len());
        for tracked in &self.entries {
            let moves = !Self::is_settled(self.side, tracked);
            if moves {
                Self::check(&context, tracked, next)?;
            }
            selected.push(moves);
        }

        let mut events = Vec::new();
        for (tracked, moves) in self.entries.iter_mut().zip(selected) {
            if moves {
                tracked.state = next;
                events.push(LifecycleEvent {
                    mod_id: tracked.entry.mod_id.clone(),
                    state: next,
                });
            }
        }
        self.transcript.extend(events);
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

    /// Claims a registry identifier for a mod before the registration barrier.
    ///
    /// `SPEC.md` §14 makes registry identifiers immutable after *the*
    /// registration barrier, one barrier for the whole load, so a claim is
    /// refused once the set has closed it no matter which mod asks. Before
    /// then, a mod claims while it is [`LifecycleState::Constructed`]: its
    /// instance exists and its registrations are not yet committed. This is the
    /// barrier, not a registry — M3-W1 builds what the identifiers name.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::UnknownMod`] for an id outside the plan,
    /// [`LifecycleError::RegistrationBarrierClosed`] once the set has
    /// registered, [`LifecycleError::RegistrationWindow`] while the set is
    /// still registering but this mod is not constructed, and
    /// [`LifecycleError::ConflictingRegistryClaim`] when the identifier is
    /// already owned.
    pub fn claim_registry_identifier(
        &mut self,
        mod_id: &ModId,
        identifier: &str,
    ) -> Result<(), LifecycleError> {
        self.claim_described_registry_identifier(mod_id, identifier, None)
    }

    /// Claims `identifier` and records how the value it names behaves.
    ///
    /// The behaviour is checked against the identifier's own kind rather than
    /// trusted: a mod describing `item:example_mod:copper` as a block has asked
    /// for a value no registry takes, and admitting it would move the failure to
    /// the moment the game tries to build one.
    ///
    /// # Errors
    ///
    /// Every refusal [`Self::claim_registry_identifier`] makes, plus
    /// [`LifecycleError::MismatchedContentBehaviour`] when the description does
    /// not describe the kind the identifier names.
    pub fn claim_described_registry_identifier(
        &mut self,
        mod_id: &ModId,
        identifier: &str,
        behaviour: Option<ContentBehaviour>,
    ) -> Result<(), LifecycleError> {
        let closed = self.registration_barrier_closed();
        let tracked = self.tracked(mod_id)?;
        if closed {
            return Err(LifecycleError::RegistrationBarrierClosed {
                mod_id: tracked.entry.mod_id.clone(),
                identifier: identifier.to_owned(),
            });
        }
        if tracked.state != LifecycleState::Constructed {
            return Err(LifecycleError::RegistrationWindow {
                mod_id: tracked.entry.mod_id.clone(),
                identifier: identifier.to_owned(),
                state: tracked.state,
            });
        }
        if let Some(owner) = self.claims.get(identifier) {
            return Err(LifecycleError::ConflictingRegistryClaim {
                identifier: identifier.to_owned(),
                owner: owner.clone(),
                claimant: tracked.entry.mod_id.clone(),
            });
        }

        // Checked before either table is written, so a refused description
        // leaves no identifier owned. A claim that half happened would be a name
        // a mod can neither re-claim nor use.
        if let Some(behaviour) = behaviour
            && !identifier
                .parse::<RegistryKey>()
                .is_ok_and(|key| behaviour.describes(key.kind()))
        {
            return Err(LifecycleError::MismatchedContentBehaviour {
                identifier: identifier.to_owned(),
                described: behaviour.kind(),
            });
        }

        let owner = tracked.entry.mod_id.clone();
        if let Some(behaviour) = behaviour {
            self.behaviours.insert(identifier.to_owned(), behaviour);
        }
        self.claims.insert(identifier.to_owned(), owner);
        Ok(())
    }

    /// Decides whether the set may open a world.
    ///
    /// Access needs every mod that loads on this side to be
    /// [`LifecycleState::Running`], except an optional mod that failed, whose
    /// absence the plan already tolerates. A halted set never regains access.
    /// M2-W7 renders the refusal; this decides it.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::WorldAccessHalted`] when a failure followed
    /// world mutation, and [`LifecycleError::WorldAccessBlocked`] naming the
    /// first mod in resolved order that is not running.
    pub fn world_access(&self) -> Result<(), LifecycleError> {
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
    /// After this point a failure is no longer recoverable by stopping quietly,
    /// because state a mod already changed is on disk's critical path.
    ///
    /// # Errors
    ///
    /// Returns whatever [`LifecycleMachine::world_access`] refuses: a set that
    /// cannot open a world cannot have mutated one.
    pub fn record_world_mutation(&mut self) -> Result<(), LifecycleError> {
        self.world_access()?;
        self.world_mutated = true;
        Ok(())
    }

    /// Decides whether saved state may be written.
    ///
    /// A halted set is refused, which is the half of the sixth `SPEC.md` §14
    /// invariant that forbids saving partial state; the other half is that
    /// [`LifecycleMachine::advance`] refuses forward progress. M3-W4 owns what
    /// a permitted save then writes.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::PartialStateRefused`] when a failure followed
    /// world mutation.
    pub fn request_save(&self) -> Result<(), LifecycleError> {
        match &self.halted {
            Some(mod_id) => Err(LifecycleError::PartialStateRefused {
                mod_id: mod_id.clone(),
            }),
            None => Ok(()),
        }
    }

    /// The set-wide facts a per-mod check reads, taken before the mod itself is
    /// borrowed.
    fn context(&self) -> TransitionContext {
        TransitionContext {
            side: self.side,
            halted: self.halted.is_some(),
            unregistered: self.unregistered().cloned(),
        }
    }

    /// The first mod in resolved order still holding the registration barrier
    /// open.
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
        // Running is the first state past the registration barrier, so no mod
        // starts until the whole set has committed its registrations.
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

    /// Whether a mod does not load on this side and has gone as far as a mod
    /// that never loads can go.
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

/// Why the lifecycle refused an operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LifecycleError {
    /// Two entries of one plan claim the same mod id.
    DuplicateIdentity {
        /// The contested identity.
        mod_id: ModId,
    },
    /// The plan holds no such mod.
    UnknownMod {
        /// The requested identity.
        mod_id: ModId,
    },
    /// The `SPEC.md` §14 table refuses this pair of states.
    IllegalTransition {
        /// The mod that would have moved.
        mod_id: ModId,
        /// The state it is in.
        from: LifecycleState,
        /// The state it was asked to enter.
        to: LifecycleState,
    },
    /// A move to `failed` arrived without structured attribution.
    AttributionRequired,
    /// The set halted after a failure that followed world mutation.
    Halted {
        /// The mod that would have moved.
        mod_id: ModId,
        /// The state it is in.
        from: LifecycleState,
        /// The state it was asked to enter.
        to: LifecycleState,
    },
    /// Mod code would have run on a side the mod does not declare.
    SideMismatch {
        /// The mod that would have run.
        mod_id: ModId,
        /// The state it is in.
        from: LifecycleState,
        /// The side the set is running on.
        side: Side,
    },
    /// A mod would have started running before the set finished registering.
    RegistrationBarrierOpen {
        /// The mod that would have started.
        mod_id: ModId,
        /// The mod still holding the barrier open.
        blocking: ModId,
    },
    /// A registry identifier was claimed after the set closed the barrier.
    RegistrationBarrierClosed {
        /// The claiming mod.
        mod_id: ModId,
        /// The identifier it claimed.
        identifier: String,
    },
    /// A registry identifier was claimed by a mod that is not constructed.
    RegistrationWindow {
        /// The claiming mod.
        mod_id: ModId,
        /// The identifier it claimed.
        identifier: String,
        /// The state the claim arrived in.
        state: LifecycleState,
    },
    /// Two mods claim one registry identifier.
    ConflictingRegistryClaim {
        /// The contested identifier.
        identifier: String,
        /// The mod that already owns it.
        owner: ModId,
        /// The mod that claimed it second.
        claimant: ModId,
    },
    /// A mod described claimed content as a kind the identifier does not name.
    MismatchedContentBehaviour {
        /// The identifier that was described.
        identifier: String,
        /// The kind the description describes.
        described: ContentKind,
    },
    /// A mod is not running, so the set cannot open a world.
    WorldAccessBlocked {
        /// The mod that is not running.
        mod_id: ModId,
        /// The state it is in.
        state: LifecycleState,
    },
    /// The set failed after mutating the world, so it never regains access.
    WorldAccessHalted {
        /// The mod whose failure halted the set.
        mod_id: ModId,
    },
    /// A save was requested after a failure that followed world mutation.
    PartialStateRefused {
        /// The mod whose failure halted the set.
        mod_id: ModId,
    },
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateIdentity { mod_id } => write!(
                formatter,
                "two packages claim the mod id `{mod_id}`, so nothing can own its registrations \
                 or saved data"
            ),
            Self::UnknownMod { mod_id } => {
                write!(formatter, "the load plan holds no mod `{mod_id}`")
            }
            Self::IllegalTransition { mod_id, from, to } => write!(
                formatter,
                "`{mod_id}` cannot move from `{}` to `{}`",
                state_name(*from),
                state_name(*to)
            ),
            Self::AttributionRequired => formatter.write_str(
                "a move to `failed` carries structured attribution, so it is recorded by `fail`",
            ),
            Self::Halted { mod_id, from, to } => write!(
                formatter,
                "the set failed after mutating the world, so `{mod_id}` cannot move from `{}` to \
                 `{}`; only stopping remains",
                state_name(*from),
                state_name(*to)
            ),
            Self::SideMismatch { mod_id, from, side } => write!(
                formatter,
                "`{mod_id}` does not declare the `{}` side, so its code never loads here and it \
                 stays `{}`",
                side_name(*side),
                state_name(*from)
            ),
            Self::RegistrationBarrierOpen { mod_id, blocking } => write!(
                formatter,
                "`{mod_id}` cannot start running until the set has registered, and `{blocking}` \
                 has not"
            ),
            Self::RegistrationBarrierClosed { mod_id, identifier } => write!(
                formatter,
                "the set closed the registration barrier, so `{mod_id}` cannot claim \
                 `{identifier}`"
            ),
            Self::RegistrationWindow {
                mod_id,
                identifier,
                state,
            } => write!(
                formatter,
                "`{mod_id}` can claim `{identifier}` only while constructed, but it is `{}`",
                state_name(*state)
            ),
            Self::ConflictingRegistryClaim {
                identifier,
                owner,
                claimant,
            } => write!(
                formatter,
                "`{claimant}` claimed the registry identifier `{identifier}`, which `{owner}` \
                 already owns"
            ),
            Self::MismatchedContentBehaviour {
                identifier,
                described,
            } => write!(
                formatter,
                "`{identifier}` was described as {described} content, which is not the kind it \
                 names"
            ),
            Self::WorldAccessBlocked { mod_id, state } => write!(
                formatter,
                "`{mod_id}` is `{}` rather than running, so the set cannot open a world",
                state_name(*state)
            ),
            Self::WorldAccessHalted { mod_id } => write!(
                formatter,
                "`{mod_id}` failed after the world was mutated, so the set cannot open a world"
            ),
            Self::PartialStateRefused { mod_id } => write!(
                formatter,
                "`{mod_id}` failed after the world was mutated, so partial state is not saved"
            ),
        }
    }
}

impl std::error::Error for LifecycleError {}

impl StructuredFailure for LifecycleError {
    /// Only three lifecycle refusals are `SPEC.md` §9.2 graph outcomes. The
    /// rest are sequencing rules, which a report describes by their phase and
    /// text rather than by borrowing a dependency-graph kind they are not.
    fn failure_kind(&self) -> Option<FailureKind> {
        match self {
            Self::DuplicateIdentity { .. } => Some(FailureKind::DuplicateIdentity),
            Self::SideMismatch { .. } => Some(FailureKind::SideMismatch),
            Self::ConflictingRegistryClaim { .. } => Some(FailureKind::ConflictingRegistryClaim),
            _ => None,
        }
    }

    /// The phase the subject had reached, taken from the refusal itself
    /// wherever it names one.
    fn lifecycle_state(&self) -> LifecycleState {
        match self {
            Self::IllegalTransition { from, .. }
            | Self::Halted { from, .. }
            | Self::SideMismatch { from, .. } => *from,
            Self::RegistrationWindow { state, .. } | Self::WorldAccessBlocked { state, .. } => {
                *state
            }
            // Both are refusals of a claim, and a claim is only ever made from
            // the one state the window is open in.
            Self::ConflictingRegistryClaim { .. } | Self::MismatchedContentBehaviour { .. } => {
                LifecycleState::Constructed
            }
            Self::RegistrationBarrierOpen { .. } | Self::RegistrationBarrierClosed { .. } => {
                LifecycleState::Registered
            }
            Self::WorldAccessHalted { .. } | Self::PartialStateRefused { .. } => {
                LifecycleState::Failed
            }
            // A plan the loader would not admit, and an operation naming a mod
            // that is not in one, both precede every lifecycle step.
            Self::DuplicateIdentity { .. }
            | Self::UnknownMod { .. }
            | Self::AttributionRequired => LifecycleState::Discovered,
        }
    }
}

/// The `SPEC.md` §14 name of a state, matching its serialized form.
const fn state_name(state: LifecycleState) -> &'static str {
    state.name()
}

/// The `SPEC.md` §3.4 name of a side, matching its serialized form.
///
/// A delegate for the reason [`state_name`] is one: the spelling belongs to
/// `ravel-core` beside the type, and a second `match` here would be a second
/// answer that can disagree with it.
const fn side_name(side: Side) -> &'static str {
    side.name()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use ravel_core::{
        ContentBehaviour, ContentKind, FailureAttribution, FailureEnvironment, FailureKind,
        LifecycleState, ModId, Redaction, Side, SideSet, StructuredFailure,
    };
    use ravel_observability::redact;

    use super::{
        LifecycleEntry, LifecycleError, LifecycleMachine, Requirement, executes_mod_code,
        has_registered, side_name, state_name,
    };

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

    fn entry(name: &str, sides: &[Side]) -> LifecycleEntry {
        LifecycleEntry {
            mod_id: id(name),
            sides: SideSet::new(sides.iter().copied()).expect("at least one side"),
            requirement: Requirement::Required,
            saved_data_versions: BTreeSet::new(),
        }
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
            message: redact("the dependency is absent"),
        }
    }

    /// `loads` answers the question an optional integration asks, and every way
    /// the answer is no is a different absence.
    ///
    /// Written against a client machine holding a client mod, a server-only mod
    /// and a mod that fails: the first runs here, the second is installed and
    /// parked by its own `sides` declaration, the third ran and stopped, and a
    /// fourth id was never in the plan at all.
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

    /// Drives a single-mod machine to `target` through legal steps only.
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
            machine.claim_registry_identifier(&mod_id, "example_mod:block"),
            Err(LifecycleError::RegistrationWindow {
                mod_id: mod_id.clone(),
                identifier: "example_mod:block".to_owned(),
                state: LifecycleState::Loaded,
            })
        );

        machine
            .advance(&mod_id, LifecycleState::Constructed)
            .expect("legal");
        machine
            .claim_registry_identifier(&mod_id, "example_mod:block")
            .expect("the window is open while constructed");
        assert_eq!(machine.registry_owner("example_mod:block"), Some(&mod_id));
        assert_eq!(machine.registry_owner("example_mod:item"), None);
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

        // Claimed out of sorted order, and by two different owners, so the order
        // the table answers in is the table's and not the claimant's.
        machine
            .claim_registry_identifier(&id("second_mod"), "item:second_mod:ingot")
            .expect("accepted");
        machine
            .claim_registry_identifier(&id("first_mod"), "block:first_mod:copper")
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
    fn a_claim_carries_what_the_mod_described_and_its_kind_default_otherwise() {
        let mut machine = machine_at(LifecycleState::Constructed);
        let mod_id = id("example_mod");
        machine
            .claim_described_registry_identifier(
                &mod_id,
                "block:example_mod:obsidian",
                Some(ContentBehaviour::Block(ravel_core::BlockBehaviour {
                    hardness: 50.0,
                    explosion_resistance: 1200.0,
                    requires_correct_tool: true,
                    random_ticks: false,
                })),
            )
            .expect("accepted");
        machine
            .claim_registry_identifier(&mod_id, "item:example_mod:ingot")
            .expect("accepted");

        let described: Vec<ContentBehaviour> = machine
            .claimed_content()
            .map(|(_, _, behaviour)| behaviour)
            .collect();

        assert_eq!(
            described,
            vec![
                ContentBehaviour::Block(ravel_core::BlockBehaviour {
                    hardness: 50.0,
                    explosion_resistance: 1200.0,
                    requires_correct_tool: true,
                    random_ticks: false,
                }),
                ContentBehaviour::Item(ravel_core::ItemBehaviour::default()),
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
            .claim_described_registry_identifier(
                &mod_id,
                "item:example_mod:copper",
                Some(ContentBehaviour::Block(
                    ravel_core::BlockBehaviour::default(),
                )),
            )
            .expect_err("an item described as a block is a value no registry builds");

        assert!(
            matches!(error, LifecycleError::MismatchedContentBehaviour { .. }),
            "{error}"
        );
        assert_eq!(
            machine.registry_claims(&mod_id),
            0,
            "a refused description leaves no identifier owned, or the mod holds a \
             name it can neither re-claim nor use"
        );
    }

    #[test]
    fn a_claim_that_names_no_kind_is_skipped_rather_than_taking_the_game_down() {
        let mut machine = machine_at(LifecycleState::Constructed);
        machine
            .claim_registry_identifier(&id("example_mod"), "example_mod:block")
            .expect("the barrier stores whatever text it is given");

        assert_eq!(
            machine.claimed_content().count(),
            0,
            "an identifier the barrier holds that names no kind carries no \
             content, and carrying less is not the same as failing"
        );
        assert_eq!(
            machine.registry_claims(&id("example_mod")),
            1,
            "the barrier still owns it, so the footprint is unchanged"
        );
    }

    #[test]
    fn two_mods_cannot_claim_one_registry_identifier() {
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
            .claim_registry_identifier(&id("first_mod"), "shared:block")
            .expect("the first claim is accepted");
        assert_eq!(
            machine.claim_registry_identifier(&id("second_mod"), "shared:block"),
            Err(LifecycleError::ConflictingRegistryClaim {
                identifier: "shared:block".to_owned(),
                owner: id("first_mod"),
                claimant: id("second_mod"),
            })
        );
        assert_eq!(
            machine.registry_owner("shared:block"),
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
            machine.claim_registry_identifier(&unknown, "other_mod:block"),
            Err(refusal())
        );
    }

    #[test]
    fn an_empty_plan_carries_no_order_and_blocks_nothing() {
        let mut machine =
            LifecycleMachine::new(Side::DedicatedServer, Vec::new()).expect("empty plan");
        assert_eq!(machine.side(), Side::DedicatedServer);
        assert_eq!(machine.order().count(), 0);
        assert!(machine.transcript().is_empty());
        assert!(machine.registration_barrier_closed());
        machine
            .advance_all(LifecycleState::Validated)
            .expect("legal");
        assert!(machine.transcript().is_empty());
        machine.world_access().expect("nothing blocks an empty set");
        machine.request_save().expect("nothing halted an empty set");
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

    /// Every variant with the text it renders, so rendering and structured
    /// reporting are both checked across the whole error surface.
    fn every_refusal() -> Vec<(LifecycleError, &'static str)> {
        vec![
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
                    identifier: "example_mod:block".to_owned(),
                },
                "the set closed the registration barrier, so `example_mod` cannot claim \
                 `example_mod:block`",
            ),
            (
                LifecycleError::RegistrationWindow {
                    mod_id: id("example_mod"),
                    identifier: "example_mod:block".to_owned(),
                    state: LifecycleState::Loaded,
                },
                "`example_mod` can claim `example_mod:block` only while constructed, but it is \
                 `loaded`",
            ),
            (
                LifecycleError::ConflictingRegistryClaim {
                    identifier: "shared:block".to_owned(),
                    owner: id("first_mod"),
                    claimant: id("second_mod"),
                },
                "`second_mod` claimed the registry identifier `shared:block`, which `first_mod` \
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

    #[test]
    fn every_refusal_renders_the_rule_it_broke() {
        for (error, expected) in every_refusal() {
            assert_eq!(error.to_string(), expected);
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
                redact(diagnostic.text()).text(),
                expected,
                "a lifecycle message names mods, so the policy has nothing to take"
            );

            let expected_kind = match error {
                LifecycleError::DuplicateIdentity { .. } => Some(FailureKind::DuplicateIdentity),
                LifecycleError::SideMismatch { .. } => Some(FailureKind::SideMismatch),
                LifecycleError::ConflictingRegistryClaim { .. } => {
                    Some(FailureKind::ConflictingRegistryClaim)
                }
                _ => None,
            };
            assert_eq!(
                error.failure_kind(),
                expected_kind,
                "only a §9.2 outcome reports a kind: {error:?}"
            );
        }
    }

    #[test]
    fn a_refusal_reports_the_phase_it_names_rather_than_a_default() {
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
                    identifier: "example_mod:block".to_owned(),
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
            (
                LifecycleError::ConflictingRegistryClaim {
                    identifier: "shared:block".to_owned(),
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
                    identifier: "example_mod:block".to_owned(),
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

    #[test]
    fn rendered_state_and_side_names_match_their_serialized_forms() {
        for state in LifecycleState::ALL {
            let encoded = serde_json::to_string(&state).expect("state should serialize");
            assert_eq!(encoded, format!("\"{}\"", state_name(state)));
        }
        for side in Side::ALL {
            let encoded = serde_json::to_string(&side).expect("side should serialize");
            assert_eq!(encoded, format!("\"{}\"", side_name(side)));
        }
    }
}
