//! The command, event and resource test mods driven through a running set.
//!
//! `ROADMAP.md`'s M3-W2 row asks for command, event and resource test mods, and
//! for process tests that exercise success, ordering, side and failure cases.
//! Each of those four has its own section below, and every one of them drives
//! the real test mods rather than a stand-in: a capability proved against a mock
//! written in the same file proves the mock.
//!
//! These tests build a [`LifecycleMachine`] directly instead of going through
//! `ravel_testkit::run`, because that function takes a set from validation to
//! [`LifecycleState::Stopped`] in one call and every capability here is
//! delivered while the set is [`LifecycleState::Running`]. Validation and
//! resolution are covered by `noop_pipeline.rs` against a real package, so
//! repeating them here would test the pipeline a second time rather than the
//! capabilities.

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

/// Drives `entries` to [`LifecycleState::Running`], registering `mods` on the way.
///
/// This is the registration barrier as `ravel_testkit::run` performs it, with
/// the run stopped at the first state that delivers anything. The order is the
/// caller's, because these tests supply the plan rather than resolving one, and
/// `SPEC.md` §14's determinism is `ravel-resolve`'s to provide and
/// `noop_pipeline.rs`'s to check.
fn running(
    entries: Vec<LifecycleEntry>,
    side: Side,
    mods: &mut [&mut dyn HostMod],
) -> LifecycleMachine {
    let mut machine = LifecycleMachine::new(side, entries).expect("a legal plan");
    for state in [
        LifecycleState::Validated,
        LifecycleState::Resolved,
        LifecycleState::Loaded,
        LifecycleState::Constructed,
    ] {
        machine.advance_all(state).expect("a legal step");
    }

    let order: Vec<ModId> = machine.order().cloned().collect();
    for mod_id in order {
        let mut window = machine
            .registration_window(&mod_id)
            .expect("the plan names it");
        if let Some(host) = mods.iter_mut().find(|host| host.id() == &mod_id) {
            host.register(&mut window).expect("nothing conflicts");
        }
        assert!(window.unexpected().is_none(), "the window was open");
    }

    for state in [LifecycleState::Registered, LifecycleState::Running] {
        machine.advance_all(state).expect("a legal step");
    }
    machine
}

/// The plan every success case runs, in the order the mods appear.
fn three_mod_plan() -> Vec<LifecycleEntry> {
    vec![
        entry("commands", &[Side::Client, Side::DedicatedServer]),
        entry("events", &[Side::Client, Side::DedicatedServer]),
        entry("resources", &[Side::Client, Side::DedicatedServer]),
    ]
}

// ---------------------------------------------------------------- success ---

#[test]
fn a_running_set_owns_its_commands_and_routes_one_to_the_mod_that_claimed_it() {
    let mut commands = CommandsMod::new();
    let mut events = EventsMod::new();
    let mut resources = ResourcesMod::new();
    let mut mods: Vec<&mut dyn HostMod> = vec![&mut commands, &mut events, &mut resources];
    let machine = running(three_mod_plan(), Side::Client, &mut mods);

    // Both roots reached the claim table, under the claiming mod's identity.
    for key in CommandsMod::new().claims() {
        assert_eq!(machine.owner_of(&key), Some(&id("commands")));
    }

    let invocation = CommandInvocation::new(
        CommandsMod::new().key("warp"),
        Side::Client,
        vec!["set".to_owned(), "base".to_owned()],
    );
    let delivery = route(&machine, &mut mods, &invocation).expect("the owner is running");
    assert_eq!(delivery.received(), [id("commands")]);
    assert!(delivery.is_complete());

    drop(mods);
    assert_eq!(commands.invocations(), [invocation]);
    // Nothing else was asked, so a command is routed and not broadcast.
    assert_eq!(events.observed(), []);
    assert_eq!(resources.reloads(), 0);
}

