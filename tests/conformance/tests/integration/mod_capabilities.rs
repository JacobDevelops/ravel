//! Command routing, event delivery, resource layering, and dispatch boundaries.
//!
//! Tests stop the lifecycle at Running so capabilities can be exercised.

use std::collections::BTreeSet;

use ravel_core::{ContentKind, LifecycleState, ModId, RegistryKey, Side, SideSet};
use ravel_data::ResourceSet;
use ravel_host_api::{CommandInvocation, GameEvent, HostError, HostMod};
use ravel_loader::{
    DispatchRefused, LifecycleEntry, LifecycleMachine, Requirement, broadcast, route,
};
use ravel_test_mod_commands::CommandsMod;
use ravel_test_mod_events::EventsMod;
use ravel_test_mod_resources::ResourcesMod;

fn id(text: &str) -> ModId {
    text.parse().expect("a legal mod id")
}

fn entry(name: &str, sides: &[Side]) -> LifecycleEntry {
    LifecycleEntry {
        mod_id: id(name),
        sides: SideSet::new(sides.iter().copied()).expect("a non-empty side set"),
        requirement: Requirement::Required,
        saved_data_versions: BTreeSet::new(),
    }
}

fn running(
    entries: Vec<LifecycleEntry>,
    side: Side,
    mods: &mut [&mut dyn HostMod],
) -> LifecycleMachine {
    let mut machine = LifecycleMachine::new(side, entries).expect("a legal lifecycle plan");
    ravel_loader::drive_loaded(
        &mut ravel_loader::LoadedMods::new(&mut machine, mods).expect("all active mods have hosts"),
        &ravel_host_runtime::environment(),
    )
    .expect("the production driver starts the set");
    machine
}

fn three_mod_plan() -> Vec<LifecycleEntry> {
    vec![
        entry("commands", &[Side::Client, Side::DedicatedServer]),
        entry("events", &[Side::Client, Side::DedicatedServer]),
        entry("resources", &[Side::Client, Side::DedicatedServer]),
    ]
}

#[test]
fn a_running_set_owns_its_commands_and_routes_one_to_the_mod_that_claimed_it() {
    let mut commands = CommandsMod::new();
    let mut events = EventsMod::new();
    let mut resources = ResourcesMod::new();
    let mut mods: Vec<&mut dyn HostMod> = vec![&mut commands, &mut events, &mut resources];
    let mut machine = running(three_mod_plan(), Side::Client, &mut mods);

    for key in CommandsMod::new().claims() {
        assert_eq!(machine.owner_of(&key), Some(&id("commands")));
    }

    let invocation = CommandInvocation::new(
        CommandsMod::new().key("warp"),
        Side::Client,
        vec!["set".to_owned(), "base".to_owned()],
    );
    let delivery = route(
        &mut ravel_loader::LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
            .expect("all active mods have hosts"),
        &invocation,
    )
    .expect("the owner is running");
    assert_eq!(delivery.received(), [id("commands")]);
    assert!(delivery.is_complete());

    drop(mods);
    assert_eq!(commands.invocations(), [invocation]);
    assert_eq!(events.observed(), []);
    assert_eq!(resources.reloads(), 0);
}

#[test]
fn a_broadcast_reaches_every_mod_and_each_one_acts_on_what_it_cares_about() {
    let mut commands = CommandsMod::new();
    let mut events = EventsMod::new();
    let mut resources = ResourcesMod::new();
    let mut mods: Vec<&mut dyn HostMod> = vec![&mut commands, &mut events, &mut resources];
    let mut machine = running(three_mod_plan(), Side::Client, &mut mods);

    let delivery = broadcast(
        &mut ravel_loader::LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
            .expect("all active mods have hosts"),
        GameEvent::ResourcesReloaded,
    )
    .expect("the set is running");
    assert_eq!(
        delivery.received(),
        [id("commands"), id("events"), id("resources")]
    );
    assert!(delivery.is_complete());

    drop(mods);
    assert_eq!(events.observed(), [GameEvent::ResourcesReloaded]);
    assert_eq!(resources.reloads(), 1);
    assert_eq!(commands.invocations(), []);
}

