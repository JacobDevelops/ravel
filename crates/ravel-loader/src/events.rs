use std::fmt;

use ravel_core::{FailureKind, LifecycleState, ModId, RegistryKey, Side, StructuredFailure};
use ravel_host_api::{CommandInvocation, GameEvent, HostError, HostMod};
use ravel_network::{Direction, PacketDelivery, PacketId};
use ravel_observability::{CallKind, Profiler};

use crate::lifecycle::LifecycleMachine;

/// Only running and stopping mods may react.
const fn reacts(state: LifecycleState) -> bool {
    matches!(state, LifecycleState::Running | LifecycleState::Stopping)
}

/// Why an event or a command reached no mod.
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
    OwnerWrongSide {
        /// The mod that owns the command.
        mod_id: ModId,
    },
    /// The delivery's own side field disagrees with the side the set runs on.
    PacketForeignSide {
        /// The contract that was refused.
        id: PacketId,
        /// The side the delivery claimed to arrive on.
        claimed: Side,
        /// The side the set is running on.
        running: Side,
    },
    /// The packet travels a way this side is not an end of.
    PacketWrongSide {
        /// The contract that was refused.
        id: PacketId,
        /// The way it travels.
        direction: Direction,
        /// The side it arrived on.
        side: Side,
    },
    /// The set holds no value for a mod the order names.
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
            Self::PacketForeignSide {
                id,
                claimed,
                running,
            } => write!(
                formatter,
                "`{id}` arrived claiming the `{}` side and the set is running as `{}`",
                claimed.name(),
                running.name()
            ),
            Self::PacketWrongSide {
                id,
                direction,
                side,
            } => write!(
                formatter,
                "`{id}` travels {direction} and the `{}` side is not an end of that",
                side.name()
            ),
            Self::Absent { mod_id } => {
                write!(formatter, "the order names `{mod_id}` and no value for it")
            }
        }
    }
}

impl std::error::Error for DispatchRefused {}

impl StructuredFailure for DispatchRefused {
    /// Dispatch refusals are not dependency-graph failures.
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    fn lifecycle_state(&self) -> LifecycleState {
        match self {
            Self::NotReacting { state, .. } | Self::OwnerNotReacting { state, .. } => *state,
            Self::Unclaimed { .. }
            | Self::OwnerWrongSide { .. }
            | Self::PacketForeignSide { .. }
            | Self::PacketWrongSide { .. }
            | Self::Absent { .. } => LifecycleState::Running,
        }
    }

    /// Messages contain only validated identifiers and loader-owned text.
    fn quotes_untrusted_text(&self) -> bool {
        false
    }
}

/// Successful recipients in resolved order and any mod failure that stopped delivery.
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
    /// Returns side-compatible recipients of `event`, in resolved order.
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
    fn loads_here(&self, mod_id: &ModId) -> bool {
        self.entry(mod_id)
            .is_ok_and(|entry| entry.sides.loads_on(self.side()))
    }
}

