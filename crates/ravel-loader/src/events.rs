use std::fmt;

use ravel_core::{FailureKind, LifecycleState, ModId, RegistryKey, StructuredFailure};
use ravel_host_api::{CommandInvocation, GameEvent, HostError, HostMod};

use crate::lifecycle::LifecycleMachine;

/// Whether a state may receive an event or run a command.
///
/// A mod reacts only while the set is past the registration barrier and has not
/// stopped. `SPEC.md` §14 puts every registration before
/// [`LifecycleState::Running`], so an event delivered earlier would reach a mod
/// whose own content does not exist yet; one delivered after
/// [`LifecycleState::Stopped`] would reach a mod that has already been told the
/// run is over. [`crate::executes_mod_code`] is the wider window that includes
/// both, so this is not that check and cannot be replaced by it.
const fn reacts(state: LifecycleState) -> bool {
    matches!(state, LifecycleState::Running | LifecycleState::Stopping)
}

/// Why an event or a command reached no mod.
///
/// Every variant is a fact about the set rather than about the thing being
/// delivered, because a delivery that fails does so for the same reason for
/// every recipient. A mod that fails *while* reacting is not here: that is
/// [`HostError`], raised by the mod, and [`Delivery`] carries it out whole.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchRefused {
    /// The set is not in a state that reacts.
    NotReacting {
        /// The mod whose state was checked first, in resolved order.
        mod_id: ModId,
        /// The state it is in.
        state: LifecycleState,
    },
    /// No mod claimed the command's root literal.
    Unclaimed {
        /// The key nobody owns.
        key: RegistryKey,
    },
    /// The command's owner is not in a state that reacts.
    OwnerNotReacting {
        /// The mod that owns the command.
        mod_id: ModId,
        /// The state it is in.
        state: LifecycleState,
    },
    /// The owner is loaded on sides that do not include the running one.
    ///
    /// This is distinct from a mod that simply does not receive a broadcast.
    /// A command a player can type on this side, owned by a mod that does not
    /// run on this side, is a set that would silently swallow the command, so
    /// it is refused with the reason rather than ignored.
    OwnerWrongSide {
        /// The mod that owns the command.
        mod_id: ModId,
    },
    /// The set holds no value for a mod the order names.
    ///
    /// The caller supplies mod values alongside the machine, and the two can
    /// disagree. A missing value is the caller's mistake and not the set's, so
    /// it is reported rather than skipped: skipping would make a mod that was
    /// never supplied indistinguishable from one that chose to ignore the
    /// event.
    Absent {
        /// The mod the order names and the caller did not supply.
        mod_id: ModId,
    },
}

impl fmt::Display for DispatchRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotReacting { mod_id, state } => write!(
                formatter,
                "the set is `{}` at `{mod_id}`, which does not react",
                state.name()
            ),
            Self::Unclaimed { key } => write!(formatter, "no mod claimed `{key}`"),
            Self::OwnerNotReacting { mod_id, state } => write!(
                formatter,
                "`{mod_id}` owns the command and is `{}`, which does not react",
                state.name()
            ),
            Self::OwnerWrongSide { mod_id } => write!(
                formatter,
                "`{mod_id}` owns the command and does not load on this side"
            ),
            Self::Absent { mod_id } => {
                write!(formatter, "the order names `{mod_id}` and no value for it")
            }
        }
    }
}

impl std::error::Error for DispatchRefused {}

impl StructuredFailure for DispatchRefused {
    /// None of these is one of `SPEC.md` §9.2's dependency-graph outcomes. A
    /// set that is not running, a command nobody claimed, and a value the
    /// caller did not supply are all facts about this delivery rather than
    /// about the mod graph, so this reports no kind rather than borrowing one.
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    fn lifecycle_state(&self) -> LifecycleState {
        match self {
            Self::NotReacting { state, .. } | Self::OwnerNotReacting { state, .. } => *state,
            Self::Unclaimed { .. } | Self::OwnerWrongSide { .. } | Self::Absent { .. } => {
                LifecycleState::Running
            }
        }
    }

    /// Every word is this crate's own, about identifiers `ravel-core` has
    /// already validated against a charset holding no path separator and no
    /// whitespace.
    fn quotes_untrusted_text(&self) -> bool {
        false
    }
}

