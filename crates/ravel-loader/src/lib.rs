#![forbid(unsafe_code)]

//! Loader orchestration independent of any concrete Minecraft version.

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

pub use admission::admitted;
pub use config::{ConfigRefused, Configured, open_configuration};
pub use discovery::{Discovered, DiscoveredPackage, DiscoveryRefused, MANIFEST_FILE, discover};
pub use driver::{DriveError, drive_loaded, stop_all};
pub use events::{
    Delivery, DispatchRefused, PlayRequests, accepts_owner, broadcast, deliver_command,
    deliver_packet, ensure_active, take_play_requests,
};
pub use interception::{InterceptRefused, intercept, open_interceptions};
pub use lifecycle::{
    LifecycleEntry, LifecycleError, LifecycleEvent, LifecycleMachine, Requirement,
    executes_mod_code,
};
pub use loaded::{Loaded, LoadedMods, MissingMod};
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

    pub(crate) fn environment() -> ravel_core::FailureEnvironment {
        ravel_core::FailureEnvironment {
            ravel: "0.1.0".parse().expect("valid version"),
            minecraft: "Synthetic26.1.5".to_owned(),
            platform: "test-platform".to_owned(),
            architecture: "test-architecture".to_owned(),
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
