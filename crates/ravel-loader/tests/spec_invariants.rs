//! One test per required invariant of `SPEC.md` §14, driven through the public
//! lifecycle API so each test fails when the invariant stops holding.

use std::collections::BTreeSet;
use std::num::NonZeroU32;

use ravel_core::{
    DiagnosticMessage, FailureAttribution, FailureEnvironment, FailureKind, LifecycleState, ModId,
    Redaction, Side, SideSet,
};
use ravel_loader::{
    LifecycleEntry, LifecycleError, LifecycleEvent, LifecycleMachine, Requirement,
    executes_mod_code,
};

/// The states a set walks from `Discovered` to `Running`.
const STARTUP: [LifecycleState; 6] = [
    LifecycleState::Validated,
    LifecycleState::Resolved,
    LifecycleState::Loaded,
    LifecycleState::Constructed,
    LifecycleState::Registered,
    LifecycleState::Running,
];

fn id(name: &str) -> ModId {
    name.parse().expect("valid mod id")
}

fn entry(name: &str, sides: &[Side], requirement: Requirement) -> LifecycleEntry {
    LifecycleEntry {
        mod_id: id(name),
        sides: SideSet::new(sides.iter().copied()).expect("at least one side"),
        requirement,
        saved_data_versions: BTreeSet::new(),
    }
}

fn attribution(name: &str, state: LifecycleState, kind: FailureKind) -> FailureAttribution {
    FailureAttribution {
        mod_id: id(name),
        version: Some("1.0.0".parse().expect("valid version")),
        dependency_chain: Vec::new(),
        state,
        kind,
        surface: None,
        environment: FailureEnvironment {
            ravel: "0.1.0".parse().expect("valid version"),
            minecraft: "26.0.0".to_owned(),
            platform: "linux".to_owned(),
            architecture: "x86_64".to_owned(),
        },
        message: DiagnosticMessage {
            text: "the mod could not continue".to_owned(),
            redaction: Redaction::Applied,
        },
    }
}

fn started(machine: &mut LifecycleMachine) {
    for state in STARTUP {
        machine
            .advance_all(state)
            .expect("the whole set advances one step");
    }
}

/// The first invariant: no mod code runs before validation and dependency
/// resolution.
#[test]
fn no_mod_code_runs_before_validation_and_dependency_resolution() {
    let mut machine = LifecycleMachine::new(
        Side::Client,
        vec![
            entry("first_mod", &[Side::Client], Requirement::Required),
            entry("second_mod", &[Side::Client], Requirement::Required),
        ],
    )
    .expect("two mods are a valid plan");

    // Every state that runs code is unreachable until both bookkeeping states
    // have been passed, and skipping either is refused rather than tolerated.
    for reached in [LifecycleState::Discovered, LifecycleState::Validated] {
        let code_states = LifecycleState::ALL
            .into_iter()
            .filter(|state| executes_mod_code(*state));
        for target in code_states {
            assert_eq!(
                machine.advance(&id("first_mod"), target),
                Err(LifecycleError::IllegalTransition {
                    mod_id: id("first_mod"),
                    from: reached,
                    to: target,
                }),
                "{reached:?} must not reach {target:?}"
            );
        }
        if reached == LifecycleState::Discovered {
            machine
                .advance_all(LifecycleState::Validated)
                .expect("validation is the only next step");
        }
    }

    for state in STARTUP.into_iter().skip(1) {
        machine.advance_all(state).expect("the whole set advances");
    }
    for mod_id in ["first_mod", "second_mod"] {
        let history: Vec<LifecycleState> = machine
            .transcript()
            .iter()
            .filter(|event| event.mod_id == id(mod_id))
            .map(|event| event.state)
            .collect();
        let first_code = history
            .iter()
            .position(|state| executes_mod_code(*state))
            .expect("the mod reached a state that runs code");
        let validated = history
            .iter()
            .position(|state| *state == LifecycleState::Validated)
            .expect("the mod was validated");
        let resolved = history
            .iter()
            .position(|state| *state == LifecycleState::Resolved)
            .expect("the mod was resolved");
        assert!(
            validated < first_code,
            "{mod_id} ran code before validation"
        );
        assert!(resolved < first_code, "{mod_id} ran code before resolution");
    }
}

