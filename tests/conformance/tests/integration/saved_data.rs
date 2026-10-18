//! Saved-data fixture admission, migration, byte round trips, and refusal boundaries.
//!
//! These tests exercise the public loader open pipeline; bootstrap tests own JNI delivery.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use ravel_core::{LifecycleState, ModId, Side, SideSet, StructuredFailure, Version};
use ravel_host_api::HostError;
use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement, SaveRefused};
use ravel_save::{
    MAX_AGGREGATE_BYTES, SavedDataAggregate, SavedDataDeclaration, SavedDataEnvelope,
    SavedDataError, SavedDataPlan, SavedDataVersion,
};
use ravel_sdk::Mod;
use ravel_test_mod_saved_data::{MOD_ID, SavedDataMod, SavedState};
use serde_json::{Value, json};

const WORLDS: &str = "tests/fixtures/public/worlds";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two directories below the repository root")
        .to_path_buf()
}

fn world(name: &str) -> Value {
    let path = repository_root().join(WORLDS).join(name).join("world.json");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("`{WORLDS}/{name}/world.json` should be readable: {error}"));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("`{WORLDS}/{name}/world.json` should be JSON: {error}"))
}

// A missing savedData field is a broken fixture, not an empty world.
fn entries(name: &str) -> Vec<Value> {
    world(name)["savedData"]
        .as_array()
        .unwrap_or_else(|| panic!("`{name}` records no `savedData` array"))
        .clone()
}

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

fn state_of(state: &Value) -> SavedState {
    SavedState {
        visits: state["visits"]
            .as_u64()
            .and_then(|visits| u32::try_from(visits).ok())
            .expect("a world records a visit count"),
        // Version 1 has no waypoint list.
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

fn entry(saved: &SavedDataMod) -> LifecycleEntry {
    LifecycleEntry {
        mod_id: saved.id().clone(),
        sides: SideSet::new([Side::Client, Side::DedicatedServer]).expect("a non-empty side set"),
        requirement: Requirement::Required,
        saved_data_versions: saved
            .declaration()
            .expect("valid declaration")
            .versions()
            .collect(),
    }
}

fn running(saved: &mut SavedDataMod) -> LifecycleMachine {
    let mut machine =
        LifecycleMachine::new(Side::Client, vec![entry(saved)]).expect("a legal lifecycle plan");
    ravel_loader::drive_loaded(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [saved])
            .expect("all active mods have hosts"),
        &ravel_bootstrap_runtime::environment(),
    )
    .expect("the production driver starts the saved-data mod");
    machine
}

// A version gap belongs to the mod declaration and cannot be a world fixture.
fn refusals() -> [(&'static str, SavedDataError); 2] {
    let gapped = SavedDataDeclaration::new(
        MOD_ID.parse().expect("a legal mod id"),
        [1, 3].map(|raw| SavedDataVersion::new(raw).expect("non-zero")),
    )
    .expect("two versions");

    [
        ("future", admission_refusal("future")),
        (
            "gapped",
            SavedDataPlan::resolve(&gapped, SavedDataVersion::new(1).expect("non-zero"))
                .expect_err("a gap has no migration path"),
        ),
    ]
}

// Bypass constructors so fixtures can carry unsupported format and schema versions.
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

fn installation() -> BTreeMap<ModId, Version> {
    BTreeMap::from([(
        "base".parse().expect("a legal mod id"),
        Version::new(1, 0, 0),
    )])
}

fn admission_refusal(name: &str) -> SavedDataError {
    admit(Some(&aggregate_bytes(name))).expect_err("this world does not open")
}

// Non-JSON filler distinguishes size admission from parsing.
fn oversized_bytes() -> Vec<u8> {
    vec![b'\0'; MAX_AGGREGATE_BYTES + 1]
}

fn admission_refusals() -> [(&'static str, SavedDataError); 4] {
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
        (
            "oversized",
            admit(Some(&oversized_bytes()))
                .expect_err("an aggregate over the ceiling does not open a world"),
        ),
    ]
}

fn admit(bytes: Option<&[u8]>) -> Result<SavedDataAggregate, SavedDataError> {
    SavedDataAggregate::open(
        bytes,
        &[SavedDataMod::new().declaration()?],
        &installation(),
    )
}

// One envelope this mod owns, at `version`, outside any fixture world.
fn world_holding(version: SavedDataVersion, payload: Value) -> Vec<u8> {
    let mut aggregate = SavedDataAggregate::fresh();
    aggregate.replace(SavedDataEnvelope::new(
        MOD_ID.parse().expect("a legal mod id"),
        version,
        Vec::new(),
        Some("urn:ravel:test:saved-state:1".to_owned()),
        payload,
    ));
    aggregate.to_bytes().expect("one small envelope is bounded")
}

fn emitted(saved: &SavedDataMod) -> Vec<u8> {
    let mut aggregate = SavedDataAggregate::fresh();
    if let Some(envelope) = saved
        .save_saved_data()
        .expect("a running mod produces its envelope")
    {
        aggregate.replace(envelope);
    }
    aggregate
        .to_bytes()
        .expect("one small envelope is under the ceiling")
}

// The loader opens the world and delivers it; the plan names the steps it had to take.
fn reload(saved: &mut SavedDataMod, bytes: Option<&[u8]>) -> Result<SavedDataPlan, SaveRefused> {
    let mut machine = running(saved);
    ravel_loader::open_saved_data(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut *saved])
            .expect("the saved-data mod is loaded"),
        &ravel_bootstrap_runtime::environment(),
        bytes,
        &installation(),
    )?;
    let declaration = saved.declaration().map_err(SaveRefused::Data)?;
    let aggregate = admit(bytes).map_err(SaveRefused::Data)?;
    let envelope = aggregate.envelope(declaration.owner());
    SavedDataPlan::open(&declaration, envelope.map(SavedDataEnvelope::data_version))
        .map_err(SaveRefused::Data)
}