#[test]
fn resources_layer_so_the_last_mod_to_declare_a_path_is_the_one_that_supplies_it() {
    let resources = ResourcesMod::new();
    let declared = resources.declared();
    let shared = declared[0].clone();
    let own = declared[1].clone();
    let later = id("skin_mod");

    let set = resources.layered_under(&later, std::slice::from_ref(&shared));

    assert_eq!(set.supplier(&shared), Some(&later));
    assert_eq!(set.supplier(&own), Some(&id("resources")));
    assert_eq!(set.overrides().len(), 1);
    assert_eq!(set.overrides()[0].shadowed, [id("resources")]);
}

#[test]
fn a_broadcast_arrives_in_the_plans_order_and_the_reversed_plan_reverses_it() {
    let observed_order = |reversed: bool| {
        let mut first = EventsMod::new();
        let mut second = CommandsMod::new();
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut first, &mut second];
        let mut plan = vec![
            entry("events", &[Side::Client]),
            entry("commands", &[Side::Client]),
        ];
        if reversed {
            plan.reverse();
        }
        let mut machine = running(plan, Side::Client, &mut mods);
        broadcast(
            &mut ravel_loader::LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active mods have hosts"),
            GameEvent::ClientTick,
        )
        .expect("the set is running")
        .received()
        .to_vec()
    };

    assert_eq!(observed_order(false), [id("events"), id("commands")]);
    assert_eq!(observed_order(true), [id("commands"), id("events")]);
}

#[test]
fn the_same_set_broadcast_twice_delivers_to_the_same_mods_in_the_same_order() {
    let run_once = || {
        let mut commands = CommandsMod::new();
        let mut events = EventsMod::new();
        let mut resources = ResourcesMod::new();
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut commands, &mut events, &mut resources];
        let mut machine = running(three_mod_plan(), Side::Client, &mut mods);
        let audience = machine.audience(GameEvent::ClientTick);
        let received = broadcast(
            &mut ravel_loader::LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active mods have hosts"),
            GameEvent::ClientTick,
        )
        .expect("the set is running")
        .received()
        .to_vec();
        (audience, received)
    };

    let first = run_once();
    assert_eq!(first, run_once());
    assert_eq!(first.0, first.1);
}

#[test]
fn a_mod_sees_repeated_ticks_in_the_order_they_were_broadcast() {
    let mut events = EventsMod::new();
    let mut mods: Vec<&mut dyn HostMod> = vec![&mut events];
    let mut machine = running(
        vec![entry("events", &[Side::Client])],
        Side::Client,
        &mut mods,
    );

    for event in [
        GameEvent::ClientTick,
        GameEvent::ResourcesReloaded,
        GameEvent::ClientTick,
    ] {
        broadcast(
            &mut ravel_loader::LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active mods have hosts"),
            event,
        )
        .expect("the set is running");
    }

    drop(mods);
    assert_eq!(
        events.observed(),
        [
            GameEvent::ClientTick,
            GameEvent::ResourcesReloaded,
            GameEvent::ClientTick,
        ]
    );
    assert_eq!(events.count(GameEvent::ClientTick), 2);
}

#[test]
fn a_client_tick_reaches_nobody_on_a_dedicated_server_and_a_server_tick_reaches_everybody() {
    let mut commands = CommandsMod::new();
    let mut events = EventsMod::new();
    let mut resources = ResourcesMod::new();
    let mut mods: Vec<&mut dyn HostMod> = vec![&mut commands, &mut events, &mut resources];
    let mut machine = running(three_mod_plan(), Side::DedicatedServer, &mut mods);

    assert_eq!(machine.audience(GameEvent::ClientTick), Vec::<ModId>::new());
    let ignored = broadcast(
        &mut ravel_loader::LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
            .expect("all active mods have hosts"),
        GameEvent::ClientTick,
    )
    .expect("an empty audience");
    assert_eq!(ignored.received(), []);

    let ticked = broadcast(
        &mut ravel_loader::LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
            .expect("all active mods have hosts"),
        GameEvent::ServerTick,
    )
    .expect("the set is running");
    assert_eq!(
        ticked.received(),
        [id("commands"), id("events"), id("resources")]
    );

    drop(mods);
    assert_eq!(events.observed(), [GameEvent::ServerTick]);
}