/// The second invariant: registry identifiers become immutable after the
/// registration barrier.
///
/// `SPEC.md` §14 says *the* registration barrier, one barrier for the load, so
/// the plan here holds more than one mod: a per-mod checkpoint would let a mod
/// that has already registered start running while another is still adding
/// identifiers, and this fails when it can.
#[test]
fn registry_identifiers_become_immutable_after_the_registration_barrier() {
    let mut machine = LifecycleMachine::new(
        Side::Client,
        vec![
            entry("first_mod", &[Side::Client], Requirement::Required),
            entry("second_mod", &[Side::Client], Requirement::Required),
        ],
    )
    .expect("two mods are a valid plan");
    let names = ["first_mod", "second_mod"];

    for state in [
        LifecycleState::Validated,
        LifecycleState::Resolved,
        LifecycleState::Loaded,
        LifecycleState::Constructed,
    ] {
        machine.advance_all(state).expect("the set advances");
    }
    assert!(!machine.registration_barrier_closed());
    machine
        .claim_registry_identifier(&id("first_mod"), "first_mod:block")
        .expect("claims are accepted before the barrier");

    // One mod committing its registrations does not close the barrier, and it
    // may not run ahead of a mod that is still registering.
    machine
        .advance(&id("first_mod"), LifecycleState::Registered)
        .expect("the first mod commits");
    assert!(!machine.registration_barrier_closed());
    assert_eq!(
        machine.advance(&id("first_mod"), LifecycleState::Running),
        Err(LifecycleError::RegistrationBarrierOpen {
            mod_id: id("first_mod"),
            blocking: id("second_mod"),
        }),
        "no mod starts while another is still registering"
    );
    machine
        .claim_registry_identifier(&id("second_mod"), "second_mod:block")
        .expect("the barrier is still open for a mod that has not registered");

    // Closing it makes every identifier immutable, whichever mod asks.
    machine
        .advance(&id("second_mod"), LifecycleState::Registered)
        .expect("the second mod commits");
    assert!(machine.registration_barrier_closed());
    for name in names {
        assert_eq!(
            machine.claim_registry_identifier(&id(name), "late:block"),
            Err(LifecycleError::RegistrationBarrierClosed {
                mod_id: id(name),
                identifier: "late:block".to_owned(),
            }),
            "{name} claimed an identifier after the barrier closed"
        );
    }

    machine
        .advance_all(LifecycleState::Running)
        .expect("a registered set starts");
    for name in names {
        assert_eq!(
            machine.claim_registry_identifier(&id(name), "late:block"),
            Err(LifecycleError::RegistrationBarrierClosed {
                mod_id: id(name),
                identifier: "late:block".to_owned(),
            }),
            "{name} claimed an identifier while the set was running"
        );
    }

    // What was committed before the barrier keeps its owner, and nothing was
    // added after it.
    assert_eq!(
        machine.registry_owner("first_mod:block"),
        Some(&id("first_mod"))
    );
    assert_eq!(
        machine.registry_owner("second_mod:block"),
        Some(&id("second_mod"))
    );
    assert_eq!(machine.registry_owner("late:block"), None);
}

/// The third invariant: required dependency failure prevents world access.
#[test]
fn required_dependency_failure_prevents_world_access() {
    let mut machine = LifecycleMachine::new(
        Side::Client,
        vec![
            entry("base_mod", &[Side::Client], Requirement::Required),
            entry("extra_mod", &[Side::Client], Requirement::Optional),
        ],
    )
    .expect("two mods are a valid plan");
    started(&mut machine);
    machine.world_access().expect("a running set opens a world");

    // An optional mod that fails does not withhold the world.
    let mut optional = machine.clone();
    optional
        .fail(
            &id("extra_mod"),
            attribution(
                "extra_mod",
                LifecycleState::Running,
                FailureKind::IncompatibleVersion,
            ),
        )
        .expect("failure is legal from running");
    optional
        .world_access()
        .expect("an optional failure does not fail the set closed");

    // A required mod that fails does, naming itself.
    machine
        .fail(
            &id("base_mod"),
            attribution(
                "base_mod",
                LifecycleState::Running,
                FailureKind::MissingRequiredDependency,
            ),
        )
        .expect("failure is legal from running");
    assert_eq!(
        machine.world_access(),
        Err(LifecycleError::WorldAccessBlocked {
            mod_id: id("base_mod"),
            state: LifecycleState::Failed,
        })
    );

    // The same holds before the set ever finished starting.
    let mut early = LifecycleMachine::new(
        Side::Client,
        vec![entry("base_mod", &[Side::Client], Requirement::Required)],
    )
    .expect("one mod is a valid plan");
    early
        .advance_all(LifecycleState::Validated)
        .expect("the set advances");
    assert_eq!(
        early.world_access(),
        Err(LifecycleError::WorldAccessBlocked {
            mod_id: id("base_mod"),
            state: LifecycleState::Validated,
        })
    );
}

