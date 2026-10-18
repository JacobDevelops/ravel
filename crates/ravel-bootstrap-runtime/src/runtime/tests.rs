use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use ravel_core::{
    ContentId, ContentKind, LifecycleState, ModId, RegistryKey, Side, SideSet, StructuredFailure,
};
use ravel_host_api::{CommandInvocation, GameEvent, HostError, Mod, ModError, Registrar};
use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement, Supplied};
use ravel_minecraft::{HookId, HookKind, HookTarget, HookValue};

use super::dispatch::{COMMAND, EVENTS};
use super::{LoaderRuntime, advance_through, environment};

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

impl Mod for Recording {
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
        match self.behaviour {
            Behaviour::Fails => return Err(ModError::refused("choose a destination")),
            Behaviour::Panics => panic!("this command is broken"),
            Behaviour::Unsupported => return Err(ModError::Unsupported),
            Behaviour::Records => {}
        }
        self.log
            .lock()
            .expect("nothing panicked")
            .ran
            .push(invocation.clone());
        Ok(())
    }

    fn receive_packet(
        &mut self,
        _delivery: &ravel_network::PacketDelivery,
    ) -> Result<(), ModError> {
        match self.behaviour {
            Behaviour::Fails => Err(ModError::refused("this mod declines every packet")),
            Behaviour::Panics => panic!("this mod is broken"),
            Behaviour::Records => Ok(()),
            Behaviour::Unsupported => Err(ModError::Unsupported),
        }
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
    (
        LoaderRuntime::new(machine, mods, crate::runtime::test_launch()),
        log,
    )
}

