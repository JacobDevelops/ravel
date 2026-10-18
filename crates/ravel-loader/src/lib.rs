#![forbid(unsafe_code)]

//! Loader orchestration independent of any concrete Minecraft version.

mod admission;
mod config;
mod discovery;
mod driver;
mod events;
mod game_pack;
mod interception;
mod lifecycle;
mod loaded;
mod registry;
mod resources;
mod saves;
#[cfg(test)]
mod saves_snapshot_tests;
mod services;
mod surface;
mod world;

pub use admission::admitted;
pub use config::{ConfigRefused, Configured, open_configuration};
pub use discovery::{
    Discovered, DiscoveredPackage, DiscoveryRefused, MANIFEST_FILE, PACKAGES_DIRECTORY, discover,
};
pub use driver::{DriveError, drive, drive_loaded, stop_all};
pub use events::{
    Delivery, DispatchRefused, broadcast, broadcast_profiled, deliver_packet,
    deliver_packet_profiled, route, route_profiled, take_play_requests,
};
pub use game_pack::{
    GAME_PACK_DIRECTORY, GamePackError, Half, RESOURCE_PACKS, WORLD_DATA_PACKS, Written, pack_id,
    replaced_paths, write_game_pack,
};
pub use interception::{InterceptRefused, intercept};
pub use lifecycle::{
    LifecycleEntry, LifecycleError, LifecycleEvent, LifecycleMachine, Requirement,
    executes_mod_code,
};
pub use loaded::{LoadedMods, MissingHost};
pub use registry::Registration;
pub use resources::{ResourceRefused, Supplied, open_resources};
pub use saves::{SaveRefused, open_saved_data, produce_saved_data};
pub use services::{ServiceOrder, ServiceRefused, open_services};
pub use surface::{ModEntry, ModSurface};
pub use world::WorldRefused;

/// Invokes mod code behind the loader's owning-mod panic and refusal boundary.
///
/// # Errors
///
/// Returns an attributed refusal or panic for the supplied owner and lifecycle state.
pub fn callback<T>(
    mod_id: &ravel_core::ModId,
    state: ravel_core::LifecycleState,
    call: impl FnOnce() -> Result<T, ravel_host_api::ModError>,
) -> Result<T, ravel_host_api::HostError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(call)).map_or_else(
        |_| {
            Err(ravel_host_api::HostError::Panicked {
                mod_id: mod_id.clone(),
                state,
            })
        },
        |result| result.map_err(|error| error.with_context(mod_id.clone(), state)),
    )
}

#[cfg(test)]
mod tests {
    use ravel_core::{LifecycleState, ModId, Side, SideSet, StructuredFailure};
    use ravel_host_api::{HostError, ModError};

    use crate::lifecycle::{LifecycleEntry, LifecycleMachine, Requirement};

    pub(crate) fn entry(name: &str, sides: &[Side]) -> LifecycleEntry {
        LifecycleEntry {
            mod_id: name.parse().expect("a legal mod id"),
            sides: SideSet::new(sides.iter().copied()).expect("a non-empty side set"),
            requirement: Requirement::Required,
            saved_data_versions: std::collections::BTreeSet::new(),
        }
    }

    /// Advances every mod in `machine` along the success path, from `Validated` through `last`.
    pub(crate) fn advance_through(machine: &mut LifecycleMachine, last: LifecycleState) {
        const SUCCESS_PATH: [LifecycleState; 6] = [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
            LifecycleState::Running,
        ];
        assert!(
            SUCCESS_PATH.contains(&last),
            "{last:?} is not on the success path"
        );
        for state in SUCCESS_PATH {
            machine.advance_all(state).expect("a legal step");
            if state == last {
                break;
            }
        }
    }

    #[test]
    fn panics_are_distinct_from_mod_refusals_with_the_same_text() {
        let owner: ModId = "actual_owner".parse().expect("valid id");
        let panic = super::callback::<()>(&owner, LifecycleState::Stopping, || {
            panic!("secret panic payload");
        })
        .expect_err("the host catches the panic");
        let refusal = super::callback::<()>(&owner, LifecycleState::Stopping, || {
            Err(ModError::refused("mod callback panicked"))
        })
        .expect_err("the mod refused");
        assert_eq!(
            panic,
            HostError::Panicked {
                mod_id: owner.clone(),
                state: LifecycleState::Stopping,
            }
        );
        assert_ne!(panic, refusal);
        assert_eq!(panic.mod_id(), &owner);
        assert_eq!(panic.lifecycle_state(), LifecycleState::Stopping);
        assert_eq!(panic.failure_kind(), None);
        assert!(!panic.quotes_untrusted_text());
        assert!(refusal.quotes_untrusted_text());
        assert_eq!(
            panic.to_string(),
            "`actual_owner` panicked during `stopping`"
        );
    }
}
