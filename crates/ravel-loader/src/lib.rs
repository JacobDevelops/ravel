#![forbid(unsafe_code)]

//! Loader orchestration independent of any concrete Minecraft version.

mod admission;
mod config;
mod discovery;
mod driver;
mod events;
mod interception;
mod lifecycle;
mod pack;
mod registry;
mod resources;
mod saves;
mod services;
mod surface;
mod world;

pub use admission::admitted;
pub use config::{ConfigRefused, Configured, open_configuration};
pub use discovery::{
    Discovered, DiscoveredPackage, DiscoveryRefused, MANIFEST_FILE, PACKAGES_DIRECTORY, discover,
};
pub use driver::{DriveError, drive, drive_borrowed, stop_all};
pub use events::{
    Delivery, DispatchRefused, broadcast, broadcast_profiled, deliver_packet, route, route_profiled,
};
pub use interception::{InterceptRefused, intercept};
pub use lifecycle::{
    LifecycleEntry, LifecycleError, LifecycleEvent, LifecycleMachine, Requirement,
    executes_mod_code,
};
pub use pack::{
    Half, PACK_DIRECTORY, PackError, RESOURCE_PACKS, WORLD_DATA_PACKS, Written, pack_id, write_pack,
};
pub use registry::Registration;
pub use resources::{ResourceRefused, Supplied, open_resources};
pub use saves::{SaveRefused, open_saved_data};
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
    use ravel_core::{LifecycleState, ModId, StructuredFailure};
    use ravel_host_api::{HostError, ModError};

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