fn parts(
    side: Side,
    claims: Vec<RegistryKey>,
    behaviour: Behaviour,
) -> (LifecycleMachine, Vec<Box<dyn Mod>>, Arc<Mutex<Log>>) {
    let log = Arc::new(Mutex::new(Log::default()));
    let mut host = Recording {
        id: id(MOD),
        log: Arc::clone(&log),
        behaviour,
        claims,
    };

    let entry = LifecycleEntry {
        mod_id: id(MOD),
        sides: SideSet::new([side, Side::IntegratedServer]).expect("a non-empty side set"),
        requirement: Requirement::Required,
        saved_data_versions: BTreeSet::new(),
    };
    let mut machine = LifecycleMachine::new(side, vec![entry]).expect("a legal plan");
    advance_through(&mut machine, LifecycleState::Constructed);
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

fn held(side: Side, host: Box<dyn Mod>) -> LoaderRuntime {
    let entry = LifecycleEntry {
        mod_id: host.id().clone(),
        sides: SideSet::new([side]).expect("a non-empty side set"),
        requirement: Requirement::Required,
        saved_data_versions: BTreeSet::new(),
    };
    let mut machine = LifecycleMachine::new(side, vec![entry]).expect("a legal plan");
    advance_through(&mut machine, LifecycleState::Running);
    LoaderRuntime::new(machine, vec![host], crate::runtime::test_launch())
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

impl Mod for AsksOnEveryEvent {
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

#[test]
fn an_outward_request_cannot_impersonate_another_owner() {
    let runtime = held(
        Side::Client,
        Box::new(AsksOnEveryEvent {
            id: id("thief"),
            asked: Vec::new(),
        }),
    );
    runtime.observe(&target("client.tick"), &[]);
    assert_eq!(runtime.drain_outward(), "");
    let inner = runtime.inner.lock().expect("healthy lock");
    assert_eq!(
        inner.loaded.machine().state(&id("thief")),
        Ok(LifecycleState::Failed)
    );
    let failure = inner
        .loaded
        .machine()
        .failure(&id("thief"))
        .expect("owner")
        .expect("attributed");
    assert_eq!(failure.mod_id, Some(id("thief")));
    drop(inner);
}

fn failing_pair(behaviour: Behaviour, log: &Arc<Mutex<Log>>) -> LoaderRuntime {
    let (machine, mods) = driven_pair(behaviour, log);
    LoaderRuntime::new(machine, mods, crate::runtime::test_launch())
}

fn driven_pair(
    behaviour: Behaviour,
    log: &Arc<Mutex<Log>>,
) -> (LifecycleMachine, Vec<Box<dyn Mod>>) {
    let mut machine = LifecycleMachine::new(
        Side::Client,
        [MOD, "innocent"]
            .into_iter()
            .map(|name| LifecycleEntry {
                mod_id: id(name),
                sides: SideSet::new([Side::Client, Side::IntegratedServer])
                    .expect("client and integrated server"),
                requirement: Requirement::Optional,
                saved_data_versions: BTreeSet::new(),
            })
            .collect(),
    )
    .expect("pair");
    let mut mods: Vec<Box<dyn Mod>> = [MOD, "innocent"]
        .into_iter()
        .map(|name| -> Box<dyn Mod> {
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
    advance_through(&mut machine, LifecycleState::Constructed);
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
    let runtime = LoaderRuntime::new(machine, mods, crate::runtime::test_launch());

    runtime.observe(&target("client.tick"), &[]);

    assert_eq!(
        log.lock().expect("nothing panicked").drained,
        [id(MOD), id("innocent")]
    );
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
        assert_eq!(
            inner.loaded.machine().state(&id(MOD)),
            Ok(LifecycleState::Failed)
        );
        assert_eq!(
            inner.loaded.machine().state(&id("innocent")),
            Ok(LifecycleState::Running)
        );
        let none: [ravel_observability::Refusal; 0] = [];
        assert_eq!(inner.report.refusals(), none);
        let failure = inner
            .loaded
            .machine()
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
        inner.loaded.machine().state(&id(MOD)).expect("known mod"),
        LifecycleState::Failed
    );
    assert_eq!(
        inner
            .loaded
            .machine()
            .state(&id("innocent"))
            .expect("healthy mod"),
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

impl Mod for PanickingDrain {
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
        crate::runtime::test_launch(),
    );
    for _ in 0..2 {
        runtime.observe(&target("client.tick"), &[]);
    }
    assert_eq!(drains.load(std::sync::atomic::Ordering::SeqCst), 1);
    let inner = runtime
        .inner
        .lock()
        .expect("the drain panic is caught before unlocking");
    assert_eq!(
        inner.loaded.machine().state(&id(MOD)),
        Ok(LifecycleState::Failed)
    );
    let failure = inner
        .loaded
        .machine()
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
    let (mut machine, _, _) = parts(Side::Client, Vec::new(), Behaviour::Records);
    let drains = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    machine
        .fail_callback(
            &HostError::Panicked {
                mod_id: id(MOD),
                state: LifecycleState::Running,
            },
            &environment(),
            None,
        )
        .expect("known owner can fail");
    let runtime = LoaderRuntime::new(
        machine,
        vec![Box::new(PanickingDrain {
            id: id(MOD),
            drains: Arc::clone(&drains),
        })],
        crate::runtime::test_launch(),
    );
    runtime.observe(&target("client.tick"), &[]);
    assert_eq!(drains.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn an_installed_set_receives_what_the_single_runtime_dispatch_delivers() {
    let (machine, mods, log) = parts(Side::DedicatedServer, Vec::new(), Behaviour::Records);

    super::install(
        super::Composition {
            loaded: ravel_loader::Loaded::new(machine, mods).expect("complete set"),
            configurations: Vec::new(),
            resources: Supplied::default(),
            interceptions: ravel_host_api::Points::default()
                .admit()
                .expect("empty interceptions"),
            installed_packages: BTreeMap::new(),
        },
        super::pre_world_report(),
        std::env::temp_dir(),
        ravel_observability::Profiler::off(),
    )
    .expect("the runtime installs");
    super::with_selected(|runtime| runtime.observe(&target("server.tick"), &[]));

    assert_eq!(
        log.lock().expect("nothing panicked").seen,
        [GameEvent::ServerTick],
        "the guard dispatches through the installed runtime"
    );
    let (machine, mods, replacement_log) =
        parts(Side::DedicatedServer, Vec::new(), Behaviour::Records);
    assert!(matches!(
        super::install(
            super::Composition {
                loaded: ravel_loader::Loaded::new(machine, mods).expect("complete set"),
                configurations: Vec::new(),
                resources: Supplied::default(),
                interceptions: ravel_host_api::Points::default()
                    .admit()
                    .expect("empty interceptions"),
                installed_packages: BTreeMap::new(),
            },
            super::pre_world_report(),
            std::env::temp_dir(),
            ravel_observability::Profiler::from_switch(true),
        ),
        Err(super::InstallError::AlreadyInstalled)
    ));
    super::with_selected(|runtime| runtime.observe(&target("server.tick"), &[]));
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

        runtime.observe(&target(hook), &[]);
        assert_eq!(
            log.lock().expect("nothing panicked").seen,
            [expected],
            "{hook} should reach the mod as {expected:?}"
        );
    }
}

#[test]
fn integrated_client_ticks_use_the_executing_side_without_superclass_duplicates()
-> Result<(), String> {
    let (runtime, log) = running(Side::Client, Vec::new(), Behaviour::Records);
    runtime.observe(&target("server.tick"), &[]);
    assert_eq!(
        log.lock().map_err(|error| error.to_string())?.seen,
        Vec::<GameEvent>::new()
    );
    runtime.observe(&target("server.tick-integrated"), &[]);
    assert_eq!(
        log.lock().map_err(|error| error.to_string())?.seen,
        [GameEvent::ServerTick]
    );
    Ok(())
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

    runtime.observe(&target("chat.filter-text"), &[HookValue::Int(65)]);
    let seen = log.lock().expect("nothing panicked");
    assert!(seen.seen.is_empty() && seen.ran.is_empty());
    drop(seen);
}

#[test]
fn a_typed_command_reaches_the_mod_that_claimed_its_key_with_its_arguments() {
    let (runtime, log) = running(Side::Client, vec![key("home")], Behaviour::Records);

    runtime.observe(
        &target(COMMAND),
        &[HookValue::Text(format!("{MOD}:home set  spawn"))],
    );

    let ran = &log.lock().expect("nothing panicked").ran;
    assert_eq!(ran.len(), 1);
    assert_eq!(ran[0].key, key("home"));
    assert_eq!(ran[0].side, Side::IntegratedServer);
    assert_eq!(
        ran[0].arguments,
        ["set", "spawn"],
        "arguments are split on whitespace and not otherwise interpreted"
    );
}

#[test]
fn integrated_commands_run_a_server_gated_fixture_on_a_client_distribution() {
    let host = ravel_test_mod_commands::__ravel_entry(Side::Client);
    let owner = host.id().clone();
    let entry = LifecycleEntry {
        mod_id: owner.clone(),
        sides: SideSet::new([Side::Client, Side::IntegratedServer]).expect("client and server"),
        requirement: Requirement::Required,
        saved_data_versions: BTreeSet::new(),
    };
    let mut machine = LifecycleMachine::new(Side::Client, vec![entry]).expect("one mod");
    let mut mods = vec![host];
    ravel_loader::start_all(
        &mut ravel_loader::LoadedMods::new(&mut machine, &mut mods).expect("complete set"),
        &environment(),
    )
    .expect("the fixture registers its commands");
    let runtime = LoaderRuntime::new(machine, mods, crate::runtime::test_launch());
    runtime.observe(
        &target(COMMAND),
        &[HookValue::Text(format!(
            "{owner}:{}",
            ravel_test_mod_commands::SERVER_PROBE
        ))],
    );
    let mut inner = runtime.inner.lock().expect("callback did not panic");
    let work = inner
        .command(&format!(
            "{owner}:{}",
            ravel_test_mod_commands::SERVER_PROBE
        ))
        .expect("a typed command");
    inner
        .deliver_checked(&work)
        .expect("the server-gated fixture executes with a server identity");
    assert_eq!(
        inner.loaded.machine().state(&owner),
        Ok(LifecycleState::Running)
    );
    assert_eq!(inner.loaded.machine().failure(&owner), Ok(None));
}

#[test]
fn integrated_commands_refuse_a_client_only_owner() {
    let host = ravel_test_mod_commands::__ravel_entry(Side::Client);
    let owner = host.id().clone();
    let entry = LifecycleEntry {
        mod_id: owner.clone(),
        sides: SideSet::new([Side::Client]).expect("client only"),
        requirement: Requirement::Required,
        saved_data_versions: BTreeSet::new(),
    };
    let mut machine = LifecycleMachine::new(Side::Client, vec![entry]).expect("one mod");
    let mut mods = vec![host];
    ravel_loader::start_all(
        &mut ravel_loader::LoadedMods::new(&mut machine, &mut mods).expect("complete set"),
        &environment(),
    )
    .expect("fixture registers its commands");
    let invocation = CommandInvocation::new(
        RegistryKey::new(
            ContentKind::Command,
            format!("{owner}:{}", ravel_test_mod_commands::SERVER_PROBE)
                .parse()
                .expect("command id"),
        ),
        Side::IntegratedServer,
        Vec::new(),
    );
    let refused = ravel_loader::deliver_command(
        &mut ravel_loader::LoadedMods::new(&mut machine, &mut mods).expect("complete set"),
        &invocation,
        &mut ravel_observability::Profiler::off(),
        &environment(),
    )
    .expect_err("client-only owners cannot execute integrated commands");
    assert_eq!(
        refused,
        ravel_loader::DispatchRefused::OwnerWrongSide {
            mod_id: owner.clone()
        }
    );
    assert_eq!(machine.state(&owner), Ok(LifecycleState::Running));
    assert_eq!(machine.failure(&owner), Ok(None));
    let runtime = LoaderRuntime::new(machine, mods, crate::runtime::test_launch());
    let mut inner = runtime.inner.lock().expect("healthy lock");
    assert_eq!(
        inner.deliver_checked(&super::dispatch::Work::Command(invocation)),
        Err(super::dispatch::Undelivered::Refused(refused))
    );
    assert_eq!(inner.report.refusals(), []);
    assert!(inner.loaded.machine().world_access(&inner.report).is_ok());
    drop(inner);
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
        runtime.observe(&target(COMMAND), &[HookValue::Text(typed.to_owned())]);
    }

    assert!(
        log.lock().expect("nothing panicked").ran.is_empty(),
        "an unqualified literal names no claim that could exist, and an \
         unclaimed one names no mod here"
    );
    let inner = runtime.inner.lock().expect("healthy lock");
    assert_eq!(inner.report.refusals(), []);
    assert!(inner.loaded.machine().world_access(&inner.report).is_ok());
    drop(inner);
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

    runtime.observe(&target(COMMAND), &[HookValue::Int(1)]);
    let empty: [CommandInvocation; 0] = [];
    assert_eq!(log.lock().expect("nothing panicked").ran, empty);
}

#[test]
fn a_mod_that_fails_while_reacting_is_failed_rather_than_thrown_at_the_game() {
    let (runtime, _log) = running(Side::DedicatedServer, Vec::new(), Behaviour::Fails);

    runtime.observe(&target("server.tick"), &[]);

    let inner = runtime.inner.lock().expect("nothing panicked");
    assert_eq!(
        inner.loaded.machine().state(&id(MOD)).expect("known mod"),
        LifecycleState::Failed
    );
    let attributed = inner
        .loaded
        .machine()
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
        .loaded
        .machine()
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
        runtime.observe(&target("server.tick"), &[]);
    }

    let inner = runtime.inner.lock().expect("nothing panicked");
    let failures = inner
        .loaded
        .machine()
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
fn every_direct_hook_projection_is_reviewed_for_the_selected_version() {
    use ravel_minecraft_adapter::pinned::{LEGACY_ADAPTER, MINECRAFT_VERSION, NON_GOALS, TARGETS};

    let reviewed: Vec<&str> = TARGETS.iter().map(|target| target.id().as_str()).collect();
    if LEGACY_ADAPTER {
        assert!(
            EVENTS.iter().all(|(hook, _)| !reviewed.contains(hook)) && !reviewed.contains(&COMMAND),
            "legacy semantic events and commands cross through the legacy adapter, not modern hooks"
        );
        return;
    }

    for hook in EVENTS.iter().map(|(hook, _)| *hook).chain([COMMAND]) {
        assert!(
            reviewed.contains(&hook) || NON_GOALS.iter().any(|goal| goal.as_str() == hook),
            "{hook} is neither a reviewed hook nor a declared non-goal of {MINECRAFT_VERSION}",
        );
    }
}

#[test]
fn the_command_seam_uses_the_selected_version_contract() {
    use ravel_minecraft_adapter::pinned::{LEGACY_ADAPTER, TARGETS};

    assert_eq!(
        TARGETS.iter().any(|target| target.id().as_str() == COMMAND),
        !LEGACY_ADAPTER,
        "modern versions receive commands through their reviewed hook; legacy uses its explicit adapter"
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

#[test]
fn legacy_observations_do_not_duplicate_checked_tick_and_reload_events() -> Result<(), String> {
    let (runtime, log) = running(Side::Client, Vec::new(), Behaviour::Records);
    for raw in [
        "legacy.client-tick",
        "legacy.server-tick",
        "legacy.integrated-server-tick",
        "legacy.server-world-tick",
        "legacy.resource-reload",
        "legacy.atlas-reload",
    ] {
        runtime.observe(&target(raw), &[]);
    }
    assert_eq!(
        log.lock().map_err(|error| error.to_string())?.seen,
        Vec::<GameEvent>::new()
    );
    runtime
        .legacy_event(GameEvent::ClientTick)
        .map_err(|error| error.to_string())?;
    runtime
        .legacy_event(GameEvent::ResourcesReloaded)
        .map_err(|error| error.to_string())?;
    assert_eq!(
        log.lock().map_err(|error| error.to_string())?.seen,
        [GameEvent::ClientTick, GameEvent::ResourcesReloaded]
    );
    Ok(())
}

#[test]
fn legacy_base_server_tick_is_the_only_simulation_event() -> Result<(), String> {
    let (runtime, log) = running(Side::IntegratedServer, Vec::new(), Behaviour::Records);
    runtime.observe(&target("legacy.integrated-server-tick"), &[]);
    runtime
        .legacy_event(GameEvent::ServerTick)
        .map_err(|error| error.to_string())?;
    runtime.observe(&target("legacy.server-world-tick"), &[]);
    runtime.observe(&target("legacy.integrated-server-tick"), &[]);
    assert_eq!(
        log.lock().map_err(|error| error.to_string())?.seen,
        [GameEvent::ServerTick]
    );
    Ok(())
}

#[test]
fn legacy_commands_preserve_arguments_and_refuse_unclaimed_roots() -> Result<(), String> {
    let (runtime, log) = running(Side::DedicatedServer, vec![key("warp")], Behaviour::Records);
    runtime
        .claim_command_thread()
        .map_err(|error| error.to_string())?;
    runtime
        .legacy_command("example_mod:warp set home")
        .map_err(|error| error.to_string())?;
    assert!(
        runtime
            .legacy_command("example_mod:missing set home")
            .is_err()
    );
    let log = log.lock().map_err(|error| error.to_string())?;
    assert_eq!(log.ran.len(), 1);
    assert_eq!(log.ran[0].arguments, ["set", "home"]);
    Ok(())
}

#[test]
fn legacy_packets_reject_untrusted_input_without_poisoning_the_recipient() -> Result<(), String> {
    let runtime = held(
        Side::DedicatedServer,
        Box::new(ravel_test_mod_networking::NetworkingMod::new()),
    );
    let ping = ravel_test_mod_networking::Ping::noted(71, "legacy", true);
    let bytes =
        ravel_network::encode_from(&ping, Side::Client).map_err(|error| error.to_string())?;
    runtime
        .legacy_packet(ravel_test_mod_networking::ping_id(), bytes.clone(), true)
        .map_err(|error| error.to_string())?;
    for payload in [vec![0], [bytes.clone(), vec![0]].concat()] {
        assert!(
            runtime
                .legacy_packet(ravel_test_mod_networking::ping_id(), payload, true)
                .is_err()
        );
    }
    assert!(
        runtime
            .legacy_packet(
                ravel_network::PacketId::new(
                    "networking:unknown"
                        .parse()
                        .map_err(|error: ravel_core::ContentIdError| error.to_string())?
                ),
                bytes.clone(),
                true,
            )
            .is_err()
    );
    {
        let inner = runtime.inner.lock().map_err(|error| error.to_string())?;
        assert_eq!(
            inner
                .loaded
                .machine()
                .state(&id("networking"))
                .map_err(|error| error.to_string())?,
            LifecycleState::Running
        );
        assert!(inner.loaded.machine().save_access().is_ok());
        assert_eq!(inner.report.refusals(), []);
        drop(inner);
    }
    runtime
        .legacy_packet(ravel_test_mod_networking::ping_id(), bytes, true)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[test]
fn peer_selected_packet_contract_direction_cannot_fail_the_owner() -> Result<(), String> {
    let runtime = held(
        Side::Client,
        Box::new(ravel_test_mod_networking::NetworkingMod::new()),
    );
    let ping = ravel_test_mod_networking::Ping::noted(72, "misdirected", false);
    let bytes =
        ravel_network::encode_from(&ping, Side::Client).map_err(|error| error.to_string())?;
    assert!(
        runtime
            .legacy_packet(ravel_test_mod_networking::ping_id(), bytes, false)
            .is_err()
    );
    let inner = runtime.inner.lock().map_err(|error| error.to_string())?;
    assert_eq!(
        inner
            .loaded
            .machine()
            .state(&id("networking"))
            .map_err(|error| error.to_string())?,
        LifecycleState::Running
    );
    assert!(inner.loaded.machine().save_access().is_ok());
    assert_eq!(inner.report.refusals(), []);
    drop(inner);
    Ok(())
}

fn integrated_composition() -> (super::Composition, Arc<Mutex<Log>>) {
    let (machine, mods, log) = parts(Side::IntegratedServer, Vec::new(), Behaviour::Records);
    (
        super::Composition {
            loaded: ravel_loader::Loaded::new(machine, mods).expect("complete set"),
            configurations: Vec::new(),
            resources: Supplied::default(),
            interceptions: ravel_host_api::Points::default()
                .admit()
                .expect("empty interceptions"),
            installed_packages: BTreeMap::default(),
        },
        log,
    )
}

#[test]
fn integrated_composition_is_thread_owned_and_recreated_for_each_server() -> Result<(), String> {
    let client_thread_state = super::integrated_installed();
    for _ in 0..2 {
        let log = std::thread::spawn(|| -> Result<Arc<Mutex<Log>>, String> {
            assert!(!super::integrated_installed());
            let (composition, log) = integrated_composition();
            super::install_integrated(
                composition,
                std::env::temp_dir(),
                ravel_observability::Profiler::from_switch(true),
            )
            .map_err(|error| error.to_string())?;
            let runtime = super::installation::with_thread_selected(Arc::clone)
                .ok_or("missing thread-owned runtime")?;
            assert_eq!(
                runtime
                    .inner
                    .lock()
                    .map_err(|error| error.to_string())?
                    .loaded
                    .machine()
                    .side(),
                Side::IntegratedServer
            );
            runtime
                .legacy_event(GameEvent::ServerTick)
                .map_err(|error| error.to_string())?;
            assert!(runtime.legacy_event(GameEvent::ClientTick).is_err());
            let (duplicate, _) = integrated_composition();
            assert_eq!(
                super::install_integrated(
                    duplicate,
                    std::env::temp_dir(),
                    ravel_observability::Profiler::off()
                ),
                Err(super::InstallError::AlreadyInstalled)
            );
            let (mut wrong_side, _) = integrated_composition();
            let (machine, mods, _) = parts(Side::Client, Vec::new(), Behaviour::Records);
            wrong_side.loaded = ravel_loader::Loaded::new(machine, mods).expect("complete set");
            assert_eq!(
                super::install_integrated(
                    wrong_side,
                    std::env::temp_dir(),
                    ravel_observability::Profiler::off()
                ),
                Err(super::InstallError::WrongSide)
            );
            Ok(log)
        })
        .join()
        .map_err(|_| "integrated server thread panicked")??;
        assert!(super::installation::integrated_runtime().is_none());
        assert_eq!(
            log.lock().map_err(|error| error.to_string())?.seen,
            [GameEvent::ServerTick]
        );
        assert_eq!(super::integrated_installed(), client_thread_state);
    }
    Ok(())
}

#[test]
fn integrated_install_uses_explicit_context_without_a_process_runtime() {
    const CHILD: &str = "RAVEL_INTEGRATED_CONTEXT_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args(["--exact", "runtime::tests::integrated_install_uses_explicit_context_without_a_process_runtime", "--nocapture"])
            .env(CHILD, "1")
            .output().expect("isolated integrated install");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    assert!(!super::is_installed());
    assert!(matches!(
        super::process_launch_context(),
        Err(super::InstallError::ProcessNotInstalled)
    ));
    let directory = std::env::temp_dir().join("ravel-explicit-integrated-directory");
    let (composition, _) = integrated_composition();
    super::install_integrated(
        composition,
        directory.clone(),
        ravel_observability::Profiler::from_switch(true),
    )
    .expect("explicit integrated context requires no process runtime");
    let runtime =
        super::installation::with_thread_selected(Arc::clone).expect("integrated runtime");
    assert_eq!(runtime.launch.game_directory, directory);
    assert!(runtime.inner.lock().expect("healthy lock").profiler.is_on());
    assert!(!super::is_installed());
}

#[test]
fn checked_events_surface_callback_failures_then_stop_delivery() -> Result<(), String> {
    let (runtime, _) = running(Side::Client, Vec::new(), Behaviour::Fails);
    let refused = runtime.legacy_event(GameEvent::ServerTick);
    assert_eq!(
        refused,
        Err(super::dispatch::Undelivered::Refused(
            ravel_loader::DispatchRefused::EventWrongSide {
                event: GameEvent::ServerTick,
                side: Side::Client,
            }
        ))
    );
    assert_eq!(
        refused.expect_err("wrong side").to_string(),
        "ServerTick does not happen on Client"
    );
    assert!(runtime.legacy_event(GameEvent::ClientTick).is_err());
    assert_eq!(
        runtime
            .inner
            .lock()
            .map_err(|error| error.to_string())?
            .loaded
            .machine()
            .state(&id(MOD))
            .map_err(|error| error.to_string())?,
        LifecycleState::Failed
    );
    assert!(runtime.legacy_event(GameEvent::ResourcesReloaded).is_ok());
    Ok(())
}

#[test]
fn checked_commands_and_packets_respect_real_halts_without_launch_refusals() -> Result<(), String> {
    let (runtime, log) = running(Side::DedicatedServer, vec![key("warp")], Behaviour::Panics);
    runtime
        .claim_command_thread()
        .map_err(|error| error.to_string())?;
    {
        let mut inner = runtime.inner.lock().map_err(|error| error.to_string())?;
        inner
            .loaded
            .record_world_mutation()
            .map_err(|error| error.to_string())?;
    }
    assert!(runtime.legacy_event(GameEvent::ServerTick).is_err());
    assert!(!runtime.accepts_packet_owner(&id(MOD)));
    for _ in 0..3 {
        assert!(runtime.legacy_command("example_mod:warp home").is_err());
        assert!(
            runtime
                .legacy_packet(ravel_test_mod_networking::ping_id(), Vec::new(), true,)
                .is_err()
        );
    }
    let inner = runtime.inner.lock().map_err(|error| error.to_string())?;
    assert_eq!(inner.report.refusals(), []);
    assert!(inner.loaded.machine().world_access(&inner.report).is_err());
    drop(inner);
    assert_eq!(log.lock().map_err(|error| error.to_string())?.ran, []);
    Ok(())
}

#[test]
fn packet_admission_rejects_required_failure_before_the_halt_latch() {
    let entries = [MOD, "innocent"]
        .into_iter()
        .map(|name| LifecycleEntry {
            mod_id: id(name),
            sides: SideSet::new([Side::Client]).expect("client side"),
            requirement: Requirement::Required,
            saved_data_versions: BTreeSet::new(),
        })
        .collect();
    let mut machine = LifecycleMachine::new(Side::Client, entries).expect("two required mods");
    advance_through(&mut machine, LifecycleState::Running);
    let mods = [MOD, "innocent"]
        .into_iter()
        .map(|name| -> Box<dyn Mod> {
            Box::new(Recording {
                id: id(name),
                log: Arc::new(Mutex::new(Log::default())),
                behaviour: if name == MOD {
                    Behaviour::Panics
                } else {
                    Behaviour::Records
                },
                claims: Vec::new(),
            })
        })
        .collect();
    let runtime = LoaderRuntime::new(machine, mods, crate::runtime::test_launch());
    assert!(runtime.accepts_packet_owner(&id("innocent")));
    assert!(runtime.legacy_event(GameEvent::ClientTick).is_err());
    {
        let inner = runtime.inner.lock().expect("healthy lock");
        assert!(inner.loaded.machine().save_access().is_ok());
        assert_eq!(
            inner.loaded.machine().state(&id(MOD)),
            Ok(LifecycleState::Failed)
        );
        assert_eq!(
            inner.loaded.machine().state(&id("innocent")),
            Ok(LifecycleState::Running)
        );
        assert_eq!(inner.report.refusals(), []);
    }
    assert!(!runtime.accepts_packet_owner(&id("innocent")));
    assert!(
        runtime
            .legacy_packet(
                ravel_network::PacketId::new("innocent:ping".parse().expect("packet id")),
                Vec::new(),
                false,
            )
            .is_err()
    );
}

#[test]
fn checked_commands_and_packets_charge_the_existing_profiler() -> Result<(), String> {
    let (runtime, _) = running(Side::DedicatedServer, vec![key("warp")], Behaviour::Records);
    runtime
        .inner
        .lock()
        .map_err(|error| error.to_string())?
        .profiler = ravel_observability::Profiler::from_switch(true);
    runtime
        .claim_command_thread()
        .map_err(|error| error.to_string())?;
    runtime
        .legacy_command("example_mod:warp home")
        .map_err(|error| error.to_string())?;
    assert_eq!(runtime.profiling_report().mods()[0].cost.command_calls, 1);
    let packet_runtime = held(
        Side::DedicatedServer,
        Box::new(ravel_test_mod_networking::NetworkingMod::new()),
    );
    packet_runtime
        .inner
        .lock()
        .map_err(|error| error.to_string())?
        .profiler = ravel_observability::Profiler::from_switch(true);
    packet_runtime
        .legacy_packet(
            ravel_test_mod_networking::ping_id(),
            ravel_network::encode(&ravel_test_mod_networking::Ping::new(1)),
            true,
        )
        .map_err(|error| error.to_string())?;
    assert_eq!(
        packet_runtime.profiling_report().mods()[0].cost.event_calls,
        1
    );
    Ok(())
}

#[test]
fn unsupported_packet_is_invalid_input_not_an_owner_failure() -> Result<(), String> {
    let (runtime, _) = running(Side::DedicatedServer, Vec::new(), Behaviour::Unsupported);
    let packet = ravel_network::PacketId::new(
        format!("{MOD}:packet")
            .parse()
            .map_err(|error: ravel_core::ContentIdError| error.to_string())?,
    );
    assert!(runtime.legacy_packet(packet, Vec::new(), true).is_err());
    let inner = runtime.inner.lock().map_err(|error| error.to_string())?;
    assert_eq!(
        inner
            .loaded
            .machine()
            .state(&id(MOD))
            .map_err(|error| error.to_string())?,
        LifecycleState::Running
    );
    assert!(inner.loaded.machine().save_access().is_ok());
    drop(inner);
    Ok(())
}

#[test]
fn packet_callback_failure_is_attributed() -> Result<(), String> {
    let (runtime, _) = running(Side::DedicatedServer, Vec::new(), Behaviour::Fails);
    let packet = ravel_network::PacketId::new(
        format!("{MOD}:packet")
            .parse()
            .map_err(|error: ravel_core::ContentIdError| error.to_string())?,
    );
    assert!(runtime.legacy_packet(packet, Vec::new(), true).is_err());
    assert_eq!(
        runtime
            .inner
            .lock()
            .map_err(|error| error.to_string())?
            .loaded
            .machine()
            .state(&id(MOD))
            .map_err(|error| error.to_string())?,
        LifecycleState::Failed
    );
    Ok(())
}

#[test]
fn native_command_inventory_binds_the_actual_server_thread() -> Result<(), String> {
    let (runtime, log) = running(Side::DedicatedServer, vec![key("warp")], Behaviour::Records);
    let runtime = Arc::new(runtime);
    assert!(runtime.legacy_command("example_mod:warp home").is_err());
    runtime
        .claim_command_thread()
        .map_err(|error| error.to_string())?;
    runtime
        .claim_command_thread()
        .map_err(|error| error.to_string())?;
    runtime
        .legacy_command("example_mod:warp home")
        .map_err(|error| error.to_string())?;
    let remote = Arc::clone(&runtime);
    std::thread::spawn(move || {
        assert!(remote.claim_command_thread().is_err());
        assert!(remote.legacy_command("example_mod:warp home").is_err());
    })
    .join()
    .map_err(|_| "remote command test thread panicked")?;
    assert_eq!(log.lock().map_err(|error| error.to_string())?.ran.len(), 1);
    let (client, _) = running(Side::Client, Vec::new(), Behaviour::Records);
    assert!(client.claim_command_thread().is_err());
    Ok(())
}

#[test]
fn checked_command_outcomes_preserve_dispatch_refusals_and_callback_failures() -> Result<(), String>
{
    use super::dispatch::Undelivered;
    use std::error::Error;

    let (runtime, _) = running(Side::DedicatedServer, vec![key("warp")], Behaviour::Fails);
    let precondition = runtime
        .legacy_command("example_mod:warp")
        .expect_err("the command thread is not claimed");
    assert!(matches!(precondition, Undelivered::Precondition(_)));
    assert!(precondition.source().is_none());
    runtime
        .claim_command_thread()
        .map_err(|error| error.to_string())?;
    let refused = runtime
        .legacy_command("example_mod:missing")
        .expect_err("the command is not claimed");
    assert!(matches!(
        refused,
        Undelivered::Refused(ravel_loader::DispatchRefused::Unclaimed { .. })
    ));
    let Undelivered::Refused(expected) = &refused else {
        unreachable!("the checked refusal variant");
    };
    assert_eq!(
        refused
            .source()
            .and_then(|error| error.downcast_ref::<ravel_loader::DispatchRefused>()),
        Some(expected)
    );
    let declined = runtime
        .legacy_command("example_mod:warp")
        .expect_err("the callback declines the command");
    assert!(matches!(
        &declined,
        Undelivered::Declined(HostError::OperationRefused { detail, .. })
            if detail == "choose a destination"
    ));
    let Undelivered::Declined(expected) = &declined else {
        unreachable!("the checked decline variant");
    };
    assert_eq!(
        declined
            .source()
            .and_then(|error| error.downcast_ref::<HostError>()),
        Some(expected)
    );
    let (panicking, _) = running(Side::DedicatedServer, vec![key("warp")], Behaviour::Panics);
    panicking
        .claim_command_thread()
        .map_err(|error| error.to_string())?;
    let failed = panicking
        .legacy_command("example_mod:warp")
        .expect_err("the callback panics");
    assert!(matches!(
        &failed,
        Undelivered::Failed(HostError::Panicked { .. })
    ));
    let Undelivered::Failed(expected) = &failed else {
        unreachable!("the checked failure variant");
    };
    assert_eq!(
        failed
            .source()
            .and_then(|error| error.downcast_ref::<HostError>()),
        Some(expected)
    );
    Ok(())
}

#[test]
fn integrated_ticks_exclude_client_only_and_parked_server_only_owners()
-> Result<(), Box<dyn std::error::Error>> {
    let log = Arc::new(Mutex::new(Log::default()));
    let entries = [
        ("client_only", vec![Side::Client]),
        ("both", vec![Side::Client, Side::IntegratedServer]),
        ("server_only", vec![Side::IntegratedServer]),
    ]
    .into_iter()
    .map(|(name, sides)| {
        Ok(LifecycleEntry {
            mod_id: name.parse()?,
            sides: SideSet::new(sides)?,
            requirement: Requirement::Required,
            saved_data_versions: BTreeSet::new(),
        })
    })
    .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    let mut machine = LifecycleMachine::new(Side::Client, entries)?;
    advance_through(&mut machine, LifecycleState::Running);
    let mods = ["client_only", "both"]
        .into_iter()
        .map(|name| {
            let module: Box<dyn Mod> = Box::new(Recording {
                id: name.parse()?,
                log: Arc::clone(&log),
                behaviour: Behaviour::Records,
                claims: Vec::new(),
            });
            Ok(module)
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    let runtime = LoaderRuntime::new(machine, mods, crate::runtime::test_launch());
    runtime.observe(&target("server.tick-integrated"), &[]);
    assert_eq!(
        log.lock().map_err(|error| error.to_string())?.seen,
        [GameEvent::ServerTick]
    );
    Ok(())
}