#[test]
fn a_mod_that_does_not_load_on_this_side_is_not_in_any_audience() {
    let machine = LifecycleMachine::new(
        Side::DedicatedServer,
        vec![
            entry("commands", &[Side::Client, Side::DedicatedServer]),
            entry("events", &[Side::Client]),
        ],
    )
    .expect("a legal plan");

    for event in GameEvent::ALL {
        assert!(
            !machine.audience(event).contains(&id("events")),
            "a client-only mod is in the audience for {event}"
        );
    }
    assert_eq!(machine.audience(GameEvent::ServerTick), [id("commands")]);
}

#[test]
fn a_command_owned_by_a_mod_that_does_not_load_here_is_refused_rather_than_swallowed() {
    let mut commands = CommandsMod::new();
    let mut mods: Vec<&mut dyn HostMod> = vec![&mut commands];
    let machine = running(
        vec![entry("commands", &[Side::Client])],
        Side::Client,
        &mut mods,
    );
    let owned = CommandsMod::new().key("warp");
    assert_eq!(machine.owner_of(&owned), Some(&id("commands")));

    let mut elsewhere = LifecycleMachine::new(
        Side::DedicatedServer,
        vec![entry("commands", &[Side::Client])],
    )
    .expect("a legal plan");
    let refused = route(
        &mut ravel_loader::LoadedMods::<&mut dyn HostMod>::new(&mut elsewhere, &mut mods)
            .expect("all active mods have hosts"),
        &CommandInvocation::new(owned.clone(), Side::DedicatedServer, vec!["set".to_owned()]),
    )
    .expect_err("nothing claimed it on this side");
    assert_eq!(refused, DispatchRefused::Unclaimed { key: owned });
}

#[test]
fn two_mods_cannot_claim_the_same_command_root() {
    struct Squatter {
        id: ModId,
        key: RegistryKey,
    }

    impl HostMod for Squatter {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), ravel_host_api::ModError> {
            Ok(())
        }

        fn register(
            &mut self,
            registrar: &mut dyn ravel_host_api::Registrar,
        ) -> Result<(), ravel_host_api::ModError> {
            registrar
                .claim(self.key.clone())
                .map_err(|error| ravel_host_api::ModError::refused(error.to_string()))
        }
    }

    let contested = CommandsMod::new().key("warp");
    let mut commands = CommandsMod::new();
    let mut squatter = Squatter {
        id: id("squatter"),
        key: contested.clone(),
    };

    let mut machine = LifecycleMachine::new(
        Side::Client,
        vec![
            entry("commands", &[Side::Client]),
            entry("squatter", &[Side::Client]),
        ],
    )
    .expect("a legal plan");
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
            .registration_window(&id("commands"))
            .expect("in the plan");
        commands.register(&mut window).expect("it claims first");
    }
    let mut window = machine
        .registration_window(&id("squatter"))
        .expect("in the plan");
    let error = squatter
        .register(&mut window)
        .expect_err("the root is taken");

    let text = error.to_string();
    assert!(text.contains("commands"), "{text}");
    assert!(text.contains(ContentKind::Command.as_str()), "{text}");
    assert_eq!(machine.owner_of(&contested), Some(&id("commands")));
}

