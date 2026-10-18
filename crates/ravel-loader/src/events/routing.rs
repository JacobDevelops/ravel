use ravel_core::{FailureEnvironment, LifecycleState, ModId};
use ravel_host_api::{CommandInvocation, HostError, Mod};
use ravel_network::PacketDelivery;
use ravel_observability::{CallKind, Profiler};

use super::{Delivery, DispatchRefused, ensure_active, reacts};
use crate::loaded::LoadedMods;

/// Routes `delivery` to its owner after checking the claimed side and direction.
///
/// Malformed bytes and unsupported packet contracts remain typed delivery refusals,
/// without failing the receiving mod. Actual callback failures are attributed normally.
///
/// # Errors
///
/// Returns [`DispatchRefused`] for a halted set, incompatible sides, or an absent or inactive owner.
pub fn deliver_packet<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut LoadedMods<'_, H>,
    delivery: &PacketDelivery,
    environment: &FailureEnvironment,
) -> Result<Delivery, DispatchRefused> {
    deliver_packet_profiled(loaded, delivery, &mut Profiler::off(), environment)
}

/// Routes a packet, attributing its receive callback as event cost to `profiler`.
///
/// # Errors
///
/// The same as [`deliver_packet`].
pub fn deliver_packet_profiled<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut LoadedMods<'_, H>,
    delivery: &PacketDelivery,
    profiler: &mut Profiler,
    environment: &FailureEnvironment,
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

    to_owner(
        loaded,
        delivery.id.owner().clone(),
        |owner, state, instance| {
            profiler.measure(owner, CallKind::Event, || {
                crate::callback(owner, state, || instance.receive_packet(delivery))
            })
        },
        Some(ravel_core::FailureSurface::Packet(delivery.id.to_string())),
        environment,
    )
}
fn to_owner<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut LoadedMods<'_, H>,
    owner: ModId,
    call: impl FnOnce(&ModId, LifecycleState, &mut H::Target) -> Result<(), HostError>,
    surface: Option<ravel_core::FailureSurface>,
    environment: &FailureEnvironment,
) -> Result<Delivery, DispatchRefused> {
    let (machine, mods) = loaded.parts();
    let (entry, state) = machine
        .tracked_entries()
        .find(|(entry, _)| entry.mod_id == owner)
        .ok_or_else(|| DispatchRefused::Absent {
            mod_id: owner.clone(),
        })?;
    if !entry.sides.loads_on(machine.side()) {
        return Err(DispatchRefused::OwnerWrongSide { mod_id: owner });
    }
    if !reacts(state) {
        return Err(DispatchRefused::OwnerNotReacting {
            mod_id: owner,
            state,
        });
    }
    let instance = crate::loaded::instance_mut(mods, &owner);
    match call(&owner, state, instance) {
        Ok(()) => Ok(Delivery {
            received: vec![owner],
            failure: None,
        }),
        Err(error) => {
            // Declining one delivery is the owner answering it: malformed or unsupported
            // packet contracts and refused command invocations leave the owner running.
            let declined = matches!(
                (&surface, &error),
                (
                    Some(ravel_core::FailureSurface::Packet(_)),
                    HostError::InvalidPacket { .. } | HostError::Unsupported { .. }
                ) | (
                    Some(ravel_core::FailureSurface::Registry(_)),
                    HostError::OperationRefused { .. }
                )
            );
            if !declined {
                machine
                    .fail_callback(&error, environment, surface)
                    .map_err(DispatchRefused::Lifecycle)?;
            }
            Ok(Delivery {
                received: Vec::new(),
                failure: Some(error),
            })
        }
    }
}

/// Routes `invocation` to the one mod that claimed its root literal.
///
/// An owner refusing the invocation is reported in the delivery and keeps running;
/// unsupported commands and panics fail the owner.
///
/// # Errors
///
/// Returns [`DispatchRefused`] for a halted set, an invalid command key, or an absent or inactive owner.
pub fn route<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut LoadedMods<'_, H>,
    invocation: &CommandInvocation,
    environment: &FailureEnvironment,
) -> Result<Delivery, DispatchRefused> {
    route_profiled(loaded, invocation, &mut Profiler::off(), environment)
}

/// Routes `invocation`, attributing its owner's cost to `profiler`.
///
/// # Errors
///
/// The same as [`route`].
pub fn route_profiled<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut LoadedMods<'_, H>,
    invocation: &CommandInvocation,
    profiler: &mut Profiler,
    environment: &FailureEnvironment,
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

    to_owner(
        loaded,
        owner,
        |owner, state, instance| {
            profiler.measure(owner, CallKind::Command, || {
                crate::callback(owner, state, || instance.run_command(invocation))
            })
        },
        Some(ravel_core::FailureSurface::Registry(invocation.key.clone())),
        environment,
    )
}