#[test]
fn every_fixture_world_is_written_for_the_mod_this_test_drives() {
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
        saved
            .declaration()
            .expect("valid declaration")
            .versions()
            .collect::<Vec<_>>(),
        [1, 2, 3].map(|raw| SavedDataVersion::new(raw).expect("non-zero")),
        "the fixtures assume this mod declares 1, 2 and 3"
    );
}

#[test]
fn the_loader_carries_the_versions_the_mod_declares() {
    let mut saved = SavedDataMod::new();
    let machine = running(&mut saved);

    assert_eq!(
        machine.saved_data_versions(saved.id()),
        Ok(&saved
            .declaration()
            .expect("valid declaration")
            .versions()
            .collect::<BTreeSet<_>>()),
        "a plan that lost the declared versions could not decide a world"
    );
}

#[test]
fn a_created_world_reloads_with_the_state_it_saved() {
    let mut created = SavedDataMod::new();
    assert_eq!(created.state(), &SavedState::default());

    created.visit("spawn");
    created.visit("mine");
    let written = created.state().clone();
    let bytes = emitted(&created);

    drop(created);

    let mut reloaded = SavedDataMod::new();
    let plan = reload(&mut reloaded, Some(&bytes)).expect("a world this build just wrote opens");

    assert!(plan.is_current(), "a world just written needs no migration");
    assert_eq!(
        reloaded.state().waypoints,
        written.waypoints,
        "the bytes that crossed did not carry the state that was saved"
    );
    // Opening increments visits; waypoints distinguish restoration from fresh state.
    assert_eq!(reloaded.state().visits, written.visits + 1);
}

