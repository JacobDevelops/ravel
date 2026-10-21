//! Delivers owned events, commands, and packets while retaining the runtime lock.
//! JNI handles never cross this module's mod-callback interface.

use std::sync::PoisonError;

use ravel_core::{ContentId, ContentKind, RegistryKey, Side};
use ravel_host_api::{CommandInvocation, GameEvent, HostError};
use ravel_loader::{DispatchRefused, broadcast, deliver_command};
use ravel_minecraft::{HookTarget, HookValue};

use super::{Inner, LoaderRuntime};
use crate::{guard::report, outward};

pub(super) const EVENTS: [(&str, GameEvent, EventSide); 5] = [
    ("server.tick", GameEvent::ServerTick, EventSide::Server),
    (
        "server.tick-integrated",
        GameEvent::ServerTick,
        EventSide::World,
    ),
    ("client.tick", GameEvent::ClientTick, EventSide::Process),
    (
        "resource.load-server-resources",
        GameEvent::ResourcesReloaded,
        EventSide::Process,
    ),
    (
        "resource.reload-client-packs",
        GameEvent::ResourcesReloaded,
        EventSide::Process,
    ),
];

#[derive(Clone, Copy)]
pub(super) enum EventSide {
    Process,
    Server,
    World,
}

impl EventSide {
    fn resolve(self, process: Side) -> Option<Side> {
        match self {
            Self::Process => Some(process),
            Self::Server => (process != Side::Client).then_some(process),
            Self::World => Some(process.world_side()),
        }
    }
}

pub(super) const COMMAND: &str = "command.perform-prefixed";

const NOT_SERVER: &str = "native commands require the owning server runtime";
const WRONG_THREAD: &str = "native commands require the initialized server thread";

pub(super) enum Work {
    Event(GameEvent, Side),
    Command(CommandInvocation),
    Packet(ravel_network::PacketDelivery),
}

/// Preserves callback failures separately from dispatch and adapter refusals.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Undelivered {
    Refused(DispatchRefused),
    Failed(HostError),
    Declined(HostError),
    Precondition(&'static str),
}

impl Undelivered {
    pub(crate) fn context(&self) -> &'static str {
        match self {
            Self::Refused(_) | Self::Precondition(_) => "hook delivery refused",
            Self::Failed(_) => "hook delivery failed",
            Self::Declined(_) => "hook delivery declined",
        }
    }
}

impl std::fmt::Display for Undelivered {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(error) => std::fmt::Display::fmt(error, formatter),
            Self::Failed(error) | Self::Declined(error) => std::fmt::Display::fmt(error, formatter),
            Self::Precondition(reason) => formatter.write_str(reason),
        }
    }
}

impl std::error::Error for Undelivered {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Refused(error) => Some(error),
            Self::Failed(error) | Self::Declined(error) => Some(error),
            Self::Precondition(_) => None,
        }
    }
}

impl LoaderRuntime {
    /// Delivers a semantic event only after the Java adapter's success/thread checks.
    pub(crate) fn legacy_event(&self, event: GameEvent) -> Result<(), Undelivered> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let side = inner.loaded.machine().side();
        inner.deliver_checked(&Work::Event(event, side))
    }

    /// Binds command installation after vanilla confirms its owning server thread.
    pub(crate) fn claim_command_thread(&self) -> Result<(), Undelivered> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if !inner.loaded.machine().side().is_server() {
            return Err(Undelivered::Precondition(NOT_SERVER));
        }
        let thread = std::thread::current().id();
        let result = match inner.command_thread {
            Some(owner) if owner != thread => Err(Undelivered::Precondition(WRONG_THREAD)),
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
    pub(crate) fn legacy_command(&self, typed: &str) -> Result<(), Undelivered> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if !inner.loaded.machine().side().is_server() {
            return Err(Undelivered::Precondition(NOT_SERVER));
        }
        if inner.command_thread != Some(std::thread::current().id()) {
            return Err(Undelivered::Precondition(WRONG_THREAD));
        }
        let Some(work) = inner.command(typed) else {
            return Err(Undelivered::Precondition("invalid native command"));
        };
        inner.deliver_checked(&work)
    }

    /// Routes a bounded packet using the SDK's existing owned delivery contract.
    pub(crate) fn legacy_packet(
        &self,
        id: ravel_network::PacketId,
        payload: Vec<u8>,
        to_server: bool,
    ) -> Result<(), Undelivered> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let direction = if to_server {
            ravel_network::Direction::ToServer
        } else {
            ravel_network::Direction::ToClient
        };
        let packet = ravel_network::PacketDelivery::new(
            id,
            direction,
            inner.loaded.machine().side(),
            payload,
        );
        inner.deliver_checked(&Work::Packet(packet))
    }
}

