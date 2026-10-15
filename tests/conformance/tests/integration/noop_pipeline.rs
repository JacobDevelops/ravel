//! No-op package lifecycle, schema conformance, and reproducible pipeline evidence.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ravel_core::{LifecycleState, Side};
use ravel_host_api::HostMod;
use ravel_package::{NonEmptyText, PackageManifest};
use ravel_test_mod_noop::NoopMod;
use ravel_testkit::{DiscoveredPackage, Host, PipelineRun, executed_states, run, run_order};
use serde_json::{Value, json};

// Workspace crates share the loader's running version.
const RUNNING_RAVEL: &str = env!("CARGO_PKG_VERSION");

const PACKAGE_DIRECTORY: &str = "tests/fixtures/public/packages/noop";
const EVIDENCE_FILE: &str = "evidence/m2/noop-pipeline.json";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two directories below the repository root")
        .to_path_buf()
}

fn package() -> DiscoveredPackage {
    DiscoveredPackage::read(repository_root().join(PACKAGE_DIRECTORY))
        .expect("the no-op package should be readable")
}

fn manifest() -> PackageManifest {
    PackageManifest::parse(&package().document).expect("the no-op manifest should parse")
}

// Require the package's declared game version to have pinned repository data.
fn running_minecraft() -> String {
    let manifest = manifest();
    let declared = manifest
        .compatibility
        .minecraft
        .first()
        .expect("the schema requires at least one Minecraft version")
        .as_str()
        .to_owned();
    assert!(
        repository_root()
            .join("minecraft/versions")
            .join(&declared)
            .is_dir(),
        "the no-op package declares a Minecraft version that `minecraft/versions/` does not pin"
    );
    declared
}

fn host() -> Host {
    Host {
        ravel: RUNNING_RAVEL.parse().expect("the crate version is semver"),
        minecraft: running_minecraft(),
    }
}

fn run_on(side: Side) -> (PipelineRun, NoopMod) {
    let mut noop = NoopMod::new();
    let completed = run(
        &[package()],
        &host(),
        side,
        &mut [&mut noop as &mut dyn HostMod],
    )
    .expect("the no-op package should complete the pipeline");
    (completed, noop)
}

fn names(states: &[LifecycleState]) -> Vec<&'static str> {
    states.iter().map(|state| state.name()).collect()
}

fn record() -> Value {
    let manifest = manifest();
    let source_files: Vec<Value> = manifest
        .source
        .files
        .iter()
        .map(|source| {
            json!({
                "path": source.path.as_str(),
                "sha256": source.sha256.to_string(),
            })
        })
        .collect();

    let runs: Vec<Value> = Side::ALL
        .into_iter()
        .map(|side| {
            let (completed, noop) = run_on(side);
            let repeated = run_on(side);
            json!({
                "side": side,
                "loadOrder": completed
                    .plan()
                    .order()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                "transcript": completed
                    .transcript()
                    .iter()
                    .map(|event| json!({
                        "mod": event.mod_id.to_string(),
                        "state": event.state.name(),
                    }))
                    .collect::<Vec<_>>(),
                "modEntered": names(noop.entered()),
                "repeatedRunIsIdentical": completed == repeated.0
                    && noop.entered() == repeated.1.entered(),
            })
        })
        .collect();

    json!({
        "schemaVersion": 1,
        "record": "m2-w6-noop-pipeline",
        "milestone": "M2",
        "workPackage": "M2-W6",
        "summary": "The no-op package validated, resolved, and ran through the eight SPEC.md \
                    section 14 states a successful run reaches, in the same order on every side it \
                    declares. Of the ten states, discovered precedes this transcript and failed \
                    cannot appear in a run that succeeded.",
        "package": {
            "id": manifest.id.to_string(),
            "version": manifest.version.to_string(),
            "sides": manifest.sides,
            "compatibility": {
                "ravel": manifest.compatibility.ravel.to_string(),
                "minecraft": manifest
                    .compatibility
                    .minecraft
                    .iter()
                    .map(NonEmptyText::as_str)
                    .collect::<Vec<_>>(),
            },
            "sourceFiles": source_files,
            "provenance": {
                "file": manifest.provenance.record.file_name(),
                "sha256": manifest.provenance.sha256.to_string(),
            },
        },
        "mod": {
            "crate": "ravel-test-mod-noop",
            "executionModel": "A safe Rust value the loader owns and drives in process. ADG-2 \
                               selects portable source distribution; later compilation and binding \
                               work produces the value this M2 pipeline drives.",
        },
        "host": {
            "ravel": RUNNING_RAVEL,
            "minecraft": running_minecraft(),
        },
        "phases": names(&run_order()),
        "modExecutesIn": names(&executed_states()),
        "runs": runs,
    })
}

