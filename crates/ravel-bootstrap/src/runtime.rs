//! Owns the driven loader and mods and projects hooks into events and commands.
//! Mod callbacks run under a non-reentrant mutex: they must not call the JVM or re-enter hooks.
//! Only owned event data reaches mods; no JNI handles are retained.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use ravel_config::LayeredConfiguration;

use ravel_core::{
    ContentId, ContentKind, DiagnosticMessage, FailureAttribution, FailureEnvironment,
    FailureSurface, RegistryKey, StructuredFailure,
};
use ravel_host_api::{CommandInvocation, GameEvent, HostMod};
use ravel_loader::{
    Delivery, LifecycleMachine, LoadedMods, ModSurface, Supplied, broadcast_profiled,
    route_profiled,
};
use ravel_minecraft::{HookObserver, HookOutcome, HookTarget, HookValue};
use ravel_observability::{PreWorldReport, Profiler, ProfilingReport};

use crate::guard::HookError;
use crate::outward;

const EVENTS: [(&str, GameEvent); 5] = [
    ("server.tick", GameEvent::ServerTick),
    ("server.tick-integrated", GameEvent::ServerTick),
    ("client.tick", GameEvent::ClientTick),
    (
        "resource.load-server-resources",
        GameEvent::ResourcesReloaded,
    ),
    ("resource.reload-client-packs", GameEvent::ResourcesReloaded),
];

const COMMAND: &str = "command.perform-prefixed";

/// The loader, and the mods it drives, for as long as the process runs.
pub struct LoaderRuntime {
    pub(crate) inner: Mutex<Inner>,
    pub(crate) launch: Option<LaunchContext>,
}

pub(crate) struct LaunchContext {
    pub(crate) game_directory: PathBuf,
    pub(crate) supplied: Supplied,
    pub(crate) settings: Mutex<crate::settings::Store>,
}

static RUNTIME: OnceLock<Arc<LoaderRuntime>> = OnceLock::new();

pub(crate) fn installed() -> Option<&'static Arc<LoaderRuntime>> {
    RUNTIME.get()
}

/// Reports whether the process-wide runtime has already been installed.
#[must_use]
pub fn is_installed() -> bool {
    installed().is_some()
}

/// Installs the process-wide host panic reporter once, before invoking mod code.
///
/// The default hook is deliberately not chained: its output quotes raw mod text.
pub fn install_panic_hook() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        std::panic::set_hook(Box::new(|panic| {
            let message = ravel_observability::redact(&panic.to_string());
            eprintln!("Ravel host panic: {}", message.text());
        }));
    });
}

pub(crate) struct Inner {
    pub(crate) machine: LifecycleMachine,
    pub(crate) mods: Vec<Box<dyn HostMod>>,
    profiler: Profiler,
    pairing_refused: bool,
    outward_order: Vec<usize>,
    pub(crate) persistence: crate::persistence::PersistenceState,
    /// Pre-world failures absent from the lifecycle machine's admitted set.
    pub(crate) report: PreWorldReport,
    pub(crate) outward: Vec<outward::Action>,
}

enum Work {
    Event(GameEvent),
    Command(CommandInvocation),
}

impl LoaderRuntime {
    /// Owns a driven mod set with profiling disabled and an empty pre-world report.
    #[must_use]
    pub fn new(machine: LifecycleMachine, mods: Vec<Box<dyn HostMod>>) -> Self {
        Self::with_observers(
            machine,
            mods,
            Profiler::off(),
            PreWorldReport::new(environment()),
        )
    }

    /// Owns a driven mod set with the supplied profiler and pre-world report.
    #[must_use]
    pub fn with_observers(
        machine: LifecycleMachine,
        mods: Vec<Box<dyn HostMod>>,
        profiler: Profiler,
        report: PreWorldReport,
    ) -> Self {
        let outward_order = machine
            .entries()
            .filter(|entry| entry.sides.loads_on(machine.side()))
            .filter_map(|entry| mods.iter().position(|host| host.id() == &entry.mod_id))
            .collect();
        Self {
            launch: None,
            inner: Mutex::new(Inner {
                machine,
                mods,
                profiler,
                pairing_refused: false,
                outward_order,
                persistence: crate::persistence::PersistenceState::default(),
                report,
                outward: Vec::new(),
            }),
        }
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
            .machine
            .order()
            .map(|mod_id| (mod_id.clone(), inner.machine.registry_claim_count(mod_id)))
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
            ModSurface::of(&inner.machine),
            inner.report.notices().iter().cloned().collect(),
        )
    }
}

