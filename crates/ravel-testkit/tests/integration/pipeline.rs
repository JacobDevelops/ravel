//! End-to-end pipeline ordering and failure boundaries over packages written to disk.

use std::fs;
use std::path::{Path, PathBuf};

use ravel_core::{LifecycleState, ModId, Side};
use ravel_host_api::{Mod, ModError};
use ravel_loader::LifecycleError;
use ravel_package::{CompatibilityError, IntegrityError, ManifestError, Sha256Digest};
use ravel_resolve::ResolutionError;
use ravel_testkit::{Host, PipelineError, TestPackage, executed_states, run_order, run_pipeline};
use serde_json::{Value, json};

const RUNNING_RAVEL: &str = "0.1.0";
const RUNNING_MINECRAFT: &str = "26.0.0";

/// A mod that records what it was asked to do, and optionally refuses one step.
struct Recording {
    id: ModId,
    entered: Vec<LifecycleState>,
    refuses: Option<LifecycleState>,
    requires_service: bool,
}

impl Recording {
    fn new(id: &str) -> Self {
        Self {
            id: id.parse().expect("a valid mod id"),
            entered: Vec::new(),
            refuses: None,
            requires_service: false,
        }
    }

    fn refusing(id: &str, state: LifecycleState) -> Self {
        Self {
            refuses: Some(state),
            ..Self::new(id)
        }
    }
}

impl Mod for Recording {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        if self.refuses == Some(state) {
            return Err(ModError::refused("this mod was built to decline"));
        }
        self.entered.push(state);
        Ok(())
    }

    fn service_declaration(&self) -> Result<Option<ravel_host_api::ServiceDeclaration>, ModError> {
        if !self.requires_service {
            return Ok(None);
        }
        let required =
            ravel_host_api::ServiceId::new("absent:prices".parse().expect("a legal service id"));
        Ok(Some(
            ravel_host_api::ServiceDeclaration::new(self.id.clone(), [], [required])
                .expect("a legal requirement"),
        ))
    }
}

fn id(name: &str) -> ModId {
    name.parse().expect("a valid mod id")
}

fn host() -> Host {
    Host {
        ravel: RUNNING_RAVEL.parse().expect("a valid version"),
        minecraft: RUNNING_MINECRAFT.to_owned(),
    }
}

fn digest_of(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let mut digest = [0_u8; 32];
    digest.copy_from_slice(&Sha256::digest(bytes));
    Sha256Digest::from_bytes(digest).to_string()
}

fn scratch(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("ravel-testkit-{label}-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("a stale directory should be removable");
    }
    fs::create_dir_all(&root).expect("the directory should be creatable");
    root
}

fn write_package(parent: &Path, name: &str, dependencies: &Value) -> TestPackage {
    let root = parent.join(name);
    let source = format!("pub fn {name}() {{}}\n").into_bytes();
    let record = format!("{{\"package\":\"{name}\"}}").into_bytes();
    fs::create_dir_all(root.join("src")).expect("the package should be creatable");
    fs::write(root.join("src/lib.rs"), &source).expect("source written");
    fs::write(root.join("provenance.json"), &record).expect("record written");

    let document = json!({
        "schemaVersion": 1,
        "id": name,
        "version": "1.0.0",
        "compatibility": { "ravel": ">=0.1.0, <1.0.0", "minecraft": [RUNNING_MINECRAFT] },
        "sides": ["client"],
        "dependencies": dependencies,
        "source": {
            "repository": "https://example.invalid/package",
            "files": [{ "path": "src/lib.rs", "sha256": digest_of(&source) }]
        },
        "authors": [{ "name": "Example" }],
        "license": "MIT",
        "provenance": { "record": "provenance.json", "sha256": digest_of(&record) }
    })
    .to_string();
    fs::write(root.join(TestPackage::MANIFEST_FILE), &document).expect("manifest written");

    TestPackage::new(root, document)
}

