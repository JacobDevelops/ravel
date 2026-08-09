//! The save test mod driven over the world fixtures, opening what it may and
//! refusing what it may not.
//!
//! `ROADMAP.md`'s M3-W4 row asks that create/save/exit/reload preserve state and
//! that incompatible data fail closed. Each world under
//! `tests/fixtures/public/worlds/` is one of those cases, and wherever a case can
//! be recorded as a world it is read off disk rather than built here, so what is
//! checked is the recorded case rather than a restatement of it. Two cases cannot
//! be recorded that way and are built in memory instead, each said so at its
//! construction: a gap in the declared versions is a property of the *mod's*
//! declaration rather than of any world, and an aggregate over the ceiling is a
//! megabyte of filler that would be tracked for one integer comparison. ADG-7
//! settles the one case a world file could state and should not: absence
//! initializes fresh state for any world, so `absent` records an initialization
//! rather than a refusal.
//!
//! Every reload here crosses bytes. A save builds each owner's envelope through
//! [`HostMod::save_saved_data`], collects them into one
//! [`SavedDataAggregate`] and serializes it with
//! [`SavedDataAggregate::to_bytes`]; an open measures and parses those bytes back
//! through [`SavedDataAggregate::open`], resolves a plan and delivers the
//! envelope with [`HostMod::open_saved_data`]. That is the sequence
//! `ravel-bootstrap`'s `stage_world` and `save_world` run, in the same order,
//! and `reload` below is written against it — so a serializer that dropped what
//! it was given fails here. What `reload` cannot speak for is production's own
//! delivery: it restates that sequence rather than calling it, so a
//! `stage_world` that stopped handing the envelope over would leave every test
//! in this file green. `ravel-bootstrap`'s
//! `a_stored_aggregate_reaches_the_mod_that_owns_it` and
//! `a_world_reopened_from_what_its_save_emitted_holds_the_same_state` are what
//! constrain that, for the same reason Java is out of reach here: the crossing
//! and the identity-keyed staging around it are `ravel-bootstrap`'s, which no
//! maintained crate may depend on, so their own tests own that half.
//!
//! Refusals travel the route production has. A saved-data refusal is decided at
//! the open crossing, after the pre-world gate has already been asked, and it
//! leaves Rust as the diagnostic text of an `Err` that `stage_world` turns into
//! `OpenOutcome::Refused` for Java to clean up and present, exactly as ADG-7's
//! failure invariants say. It never enters a [`PreWorldReport`]: that report is
//! process-lifetime state built before any world is selected, so a refusal
//! placed in it would outlive the world that caused it and refuse every later
//! one — which is the opposite of ADG-7 making file deletion the recovery. The
//! report's own route is proven by `pre_world_gate.rs` and nothing here restates
//! it.
//!
//! Two refusals turn on a stored data version, which [`SavedDataPlan`] decides on
//! its own. The others cannot be stated by a version at all: the format the
//! canonical aggregate arrived in, the schema one owner's envelope was written
//! in, a package the stored data requires at a version this installation does not
//! hold, and a stored value larger than may cross at all. Those go through
//! [`SavedDataAggregate::open`], which is the admission the host runs over the
//! whole stored value. The aggregate-format and size ones are the refusals with
//! no owner to blame, and they name none rather than pinning one on whichever mod
//! happened to be installed.
//!
//! What no test here does is move bytes inside a real world. `ravel-save`'s own
//! documentation and the M3-W4 row both put that behind the game objects ADG-4's
//! amendment admits, so a fixture records the versions and the state, which is
//! what the decision turns on.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use ravel_core::{LifecycleState, ModId, Side, SideSet, StructuredFailure, Version};
use ravel_host_api::HostMod;
use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement};
use ravel_save::{
    MAX_AGGREGATE_BYTES, SavedDataAggregate, SavedDataDeclaration, SavedDataEnvelope,
    SavedDataError, SavedDataPlan, SavedDataVersion,
};
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

/// Every incompatibility a stored data version alone decides.
///
/// Absence is not among them: ADG-7 makes it fresh initialization. What is left
/// is a world this build cannot read, and both cases turn on the declared
/// versions rather than on the file being there — `future` is a version above
/// the highest declared, and a gap is a version below it with no path across.
///
/// `future` is a world and is read as one. A gap is not: it is a property of the
/// *mod's* declaration, so no world file could record it, and the declaration
/// that has one is built here.
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
    admit(Some(&aggregate_bytes(name))).expect_err("this world does not open")
}

/// A stored value larger than may cross at all.
///
/// Built rather than recorded: a world holding this would be a megabyte of
/// tracked filler existing for one integer comparison, and the ceiling is a
/// property of the crossing rather than of any world's contents. Deliberately not
/// JSON, so anything that parsed before measuring would refuse differently.
fn oversized_bytes() -> Vec<u8> {
    vec![b'\0'; MAX_AGGREGATE_BYTES + 1]
}

/// Every incompatibility the canonical aggregate decides, named for reporting.
///
/// These are the four a stored data version cannot state on its own: the format
/// the aggregate itself arrived in, the schema one owner's envelope was written
/// in, a package that data requires at a version this installation does not hold,
/// and a value too large to cross. Each fixture carries a data version and a
/// state the mod reads, so the field it exists for is the only thing that can
/// have refused it.
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

// ------------------------------------------------------- the production path ---

/// Admits at most one aggregate exactly as the open crossing does.
fn admit(bytes: Option<&[u8]>) -> Result<SavedDataAggregate, SavedDataError> {
    SavedDataAggregate::open(bytes, &[SavedDataMod::new().declaration()], &installation())
}