impl LoaderRuntime {
    /// Delivers hooks without vetoing game execution; mod failures update the machine.
    ///
    /// Values have a fresh lifetime independent of `&self`; borrowed JNI handles
    /// cannot be retained after observation.
    pub(crate) fn observe(&self, target: &HookTarget, values: &[HookValue<'_>]) {
        // Mod callback panics are attributed by loader delivery before this lock is released.
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(work) = inner.work(target, values) {
            inner.deliver(&work);
        }
    }
}

impl Inner {
    fn work(&self, target: &HookTarget, values: &[HookValue<'_>]) -> Option<Work> {
        let id = target.id().as_str();
        let process_side = self.loaded.machine().side();
        if let Some((_, event, side)) = EVENTS.iter().find(|(hook, _, _)| *hook == id) {
            return Some(Work::Event(*event, side.resolve(process_side)?));
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

    pub(super) fn command(&self, typed: &str) -> Option<Work> {
        let (literal, rest) = typed.split_once(char::is_whitespace).unwrap_or((typed, ""));
        let id = literal.parse::<ContentId>().ok()?;
        Some(Work::Command(CommandInvocation::new(
            RegistryKey::new(ContentKind::Command, id),
            self.loaded.machine().side().world_side(),
            rest.split_whitespace().map(str::to_owned).collect(),
        )))
    }

    pub(super) fn deliver_checked(&mut self, work: &Work) -> Result<(), Undelivered> {
        let mut loaded = self.loaded.borrow();
        let delivered = match work {
            Work::Event(event, side) => broadcast(
                &mut loaded,
                *event,
                *side,
                &mut self.profiler,
                self.report.environment(),
            ),
            Work::Command(invocation) => deliver_command(
                &mut loaded,
                invocation,
                &mut self.profiler,
                self.report.environment(),
            ),
            Work::Packet(packet) => ravel_loader::deliver_packet(
                &mut loaded,
                packet,
                &mut self.profiler,
                self.report.environment(),
            ),
        };
        let delivery = match delivered {
            Ok(delivery) => delivery,
            Err(DispatchRefused::Halted { .. }) if matches!(work, Work::Event(_, _)) => {
                return Ok(());
            }
            Err(error) => return Err(Undelivered::Refused(error)),
        };
        self.carry_outward();
        if let Some(error) = delivery.failure() {
            return Err(Undelivered::Failed(error.clone()));
        }
        if let Some(error) = delivery.declined() {
            return Err(Undelivered::Declined(error.clone()));
        }
        Ok(())
    }

    fn deliver(&mut self, work: &Work) {
        // Dispatch refusals are per-call diagnostics, not pre-world launch failures.
        if let Err(error) = self.deliver_checked(work) {
            report(error.context(), &error);
        }
    }

    fn carry_outward(&mut self) {
        let mut loaded = self.loaded.borrow();
        match ravel_loader::take_play_requests(&mut loaded, self.report.environment()) {
            Ok(drained) => {
                if let Some(error) = drained.failure() {
                    report("play request failed", error);
                }
                self.outward.extend(
                    drained
                        .into_requests()
                        .into_iter()
                        .map(|request| outward::play(request.id())),
                );
            }
            // The halt already has its owning failure attribution.
            Err(DispatchRefused::Halted { .. }) => {}
            Err(error) => report("play request drain refused", &error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EVENTS, Side, Undelivered};
    use ravel_core::LifecycleState;
    use ravel_host_api::HostError;

    #[test]
    fn hook_table_retains_each_executing_side_rule() {
        for (hook, _, rule) in EVENTS {
            for process in [Side::Client, Side::IntegratedServer, Side::DedicatedServer] {
                let expected = match hook {
                    "server.tick" if process == Side::Client => None,
                    "server.tick-integrated" => Some(process.world_side()),
                    _ => Some(process),
                };
                assert_eq!(rule.resolve(process), expected, "{hook} on {process:?}");
            }
        }
    }

    #[test]
    fn callback_diagnostic_contexts_preserve_outcome_distinctions() {
        let error = HostError::Panicked {
            mod_id: "example_mod".parse().expect("valid mod identifier"),
            state: LifecycleState::Running,
        };
        assert_eq!(Undelivered::Failed(error).context(), "hook delivery failed");
        let refusal = HostError::OperationRefused {
            mod_id: "example_mod".parse().expect("valid mod identifier"),
            state: LifecycleState::Running,
            detail: "choose a destination".to_owned(),
        };
        assert_eq!(
            Undelivered::Declined(refusal).context(),
            "hook delivery declined"
        );
        assert_eq!(
            Undelivered::Precondition("unavailable").context(),
            "hook delivery refused"
        );
    }
}
