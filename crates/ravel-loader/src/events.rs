mod play;
mod refusal;
mod routing;

pub use play::{PlayRequests, take_play_requests};
pub use refusal::DispatchRefused;
pub use routing::{deliver_packet, deliver_packet_profiled, route, route_profiled};

use ravel_core::{FailureEnvironment, LifecycleState, ModId};
use ravel_host_api::{GameEvent, HostError, Mod};
use ravel_observability::{CallKind, Profiler};

use crate::lifecycle::LifecycleMachine;
use crate::loaded::LoadedMods;

/// Only running and stopping mods may react.
const fn reacts(state: LifecycleState) -> bool {
    matches!(state, LifecycleState::Running | LifecycleState::Stopping)
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
    pub fn failure(&self) -> Option<&HostError> {
        self.failure.as_ref()
    }

    /// Returns the actual owner whose callback failed.
    #[must_use]
    pub fn failed_by(&self) -> Option<&ModId> {
        self.failure.as_ref().map(HostError::mod_id)
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
pub fn broadcast<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut LoadedMods<'_, H>,
    event: GameEvent,
    environment: &FailureEnvironment,
) -> Result<Delivery, DispatchRefused> {
    broadcast_profiled(loaded, event, &mut Profiler::off(), environment)
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
pub fn broadcast_profiled<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut LoadedMods<'_, H>,
    event: GameEvent,
    profiler: &mut Profiler,
    environment: &FailureEnvironment,
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
        let instance = crate::loaded::instance_mut(mods, &mod_id);
        let observed = profiler.measure(&mod_id, CallKind::Event, || {
            crate::callback(&mod_id, state, || instance.observe(event))
        });
        if let Err(failure) = observed {
            machine
                .fail_callback(
                    &failure,
                    environment,
                    Some(ravel_core::FailureSurface::Event(event.as_str().to_owned())),
                )
                .map_err(DispatchRefused::Lifecycle)?;
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

pub(crate) fn ensure_active(machine: &LifecycleMachine) -> Result<(), DispatchRefused> {
    machine.halted_by().map_or(Ok(()), |mod_id| {
        Err(DispatchRefused::NotReacting {
            mod_id: mod_id.clone(),
            state: LifecycleState::Failed,
        })
    })
}

#[cfg(test)]
mod tests;