#[test]
fn a_fixture_world_survives_a_visit_and_a_reload() {
    let (_, held) = stored("current", MOD_ID).expect("`current` holds this mod's data");

    let mut opened = SavedDataMod::new();
    reload(&mut opened, Some(&aggregate_bytes("current"))).expect("`current` opens");
    assert_eq!(
        opened.state().waypoints,
        held.waypoints,
        "opening changed the state"
    );
    assert_eq!(
        opened.state().visits,
        held.visits + 1,
        "the open went uncounted"
    );

    opened.visit("summit");
    let bytes = emitted(&opened);
    drop(opened);

    let mut reloaded = SavedDataMod::new();
    reload(&mut reloaded, Some(&bytes)).expect("the world it just wrote opens");

    assert_eq!(reloaded.state().visits, held.visits + 3);
    assert_eq!(reloaded.state().waypoints, ["mine", "spawn", "summit"]);
}

#[test]
fn every_byte_a_save_emits_is_admissible_again_unchanged() {
    let mut saved = SavedDataMod::new();
    reload(&mut saved, Some(&aggregate_bytes("current"))).expect("`current` opens");
    saved.visit("summit");

    let first = emitted(&saved);
    let admitted = admit(Some(&first)).expect("a produced aggregate is admissible");
    assert_eq!(
        admitted.to_bytes().expect("bounded"),
        first,
        "the aggregate a save emitted did not survive being read and written again"
    );

    let envelope = admitted
        .envelope(SavedDataMod::new().id())
        .expect("the owner that saved is in what it saved");
    assert_eq!(envelope.data_version(), saved.current());
    assert_eq!(
        envelope.payload(),
        &serde_json::to_value(saved.state()).expect("the state serializes"),
        "the envelope that crossed holds something other than the mod's state"
    );
}

#[test]
fn an_older_world_opens_and_names_every_version_it_steps_through() {
    let mut saved = SavedDataMod::new();
    let (version, held) = stored("migratable", MOD_ID).expect("`migratable` holds data");
    assert_eq!(version.get(), 1);

    let plan = reload(&mut saved, Some(&aggregate_bytes("migratable")))
        .expect("this build reads version 1");

    assert_eq!(
        plan.steps(),
        [2, 3].map(|raw| SavedDataVersion::new(raw).expect("non-zero")),
        "a plan that skipped a version would be migrating across a gap"
    );
    assert_eq!(saved.state().visits, held.visits + 1);
    // Version 1 kept the spawn point implicit, so only the step into 2 names it.
    assert_eq!(saved.state().waypoints, ["spawn"]);

    let migrated = admit(Some(&emitted(&saved))).expect("what a migration produced is admissible");
    let envelope = migrated
        .envelope(saved.id())
        .expect("the owner that migrated is in what it saved");
    assert_eq!(envelope.data_version(), saved.current());
    assert_eq!(envelope.payload()["waypoints"], json!(["spawn"]));

    // Version 2 stored waypoints in insertion order, duplicates included. Only the
    // step into 3 sorts and deduplicates them, and a second step into 2 would add
    // a spawn point this world never held.
    let mut ordered = SavedDataMod::new();
    let insertion_order = world_holding(
        SavedDataVersion::new(2).expect("non-zero"),
        json!({ "visits": 4, "waypoints": ["summit", "mine", "summit"] }),
    );
    let plan = reload(&mut ordered, Some(&insertion_order)).expect("this build reads version 2");

    assert_eq!(
        plan.steps(),
        [SavedDataVersion::new(3).expect("non-zero")],
        "a version-2 world steps only into the current version"
    );
    assert_eq!(
        ordered.state(),
        &SavedState {
            visits: 5,
            waypoints: vec!["mine".to_owned(), "summit".to_owned()],
        }
    );
}