fn single(label: &str) -> (PathBuf, Vec<TestPackage>) {
    let parent = scratch(label);
    let packages = vec![write_package(&parent, "example_mod", &json!([]))];
    (parent, packages)
}

fn pair(label: &str) -> (PathBuf, Vec<TestPackage>) {
    let parent = scratch(label);
    let packages = vec![
        write_package(
            &parent,
            "example_mod",
            &json!([{ "id": "example_library", "relationship": "required", "version": "^1.0.0" }]),
        ),
        write_package(&parent, "example_library", &json!([])),
    ];
    (parent, packages)
}

#[test]
fn a_valid_package_reaches_every_spec_state_in_order() {
    let (parent, packages) = single("ordered");
    let mut noop = Recording::new("example_mod");

    let completed = run_pipeline(&packages, &host(), Side::Client, &mut [&mut noop])
        .expect("a valid package completes the pipeline");

    assert_eq!(completed.plan().side(), Side::Client);
    assert_eq!(completed.plan().order(), [id("example_mod")]);
    assert_eq!(completed.plan().discouraged(), []);
    assert_eq!(
        completed.states_of(&id("example_mod")),
        [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
            LifecycleState::Running,
            LifecycleState::Stopping,
            LifecycleState::Stopped,
        ],
        "the run covers production startup and shutdown"
    );
    assert_eq!(completed.states_of(&id("other_mod")), []);

    fs::remove_dir_all(&parent).expect("cleanup");
}

#[test]
fn mod_code_runs_only_in_the_states_the_loader_admits_it() {
    let (parent, packages) = single("executed");
    let mut noop = Recording::new("example_mod");

    run_pipeline(&packages, &host(), Side::Client, &mut [&mut noop])
        .expect("a valid package completes the pipeline");

    assert_eq!(noop.entered, executed_states());
    assert_eq!(
        noop.entered,
        [
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
            LifecycleState::Running,
            LifecycleState::Stopping,
        ],
        "no mod code runs before validation and resolution"
    );

    fs::remove_dir_all(&parent).expect("cleanup");
}

#[test]
fn the_run_order_is_the_spec_states_a_successful_run_passes_through() {
    assert_eq!(
        run_order(),
        [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
            LifecycleState::Running,
            LifecycleState::Stopping,
            LifecycleState::Stopped,
        ]
    );
    assert!(!run_order().contains(&LifecycleState::Discovered));
    assert!(!run_order().contains(&LifecycleState::Failed));
    assert!(
        executed_states()
            .iter()
            .all(|state| run_order().contains(state))
    );
}

#[test]
fn the_same_package_set_produces_the_same_transcript_however_it_was_discovered() {
    let (parent, packages) = pair("deterministic");
    let reversed: Vec<TestPackage> = packages.iter().rev().cloned().collect();

    let mut first_a = Recording::new("example_mod");
    let mut first_b = Recording::new("example_library");
    let first = run_pipeline(
        &packages,
        &host(),
        Side::Client,
        &mut [&mut first_a, &mut first_b],
    )
    .expect("the pair resolves");

    let mut second_a = Recording::new("example_library");
    let mut second_b = Recording::new("example_mod");
    let second = run_pipeline(
        &reversed,
        &host(),
        Side::Client,
        &mut [&mut second_a, &mut second_b],
    )
    .expect("the reversed pair resolves");

    assert_eq!(
        first.plan().order(),
        [id("example_library"), id("example_mod")],
        "the dependency loads first regardless of discovery order"
    );
    assert_eq!(first.plan(), second.plan());
    assert_eq!(first, second);

    fs::remove_dir_all(&parent).expect("cleanup");
}

