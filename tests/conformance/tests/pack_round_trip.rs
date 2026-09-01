#![forbid(unsafe_code)]

//! Whether a launch reads back the settings layer `ravel-pack` writes.
//!
//! M8's open question, and it is a round trip rather than two assertions: the pack
//! is built by `ravel_pack::validate`, written to disk by `ValidatedPack::write`,
//! and then read by `ravel_host_runtime::compose` — the same function every host's
//! JNI entry calls. Nothing here spells the path the two halves meet at. That is
//! the whole point: the writer and the reader take it from
//! `ravel_config::file::PACK_DIRECTORY`, and a test that named it itself would keep
//! passing after they drifted apart.
//!
//! What the assertions read is the value inside each mod, not the resolved
//! configuration beside it. A mod that was handed nothing and a mod that was handed
//! its author's default are the same value on the way out, so each mod records what
//! reached its own `configure`, and every test states a value no default could be.
//!
//! Two configuring mods in every composition, because one is what hid the defect
//! this file was written for: the launch read one file for the whole set and
//! checked its `owner`, so a second configuring mod was handed the first's document
//! and refused the launch. One mod could never see it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use ravel_config::{ConfigurationDeclaration, LayeredConfiguration, ValueDocument, file};
use ravel_core::{LifecycleState, ModId, Side};
use ravel_data::ContentDeclaration;
use ravel_host_api::{HostError, HostMod};
use ravel_host_runtime::{Member, compose, environment};
use ravel_pack::{ClosureError, PackDeclaration, ValidatedPack, validate};
use ravel_package::PackageManifest;

/// The side every composition here runs on.
///
/// A dedicated server, because both members load there and nothing in this file is
/// about a side.
const SIDE: Side = Side::DedicatedServer;

/// The two configuring mods every test composes.
const MEMBERS: [&str; 2] = ["first_mod", "second_mod"];

/// The setting each member declares, and the only key any pack here states.
const LIMIT: &str = "limit";

fn id(text: &str) -> ModId {
    text.parse().expect("a legal mod id")
}

fn directory(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "ravel-pack-round-trip-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("a writable temporary directory");
    directory
}

/// One member's package document, compatible with the versions this build is.
///
/// Read from [`environment`] rather than spelled, for the reason
/// `ravel-host-runtime`'s own suite gives: a literal would stop admitting this set
/// the day either version moved.
fn package(owner: &str) -> String {
    let running = environment();
    let zero = "0".repeat(64);
    format!(
        r#"{{
          "schemaVersion": 1,
          "id": "{owner}",
          "version": "1.0.0",
          "compatibility": {{ "ravel": ">={ravel}", "minecraft": ["{minecraft}"] }},
          "sides": ["client", "integrated-server", "dedicated-server"],
          "authors": [{{ "name": "Ravel" }}],
          "license": "MIT",
          "source": {{
            "repository": "https://example.invalid/{owner}",
            "files": [{{ "path": "src/lib.rs", "sha256": "{zero}" }}]
          }},
          "provenance": {{ "record": "provenance.json", "sha256": "{zero}" }}
        }}"#,
        ravel = running.ravel,
        minecraft = running.minecraft,
    )
}

/// One member's configuration declaration: a single integer a pack may state.
fn declaration(owner: &str) -> ConfigurationDeclaration {
    ConfigurationDeclaration::parse(&format!(
        r#"{{
          "schemaVersion": 1,
          "owner": "{owner}",
          "version": 1,
          "settings": [
            {{
              "key": "{LIMIT}",
              "valueSchema": {{ "type": "integer" }},
              "default": 4,
              "side": "common",
              "restartRequired": false,
              "packOverride": "allowed"
            }}
          ]
        }}"#
    ))
    .expect("a legal configuration declaration")
}

/// One pack override document stating `limit` for `owner`.
fn document(owner: &str, limit: i64) -> ValueDocument {
    ValueDocument::parse(&format!(
        r#"{{ "schemaVersion": 1, "owner": "{owner}", "values": {{ "{LIMIT}": {limit} }} }}"#
    ))
    .expect("a legal value document")
}

/// A mod that records the value its own `configure` was handed.
///
/// The record is shared rather than read back off the value, because a composition
/// hands its mods out as `Box<dyn HostMod>` and there is nothing to downcast to.
struct Settled {
    id: ModId,
    applied: Applied,
}

impl HostMod for Settled {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
        Ok(())
    }

    fn configuration_declaration(&self) -> Option<ConfigurationDeclaration> {
        Some(declaration(self.id.as_str()))
    }

    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), HostError> {
        *self.applied.lock().unwrap_or_else(PoisonError::into_inner) = resolved
            .value(&LIMIT.parse().expect("a legal setting key"))
            .and_then(serde_json::Value::as_i64);
        Ok(())
    }
}

/// What one mod's `configure` was handed, readable from outside the composition.
type Applied = Arc<Mutex<Option<i64>>>;

/// The members [`compose`] is handed, beside the record each one writes into.
fn members() -> (Vec<Member>, Vec<Applied>) {
    let mut composed = Vec::new();
    let mut applied = Vec::new();
    for owner in MEMBERS {
        let record = Arc::new(Mutex::new(None));
        composed.push(Member {
            package: package(owner),
            host: Box::new(Settled {
                id: id(owner),
                applied: Arc::clone(&record),
            }) as Box<dyn HostMod>,
        });
        applied.push(record);
    }
    (composed, applied)
}

