use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use ravel_core::{
    ContentId, ContentKind, LifecycleState, ModId, RegistryKey, Side, SideSet, StructuredFailure,
};
use ravel_host_api::{CommandInvocation, GameEvent, HostError, HostMod, ModError, Registrar};
use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement};
use ravel_minecraft::{HookId, HookKind, HookObserver, HookOutcome, HookTarget, HookValue};

use super::{COMMAND, EVENTS, LoaderRuntime, environment};

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
    drained: Vec<ModId>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Behaviour {
    Records,
    Fails,
    Panics,
    Unsupported,
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

    fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
        Ok(())
    }

    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
        for claim in &self.claims {
            registrar
                .claim(claim.clone())
                .map_err(ModError::from_display)?;
        }
        Ok(())
    }

    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        match self.behaviour {
            Behaviour::Records => {
                self.log.lock().expect("nothing panicked").seen.push(event);
                Ok(())
            }
            Behaviour::Fails => Err(ModError::refused("this mod declines every event")),
            Behaviour::Unsupported => Err(ModError::Unsupported),
            Behaviour::Panics => panic!("this mod is broken"),
        }
    }

    fn run_command(&mut self, invocation: &CommandInvocation) -> Result<(), ModError> {
        if self.behaviour == Behaviour::Unsupported {
            return Err(ModError::Unsupported);
        }
        self.log
            .lock()
            .expect("nothing panicked")
            .ran
            .push(invocation.clone());
        Ok(())
    }

    fn take_play_requests(&mut self) -> Vec<PlayRequest> {
        self.log
            .lock()
            .expect("nothing panicked")
            .drained
            .push(self.id.clone());
        Vec::new()
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
    let runtime = playing(Side::Client);
    runtime.observe(&target("client.tick"), &[]);

    assert_eq!(
        runtime.drain_outward(),
        "1:11:110:audio.play1:35:audio10:door_opens1:1"
    );
}

#[test]
fn outward_actions_belong_only_to_the_runtime_that_produced_them() {
    let producing = playing(Side::Client);
    let idle = playing(Side::Client);
    producing.observe(&target("client.tick"), &[]);

    assert_eq!(idle.drain_outward(), "");
    assert_eq!(
        producing.drain_outward(),
        "1:11:110:audio.play1:35:audio10:door_opens1:1"
    );
    assert_eq!(producing.drain_outward(), "");
}

#[test]
fn a_delivery_a_mod_asked_nothing_on_queues_nothing() {
    let runtime = playing(Side::Client);
    runtime.observe(&target("resource.reload-client-packs"), &[]);

    assert_eq!(runtime.drain_outward(), "");
}

struct AsksOnEveryEvent {
    id: ModId,
    asked: Vec<PlayRequest>,
}

impl HostMod for AsksOnEveryEvent {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
        Ok(())
    }

    fn observe(&mut self, _event: GameEvent) -> Result<(), ModError> {
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
    let runtime = held(
        Side::DedicatedServer,
        Box::new(AsksOnEveryEvent {
            id: id("audio"),
            asked: Vec::new(),
        }),
    );
    runtime.observe(&target("server.tick"), &[]);

    assert_eq!(runtime.drain_outward(), "");
}

#[test]
fn the_same_mod_on_a_client_does_fill_it() {
    let runtime = held(
        Side::Client,
        Box::new(AsksOnEveryEvent {
            id: id("audio"),
            asked: Vec::new(),
        }),
    );
    runtime.observe(&target("client.tick"), &[]);

    assert_eq!(
        runtime.drain_outward(),
        "1:11:110:audio.play1:35:audio10:door_opens1:1"
    );
}

fn failing_pair(behaviour: Behaviour, log: &Arc<Mutex<Log>>) -> LoaderRuntime {
    let (machine, mods) = driven_pair(behaviour, log);
    LoaderRuntime::new(machine, mods)
}