/// The fourth invariant: client-only code never loads on server sides.
#[test]
fn client_only_code_never_loads_on_server_sides() {
    for side in Side::ALL.into_iter().filter(|side| side.is_server()) {
        let mut machine = LifecycleMachine::new(
            side,
            vec![
                entry("client_mod", &[Side::Client], Requirement::Optional),
                entry("both_mod", &[Side::Client, side], Requirement::Required),
            ],
        )
        .expect("two mods are a valid plan");

        machine
            .advance_all(LifecycleState::Validated)
            .expect("metadata bookkeeping is side-neutral");
        machine
            .advance_all(LifecycleState::Resolved)
            .expect("graph bookkeeping is side-neutral");

        assert_eq!(
            machine.advance(&id("client_mod"), LifecycleState::Loaded),
            Err(LifecycleError::SideMismatch {
                mod_id: id("client_mod"),
                from: LifecycleState::Resolved,
                side,
            }),
            "client-only code must not load on {side:?}"
        );

        // Driving the set leaves the client-only mod parked rather than loaded,
        // and the sided mod still reaches a world.
        for state in [
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
            LifecycleState::Running,
        ] {
            machine.advance_all(state).expect("the sided mod advances");
        }
        assert_eq!(
            machine.state(&id("client_mod")).expect("known mod"),
            LifecycleState::Resolved
        );
        assert_eq!(
            machine.state(&id("both_mod")).expect("known mod"),
            LifecycleState::Running
        );
        assert!(
            !machine
                .transcript()
                .iter()
                .any(|event| event.mod_id == id("client_mod") && executes_mod_code(event.state)),
            "client-only code entered a code state on {side:?}"
        );
        machine
            .world_access()
            .expect("a mod that never loads here does not withhold the world");
    }
}

/// The fifth invariant: persistent data is versioned and owned by a stable
/// canonical mod ID.
#[test]
fn persistent_data_is_versioned_and_owned_by_a_stable_canonical_mod_id() {
    // Versioning is carried by the type the manifest uses, so an unversioned
    // declaration cannot be written rather than being rejected at run time.
    let declared: BTreeSet<NonZeroU32> = [1, 2, 5]
        .into_iter()
        .map(|version| NonZeroU32::new(version).expect("a declared version is never zero"))
        .collect();
    assert!(
        NonZeroU32::new(0).is_none(),
        "zero is not a saved-data version"
    );

    let mut owner = entry("example_mod", &[Side::Client], Requirement::Required);
    owner.saved_data_versions = declared.clone();
    let mut machine = LifecycleMachine::new(
        Side::Client,
        vec![
            owner,
            entry("stateless_mod", &[Side::Client], Requirement::Required),
        ],
    )
    .expect("two mods are a valid plan");
    let mod_id = id("example_mod");

    assert_eq!(
        machine.saved_data_versions(&mod_id).expect("known mod"),
        &declared
    );
    assert!(
        machine
            .saved_data_versions(&id("stateless_mod"))
            .expect("known mod")
            .is_empty()
    );

    // Ownership is keyed by the canonical identity and no lifecycle step moves
    // it, including a failure.
    started(&mut machine);
    machine
        .fail(
            &mod_id,
            attribution(
                "example_mod",
                LifecycleState::Running,
                FailureKind::DuplicateIdentity,
            ),
        )
        .expect("failure is legal from running");
    assert_eq!(
        machine.saved_data_versions(&mod_id).expect("known mod"),
        &declared
    );
    assert_eq!(
        machine.saved_data_versions(&id("other_mod")),
        Err(LifecycleError::UnknownMod {
            mod_id: id("other_mod")
        })
    );

    // A second claim on the canonical identity that owns the data is refused,
    // because saved data with two owners has none.
    assert_eq!(
        LifecycleMachine::new(
            Side::Client,
            vec![
                entry("example_mod", &[Side::Client], Requirement::Required),
                entry("example_mod", &[Side::Client], Requirement::Required),
            ]
        )
        .map(|_| ()),
        Err(LifecycleError::DuplicateIdentity { mod_id })
    );
}

