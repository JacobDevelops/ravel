//! The save test mod driven over the world fixtures, opening what it may and
//! refusing what it may not.
//!
//! `ROADMAP.md`'s M3-W4 row asks that create/save/exit/reload preserve state and
//! that incompatible or missing required data fail closed. Each world under
//! `tests/fixtures/public/worlds/` is one of those cases, and every test below
//! reads the fixture off disk rather than building a world in memory, so what is
//! checked is the recorded case rather than a restatement of it.
//!
//! The two refusals are then driven through the real gate. `SavedDataError`
//! implements `StructuredFailure`, so a refusal goes into a
//! [`PreWorldReport`] like any other and
//! [`LifecycleMachine::open_world`] withholds the world — the same path
//! `pre_world_gate.rs` proves for invalid packages. Nothing here builds a second
//! gate to check the first one against.
//!
//! What no test here does is move bytes inside a real world. `ravel-save`'s own
//! documentation and the M3-W4 row both put that behind the game objects ADG-4's
//! amendment admits, so a fixture records the versions and the state, which is
//! what the decision turns on.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use ravel_core::{FailureEnvironment, LifecycleState, ModId, Side, SideSet, StructuredFailure};
use ravel_host_api::HostMod;
use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement, WorldRefused};
use ravel_observability::{PreWorldReport, Subject};
use ravel_save::{SavedDataError, SavedDataVersion};
use ravel_test_mod_saved_data::{MOD_ID, SavedDataMod, SavedState};
use serde_json::Value;

const WORLDS: &str = "tests/fixtures/public/worlds";

const RUNNING_RAVEL: &str = "0.1.0";
const RUNNING_MINECRAFT: &str = "26.0.0";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two directories below the repository root")
        .to_path_buf()
}

/// Reads one fixture world off disk.
fn world(name: &str) -> Value {
    let path = repository_root().join(WORLDS).join(name).join("world.json");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("`{WORLDS}/{name}/world.json` should be readable: {error}"));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("`{WORLDS}/{name}/world.json` should be JSON: {error}"))
}

/// Returns the entries a world holds, which is always an array.
///
/// A world that omitted the field would be indistinguishable from one holding
/// nothing, and the fixture README turns on those two being different events, so
/// an absent field is a broken fixture rather than an empty world.
fn entries(name: &str) -> Vec<Value> {
    world(name)["savedData"]
        .as_array()
        .unwrap_or_else(|| panic!("`{name}` records no `savedData` array"))
        .clone()
}

/// Returns what `name` holds for `owner`, or `None` when it holds nothing.
fn stored(name: &str, owner: &str) -> Option<(SavedDataVersion, SavedState)> {
    entries(name)
        .iter()
        .find(|entry| entry["owner"].as_str() == Some(owner))
        .map(|entry| {
            let raw = entry["version"]
                .as_u64()
                .unwrap_or_else(|| panic!("`{name}` records no version for `{owner}`"));
            let version = u32::try_from(raw)
                .ok()
                .and_then(SavedDataVersion::new)
                .unwrap_or_else(|| {
                    panic!("`{name}` records version {raw}, which is not a version")
                });
            (version, state_of(&entry["state"]))
        })
}

/// Reads the opaque state a world holds into the shape its owning mod uses.
fn state_of(state: &Value) -> SavedState {
    SavedState {
        visits: state["visits"]
            .as_u64()
            .and_then(|visits| u32::try_from(visits).ok())
            .expect("a world records a visit count"),
        // Version 1 predates the waypoint list, so its absence is the format
        // rather than a broken fixture.
        waypoints: state["waypoints"]
            .as_array()
            .map_or_else(Vec::new, |items| {
                items
                    .iter()
                    .map(|item| item.as_str().expect("a waypoint is text").to_owned())
                    .collect()
            }),
    }
}

fn environment() -> FailureEnvironment {
    FailureEnvironment {
        ravel: RUNNING_RAVEL.parse().expect("a valid version"),
        minecraft: RUNNING_MINECRAFT.to_owned(),
        platform: "linux".to_owned(),
        architecture: "x86_64".to_owned(),
    }
}

/// The plan entry the loader carries for this mod.
///
/// `saved_data_versions` is filled from the mod's own declaration because that
/// is what a package manifest supplies at runtime: `ravel-package` parses the
/// field and `ravel-testkit`'s pipeline copies it into the entry. Writing it
/// here from the same declaration keeps the two agreeing without a package.
fn entry(saved: &SavedDataMod) -> LifecycleEntry {
    LifecycleEntry {
        mod_id: saved.id().clone(),
        sides: SideSet::new([Side::Client, Side::DedicatedServer]).expect("a non-empty side set"),
        requirement: Requirement::Required,
        saved_data_versions: saved.declaration().versions().collect(),
    }
}

/// A machine holding only this mod, driven to [`LifecycleState::Running`].
fn running(saved: &SavedDataMod) -> LifecycleMachine {
    let mut machine =
        LifecycleMachine::new(Side::Client, vec![entry(saved)]).expect("one mod is a legal plan");
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
    machine
}

/// Opens `name` and returns whatever refused it.
fn refusal(name: &str) -> SavedDataError {
    SavedDataMod::new()
        .open(stored(name, MOD_ID))
        .expect_err("this world does not open")
}

// ------------------------------------------------------- fixture agreement ---