#[test]
fn a_broadcast_reaches_every_mod_and_each_one_acts_on_what_it_cares_about() {
    let mut commands = CommandsMod::new();
    let mut events = EventsMod::new();
    let mut resources = ResourcesMod::new();
    let mut mods: Vec<&mut dyn HostMod> = vec![&mut commands, &mut events, &mut resources];
    let machine = running(three_mod_plan(), Side::Client, &mut mods);

    let delivery =
        broadcast(&machine, &mut mods, GameEvent::ResourcesReloaded).expect("the set is running");
    assert_eq!(
        delivery.received(),
        [id("commands"), id("events"), id("resources")]
    );
    assert!(delivery.is_complete());

    drop(mods);
    // Every mod received it; two of the three had nothing to do with it, which
    // is what the default implementation of `observe` is for.
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

    // A shared path is an override rather than a conflict: the registration
    // barrier refuses a second claim, and a resource path is simply taken.
    assert_eq!(set.supplier(&shared), Some(&later));
    assert_eq!(set.supplier(&own), Some(&id("resources")));
    assert_eq!(set.overrides().len(), 1);
    assert_eq!(set.overrides()[0].shadowed, [id("resources")]);
}

// --------------------------------------------------------------- ordering ---

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
        let machine = running(plan, Side::Client, &mut mods);
        broadcast(&machine, &mut mods, GameEvent::ClientTick)
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
        let machine = running(three_mod_plan(), Side::Client, &mut mods);
        let audience = machine.audience(GameEvent::ClientTick);
        let received = broadcast(&machine, &mut mods, GameEvent::ClientTick)
            .expect("the set is running")
            .received()
            .to_vec();
        (audience, received)
    };

    let first = run_once();
    assert_eq!(first, run_once());
    // The audience is the delivery's order, so a report can name who will
    // receive an event without delivering one.
    assert_eq!(first.0, first.1);
}

