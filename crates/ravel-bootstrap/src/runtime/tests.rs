use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use ravel_core::{
    ContentId, ContentKind, LifecycleState, ModId, RegistryKey, Side, SideSet, StructuredFailure,
};
use ravel_host_api::{CommandInvocation, GameEvent, HostError, HostMod, Registrar};
use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement};
use ravel_minecraft::{HookId, HookKind, HookObserver, HookOutcome, HookTarget, HookValue};

use super::{COMMAND, EVENTS, LoaderRuntime, environment};
use crate::outward;

use ravel_data::PlayRequest;

const MOD: &str = "example_mod";

fn id(name: &str) -> ModId {
    name.parse().expect("a legal mod id")
}

fn key(literal: &str) -> RegistryKey {
    RegistryKey::new(
        ContentKind::Command,
        format!("{MOD}:{literal}")
            .parse::<ContentId>()
            .expect("a legal content id"),
    )
}

fn target(hook: &'static str) -> HookTarget {
    HookTarget::new(
        HookId::new(hook),
        "net/minecraft/Ignored",
        "ignored",
        "()V",
        HookKind::InstanceMethod,
    )
}

#[derive(Debug, Default)]
struct Log {
    seen: Vec<GameEvent>,
    ran: Vec<CommandInvocation>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Behaviour {
    Records,
    Fails,
    Panics,
    Forges(&'static str),
}

struct Recording {
    id: ModId,
    log: Arc<Mutex<Log>>,
    behaviour: Behaviour,
    claims: Vec<RegistryKey>,
}

impl HostMod for Recording {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
        Ok(())
    }

    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), HostError> {
        for claim in &self.claims {
            registrar
                .claim(claim.clone())
                .map_err(|refused| HostError::Refused {
                    mod_id: self.id.clone(),
                    state: LifecycleState::Constructed,
                    detail: refused.to_string(),
                })?;
        }
        Ok(())
    }

    fn observe(&mut self, event: GameEvent) -> Result<(), HostError> {
        match self.behaviour {
            Behaviour::Records => {
                self.log.lock().expect("nothing panicked").seen.push(event);
                Ok(())
            }
            Behaviour::Fails => Err(HostError::Refused {
                mod_id: self.id.clone(),
                state: LifecycleState::Running,
                detail: "this mod declines every event".to_owned(),
            }),
            Behaviour::Forges(owner) => Err(HostError::Unsupported {
                mod_id: id(owner),
                state: LifecycleState::Running,
            }),
            Behaviour::Panics => panic!("this mod is broken"),
        }
    }

    fn run_command(&mut self, invocation: &CommandInvocation) -> Result<(), HostError> {
        if let Behaviour::Forges(owner) = self.behaviour {
            return Err(HostError::Unsupported {
                mod_id: id(owner),
                state: LifecycleState::Running,
            });
        }
        self.log
            .lock()
            .expect("nothing panicked")
            .ran
            .push(invocation.clone());
        Ok(())
    }
}

fn running(
    side: Side,
    claims: Vec<RegistryKey>,
    behaviour: Behaviour,
) -> (LoaderRuntime, Arc<Mutex<Log>>) {
    let (machine, mods, log) = parts(side, claims, behaviour);
    (LoaderRuntime::new(machine, mods), log)
}

fn parts(
    side: Side,
    claims: Vec<RegistryKey>,
    behaviour: Behaviour,
) -> (LifecycleMachine, Vec<Box<dyn HostMod>>, Arc<Mutex<Log>>) {
    let log = Arc::new(Mutex::new(Log::default()));
    let mut host = Recording {
        id: id(MOD),
        log: Arc::clone(&log),
        behaviour,
        claims,
    };

    let entry = LifecycleEntry {
        mod_id: id(MOD),
        sides: SideSet::new([side]).expect("a non-empty side set"),
        requirement: Requirement::Required,
        saved_data_versions: BTreeSet::new(),
    };
    let mut machine = LifecycleMachine::new(side, vec![entry]).expect("a legal plan");
    for state in [
        LifecycleState::Validated,
        LifecycleState::Resolved,
        LifecycleState::Loaded,
        LifecycleState::Constructed,
    ] {
        machine.advance_all(state).expect("a legal step");
    }
    {
        let mut window = machine
            .registration_window(&id(MOD))
            .expect("the mod is in the plan");
        host.register(&mut window).expect("nothing conflicts");
    }
    for state in [LifecycleState::Registered, LifecycleState::Running] {
        machine.advance_all(state).expect("a legal step");
    }

    (machine, vec![Box::new(host)], log)
}