/// The bytes a save emits, built the way the host builds them.
///
/// `save_world` asks each installed mod for its envelope, collects them into one
/// aggregate and serializes that, so this asks the one mod under test for its
/// envelope and serializes the aggregate holding it. Nothing here reaches into
/// the mod's state: what crosses is whatever the mod chose to write.
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

/// Opens `bytes` into `saved` exactly as `stage_world` does, and answers as it
/// answers.
///
/// The sequence is the production one: admit the whole stored value, resolve this
/// owner's plan from the data version the admitted envelope carries, then deliver
/// that envelope. The error is text rather than a typed failure for the same
/// reason `stage_world`'s is — the diagnostic is what leaves Rust, and Java
/// cleans up and presents it without reproducing the taxonomy behind it.
fn reload(saved: &mut SavedDataMod, bytes: Option<&[u8]>) -> Result<SavedDataPlan, String> {
    let declaration = saved.declaration();
    let aggregate = admit(bytes).map_err(|error| error.to_string())?;
    let envelope = aggregate.envelope(declaration.owner());
    let plan = SavedDataPlan::open(&declaration, envelope.map(SavedDataEnvelope::data_version))
        .map_err(|error| error.to_string())?;
    saved
        .open_saved_data(envelope, &plan)
        .map_err(|error| error.to_string())?;
    Ok(plan)
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

    // Save, after a session that changed something. What leaves the process is
    // the serialized aggregate, so that is what this holds on to.
    created.visit("spawn");
    created.visit("mine");
    let written = created.state().clone();
    let bytes = emitted(&created);

    // Exit. Nothing but `bytes` survives it.
    drop(created);

    // Reload, from those bytes and nothing else.
    let mut reloaded = SavedDataMod::new();
    let plan = reload(&mut reloaded, Some(&bytes)).expect("a world this build just wrote opens");

    assert!(plan.is_current(), "a world just written needs no migration");
    assert_eq!(
        reloaded.state(),
        &written,
        "the bytes that crossed did not carry the state that was saved"
    );
    assert_eq!(reloaded.state().visits, 2);
}

#[test]
fn a_fixture_world_survives_a_visit_and_a_reload() {
    let (_, held) = stored("current", MOD_ID).expect("`current` holds this mod's data");

    let mut opened = SavedDataMod::new();
    reload(&mut opened, Some(&aggregate_bytes("current"))).expect("`current` opens");
    assert_eq!(opened.state(), &held, "opening changed the state");

    opened.visit("summit");
    let bytes = emitted(&opened);
    drop(opened);

    let mut reloaded = SavedDataMod::new();
    reload(&mut reloaded, Some(&bytes)).expect("the world it just wrote opens");

    assert_eq!(reloaded.state().visits, held.visits + 1);
    assert_eq!(reloaded.state().waypoints, ["mine", "spawn", "summit"]);
}

#[test]
fn every_byte_a_save_emits_is_admissible_again_unchanged() {
    // The pair of operations the game runs, back to back, with nothing between
    // them: whatever a save produced has to be exactly what the next open admits.
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
    // Version 1 kept no waypoint list, and the state it did keep survives.
    assert_eq!(saved.state().visits, held.visits);
    assert_eq!(saved.state().waypoints, ["spawn"]);

    // Migrated in memory is not migrated on disk until a save writes it, and what
    // it writes is the version this build writes rather than the one it read.
    let migrated = admit(Some(&emitted(&saved))).expect("what a migration produced is admissible");
    let envelope = migrated
        .envelope(saved.id())
        .expect("the owner that migrated is in what it saved");
    assert_eq!(envelope.data_version(), saved.current());
    assert_eq!(envelope.payload()["waypoints"], json!(["spawn"]));
}

// ------------------------------------------------------------ fails closed ---

#[test]
fn a_world_from_a_newer_build_cannot_open() {
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

    // ADG-7 gives absence one policy for every world: it initializes rather than
    // refusing. Treating it as corruption would invent a decision from file
    // identity instead of asking `SavedDataPlan`.
    let mut saved = SavedDataMod::new();
    // Both shapes of absence: a world holding an empty aggregate, and no
    // aggregate at all. ADG-7 opens them the same way.
    for bytes in [Some(aggregate_bytes("absent")), None] {
        let plan = reload(&mut saved, bytes.as_deref())
            .expect("a world holding nothing for this mod initializes fresh state");

        assert!(plan.is_current());
        assert_eq!(saved.state(), &SavedState::default());
    }
}

#[test]
fn a_refused_world_never_reaches_the_mod_that_owns_its_data() {
    // The route production has: admission answers with the diagnostic text and
    // no envelope is delivered, which is what `stage_world` turns into
    // `OpenOutcome::Refused` without any of it reaching a mod.
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
        // The same mod opens an admissible world, which is what makes the
        // refusal below this world's doing rather than the mod's.
        reload(&mut saved, Some(&admissible)).expect("`current` opens");
        let before = saved.state().clone();

        let refused = reload(&mut saved, Some(&bytes)).expect_err("this world does not open");

        assert!(!refused.is_empty(), "`{name}` refused without saying why");
        assert_eq!(
            saved.state(),
            &before,
            "`{name}` was refused and delivered to the mod anyway"
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
    // One byte under is admitted on length, so what refused above is the ceiling
    // rather than the filler being unreadable.
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

    // M3-W4 names schema, package, migration and size. Migration is two refusals
    // rather than one — a version above what this build reads and a gap below it
    // are different reports — and schema is two, because the container's format
    // and one owner's envelope schema are separately decided.
    assert_eq!(read.len(), 6, "every refusal M3-W4 names belongs in here");
}
