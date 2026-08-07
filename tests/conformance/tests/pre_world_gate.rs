//! The pre-world gate: what cannot open a world, and what a player is told
//! instead.
//!
//! `ROADMAP.md`'s M2-W7 row asks for a pre-world UI/log diagnostic path where
//! invalid packages, sides, integrity and graphs cannot open a world and the
//! reports redact secrets and paths. Every case below is a package set written
//! to disk and driven through the real pipeline, so what is tested is the
//! loader refusing rather than a harness deciding to.
//!
//! Each refusal is then rendered through `ravel_observability::PreWorldReport`,
//! which is both the report and the gate: it holds the redacted attribution a
//! surface renders, and it withholds the world while it holds anything at all.
//! The run is written to `evidence/m2/pre-world-gate.json` and this test
//! regenerates it and refuses any difference, so the record cannot describe a
//! gate that no longer behaves that way.
//!
//! These tests live here rather than in `crates/ravel-observability/tests/`
//! because they compose every crate of the pipeline and write the repository's
//! evidence directory, which is what this workspace member exists for.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use ravel_core::{FailureEnvironment, FailureKind, LifecycleState, ModId, Side, StructuredFailure};
use ravel_host_api::{HostError, HostMod};
use ravel_observability::{PreWorldReport, Subject, redact};
use ravel_package::Sha256Digest;
use ravel_testkit::{DiscoveredPackage, Host, PipelineError, run};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const RUNNING_RAVEL: &str = "0.1.0";
const RUNNING_MINECRAFT: &str = "26.0.0";
const RUNNING_PLATFORM: &str = "linux";
const RUNNING_ARCHITECTURE: &str = "x86_64";

const SOURCE_BYTES: &[u8] = b"pub fn load() {}\n";
const RECORD_BYTES: &[u8] = b"{\"origin\":\"example\"}";

const EVIDENCE_FILE: &str = "evidence/m2/pre-world-gate.json";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two directories below the repository root")
        .to_path_buf()
}

fn host() -> Host {
    Host {
        ravel: RUNNING_RAVEL.parse().expect("a valid version"),
        minecraft: RUNNING_MINECRAFT.to_owned(),
    }
}

fn environment() -> FailureEnvironment {
    FailureEnvironment {
        ravel: RUNNING_RAVEL.parse().expect("a valid version"),
        minecraft: RUNNING_MINECRAFT.to_owned(),
        platform: RUNNING_PLATFORM.to_owned(),
        architecture: RUNNING_ARCHITECTURE.to_owned(),
    }
}

fn digest_of(bytes: &[u8]) -> String {
    let mut digest = [0_u8; 32];
    digest.copy_from_slice(&Sha256::digest(bytes));
    Sha256Digest::from_bytes(digest).to_string()
}

fn id(name: &str) -> ModId {
    name.parse().expect("a valid mod id")
}

/// A fresh directory the whole case is built inside.
///
/// The counter is what keeps two directories apart, not the label. Several
/// tests each build the whole set of cases, so the label repeats, and the
/// process id only separates them under a runner that gives every test its own
/// process. `cargo nextest` does; `cargo test` runs them as threads in one
/// process, where two tests would share a directory and delete each other's
/// fixtures partway through. That presents as the gate refusing the wrong
/// thing, which is a much more expensive thing to read than a harness artefact.
///
/// The process id stays because the counter restarts every run, and the
/// existence check stays because a case that panics leaks its directory.
fn workspace(label: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);

    let unique = NEXT.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "ravel-pre-world-{label}-{}-{unique}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("a stale workspace should be removable");
    }
    fs::create_dir_all(&root).expect("the workspace should be creatable");
    root
}

/// What one fixture package declares.
struct Declaration<'a> {
    id: &'a str,
    version: &'a str,
    sides: &'a str,
    dependencies: &'a str,
}

impl<'a> Declaration<'a> {
    /// A client-side package at 1.0.0 for the running host.
    fn client(id: &'a str) -> Self {
        Self {
            id,
            version: "1.0.0",
            sides: r#"["client"]"#,
            dependencies: "[]",
        }
    }

    fn declaring(mut self, dependencies: &'a str) -> Self {
        self.dependencies = dependencies;
        self
    }

    fn on(mut self, sides: &'a str) -> Self {
        self.sides = sides;
        self
    }