/// What one delivery did.
///
/// A refused delivery reached nobody and a failed one reached everybody before
/// the mod that failed, so the two are not the same shape and are not folded
/// into one `Result`. [`Self::received`] is the ordering evidence `ROADMAP.md`'s
/// M3-W2 row asks for: it is the resolved load order filtered by side, and it is
/// the same list every run for the same set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Delivery {
    received: Vec<ModId>,
    failure: Option<HostError>,
}

impl Delivery {
    /// Returns the mods that reacted, in the order they were called.
    #[must_use]
    pub fn received(&self) -> &[ModId] {
        &self.received
    }

    /// Returns the failure that ended the delivery, when one did.
    #[must_use]
    pub const fn failure(&self) -> Option<&HostError> {
        self.failure.as_ref()
    }

    /// Returns whether every mod that should have reacted did so.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.failure.is_none()
    }
}

impl LifecycleMachine {
    /// Returns the mods that receive a broadcast of `event`, in resolved order.
    ///
    /// The audience is the load order filtered twice: by whether the mod loads
    /// on the running side, and by whether the event happens on that side at
    /// all. The second filter is why a client tick on a dedicated server has an
    /// empty audience rather than one that happens to be empty because no mod
    /// is client-side.
    ///
    /// This answers the question without needing the mod values, so a report or
    /// a test can ask who *would* receive an event without delivering one.
    #[must_use]
    pub fn audience(&self, event: GameEvent) -> Vec<ModId> {
        if !event.happens_on(self.side()) {
            return Vec::new();
        }
        self.order()
            .filter(|mod_id| self.loads_here(mod_id))
            .cloned()
            .collect()
    }

    /// Returns whether `mod_id` loads on the side this set is running on.
    ///
    /// The machine already refuses to advance a mod onto the wrong side, so a
    /// running set cannot hold one. It can still be asked before the set runs,
    /// which is what makes an empty audience checkable without driving a set.
    fn loads_here(&self, mod_id: &ModId) -> bool {
        self.entry(mod_id)
            .is_ok_and(|entry| entry.sides.loads_on(self.side()))
    }
}