#[test]
fn aggregate_admission_rejects_data_from_a_newer_build() {
    let (version, _) = stored("future", MOD_ID).expect("`future` holds data");
    assert_eq!(version.get(), 9);

    assert_eq!(
        admission_refusal("future"),
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

    let mut saved = SavedDataMod::new();
    for bytes in [Some(aggregate_bytes("absent")), None] {
        let plan = reload(&mut saved, bytes.as_deref())
            .expect("a world holding nothing for this mod initializes fresh state");

        assert!(plan.is_current());
        // Absence resets the prior world's state rather than accumulating visits.
        assert_eq!(
            saved.state(),
            &SavedState {
                visits: 1,
                waypoints: Vec::new()
            }
        );
    }
}

#[test]
fn a_refused_world_never_reaches_the_mod_that_owns_its_data() {
    let admissible = aggregate_bytes("current");
    for (name, bytes) in [
        ("future", aggregate_bytes("future")),
        ("unsupported-format", aggregate_bytes("unsupported-format")),
        ("unsupported-schema", aggregate_bytes("unsupported-schema")),
        (
            "conflicting-package",
            aggregate_bytes("conflicting-package"),
        ),
        ("oversized", oversized_bytes()),
    ] {
        let mut saved = SavedDataMod::new();
        reload(&mut saved, Some(&admissible)).expect("`current` opens");
        let before = saved.state().clone();

        let refused = reload(&mut saved, Some(&bytes)).expect_err("this world does not open");

        assert!(
            !refused.to_string().is_empty(),
            "`{name}` refused without saying why"
        );
        assert_eq!(
            saved.state(),
            &before,
            "`{name}` was refused and delivered to the mod anyway"
        );
    }
}

#[test]
fn a_payload_its_owner_cannot_read_refuses_the_world_rather_than_opening_it_fresh() {
    let bytes = world_holding(SavedDataMod::new().current(), json!({ "visits": "many" }));

    admit(Some(&bytes)).expect("nothing `ravel-save` decides refuses this world");

    let mut saved = SavedDataMod::new();
    reload(&mut saved, Some(&aggregate_bytes("current"))).expect("`current` opens");
    let before = saved.state().clone();

    let refused = reload(&mut saved, Some(&bytes))
        .expect_err("a payload its owner cannot read opens no world");

    let owner: ModId = MOD_ID.parse().expect("a legal mod id");
    assert_eq!(refused.mod_id(), Some(&owner));
    let source = std::error::Error::source(&refused)
        .and_then(|source| source.downcast_ref::<HostError>())
        .unwrap_or_else(|| panic!("the owner's own refusal is the source of {refused:?}"));
    assert_eq!(
        source,
        &HostError::OperationRefused {
            mod_id: owner,
            state: LifecycleState::Running,
            detail: r#"invalid type: string "many", expected u32"#.to_owned(),
        }
    );
    assert_eq!(
        saved.state(),
        &before,
        "a world the owner refused was delivered anyway"
    );
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

    let (version, state) = stored("unsupported-schema", MOD_ID).expect("the world holds this mod");
    let mut opened = SavedDataMod::new();
    let envelope = SavedDataEnvelope::new(
        opened.id().clone(),
        version,
        Vec::new(),
        Some("urn:ravel:test:saved-state:1".to_owned()),
        serde_json::to_value(&state).expect("the fixture state serializes"),
    );
    opened
        .open_saved_data(Some(&envelope))
        .expect("the data version this envelope carries opens on its own");
    assert_eq!(
        opened.state(),
        &SavedState {
            visits: state.visits + 1,
            waypoints: state.waypoints,
        }
    );
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
fn a_value_too_large_to_cross_is_refused_and_blames_no_mod() {
    let error = admit(Some(&oversized_bytes())).expect_err("the ceiling is not negotiable");

    assert_eq!(
        error,
        SavedDataError::AggregateTooLarge {
            limit: MAX_AGGREGATE_BYTES
        }
    );
    assert_eq!(
        error.owner(),
        None,
        "the crossing refused the world, so no installed mod is at fault for it"
    );
    assert!(matches!(
        admit(Some(&vec![b'\0'; MAX_AGGREGATE_BYTES])),
        Err(SavedDataError::InvalidAggregate(_))
    ));
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

    assert_eq!(read.len(), 6, "every refusal M3-W4 names belongs in here");
}
