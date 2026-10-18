//! Owns the driven loader and mods and projects hooks into events and commands.
//! Mod callbacks run under a non-reentrant mutex: they must not call the JVM or re-enter hooks.
//! Only owned event data reaches mods; no JNI handles are retained.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};

use ravel_config::LayeredConfiguration;

use ravel_core::{
    ContentId, ContentKind, DiagnosticMessage, FailureEnvironment, FailureSurface, RegistryKey,
};
use ravel_host_api::{CommandInvocation, GameEvent, HostMod};
use ravel_loader::{
    Delivery, LifecycleMachine, LoadedMods, ModSurface, Supplied, broadcast_profiled,
    route_profiled,
};
use ravel_minecraft::{HookObserver, HookOutcome, HookTarget, HookValue};
use ravel_observability::{PreWorldReport, Profiler, ProfilingReport};

use crate::outward;
#[cfg(test)]
use ravel_core::FailureAttribution;
use semver::Version;

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
    pub(crate) launch: LaunchContext,
}

/// Admitted and driven inputs owned by one launch.
pub struct Composition {
    pub machine: LifecycleMachine,
    pub mods: Vec<Box<dyn HostMod>>,
    pub configurations: Vec<LayeredConfiguration>,
    pub resources: Supplied,
    pub installed_packages: BTreeMap<ravel_core::ModId, Version>,
}

/// Launch-owned settings and resource context required by every runtime.
pub struct LaunchContext {
    pub(crate) game_directory: PathBuf,
    pub(crate) supplied: Supplied,
    pub(crate) settings: Mutex<crate::settings::Store>,
    pub(crate) installed_packages: BTreeMap<ravel_core::ModId, Version>,
}

