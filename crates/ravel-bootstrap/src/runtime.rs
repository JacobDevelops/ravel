//! Owns the driven loader and mods and projects hooks into events and commands.
//! Mod callbacks run under a non-reentrant mutex: they must not call the JVM or re-enter hooks.
//! Only owned event data reaches mods; no JNI handles are retained.

use std::sync::{Arc, Mutex, PoisonError};

use ravel_core::{
    ContentId, ContentKind, DiagnosticMessage, FailureAttribution, FailureEnvironment, RegistryKey,
    StructuredFailure,
};
use ravel_host_api::{CommandInvocation, GameEvent, HostMod};
use ravel_loader::{Delivery, LifecycleMachine, ModSurface, broadcast_profiled, route_profiled};
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
}

pub(crate) struct Inner {
    pub(crate) machine: LifecycleMachine,
    pub(crate) mods: Vec<Box<dyn HostMod>>,
    profiler: Profiler,
    pub(crate) persistence: crate::persistence::PersistenceState,
    /// Pre-world failures absent from the lifecycle machine's admitted set.
    pub(crate) report: PreWorldReport,
}

enum Work {
    Event(GameEvent),
    Command(CommandInvocation),
}

impl LoaderRuntime {
    /// Owns a driven mod set with profiling disabled and an empty pre-world report.
    #[must_use]
    pub fn new(machine: LifecycleMachine, mods: Vec<Box<dyn HostMod>>) -> Self {
        Self::profiling(
            machine,
            mods,
            Profiler::off(),
            PreWorldReport::new(environment()),
        )
    }

    /// Owns a driven mod set with the supplied profiler and pre-world report.
    #[must_use]
    pub fn profiling(
        machine: LifecycleMachine,
        mods: Vec<Box<dyn HostMod>>,
        profiler: Profiler,
        report: PreWorldReport,
    ) -> Self {
        Self {
            inner: Mutex::new(Inner {
                machine,
                mods,
                profiler,
                persistence: crate::persistence::PersistenceState::default(),
                report,
            }),
        }
    }

    /// Reads current collector data and registry claims under the runtime lock.
    #[must_use]
    pub(crate) fn profiling_report(&self) -> ProfilingReport {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let claims = inner
            .machine
            .order()
            .map(|mod_id| (mod_id.clone(), inner.machine.registry_claims(mod_id)))
            .collect::<Vec<_>>();
        for (mod_id, count) in claims {
            inner
                .profiler
                .registry_claims(&mod_id, count.try_into().unwrap_or(u64::MAX));
        }
        inner.profiler.report()
    }

    /// Reads mod rows and notices under one lock for a consistent live view.
    #[must_use]
    pub(crate) fn mod_view(&self) -> (ModSurface, Vec<DiagnosticMessage>) {
        let inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        (
            ModSurface::of(&inner.machine),
            inner.report.notices().to_vec(),
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
        identifier: &str,
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
        identifier: &str,
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
/// Returns [`HookError::Unavailable`] if the observer registry is poisoned.
pub fn install(
    machine: LifecycleMachine,
    mods: Vec<Box<dyn HostMod>>,
    report: PreWorldReport,
) -> Result<(), HookError> {
    let runtime = Arc::new(LoaderRuntime::profiling(
        machine,
        mods,
        Profiler::from_switch(crate::profiling::requested()),
        report,
    ));
    crate::content::install(Arc::clone(&runtime));
    crate::mods::install(Arc::clone(&runtime));
    crate::persistence::install(Arc::clone(&runtime));
    crate::profiling::install(Arc::clone(&runtime));
    crate::dispatch::register(Box::new(Shared(runtime)))
}

impl HookObserver for LoaderRuntime {
    /// Delivers hooks without vetoing game execution; mod failures update the machine.
    fn observe(&self, target: &HookTarget, values: &[HookValue<'_>]) -> HookOutcome {
        // A delivery panic precedes machine mutation; recover poison without attributing that panic.
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
        let delivered = {
            let mut borrowed: Vec<&mut dyn HostMod> =
                self.mods.iter_mut().map(|host| &mut **host).collect();
            match work {
                Work::Event(event) => {
                    broadcast_profiled(&self.machine, &mut borrowed, *event, &mut self.profiler)
                }
                Work::Command(invocation) => {
                    route_profiled(&self.machine, &mut borrowed, invocation, &mut self.profiler)
                }
            }
        };

        // Inactive sets, unknown commands, and side mismatches are not mod failures.
        if let Ok(delivery) = delivered {
            self.record(&delivery, work);
        }
        self.carry_outward();
    }

    // Drain every mod, including failed deliveries; discard server audio to bound pending requests.
    fn carry_outward(&mut self) {
        let plays = !self.machine.side().is_server();
        for host in &mut self.mods {
            for request in host.take_play_requests() {
                if plays {
                    outward::play(request.owner().as_str(), request.entry());
                }
            }
        }
    }

    fn record(&mut self, delivery: &Delivery, work: &Work) {
        let Some(failure) = delivery.failure() else {
            return;
        };
        let owner = match work {
            Work::Event(event) => self
                .machine
                .audience(*event)
                .get(delivery.received().len())
                .cloned(),
            Work::Command(invocation) => self.machine.owner_of(&invocation.key).cloned(),
        }
        .expect("a delivered failure belongs to a dispatched owner");
        if let Err(error) = self.record_failure(&owner, failure) {
            self.report
                .refuse(&ravel_observability::Subject::of(owner), &error);
        }
    }

    pub(crate) fn record_failure(
        &mut self,
        owner: &ravel_core::ModId,
        failure: &ravel_host_api::HostError,
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
            surface: None,
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