fn rendered(record: &Value) -> String {
    let mut text = serde_json::to_string_pretty(record).expect("the record should serialize");
    text.push('\n');
    text
}

fn validates(schema: &str, instance: &Path) -> bool {
    Command::new("check-jsonschema")
        .arg("--schemafile")
        .arg(repository_root().join(schema))
        .arg(instance)
        .output()
        .expect("check-jsonschema should be on PATH inside the devenv shell")
        .status
        .success()
}

#[test]
fn the_no_op_package_is_one_the_published_schemas_validate() {
    let root = repository_root().join(PACKAGE_DIRECTORY);

    assert!(
        validates(
            "schemas/package.schema.json",
            &root.join(DiscoveredPackage::MANIFEST_FILE)
        ),
        "the no-op manifest is not one `schemas/package.schema.json` accepts"
    );
    assert!(
        validates(
            "schemas/provenance.schema.json",
            &root.join(manifest().provenance.record.as_str())
        ),
        "the no-op provenance record is not one `schemas/provenance.schema.json` accepts"
    );
}

#[test]
fn the_no_op_package_reaches_every_state_a_successful_run_can_reach_in_order_on_every_side() {
    let expected = [
        LifecycleState::Validated,
        LifecycleState::Resolved,
        LifecycleState::Loaded,
        LifecycleState::Constructed,
        LifecycleState::Registered,
        LifecycleState::Running,
        LifecycleState::Stopping,
        LifecycleState::Stopped,
    ];

    for side in Side::ALL {
        let (completed, noop) = run_on(side);
        let mod_id = manifest().id;

        assert_eq!(completed.plan().side(), side);
        assert_eq!(completed.plan().order(), std::slice::from_ref(&mod_id));
        assert_eq!(completed.plan().discouraged(), []);
        assert_eq!(completed.states_of(&mod_id), expected, "{side:?}");
        assert_eq!(
            noop.entered(),
            executed_states(),
            "the mod ran in exactly the states the loader admits mod code"
        );
    }
}

#[test]
fn the_same_package_runs_the_same_way_every_time() {
    for side in Side::ALL {
        let (first, first_mod) = run_on(side);
        let (second, second_mod) = run_on(side);

        assert_eq!(first, second, "{side:?}");
        assert_eq!(first_mod.entered(), second_mod.entered(), "{side:?}");
    }
}

#[test]
fn the_recorded_evidence_is_what_the_loader_does_now() {
    let path = repository_root().join(EVIDENCE_FILE);
    if std::env::var_os("RAVEL_REGENERATE_EVIDENCE").is_some() {
        fs::write(&path, rendered(&record())).expect("the evidence should be writable");
    }
    let stored = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("`{EVIDENCE_FILE}` should be readable: {error}");
    });

    assert_eq!(
        stored,
        rendered(&record()),
        "`{EVIDENCE_FILE}` no longer matches a fresh run; rerun the pipeline and rewrite it"
    );
}

#[test]
fn the_recorded_evidence_names_no_path_and_no_secret() {
    let stored = fs::read_to_string(repository_root().join(EVIDENCE_FILE))
        .expect("the record should be readable");
    let lowered = stored.to_lowercase();

    for forbidden in [
        "/home/",
        "/users/",
        "c:\\users\\",
        "/.minecraft",
        "tests/fixtures/local/",
        "evidence/private/",
        "token",
        "password",
        "secret",
        "credential",
        "authorization",
    ] {
        assert!(
            !lowered.contains(forbidden),
            "the record contains `{forbidden}`"
        );
    }
    assert!(
        !stored.contains(&repository_root().display().to_string()),
        "the record names the directory it was generated in"
    );
}
