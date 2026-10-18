use std::fmt;

use ravel_core::{FailureKind, LifecycleState, ModId, RegistryKey, Side, StructuredFailure};
use ravel_host_api::{CommandInvocation, GameEvent, HostError, HostMod};
use ravel_network::{Direction, PacketDelivery, PacketId};
use ravel_observability::{CallKind, Profiler};

use crate::lifecycle::LifecycleMachine;
use crate::loaded::LoadedMods;

type DrainedPlayRequests = (Vec<ravel_data::PlayRequest>, Option<(ModId, HostError)>);

/// Only running and stopping mods may react.
const fn reacts(state: LifecycleState) -> bool {
    matches!(state, LifecycleState::Running | LifecycleState::Stopping)
}

/// Why an event or a command reached no mod.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchRefused {
    /// The set is not in a state that reacts.
    NotReacting {
        /// The halting mod, or the first non-reacting audience member in resolved order.
        mod_id: ModId,
        /// The state it is in.
        state: LifecycleState,
    },
    /// No mod claimed the command's root literal.
    Unclaimed {
        /// The key nobody owns.
        key: RegistryKey,
    },
    /// The supplied key identifies content other than a command.
    NotACommand {
        /// The non-command key that cannot be routed.
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
    /// The requested owner is not a member of the lifecycle plan.
    Absent {
        /// The owner absent from the lifecycle plan.
        mod_id: ModId,
    },
}

impl fmt::Display for DispatchRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotReacting { mod_id, state } => write!(
                formatter,
                "the set is `{}` at `{mod_id}`, which does not react",
                state.as_str()
            ),
            Self::Unclaimed { key } => write!(formatter, "no mod claimed `{key}`"),
            Self::NotACommand { key } => write!(formatter, "`{key}` is not a command"),
            Self::OwnerNotReacting { mod_id, state } => write!(
                formatter,
                "`{mod_id}` owns the command and is `{}`, which does not react",
                state.as_str()
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
                claimed.as_str(),
                running.as_str()
            ),
            Self::PacketWrongSide {
                id,
                direction,
                side,
            } => write!(
                formatter,
                "`{id}` travels {direction} and the `{}` side is not an end of that",
                side.as_str()
            ),
            Self::Absent { mod_id } => {
                write!(formatter, "the lifecycle plan has no owner `{mod_id}`")
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
            | Self::NotACommand { .. }
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
    failure: Option<(ModId, HostError)>,
}

impl Delivery {
    /// Returns the mods that reacted, in the order they were called.
    #[must_use]
    pub fn received(&self) -> &[ModId] {
        &self.received
    }

    /// Returns the failure that ended the delivery, when one did.
    #[must_use]
    pub fn failure(&self) -> Option<&HostError> {
        self.failure.as_ref().map(|(_, error)| error)
    }

    /// Returns the actual owner whose callback failed.
    #[must_use]
    pub fn failed_by(&self) -> Option<&ModId> {
        self.failure.as_ref().map(|(owner, _)| owner)
    }

    /// Returns whether every mod that should have reacted did so.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.failure.is_none()
    }
}

impl LifecycleMachine {
    /// Returns side-compatible recipients, excluding failed optional mods, in resolved order.
    #[must_use]
    pub fn audience(&self, event: GameEvent) -> Vec<ModId> {
        self.audience_entries(event)
            .map(|(entry, _)| entry.mod_id.clone())
            .collect()
    }

