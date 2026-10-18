use ravel_core::{FailureEnvironment, FailureSurface, LifecycleState, ModId, Side};
use ravel_host_api::{CommandInvocation, HostError, Mod};
use ravel_network::PacketDelivery;
use ravel_observability::{CallKind, Profiler};

use super::{Delivery, DispatchRefused, ensure_active, reacts};
use crate::loaded::LoadedMods;

/// Routes `delivery` to its owner after checking the claimed side and direction.
///
/// Each receive callback is attributed as event cost to `profiler`.
/// Malformed bytes and unsupported packet contracts remain typed delivery refusals,
/// without failing the receiving mod. Actual callback failures are attributed normally.
///
/// # Errors
///
/// Returns [`DispatchRefused`] for a halted set, incompatible sides, or an absent or inactive owner.
pub fn deliver_packet<H: std::ops::DerefMut<Target: Mod>>(
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
        delivery.side,
        |owner, state, instance| {
            profiler.measure(owner, CallKind::Event, || {
                crate::callback(owner, state, || instance.receive_packet(delivery))
            })
        },
        FailureSurface::Packet(delivery.id.to_string()),
        |error| {
            matches!(
                error,
                HostError::InvalidPacket { .. } | HostError::Unsupported { .. }
            )
        },
        environment,
    )
}
fn to_owner<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut LoadedMods<'_, H>,
    owner: ModId,
    side: Side,
    call: impl FnOnce(&ModId, LifecycleState, &mut H::Target) -> Result<(), HostError>,
    surface: FailureSurface,
    is_decline: fn(&HostError) -> bool,
    environment: &FailureEnvironment,
) -> Result<Delivery, DispatchRefused> {
    let (machine, mods) = loaded.parts();
    let (entry, state) = machine
        .tracked_entries()
        .find(|(entry, _)| entry.mod_id == owner)
        .ok_or_else(|| DispatchRefused::Absent {
            mod_id: owner.clone(),
        })?;
    if !entry.sides.loads_on(side) {
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
            declined: None,
        }),
        Err(error) => {
            let declined = is_decline(&error);
            if !declined {
                machine
                    .fail_callback(&error, environment, Some(surface))
                    .map_err(DispatchRefused::Lifecycle)?;
            }
            let (failure, declined) = if declined {
                (None, Some(error))
            } else {
                (Some(error), None)
            };
            Ok(Delivery {
                received: Vec::new(),
                failure,
                declined,
            })
        }
    }
}

/// Routes `invocation` to the one mod that claimed its root literal.
///
/// Each command callback's cost is attributed to `profiler`.
/// An owner refusing the invocation is reported in the delivery and keeps running;
/// unsupported commands and panics fail the owner.
///
/// # Errors
///
/// Returns [`DispatchRefused`] for a halted set, an invalid command key, or an absent or inactive owner.
pub fn deliver_command<H: std::ops::DerefMut<Target: Mod>>(
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
        invocation.side,
        |owner, state, instance| {
            profiler.measure(owner, CallKind::Command, || {
                crate::callback(owner, state, || instance.run_command(invocation))
            })
        },
        FailureSurface::Registry(invocation.key.clone()),
        |error| matches!(error, HostError::OperationRefused { .. }),
        environment,
    )
}