/// Broadcasts `event` to every mod in the audience, in resolved order.
///
/// The whole set is checked before any mod reacts. A delivery that would find
/// the set in the wrong state part-way through is one that has already changed
/// something, and `SPEC.md` §9.2 wants a refusal a player can act on rather than
/// a half-delivered event.
///
/// # Errors
///
/// Returns [`DispatchRefused`] when nothing was delivered: the set does not
/// react, or the caller supplied no value for a mod the order names. A mod that
/// fails while reacting is not an error here — it ends the broadcast and is
/// carried out in [`Delivery::failure`], because the mods before it did react
/// and a report that lost that would be describing a different run.
///
/// # Panics
///
/// Never. The audience is checked against the supplied values before anything
/// is delivered, so the lookup during delivery is asking about a mod the check
/// already found.
///
/// `mods` writes the trait object's own lifetime bound rather than letting it
/// default to the borrow's. Elision would tie the two together, and `&mut` is
/// invariant in what it points at, so a caller holding `Box<dyn HostMod>` —
/// which is `dyn HostMod + 'static` — could not lend them for anything shorter
/// than `'static`. That is exactly what a runtime owning its mods does, and the
/// bound is what lets it.
pub fn broadcast(
    machine: &LifecycleMachine,
    mods: &mut [&mut (dyn HostMod + '_)],
    event: GameEvent,
) -> Result<Delivery, DispatchRefused> {
    let audience = machine.audience(event);
    for mod_id in &audience {
        let state = machine.state(mod_id).map_err(|_| DispatchRefused::Absent {
            mod_id: mod_id.clone(),
        })?;
        if !reacts(state) {
            return Err(DispatchRefused::NotReacting {
                mod_id: mod_id.clone(),
                state,
            });
        }
        if !mods.iter().any(|host| host.id() == mod_id) {
            return Err(DispatchRefused::Absent {
                mod_id: mod_id.clone(),
            });
        }
    }

    let mut received = Vec::with_capacity(audience.len());
    for mod_id in audience {
        let host = mods
            .iter_mut()
            .find(|host| host.id() == &mod_id)
            .expect("the audience was checked against the supplied values");
        if let Err(failure) = host.observe(event) {
            return Ok(Delivery {
                received,
                failure: Some(failure),
            });
        }
        received.push(mod_id);
    }
    Ok(Delivery {
        received,
        failure: None,
    })
}

/// Routes `invocation` to the one mod that claimed its root literal.
///
/// A command is routed and an event is broadcast, and the difference is the
/// registration barrier: exactly one mod owns a command name, so asking every
/// mod would be asking the ones that already lost the claim.
///
/// # Errors
///
/// Returns [`DispatchRefused`] when the command reached nobody: nothing claimed
/// the key, the owner is not reacting, the owner does not load on this side, or
/// the caller supplied no value for it.
pub fn route(
    machine: &LifecycleMachine,
    mods: &mut [&mut (dyn HostMod + '_)],
    invocation: &CommandInvocation,
) -> Result<Delivery, DispatchRefused> {
    let owner = machine
        .owner_of(&invocation.key)
        .ok_or_else(|| DispatchRefused::Unclaimed {
            key: invocation.key.clone(),
        })?
        .clone();

    if !machine.loads_here(&owner) {
        return Err(DispatchRefused::OwnerWrongSide { mod_id: owner });
    }
    let state = machine.state(&owner).map_err(|_| DispatchRefused::Absent {
        mod_id: owner.clone(),
    })?;
    if !reacts(state) {
        return Err(DispatchRefused::OwnerNotReacting {
            mod_id: owner,
            state,
        });
    }

    let host = mods
        .iter_mut()
        .find(|host| host.id() == &owner)
        .ok_or_else(|| DispatchRefused::Absent {
            mod_id: owner.clone(),
        })?;

    match host.run_command(invocation) {
        Ok(()) => Ok(Delivery {
            received: vec![owner],
            failure: None,
        }),
        Err(failure) => Ok(Delivery {
            received: Vec::new(),
            failure: Some(failure),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use ravel_core::{
        ContentId, ContentKind, LifecycleState, ModId, RegistryKey, Side, SideSet,
        StructuredFailure,
    };
    use ravel_host_api::{CommandInvocation, GameEvent, HostError, HostMod, Registrar};

    use super::{Delivery, DispatchRefused, broadcast, route};
    use crate::lifecycle::{LifecycleEntry, LifecycleMachine, Requirement};

    fn id(text: &str) -> ModId {
        text.parse().expect("a legal mod id")
    }

    fn command(namespace: &str, path: &str) -> RegistryKey {
        RegistryKey::new(
            ContentKind::Command,
            ContentId::new(id(namespace), path).expect("a legal content path"),
        )
    }

    fn entry(name: &str, sides: &[Side]) -> LifecycleEntry {
        LifecycleEntry {
            mod_id: id(name),
            sides: SideSet::new(sides.iter().copied()).expect("a non-empty side set"),
            requirement: Requirement::Required,
            saved_data_versions: BTreeSet::new(),
        }
    }

    /// A mod that records every event it saw and every command it ran.
    struct Recording {
        id: ModId,
        seen: Vec<GameEvent>,
        ran: Vec<CommandInvocation>,
        fails_on: Option<GameEvent>,
        claims: Vec<RegistryKey>,
    }

    impl Recording {
        fn new(name: &str) -> Self {
            Self {
                id: id(name),
                seen: Vec::new(),
                ran: Vec::new(),
                fails_on: None,
                claims: Vec::new(),
            }
        }

        fn claiming(name: &str, keys: Vec<RegistryKey>) -> Self {
            Self {
                claims: keys,
                ..Self::new(name)
            }
        }

        fn failing(name: &str, event: GameEvent) -> Self {
            Self {
                fails_on: Some(event),
                ..Self::new(name)
            }
        }
    }

    impl HostMod for Recording {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), HostError> {
            for key in &self.claims {
                registrar
                    .claim(key.clone())
                    .map_err(|refused| HostError::Refused {
                        mod_id: self.id.clone(),
                        state: LifecycleState::Constructed,
                        detail: refused.to_string(),
                    })?;
            }
            Ok(())
        }

        fn observe(&mut self, event: GameEvent) -> Result<(), HostError> {
            if self.fails_on == Some(event) {
                return Err(HostError::Refused {
                    mod_id: self.id.clone(),
                    state: LifecycleState::Running,
                    detail: format!("`{event}` is more than it can take"),
                });
            }
            self.seen.push(event);
            Ok(())
        }

        fn run_command(&mut self, invocation: &CommandInvocation) -> Result<(), HostError> {
            self.ran.push(invocation.clone());
            Ok(())
        }
    }

    /// Drives `entries` to `Running`, letting `mods` register on the way.
    fn running(
        entries: Vec<LifecycleEntry>,
        side: Side,
        mods: &mut [&mut dyn HostMod],
    ) -> LifecycleMachine {
        let mut machine = LifecycleMachine::new(side, entries).expect("a legal plan");
        for state in [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
        ] {
            machine.advance_all(state).expect("a legal step");
        }
        let order: Vec<ModId> = machine.order().cloned().collect();
        for mod_id in order {
            let mut window = machine
                .registration_window(&mod_id)
                .expect("the mod is in the plan");
            if let Some(host) = mods.iter_mut().find(|host| host.id() == &mod_id) {
                host.register(&mut window).expect("nothing conflicts");
            }
        }
        for state in [LifecycleState::Registered, LifecycleState::Running] {
            machine.advance_all(state).expect("a legal step");
        }
        machine
    }

    #[test]
    fn a_broadcast_reaches_every_mod_in_resolved_order() {
        let mut first = Recording::new("first_mod");
        let mut second = Recording::new("second_mod");
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut first, &mut second];
        let machine = running(
            vec![
                entry("first_mod", &[Side::Client]),
                entry("second_mod", &[Side::Client]),
            ],
            Side::Client,
            &mut mods,
        );

        let delivery = broadcast(&machine, &mut mods, GameEvent::ClientTick).expect("the set runs");
        assert_eq!(delivery.received(), [id("first_mod"), id("second_mod")]);
        assert!(delivery.is_complete());
        assert_eq!(delivery.failure(), None);

        // The order is the audience's, and the audience is the plan's.
        assert_eq!(
            machine.audience(GameEvent::ClientTick),
            [id("first_mod"), id("second_mod")]
        );
        assert_eq!(first.seen, [GameEvent::ClientTick]);
        assert_eq!(second.seen, [GameEvent::ClientTick]);
    }

    #[test]
    fn an_event_that_cannot_happen_on_this_side_reaches_nobody() {
        let mut only = Recording::new("server_mod");
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut only];
        let machine = running(
            vec![entry("server_mod", &[Side::DedicatedServer])],
            Side::DedicatedServer,
            &mut mods,
        );

        assert!(machine.audience(GameEvent::ClientTick).is_empty());
        let delivery =
            broadcast(&machine, &mut mods, GameEvent::ClientTick).expect("an empty audience");
        assert!(delivery.received().is_empty());
        assert!(delivery.is_complete());

        // A server tick on the same set is delivered, so the empty audience
        // above is the event's doing and not the set's.
        let ticked = broadcast(&machine, &mut mods, GameEvent::ServerTick).expect("the set runs");
        assert_eq!(ticked.received(), [id("server_mod")]);

        drop(mods);
        // The client tick reached nobody, and the server tick reached this mod,
        // so the one event it saw is the one that happens on this side.
        assert_eq!(only.seen, [GameEvent::ServerTick]);
    }

    #[test]
    fn a_mod_that_does_not_load_here_is_left_out_of_the_audience() {
        // The client-only mod is not driven, because the machine refuses to
        // advance it onto a dedicated server at all. What is asserted is that
        // the audience already knows that before anything is delivered.
        let machine = LifecycleMachine::new(
            Side::DedicatedServer,
            vec![
                entry("both_mod", &[Side::Client, Side::DedicatedServer]),
                entry("client_mod", &[Side::Client]),
            ],
        )
        .expect("a legal plan");

        assert_eq!(machine.audience(GameEvent::ServerTick), [id("both_mod")]);
    }

    #[test]
    fn a_broadcast_before_the_set_runs_is_refused_and_reaches_nobody() {
        let mut only = Recording::new("example_mod");
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut only];
        let mut machine =
            LifecycleMachine::new(Side::Client, vec![entry("example_mod", &[Side::Client])])
                .expect("a legal plan");
        machine
            .advance_all(LifecycleState::Validated)
            .expect("a legal step");

        let refused = broadcast(&machine, &mut mods, GameEvent::ClientTick)
            .expect_err("the set has not registered");
        assert_eq!(
            refused,
            DispatchRefused::NotReacting {
                mod_id: id("example_mod"),
                state: LifecycleState::Validated,
            }
        );
        assert!(only.seen.is_empty());
        assert_eq!(refused.failure_kind(), None);
        assert!(!refused.quotes_untrusted_text());
    }

    #[test]
    fn a_mod_that_fails_ends_the_broadcast_and_the_ones_before_it_still_reacted() {
        let mut first = Recording::new("first_mod");
        let mut second = Recording::failing("second_mod", GameEvent::ClientTick);
        let mut third = Recording::new("third_mod");
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut first, &mut second, &mut third];
        let machine = running(
            vec![
                entry("first_mod", &[Side::Client]),
                entry("second_mod", &[Side::Client]),
                entry("third_mod", &[Side::Client]),
            ],
            Side::Client,
            &mut mods,
        );

        let delivery = broadcast(&machine, &mut mods, GameEvent::ClientTick)
            .expect("the set runs, so the delivery happened");
        assert_eq!(delivery.received(), [id("first_mod")]);
        assert!(!delivery.is_complete());
        match delivery.failure() {
            Some(HostError::Refused { mod_id, .. }) => assert_eq!(mod_id, &id("second_mod")),
            other => panic!("expected the second mod to refuse, got {other:?}"),
        }

        // The third mod is never asked, so it cannot act on a set that failed.
        assert!(third.seen.is_empty());
    }

    #[test]
    fn a_broadcast_missing_a_supplied_value_is_refused_before_anyone_reacts() {
        let mut first = Recording::new("first_mod");
        let mut second = Recording::new("second_mod");
        let mut both: Vec<&mut dyn HostMod> = vec![&mut first, &mut second];
        let machine = running(
            vec![
                entry("first_mod", &[Side::Client]),
                entry("second_mod", &[Side::Client]),
            ],
            Side::Client,
            &mut both,
        );
        drop(both);

        let mut only: Vec<&mut dyn HostMod> = vec![&mut first];
        let refused = broadcast(&machine, &mut only, GameEvent::ClientTick)
            .expect_err("the plan names a mod that was not supplied");
        assert_eq!(
            refused,
            DispatchRefused::Absent {
                mod_id: id("second_mod"),
            }
        );
        // The first mod is in the audience and ahead of the missing one, and it
        // still saw nothing: the whole set is checked before anyone reacts.
        assert!(first.seen.is_empty());
    }

    #[test]
    fn a_command_is_routed_to_its_owner_and_to_nobody_else() {
        let owned = command("first_mod", "home");
        let mut first = Recording::claiming("first_mod", vec![owned.clone()]);
        let mut second = Recording::new("second_mod");
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut first, &mut second];
        let machine = running(
            vec![
                entry("first_mod", &[Side::Client]),
                entry("second_mod", &[Side::Client]),
            ],
            Side::Client,
            &mut mods,
        );

        let invocation = CommandInvocation::new(
            owned.clone(),
            Side::Client,
            vec!["set".to_owned(), "base".to_owned()],
        );
        let delivery = route(&machine, &mut mods, &invocation).expect("the owner runs");
        assert_eq!(delivery.received(), [id("first_mod")]);
        assert!(delivery.is_complete());

        assert_eq!(first.ran, [invocation]);
        assert!(second.ran.is_empty());
    }

    #[test]
    fn a_command_nobody_claimed_names_the_key_rather_than_a_mod() {
        let mut only = Recording::new("example_mod");
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut only];
        let machine = running(
            vec![entry("example_mod", &[Side::Client])],
            Side::Client,
            &mut mods,
        );

        let orphan = command("example_mod", "nowhere");
        let refused = route(
            &machine,
            &mut mods,
            &CommandInvocation::new(orphan.clone(), Side::Client, Vec::new()),
        )
        .expect_err("nothing claimed it");
        assert_eq!(refused, DispatchRefused::Unclaimed { key: orphan });
        assert!(refused.to_string().contains("no mod claimed"));
        assert!(only.ran.is_empty());
    }

    #[test]
    fn a_command_whose_owner_has_not_started_is_refused_naming_the_owner() {
        let owned = command("example_mod", "home");
        let mut only = Recording::claiming("example_mod", vec![owned.clone()]);

        let mut machine =
            LifecycleMachine::new(Side::Client, vec![entry("example_mod", &[Side::Client])])
                .expect("a legal plan");
        for state in [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
        ] {
            machine.advance_all(state).expect("a legal step");
        }
        {
            let mut window = machine
                .registration_window(&id("example_mod"))
                .expect("in the plan");
            only.register(&mut window).expect("nothing conflicts");
        }

        // The command is claimed and the set never reached `Running`.
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut only];
        let refused = route(
            &machine,
            &mut mods,
            &CommandInvocation::new(owned, Side::Client, Vec::new()),
        )
        .expect_err("the set has not started");
        assert_eq!(
            refused,
            DispatchRefused::OwnerNotReacting {
                mod_id: id("example_mod"),
                state: LifecycleState::Constructed,
            }
        );
        assert!(only.ran.is_empty());
    }

    #[test]
    fn a_mod_that_refuses_its_own_command_reports_the_failure_and_no_recipient() {
        /// A mod that claims a command and then will not run it.
        struct Claiming {
            id: ModId,
            key: RegistryKey,
        }

        impl HostMod for Claiming {
            fn id(&self) -> &ModId {
                &self.id
            }

            fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
                Ok(())
            }

            fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), HostError> {
                registrar
                    .claim(self.key.clone())
                    .map_err(|refused| HostError::Refused {
                        mod_id: self.id.clone(),
                        state: LifecycleState::Constructed,
                        detail: refused.to_string(),
                    })
            }
        }

        let owned = command("example_mod", "home");
        let mut only = Claiming {
            id: id("example_mod"),
            key: owned.clone(),
        };
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut only];
        let machine = running(
            vec![entry("example_mod", &[Side::Client])],
            Side::Client,
            &mut mods,
        );

        let delivery = route(
            &machine,
            &mut mods,
            &CommandInvocation::new(owned, Side::Client, Vec::new()),
        )
        .expect("the owner runs, so routing succeeded");
        assert!(delivery.received().is_empty());
        // A mod that claims a command name and does not implement it has told a
        // player the command exists, so the default is a refusal and not a
        // silent success.
        assert_eq!(
            delivery.failure(),
            Some(&HostError::Unsupported {
                mod_id: id("example_mod"),
                state: LifecycleState::Running,
            })
        );
    }

    #[test]
    fn every_refusal_says_which_state_it_is_about() {
        let key = command("example_mod", "home");
        let cases = [
            (
                DispatchRefused::NotReacting {
                    mod_id: id("example_mod"),
                    state: LifecycleState::Loaded,
                },
                LifecycleState::Loaded,
            ),
            (
                DispatchRefused::OwnerNotReacting {
                    mod_id: id("example_mod"),
                    state: LifecycleState::Stopped,
                },
                LifecycleState::Stopped,
            ),
            (
                DispatchRefused::Unclaimed { key: key.clone() },
                LifecycleState::Running,
            ),
            (
                DispatchRefused::OwnerWrongSide {
                    mod_id: id("example_mod"),
                },
                LifecycleState::Running,
            ),
            (
                DispatchRefused::Absent {
                    mod_id: id("example_mod"),
                },
                LifecycleState::Running,
            ),
        ];

        for (refused, state) in cases {
            assert_eq!(refused.lifecycle_state(), state);
            assert_eq!(refused.failure_kind(), None);
            assert!(!refused.quotes_untrusted_text());
            assert!(!refused.to_string().is_empty());
        }
    }

    #[test]
    fn a_delivery_that_reached_nobody_is_still_complete_when_nothing_failed() {
        let empty = Delivery {
            received: Vec::new(),
            failure: None,
        };
        assert!(empty.is_complete());
        assert!(empty.received().is_empty());
        assert_eq!(empty.failure(), None);
    }

    #[test]
    fn every_event_happens_somewhere_and_a_client_tick_happens_only_on_the_client() {
        for event in GameEvent::ALL {
            assert!(
                [Side::Client, Side::IntegratedServer, Side::DedicatedServer]
                    .into_iter()
                    .any(|side| event.happens_on(side)),
                "{event} happens nowhere"
            );
            assert!(!event.as_str().is_empty());
            assert_eq!(event.to_string(), event.as_str());
        }

        assert!(GameEvent::ClientTick.happens_on(Side::Client));
        assert!(!GameEvent::ClientTick.happens_on(Side::DedicatedServer));
        assert!(GameEvent::ServerTick.happens_on(Side::IntegratedServer));
        assert!(!GameEvent::ServerTick.happens_on(Side::Client));
    }
}
