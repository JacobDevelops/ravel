//! Owns each launch's driven loader, mods, settings, and diagnostic views.
//! Installation selects the thread-owned runtime; dispatch projects hooks into owned events.
//! Mod callbacks must not call the JVM or re-enter the non-reentrant runtime mutex.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use ravel_config::LayeredConfiguration;

#[cfg(test)]
use ravel_host_api::Mod;
#[cfg(test)]
use ravel_loader::LifecycleMachine;
use ravel_loader::{Loaded, ModSurface, Supplied};
#[cfg(test)]
use ravel_minecraft_adapter::environment;
use ravel_observability::{PreWorldReport, Profiler, ProfilingReport};

use crate::outward;
use ravel_core::{DiagnosticMessage, RegistryKey, Version};

mod dispatch;
pub(crate) mod installation;
pub(crate) use dispatch::Undelivered;

pub use installation::{
    InstallError, install, install_integrated, integrated_installed, is_installed,
    process_game_directory,
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
    pub interceptions: ravel_loader::AdmittedPoints,
    pub installed_packages: BTreeMap<ravel_core::ModId, Version>,
}

/// Launch-owned settings and resource context required by every runtime.
pub(crate) struct LaunchContext {
    pub(crate) game_directory: PathBuf,
    pub(crate) supplied: Supplied,
    pub(crate) settings: Mutex<crate::settings::Store>,
    pub(crate) profiling: bool,
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
            profiling: false,
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
            eprintln!(
                "Ravel host panic: {}",
                ravel_core::redact_text(&panic.to_string())
            );
        }));
    });
}

pub(crate) struct Inner {
    pub(crate) loaded: Loaded,
    pub(crate) interceptions: ravel_loader::AdmittedPoints,
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
                    ravel_loader::Points::default()
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
        mut launch: LaunchContext,
        installed_packages: BTreeMap<ravel_core::ModId, Version>,
        interceptions: ravel_loader::AdmittedPoints,
    ) -> Self {
        launch.profiling = profiler.is_on();
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
        let accepted = ravel_loader::admit_owner(machine, owner, machine.side()).is_ok();
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
        let Inner {
            loaded, profiler, ..
        } = &mut *inner;
        for mod_id in loaded.machine().order() {
            let count = loaded.machine().registry_claim_count(mod_id);
            profiler.record_registry_claims(mod_id, count.try_into().unwrap_or(u64::MAX));
        }
        let report = profiler.report();
        drop(inner);
        report
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

#[cfg(test)]
mod tests;