fn recorded(applied: &[Applied]) -> Vec<Option<i64>> {
    applied
        .iter()
        .map(|record| *record.lock().unwrap_or_else(PoisonError::into_inner))
        .collect()
}

/// A pack pinning both members, stating `limits` for the members it names.
fn pack(limits: &BTreeMap<&str, i64>) -> ValidatedPack {
    let mut declared = PackDeclaration::new(id("example_pack"), ravel_core::Version::new(0, 1, 0));
    for owner in MEMBERS {
        declared
            .pin(id(owner), ravel_core::Version::new(1, 0, 0))
            .expect("a legal pin");
    }
    for (owner, limit) in limits {
        declared
            .configure(id(owner), document(owner, *limit))
            .expect("a document that names its own mod");
    }
    validate(
        &declared,
        &manifests(),
        &MEMBERS.map(|owner| ContentDeclaration::new(id(owner))),
        &MEMBERS.map(declaration),
    )
    .expect("every closure holds")
}

fn manifests() -> Vec<PackageManifest> {
    MEMBERS
        .iter()
        .map(|owner| PackageManifest::parse(&package(owner)).expect("a legal manifest"))
        .collect()
}

/// Composes `game` and returns what reached each mod, in [`MEMBERS`] order.
fn launch(game: &Path) -> Vec<Option<i64>> {
    let (composed, applied) = members();
    let (machine, _, configurations) =
        compose(game, SIDE, composed).expect("the composition should start");

    for owner in MEMBERS {
        assert_eq!(
            machine.state(&id(owner)),
            Ok(LifecycleState::Running),
            "`{owner}` should be running"
        );
    }
    assert_eq!(
        configurations.len(),
        MEMBERS.len(),
        "every configuring mod's resolved settings are handed back"
    );
    recorded(&applied)
}

#[test]
fn the_settings_layer_a_pack_writes_is_the_one_a_launch_reads_back() {
    let game = directory("round-trip");
    let limits = BTreeMap::from([("first_mod", 9), ("second_mod", 11)]);
    pack(&limits).write(&game).expect("the pack should write");

    assert_eq!(
        launch(&game),
        [Some(9), Some(11)],
        "each mod was handed the value the pack wrote for it, and not the other's"
    );

    // The mutation the round trip is only worth anything with: change the byte on
    // disk and the value inside the mod changes with it. Without this the test
    // could not tell a file it read from a declaration it never left.
    let written = file::pack_path(&game, &id("second_mod"));
    std::fs::write(&written, document("second_mod", 12).to_json())
        .expect("the written layer should be editable");

    assert_eq!(launch(&game), [Some(9), Some(12)]);
}

#[test]
fn a_member_the_pack_states_nothing_for_gets_its_own_default_rather_than_the_others_document() {
    // The refusal this file exists for. A launch that read one file for the whole
    // set found `first_mod`'s document, checked its `owner` against `second_mod`,
    // and stopped the game — so a pack could configure at most one mod, and doing
    // it at all broke every other configuring mod in the instance.
    let game = directory("one-of-two");
    let limits = BTreeMap::from([("first_mod", 9)]);
    pack(&limits).write(&game).expect("the pack should write");

    assert_eq!(launch(&game), [Some(9), Some(4)]);
}

#[test]
fn a_pack_that_states_nothing_writes_no_layer_and_every_member_keeps_its_default() {
    let game = directory("no-layer");
    pack(&BTreeMap::new())
        .write(&game)
        .expect("the pack should write");

    assert!(
        !game.join(file::PACK_DIRECTORY).exists(),
        "a pack with no overrides writes no settings directory"
    );
    assert_eq!(launch(&game), [Some(4), Some(4)]);
}

#[test]
fn a_settings_key_no_member_declares_is_refused_when_the_pack_is_built() {
    // The build-time half. The same misspelling reaches a launch as a refusal, so
    // what this asserts is *who* is told: the author, before the pack is written.
    let mut declared = PackDeclaration::new(id("example_pack"), ravel_core::Version::new(0, 1, 0));
    for owner in MEMBERS {
        declared
            .pin(id(owner), ravel_core::Version::new(1, 0, 0))
            .expect("a legal pin");
    }
    let typo = ValueDocument::parse(
        r#"{ "schemaVersion": 1, "owner": "first_mod", "values": { "limt": 9 } }"#,
    )
    .expect("a legal value document");
    declared
        .configure(id("first_mod"), typo)
        .expect("a document that names its own mod");

    let refused = validate(
        &declared,
        &manifests(),
        &MEMBERS.map(|owner| ContentDeclaration::new(id(owner))),
        &MEMBERS.map(declaration),
    )
    .expect_err("`limt` is not a setting `first_mod` declares");

    assert!(
        matches!(refused, ClosureError::Settings { ref owner, .. } if owner == &id("first_mod")),
        "{refused}"
    );
    let text = refused.to_string();
    assert!(
        text.contains("first_mod") && text.contains("limt"),
        "{text}"
    );

    // The positive control: the same pack with the key spelled right builds, so the
    // refusal above is about the key and not about the shape of the test.
    pack(&BTreeMap::from([("first_mod", 9)]));
}
