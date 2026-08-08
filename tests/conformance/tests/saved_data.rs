//! The save test mod driven over the world fixtures, opening what it may and
//! refusing what it may not.
//!
//! `ROADMAP.md`'s M3-W4 row asks that create/save/exit/reload preserve state and
//! that incompatible data fail closed. Each world under
//! `tests/fixtures/public/worlds/` is one of those cases, and every test below
//! reads the fixture off disk rather than building a world in memory, so what is
//! checked is the recorded case rather than a restatement of it. ADG-7 settles
//! the one case a world file cannot state: absence initializes fresh state for
//! any world, so `absent` records an initialization rather than a refusal.
//!
//! Every refusal is then driven through the real gate. `SavedDataError`
//! implements `StructuredFailure`, so a refusal goes into a
//! [`PreWorldReport`] like any other and
//! [`LifecycleMachine::open_world`] withholds the world — the same path
//! `pre_world_gate.rs` proves for invalid packages. Nothing here builds a second
//! gate to check the first one against.
//!
//! Two of them turn on a stored data version, which [`SavedDataPlan`] decides on
//! its own. The other three cannot be stated by a version at all: the format the
//! canonical aggregate arrived in, the schema one owner's envelope was written
//! in, and a package the stored data requires at a version this installation
//! does not hold. Those go through [`SavedDataAggregate::open`], which is the
//! admission the host runs over the whole stored value, and the fixtures
//! carrying them record the fields the aggregate turns on rather than a version
//! alone. The aggregate-format one is the only refusal here with no owner to
//! blame, and it is reported against an unidentified subject instead of being
//! pinned on whichever mod happened to be installed.
//!
//! What no test here does is move bytes inside a real world. `ravel-save`'s own
//! documentation and the M3-W4 row both put that behind the game objects ADG-4's
//! amendment admits, so a fixture records the versions and the state, which is
//! what the decision turns on.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use ravel_core::{
    FailureEnvironment, LifecycleState, ModId, Side, SideSet, StructuredFailure, Version,
};
use ravel_host_api::HostMod;
use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement, WorldRefused};
use ravel_observability::{PreWorldReport, Subject};
use ravel_save::{
    SavedDataAggregate, SavedDataDeclaration, SavedDataError, SavedDataPlan, SavedDataVersion,
};
use ravel_test_mod_saved_data::{MOD_ID, SavedDataMod, SavedState};
use serde_json::{Value, json};

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

