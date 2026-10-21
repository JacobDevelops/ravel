use ravel_core::RegistryKey;
use ravel_host_api::Mod;
use ravel_loader::{LifecycleMachine, Supplied};
use ravel_minecraft_adapter::environment;

use crate::runtime::LaunchContext;

pub(crate) fn test_launch() -> LaunchContext {
    LaunchContext::new(
        ravel_core::Side::Client,
        std::env::temp_dir(),
        Supplied::default(),
        Vec::new(),
    )
}

/// Owns a document-only fixture's explicit plan and its callback-free hosts.
pub(crate) struct DocumentFixture {
    pub(crate) machine: LifecycleMachine,
    pub(crate) mods: Vec<Box<dyn Mod>>,
}

impl DocumentFixture {
    pub(crate) fn new(side: ravel_core::Side, entries: Vec<ravel_loader::LifecycleEntry>) -> Self {
        struct Quiet(ravel_core::ModId);

        impl Mod for Quiet {
            fn id(&self) -> &ravel_core::ModId {
                &self.0
            }

            fn enter(
                &mut self,
                _state: ravel_core::LifecycleState,
            ) -> Result<(), ravel_host_api::ModError> {
                Ok(())
            }
        }

        let mods = entries
            .iter()
            .map(|entry| -> Box<dyn Mod> { Box::new(Quiet(entry.mod_id.clone())) })
            .collect();
        Self {
            machine: LifecycleMachine::new(side, entries).expect("a legal document fixture plan"),
            mods,
        }
    }

    pub(crate) fn start_with_claims(
        &mut self,
        claims: &[(RegistryKey, Option<ravel_data::ContentBehaviour>)],
    ) {
        struct Claiming {
            owner: ravel_core::ModId,
            claims: Vec<(RegistryKey, Option<ravel_data::ContentBehaviour>)>,
        }
        impl Mod for Claiming {
            fn id(&self) -> &ravel_core::ModId {
                &self.owner
            }

            fn enter(
                &mut self,
                _state: ravel_core::LifecycleState,
            ) -> Result<(), ravel_host_api::ModError> {
                Ok(())
            }

            fn register(
                &mut self,
                registrar: &mut dyn ravel_host_api::Registrar,
            ) -> Result<(), ravel_host_api::ModError> {
                for (key, behaviour) in &self.claims {
                    let outcome = match behaviour {
                        None => registrar.claim(key.clone()),
                        Some(behaviour) => registrar.claim_describing(key.clone(), *behaviour),
                    };
                    outcome.map_err(ravel_host_api::ModError::from_display)?;
                }
                Ok(())
            }
        }
        assert!(
            claims.iter().all(|(key, _)| self
                .machine
                .entries()
                .any(|entry| &entry.mod_id == key.id().namespace())),
            "every fixture claim must be routed to a registered owner"
        );
        self.mods = self
            .machine
            .entries()
            .map(|entry| -> Box<dyn Mod> {
                let owner = entry.mod_id.clone();
                let claims = claims
                    .iter()
                    .filter(|(key, _)| key.id().namespace() == &owner)
                    .cloned()
                    .collect();
                Box::new(Claiming { owner, claims })
            })
            .collect();
        let mut loaded = ravel_loader::LoadedMods::new(&mut self.machine, &mut self.mods)
            .expect("paired fixtures");
        ravel_loader::start_all(&mut loaded, &environment())
            .expect("fixture registration succeeds");
    }
}

pub(crate) struct RefusingStart {
    pub(crate) owner: ravel_core::ModId,
    pub(crate) state: ravel_core::LifecycleState,
    pub(crate) detail: &'static str,
}

impl Mod for RefusingStart {
    fn id(&self) -> &ravel_core::ModId {
        &self.owner
    }

    fn enter(&mut self, state: ravel_core::LifecycleState) -> Result<(), ravel_host_api::ModError> {
        if state == self.state {
            return Err(ravel_host_api::ModError::refused(self.detail));
        }
        Ok(())
    }
}

/// Advances every mod in `machine` along the success path, from `Validated` through `last`.
pub(crate) fn advance_through(machine: &mut LifecycleMachine, last: ravel_core::LifecycleState) {
    use ravel_core::LifecycleState;

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

/// Creates an empty per-process directory under the system temporary directory, clearing
/// whatever an earlier run left behind.
pub(crate) fn fresh_directory(label: &str, name: &str) -> std::path::PathBuf {
    let at = std::env::temp_dir().join(format!("ravel-{label}-{}-{name}", std::process::id()));
    if at.exists() {
        std::fs::remove_dir_all(&at).expect("remove the previous test directory");
    }
    std::fs::create_dir_all(&at).expect("a writable temporary directory");
    at
}