#[test]
fn a_command_its_owner_refuses_reports_the_refusal_and_leaves_the_set_running() {
    let mut commands = CommandsMod::new();
    let mut mods: Vec<&mut dyn HostMod> = vec![&mut commands];
    let mut machine = running(
        vec![entry("commands", &[Side::Client])],
        Side::Client,
        &mut mods,
    );

    let delivery = route(
        &mut ravel_loader::LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
            .expect("all active mods have hosts"),
        &CommandInvocation::new(CommandsMod::new().key("warp"), Side::Client, Vec::new()),
    )
    .expect("routing found the owner");
    assert_eq!(delivery.received(), []);
    match delivery.failure() {
        Some(HostError::Refused { mod_id, detail, .. }) => {
            assert_eq!(mod_id, &id("commands"));
            assert!(detail.contains("needs a subcommand"), "{detail}");
        }
        other => panic!("expected the owner to refuse, got {other:?}"),
    }

    let accepted = route(
        &mut ravel_loader::LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
            .expect("all active mods have hosts"),
        &CommandInvocation::new(
            CommandsMod::new().key("warp"),
            Side::Client,
            vec!["set".to_owned()],
        ),
    )
    .expect("routing found the owner");
    assert!(accepted.is_complete());

    drop(mods);
    assert_eq!(commands.invocations().len(), 1);
}

#[test]
fn a_mod_that_fails_on_an_event_ends_the_broadcast_and_the_ones_after_it_never_see_it() {
    let mut first = ResourcesMod::new();
    let mut second = EventsMod::refusing(GameEvent::ResourcesReloaded);
    let mut third = CommandsMod::new();
    let mut mods: Vec<&mut dyn HostMod> = vec![&mut first, &mut second, &mut third];
    let mut machine = running(
        vec![
            entry("resources", &[Side::Client]),
            entry("events", &[Side::Client]),
            entry("commands", &[Side::Client]),
        ],
        Side::Client,
        &mut mods,
    );

    let delivery = broadcast(
        &mut ravel_loader::LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
            .expect("all active mods have hosts"),
        GameEvent::ResourcesReloaded,
    )
    .expect("the set is running");
    assert_eq!(delivery.received(), [id("resources")]);
    assert!(!delivery.is_complete());
    match delivery.failure() {
        Some(HostError::Refused { mod_id, .. }) => assert_eq!(mod_id, &id("events")),
        other => panic!("expected the second mod to refuse, got {other:?}"),
    }

    drop(mods);
    assert_eq!(first.reloads(), 1);
    assert_eq!(second.observed(), []);
    assert_eq!(third.invocations(), []);
}

#[test]
fn nothing_is_delivered_before_the_set_reaches_running() {
    let mut commands = CommandsMod::new();
    let mut events = EventsMod::new();
    let mut mods: Vec<&mut dyn HostMod> = vec![&mut commands, &mut events];

    let mut machine = LifecycleMachine::new(
        Side::Client,
        vec![
            entry("commands", &[Side::Client]),
            entry("events", &[Side::Client]),
        ],
    )
    .expect("a legal plan");
    for state in [
        LifecycleState::Validated,
        LifecycleState::Resolved,
        LifecycleState::Loaded,
        LifecycleState::Constructed,
    ] {
        machine.advance_all(state).expect("a legal step");
    }

    let refused = broadcast(
        &mut ravel_loader::LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
            .expect("all active mods have hosts"),
        GameEvent::ClientTick,
    )
    .expect_err("the set has not started");
    assert_eq!(
        refused,
        DispatchRefused::NotReacting {
            mod_id: id("commands"),
            state: LifecycleState::Constructed,
        }
    );

    drop(mods);
    assert_eq!(events.observed(), []);
}

#[test]
fn a_command_nobody_claimed_is_refused_by_name() {
    let mut commands = CommandsMod::new();
    let mut mods: Vec<&mut dyn HostMod> = vec![&mut commands];
    let mut machine = running(
        vec![entry("commands", &[Side::Client])],
        Side::Client,
        &mut mods,
    );

    let orphan = RegistryKey::new(
        ContentKind::Command,
        "commands:never-claimed"
            .parse()
            .expect("a legal content id"),
    );
    let refused = route(
        &mut ravel_loader::LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
            .expect("all active mods have hosts"),
        &CommandInvocation::new(orphan.clone(), Side::Client, vec!["x".to_owned()]),
    )
    .expect_err("nothing claimed it");

    assert_eq!(refused, DispatchRefused::Unclaimed { key: orphan });
    drop(mods);
    assert_eq!(commands.invocations(), []);
}