#[test]
fn a_set_advances_together_so_no_mod_reaches_a_state_alone() {
    let (parent, packages) = pair("together");
    let mut library = Recording::new("example_library");
    let mut consumer = Recording::new("example_mod");

    let completed = run_pipeline(
        &packages,
        &host(),
        Side::Client,
        &mut [&mut library, &mut consumer],
    )
    .expect("the pair resolves");

    let states: Vec<LifecycleState> = completed
        .transcript()
        .iter()
        .map(|event| event.state)
        .collect();
    let expected: Vec<LifecycleState> = run_order()
        .into_iter()
        .flat_map(|state| [state, state])
        .collect();
    assert_eq!(
        states, expected,
        "each state is entered by both mods before the next"
    );
    assert_eq!(
        completed.states_of(&id("example_library")),
        completed.states_of(&id("example_mod"))
    );

    fs::remove_dir_all(&parent).expect("cleanup");
}

#[test]
fn a_document_that_is_not_a_manifest_stops_the_run_before_anything_else() {
    let parent = scratch("unparseable");
    let packages = vec![TestPackage::new(&parent, "not json at all")];
    let mut noop = Recording::new("example_mod");

    let error = run_pipeline(&packages, &host(), Side::Client, &mut [&mut noop])
        .expect_err("the document is not a manifest");

    assert!(
        matches!(error, PipelineError::Manifest(ManifestError::Syntax(_))),
        "{error}"
    );
    assert!(error.to_string().starts_with("validation failed"));
    assert!(noop.entered.is_empty(), "no mod code ran");

    fs::remove_dir_all(&parent).expect("cleanup");
}

#[test]
fn an_altered_source_file_stops_the_run_before_any_mod_code() {
    let (parent, packages) = single("tampered");
    fs::write(
        packages[0].root.join("src/lib.rs"),
        b"something else entirely",
    )
    .expect("source rewritten");
    let mut noop = Recording::new("example_mod");

    let error = run_pipeline(&packages, &host(), Side::Client, &mut [&mut noop])
        .expect_err("the source file was altered");

    assert!(
        matches!(
            error,
            PipelineError::Integrity(IntegrityError::Mismatch { .. })
        ),
        "{error}"
    );
    assert!(!error.to_string().contains('/'), "{error}");
    assert!(noop.entered.is_empty(), "no mod code ran");

    fs::remove_dir_all(&parent).expect("cleanup");
}

#[test]
fn a_package_written_for_another_host_is_refused_on_either_version() {
    let (parent, packages) = single("incompatible");

    for (ravel, minecraft, expected) in [
        ("9.9.9", RUNNING_MINECRAFT, "Ravel"),
        (RUNNING_RAVEL, "25.0.0", "Minecraft"),
    ] {
        let elsewhere = Host {
            ravel: ravel.parse().expect("a valid version"),
            minecraft: minecraft.to_owned(),
        };
        let mut noop = Recording::new("example_mod");
        let error = run_pipeline(&packages, &elsewhere, Side::Client, &mut [&mut noop])
            .expect_err("the package does not support this host");

        assert!(
            matches!(
                (&error, expected),
                (
                    PipelineError::Compatibility(CompatibilityError::Ravel { .. }),
                    "Ravel"
                ) | (
                    PipelineError::Compatibility(CompatibilityError::Minecraft { .. }),
                    "Minecraft"
                )
            ),
            "{expected} mismatch reported as {error}"
        );
        assert!(noop.entered.is_empty(), "no mod code ran");
    }

    // Control: the unchanged package succeeds on its declared host.
    let mut noop = Recording::new("example_mod");
    run_pipeline(&packages, &host(), Side::Client, &mut [&mut noop])
        .expect("the running host is supported");

    fs::remove_dir_all(&parent).expect("cleanup");
}