/// The sixth invariant: failure after world mutation cannot silently continue
/// or save partial state.
#[test]
fn failure_after_world_mutation_cannot_silently_continue_or_save_partial_state() {
    let mut machine = LifecycleMachine::new(
        Side::IntegratedServer,
        vec![
            entry(
                "first_mod",
                &[Side::IntegratedServer],
                Requirement::Required,
            ),
            entry(
                "second_mod",
                &[Side::IntegratedServer],
                Requirement::Required,
            ),
        ],
    )
    .expect("two mods are a valid plan");
    started(&mut machine);
    machine.request_save().expect("a healthy set may save");
    machine
        .record_world_mutation()
        .expect("a running set may mutate the world");

    machine
        .fail(
            &id("first_mod"),
            attribution(
                "first_mod",
                LifecycleState::Running,
                FailureKind::ConflictingRegistryClaim,
            ),
        )
        .expect("failure is legal from running");

    // Nothing about the run continues: no save, no world, no forward progress.
    assert_eq!(
        machine.request_save(),
        Err(LifecycleError::PartialStateRefused {
            mod_id: id("first_mod")
        })
    );
    assert_eq!(
        machine.world_access(),
        Err(LifecycleError::WorldAccessHalted {
            mod_id: id("first_mod")
        })
    );
    assert_eq!(
        machine.record_world_mutation(),
        Err(LifecycleError::WorldAccessHalted {
            mod_id: id("first_mod")
        })
    );

    // Quiescing is the only progression left, and it does not clear the halt.
    machine
        .advance_all(LifecycleState::Stopping)
        .expect("the survivors may quiesce");
    machine
        .advance_all(LifecycleState::Stopped)
        .expect("the survivors may finish stopping");
    assert_eq!(
        machine.state(&id("second_mod")).expect("known mod"),
        LifecycleState::Stopped
    );
    assert_eq!(
        machine.request_save(),
        Err(LifecycleError::PartialStateRefused {
            mod_id: id("first_mod")
        })
    );

    // A failure before any mutation leaves the set free to stop normally.
    let mut untouched = LifecycleMachine::new(
        Side::IntegratedServer,
        vec![
            entry(
                "first_mod",
                &[Side::IntegratedServer],
                Requirement::Required,
            ),
            entry(
                "second_mod",
                &[Side::IntegratedServer],
                Requirement::Required,
            ),
        ],
    )
    .expect("two mods are a valid plan");
    for state in [LifecycleState::Validated, LifecycleState::Resolved] {
        untouched.advance_all(state).expect("the set advances");
    }
    untouched
        .fail(
            &id("first_mod"),
            attribution(
                "first_mod",
                LifecycleState::Resolved,
                FailureKind::MissingRequiredDependency,
            ),
        )
        .expect("failure is legal from resolved");
    untouched
        .request_save()
        .expect("nothing was mutated, so nothing is partial");
    untouched
        .advance_all(LifecycleState::Loaded)
        .expect("the survivor may still start");
}

/// The seventh invariant: lifecycle order is deterministic for an identical
/// package set.
#[test]
fn lifecycle_order_is_deterministic_for_an_identical_package_set() {
    // Resolved order deliberately disagrees with alphabetical order, so an
    // implementation that iterated a sorted map would fail here.
    let plan = || {
        vec![
            entry("zulu_mod", &[Side::Client], Requirement::Required),
            entry("alpha_mod", &[Side::Client], Requirement::Required),
            entry("mike_mod", &[Side::Client], Requirement::Optional),
        ]
    };
    let names = ["zulu_mod", "alpha_mod", "mike_mod"];

    let transcript = |plan: Vec<LifecycleEntry>| -> Vec<LifecycleEvent> {
        let mut machine = LifecycleMachine::new(Side::Client, plan).expect("valid plan");
        assert_eq!(
            machine.order().cloned().collect::<Vec<_>>(),
            names.map(id).to_vec(),
            "the resolved order survives admission"
        );
        started(&mut machine);
        machine
            .fail(
                &id("mike_mod"),
                attribution(
                    "mike_mod",
                    LifecycleState::Running,
                    FailureKind::SideMismatch,
                ),
            )
            .expect("failure is legal from running");
        for state in [LifecycleState::Stopping, LifecycleState::Stopped] {
            machine.advance_all(state).expect("the set stops");
        }
        machine.transcript().to_vec()
    };

    let first = transcript(plan());
    let second = transcript(plan());
    assert_eq!(first, second, "an identical package set produces one order");

    // Each phase visits the resolved order exactly, mod by mod.
    let expected = names.map(id).to_vec();
    for (index, state) in STARTUP.into_iter().enumerate() {
        let observed: Vec<ModId> = first
            .iter()
            .skip(index * names.len())
            .take(names.len())
            .map(|event| {
                assert_eq!(event.state, state);
                event.mod_id.clone()
            })
            .collect();
        assert_eq!(
            observed, expected,
            "{state:?} did not follow the resolved order"
        );
    }
    assert_eq!(
        first.len(),
        STARTUP.len() * names.len() + 1 + 2 * (names.len() - 1),
        "startup, one failure, then two shutdown steps for the survivors"
    );
}
