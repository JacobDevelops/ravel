#![forbid(unsafe_code)]

//! Round-trip pack settings through disk and host composition.
//! Two configuring mods catch cross-owner document reuse; each records its received value.

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

const SIDE: Side = Side::DedicatedServer;

const MEMBERS: [&str; 2] = ["first_mod", "second_mod"];

const LIMIT: &str = "limit";

fn id(text: &str) -> ModId {
    text.parse().expect("a legal mod id")
}

fn directory(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "ravel-pack-round-trip-{}-{name}",
        std::process::id()
    ));
    if directory.exists() {
        std::fs::remove_dir_all(&directory).expect("a stale directory is removable");
    }
    std::fs::create_dir_all(&directory).expect("a writable temporary directory");
    directory
}

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

fn document(owner: &str, limit: i64) -> ValueDocument {
    ValueDocument::parse(&format!(
        r#"{{ "schemaVersion": 1, "owner": "{owner}", "values": {{ "{LIMIT}": {limit} }} }}"#
    ))
    .expect("a legal value document")
}

// Shared records let tests observe configure calls after mods become trait objects.
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

    fn configuration_declaration(&self) -> Result<Option<ConfigurationDeclaration>, HostError> {
        Ok(Some(declaration(self.id.as_str())))
    }

    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), HostError> {
        *self.applied.lock().unwrap_or_else(PoisonError::into_inner) = resolved
            .value(&LIMIT.parse().expect("a legal setting key"))
            .and_then(serde_json::Value::as_i64);
        Ok(())
    }
}

type Applied = Arc<Mutex<Option<i64>>>;

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
            }),
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

fn launch(game: &Path) -> Vec<Option<i64>> {
    let (composed, applied) = members();
    let (machine, _, configurations, _) =
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

    // Mutating disk distinguishes a real read from reuse of the original declaration.
    let written = file::pack_path(&game, &id("second_mod"));
    std::fs::write(&written, document("second_mod", 12).to_json())
        .expect("the written layer should be editable");

    assert_eq!(launch(&game), [Some(9), Some(12)]);
}

#[test]
fn a_member_the_pack_states_nothing_for_gets_its_own_default_rather_than_the_others_document() {
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

    pack(&BTreeMap::from([("first_mod", 9)]));
}