impl LaunchContext {
    /// Creates the launch context before the runtime can receive callbacks.
    #[must_use]
    pub fn new(
        side: ravel_core::Side,
        game_directory: PathBuf,
        supplied: Supplied,
        configurations: Vec<LayeredConfiguration>,
        installed_packages: BTreeMap<ravel_core::ModId, Version>,
    ) -> Self {
        let settings = crate::settings::Store::new(side, configurations, game_directory.clone());
        Self {
            game_directory,
            supplied,
            settings: Mutex::new(settings),
            installed_packages,
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
        BTreeMap::new(),
    )
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

fn with_thread_selected<T>(body: impl FnOnce(&Arc<LoaderRuntime>) -> T) -> Option<T> {
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
        let Composition {
            machine,
            mods,
            configurations,
            resources,
            installed_packages,
        } = composition;
        let launch = LaunchContext::new(
            machine.side(),
            game_directory,
            resources,
            configurations,
            installed_packages,
        );
        let profiling = installed().is_some_and(|runtime| {
            runtime
                .inner
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .profiler
                .is_on()
        });
        let runtime = Arc::new(LoaderRuntime::with_observers(
            machine,
            mods,
            Profiler::from_switch(profiling),
            pre_world_report(),
            launch,
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
    command_thread: Option<std::thread::ThreadId>,
    pub(crate) installed_packages: BTreeMap<ravel_core::ModId, Version>,
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
    /// Delivers a semantic event only after the Java adapter's success/thread checks.
    pub(crate) fn legacy_event(&self, kind: &str) -> Result<(), String> {
        let event = match kind {
            "client-tick" => GameEvent::ClientTick,
            "server-tick" => GameEvent::ServerTick,
            "resource-reloaded" => GameEvent::ResourcesReloaded,
            _ => return Err("unknown legacy semantic event".to_owned()),
        };
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if !event.happens_on(inner.machine.side()) {
            return Err(format!(
                "event `{kind}` does not happen on {}",
                inner.machine.side().as_str()
            ));
        }
        inner.deliver_checked(&Work::Event(event))
    }

    /// Binds command installation after vanilla confirms its owning server thread.
    pub(crate) fn claim_command_thread(&self) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if !inner.machine.side().is_server() {
            return Err("native commands require the owning server runtime".to_owned());
        }
        let thread = std::thread::current().id();
        let result = match inner.command_thread {
            Some(owner) if owner != thread => {
                Err("native commands require the server thread".to_owned())
            }
            Some(_) => Ok(()),
            None => {
                inner.command_thread = Some(thread);
                Ok(())
            }
        };
        drop(inner);
        result
    }

    /// Routes only commands claimed by the admitted native set.
    pub(crate) fn legacy_command(&self, typed: &str) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if !inner.machine.side().is_server() {
            return Err("native commands require the owning server runtime".to_owned());
        }
        if inner.command_thread != Some(std::thread::current().id()) {
            return Err("native commands require the initialized server thread".to_owned());
        }
        let Some(work) = inner.command(typed) else {
            return Err("invalid native command".to_owned());
        };
        inner.deliver_checked(&work)
    }

    /// Routes a bounded packet using the SDK's existing owned delivery contract.
    pub(crate) fn legacy_packet(
        &self,
        id: ravel_network::PacketId,
        payload: Vec<u8>,
        to_server: bool,
    ) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if inner.pairing_refused {
            return Err("the installed host set is incomplete".to_owned());
        }
        let direction = if to_server {
            ravel_network::Direction::ToServer
        } else {
            ravel_network::Direction::ToClient
        };
        let packet =
            ravel_network::PacketDelivery::new(id, direction, inner.machine.side(), payload);
        let Inner {
            machine,
            mods,
            profiler,
            pairing_refused,
            report,
            ..
        } = &mut *inner;
        let mut loaded = match LoadedMods::new(machine, mods) {
            Ok(loaded) => loaded,
            Err(error) => {
                *pairing_refused = true;
                report.refuse(
                    &ravel_observability::Subject::of(error.mod_id().clone()),
                    &error,
                );
                return Err(error.to_string());
            }
        };
        let delivery = ravel_loader::deliver_packet_profiled(&mut loaded, &packet, profiler)
            .map_err(|error| error.to_string())?;
        if let Some((owner, failure)) = delivery.failed_by().zip(delivery.failure())
            && !matches!(
                failure,
                ravel_host_api::HostError::InvalidPacket { .. }
                    | ravel_host_api::HostError::Unsupported { .. }
            )
            && let Err(error) = inner.record_failure(
                owner,
                failure,
                Some(FailureSurface::Packet(packet.id.to_string())),
            )
        {
            inner
                .report
                .refuse(&ravel_observability::Subject::of(owner.clone()), &error);
        }
        inner.carry_outward();
        let result = delivery
            .failure()
            .map_or(Ok(()), |error| Err(error.to_string()));
        drop(inner);
        result
    }
}

impl LoaderRuntime {
    /// Owns a driven mod set with profiling disabled and an empty pre-world report.
    #[must_use]
    pub fn new(
        machine: LifecycleMachine,
        mods: Vec<Box<dyn HostMod>>,
        launch: LaunchContext,
    ) -> Self {
        Self::with_observers(
            machine,
            mods,
            Profiler::off(),
            PreWorldReport::new(environment()),
            launch,
        )
    }