    fn audience_entries(
        &self,
        event: GameEvent,
    ) -> impl Iterator<Item = (&crate::LifecycleEntry, LifecycleState)> {
        self.tracked_entries().filter(move |(entry, state)| {
            event.happens_on(self.side())
                && entry.sides.loads_on(self.side())
                && (entry.requirement == crate::Requirement::Required
                    || *state != LifecycleState::Failed)
        })
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
/// Returns [`DispatchRefused`] if the set is halted or an audience member is not reacting.
///
/// # Panics
///
/// Never; all audience members are checked before delivery.
pub fn broadcast<H: std::ops::DerefMut<Target: HostMod>>(
    loaded: &mut LoadedMods<'_, H>,
    event: GameEvent,
) -> Result<Delivery, DispatchRefused> {
    broadcast_profiled(loaded, event, &mut Profiler::off())
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
pub fn broadcast_profiled<H: std::ops::DerefMut<Target: HostMod>>(
    loaded: &mut LoadedMods<'_, H>,
    event: GameEvent,
    profiler: &mut Profiler,
) -> Result<Delivery, DispatchRefused> {
    let (machine, mods) = loaded.parts();
    ensure_active(machine)?;
    let audience: Vec<_> = machine
        .audience_entries(event)
        .map(|(entry, state)| (entry.mod_id.clone(), state))
        .collect();
    for (mod_id, state) in &audience {
        if !reacts(*state) {
            return Err(DispatchRefused::NotReacting {
                mod_id: mod_id.clone(),
                state: *state,
            });
        }
    }

    let mut received = Vec::with_capacity(audience.len());
    for (mod_id, state) in audience {
        let host = mods
            .iter_mut()
            .find(|host| host.id() == &mod_id)
            .expect("LoadedMods checked every loading owner");
        let observed = profiler.measure(&mod_id, CallKind::Event, || {
            crate::callback(&mod_id, state, || host.observe(event))
        });
        if let Err(failure) = observed {
            return Ok(Delivery {
                received,
                failure: Some((mod_id, failure)),
            });
        }
        received.push(mod_id);
    }
    Ok(Delivery {
        received,
        failure: None,
    })
}

/// Drains outward play requests in lifecycle order, stopping at an attributable failure.
///
/// Failed, parked and nonexecuting owners are not called. Requests naming another
/// owner are refused before any request from that callback is exposed.
///
/// # Errors
///
/// Returns [`DispatchRefused`] if the set is halted.
pub fn take_play_requests<H: std::ops::DerefMut<Target: HostMod>>(
    loaded: &mut LoadedMods<'_, H>,
) -> Result<DrainedPlayRequests, DispatchRefused> {
    let (machine, mods) = loaded.parts();
    ensure_active(machine)?;
    let mut requests = Vec::new();
    for (entry, state) in machine.tracked_entries() {
        if !entry.sides.loads_on(machine.side()) || !reacts(state) {
            continue;
        }
        let mod_id = &entry.mod_id;
        let host = mods
            .iter_mut()
            .find(|host| host.id() == mod_id)
            .expect("LoadedMods checked every loading owner");
        let drained = crate::callback(mod_id, state, || Ok(host.take_play_requests()));
        let drained = match drained {
            Ok(drained) => drained,
            Err(error) => return Ok((requests, Some((mod_id.clone(), error)))),
        };
        if let Some(foreign) = drained.iter().find(|request| request.owner() != mod_id) {
            let error = HostError::Refused {
                mod_id: mod_id.clone(),
                state,
                detail: format!(
                    "outward play request belongs to `{}`, not `{mod_id}`",
                    foreign.owner()
                ),
            };
            return Ok((requests, Some((mod_id.clone(), error))));
        }
        requests.extend(drained);
    }
    Ok((requests, None))
}

/// Routes `delivery` to its owner after checking the claimed side and direction.
///
/// # Errors
///
/// Returns [`DispatchRefused`] for a halted set, incompatible sides, or an absent or inactive owner.
pub fn deliver_packet<H: std::ops::DerefMut<Target: HostMod>>(
    loaded: &mut LoadedMods<'_, H>,
    delivery: &PacketDelivery,
) -> Result<Delivery, DispatchRefused> {
    deliver_packet_profiled(loaded, delivery, &mut Profiler::off())
}

/// Routes a packet, attributing its receive callback as event cost to `profiler`.
///
/// # Errors
///
/// The same as [`deliver_packet`].
pub fn deliver_packet_profiled<H: std::ops::DerefMut<Target: HostMod>>(
    loaded: &mut LoadedMods<'_, H>,
    delivery: &PacketDelivery,
    profiler: &mut Profiler,
) -> Result<Delivery, DispatchRefused> {
    let machine = loaded.machine();
    ensure_active(machine)?;
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

    to_owner(loaded, delivery.id.owner().clone(), |owner, state, host| {
        profiler.measure(owner, CallKind::Event, || {
            crate::callback(owner, state, || host.receive_packet(delivery))
        })
    })
}

pub(crate) fn ensure_active(machine: &LifecycleMachine) -> Result<(), DispatchRefused> {
    machine.halted_by().map_or(Ok(()), |mod_id| {
        Err(DispatchRefused::NotReacting {
            mod_id: mod_id.clone(),
            state: LifecycleState::Failed,
        })
    })
}

fn to_owner<H: std::ops::DerefMut<Target: HostMod>>(
    loaded: &mut LoadedMods<'_, H>,
    owner: ModId,
    call: impl FnOnce(&ModId, LifecycleState, &mut H::Target) -> Result<(), HostError>,
) -> Result<Delivery, DispatchRefused> {
    let (machine, mods) = loaded.parts();
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
        .expect("LoadedMods checked every loading owner");
    match call(&owner, state, &mut **host) {
        Ok(()) => Ok(Delivery {
            received: vec![owner],
            failure: None,
        }),
        Err(error) => Ok(Delivery {
            received: Vec::new(),
            failure: Some((owner, error)),
        }),
    }
}

/// Routes `invocation` to the one mod that claimed its root literal.
///
/// # Errors
///
/// Returns [`DispatchRefused`] for a halted set, an invalid command key, or an absent or inactive owner.
pub fn route<H: std::ops::DerefMut<Target: HostMod>>(
    loaded: &mut LoadedMods<'_, H>,
    invocation: &CommandInvocation,
) -> Result<Delivery, DispatchRefused> {
    route_profiled(loaded, invocation, &mut Profiler::off())
}

/// Routes `invocation`, attributing its owner's cost to `profiler`.
///
/// # Errors
///
/// The same as [`route`].
pub fn route_profiled<H: std::ops::DerefMut<Target: HostMod>>(
    loaded: &mut LoadedMods<'_, H>,
    invocation: &CommandInvocation,
    profiler: &mut Profiler,
) -> Result<Delivery, DispatchRefused> {
    let machine = loaded.machine();
    ensure_active(machine)?;
    if invocation.key.kind() != ravel_core::ContentKind::Command {
        return Err(DispatchRefused::NotACommand {
            key: invocation.key.clone(),
        });
    }
    let owner = machine
        .registry_owner(&invocation.key)
        .ok_or_else(|| DispatchRefused::Unclaimed {
            key: invocation.key.clone(),
        })?
        .clone();

    to_owner(loaded, owner, |owner, state, host| {
        profiler.measure(owner, CallKind::Command, || {
            crate::callback(owner, state, || host.run_command(invocation))
        })
    })
}

#[cfg(test)]
mod tests {
    use ravel_core::{
        ContentId, ContentKind, LifecycleState, ModId, RegistryKey, Side, StructuredFailure,
    };
    use ravel_host_api::{CommandInvocation, GameEvent, HostError, HostMod, ModError, Registrar};
    use ravel_network::{Direction, PacketDelivery, PacketId};

    use super::{Delivery, DispatchRefused, broadcast, deliver_packet, route};
    use crate::lifecycle::{LifecycleEntry, LifecycleMachine, Requirement};
    use crate::loaded::LoadedMods;
    use crate::tests::{advance_through, entry};

    fn id(text: &str) -> ModId {
        text.parse().expect("a legal mod id")
    }

    fn command(namespace: &str, path: &str) -> RegistryKey {
        RegistryKey::new(
            ContentKind::Command,
            ContentId::new(id(namespace), path).expect("a legal content path"),
        )
    }

    /// A mod that records every event it saw and every command it ran.
    struct Recording {
        id: ModId,
        seen: Vec<GameEvent>,
        ran: Vec<CommandInvocation>,
        fails_on: Option<GameEvent>,
        claims: Vec<RegistryKey>,
        panics: bool,
    }

    impl Recording {
        fn new(name: &str) -> Self {
            Self {
                id: id(name),
                seen: Vec::new(),
                ran: Vec::new(),
                fails_on: None,
                claims: Vec::new(),
                panics: false,
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

    #[test]
    fn event_and_command_panics_name_the_called_owner() {
        let key = command("example_mod", "run");
        let mut host = Recording::claiming("example_mod", vec![key.clone()]);
        host.panics = true;
        let mut machine = running(
            vec![entry("example_mod", &Side::ALL)],
            Side::Client,
            &mut [&mut host],
        );
        let event = broadcast(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                .expect("all active hosts are supplied"),
            GameEvent::ClientTick,
        )
        .expect("dispatch itself is valid");
        assert_eq!(event.failed_by(), Some(&id("example_mod")));
        assert_eq!(
            event.failure().map(HostError::mod_id),
            Some(&id("example_mod"))
        );
        let invocation = CommandInvocation::new(key, Side::Client, Vec::new());
        let command = route(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                .expect("all active hosts are supplied"),
            &invocation,
        )
        .expect("dispatch itself is valid");
        assert_eq!(command.failed_by(), Some(&id("example_mod")));
        assert_eq!(host.seen, []);
        assert_eq!(host.ran, []);
    }

    #[test]
    fn packet_panics_name_the_called_owner() {
        let mut host = Networked::new("example_mod");
        host.panics = true;
        let mut machine = running(
            vec![entry("example_mod", &Side::ALL)],
            Side::Client,
            &mut [&mut host],
        );
        let packet = delivery("example_mod:ping", Direction::ToClient, Side::Client);
        let delivered = deliver_packet(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                .expect("all active hosts are supplied"),
            &packet,
        )
        .expect("dispatch itself is valid");
        assert_eq!(delivered.failed_by(), Some(&id("example_mod")));
        assert_eq!(host.received, []);
    }

    #[test]
    fn non_command_claims_never_invoke_command_code() {
        let key = RegistryKey::new(
            ContentKind::Item,
            ContentId::new(id("example_mod"), "item").expect("a valid item"),
        );
        let mut host = Recording::claiming("example_mod", vec![key.clone()]);
        let mut machine = running(
            vec![entry("example_mod", &Side::ALL)],
            Side::Client,
            &mut [&mut host],
        );
        let invocation = CommandInvocation::new(key.clone(), Side::Client, Vec::new());
        let refused = route(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                .expect("all active hosts are supplied"),
            &invocation,
        )
        .expect_err("items cannot be routed as commands");
        assert_eq!(refused, DispatchRefused::NotACommand { key: key.clone() });
        assert_eq!(refused.to_string(), format!("`{key}` is not a command"));
        assert_eq!(refused.lifecycle_state(), LifecycleState::Running);
        assert!(!refused.quotes_untrusted_text());
        assert_eq!(host.ran, []);
    }

    #[test]
    fn panicking_commands_are_charged_to_the_owner() {
        let key = command("example_mod", "run");
        let mut host = Recording::claiming("example_mod", vec![key.clone()]);
        host.panics = true;
        let mut machine = running(
            vec![entry("example_mod", &Side::ALL)],
            Side::Client,
            &mut [&mut host],
        );
        let mut profiler = ravel_observability::Profiler::on(std::time::Duration::ZERO);
        let invocation = CommandInvocation::new(key, Side::Client, Vec::new());
        let delivered = super::route_profiled(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                .expect("all active hosts are supplied"),
            &invocation,
            &mut profiler,
        )
        .expect("the command is routable");
        assert_eq!(delivered.failed_by(), Some(&id("example_mod")));
        let report = profiler.report();
        assert_eq!(report.mods().len(), 1);
        assert_eq!(report.mods()[0].cost.command_calls, 1);
        assert_eq!(report.mods()[0].cost.slow_callbacks, 1);
    }

    #[test]
    fn second_broadcast_recipient_panics_are_attributed_to_that_recipient() {
        let mut first = Recording::new("first_mod");
        let mut second = Recording::new("second_mod");
        second.panics = true;
        let mut machine = running(
            vec![
                entry("first_mod", &Side::ALL),
                entry("second_mod", &Side::ALL),
            ],
            Side::Client,
            &mut [&mut first, &mut second],
        );
        let delivered = broadcast(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut first, &mut second])
                .expect("all active hosts are supplied"),
            GameEvent::ClientTick,
        )
        .expect("the audience is running");
        assert_eq!(delivered.received(), &[id("first_mod")]);
        assert_eq!(delivered.failed_by(), Some(&id("second_mod")));
        assert_eq!(
            delivered.failure().map(HostError::mod_id),
            Some(&id("second_mod"))
        );
    }

    #[test]
    fn stopping_event_panics_retain_the_owners_current_state() {
        let mut host = Recording::new("example_mod");
        host.panics = true;
        let mut machine = running(
            vec![entry("example_mod", &Side::ALL)],
            Side::Client,
            &mut [&mut host],
        );
        machine
            .advance_all(LifecycleState::Stopping)
            .expect("running mods may stop");
        let delivered = broadcast(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                .expect("all active hosts are supplied"),
            GameEvent::ClientTick,
        )
        .expect("stopping mods may react");
        assert_eq!(
            delivered.failure(),
            Some(&HostError::Panicked {
                mod_id: id("example_mod"),
                state: LifecycleState::Stopping,
            })
        );
    }

    #[test]
    fn halted_sets_refuse_claimed_commands_and_non_command_keys_before_dispatch() {
        let key = command("healthy_mod", "run");
        let mut failed = Recording::new("required_mod");
        let mut healthy = Recording::claiming("healthy_mod", vec![key.clone()]);
        let mut machine = running(
            vec![
                entry("required_mod", &Side::ALL),
                entry("healthy_mod", &Side::ALL),
            ],
            Side::Client,
            &mut [&mut healthy, &mut failed],
        );
        machine
            .record_world_mutation()
            .expect("a running set can mutate the world");
        machine
            .fail(
                &id("required_mod"),
                failure_attribution("required_mod", "required mod failed"),
            )
            .expect("a running mod can fail");
        assert_eq!(
            machine.state(&id("healthy_mod")),
            Ok(LifecycleState::Running)
        );
        let item = RegistryKey::new(
            ContentKind::Item,
            ContentId::new(id("healthy_mod"), "item").expect("valid item"),
        );
        for key in [key, item] {
            let invocation = CommandInvocation::new(key, Side::Client, Vec::new());
            assert_eq!(
                route(
                    &mut LoadedMods::<&mut dyn HostMod>::new(
                        &mut machine,
                        &mut [&mut healthy, &mut failed]
                    )
                    .expect("all active hosts are supplied"),
                    &invocation
                ),
                Err(DispatchRefused::NotReacting {
                    mod_id: id("required_mod"),
                    state: LifecycleState::Failed,
                }),
            );
        }
        assert_eq!(healthy.ran, []);
    }

    impl HostMod for Recording {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
            for key in &self.claims {
                registrar
                    .claim(key.clone())
                    .map_err(ModError::from_display)?;
            }
            Ok(())
        }

        fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
            assert!(!self.panics, "mod callback panic");
            if self.fails_on == Some(event) {
                return Err(ModError::refused(format!(
                    "`{event}` is more than it can take"
                )));
            }
            self.seen.push(event);
            Ok(())
        }

        fn run_command(&mut self, invocation: &CommandInvocation) -> Result<(), ModError> {
            assert!(!self.panics, "mod callback panic");
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
        advance_through(&mut machine, LifecycleState::Constructed);
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

    fn failure_attribution(name: &str, message: &str) -> ravel_core::FailureAttribution {
        ravel_core::FailureAttribution {
            mod_id: Some(id(name)),
            version: None,
            dependency_chain: Vec::new(),
            state: LifecycleState::Running,
            kind: None,
            surface: None,
            environment: ravel_core::FailureEnvironment {
                ravel: "0.1.0".parse().expect("valid version"),
                minecraft: "test".to_owned(),
                platform: "test".to_owned(),
                architecture: "test".to_owned(),
            },
            message: ravel_observability::redact(message),
        }
    }

    #[test]
    fn failed_required_mod_blocks_healthy_broadcasts() {
        let mut failed = Recording::new("required_mod");
        let mut healthy = Recording::new("healthy_mod");
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut healthy, &mut failed];
        let mut machine = running(
            vec![
                entry("required_mod", &[Side::Client]),
                entry("healthy_mod", &[Side::Client]),
            ],
            Side::Client,
            &mut mods,
        );
        machine
            .fail(
                &id("required_mod"),
                failure_attribution("required_mod", "required mod failed"),
            )
            .expect("a running mod can fail");
        assert_eq!(
            broadcast(
                &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                    .expect("all active hosts are supplied"),
                GameEvent::ClientTick
            ),
            Err(DispatchRefused::NotReacting {
                mod_id: id("required_mod"),
                state: LifecycleState::Failed,
            })
        );
        assert_eq!(healthy.seen, Vec::<GameEvent>::new());
    }

    #[test]
    fn failed_optional_mod_does_not_block_healthy_broadcasts() {
        let mut failed = Recording::new("optional_mod");
        let mut healthy = Recording::new("healthy_mod");
        let mut optional = entry("optional_mod", &[Side::Client]);
        optional.requirement = Requirement::Optional;
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut healthy, &mut failed];
        let mut machine = running(
            vec![optional, entry("healthy_mod", &[Side::Client])],
            Side::Client,
            &mut mods,
        );
        machine
            .fail(
                &id("optional_mod"),
                failure_attribution("optional_mod", "optional mod failed"),
            )
            .expect("record optional failure");
        let delivery = broadcast(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            GameEvent::ClientTick,
        )
        .expect("healthy recipients remain available");
        assert_eq!(delivery.received(), [id("healthy_mod")]);
        assert_eq!(healthy.seen, [GameEvent::ClientTick]);
    }

    #[test]
    fn failed_optional_mod_after_world_mutation_blocks_healthy_broadcasts() {
        let mut failed = Recording::new("optional_mod");
        let mut healthy = Recording::new("healthy_mod");
        let mut optional = entry("optional_mod", &[Side::Client]);
        optional.requirement = Requirement::Optional;
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut healthy, &mut failed];
        let mut machine = running(
            vec![optional, entry("healthy_mod", &[Side::Client])],
            Side::Client,
            &mut mods,
        );
        machine
            .record_world_mutation()
            .expect("a running set can mutate the world");
        machine
            .fail(
                &id("optional_mod"),
                failure_attribution("optional_mod", "optional mod failed"),
            )
            .expect("record optional failure");
        let refused = Err(DispatchRefused::NotReacting {
            mod_id: id("optional_mod"),
            state: LifecycleState::Failed,
        });
        assert_eq!(
            broadcast(
                &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                    .expect("all active hosts are supplied"),
                GameEvent::ClientTick
            ),
            refused
        );
        assert_eq!(
            route(
                &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                    .expect("all active hosts are supplied"),
                &CommandInvocation {
                    key: command("healthy_mod", "run"),
                    side: Side::Client,
                    arguments: Vec::new(),
                }
            ),
            refused
        );
        assert_eq!(
            deliver_packet(
                &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                    .expect("all active hosts are supplied"),
                &delivery("healthy_mod:ping", Direction::ToClient, Side::Client,)
            ),
            refused
        );
        assert_eq!(healthy.seen, Vec::<GameEvent>::new());
    }

    #[test]
    fn a_broadcast_reaches_every_mod_in_resolved_order() {
        let mut first = Recording::new("first_mod");
        let mut second = Recording::new("second_mod");
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut first, &mut second];
        let mut machine = running(
            vec![
                entry("first_mod", &[Side::Client]),
                entry("second_mod", &[Side::Client]),
            ],
            Side::Client,
            &mut mods,
        );

        let delivery = broadcast(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            GameEvent::ClientTick,
        )
        .expect("the set runs");
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
        let mut machine = running(
            vec![entry("server_mod", &[Side::DedicatedServer])],
            Side::DedicatedServer,
            &mut mods,
        );

        assert_eq!(machine.audience(GameEvent::ClientTick), Vec::<ModId>::new());
        let delivery = broadcast(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            GameEvent::ClientTick,
        )
        .expect("an empty audience");
        assert_eq!(delivery.received(), []);
        assert!(delivery.is_complete());

        // A server tick on the same set is delivered, so the empty audience
        // above is the event's doing and not the set's.
        let ticked = broadcast(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            GameEvent::ServerTick,
        )
        .expect("the set runs");
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

        let refused = broadcast(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            GameEvent::ClientTick,
        )
        .expect_err("the set has not registered");
        assert_eq!(
            refused,
            DispatchRefused::NotReacting {
                mod_id: id("example_mod"),
                state: LifecycleState::Validated,
            }
        );
        assert_eq!(only.seen, Vec::<GameEvent>::new());
        assert_eq!(refused.failure_kind(), None);
        assert!(!refused.quotes_untrusted_text());
    }

