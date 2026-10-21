#![forbid(unsafe_code)]

//! Loader orchestration independent of any concrete Minecraft version.
//!
//! Configuration, interception setup, resources, saves, and services share lifecycle
//! isolation: attribute failures before deciding whether optional owners may continue,
//! and preserve subsystem refusals when required owners or world mutation force a stop.
//! Services also retain callback errors for dependent-provider diagnostics. Interception
//! admission attributes every claimant before deciding whether to stop. Saved-data
//! snapshot (`produce_saved_data`) and open (`open_saved_data`) record terminal refusals
//! without applying the setup continuation policy.

mod admission;
mod config;
mod discovery;
mod driver;
mod events;
mod interception;
mod lifecycle;
mod loaded;
mod registry;
mod resources;
mod saves;
mod services;
mod surface;
mod world;

#[cfg(test)]
mod test_support;
pub use admission::admitted;
pub use config::{ConfigRefused, Configured, open_configuration};
pub use discovery::{Discovered, DiscoveredPackage, DiscoveryRefused, MANIFEST_FILE, discover};
pub use driver::{DriveError, start_all, stop_all};
pub use events::{
    Delivery, DispatchRefused, PlayRequests, admit_owner, broadcast, deliver_command,
    deliver_packet, take_play_requests,
};
pub use interception::{
    AdmittedPoints, InterceptRefused, Points, PointsRefused, Round, intercept, open_interceptions,
};
pub use lifecycle::{
    LifecycleEntry, LifecycleError, LifecycleEvent, LifecycleMachine, Requirement,
    executes_mod_code,
};
pub use loaded::{Loaded, LoadedMods, MissingMod};
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
pub(crate) fn callback<T>(
    mod_id: &ravel_core::ModId,
    state: ravel_core::LifecycleState,
    call: impl FnOnce() -> Result<T, ravel_host_api::ModError>,
) -> Result<T, ravel_host_api::HostError> {
    invoke(
        mod_id,
        state,
        call,
        ravel_host_api::HostError::from_operation_refusal,
    )
}

pub(crate) fn transition_callback<T>(
    mod_id: &ravel_core::ModId,
    state: ravel_core::LifecycleState,
    call: impl FnOnce() -> Result<T, ravel_host_api::ModError>,
) -> Result<T, ravel_host_api::HostError> {
    invoke(mod_id, state, call, ravel_host_api::HostError::from_refusal)
}

fn invoke<T>(
    mod_id: &ravel_core::ModId,
    state: ravel_core::LifecycleState,
    call: impl FnOnce() -> Result<T, ravel_host_api::ModError>,
    refused: fn(
        ravel_core::ModId,
        ravel_core::LifecycleState,
        ravel_host_api::ModError,
    ) -> ravel_host_api::HostError,
) -> Result<T, ravel_host_api::HostError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(call)).map_or_else(
        |_| {
            Err(ravel_host_api::HostError::Panicked {
                mod_id: mod_id.clone(),
                state,
            })
        },
        |result| result.map_err(|error| refused(mod_id.clone(), state, error)),
    )
}

pub(crate) fn owner_refusal(
    machine: &LifecycleMachine,
    mod_id: &ravel_core::ModId,
    detail: impl std::fmt::Display,
) -> Result<ravel_host_api::HostError, LifecycleError> {
    Ok(ravel_host_api::HostError::OperationRefused {
        mod_id: mod_id.clone(),
        state: machine.state(mod_id)?,
        detail: detail.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use ravel_core::{LifecycleState, ModId, StructuredFailure};
    use ravel_host_api::{HostError, ModError};

    #[test]
    fn phase_wrappers_preserve_callback_failure_classification() {
        let owner: ModId = "actual_owner".parse().expect("valid id");
        for source in [
            HostError::Panicked {
                mod_id: owner.clone(),
                state: LifecycleState::Running,
            },
            HostError::Unsupported {
                mod_id: owner.clone(),
                state: LifecycleState::Running,
            },
            HostError::OperationRefused {
                mod_id: owner.clone(),
                state: LifecycleState::Running,
                detail: "secret".to_owned(),
            },
        ] {
            let config = crate::ConfigRefused::Mod {
                mod_id: owner.clone(),
                source: source.clone(),
            };
            let resource = crate::ResourceRefused::Mod {
                mod_id: owner.clone(),
                source: source.clone(),
            };
            let intercept = crate::InterceptRefused::Mod {
                mod_id: owner.clone(),
                source: source.clone(),
            };
            let saved = crate::SaveRefused::Mod {
                mod_id: owner.clone(),
                source: source.clone(),
            };
            let service = crate::ServiceRefused::Mod {
                mod_id: owner.clone(),
                source: source.clone(),
            };
            assert_eq!(config.mod_id(), Some(&owner));
            assert_eq!(resource.mod_id(), Some(&owner));
            assert_eq!(intercept.mod_id(), Some(&owner));
            assert_eq!(saved.mod_id(), Some(&owner));
            assert_eq!(service.mod_id(), Some(&owner));
            let wrappers: [&dyn StructuredFailure; 5] =
                [&config, &resource, &intercept, &saved, &service];
            for wrapper in wrappers {
                assert_eq!(wrapper.failure_kind(), source.failure_kind());
                assert_eq!(wrapper.lifecycle_state(), source.lifecycle_state());
                assert_eq!(
                    wrapper.quotes_untrusted_text(),
                    source.quotes_untrusted_text()
                );
            }
        }
    }

    #[test]
    fn foreign_declarations_identify_the_returning_mod_not_the_claimed_owner() {
        let mod_id: ModId = "actual_mod".parse().expect("valid id");
        let owner: ModId = "foreign_owner".parse().expect("valid id");
        let config = crate::ConfigRefused::ForeignDeclaration {
            mod_id: mod_id.clone(),
            owner: owner.clone(),
        };
        let resource = crate::ResourceRefused::ForeignDeclaration {
            mod_id: mod_id.clone(),
            owner: owner.clone(),
        };
        let intercept = crate::InterceptRefused::ForeignDeclaration {
            mod_id: mod_id.clone(),
            owner: owner.clone(),
        };
        let saved = crate::SaveRefused::ForeignDeclaration {
            mod_id: mod_id.clone(),
            owner: owner.clone(),
        };
        let service = crate::ServiceRefused::ForeignDeclaration {
            mod_id: mod_id.clone(),
            owner,
        };
        for actual in [
            config.mod_id(),
            resource.mod_id(),
            intercept.mod_id(),
            saved.mod_id(),
            service.mod_id(),
        ] {
            assert_eq!(actual, Some(&mod_id));
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
