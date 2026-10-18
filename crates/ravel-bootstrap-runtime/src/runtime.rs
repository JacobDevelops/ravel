//! Owns each launch's driven loader, mods, settings, and diagnostic views.
//! Installation selects the thread-owned runtime; dispatch projects hooks into owned events.
//! Mod callbacks must not call the JVM or re-enter the non-reentrant runtime mutex.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use ravel_config::LayeredConfiguration;

use ravel_core::{DiagnosticMessage, FailureEnvironment};
#[cfg(test)]
use ravel_host_api::Mod;
use ravel_loader::{Loaded, ModSurface, Supplied};
use ravel_observability::{PreWorldReport, Profiler, ProfilingReport};

use crate::outward;
use ravel_core::RegistryKey;
#[cfg(test)]
use ravel_loader::LifecycleMachine;
use semver::Version;

mod dispatch;
pub(crate) mod installation;
pub(crate) use dispatch::Undelivered;

pub use installation::{
    InstallError, install, install_integrated, integrated_installed, is_installed,
    process_launch_context,
};
pub(crate) use installation::{installed, integrated_runtime, selected, with_selected};

/// The loader and mods for one launch: the process slot or one integrated-server thread.
pub(crate) struct LoaderRuntime {
    pub(crate) inner: Mutex<Inner>,
    pub(crate) launch: LaunchContext,
}

/// Admitted and driven inputs owned by one launch.
pub struct Composition {
    pub loaded: Loaded,
    pub configurations: Vec<LayeredConfiguration>,
    pub resources: Supplied,
    pub interceptions: ravel_host_api::AdmittedPoints,
    pub installed_packages: BTreeMap<ravel_core::ModId, Version>,
}

/// Launch-owned settings and resource context required by every runtime.
pub(crate) struct LaunchContext {
    pub(crate) game_directory: PathBuf,
    pub(crate) supplied: Supplied,
    pub(crate) settings: Mutex<crate::settings::Store>,
}

impl LaunchContext {
    /// Creates the launch context before the runtime can receive callbacks.
    #[must_use]
    pub(crate) fn new(
        side: ravel_core::Side,
        game_directory: PathBuf,
        supplied: Supplied,
        configurations: Vec<LayeredConfiguration>,
    ) -> Self {
        let settings = crate::settings::Store::new(side, configurations);
        Self {
            game_directory,
            supplied,
            settings: Mutex::new(settings),
        }
    }
}

#[cfg(test)]
pub(crate) fn test_launch() -> LaunchContext {
    LaunchContext::new(
        ravel_core::Side::Client,
        std::env::temp_dir(),
        Supplied::default(),
        Vec::new(),
    )
}

/// Owns a document-only fixture's explicit plan and its callback-free hosts.
#[cfg(test)]
pub(crate) struct DocumentFixture {
    pub(crate) machine: LifecycleMachine,
    pub(crate) mods: Vec<Box<dyn Mod>>,
}

#[cfg(test)]
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
}

/// Advances every mod in `machine` along the success path, from `Validated` through `last`.
#[cfg(test)]
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

/// Installs the process-wide host panic reporter once, before invoking mod code.
///
/// The default hook is deliberately not chained: its output quotes raw mod text.
pub fn install_panic_hook() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        std::panic::set_hook(Box::new(|panic| {
            let message = DiagnosticMessage::redacted(&panic.to_string());
            eprintln!("Ravel host panic: {}", message.text());
        }));
    });
}

pub(crate) struct Inner {
    pub(crate) loaded: Loaded,
    pub(crate) interceptions: ravel_host_api::AdmittedPoints,
    profiler: Profiler,
    command_thread: Option<std::thread::ThreadId>,
    pub(crate) installed_packages: BTreeMap<ravel_core::ModId, Version>,
    pub(crate) persistence: crate::persistence::PersistenceState,
    /// Pre-world failures absent from the lifecycle machine's admitted set.
    pub(crate) report: PreWorldReport,
    pub(crate) outward: Vec<outward::Action>,
}

impl LoaderRuntime {
    /// Consumes admitted launch inputs into their sole private runtime owners.
    #[must_use]
    fn from_composition(
        composition: Composition,
        game_directory: PathBuf,
        profiler: Profiler,
        report: PreWorldReport,
    ) -> Self {
        let Composition {
            loaded,
            configurations,
            resources,
            installed_packages,
            interceptions,
        } = composition;
        let launch = LaunchContext::new(
            loaded.machine().side(),
            game_directory,
            resources,
            configurations,
        );
        Self::from_parts(
            loaded,
            profiler,
            report,
            launch,
            installed_packages,
            interceptions,
        )
    }