/// Broadcasts `event` in resolved order after validating the entire audience.
///
/// Mod failures stop delivery and are returned in [`Delivery::failure`].
///
/// # Errors
///
/// Returns [`DispatchRefused`] if an audience member is not reacting or was not supplied.
///
/// # Panics
///
/// Never; all audience members are checked before delivery.
pub fn broadcast(
    machine: &LifecycleMachine,
    mods: &mut [&mut (dyn HostMod + '_)],
    event: GameEvent,
) -> Result<Delivery, DispatchRefused> {
    broadcast_profiled(machine, mods, event, &mut Profiler::off())
}

/// Broadcasts `event`, attributing each mod's cost to `profiler`.
///
/// # Errors
///
/// The same as [`broadcast`].
///
/// # Panics
///
/// Never; all audience members are checked before delivery.
pub fn broadcast_profiled(
    machine: &LifecycleMachine,
    mods: &mut [&mut (dyn HostMod + '_)],
    event: GameEvent,
    profiler: &mut Profiler,
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
        let observed = profiler.measure(&mod_id, CallKind::Event, || host.observe(event));
        if let Err(failure) = observed {
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

/// Routes `delivery` to its owner after checking the claimed side and direction.
///
/// # Errors
///
/// Returns [`DispatchRefused`] for incompatible sides, an absent owner, or an owner not reacting.
pub fn deliver_packet(
    machine: &LifecycleMachine,
    mods: &mut [&mut (dyn HostMod + '_)],
    delivery: &PacketDelivery,
) -> Result<Delivery, DispatchRefused> {
    if delivery.side != machine.side() {
        return Err(DispatchRefused::PacketForeignSide {
            id: delivery.id.clone(),
            claimed: delivery.side,
            running: machine.side(),
        });
    }
    if !delivery.direction.may_receive_on(machine.side()) {
        return Err(DispatchRefused::PacketWrongSide {
            id: delivery.id.clone(),
            direction: delivery.direction,
            side: machine.side(),
        });
    }

    let owner = delivery.id.owner().clone();
    let state = machine.state(&owner).map_err(|_| DispatchRefused::Absent {
        mod_id: owner.clone(),
    })?;
    if !machine.loads_here(&owner) {
        return Err(DispatchRefused::OwnerWrongSide { mod_id: owner });
    }
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

    match host.receive_packet(delivery) {
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

/// Routes `invocation` to the one mod that claimed its root literal.
///
/// # Errors
///
/// Returns [`DispatchRefused`] if the command is unclaimed or its owner is absent, on another side, or not reacting.
pub fn route(
    machine: &LifecycleMachine,
    mods: &mut [&mut (dyn HostMod + '_)],
    invocation: &CommandInvocation,
) -> Result<Delivery, DispatchRefused> {
    route_profiled(machine, mods, invocation, &mut Profiler::off())
}

/// Routes `invocation`, attributing its owner's cost to `profiler`.
///
/// # Errors
///
/// The same as [`route`].
pub fn route_profiled(
    machine: &LifecycleMachine,
    mods: &mut [&mut (dyn HostMod + '_)],
    invocation: &CommandInvocation,
    profiler: &mut Profiler,
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

    match profiler.measure(&owner, CallKind::Command, || host.run_command(invocation)) {
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
    use ravel_network::{Direction, PacketDelivery, PacketId};

    use super::{Delivery, DispatchRefused, broadcast, deliver_packet, route};
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

        assert_eq!(
            machine.audience(GameEvent::ClientTick),
            [] as [ravel_core::ModId; 0]
        );
        let delivery =
            broadcast(&machine, &mut mods, GameEvent::ClientTick).expect("an empty audience");
        assert_eq!(delivery.received(), []);
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
        assert_eq!(only.seen, [] as [ravel_host_api::GameEvent; 0]);
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
        assert_eq!(third.seen, [] as [ravel_host_api::GameEvent; 0]);
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
        assert_eq!(first.seen, [] as [ravel_host_api::GameEvent; 0]);
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
        assert_eq!(second.ran, [] as [ravel_host_api::CommandInvocation; 0]);
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
        assert_eq!(only.ran, [] as [ravel_host_api::CommandInvocation; 0]);
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
        assert_eq!(only.ran, [] as [ravel_host_api::CommandInvocation; 0]);
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
        assert_eq!(delivery.received(), []);
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
            assert_ne!(refused.to_string(), "");
        }
    }

    #[test]
    fn a_delivery_that_reached_nobody_is_still_complete_when_nothing_failed() {
        let empty = Delivery {
            received: Vec::new(),
            failure: None,
        };
        assert!(empty.is_complete());
        assert_eq!(empty.received(), []);
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
            assert_ne!(event.as_str(), "");
            assert_eq!(event.to_string(), event.as_str());
        }

        assert!(GameEvent::ClientTick.happens_on(Side::Client));
        assert!(!GameEvent::ClientTick.happens_on(Side::DedicatedServer));
        assert!(GameEvent::ServerTick.happens_on(Side::IntegratedServer));
        assert!(!GameEvent::ServerTick.happens_on(Side::Client));
    }

    // ------------------------------------------------------------- packets ---

    /// A mod that records the packets routed to it, and can refuse one.
    struct Networked {
        id: ModId,
        received: Vec<PacketId>,
        refuses: bool,
    }

    impl Networked {
        fn new(name: &str) -> Self {
            Self {
                id: id(name),
                received: Vec::new(),
                refuses: false,
            }
        }

        fn refusing(name: &str) -> Self {
            Self {
                refuses: true,
                ..Self::new(name)
            }
        }
    }

    impl HostMod for Networked {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn receive_packet(&mut self, delivery: &PacketDelivery) -> Result<(), HostError> {
            if self.refuses {
                return Err(HostError::Refused {
                    mod_id: self.id.clone(),
                    state: LifecycleState::Running,
                    detail: format!("`{}` is refused by this instance", delivery.id),
                });
            }
            self.received.push(delivery.id.clone());
            Ok(())
        }
    }

    fn packet_id(text: &str) -> PacketId {
        PacketId::new(text.parse().expect("a legal content id"))
    }

    /// A delivery of `direction` addressed to `id`, with a body nothing decodes.
    ///
    /// The body is empty because every test here is about routing. What a mod
    /// makes of the bytes is `ravel-network`'s own tests and the conformance
    /// suite's, and a payload here would only make these tests depend on a
    /// contract they do not care about.
    fn delivery(id: &str, direction: Direction, side: Side) -> PacketDelivery {
        PacketDelivery::new(packet_id(id), direction, side, Vec::new())
    }

    #[test]
    fn a_packet_reaches_the_mod_that_owns_its_contract_and_nobody_else() {
        let mut first = Networked::new("first_mod");
        let mut second = Networked::new("second_mod");
        let machine = running(
            vec![
                entry("first_mod", &[Side::DedicatedServer]),
                entry("second_mod", &[Side::DedicatedServer]),
            ],
            Side::DedicatedServer,
            &mut [&mut first, &mut second],
        );

        let sent = delivery(
            "second_mod:ping",
            Direction::ToServer,
            Side::DedicatedServer,
        );
        let delivered = deliver_packet(&machine, &mut [&mut first, &mut second], &sent)
            .expect("the owner is running here");

        assert_eq!(delivered.received(), [id("second_mod")]);
        assert!(delivered.is_complete());
        assert_eq!(second.received, [packet_id("second_mod:ping")]);
        assert_eq!(first.received, [] as [ravel_network::PacketId; 0]);
    }

    #[test]
    fn a_delivery_claiming_another_side_than_the_set_runs_on_reaches_nobody() {
        // A legitimate server-bound packet, arriving on a dedicated server, whose
        // own `side` field says `client`. The direction is right for both sides
        // involved, so nothing but the disagreement can refuse it — and it must be
        // refused here rather than routed on the machine's answer and then read on
        // the delivery's, which is what produced a valid packet being refused
        // inside the mod for a reason that was not true.
        let mut host = Networked::new("example_mod");
        let machine = running(
            vec![entry("example_mod", &Side::ALL)],
            Side::DedicatedServer,
            &mut [&mut host],
        );

        let sent = delivery("example_mod:ping", Direction::ToServer, Side::Client);
        let refusal = deliver_packet(&machine, &mut [&mut host], &sent)
            .expect_err("the delivery claims a side the set is not running as");

        assert_eq!(
            refusal,
            DispatchRefused::PacketForeignSide {
                id: packet_id("example_mod:ping"),
                claimed: Side::Client,
                running: Side::DedicatedServer,
            }
        );
        // Both sides are named, so a reader can tell which end set it wrong.
        let rendered = refusal.to_string();
        assert!(rendered.contains(Side::Client.name()), "{rendered}");
        assert!(
            rendered.contains(Side::DedicatedServer.name()),
            "{rendered}"
        );
        assert!(host.received.is_empty(), "the mod was handed the delivery");

        // The same delivery with its side field agreeing does reach the mod, so
        // the refusal above is about the disagreement and nothing else.
        let agreeing = delivery(
            "example_mod:ping",
            Direction::ToServer,
            Side::DedicatedServer,
        );
        deliver_packet(&machine, &mut [&mut host], &agreeing).expect("the sides agree");
        assert_eq!(host.received, [packet_id("example_mod:ping")]);
    }

    #[test]
    fn a_client_bound_packet_on_a_server_side_never_reaches_the_mod() {
        // The owner loads on every side and is running, so nothing but the
        // direction can refuse this. That is the point: `SPEC.md` §14's rule holds
        // for a mod that is present rather than only for one the graph parked.
        let mut host = Networked::new("example_mod");
        let machine = running(
            vec![entry("example_mod", &Side::ALL)],
            Side::DedicatedServer,
            &mut [&mut host],
        );

        let sent = delivery(
            "example_mod:screen",
            Direction::ToClient,
            Side::DedicatedServer,
        );
        let refusal = deliver_packet(&machine, &mut [&mut host], &sent)
            .expect_err("a dedicated server is not an end of a client-bound link");

        assert_eq!(
            refusal,
            DispatchRefused::PacketWrongSide {
                id: packet_id("example_mod:screen"),
                direction: Direction::ToClient,
                side: Side::DedicatedServer,
            }
        );
        assert!(
            refusal.to_string().contains("dedicated-server"),
            "{refusal}"
        );
        assert_eq!(refusal.lifecycle_state(), LifecycleState::Running);
        assert!(refusal.failure_kind().is_none());
        assert!(
            host.received.is_empty(),
            "the mod read a client-bound packet"
        );
    }

    #[test]
    fn the_same_packet_reaches_the_client_when_the_client_is_the_side_running() {
        // The negative control for the test above. If any client-bound packet were
        // refused, that test would pass without the direction being compared.
        let mut host = Networked::new("example_mod");
        let machine = running(
            vec![entry("example_mod", &Side::ALL)],
            Side::Client,
            &mut [&mut host],
        );

        let sent = delivery("example_mod:screen", Direction::ToClient, Side::Client);
        deliver_packet(&machine, &mut [&mut host], &sent).expect("the client is the receiving end");
        assert_eq!(host.received, [packet_id("example_mod:screen")]);
    }

    #[test]
    fn a_packet_owned_by_a_mod_that_does_not_load_here_is_refused_rather_than_swallowed() {
        let mut host = Networked::new("server_only");
        let machine = running(
            vec![entry("server_only", &[Side::DedicatedServer])],
            Side::IntegratedServer,
            &mut [&mut host],
        );

        // A server-bound packet may be received on an integrated server, so the
        // direction passes and the owner's side declaration is what refuses.
        let sent = delivery(
            "server_only:ping",
            Direction::ToServer,
            Side::IntegratedServer,
        );
        assert_eq!(
            deliver_packet(&machine, &mut [&mut host], &sent),
            Err(DispatchRefused::OwnerWrongSide {
                mod_id: id("server_only"),
            })
        );
        assert_eq!(host.received, [] as [ravel_network::PacketId; 0]);
    }

    #[test]
    fn a_packet_naming_a_mod_the_plan_does_not_hold_is_refused_as_absent() {
        let mut host = Networked::new("example_mod");
        let machine = running(
            vec![entry("example_mod", &Side::ALL)],
            Side::DedicatedServer,
            &mut [&mut host],
        );

        let sent = delivery("other_mod:ping", Direction::ToServer, Side::DedicatedServer);
        assert_eq!(
            deliver_packet(&machine, &mut [&mut host], &sent),
            Err(DispatchRefused::Absent {
                mod_id: id("other_mod"),
            })
        );
    }

    #[test]
    fn a_packet_before_the_set_reaches_running_is_refused_with_the_state_it_found() {
        let mut host = Networked::new("example_mod");
        let mut machine = LifecycleMachine::new(
            Side::DedicatedServer,
            vec![entry("example_mod", &Side::ALL)],
        )
        .expect("a legal plan");
        machine
            .advance_all(LifecycleState::Validated)
            .expect("a legal step");

        let sent = delivery(
            "example_mod:ping",
            Direction::ToServer,
            Side::DedicatedServer,
        );
        assert_eq!(
            deliver_packet(&machine, &mut [&mut host], &sent),
            Err(DispatchRefused::OwnerNotReacting {
                mod_id: id("example_mod"),
                state: LifecycleState::Validated,
            })
        );
        assert_eq!(host.received, [] as [ravel_network::PacketId; 0]);
    }

    #[test]
    fn a_set_that_holds_no_value_for_the_owner_delivers_to_nobody() {
        let mut present = Networked::new("present_mod");
        let machine = running(
            vec![
                entry("present_mod", &Side::ALL),
                entry("absent_mod", &Side::ALL),
            ],
            Side::DedicatedServer,
            &mut [&mut present],
        );

        let sent = delivery(
            "absent_mod:ping",
            Direction::ToServer,
            Side::DedicatedServer,
        );
        assert_eq!(
            deliver_packet(&machine, &mut [&mut present], &sent),
            Err(DispatchRefused::Absent {
                mod_id: id("absent_mod"),
            })
        );
    }

    #[test]
    fn a_mod_that_refuses_a_packet_reports_the_refusal_and_reaches_nobody() {
        let mut host = Networked::refusing("example_mod");
        let machine = running(
            vec![entry("example_mod", &Side::ALL)],
            Side::DedicatedServer,
            &mut [&mut host],
        );

        let sent = delivery(
            "example_mod:ping",
            Direction::ToServer,
            Side::DedicatedServer,
        );
        let delivered = deliver_packet(&machine, &mut [&mut host], &sent)
            .expect("the routing succeeded and the mod declined");

        assert_eq!(delivered.received(), []);
        assert!(!delivered.is_complete());
        let failure = delivered.failure().expect("the mod refused");
        assert_eq!(failure.mod_id(), &id("example_mod"));
        assert!(
            failure.to_string().contains("example_mod:ping"),
            "{failure}"
        );
    }
}