fn driven_pair(
    behaviour: Behaviour,
    log: &Arc<Mutex<Log>>,
) -> (LifecycleMachine, Vec<Box<dyn HostMod>>) {
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
                log: Arc::clone(log),
                behaviour: if name == MOD {
                    behaviour
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
    (machine, mods)
}

#[test]
fn outward_drains_follow_lifecycle_order_after_service_slot_reordering() {
    let log = Arc::new(Mutex::new(Log::default()));
    let (machine, mut mods) = driven_pair(Behaviour::Records, &log);
    mods.swap(0, 1);
    let runtime = LoaderRuntime::new(machine, mods);

    runtime.observe(&target("client.tick"), &[]);

    assert_eq!(
        log.lock().expect("nothing panicked").drained,
        [id(MOD), id("innocent")]
    );
}

#[test]
fn a_missing_host_refuses_once_before_any_callback_on_later_ticks() {
    let log = Arc::new(Mutex::new(Log::default()));
    let runtime = failing_pair(Behaviour::Records, &log);
    runtime
        .inner
        .lock()
        .expect("nothing panicked")
        .mods
        .retain(|host| host.id().as_str() != "innocent");

    for _ in 0..3 {
        assert_eq!(
            runtime.observe(&target("client.tick"), &[]),
            HookOutcome::Proceed
        );
    }

    assert_eq!(log.lock().expect("nothing panicked").seen, []);
    assert_eq!(log.lock().expect("nothing panicked").drained, []);
    let inner = runtime.inner.lock().expect("nothing panicked");
    let [refusal] = inner.report.refusals() else {
        panic!("the incomplete set produces one refusal");
    };
    assert_eq!(refusal.attribution().mod_id, Some(id("innocent")));
    assert_eq!(refusal.attribution().state, LifecycleState::Running);
    assert_eq!(inner.machine.state(&id(MOD)), Ok(LifecycleState::Running));
    drop(inner);
}

#[test]
fn unsupported_delivery_errors_fail_the_actual_event_or_command_owner() {
    for command in [false, true] {
        let runtime = failing_pair(
            Behaviour::Unsupported,
            &Arc::new(Mutex::new(Log::default())),
        );
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
        let failure = inner
            .machine
            .failure(&id(MOD))
            .expect("owner")
            .expect("failure");
        assert_eq!(failure.mod_id, Some(id(MOD)));
        assert_eq!(
            failure.surface,
            Some(if command {
                ravel_core::FailureSurface::Registry(key("go"))
            } else {
                ravel_core::FailureSurface::Event(GameEvent::ClientTick.as_str().to_owned())
            })
        );
        drop(inner);
    }
}

#[test]
fn a_mod_that_panicked_does_not_silence_every_later_hook() {
    let log = Arc::new(Mutex::new(Log::default()));
    let runtime = failing_pair(Behaviour::Panics, &log);

    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let deliveries = [(); 2].map(|()| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime.observe(&target("client.tick"), &[]);
            log.lock()
                .expect("healthy callback did not panic")
                .seen
                .len()
        }))
    });
    std::panic::set_hook(previous);

    let observed_counts =
        deliveries.map(|caught| caught.expect("a callback panic is contained by loader delivery"));

    let inner = runtime
        .inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(
        inner.machine.state(&id(MOD)).expect("known mod"),
        LifecycleState::Failed
    );
    assert_eq!(
        inner.machine.state(&id("innocent")).expect("healthy mod"),
        LifecycleState::Running
    );
    assert!(
        observed_counts[1] > observed_counts[0],
        "after the failing first delivery, the later hook reaches the healthy mod"
    );
}

struct PanickingDrain {
    id: ModId,
    drains: Arc<std::sync::atomic::AtomicUsize>,
}

impl HostMod for PanickingDrain {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
        Ok(())
    }

    fn observe(&mut self, _event: GameEvent) -> Result<(), ModError> {
        Ok(())
    }

    fn take_play_requests(&mut self) -> Vec<PlayRequest> {
        self.drains
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        panic!("private drain callback payload");
    }
}