fn playing(side: Side) -> LoaderRuntime {
    held(side, Box::new(ravel_test_mod_audio::AudioMod::new()))
}

fn held(side: Side, host: Box<dyn HostMod>) -> LoaderRuntime {
    let entry = LifecycleEntry {
        mod_id: id("audio"),
        sides: SideSet::new([side]).expect("a non-empty side set"),
        requirement: Requirement::Required,
        saved_data_versions: BTreeSet::new(),
    };
    let mut machine = LifecycleMachine::new(side, vec![entry]).expect("a legal plan");
    for state in [
        LifecycleState::Validated,
        LifecycleState::Resolved,
        LifecycleState::Loaded,
        LifecycleState::Constructed,
        LifecycleState::Registered,
        LifecycleState::Running,
    ] {
        machine.advance_all(state).expect("a legal step");
    }
    LoaderRuntime::new(machine, vec![host])
}

#[test]
fn a_mod_asking_to_play_reaches_the_outward_queue_as_the_adapter_reads_it() {
    let _serial = outward::SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    outward::drain();

    let runtime = playing(Side::Client);
    runtime.observe(&target("client.tick"), &[]);

    assert_eq!(
        outward::drain(),
        "1:11:110:audio.play1:35:audio10:door_opens1:1"
    );
}

#[test]
fn a_delivery_a_mod_asked_nothing_on_queues_nothing() {
    let _serial = outward::SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    outward::drain();

    let runtime = playing(Side::Client);
    runtime.observe(&target("resource.reload-client-packs"), &[]);

    assert_eq!(outward::drain(), "");
}

struct AsksOnEveryEvent {
    id: ModId,
    asked: Vec<PlayRequest>,
}

impl HostMod for AsksOnEveryEvent {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
        Ok(())
    }

    fn observe(&mut self, _event: GameEvent) -> Result<(), HostError> {
        self.asked.push(
            ravel_test_mod_audio::AudioMod::play(Side::Client)
                .expect("a client request is legal to build anywhere"),
        );
        Ok(())
    }

    fn take_play_requests(&mut self) -> Vec<PlayRequest> {
        std::mem::take(&mut self.asked)
    }
}

#[test]
fn a_server_process_queues_nothing_outward() {
    let _serial = outward::SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    outward::drain();

    let runtime = held(
        Side::DedicatedServer,
        Box::new(AsksOnEveryEvent {
            id: id("audio"),
            asked: Vec::new(),
        }),
    );
    runtime.observe(&target("server.tick"), &[]);

    assert_eq!(outward::drain(), "");
}

#[test]
fn the_same_mod_on_a_client_does_fill_it() {
    let _serial = outward::SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    outward::drain();

    let runtime = held(
        Side::Client,
        Box::new(AsksOnEveryEvent {
            id: id("audio"),
            asked: Vec::new(),
        }),
    );
    runtime.observe(&target("client.tick"), &[]);

    assert_eq!(
        outward::drain(),
        "1:11:110:audio.play1:35:audio10:door_opens1:1"
    );
}

