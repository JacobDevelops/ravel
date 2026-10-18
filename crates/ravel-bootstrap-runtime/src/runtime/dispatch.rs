//! Delivers owned events, commands, and packets while retaining the runtime lock.
//! JNI handles never cross this module's mod-callback interface.

use std::sync::PoisonError;

use ravel_core::{ContentId, ContentKind, DiagnosticMessage, RegistryKey};
use ravel_host_api::{CommandInvocation, GameEvent};
use ravel_loader::{LoadedMods, broadcast, route};
use ravel_minecraft::{HookObserver, HookOutcome, HookTarget, HookValue};

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
        let delivery =
            ravel_loader::deliver_packet(&mut loaded, &packet, profiler, report.environment())
                .map_err(|error| {
                    let subject = error
                        .mod_id()
                        .map_or_else(ravel_observability::Subject::unidentified, |owner| {
                            ravel_observability::Subject::of(owner.clone())
                        });
                    report.refuse(&subject, &error);
                    error.to_string()
                })?;
        inner.carry_outward();
        let result = delivery
            .failure()
            .map_or(Ok(()), |error| Err(error.to_string()));
        drop(inner);
        result
    }
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

    pub(super) fn command(&self, typed: &str) -> Option<Work> {
        let (literal, rest) = typed.split_once(char::is_whitespace).unwrap_or((typed, ""));
        let id = literal.parse::<ContentId>().ok()?;
        Some(Work::Command(CommandInvocation::new(
            RegistryKey::new(ContentKind::Command, id),
            self.server_side(),
            rest.split_whitespace().map(str::to_owned).collect(),
        )))
    }

    fn server_side(&self) -> ravel_core::Side {
        match self.machine.side() {
            ravel_core::Side::Client | ravel_core::Side::IntegratedServer => {
                ravel_core::Side::IntegratedServer
            }
            ravel_core::Side::DedicatedServer => ravel_core::Side::DedicatedServer,
        }
    }

    pub(super) fn deliver_checked(&mut self, work: &Work) -> Result<(), String> {
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
            Work::Event(event) => broadcast(
                &mut loaded,
                *event,
                &mut self.profiler,
                self.report.environment(),
            ),
            Work::Command(invocation) => route(
                &mut loaded,
                invocation,
                &mut self.profiler,
                self.report.environment(),
            ),
        };
        let delivery = match delivered {
            Ok(delivery) => delivery,
            Err(ravel_loader::DispatchRefused::Halted { .. }) if matches!(work, Work::Event(_)) => {
                return Ok(());
            }
            Err(error @ ravel_loader::DispatchRefused::Halted { .. }) => {
                return Err(error.to_string());
            }
            Err(error) => {
                let subject = error
                    .mod_id()
                    .map_or_else(ravel_observability::Subject::unidentified, |owner| {
                        ravel_observability::Subject::of(owner.clone())
                    });
                self.report.refuse(&subject, &error);
                return Err(error.to_string());
            }
        };
        self.carry_outward();
        delivery
            .failure()
            .map_or(Ok(()), |error| Err(error.to_string()))
    }

    fn deliver(&mut self, work: &Work) {
        // The checked path publishes structural refusals before returning its error.
        if let Err(error) = self.deliver_checked(work) {
            eprintln!(
                "[ravel] hook delivery refused: {}",
                DiagnosticMessage::redacted(&error).text()
            );
        }
    }

    fn carry_outward(&mut self) {
        let mut loaded = match LoadedMods::new(&mut self.machine, &mut self.mods) {
            Ok(loaded) => loaded,
            Err(error) => {
                self.report.refuse(
                    &ravel_observability::Subject::of(error.mod_id().clone()),
                    &error,
                );
                return;
            }
        };
        match ravel_loader::take_play_requests(&mut loaded, self.report.environment()) {
            Ok(drained) => self.outward.extend(
                drained
                    .into_requests()
                    .into_iter()
                    .map(|request| outward::play(request.owner().as_str(), request.entry())),
            ),
            Err(ravel_loader::DispatchRefused::Halted { .. }) => {}
            Err(error) => {
                let subject = error
                    .mod_id()
                    .map_or_else(ravel_observability::Subject::unidentified, |owner| {
                        ravel_observability::Subject::of(owner.clone())
                    });
                self.report.refuse(&subject, &error);
            }
        }
    }
}