#[test]
fn every_fixture_world_is_written_for_the_mod_this_test_drives() {
    // The fixture README states this pairing rather than leaving it inferred,
    // so a change to either side has to break something visible.
    for name in ["current", "migratable", "future", "absent"] {
        for entry in entries(name) {
            let owner = entry["owner"].as_str().expect("an owner is text");
            assert_eq!(
                owner.parse::<ModId>().expect("a legal mod id"),
                *SavedDataMod::new().id(),
                "`{name}` records data owned by a mod this test does not drive"
            );
        }
    }

    let saved = SavedDataMod::new();
    assert_eq!(saved.current().get(), 3, "the fixtures assume 3 is written");
    assert_eq!(
        saved.declaration().versions().collect::<Vec<_>>(),
        [1, 2, 3].map(|raw| SavedDataVersion::new(raw).expect("non-zero")),
        "the fixtures assume this mod declares 1, 2 and 3"
    );
}

#[test]
fn the_loader_carries_the_versions_the_mod_declares() {
    let saved = SavedDataMod::new();
    let machine = running(&saved);

    assert_eq!(
        machine.saved_data_versions(saved.id()),
        Ok(&saved.declaration().versions().collect::<BTreeSet<_>>()),
        "a plan that lost the declared versions could not decide a world"
    );
}

// -------------------------------------------------------------- round trip ---

#[test]
fn a_created_world_reloads_with_the_state_it_saved() {
    // Create: a world nothing has been written for yet.
    let mut created = SavedDataMod::new();
    assert_eq!(created.state(), &SavedState::default());

    // Save, after a session that changed something.
    created.visit("spawn");
    created.visit("mine");
    let (version, written) = created.save();
    assert_eq!(version, created.current());

    // Exit.
    drop(created);

    // Reload.
    let mut reloaded = SavedDataMod::new();
    let plan = reloaded
        .open(Some((version, written.clone())))
        .expect("a world this build just wrote opens");

    assert!(plan.is_current(), "a world just written needs no migration");
    assert_eq!(reloaded.state(), &written);
    assert_eq!(reloaded.state().visits, 2);
}

#[test]
fn a_fixture_world_survives_a_visit_and_a_reload() {
    let held = stored("current", MOD_ID).expect("`current` holds this mod's data");

    let mut opened = SavedDataMod::new();
    opened.open(Some(held.clone())).expect("`current` opens");
    assert_eq!(opened.state(), &held.1, "opening changed the state");

    opened.visit("summit");
    let (version, written) = opened.save();
    drop(opened);

    let mut reloaded = SavedDataMod::new();
    reloaded
        .open(Some((version, written)))
        .expect("the world it just wrote opens");

    assert_eq!(reloaded.state().visits, held.1.visits + 1);
    assert_eq!(reloaded.state().waypoints, ["mine", "spawn", "summit"]);
}

#[test]
fn an_older_world_opens_and_names_every_version_it_steps_through() {
    let mut saved = SavedDataMod::new();
    let (version, held) = stored("migratable", MOD_ID).expect("`migratable` holds data");
    assert_eq!(version.get(), 1);

    let plan = saved
        .open(Some((version, held.clone())))
        .expect("this build reads version 1");

    assert_eq!(
        plan.steps(),
        [2, 3].map(|raw| SavedDataVersion::new(raw).expect("non-zero")),
        "a plan that skipped a version would be migrating across a gap"
    );
    // Version 1 kept no waypoint list, and the state it did keep survives.
    assert_eq!(saved.state().visits, held.visits);
    assert_eq!(saved.state().waypoints, ["spawn"]);
}

// ------------------------------------------------------------ fails closed ---

#[test]
fn a_world_from_a_newer_build_cannot_open() {
    let (version, _) = stored("future", MOD_ID).expect("`future` holds data");
    assert_eq!(version.get(), 9);

    assert_eq!(
        refusal("future"),
        SavedDataError::FutureVersion {
            owner: SavedDataMod::new().id().clone(),
            stored: version,
            current: SavedDataMod::new().current(),
        }
    );
}

#[test]
fn a_world_holding_no_data_this_mod_owns_cannot_open() {
    assert!(
        entries("absent").is_empty(),
        "`absent` is the world that records an empty list rather than omitting one"
    );
    assert_eq!(stored("absent", MOD_ID), None);

    assert_eq!(
        refusal("absent"),
        SavedDataError::MissingRequiredData {
            owner: SavedDataMod::new().id().clone(),
        }
    );
}

#[test]
fn neither_refusal_lets_a_world_open() {
    let saved = SavedDataMod::new();
    let machine = running(&saved);

    // The set itself is running and would open a world, which is what makes
    // the refusal below the thing that withheld it.
    assert_eq!(
        machine.open_world(&PreWorldReport::new(environment())),
        Ok(())
    );

    for name in ["future", "absent"] {
        let mut report = PreWorldReport::new(environment());
        report.refuse(&Subject::of(saved.id().clone()), &refusal(name));

        let refused = machine
            .open_world(&report)
            .expect_err("a report holding a refusal withholds the world");
        assert!(
            matches!(refused, WorldRefused::PreWorld(_)),
            "`{name}`: {refused}"
        );
    }
}

#[test]
fn every_refusal_is_decided_before_any_mod_runs() {
    for name in ["future", "absent"] {
        let error = refusal(name);

        assert_eq!(
            error.lifecycle_state(),
            LifecycleState::Registered,
            "`{name}` was refused after the point a world could already have been mutated"
        );
        assert_eq!(
            error.failure_kind(),
            None,
            "`{name}` borrowed a dependency-graph outcome for a saved-data failure"
        );
        assert!(!error.quotes_untrusted_text(), "{name}");
        assert!(error.diagnostic().text().contains(MOD_ID), "{name}");
    }
}

#[test]
fn no_two_refusals_read_the_same_way() {
    assert_ne!(
        refusal("future").to_string(),
        refusal("absent").to_string(),
        "two refusals a player cannot tell apart are one report"
    );
}