fn forged_pair(forged: &'static str) -> LoaderRuntime {
    let mut machine = LifecycleMachine::new(
        Side::Client,
        [MOD, "innocent"]
            .into_iter()
            .map(|name| LifecycleEntry {
                mod_id: id(name),
                sides: SideSet::new([Side::Client]).expect("client"),
                requirement: Requirement::Optional,
                saved_data_versions: BTreeSet::new(),
            })
            .collect(),
    )
    .expect("pair");
    let mut mods: Vec<Box<dyn HostMod>> = [MOD, "innocent"]
        .into_iter()
        .map(|name| -> Box<dyn HostMod> {
            Box::new(Recording {
                id: id(name),
                log: Arc::new(Mutex::new(Log::default())),
                behaviour: if name == MOD {
                    Behaviour::Forges(forged)
                } else {
                    Behaviour::Records
                },
                claims: if name == MOD {
                    vec![key("go")]
                } else {
                    Vec::new()
                },
            })
        })
        .collect();
    for state in [
        LifecycleState::Validated,
        LifecycleState::Resolved,
        LifecycleState::Loaded,
        LifecycleState::Constructed,
    ] {
        machine.advance_all(state).expect("step");
    }
    for host in &mut mods {
        let mut window = machine.registration_window(host.id()).expect("window");
        host.register(&mut window).expect("register");
    }
    for state in [LifecycleState::Registered, LifecycleState::Running] {
        machine.advance_all(state).expect("step");
    }
    LoaderRuntime::new(machine, mods)
}

#[test]
fn forged_delivery_errors_fail_the_actual_event_or_command_owner() {
    for forged in ["unknown_owner", "innocent"] {
        for command in [false, true] {
            let runtime = forged_pair(forged);
            if command {
                runtime.observe(&target(COMMAND), &[HookValue::Text(format!("{MOD}:go"))]);
            } else {
                runtime.observe(&target("client.tick"), &[]);
            }
            let inner = runtime.inner.lock().expect("no panic");
            assert_eq!(inner.machine.state(&id(MOD)), Ok(LifecycleState::Failed));
            assert_eq!(
                inner.machine.state(&id("innocent")),
                Ok(LifecycleState::Running)
            );
            let none: [ravel_observability::Refusal; 0] = [];
            assert_eq!(inner.report.refusals(), none);
            assert_eq!(
                inner
                    .machine
                    .failure(&id(MOD))
                    .expect("owner")
                    .expect("failure")
                    .mod_id,
                Some(id(MOD))
            );
            drop(inner);
        }
    }
}

#[test]
fn a_mod_that_panicked_does_not_silence_every_later_hook() {
    let (runtime, _log) = running(Side::DedicatedServer, Vec::new(), Behaviour::Panics);

    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let deliveries = [(); 2].map(|()| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime.observe(&target("server.tick"), &[]);
        }))
    });
    std::panic::set_hook(previous);

    for (which, caught) in ["first", "second"].into_iter().zip(deliveries) {
        let raised = caught.expect_err("the mod under test panics on every event");
        let message = raised
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| raised.downcast_ref::<String>().map(String::as_str))
            .unwrap_or_default();
        assert_eq!(
            message, "this mod is broken",
            "the {which} delivery reached the mod; refusing a poisoned lock would \
             have raised about the lock instead, for the rest of the run"
        );
    }

    let inner = runtime
        .inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(
        inner.machine.state(&id(MOD)).expect("known mod"),
        LifecycleState::Running
    );
}

#[test]
fn an_installed_set_receives_what_the_observer_registry_delivers() {
    let (machine, mods, log) = parts(Side::DedicatedServer, Vec::new(), Behaviour::Records);

    super::install(machine, mods, super::pre_world_report())
        .expect("the registry accepts an observer");
    crate::dispatch::deliver(&target("server.tick"), &[]).expect("nothing here vetoes");

    assert_eq!(
        log.lock().expect("nothing panicked").seen,
        [GameEvent::ServerTick],
        "the join is the registry the guard already delivers through, not a second path"
    );
}