#[test]
fn panicking_outward_drain_is_attributed_and_never_called_again() {
    let (machine, _, _) = parts(Side::Client, Vec::new(), Behaviour::Records);
    let drains = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let runtime = LoaderRuntime::new(
        machine,
        vec![Box::new(PanickingDrain {
            id: id(MOD),
            drains: Arc::clone(&drains),
        })],
    );
    for _ in 0..2 {
        assert_eq!(
            runtime.observe(&target("client.tick"), &[]),
            HookOutcome::Proceed
        );
    }
    assert_eq!(drains.load(std::sync::atomic::Ordering::SeqCst), 1);
    let inner = runtime
        .inner
        .lock()
        .expect("the drain panic is caught before unlocking");
    assert_eq!(inner.machine.state(&id(MOD)), Ok(LifecycleState::Failed));
    let failure = inner
        .machine
        .failure(&id(MOD))
        .expect("owner")
        .expect("failure");
    assert_eq!(failure.mod_id, Some(id(MOD)));
    assert_eq!(failure.state, LifecycleState::Running);
    assert_eq!(failure.environment, environment());
    assert_eq!(failure.surface, None);
    assert!(!failure.message.text().contains("private drain"));
    drop(inner);
}

#[test]
fn owners_failed_before_outward_delivery_are_never_drained() {
    let (machine, _, _) = parts(Side::Client, Vec::new(), Behaviour::Records);
    let drains = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let runtime = LoaderRuntime::new(
        machine,
        vec![Box::new(PanickingDrain {
            id: id(MOD),
            drains: Arc::clone(&drains),
        })],
    );
    {
        let mut inner = runtime.inner.lock().expect("healthy lock");
        inner
            .record_failure(
                &id(MOD),
                &HostError::Panicked {
                    mod_id: id(MOD),
                    state: LifecycleState::Running,
                },
                None,
            )
            .expect("known owner can fail");
    }
    runtime.observe(&target("client.tick"), &[]);
    assert_eq!(drains.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn an_installed_set_receives_what_the_observer_registry_delivers() {
    let (machine, mods, log) = parts(Side::DedicatedServer, Vec::new(), Behaviour::Records);

    super::install(
        machine,
        mods,
        super::pre_world_report(),
        std::env::temp_dir(),
        ravel_loader::Supplied::default(),
        Vec::new(),
        false,
    )
    .expect("the registry accepts an observer");
    crate::dispatch::deliver(&target("server.tick"), &[]).expect("nothing here vetoes");

    assert_eq!(
        log.lock().expect("nothing panicked").seen,
        [GameEvent::ServerTick],
        "the join is the registry the guard already delivers through, not a second path"
    );
    let (machine, mods, replacement_log) =
        parts(Side::DedicatedServer, Vec::new(), Behaviour::Records);
    assert!(matches!(
        super::install(
            machine,
            mods,
            super::pre_world_report(),
            std::env::temp_dir(),
            ravel_loader::Supplied::default(),
            Vec::new(),
            true,
        ),
        Err(crate::guard::HookError::AlreadyInstalled)
    ));
    crate::dispatch::deliver(&target("server.tick"), &[])
        .expect("the original runtime remains active");
    assert_eq!(replacement_log.lock().expect("nothing panicked").seen, []);
    assert_eq!(log.lock().expect("nothing panicked").seen.len(), 2);
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
fn a_real_event_callback_failure_records_the_delivered_event_surface() {
    let (runtime, _log) = running(Side::DedicatedServer, Vec::new(), Behaviour::Fails);
    runtime.observe(&target("server.tick"), &[]);

    let inner = runtime
        .inner
        .lock()
        .expect("callback failure stays contained");
    let failure = inner
        .machine
        .failure(&id(MOD))
        .expect("known mod")
        .expect("the event callback really failed");
    assert_eq!(
        failure.surface,
        Some(ravel_core::FailureSurface::Event(
            GameEvent::ServerTick.as_str().to_owned()
        ))
    );
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
    use ravel_minecraft_adapter::pinned::{MINECRAFT_VERSION, NON_GOALS, TARGETS};

    let reviewed: Vec<&str> = TARGETS.iter().map(|target| target.id().as_str()).collect();

    for hook in EVENTS.iter().map(|(hook, _)| *hook).chain([COMMAND]) {
        assert!(
            reviewed.contains(&hook) || NON_GOALS.iter().any(|goal| goal.as_str() == hook),
            "{hook} is neither a reviewed hook nor a declared non-goal of {MINECRAFT_VERSION}",
        );
    }
}

#[test]
fn the_command_seam_is_carried_by_every_version_that_could_decline_it() {
    use ravel_minecraft_adapter::pinned::TARGETS;

    assert!(
        TARGETS.iter().any(|target| target.id().as_str() == COMMAND),
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