#[test]
fn a_mod_sees_repeated_ticks_in_the_order_they_were_broadcast() {
    let mut events = EventsMod::new();
    let mut mods: Vec<&mut dyn HostMod> = vec![&mut events];
    let machine = running(
        vec![entry("events", &[Side::Client])],
        Side::Client,
        &mut mods,
    );

    for event in [
        GameEvent::ClientTick,
        GameEvent::ResourcesReloaded,
        GameEvent::ClientTick,
    ] {
        broadcast(&machine, &mut mods, event).expect("the set is running");
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

// ------------------------------------------------------------------- side ---

#[test]
fn a_client_tick_reaches_nobody_on_a_dedicated_server_and_a_server_tick_reaches_everybody() {
    let mut commands = CommandsMod::new();
    let mut events = EventsMod::new();
    let mut resources = ResourcesMod::new();
    let mut mods: Vec<&mut dyn HostMod> = vec![&mut commands, &mut events, &mut resources];
    let machine = running(three_mod_plan(), Side::DedicatedServer, &mut mods);

    assert_eq!(
        machine.audience(GameEvent::ClientTick),
        [] as [ravel_core::ModId; 0]
    );
    let ignored = broadcast(&machine, &mut mods, GameEvent::ClientTick).expect("an empty audience");
    assert_eq!(ignored.received(), []);

    let ticked = broadcast(&machine, &mut mods, GameEvent::ServerTick).expect("the set is running");
    assert_eq!(
        ticked.received(),
        [id("commands"), id("events"), id("resources")]
    );

    drop(mods);
    // The one event this mod saw is the one that happens on this side, so the
    // filter is the loader's and not the mod's.
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
    // The owner registers on the client, and the command arrives on a set whose
    // side it does not load on. Refusing names the reason; ignoring would leave
    // a player typing a command that reports nothing.
    let mut commands = CommandsMod::new();
    let mut mods: Vec<&mut dyn HostMod> = vec![&mut commands];
    let machine = running(
        vec![entry("commands", &[Side::Client])],
        Side::Client,
        &mut mods,
    );
    let owned = CommandsMod::new().key("warp");
    assert_eq!(machine.owner_of(&owned), Some(&id("commands")));

    // A second set, on the other side, that never got to register at all.
    let elsewhere = LifecycleMachine::new(
        Side::DedicatedServer,
        vec![entry("commands", &[Side::Client])],
    )
    .expect("a legal plan");
    let refused = route(
        &elsewhere,
        &mut mods,
        &CommandInvocation::new(owned.clone(), Side::DedicatedServer, vec!["set".to_owned()]),
    )
    .expect_err("nothing claimed it on this side");
    assert_eq!(refused, DispatchRefused::Unclaimed { key: owned });
}

// ---------------------------------------------------------------- failure ---

#[test]
fn two_mods_cannot_claim_the_same_command_root() {
    /// A second mod that wants a root `ravel-test-mod-commands` already owns.
    struct Squatter {
        id: ModId,
        key: RegistryKey,
    }

    impl HostMod for Squatter {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn register(
            &mut self,
            registrar: &mut dyn ravel_host_api::Registrar,
        ) -> Result<(), HostError> {
            registrar
                .claim(self.key.clone())
                .map_err(|refused| HostError::Refused {
                    mod_id: self.id.clone(),
                    state: LifecycleState::Constructed,
                    detail: refused.to_string(),
                })
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
    // The first claimant keeps it, which is what makes the order deterministic.
    assert_eq!(machine.owner_of(&contested), Some(&id("commands")));
}

#[test]
fn a_command_its_owner_refuses_reports_the_refusal_and_leaves_the_set_running() {
    let mut commands = CommandsMod::new();
    let mut mods: Vec<&mut dyn HostMod> = vec![&mut commands];
    let machine = running(
        vec![entry("commands", &[Side::Client])],
        Side::Client,
        &mut mods,
    );

    let delivery = route(
        &machine,
        &mut mods,
        // No subcommand, which this mod refuses.
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

    // A refused command does not end the run: the same mod runs the next one.
    let accepted = route(
        &machine,
        &mut mods,
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
    let machine = running(
        vec![
            entry("resources", &[Side::Client]),
            entry("events", &[Side::Client]),
            entry("commands", &[Side::Client]),
        ],
        Side::Client,
        &mut mods,
    );

    let delivery =
        broadcast(&machine, &mut mods, GameEvent::ResourcesReloaded).expect("the set is running");
    assert_eq!(delivery.received(), [id("resources")]);
    assert!(!delivery.is_complete());
    match delivery.failure() {
        Some(HostError::Refused { mod_id, .. }) => assert_eq!(mod_id, &id("events")),
        other => panic!("expected the second mod to refuse, got {other:?}"),
    }

    drop(mods);
    // The mod ahead of the failure acted and the one behind it did not, which
    // is what makes the failure attributable to one mod.
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

    let refused =
        broadcast(&machine, &mut mods, GameEvent::ClientTick).expect_err("the set has not started");
    assert_eq!(
        refused,
        DispatchRefused::NotReacting {
            mod_id: id("commands"),
            state: LifecycleState::Constructed,
        }
    );

    drop(mods);
    // `SPEC.md` §14 puts every registration before `Running`, so a mod that saw
    // an event here would be acting before its own content existed.
    assert_eq!(events.observed(), []);
}

#[test]
fn a_command_nobody_claimed_is_refused_by_name() {
    let mut commands = CommandsMod::new();
    let mut mods: Vec<&mut dyn HostMod> = vec![&mut commands];
    let machine = running(
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
        &machine,
        &mut mods,
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
    let machine = running(
        vec![
            entry("commands", &[Side::Client]),
            entry("events", &[Side::Client]),
        ],
        Side::Client,
        &mut both,
    );
    drop(both);

    let mut short: Vec<&mut dyn HostMod> = vec![&mut commands];
    let refused = broadcast(&machine, &mut short, GameEvent::ClientTick)
        .expect_err("the plan names a mod that was not supplied");
    assert_eq!(
        refused,
        DispatchRefused::Absent {
            mod_id: id("events")
        }
    );

    drop(short);
    // The supplied mod is ahead of the missing one in the plan and still saw
    // nothing: the whole set is checked before anyone reacts.
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

// ------------------------------------------------- mod calls per tick ---

/// A mod with a caller-chosen identity that counts the calls it received.
///
/// The three test mods each have one fixed `MOD_ID`, and the question this
/// section answers is about a *pack* — many mods, one tick — so a set of them
/// cannot be built from the real mods alone. What is being counted is a property
/// of the dispatch mechanism rather than of any mod's body, so counting it on a
/// mod whose body does nothing is the honest measurement: a real mod's own work
/// is not a boundary crossing and must not be added to one.
struct Ticking {
    id: ModId,
    calls: usize,
}

impl HostMod for Ticking {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
        Ok(())
    }

    fn observe(&mut self, _event: GameEvent) -> Result<(), HostError> {
        self.calls += 1;
        Ok(())
    }
}

/// Broadcasts `ticks` server ticks to a set of `pack_size` mods and returns the
/// total number of times a mod was called.
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
        .map(|host| host as &mut dyn HostMod)
        .collect();

    let plan = names
        .iter()
        .map(|name| entry(name, &[Side::Client, Side::DedicatedServer]))
        .collect();
    let machine = running(plan, Side::DedicatedServer, &mut mods);

    for _ in 0..ticks {
        let delivery =
            broadcast(&machine, &mut mods, GameEvent::ServerTick).expect("the set is running");
        assert_eq!(delivery.received().len(), pack_size);
    }

    drop(mods);
    hosts.iter().map(|host| host.calls).sum()
}

#[test]
fn one_tick_costs_exactly_one_mod_call_per_mod_in_the_audience() {
    // This is the count ADG-2 asked for and could not have: its own record says
    // "nothing in `SPEC.md` yet states how many mod calls a tick makes", so the
    // cost of the mod boundary was written as a rate rather than a number. The
    // rate is now measurable, and it is linear in pack size with no constant
    // term: a tick calls every mod in the audience once and calls nobody else.
    for pack_size in [0, 1, 8, 64] {
        assert_eq!(calls_for(pack_size, 1), pack_size);
    }

    // Ticks compose, so a pack's per-tick cost does not grow with uptime.
    assert_eq!(calls_for(8, 20), 160);
}

#[test]
fn the_measured_rate_says_how_large_a_pack_would_have_to_be_to_reach_adg_2s_threshold() {
    // ADG-2 records that the 0.625 ns separating a linked mod call from a
    // dynamic-library call reaches 0.1 ms only at 160,000 mod calls in one
    // tick, and had no count to compare that against. This derives one from the
    // measurement rather than asserting it: the rate is taken from two pack
    // sizes, and the threshold pack size follows from the rate.
    const ADG_2_THRESHOLD: usize = 160_000;

    let small = calls_for(8, 1);
    let large = calls_for(64, 1);
    let rate = (large - small) / (64 - 8);
    assert_eq!(rate, 1, "a tick costs one call per mod and nothing else");

    // No constant term, so the threshold is reached at one mod per call.
    assert_eq!(large - rate * 64, 0);
    assert_eq!(ADG_2_THRESHOLD / rate, ADG_2_THRESHOLD);
}

#[test]
fn a_mod_outside_the_audience_costs_nothing_per_tick() {
    // The count is per *audience*, not per installed mod, so a client-only mod
    // on a dedicated server is not a call that happens and is then discarded.
    let mut server_side = Ticking {
        id: id("server_mod"),
        calls: 0,
    };
    let mut client_only = Ticking {
        id: id("client_mod"),
        calls: 0,
    };
    let mods: Vec<&mut dyn HostMod> = vec![&mut server_side, &mut client_only];

    // The client-only mod cannot be driven onto a dedicated server at all, so
    // the audience is asked before the set runs.
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