#[test]
fn an_unresolvable_graph_stops_the_run_before_any_mod_code() {
    let parent = scratch("unresolvable");
    let packages = vec![write_package(
        &parent,
        "example_mod",
        &json!([{ "id": "example_library", "relationship": "required", "version": "^1.0.0" }]),
    )];
    let mut noop = Recording::new("example_mod");

    let error = run_pipeline(&packages, &host(), Side::Client, &mut [&mut noop])
        .expect_err("the required dependency is not installed");

    assert!(
        matches!(
            error,
            PipelineError::Resolution(ResolutionError::MissingDependency { .. })
        ),
        "{error}"
    );
    assert!(error.to_string().starts_with("resolution failed"));
    assert!(noop.entered.is_empty(), "no mod code ran");

    fs::remove_dir_all(&parent).expect("cleanup");
}

#[test]
fn a_mod_that_declines_a_step_stops_the_run_at_that_step() {
    let (parent, packages) = single("refusing");
    let mut noop = Recording::refusing("example_mod", LifecycleState::Registered);

    let error = run_pipeline(&packages, &host(), Side::Client, &mut [&mut noop])
        .expect_err("the mod declines to register");

    assert!(
        matches!(
            &error,
            PipelineError::Drive(ravel_loader::DriveError::Host { .. })
        ),
        "{error}"
    );
    assert!(error.to_string().starts_with("the run failed"));
    let PipelineError::Drive(ravel_loader::DriveError::Host {
        source,
        attribution,
    }) = &error
    else {
        panic!("the pipeline must retain the attributed driver failure");
    };
    assert_eq!(attribution.mod_id.as_ref(), Some(source.mod_id()));
    assert_eq!(attribution.state, source.state());
    assert_eq!(
        error
            .failure()
            .expect("structured driver failure")
            .lifecycle_state(),
        source.state()
    );
    assert_eq!(
        noop.entered,
        [LifecycleState::Loaded, LifecycleState::Constructed],
        "the mod ran up to the step it declined and no further"
    );

    fs::remove_dir_all(&parent).expect("cleanup");
}

#[test]
fn the_plan_cannot_reach_a_mod_the_caller_did_not_supply() {
    let (parent, packages) = single("missing");

    let error =
        run_pipeline(&packages, &host(), Side::Client, &mut []).expect_err("no mod was supplied");

    assert!(
        matches!(&error, PipelineError::MissingMod(missing) if missing.mod_id() == &id("example_mod")),
        "the paired set refuses the missing host before any lifecycle callbacks"
    );
    assert!(error.to_string().contains("example_mod"));
    assert!(error.failure().is_some(), "the loader owns this refusal");

    fs::remove_dir_all(&parent).expect("cleanup");
}

#[test]
fn a_mod_the_plan_never_ordered_is_refused_before_the_run_starts() {
    let (parent, packages) = single("unexpected");
    let mut planned = Recording::new("example_mod");
    let mut stranger = Recording::new("other_mod");

    let error = run_pipeline(
        &packages,
        &host(),
        Side::Client,
        &mut [&mut planned, &mut stranger],
    )
    .expect_err("the plan does not order `other_mod`");

    assert!(
        matches!(&error, PipelineError::UnexpectedMod { mod_id } if mod_id == &id("other_mod")),
        "{error:?}",
    );
    assert!(planned.entered.is_empty(), "nothing ran");

    fs::remove_dir_all(&parent).expect("cleanup");
}

#[test]
fn a_package_is_read_from_its_manifest_file_or_reported_by_name() {
    let (parent, packages) = single("read");

    let read = TestPackage::read(&packages[0].root).expect("the manifest is readable");
    assert_eq!(read, packages[0]);

    let error = TestPackage::read(parent.join("absent")).expect_err("there is no package");
    assert!(
        matches!(error, PipelineError::Unreadable { ref file, .. } if file == "package.json"),
        "{error}"
    );
    assert!(
        error
            .to_string()
            .starts_with("`package.json` could not be read")
    );
    assert!(!error.to_string().contains("absent"), "{error}");
    assert!(error.failure().is_none(), "no crate raised it but this one");

    fs::remove_dir_all(&parent).expect("cleanup");
}

