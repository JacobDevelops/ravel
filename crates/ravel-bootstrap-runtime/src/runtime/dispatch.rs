//! Delivers owned events, commands, and packets while retaining the runtime lock.
//! JNI handles never cross this module's mod-callback interface.

use std::sync::PoisonError;

use ravel_core::{ContentId, ContentKind, DiagnosticMessage, RegistryKey, Side};
use ravel_host_api::{CommandInvocation, GameEvent, HostError};
use ravel_loader::{DispatchRefused, broadcast, deliver_command};
use ravel_minecraft::{HookTarget, HookValue};

use super::{Inner, LoaderRuntime};
use crate::outward;

pub(super) const EVENTS: [(&str, GameEvent); 5] = [
    ("server.tick", GameEvent::ServerTick),
    ("server.tick-integrated", GameEvent::ServerTick),
    ("client.tick", GameEvent::ClientTick),
    (
        "resource.load-server-resources",
        GameEvent::ResourcesReloaded,
    ),
    ("resource.reload-client-packs", GameEvent::ResourcesReloaded),
];

pub(super) const COMMAND: &str = "command.perform-prefixed";

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
    Precondition(&'static str),
    EventSide { event: GameEvent, side: Side },
}

impl std::fmt::Display for Undelivered {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(error) => std::fmt::Display::fmt(error, formatter),
            Self::Failed(error) => std::fmt::Display::fmt(error, formatter),
            Self::Precondition(reason) => formatter.write_str(reason),
            Self::EventSide { event, side } => {
                write!(formatter, "{event:?} does not happen on {side:?}")
            }
        }
    }
}

impl std::error::Error for Undelivered {}

impl LoaderRuntime {
    /// Delivers a semantic event only after the Java adapter's success/thread checks.
    pub(crate) fn legacy_event(&self, event: GameEvent) -> Result<(), Undelivered> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let side = inner.loaded.machine().side();
        if !event.happens_on(side) {
            return Err(Undelivered::EventSide { event, side });
        }
        inner.deliver_checked(&Work::Event(event, side))
    }

    /// Binds command installation after vanilla confirms its owning server thread.
    pub(crate) fn claim_command_thread(&self) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if !inner.loaded.machine().side().is_server() {
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
    pub(crate) fn legacy_command(&self, typed: &str) -> Result<(), Undelivered> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if !inner.loaded.machine().side().is_server() {
            return Err(Undelivered::Precondition(
                "native commands require the owning server runtime",
            ));
        }
        if inner.command_thread != Some(std::thread::current().id()) {
            return Err(Undelivered::Precondition(
                "native commands require the initialized server thread",
            ));
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
        if id == "server.tick" && process_side == Side::Client {
            // IntegratedServer calls its superclass tick; only its own hook delivers.
            return None;
        }
        if let Some((_, event)) = EVENTS.iter().find(|(hook, _)| *hook == id) {
            let side = if id == "server.tick-integrated" {
                self.server_side()
            } else {
                process_side
            };
            return Some(Work::Event(*event, side));
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
            self.server_side(),
            rest.split_whitespace().map(str::to_owned).collect(),
        )))
    }

    fn server_side(&self) -> Side {
        match self.loaded.machine().side() {
            Side::Client | Side::IntegratedServer => Side::IntegratedServer,
            Side::DedicatedServer => Side::DedicatedServer,
        }
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
        delivery
            .failure()
            .or_else(|| delivery.declined())
            .map_or(Ok(()), |error| Err(Undelivered::Failed(error.clone())))
    }

    fn deliver(&mut self, work: &Work) {
        // Dispatch refusals are per-call diagnostics, not pre-world launch failures.
        if let Err(error) = self.deliver_checked(work) {
            eprintln!(
                "[ravel] hook delivery refused: {}",
                DiagnosticMessage::redacted(&error.to_string()).text()
            );
        }
    }

    fn carry_outward(&mut self) {
        let mut loaded = self.loaded.borrow();
        match ravel_loader::take_play_requests(&mut loaded, self.report.environment()) {
            Ok(drained) => self.outward.extend(
                drained
                    .into_requests()
                    .into_iter()
                    .map(|request| outward::play(request.id())),
            ),
            // The halt already has its owning failure attribution.
            Err(DispatchRefused::Halted { .. }) => {}
            Err(error) => eprintln!(
                "[ravel] play request drain refused: {}",
                DiagnosticMessage::redacted(&error.to_string()).text()
            ),
        }
    }
}