    #[test]
    fn a_mod_that_fails_ends_the_broadcast_and_the_ones_before_it_still_reacted() {
        let mut first = Recording::new("first_mod");
        let mut second = Recording::failing("second_mod", GameEvent::ClientTick);
        let mut third = Recording::new("third_mod");
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut first, &mut second, &mut third];
        let mut machine = running(
            vec![
                entry("first_mod", &[Side::Client]),
                entry("second_mod", &[Side::Client]),
                entry("third_mod", &[Side::Client]),
            ],
            Side::Client,
            &mut mods,
        );

        let delivery = broadcast(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            GameEvent::ClientTick,
        )
        .expect("the set runs, so the delivery happened");
        assert_eq!(delivery.received(), [id("first_mod")]);
        assert!(!delivery.is_complete());
        match delivery.failure() {
            Some(HostError::Refused { mod_id, .. }) => assert_eq!(mod_id, &id("second_mod")),
            other => panic!("expected the second mod to refuse, got {other:?}"),
        }

        // The third mod is never asked, so it cannot act on a set that failed.
        assert_eq!(third.seen, Vec::<GameEvent>::new());
    }

    #[test]
    fn a_broadcast_missing_a_supplied_value_is_refused_before_anyone_reacts() {
        let mut first = Recording::new("first_mod");
        let mut second = Recording::new("second_mod");
        let mut both: Vec<&mut dyn HostMod> = vec![&mut first, &mut second];
        let mut machine = running(
            vec![
                entry("first_mod", &[Side::Client]),
                entry("second_mod", &[Side::Client]),
            ],
            Side::Client,
            &mut both,
        );
        drop(both);

        let mut only: Vec<&mut dyn HostMod> = vec![&mut first];
        let refused = LoadedMods::new(&mut machine, &mut only)
            .expect_err("the plan names a mod that was not supplied");
        assert_eq!(refused.mod_id(), &id("second_mod"));
        // The first mod is in the audience and ahead of the missing one, and it
        // still saw nothing: the whole set is checked before anyone reacts.
        assert_eq!(first.seen, Vec::<GameEvent>::new());
    }

    #[test]
    fn a_command_is_routed_to_its_owner_and_to_nobody_else() {
        let owned = command("first_mod", "home");
        let mut first = Recording::claiming("first_mod", vec![owned.clone()]);
        let mut second = Recording::new("second_mod");
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut first, &mut second];
        let mut machine = running(
            vec![
                entry("first_mod", &[Side::Client]),
                entry("second_mod", &[Side::Client]),
            ],
            Side::Client,
            &mut mods,
        );

        let invocation = CommandInvocation::new(
            owned,
            Side::Client,
            vec!["set".to_owned(), "base".to_owned()],
        );
        let delivery = route(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            &invocation,
        )
        .expect("the owner runs");
        assert_eq!(delivery.received(), [id("first_mod")]);
        assert!(delivery.is_complete());

        assert_eq!(first.ran, [invocation]);
        assert_eq!(second.ran, Vec::<CommandInvocation>::new());
    }

    #[test]
    fn a_command_nobody_claimed_names_the_key_rather_than_a_mod() {
        let mut only = Recording::new("example_mod");
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut only];
        let mut machine = running(
            vec![entry("example_mod", &[Side::Client])],
            Side::Client,
            &mut mods,
        );

        let orphan = command("example_mod", "nowhere");
        let refused = route(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            &CommandInvocation::new(orphan.clone(), Side::Client, Vec::new()),
        )
        .expect_err("nothing claimed it");
        assert_eq!(refused, DispatchRefused::Unclaimed { key: orphan });
        assert!(refused.to_string().contains("no mod claimed"));
        assert_eq!(only.ran, Vec::<CommandInvocation>::new());
    }

    #[test]
    fn a_command_whose_owner_has_not_started_is_refused_naming_the_owner() {
        let owned = command("example_mod", "home");
        let mut only = Recording::claiming("example_mod", vec![owned.clone()]);

        let mut machine =
            LifecycleMachine::new(Side::Client, vec![entry("example_mod", &[Side::Client])])
                .expect("a legal plan");
        advance_through(&mut machine, LifecycleState::Constructed);
        {
            let mut window = machine
                .registration_window(&id("example_mod"))
                .expect("in the plan");
            only.register(&mut window).expect("nothing conflicts");
        }

        // The command is claimed and the set never reached `Running`.
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut only];
        let refused = route(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
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
        assert_eq!(only.ran, Vec::<CommandInvocation>::new());
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

            fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
                Ok(())
            }

            fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
                registrar
                    .claim(self.key.clone())
                    .map_err(ModError::from_display)
            }
        }

        let owned = command("example_mod", "home");
        let mut only = Claiming {
            id: id("example_mod"),
            key: owned.clone(),
        };
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut only];
        let mut machine = running(
            vec![entry("example_mod", &[Side::Client])],
            Side::Client,
            &mut mods,
        );

        let delivery = route(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
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
            (DispatchRefused::Unclaimed { key }, LifecycleState::Running),
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
        panics: bool,
    }

    impl Networked {
        fn new(name: &str) -> Self {
            Self {
                id: id(name),
                received: Vec::new(),
                refuses: false,
                panics: false,
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

        fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn receive_packet(&mut self, delivery: &PacketDelivery) -> Result<(), ModError> {
            assert!(!self.panics, "mod callback panic");
            if self.refuses {
                return Err(ModError::refused(format!(
                    "`{}` is refused by this instance",
                    delivery.id
                )));
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
        let mut machine = running(
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
        let delivered = deliver_packet(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut first, &mut second])
                .expect("all active hosts are supplied"),
            &sent,
        )
        .expect("the owner is running here");

        assert_eq!(delivered.received(), [id("second_mod")]);
        assert!(delivered.is_complete());
        assert_eq!(second.received, [packet_id("second_mod:ping")]);
        assert_eq!(first.received, Vec::<PacketId>::new());
    }

    #[test]
    fn packet_profiling_charges_only_the_receiving_callback() {
        let mut host = Networked::new("example_mod");
        let mut machine = running(
            vec![entry("example_mod", &Side::ALL)],
            Side::DedicatedServer,
            &mut [&mut host],
        );
        let mut profiler = ravel_observability::Profiler::on(std::time::Duration::ZERO);
        let packet = delivery(
            "example_mod:ping",
            Direction::ToServer,
            Side::DedicatedServer,
        );
        let delivered = super::deliver_packet_profiled(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                .expect("active host"),
            &packet,
            &mut profiler,
        )
        .expect("routable packet");
        assert!(delivered.is_complete());
        let wrong_side = delivery("example_mod:ping", Direction::ToServer, Side::Client);
        assert!(matches!(
            super::deliver_packet_profiled(
                &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                    .expect("active host"),
                &wrong_side,
                &mut profiler,
            ),
            Err(DispatchRefused::PacketForeignSide { .. })
        ));
        let report = profiler.report();
        assert_eq!(report.mods().len(), 1);
        assert_eq!(report.mods()[0].cost.event_calls, 1);
        assert_eq!(report.mods()[0].cost.command_calls, 0);
        assert_eq!(host.received, [packet_id("example_mod:ping")]);
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
        let mut machine = running(
            vec![entry("example_mod", &Side::ALL)],
            Side::DedicatedServer,
            &mut [&mut host],
        );

        let sent = delivery("example_mod:ping", Direction::ToServer, Side::Client);
        let refusal = deliver_packet(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                .expect("all active hosts are supplied"),
            &sent,
        )
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
        assert!(rendered.contains(Side::Client.as_str()), "{rendered}");
        assert!(
            rendered.contains(Side::DedicatedServer.as_str()),
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
        deliver_packet(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                .expect("all active hosts are supplied"),
            &agreeing,
        )
        .expect("the sides agree");
        assert_eq!(host.received, [packet_id("example_mod:ping")]);
    }

    #[test]
    fn a_client_bound_packet_on_a_server_side_never_reaches_the_mod() {
        // The owner loads on every side and is running, so nothing but the
        // direction can refuse this. That is the point: `SPEC.md` §14's rule holds
        // for a mod that is present rather than only for one the graph parked.
        let mut host = Networked::new("example_mod");
        let mut machine = running(
            vec![entry("example_mod", &Side::ALL)],
            Side::DedicatedServer,
            &mut [&mut host],
        );

        let sent = delivery(
            "example_mod:screen",
            Direction::ToClient,
            Side::DedicatedServer,
        );
        let refusal = deliver_packet(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                .expect("all active hosts are supplied"),
            &sent,
        )
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
        let mut machine = running(
            vec![entry("example_mod", &Side::ALL)],
            Side::Client,
            &mut [&mut host],
        );

        let sent = delivery("example_mod:screen", Direction::ToClient, Side::Client);
        deliver_packet(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                .expect("all active hosts are supplied"),
            &sent,
        )
        .expect("the client is the receiving end");
        assert_eq!(host.received, [packet_id("example_mod:screen")]);
    }

    #[test]
    fn a_packet_owned_by_a_mod_that_does_not_load_here_is_refused_rather_than_swallowed() {
        let mut host = Networked::new("server_only");
        let mut machine = running(
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
            deliver_packet(
                &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                    .expect("all active hosts are supplied"),
                &sent
            ),
            Err(DispatchRefused::OwnerWrongSide {
                mod_id: id("server_only"),
            })
        );
        assert_eq!(host.received, Vec::<PacketId>::new());
    }

    #[test]
    fn a_packet_naming_a_mod_the_plan_does_not_hold_is_refused_as_absent() {
        let mut host = Networked::new("example_mod");
        let mut machine = running(
            vec![entry("example_mod", &Side::ALL)],
            Side::DedicatedServer,
            &mut [&mut host],
        );

        let sent = delivery("other_mod:ping", Direction::ToServer, Side::DedicatedServer);
        assert_eq!(
            deliver_packet(
                &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                    .expect("all active hosts are supplied"),
                &sent
            ),
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
            deliver_packet(
                &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                    .expect("all active hosts are supplied"),
                &sent
            ),
            Err(DispatchRefused::OwnerNotReacting {
                mod_id: id("example_mod"),
                state: LifecycleState::Validated,
            })
        );
        assert_eq!(host.received, Vec::<PacketId>::new());
    }

    #[test]
    fn a_set_that_holds_no_value_for_the_owner_delivers_to_nobody() {
        let mut present = Networked::new("present_mod");
        let mut machine = running(
            vec![
                entry("present_mod", &Side::ALL),
                entry("absent_mod", &Side::ALL),
            ],
            Side::DedicatedServer,
            &mut [&mut present],
        );

        let refused = LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut present])
            .expect_err("an active owner has no supplied host");
        assert_eq!(refused.mod_id(), &id("absent_mod"));
        let empty: [PacketId; 0] = [];
        assert_eq!(present.received, empty);
    }

    #[test]
    fn a_mod_that_refuses_a_packet_reports_the_refusal_and_reaches_nobody() {
        let mut host = Networked::refusing("example_mod");
        let mut machine = running(
            vec![entry("example_mod", &Side::ALL)],
            Side::DedicatedServer,
            &mut [&mut host],
        );

        let sent = delivery(
            "example_mod:ping",
            Direction::ToServer,
            Side::DedicatedServer,
        );
        let delivered = deliver_packet(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                .expect("all active hosts are supplied"),
            &sent,
        )
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

    struct Outward {
        id: ModId,
        requests: Vec<ravel_data::PlayRequest>,
        panics: bool,
        drains: usize,
    }

    impl Outward {
        fn new(owner: &str, request_owner: &str) -> Self {
            let mut audio = ravel_data::AudioDeclaration::new(id(request_owner));
            audio.entry("tone", b"audio").expect("valid entry");
            audio.event("tone").expect("declared entry");
            Self {
                id: id(owner),
                requests: vec![audio.play("tone", Side::Client).expect("client playback")],
                panics: false,
                drains: 0,
            }
        }
    }

    impl HostMod for Outward {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn take_play_requests(&mut self) -> Vec<ravel_data::PlayRequest> {
            self.drains += 1;
            assert!(!self.panics, "outward callback panic");
            std::mem::take(&mut self.requests)
        }
    }

    #[test]
    fn outward_drain_preserves_order_and_rejects_a_foreign_batch() {
        let mut first = Outward::new("first_mod", "first_mod");
        let mut second = Outward::new("second_mod", "foreign_mod");
        let mut machine = running(
            vec![
                entry("first_mod", &Side::ALL),
                entry("second_mod", &Side::ALL),
            ],
            Side::Client,
            &mut [&mut first, &mut second],
        );
        let (requests, failure) = super::take_play_requests(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut first, &mut second])
                .expect("paired owners"),
        )
        .expect("active set");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].owner(), &id("first_mod"));
        let (owner, error) = failure.expect("foreign request refused");
        assert_eq!(owner, id("second_mod"));
        assert_eq!(error.mod_id(), &owner);
        assert!(matches!(
            error,
            HostError::Refused {
                state: LifecycleState::Running,
                ..
            }
        ));
    }

    #[test]
    fn outward_drain_catches_panics_and_skips_parked_owners() {
        let mut active = Outward::new("active_mod", "active_mod");
        active.panics = true;
        let mut parked = Outward::new("parked_mod", "parked_mod");
        let mut machine = running(
            vec![
                entry("parked_mod", &[Side::DedicatedServer]),
                entry("active_mod", &Side::ALL),
            ],
            Side::Client,
            &mut [&mut parked, &mut active],
        );
        let (requests, failure) = super::take_play_requests(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut parked, &mut active])
                .expect("paired owners"),
        )
        .expect("active set");
        assert_eq!(requests, Vec::new());
        assert_eq!(
            failure,
            Some((
                id("active_mod"),
                HostError::Panicked {
                    mod_id: id("active_mod"),
                    state: LifecycleState::Running,
                }
            ))
        );
        assert_eq!(parked.drains, 0);
        assert_eq!(active.drains, 1);
    }

    #[test]
    fn outward_drain_skips_failed_optional_owners() {
        let mut failed = Outward::new("failed_mod", "failed_mod");
        let mut active = Outward::new("active_mod", "active_mod");
        let mut optional = entry("failed_mod", &Side::ALL);
        optional.requirement = Requirement::Optional;
        let mut machine = running(
            vec![optional, entry("active_mod", &Side::ALL)],
            Side::Client,
            &mut [&mut failed, &mut active],
        );
        machine
            .fail(
                &id("failed_mod"),
                failure_attribution("failed_mod", "failed"),
            )
            .expect("optional owner can fail");
        let (requests, failure) = super::take_play_requests(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut failed, &mut active])
                .expect("paired owners"),
        )
        .expect("optional failure does not halt the set");
        assert!(failure.is_none());
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].owner(), &id("active_mod"));
        assert_eq!(failed.drains, 0);
        assert_eq!(active.drains, 1);
    }
}