#[test]
fn every_tick_hook_reaches_the_mod_as_the_event_it_stands_for() {
    for (hook, expected) in EVENTS {
        let side = if expected == GameEvent::ClientTick {
            Side::Client
        } else {
            Side::DedicatedServer
        };
        let (runtime, log) = running(side, Vec::new(), Behaviour::Records);

        let outcome = runtime.observe(&target(hook), &[]);

        assert_eq!(outcome, HookOutcome::Proceed, "{hook}");
        assert_eq!(
            log.lock().expect("nothing panicked").seen,
            [expected],
            "{hook} should reach the mod as {expected:?}"
        );
    }
}

#[test]
fn two_hooks_can_mean_one_event_and_the_mod_is_told_the_event() {
    let (runtime, log) = running(Side::DedicatedServer, Vec::new(), Behaviour::Records);

    runtime.observe(&target("server.tick"), &[]);
    runtime.observe(&target("server.tick-integrated"), &[]);

    assert_eq!(
        log.lock().expect("nothing panicked").seen,
        [GameEvent::ServerTick, GameEvent::ServerTick],
        "a mod is told a server ticked, not which class ticked it"
    );
}

#[test]
fn a_hook_the_loader_has_no_meaning_for_delivers_nothing_and_proceeds() {
    let (runtime, log) = running(Side::Client, Vec::new(), Behaviour::Records);

    let outcome = runtime.observe(&target("chat.filter-text"), &[HookValue::Int(65)]);

    assert_eq!(outcome, HookOutcome::Proceed);
    let seen = log.lock().expect("nothing panicked");
    assert!(seen.seen.is_empty() && seen.ran.is_empty());
    drop(seen);
}

#[test]
fn a_typed_command_reaches_the_mod_that_claimed_its_key_with_its_arguments() {
    let (runtime, log) = running(Side::Client, vec![key("home")], Behaviour::Records);

    let outcome = runtime.observe(
        &target(COMMAND),
        &[HookValue::Text(format!("{MOD}:home set  spawn"))],
    );

    assert_eq!(outcome, HookOutcome::Proceed);
    let ran = &log.lock().expect("nothing panicked").ran;
    assert_eq!(ran.len(), 1);
    assert_eq!(ran[0].key, key("home"));
    assert_eq!(ran[0].side, Side::Client);
    assert_eq!(
        ran[0].arguments,
        ["set", "spawn"],
        "arguments are split on whitespace and not otherwise interpreted"
    );
}

#[test]
fn a_command_with_no_arguments_reaches_its_owner_carrying_none() {
    let (runtime, log) = running(Side::Client, vec![key("home")], Behaviour::Records);

    runtime.observe(&target(COMMAND), &[HookValue::Text(format!("{MOD}:home"))]);

    let ran = &log.lock().expect("nothing panicked").ran;
    assert_eq!(ran.len(), 1);
    let empty: [String; 0] = [];
    assert_eq!(ran[0].arguments, empty);
}

#[test]
fn a_command_no_mod_claimed_is_left_alone_rather_than_guessed_at() {
    let (runtime, log) = running(Side::Client, Vec::new(), Behaviour::Records);

    for typed in ["give @s stone", "minecraft:give @s stone", ""] {
        assert_eq!(
            runtime.observe(&target(COMMAND), &[HookValue::Text(typed.to_owned())]),
            HookOutcome::Proceed,
            "`{typed}` is not Ravel's to run"
        );
    }

    assert!(
        log.lock().expect("nothing panicked").ran.is_empty(),
        "an unqualified literal names no claim that could exist, and an \
         unclaimed one names no mod here"
    );
}

#[test]
fn the_typed_line_is_found_by_kind_rather_than_by_position() {
    let (runtime, log) = running(Side::Client, vec![key("home")], Behaviour::Records);

    runtime.observe(
        &target(COMMAND),
        &[HookValue::Int(1), HookValue::Text(format!("{MOD}:home"))],
    );

    assert_eq!(log.lock().expect("nothing panicked").ran.len(), 1);
}

#[test]
fn a_command_hook_carrying_no_text_reaches_nobody() {
    let (runtime, log) = running(Side::Client, vec![key("home")], Behaviour::Records);

    let outcome = runtime.observe(&target(COMMAND), &[HookValue::Int(1)]);

    assert_eq!(outcome, HookOutcome::Proceed);
    let empty: [CommandInvocation; 0] = [];
    assert_eq!(log.lock().expect("nothing panicked").ran, empty);
}