    /// Owns a driven mod set with the supplied profiler and pre-world report.
    #[must_use]
    pub fn with_observers(
        machine: LifecycleMachine,
        mods: Vec<Box<dyn HostMod>>,
        profiler: Profiler,
        report: PreWorldReport,
        mut launch: LaunchContext,
    ) -> Self {
        Self {
            inner: Mutex::new(Inner {
                installed_packages: std::mem::take(&mut launch.installed_packages),
                machine,
                mods,
                profiler,
                pairing_refused: false,
                command_thread: None,
                persistence: crate::persistence::PersistenceState::default(),
                report,
                outward: Vec::new(),
            }),
            launch,
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
            .claim_registry_identifier_describing(mod_id, identifier, None)
    }

    pub(crate) fn claim_describing(
        &self,
        mod_id: &ravel_core::ModId,
        identifier: &RegistryKey,
        behaviour: ravel_data::ContentBehaviour,
    ) -> Result<(), ravel_loader::LifecycleError> {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .machine
            .claim_registry_identifier_describing(mod_id, identifier, Some(behaviour))
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

/// A launch could not publish its runtime and observer atomically.
#[derive(Debug, Eq, PartialEq)]
pub enum InstallError {
    /// Another runtime already owns this process.
    AlreadyInstalled,
    /// The observer registry's write lock is poisoned.
    RegistryUnavailable,
    /// A non-server composition was supplied for integrated installation.
    WrongSide,
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::AlreadyInstalled => "the loader runtime is already installed",
            Self::RegistryUnavailable => "the hook registry is unavailable",
            Self::WrongSide => "integrated installation requires the integrated-server side",
        })
    }
}

impl std::error::Error for InstallError {}

/// Installs a driven composition for hooks and diagnostic screens.
///
/// # Errors
/// Returns an error when the runtime is already installed or its registry is unavailable.
pub fn install(
    composition: Composition,
    report: PreWorldReport,
    game_directory: PathBuf,
    profiler: Profiler,
) -> Result<(), InstallError> {
    if installed().is_some() {
        return Err(InstallError::AlreadyInstalled);
    }
    let Composition {
        machine,
        mods,
        configurations,
        resources,
        installed_packages,
    } = composition;
    let side = machine.side();
    let launch = LaunchContext::new(
        machine.side(),
        game_directory,
        resources,
        configurations,
        installed_packages,
    );
    let runtime = Arc::new(LoaderRuntime::with_observers(
        machine, mods, profiler, report, launch,
    ));
    crate::dispatch::register_with(Box::new(Shared(Arc::clone(&runtime))), || {
        RUNTIME
            .set((std::thread::current().id(), side, runtime))
            .map_err(|_| InstallError::AlreadyInstalled)
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

    fn deliver_checked(&mut self, work: &Work) -> Result<(), String> {
        if self.pairing_refused {
            return Err("the installed host set is incomplete".to_owned());
        }
        let mut loaded = LoadedMods::new(&mut self.machine, &mut self.mods).map_err(|error| {
            self.pairing_refused = true;
            self.report.refuse(
                &ravel_observability::Subject::of(error.mod_id().clone()),
                &error,
            );
            error.to_string()
        })?;
        let delivered = match work {
            Work::Event(event) => broadcast_profiled(&mut loaded, *event, &mut self.profiler),
            Work::Command(invocation) => {
                route_profiled(&mut loaded, invocation, &mut self.profiler)
            }
        };
        let delivery = match delivered {
            Ok(delivery) => delivery,
            Err(ravel_loader::DispatchRefused::NotReacting { .. })
                if matches!(work, Work::Event(_)) =>
            {
                return Ok(());
            }
            Err(error) => return Err(error.to_string()),
        };
        self.record(&delivery, work);
        self.carry_outward();
        delivery
            .failure()
            .map_or(Ok(()), |error| Err(error.to_string()))
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

    fn carry_outward(&mut self) {
        let result = LoadedMods::new(&mut self.machine, &mut self.mods)
            .map_err(|error| error.to_string())
            .and_then(|mut loaded| {
                match ravel_loader::take_play_requests(&mut loaded) {
                    // The halt already has its owning failure attribution.
                    Err(ravel_loader::DispatchRefused::NotReacting { .. }) => {
                        Ok((Vec::new(), None))
                    }
                    result => result.map_err(|error| error.to_string()),
                }
            });
        match result {
            Ok((requests, failure)) => {
                if !self.machine.side().is_server() {
                    self.outward.extend(
                        requests.into_iter().map(|request| {
                            outward::play(request.owner().as_str(), request.entry())
                        }),
                    );
                }
                if let Some((owner, failure)) = failure
                    && let Err(error) = self.record_failure(&owner, &failure, None)
                {
                    self.report
                        .refuse(&ravel_observability::Subject::of(owner), &error);
                }
            }
            Err(error) => self.report.note(&error),
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
        self.machine
            .fail_callback(failure, &environment(), surface)
            .map(|_| ())
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