    fn document(&self, source_digest: &str) -> String {
        let Self {
            id,
            version,
            sides,
            dependencies,
        } = self;
        let record = digest_of(RECORD_BYTES);
        format!(
            r#"{{
              "schemaVersion": 1,
              "id": "{id}",
              "version": "{version}",
              "compatibility": {{ "ravel": "*", "minecraft": ["{RUNNING_MINECRAFT}"] }},
              "sides": {sides},
              "dependencies": {dependencies},
              "source": {{
                "repository": "https://example.invalid/{id}",
                "files": [{{ "path": "src/lib.rs", "sha256": "{source_digest}" }}]
              }},
              "authors": [{{ "name": "Example" }}],
              "license": "MIT",
              "provenance": {{ "record": "provenance.json", "sha256": "{record}" }}
            }}"#
        )
    }
}

/// Writes one package into `parent`, with the files its manifest declares.
fn write(parent: &Path, declaration: &Declaration<'_>) -> DiscoveredPackage {
    let root = parent.join(declaration.id);
    fs::create_dir_all(root.join("src")).expect("the package should be creatable");
    fs::write(root.join("src/lib.rs"), SOURCE_BYTES).expect("source written");
    fs::write(root.join("provenance.json"), RECORD_BYTES).expect("record written");
    DiscoveredPackage::new(root, declaration.document(&digest_of(SOURCE_BYTES)))
}

/// A mod that records nothing and refuses nothing, so any refusal is the
/// loader's.
struct Silent {
    id: ModId,
    entered: Vec<LifecycleState>,
}

impl Silent {
    fn new(name: &str) -> Self {
        Self {
            id: id(name),
            entered: Vec::new(),
        }
    }
}

impl HostMod for Silent {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, state: LifecycleState) -> Result<(), HostError> {
        self.entered.push(state);
        Ok(())
    }
}

/// One invalid input, the refusal it produced, and what the report made of it.
struct Refused {
    case: &'static str,
    what: &'static str,
    error: PipelineError,
    report: PreWorldReport,
    ran_any_mod_code: bool,
}

impl Refused {
    /// The failure the owning crate raised, which is what the report carries.
    ///
    /// `PipelineError` is test support and deliberately implements no reporting
    /// trait, so a report always names the crate that refused rather than the
    /// harness that noticed.
    fn stage(&self) -> &dyn StructuredFailure {
        self.error.stage().expect("a crate raised this refusal")
    }

    fn line(&self) -> String {
        self.report.refusals()[0].line()
    }

    fn record(&self) -> Value {
        let attribution = self.report.refusals()[0].attribution();
        json!({
            "case": self.case,
            "invalidInput": self.what,
            "failureKind": attribution.kind.map(FailureKind::name),
            "lifecycleState": attribution.state.name(),
            "report": self.line(),
            "quotesUntrustedText": self.report.refusals()[0].quotes_untrusted_text(),
            "worldAccess": self.report.world_access().is_ok(),
            "ranModCode": self.ran_any_mod_code,
        })
    }
}

/// Drives one package set through the pipeline and reports what stopped it.
fn refuse(
    case: &'static str,
    what: &'static str,
    parent: &Path,
    packages: &[DiscoveredPackage],
    side: Side,
    subject: &Subject,
) -> Refused {
    let mut mods: Vec<Silent> = packages
        .iter()
        .filter_map(|package| {
            serde_json::from_str::<Value>(&package.document)
                .ok()
                .and_then(|document| document["id"].as_str().map(Silent::new))
        })
        .collect();
    let mut driven: Vec<&mut dyn HostMod> = mods
        .iter_mut()
        .map(|host| host as &mut dyn HostMod)
        .collect();

    let error = run(packages, &host(), side, &mut driven)
        .err()
        .unwrap_or_else(|| panic!("`{case}` should not have completed the pipeline"));
    let ran_any_mod_code = mods.iter().any(|host| !host.entered.is_empty());

    let mut report = PreWorldReport::new(environment());
    report.refuse(subject, error.stage().expect("a crate raised this refusal"));
    fs::remove_dir_all(parent).expect("cleanup");

    Refused {
        case,
        what,
        error,
        report,
        ran_any_mod_code,
    }
}

/// Every invalid input M2-W7 has to refuse, in a fixed order.
fn every_refusal() -> Vec<Refused> {
    vec![
        unparsable_manifest(),
        tampered_source_file(),
        side_mismatch(),
        missing_required_dependency(),
        incompatible_pair(),
        dependency_cycle(),
    ]
}