#[test]
fn a_mod_that_fails_while_reacting_is_failed_rather_than_thrown_at_the_game() {
    let (runtime, _log) = running(Side::DedicatedServer, Vec::new(), Behaviour::Fails);

    let outcome = runtime.observe(&target("server.tick"), &[]);

    assert_eq!(
        outcome,
        HookOutcome::Proceed,
        "a mod declining an event must not take the game down"
    );

    let inner = runtime.inner.lock().expect("nothing panicked");
    assert_eq!(
        inner.machine.state(&id(MOD)).expect("known mod"),
        LifecycleState::Failed
    );
    let attributed = inner
        .machine
        .failure(&id(MOD))
        .expect("known mod")
        .expect("the failure was recorded");
    assert_eq!(attributed.mod_id.as_ref(), Some(&id(MOD)));
    assert_eq!(attributed.state, LifecycleState::Running);
    assert_eq!(attributed.kind, None);
    assert!(attributed.message.text().contains("declines every event"));
    drop(inner);
}

#[test]
fn a_failed_mod_is_failed_once_however_many_ticks_follow() {
    let (runtime, _log) = running(Side::DedicatedServer, Vec::new(), Behaviour::Fails);

    for _ in 0..3 {
        assert_eq!(
            runtime.observe(&target("server.tick"), &[]),
            HookOutcome::Proceed
        );
    }

    let inner = runtime.inner.lock().expect("nothing panicked");
    let failures = inner
        .machine
        .transcript()
        .iter()
        .filter(|event| event.state == LifecycleState::Failed)
        .count();
    drop(inner);
    assert_eq!(
        failures, 1,
        "a mod that has already ended cannot fail again, and the machine says so"
    );
}

#[test]
fn every_hook_the_runtime_projects_is_one_the_reviewed_inventory_decided_about() {
    use ravel_minecraft::{HookId, VersionAdapter};
    use ravel_minecraft_adapter::PinnedAdapter;

    let reviewed: Vec<&str> = PinnedAdapter
        .targets()
        .iter()
        .map(|target| target.id().as_str())
        .collect();

    for hook in EVENTS.iter().map(|(hook, _)| *hook).chain([COMMAND]) {
        assert!(
            reviewed.contains(&hook) || PinnedAdapter.is_non_goal(HookId::new(hook)),
            "{hook} is neither a reviewed hook nor a declared non-goal of {}",
            PinnedAdapter.minecraft_version()
        );
    }
}

#[test]
fn the_command_seam_is_carried_by_every_version_that_could_decline_it() {
    use ravel_minecraft::VersionAdapter;
    use ravel_minecraft_adapter::PinnedAdapter;

    assert!(
        PinnedAdapter
            .targets()
            .iter()
            .any(|target| target.id().as_str() == COMMAND),
        "{COMMAND} is what makes a command reachable, so no version may decline it"
    );
}

#[test]
fn the_environment_a_failure_reports_is_the_process_it_happened_in() {
    let environment = environment();

    assert_eq!(environment.platform, std::env::consts::OS);
    assert_eq!(environment.architecture, std::env::consts::ARCH);
    assert_eq!(
        environment.minecraft,
        ravel_minecraft_adapter::pinned::MINECRAFT_VERSION
    );
}

#[test]
fn a_host_failure_carries_the_state_the_mod_named_and_no_dependency_outcome() {
    let failure = HostError::Refused {
        mod_id: id(MOD),
        state: LifecycleState::Running,
        detail: "no".to_owned(),
    };

    assert_eq!(failure.lifecycle_state(), LifecycleState::Running);
    assert_eq!(failure.failure_kind(), None);
    assert!(
        failure.quotes_untrusted_text(),
        "the attribution carries a mod's own words and the report surface redacts them"
    );
}