#[cfg(test)]
impl LoaderRuntime {
    pub(crate) fn fail_while_running(
        &self,
        mod_id: &ravel_core::ModId,
        attribution: FailureAttribution,
    ) {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .machine
            .fail(mod_id, attribution)
            .expect("a mod the machine holds can fail");
    }

    pub(crate) fn claim(
        &self,
        mod_id: &ravel_core::ModId,
        identifier: &RegistryKey,
    ) -> Result<(), ravel_loader::LifecycleError> {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .machine
            .claim_registry_identifier(mod_id, identifier)
    }

    pub(crate) fn describe(
        &self,
        mod_id: &ravel_core::ModId,
        identifier: &RegistryKey,
        behaviour: ravel_core::ContentBehaviour,
    ) -> Result<(), ravel_loader::LifecycleError> {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .machine
            .claim_described_registry_identifier(mod_id, identifier, Some(behaviour))
    }
}

// Local wrapper permits implementing HookObserver for a shared runtime handle.
struct Shared(Arc<LoaderRuntime>);

impl HookObserver for Shared {
    fn observe(&self, target: &HookTarget, values: &[HookValue<'_>]) -> HookOutcome {
        self.0.observe(target, values)
    }
}

/// Creates an empty pre-world report for the pinned runtime environment.
#[must_use]
pub fn pre_world_report() -> PreWorldReport {
    PreWorldReport::new(environment())
}

/// Installs a driven mod set for hooks and diagnostic screens.
///
/// # Errors
/// Returns [`HookError::AlreadyInstalled`] for a second launch, or
/// [`HookError::Unavailable`] if the observer registry is poisoned.
pub fn install(
    machine: LifecycleMachine,
    mods: Vec<Box<dyn HostMod>>,
    report: PreWorldReport,
    game_directory: PathBuf,
    supplied: Supplied,
    configurations: Vec<LayeredConfiguration>,
    profiling_requested: bool,
) -> Result<(), HookError> {
    if installed().is_some() {
        return Err(HookError::AlreadyInstalled);
    }
    let settings =
        crate::settings::Store::new(machine.side(), configurations, Ok(game_directory.clone()));
    let mut runtime = LoaderRuntime::with_observers(
        machine,
        mods,
        Profiler::from_switch(profiling_requested),
        report,
    );
    runtime.launch = Some(LaunchContext {
        game_directory,
        supplied,
        settings: Mutex::new(settings),
    });
    let runtime = Arc::new(runtime);
    crate::dispatch::register_with(Box::new(Shared(Arc::clone(&runtime))), || {
        RUNTIME
            .set(runtime)
            .map_err(|_| HookError::AlreadyInstalled)
    })
}

impl HookObserver for LoaderRuntime {
    /// Delivers hooks without vetoing game execution; mod failures update the machine.
    fn observe(&self, target: &HookTarget, values: &[HookValue<'_>]) -> HookOutcome {
        // Mod callback panics are attributed by loader delivery before this lock is released.
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(work) = inner.work(target, values) {
            inner.deliver(&work);
        }
        HookOutcome::Proceed
    }
}

impl Inner {
    fn work(&self, target: &HookTarget, values: &[HookValue<'_>]) -> Option<Work> {
        let id = target.id().as_str();
        if let Some((_, event)) = EVENTS.iter().find(|(hook, _)| *hook == id) {
            return Some(Work::Event(*event));
        }
        if id != COMMAND {
            return None;
        }
        // Descriptors vary by version, so locate command text by kind rather than position.
        let typed = values.iter().find_map(|value| match value {
            HookValue::Text(text) => Some(text.as_str()),
            HookValue::Boolean(_)
            | HookValue::Byte(_)
            | HookValue::Char(_)
            | HookValue::Short(_)
            | HookValue::Int(_)
            | HookValue::Long(_)
            | HookValue::Float(_)
            | HookValue::Double(_)
            | HookValue::Handle(_) => None,
        })?;
        self.command(typed)
    }

    fn command(&self, typed: &str) -> Option<Work> {
        let (literal, rest) = typed.split_once(char::is_whitespace).unwrap_or((typed, ""));
        let id = literal.parse::<ContentId>().ok()?;
        Some(Work::Command(CommandInvocation::new(
            RegistryKey::new(ContentKind::Command, id),
            self.machine.side(),
            rest.split_whitespace().map(str::to_owned).collect(),
        )))
    }

    fn deliver(&mut self, work: &Work) {
        if self.pairing_refused {
            return;
        }
        let delivered = {
            let mut loaded = match LoadedMods::new(&mut self.machine, &mut self.mods) {
                Ok(loaded) => loaded,
                Err(error) => {
                    // Installed hosts cannot be replaced, so an incomplete set cannot recover.
                    self.pairing_refused = true;
                    self.report.refuse(
                        &ravel_observability::Subject::of(error.mod_id().clone()),
                        &error,
                    );
                    return;
                }
            };
            match work {
                Work::Event(event) => broadcast_profiled(&mut loaded, *event, &mut self.profiler),
                Work::Command(invocation) => {
                    route_profiled(&mut loaded, invocation, &mut self.profiler)
                }
            }
        };

        // Inactive sets, unknown commands, and side mismatches are not mod failures.
        if let Ok(delivery) = delivered {
            self.record(&delivery, work);
        }
        self.carry_outward();
    }

    // Failed owners are never called again; discard server audio to bound pending requests.
    fn carry_outward(&mut self) {
        let plays = !self.machine.side().is_server();
        for slot in 0..self.outward_order.len() {
            let host = &mut self.mods[self.outward_order[slot]];
            let Ok(state) = self.machine.state(host.id()) else {
                continue;
            };
            if state == ravel_core::LifecycleState::Failed {
                continue;
            }
            let Ok(entry) = self.machine.entry(host.id()) else {
                continue;
            };
            let requests =
                ravel_loader::callback(&entry.mod_id, state, || Ok(host.take_play_requests()));
            match requests {
                Ok(requests) => {
                    if plays {
                        self.outward.extend(requests.into_iter().map(|request| {
                            outward::play(request.owner().as_str(), request.entry())
                        }));
                    }
                }
                Err(failure) => {
                    let owner = failure.mod_id().clone();
                    if let Err(error) = self.record_failure(&owner, &failure, None) {
                        self.report
                            .refuse(&ravel_observability::Subject::of(owner), &error);
                    }
                }
            }
        }
    }

    fn record(&mut self, delivery: &Delivery, work: &Work) {
        let Some((owner, failure)) = delivery.failed_by().zip(delivery.failure()) else {
            return;
        };
        let surface = match work {
            Work::Event(event) => FailureSurface::Event(event.as_str().to_owned()),
            Work::Command(invocation) => FailureSurface::Registry(invocation.key.clone()),
        };
        if let Err(error) = self.record_failure(owner, failure, Some(surface)) {
            self.report
                .refuse(&ravel_observability::Subject::of(owner.clone()), &error);
        }
    }

    pub(crate) fn record_failure(
        &mut self,
        owner: &ravel_core::ModId,
        failure: &ravel_host_api::HostError,
        surface: Option<FailureSurface>,
    ) -> Result<(), ravel_loader::LifecycleError> {
        let state = self.machine.state(owner)?;
        if state == ravel_core::LifecycleState::Failed {
            return Ok(());
        }
        let attribution = FailureAttribution {
            mod_id: Some(owner.clone()),
            version: None,
            dependency_chain: Vec::new(),
            state,
            kind: failure.failure_kind(),
            surface,
            environment: environment(),
            // Redaction belongs to the report surface.
            message: DiagnosticMessage::raw(failure.to_string()),
        };
        self.machine.fail(owner, attribution)
    }
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

/// Renders resource-pack metadata using the pinned version's format.
#[must_use]
pub fn resource_pack_metadata(description: &str) -> String {
    ravel_minecraft::pack_metadata(
        ravel_minecraft_adapter::pinned::RESOURCE_PACK_FORMAT,
        ravel_minecraft_adapter::pinned::PACK_FORMAT_SPELLING,
        description,
    )
}

/// Renders data-pack metadata using the pinned version's format.
#[must_use]
pub fn data_pack_metadata(description: &str) -> String {
    ravel_minecraft::pack_metadata(
        ravel_minecraft_adapter::pinned::DATA_PACK_FORMAT,
        ravel_minecraft_adapter::pinned::PACK_FORMAT_SPELLING,
        description,
    )
}

#[cfg(test)]
mod tests;