fn unparsable_manifest() -> Refused {
    let parent = workspace("unparsable");
    let root = parent.join("example_mod");
    fs::create_dir_all(&root).expect("the package should be creatable");
    let packages = [DiscoveredPackage::new(root, "{ this is not a manifest")];

    refuse(
        "unparsable-manifest",
        "a manifest that is not JSON",
        &parent,
        &packages,
        Side::Client,
        &Subject::unidentified(),
    )
}

fn tampered_source_file() -> Refused {
    let parent = workspace("tampered");
    let packages = [write(&parent, &Declaration::client("example_mod"))];
    fs::write(
        parent.join("example_mod/src/lib.rs"),
        b"someone else's bytes",
    )
    .expect("source rewritten");

    refuse(
        "digest-mismatch",
        "a source file whose bytes are not the bytes the manifest records",
        &parent,
        &packages,
        Side::Client,
        &Subject::of(id("example_mod")),
    )
}

fn side_mismatch() -> Refused {
    let parent = workspace("side-mismatch");
    let packages = [
        write(
            &parent,
            &Declaration::client("example_mod")
                .on(r#"["client", "dedicated-server"]"#)
                .declaring(
                    r#"[{ "id": "example_library", "relationship": "required", "version": "*" }]"#,
                ),
        ),
        write(&parent, &Declaration::client("example_library")),
    ];

    refuse(
        "side-mismatch",
        "a required dependency that is installed but declares another side",
        &parent,
        &packages,
        Side::DedicatedServer,
        &Subject::of(id("example_mod")),
    )
}

fn missing_required_dependency() -> Refused {
    let parent = workspace("missing-dependency");
    let packages = [write(
        &parent,
        &Declaration::client("example_mod").declaring(
            r#"[{ "id": "example_library", "relationship": "required", "version": "^1.0.0" }]"#,
        ),
    )];

    refuse(
        "missing-required-dependency",
        "a required dependency that is not installed",
        &parent,
        &packages,
        Side::Client,
        &Subject::of(id("example_mod")),
    )
}

fn incompatible_pair() -> Refused {
    let parent = workspace("incompatible");
    let packages = [
        write(
            &parent,
            &Declaration::client("example_mod").declaring(
                r#"[{ "id": "example_rival", "relationship": "incompatible", "version": "*" }]"#,
            ),
        ),
        write(&parent, &Declaration::client("example_rival")),
    ];

    refuse(
        "incompatible-pair",
        "two mods that declare each other incompatible and are both loading",
        &parent,
        &packages,
        Side::Client,
        &Subject::of(id("example_mod")),
    )
}

fn dependency_cycle() -> Refused {
    let parent = workspace("cycle");
    let packages = [
        write(
            &parent,
            &Declaration::client("example_mod").declaring(
                r#"[{ "id": "example_library", "relationship": "required", "version": "*" }]"#,
            ),
        ),
        write(
            &parent,
            &Declaration::client("example_library").declaring(
                r#"[{ "id": "example_mod", "relationship": "required", "version": "*" }]"#,
            ),
        ),
    ];

    refuse(
        "dependency-cycle",
        "two mods that require each other",
        &parent,
        &packages,
        Side::Client,
        &Subject::of(id("example_mod")),
    )
}

/// What the policy takes out of the one message this project did not write.
///
/// `HostError::Refused` carries a sentence a mod wrote, and a mod may put an
/// absolute path or a credential in it. The raw text is built here and never
/// recorded; only what the policy left is.
fn redaction_demonstration() -> Value {
    let refused = HostError::Refused {
        mod_id: id("example_mod"),
        state: LifecycleState::Loaded,
        detail: "could not open /home/player/.minecraft/saves/level.dat using ghp_0123456789"
            .to_owned(),
    };

    let mut report = PreWorldReport::new(environment());
    report.refuse(&Subject::of(id("example_mod")), &refused);

    json!({
        "quotesUntrustedText": report.refusals()[0].quotes_untrusted_text(),
        "removed": [
            "an absolute filesystem path, reduced to the file it names",
            "an authentication value, replaced outright",
        ],
        "published": report.refusals()[0].attribution().message.text(),
    })
}

/// The record written to `evidence/m2/pre-world-gate.json`.
fn record() -> Value {
    let refusals = every_refusal();
    let mut granted = PreWorldReport::new(environment());
    granted
        .note("`example_mod` discourages `example_rival`, and `example_rival` 2.0.0 is installed");

    json!({
        "schemaVersion": 1,
        "record": "m2-w7-pre-world-gate",
        "milestone": "M2",
        "workPackage": "M2-W7",
        "summary": "Every invalid package, side, integrity and dependency-graph case was refused \
                    before a world could open, each reported as its own outcome, with every \
                    message published through the redaction policy.",
        "gate": {
            "rule": "A report holding any refusal withholds world access, including a refusal \
                     from a stage that ran before any lifecycle machine existed.",
            "refusesWorldAccess": refusals
                .iter()
                .all(|refused| refused.report.world_access().is_err()),
            "ranModCode": refusals
                .iter()
                .any(|refused| refused.ran_any_mod_code),
            "grantsWorldAccessWhenNothingIsRefused": granted.world_access().is_ok(),
            "noticesRecordedWithoutRefusing": granted.notices().len(),
        },
        "refusals": refusals.iter().map(Refused::record).collect::<Vec<_>>(),
        "redaction": redaction_demonstration(),
    })
}

fn rendered(record: &Value) -> String {
    let mut text = serde_json::to_string_pretty(record).expect("the record should serialize");
    text.push('\n');
    text
}

#[test]
fn no_invalid_package_set_can_open_a_world() {
    for refused in every_refusal() {
        assert!(
            refused.report.world_access().is_err(),
            "{} opened a world",
            refused.case
        );
        assert!(
            !refused.ran_any_mod_code,
            "{} ran mod code before it was refused",
            refused.case
        );
        assert_eq!(
            refused.report.refusals().len(),
            1,
            "{} reported more than the failure that stopped it",
            refused.case
        );
    }
}

#[test]
fn every_dependency_graph_outcome_is_reported_as_itself() {
    let by_case: Vec<(&str, Option<FailureKind>, LifecycleState)> = every_refusal()
        .iter()
        .map(|refused| {
            (
                refused.case,
                refused.stage().failure_kind(),
                refused.stage().lifecycle_state(),
            )
        })
        .collect();

    assert_eq!(
        by_case,
        [
            ("unparsable-manifest", None, LifecycleState::Discovered),
            ("digest-mismatch", None, LifecycleState::Discovered),
            (
                "side-mismatch",
                Some(FailureKind::SideMismatch),
                LifecycleState::Validated
            ),
            (
                "missing-required-dependency",
                Some(FailureKind::MissingRequiredDependency),
                LifecycleState::Validated
            ),
            (
                "incompatible-pair",
                Some(FailureKind::IncompatibleVersion),
                LifecycleState::Validated
            ),
            (
                "dependency-cycle",
                Some(FailureKind::DependencyCycle),
                LifecycleState::Validated
            ),
        ],
        "a graph outcome that borrows another's kind is one a player cannot act on"
    );
}

#[test]
fn no_two_refusals_read_the_same_way() {
    let lines: Vec<String> = every_refusal().iter().map(Refused::line).collect();
    let mut distinct = lines.clone();
    distinct.sort();
    distinct.dedup();

    assert_eq!(
        distinct.len(),
        lines.len(),
        "two refusals a player cannot tell apart are one report: {lines:#?}"
    );
}

#[test]
fn every_published_message_has_been_through_the_policy() {
    for refused in every_refusal() {
        let message = &refused.report.refusals()[0].attribution().message;
        assert!(
            message.is_publishable(),
            "{} published unredacted text",
            refused.case
        );
        assert_eq!(
            message.text(),
            redact(refused.stage().diagnostic().text()).text(),
            "{}",
            refused.case
        );
    }
}

#[test]
fn the_one_message_a_mod_wrote_loses_its_path_and_its_credential() {
    let demonstration = redaction_demonstration();
    let published = demonstration["published"]
        .as_str()
        .expect("the record holds text");

    assert!(published.contains("level.dat"), "{published}");
    assert!(!published.contains("/home/"), "{published}");
    assert!(!published.contains("ghp_"), "{published}");
    assert!(published.contains("[redacted]"), "{published}");
    assert_eq!(demonstration["quotesUntrustedText"], json!(true));
}

#[test]
fn the_recorded_evidence_is_what_the_gate_does_now() {
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
        "`{EVIDENCE_FILE}` no longer matches a fresh run; rerun the gate and rewrite it"
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
        "ghp_",
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
    assert!(
        !stored.contains(&std::env::temp_dir().display().to_string()),
        "the record names the directory the fixtures were built in"
    );
}