    #[cfg(test)]
    pub(crate) fn new(
        machine: LifecycleMachine,
        mods: Vec<Box<dyn Mod>>,
        launch: LaunchContext,
    ) -> Self {
        Self::with_observers(
            machine,
            mods,
            Profiler::off(),
            PreWorldReport::new(environment()),
            launch,
            BTreeMap::new(),
        )
    }

    #[cfg(test)]
    pub(crate) fn with_observers(
        machine: LifecycleMachine,
        mods: Vec<Box<dyn Mod>>,
        profiler: Profiler,
        report: PreWorldReport,
        launch: LaunchContext,
        installed_packages: BTreeMap<ravel_core::ModId, Version>,
    ) -> Self {
        let mut loaded = Loaded::new(machine, mods).expect("complete test composition");
        let interceptions =
            match ravel_loader::open_interceptions(&mut loaded.borrow(), report.environment()) {
                Ok(interceptions) => interceptions,
                Err(ravel_loader::InterceptRefused::Halted { .. }) => {
                    // Blocked fixtures cannot execute declarations or later interceptions.
                    ravel_host_api::Points::default()
                        .admit()
                        .expect("empty interceptions")
                }
                Err(error) => panic!("test interception declarations must admit: {error}"),
            };
        Self::from_parts(
            loaded,
            profiler,
            report,
            launch,
            installed_packages,
            interceptions,
        )
    }

    /// Owns a driven mod set with the supplied profiler and pre-world report.
    #[must_use]
    fn from_parts(
        loaded: Loaded,
        profiler: Profiler,
        report: PreWorldReport,
        launch: LaunchContext,
        installed_packages: BTreeMap<ravel_core::ModId, Version>,
        interceptions: ravel_host_api::AdmittedPoints,
    ) -> Self {
        Self {
            inner: Mutex::new(Inner {
                installed_packages,
                interceptions,
                loaded,
                profiler,
                command_thread: None,
                persistence: crate::persistence::PersistenceState::default(),
                report,
                outward: Vec::new(),
            }),
            launch,
        }
    }

    /// Lists the command keys admitted by this runtime's lifecycle.
    pub(crate) fn command_keys(&self) -> Vec<RegistryKey> {
        let inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        inner
            .loaded
            .machine()
            .claimed_keys()
            .filter(|key| key.kind() == ravel_core::ContentKind::Command)
            .cloned()
            .collect()
    }

    /// Checks lifecycle owner admission, not packet direction or payload validity.
    pub(crate) fn accepts_packet_owner(&self, owner: &ravel_core::ModId) -> bool {
        let inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let machine = inner.loaded.machine();
        let accepted = ravel_loader::accepts_owner(machine, owner, machine.side()).is_ok();
        drop(inner);
        accepted
    }

    /// Drains only the actions produced by this runtime.
    pub(crate) fn drain_outward(&self) -> String {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        outward::drain(&mut inner.outward)
    }

    /// Reads current collector data and registry claims under the runtime lock.
    #[must_use]
    pub(crate) fn profiling_report(&self) -> ProfilingReport {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let claims = inner
            .loaded
            .machine()
            .order()
            .map(|mod_id| {
                (
                    mod_id.clone(),
                    inner.loaded.machine().registry_claim_count(mod_id),
                )
            })
            .collect::<Vec<_>>();
        for (mod_id, count) in claims {
            inner
                .profiler
                .record_registry_claims(&mod_id, count.try_into().unwrap_or(u64::MAX));
        }
        inner.profiler.report()
    }

    /// Reads mod rows and notices under one lock for a consistent live view.
    #[must_use]
    pub(crate) fn mod_view(&self) -> (ModSurface, Vec<DiagnosticMessage>) {
        let inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        (
            ModSurface::of(inner.loaded.machine()),
            inner.report.notices().iter().cloned().collect(),
        )
    }
}

/// Creates an empty pre-world report for the pinned runtime environment.
#[must_use]
pub fn pre_world_report() -> PreWorldReport {
    PreWorldReport::new(environment())
}

/// Returns the pinned runtime environment for admission and failure reports.
///
/// # Panics
/// Panics if this crate's package version is not valid semver.
#[must_use]
pub fn environment() -> FailureEnvironment {
    FailureEnvironment {
        ravel: env!("CARGO_PKG_VERSION")
            .parse()
            .expect("every crate in this workspace carries a semver version"),
        minecraft: ravel_minecraft_adapter::pinned::MINECRAFT_VERSION.to_owned(),
        platform: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
    }
}

#[cfg(test)]
mod tests;
