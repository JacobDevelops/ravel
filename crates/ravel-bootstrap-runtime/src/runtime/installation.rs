//! Publishes process and integrated-server runtimes and selects their owning threads.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};

use ravel_observability::{PreWorldReport, Profiler};

use super::{Composition, LoaderRuntime, pre_world_report};

static RUNTIME: OnceLock<(std::thread::ThreadId, ravel_core::Side, Arc<LoaderRuntime>)> =
    OnceLock::new();
static INTEGRATED_VIEW: Mutex<Option<Weak<LoaderRuntime>>> = Mutex::new(None);

pub(crate) fn installed() -> Option<&'static Arc<LoaderRuntime>> {
    RUNTIME.get().map(|(_, _, runtime)| runtime)
}

thread_local! {
    static INTEGRATED: std::cell::RefCell<Option<Arc<LoaderRuntime>>> = const {
        std::cell::RefCell::new(None)
    };
}

/// Borrows the runtime selected by side and ownership without cloning its handle.
pub(crate) fn with_selected<T>(body: impl FnOnce(&Arc<LoaderRuntime>) -> T) -> Option<T> {
    if !ravel_minecraft_adapter::pinned::LEGACY_ADAPTER {
        return installed().map(body);
    }
    with_thread_selected(body)
}

pub(super) fn with_thread_selected<T>(body: impl FnOnce(&Arc<LoaderRuntime>) -> T) -> Option<T> {
    INTEGRATED.with(|slot| {
        let integrated = slot.borrow();
        if let Some(runtime) = integrated.as_ref() {
            return Some(body(runtime));
        }
        let (owner, side, runtime) = RUNTIME.get()?;
        if *side == ravel_core::Side::Client && *owner != std::thread::current().id() {
            return None;
        }
        Some(body(runtime))
    })
}

/// Selects an owned handle only when a caller must retain it.
pub(crate) fn selected() -> Option<Arc<LoaderRuntime>> {
    with_selected(Arc::clone)
}

/// Shares integrated diagnostics without transferring server-thread ownership.
pub(crate) fn integrated_runtime() -> Option<Arc<LoaderRuntime>> {
    INTEGRATED_VIEW
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .and_then(Weak::upgrade)
}

/// Whether the calling thread already owns its integrated-server composition.
#[must_use]
pub fn integrated_installed() -> bool {
    INTEGRATED.with(|runtime| runtime.borrow().is_some())
}

/// Installs a composition whose lifetime is the integrated server thread's lifetime.
///
/// # Errors
///
/// Refuses an already installed server thread or a non-integrated composition.
pub fn install_integrated(
    composition: Composition,
    game_directory: PathBuf,
) -> Result<(), InstallError> {
    if composition.machine.side() != ravel_core::Side::IntegratedServer {
        return Err(InstallError::WrongSide);
    }
    INTEGRATED.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_some() {
            return Err(InstallError::AlreadyInstalled);
        }
        let profiling = installed().is_some_and(|runtime| {
            runtime
                .inner
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .profiler
                .is_on()
        });
        let runtime = Arc::new(LoaderRuntime::from_composition(
            composition,
            game_directory,
            Profiler::from_switch(profiling),
            pre_world_report(),
        ));
        *INTEGRATED_VIEW
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(Arc::downgrade(&runtime));
        *slot = Some(runtime);
        Ok(())
    })
}

/// Reports whether the process-wide runtime has already been installed.
#[must_use]
pub fn is_installed() -> bool {
    installed().is_some()
}

/// A launch could not publish its runtime.
#[derive(Debug, Eq, PartialEq)]
pub enum InstallError {
    /// Another runtime already owns this process.
    AlreadyInstalled,
    /// A non-server composition was supplied for integrated installation.
    WrongSide,
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::AlreadyInstalled => "the loader runtime is already installed",
            Self::WrongSide => "integrated installation requires the integrated-server side",
        })
    }
}

impl std::error::Error for InstallError {}

/// Installs a driven composition for hooks and diagnostic screens.
///
/// # Errors
/// Returns an error when the runtime is already installed.
pub fn install(
    composition: Composition,
    report: PreWorldReport,
    game_directory: PathBuf,
    profiler: Profiler,
) -> Result<(), InstallError> {
    if installed().is_some() {
        return Err(InstallError::AlreadyInstalled);
    }
    let side = composition.machine.side();
    let runtime = Arc::new(LoaderRuntime::from_composition(
        composition,
        game_directory,
        profiler,
        report,
    ));
    RUNTIME
        .set((std::thread::current().id(), side, runtime))
        .map_err(|_| InstallError::AlreadyInstalled)
}
