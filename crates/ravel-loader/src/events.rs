mod play;
mod refusal;
mod routing;

pub use play::{PlayRequests, take_play_requests};
pub use refusal::DispatchRefused;
pub use routing::{accepts_owner, deliver_command, deliver_packet};

use ravel_core::{FailureEnvironment, LifecycleState, ModId, Side};
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
    declined: Option<HostError>,
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

    /// Returns a non-fatal refusal; its owner remains healthy.
    #[must_use]
    pub fn declined(&self) -> Option<&HostError> {
        self.declined.as_ref()
    }

    /// Returns the actual owner whose callback failed.
    #[must_use]
    pub fn failed_by(&self) -> Option<&ModId> {
        self.failure.as_ref().map(HostError::mod_id)
    }

    /// Returns whether every mod that should have reacted did so.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.failure.is_none() && self.declined.is_none()
    }
}

impl LifecycleMachine {
    fn audience_entries(
        &self,
        side: Side,
    ) -> impl Iterator<Item = (&crate::LifecycleEntry, LifecycleState)> {
        self.tracked_entries().filter(move |(entry, state)| {
            entry.sides.loads_on(side)
                && entry.sides.loads_on(self.side())
                && *state != LifecycleState::Failed
        })
    }
}

/// Broadcasts `event` in resolved order after validating the entire audience.
///
/// `side` is the executing side, including integrated-server work in a client process.
/// Each callback's cost is attributed to `profiler`.
/// Mod failures stop delivery and are returned in [`Delivery::failure`].
/// Events that cannot happen on the executing side are refused before any mod is called.
///
/// # Errors
///
/// Returns [`DispatchRefused`] if the executing side is foreign to the runtime,
/// the event cannot happen on that side, the set is halted, or an audience member is not reacting.
///
/// # Panics
///
/// Never; all audience members are checked before delivery.
pub fn broadcast<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut LoadedMods<'_, H>,
    event: GameEvent,
    side: Side,
    profiler: &mut Profiler,
    environment: &FailureEnvironment,
) -> Result<Delivery, DispatchRefused> {
    let (machine, mods) = loaded.parts();
    ensure_active(machine)?;
    let running = machine.side();
    if side != running && side != running.world_side() {
        return Err(DispatchRefused::EventForeignSide {
            claimed: side,
            running,
        });
    }
    if !event.happens_on(side) {
        return Err(DispatchRefused::EventWrongSide { event, side });
    }
    let audience: Vec<_> = machine
        .audience_entries(side)
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
                declined: None,
            });
        }
        received.push(mod_id);
    }
    Ok(Delivery {
        received,
        failure: None,
        declined: None,
    })
}

/// Checks the loader-owned admission rule shared by dispatch and host pre-admission.
///
/// # Errors
///
/// Returns [`DispatchRefused::Halted`] for a latched halt or a required, side-loaded
/// mod that failed before the world was modified.
pub(crate) fn ensure_active(machine: &LifecycleMachine) -> Result<(), DispatchRefused> {
    active_blocker(machine).map_or(Ok(()), |mod_id| {
        Err(DispatchRefused::Halted { by: mod_id.clone() })
    })
}

pub(crate) fn active_blocker(machine: &LifecycleMachine) -> Option<&ModId> {
    machine.halted_by().or_else(|| {
        machine
            .tracked_entries()
            .find(|(entry, state)| {
                entry.requirement == crate::Requirement::Required
                    && entry.sides.loads_on(machine.side())
                    && *state == LifecycleState::Failed
            })
            .map(|(entry, _)| &entry.mod_id)
    })
}

#[cfg(test)]
mod tests;
