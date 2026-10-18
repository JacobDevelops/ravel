use ravel_core::{FailureEnvironment, ModId};
use ravel_host_api::{HostError, Mod};

use super::{DispatchRefused, ensure_active, reacts};
use crate::lifecycle::LifecycleMachine;
use crate::loaded::LoadedMods;

/// Requests collected before an outward callback failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayRequests {
    requests: Vec<ravel_data::PlayRequest>,
    failure: Option<HostError>,
}

impl PlayRequests {
    /// Returns requests from callbacks that completed successfully.
    #[must_use]
    pub fn requests(&self) -> &[ravel_data::PlayRequest] {
        &self.requests
    }

    /// Consumes the delivery and returns its successful requests.
    #[must_use]
    pub fn into_requests(self) -> Vec<ravel_data::PlayRequest> {
        self.requests
    }

    /// Returns the attributed callback failure, if any.
    #[must_use]
    pub fn failure(&self) -> Option<&HostError> {
        self.failure.as_ref()
    }

    /// Returns the callback owner without storing a second identity.
    #[must_use]
    pub fn failed_by(&self) -> Option<&ModId> {
        self.failure.as_ref().map(HostError::mod_id)
    }
}
/// Drains outward play requests in lifecycle order, stopping at an attributable failure.
///
/// Failed, parked and nonexecuting owners are not called. Requests naming another
/// owner are refused before any request from that callback is exposed.
/// Server distributions still drain and validate callbacks, but expose no client
/// play requests; their recipients are accepted only on the client distribution.
///
/// # Errors
///
/// Returns [`DispatchRefused`] if the set is halted.
pub fn take_play_requests<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut LoadedMods<'_, H>,
    environment: &FailureEnvironment,
) -> Result<PlayRequests, DispatchRefused> {
    let (machine, mods) = loaded.parts();
    ensure_active(machine)?;
    let mut requests = Vec::new();
    let eligible: Vec<_> = machine
        .eligible()
        .filter(|(_, state)| reacts(*state))
        .map(|(entry, state)| (entry.mod_id.clone(), state))
        .collect();
    for (mod_id, state) in eligible {
        let instance = crate::loaded::instance_mut(mods, &mod_id);
        let drained = crate::callback(&mod_id, state, || Ok(instance.take_play_requests()));
        let drained = match drained {
            Ok(drained) => drained,
            Err(error) => return request_failure(machine, environment, requests, error),
        };
        if let Some(foreign) = drained.iter().find(|request| request.owner() != &mod_id) {
            let error = crate::owner_refusal(
                machine,
                &mod_id,
                format!(
                    "outward play request belongs to `{}`, not `{mod_id}`",
                    foreign.owner()
                ),
            )
            .map_err(DispatchRefused::Lifecycle)?;
            return request_failure(machine, environment, requests, error);
        }
        if !machine.side().is_server() {
            requests.extend(drained);
        }
    }
    Ok(PlayRequests {
        requests,
        failure: None,
    })
}

fn request_failure(
    machine: &mut LifecycleMachine,
    environment: &FailureEnvironment,
    requests: Vec<ravel_data::PlayRequest>,
    error: HostError,
) -> Result<PlayRequests, DispatchRefused> {
    machine
        .fail_callback(&error, environment, None)
        .map_err(DispatchRefused::Lifecycle)?;
    Ok(PlayRequests {
        requests,
        failure: Some(error),
    })
}