/// Every incompatibility that withholds a world, named for reporting.
///
/// Absence is not among them: ADG-7 makes it fresh initialization. What is left
/// is a world this build cannot read, and both cases turn on the declared
/// versions rather than on the file being there — `future` is a version above
/// the highest declared, and a gap is a version below it with no path across.
fn refusals() -> [(&'static str, SavedDataError); 2] {
    let gapped = SavedDataDeclaration::new(
        MOD_ID.parse().expect("a legal mod id"),
        [1, 3].map(|raw| SavedDataVersion::new(raw).expect("non-zero")),
    )
    .expect("two versions");

    [
        ("future", refusal("future")),
        (
            "gapped",
            SavedDataPlan::resolve(&gapped, SavedDataVersion::new(1).expect("non-zero"))
                .expect_err("a gap has no migration path"),
        ),
    ]
}

/// The canonical aggregate bytes `name` records, as a world holds them.
///
/// The fixture records the world; this builds the one value Minecraft stores out
/// of it. It is written as bytes rather than through [`ravel_save`]'s
/// constructors on purpose: the constructors stamp the only schemas this release
/// writes, so a world written under any other one could not be expressed at all,
/// and those are exactly the worlds three of these fixtures are.
fn aggregate_bytes(name: &str) -> Vec<u8> {
    let owners: Vec<Value> = entries(name)
        .iter()
        .map(|entry| {
            json!({
                "schemaVersion": entry
                    .get("envelopeSchemaVersion")
                    .cloned()
                    .unwrap_or_else(|| json!(1)),
                "owner": entry["owner"],
                "dataVersion": entry["version"],
                "requiredPackages": entry
                    .get("requiredPackages")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
                "payload": entry["state"],
            })
        })
        .collect();

    serde_json::to_vec(&json!({
        "formatVersion": world(name)
            .get("aggregateFormatVersion")
            .cloned()
            .unwrap_or_else(|| json!(1)),
        "owners": owners,
    }))
    .unwrap_or_else(|error| panic!("`{name}` should serialize as an aggregate: {error}"))
}

/// What this installation holds, which is not something a world can state.
///
/// One package at one version, so a world requiring another version of it is
/// refused for the version rather than for the package being absent.
fn installation() -> BTreeMap<ModId, Version> {
    BTreeMap::from([(
        "base".parse().expect("a legal mod id"),
        Version::new(1, 0, 0),
    )])
}

/// Admits `name` through the canonical aggregate and returns whatever refused it.
fn admission_refusal(name: &str) -> SavedDataError {
    SavedDataAggregate::open(
        Some(&aggregate_bytes(name)),
        &[SavedDataMod::new().declaration()],
        &installation(),
    )
    .expect_err("this world does not open")
}

/// Every incompatibility the canonical aggregate decides, named for reporting.
///
/// These are the three a stored data version cannot state on its own: the format
/// the aggregate itself arrived in, the schema one owner's envelope was written
/// in, and a package that data requires at a version this installation does not
/// hold. Each fixture carries a data version and a state the mod reads, so the
/// field it exists for is the only thing that can have refused it.
fn admission_refusals() -> [(&'static str, SavedDataError); 3] {
    [
        (
            "unsupported-format",
            admission_refusal("unsupported-format"),
        ),
        (
            "unsupported-schema",
            admission_refusal("unsupported-schema"),
        ),
        (
            "conflicting-package",
            admission_refusal("conflicting-package"),
        ),
    ]
}

// ------------------------------------------------------- fixture agreement ---

#[test]
fn every_fixture_world_is_written_for_the_mod_this_test_drives() {
    // The fixture README states this pairing rather than leaving it inferred,
    // so a change to either side has to break something visible.
    for name in [
        "current",
        "migratable",
        "future",
        "absent",
        "unsupported-format",
        "unsupported-schema",
        "conflicting-package",
    ] {
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
fn a_world_holding_no_data_this_mod_owns_initializes_fresh_state() {
    assert!(
        entries("absent").is_empty(),
        "`absent` is the world that records an empty list rather than omitting one"
    );
    assert_eq!(stored("absent", MOD_ID), None);

    // ADG-7 gives absence one policy for every world: it initializes rather than
    // refusing. Treating it as corruption would invent a decision from file
    // identity instead of asking `SavedDataPlan`.
    let mut saved = SavedDataMod::new();
    let plan = saved
        .open(stored("absent", MOD_ID))
        .expect("a world holding nothing for this mod initializes fresh state");

    assert!(plan.is_current());
    assert_eq!(saved.state(), &SavedState::default());
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

    for (name, error) in refusals() {
        let mut report = PreWorldReport::new(environment());
        report.refuse(&Subject::of(saved.id().clone()), &error);

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
    for (name, error) in refusals() {
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
    let [(_, future), (_, gapped)] = refusals();

    assert_ne!(
        future.to_string(),
        gapped.to_string(),
        "two refusals a player cannot tell apart are one report"
    );
}

// ------------------------------------------- fails closed: the whole value ---

#[test]
fn an_aggregate_format_this_build_cannot_read_blames_no_mod() {
    let error = admission_refusal("unsupported-format");
    assert_eq!(
        error.owner(),
        None,
        "the container refused the world, so no installed mod is at fault for it"
    );

    let SavedDataError::UnsupportedAggregateVersion { stored, current } = &error else {
        panic!("`unsupported-format` is refused for its format, not for {error}");
    };
    assert_eq!(*stored, 2);
    assert_eq!(
        *current, 1,
        "the worlds that do open are written in the format this build reads"
    );
}

#[test]
fn an_envelope_schema_this_build_cannot_read_blames_its_owner() {
    let error = admission_refusal("unsupported-schema");

    let SavedDataError::UnsupportedEnvelopeVersion {
        owner,
        stored: schema,
        current,
    } = &error
    else {
        panic!("`unsupported-schema` is refused for its envelope, not for {error}");
    };
    assert_eq!(owner, SavedDataMod::new().id());
    assert_eq!(*schema, 2);
    assert_eq!(*current, 1);

    // The data version inside is one this build reads, so a plan would have
    // admitted it. The envelope carrying it is what did not.
    let (version, _) = stored("unsupported-schema", MOD_ID).expect("the world holds this mod");
    let mut opened = SavedDataMod::new();
    opened
        .open(Some((version, SavedState::default())))
        .expect("the data version this envelope carries opens on its own");
}

#[test]
fn a_package_installed_at_another_version_refuses_the_data_that_requires_it() {
    let error = admission_refusal("conflicting-package");

    let SavedDataError::IncompatiblePackage(conflict) = &error else {
        panic!("`conflicting-package` is refused for its package, not for {error}");
    };
    assert_eq!(&conflict.owner, SavedDataMod::new().id());
    assert_eq!(conflict.package, "base".parse().expect("a legal mod id"));
    assert_eq!(conflict.required, Version::new(2, 0, 0));
    assert_eq!(
        conflict.installed,
        installation()
            .get(&conflict.package)
            .expect("the package is installed")
            .clone(),
        "a version this installation does not hold would be the missing-package case"
    );
}

#[test]
fn no_admission_refusal_lets_a_world_open() {
    let saved = SavedDataMod::new();
    let machine = running(&saved);

    for (name, error) in admission_refusals() {
        let mut report = PreWorldReport::new(environment());
        // A refusal the aggregate blames nobody for is reported as the
        // unidentified subject rather than pinned on an arbitrary installed mod.
        let subject = error
            .owner()
            .cloned()
            .map_or_else(Subject::unidentified, Subject::of);
        report.refuse(&subject, &error);

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
fn every_admission_refusal_is_decided_before_any_mod_runs() {
    for (name, error) in admission_refusals() {
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
        assert_eq!(
            error.diagnostic().text().contains(MOD_ID),
            error.owner().is_some(),
            "`{name}` names a mod exactly when one is what the refusal is about"
        );
    }
}

#[test]
fn no_refusal_reads_the_same_way_as_another() {
    let mut read: Vec<(&str, String)> = Vec::new();

    for (name, error) in refusals().into_iter().chain(admission_refusals()) {
        let text = error.to_string();
        for (other, previous) in &read {
            assert_ne!(
                &text, previous,
                "`{name}` and `{other}` are one report a player cannot tell apart"
            );
        }
        read.push((name, text));
    }

    assert_eq!(read.len(), 5, "every refusal M3-W4 names belongs in here");
}