fn missing_mod() -> PipelineError {
    let entry = ravel_loader::LifecycleEntry {
        mod_id: id("example_mod"),
        sides: ravel_core::SideSet::new([Side::Client]).expect("one side"),
        requirement: ravel_loader::Requirement::Required,
        saved_data_versions: std::collections::BTreeSet::new(),
    };
    let mut machine =
        ravel_loader::LifecycleMachine::new(Side::Client, vec![entry]).expect("a legal plan");
    let mut mods: [&mut dyn Mod; 0] = [];
    let Err(refused) = ravel_loader::LoadedMods::new(&mut machine, &mut mods) else {
        panic!("the active member has no host");
    };
    PipelineError::MissingMod(refused)
}

#[test]
fn every_forwarded_failure_names_its_stage_and_keeps_the_error_that_raised_it() {
    use std::error::Error;

    for (error, stage) in [
        (
            PipelineError::Manifest(ManifestError::Syntax("not a document".to_owned())),
            "validation failed: ",
        ),
        (
            PipelineError::Integrity(IntegrityError::Missing {
                file: "lib.rs".to_owned(),
            }),
            "validation failed: ",
        ),
        (
            PipelineError::Compatibility(CompatibilityError::Ravel {
                declared: ">=0.1.0, <1.0.0".parse().expect("a valid range"),
                running: "9.9.9".parse().expect("a valid version"),
            }),
            "validation failed: ",
        ),
        (
            PipelineError::Resolution(ResolutionError::DuplicateIdentity {
                id: id("example_mod"),
            }),
            "resolution failed: ",
        ),
        (
            PipelineError::Lifecycle(LifecycleError::UnknownMod {
                mod_id: id("example_mod"),
            }),
            "the run failed: ",
        ),
        (
            PipelineError::Drive(ravel_loader::DriveError::Lifecycle(
                LifecycleError::UnknownMod {
                    mod_id: id("example_mod"),
                },
            )),
            "the run failed: ",
        ),
        (missing_mod(), "the run failed: "),
    ] {
        let source = error
            .source()
            .unwrap_or_else(|| panic!("{error:?} forwards another crate's failure"));
        assert_eq!(error.to_string(), format!("{stage}{source}"));
    }

    let unreadable = PipelineError::Unreadable {
        file: "package.json".to_owned(),
        source: std::io::Error::from(std::io::ErrorKind::NotFound),
    };
    let source = unreadable
        .source()
        .expect("manifest IO preserves its source");
    assert_eq!(
        source
            .downcast_ref::<std::io::Error>()
            .expect("typed IO source")
            .kind(),
        std::io::ErrorKind::NotFound
    );
    let own = PipelineError::UnexpectedMod {
        mod_id: id("other_mod"),
    };
    assert!(
        own.source().is_none(),
        "this harness wrote that message: {own:?}"
    );
}

#[test]
fn no_refusal_this_harness_writes_names_a_path_or_a_directory() {
    for error in [
        PipelineError::Unreadable {
            file: "package.json".to_owned(),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        },
        missing_mod(),
        PipelineError::UnexpectedMod {
            mod_id: id("example_mod"),
        },
    ] {
        let rendered = error.to_string();
        assert!(!rendered.contains('/'), "{rendered}");
        assert!(!rendered.contains("tmp"), "{rendered}");
    }
}

#[test]
fn the_harness_uses_the_production_service_barrier_before_running() {
    let (parent, packages) = single("missing-service");
    let mut consumer = Recording::new("example_mod");
    consumer.requires_service = true;

    let error = run_pipeline(&packages, &host(), Side::Client, &mut [&mut consumer])
        .expect_err("the required service provider is absent");

    assert!(
        matches!(
            &error,
            PipelineError::Drive(ravel_loader::DriveError::Services(_))
        ),
        "{error}"
    );
    assert_eq!(
        consumer.entered,
        [
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
        ],
        "service admission must refuse before any Running callback"
    );
    fs::remove_dir_all(parent).expect("cleanup");
}