#[test]
fn a_set_missing_a_mod_the_plan_names_delivers_to_nobody() {
    let mut commands = CommandsMod::new();
    let mut events = EventsMod::new();
    let mut both: Vec<&mut dyn HostMod> = vec![&mut commands, &mut events];
    let mut machine = running(
        vec![
            entry("commands", &[Side::Client]),
            entry("events", &[Side::Client]),
        ],
        Side::Client,
        &mut both,
    );
    drop(both);

    let mut short: Vec<&mut dyn HostMod> = vec![&mut commands];
    let refused = ravel_loader::LoadedMods::new(&mut machine, &mut short)
        .expect_err("the plan names a mod that was not supplied");
    assert_eq!(refused.mod_id(), &id("events"));

    drop(short);
    assert_eq!(events.observed(), []);
}

#[test]
fn an_empty_resource_set_supplies_nothing_and_a_declared_one_supplies_its_own() {
    let empty = ResourceSet::default();
    let declared = ResourcesMod::new().declared();
    assert_eq!(empty.supplier(&declared[0]), None);

    let owner = id("resources");
    let set = ResourceSet::layered([(&owner, declared.iter())]);
    for path in &declared {
        assert_eq!(set.supplier(path), Some(&owner));
    }
    assert!(!set.has_overrides());
}

// Caller-chosen identities let dispatch counts cover arbitrary pack sizes.
struct Ticking {
    id: ModId,
    calls: usize,
}

impl HostMod for Ticking {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), ravel_host_api::ModError> {
        Ok(())
    }

    fn observe(&mut self, _event: GameEvent) -> Result<(), ravel_host_api::ModError> {
        self.calls += 1;
        Ok(())
    }
}

fn calls_for(pack_size: usize, ticks: usize) -> usize {
    let names: Vec<String> = (0..pack_size)
        .map(|index| format!("mod_{index:03}"))
        .collect();
    let mut hosts: Vec<Ticking> = names
        .iter()
        .map(|name| Ticking {
            id: id(name),
            calls: 0,
        })
        .collect();
    let mut mods: Vec<&mut dyn HostMod> = hosts
        .iter_mut()
        .map(|host| -> &mut dyn HostMod { host })
        .collect();

    let plan = names
        .iter()
        .map(|name| entry(name, &[Side::Client, Side::DedicatedServer]))
        .collect();
    let mut machine = running(plan, Side::DedicatedServer, &mut mods);

    for _ in 0..ticks {
        let delivery = broadcast(
            &mut ravel_loader::LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active mods have hosts"),
            GameEvent::ServerTick,
        )
        .expect("the set is running");
        assert_eq!(delivery.received().len(), pack_size);
    }

    drop(mods);
    hosts.iter().map(|host| host.calls).sum()
}

#[test]
fn one_tick_costs_exactly_one_mod_call_per_mod_in_the_audience() {
    for pack_size in [0, 1, 8, 64] {
        assert_eq!(calls_for(pack_size, 1), pack_size);
    }

    assert_eq!(calls_for(8, 20), 160);
}

#[test]
fn a_mod_outside_the_audience_costs_nothing_per_tick() {
    let mut server_side = Ticking {
        id: id("server_mod"),
        calls: 0,
    };
    let mut client_only = Ticking {
        id: id("client_mod"),
        calls: 0,
    };
    let mods: Vec<&mut dyn HostMod> = vec![&mut server_side, &mut client_only];

    let machine = LifecycleMachine::new(
        Side::DedicatedServer,
        vec![
            entry("server_mod", &[Side::Client, Side::DedicatedServer]),
            entry("client_mod", &[Side::Client]),
        ],
    )
    .expect("a legal plan");
    assert_eq!(machine.audience(GameEvent::ServerTick), [id("server_mod")]);

    drop(mods);
    assert_eq!(client_only.calls, 0);
}
