#![forbid(unsafe_code)]

//! Repository automation for Ravel.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};

mod instance_scan;

pub use instance_scan::{Finding, InstanceScan, scan_instance};

const CRATE_NAMES: &[&str] = &[
    "ravel-core",
    "ravel-package",
    "ravel-resolve",
    "ravel-loader",
    "ravel-sdk",
    "ravel-sdk-macros",
    "ravel-host-api",
    "ravel-bootstrap",
    "ravel-composed-host",
    "ravel-minecraft",
    "ravel-minecraft-adapter",
    "ravel-config",
    "ravel-data",
    "ravel-save",
    "ravel-network",
    "ravel-observability",
    "ravel-pack",
    "ravel-testkit",
];

const TEST_MOD_NAMES: &[&str] = &[
    "commands",
    "configuration",
    "dependency-base",
    "dependency-consumer",
    "events",
    "failure-cases",
    "inter-mod-service",
    "networking",
    "noop",
    "registries",
    "resources",
    "saved-data",
];

const EXPERIMENT_NAMES: &[&str] = &[
    "adg-1-jvm-entry",
    "adg-2-guest",
    "adg-2-mod-execution",
    "adg-3-bindings",
    "adg-4-state",
    "adg-5-prism",
    "m2-loader-overhead",
    "m3-pack-compile",
    "m3-profiling-overhead",
    "spike-harness",
];

/// Gate directories under `evidence/m1/spikes` and the gate ID they must carry.
const SPIKE_GATES: &[(&str, &str)] = &[
    ("adg-1", "ADG-1"),
    ("adg-2", "ADG-2"),
    ("adg-3", "ADG-3"),
    ("adg-4", "ADG-4"),
    ("adg-5", "ADG-5"),
];

const ROOT_FILES: &[&str] = &[
    ".cargo/config.toml",
    ".config/nextest.toml",
    ".envrc",
    ".gitignore",
    ".markdownlint.yaml",
    ".tangled/workflows/ci.yml",
    "AGENTS.md",
    "CLAUDE.md",
    "Cargo.lock",
    "Cargo.toml",
    "LICENSE-APACHE",
    "LICENSE-MIT",
    "README.md",
    "ROADMAP.md",
    "SPEC.md",
    "deny.toml",
    "devenv.lock",
    "devenv.nix",
    "devenv.yaml",
    "docs/architecture/m1-measurement-gate.md",
    "docs/decisions/README.md",
    "evidence/m1/README.md",
    "evidence/m2/README.md",
    "docs/runbooks/darwin-verification.md",
    "experiments/README.md",
    "flake.lock",
    "flake.nix",
    "jvm/README.md",
    "jvm/bootstrap/dev/ravel/jvm/Agent.java",
    "jvm/bootstrap/dev/ravel/jvm/HookTarget.java",
    "jvm/bootstrap/dev/ravel/jvm/Screens.java",
    "jvm/bootstrap/dev/ravel/jvm/Transformer.java",
    "jvm/build/build-agent.sh",
    "jvm/build/build-screen.sh",
    "jvm/harness/Main.java",
    "jvm/harness/net/minecraft/util/StringUtil.java",
    "jvm/harness/run-harness.sh",
    "jvm/screen/src/dev/ravel/jvm/screen/RavelModsScreen.java",
    "jvm/screen/src/dev/ravel/jvm/screen/RavelSettingsScreen.java",
    "jvm/screen/src/dev/ravel/jvm/screen/Wire.java",
    "minecraft/README.md",
    "minecraft/versions/26.1.2/README.md",
    "minecraft/versions/26.1.2/adapter.toml",
    "minecraft/versions/26.1.2/hooks.toml",
    "minecraft/versions/26.1.2/mappings.lock",
    "minecraft/versions/26.1.2/stubs.lock",
    "minecraft/versions/26.1.2/stubs.toml",
    "nix/ci.nix",
    "nix/common.nix",
    "nix/darwin.nix",
    "nix/linux.nix",
    "rust-toolchain.toml",
    "rustfmt.toml",
    "schemas/configuration-override.schema.json",
    "schemas/configuration.schema.json",
    "schemas/evidence.schema.json",
    "schemas/fixture.schema.json",
    "schemas/pack.schema.json",
    "schemas/package.schema.json",
    "schemas/performance-collector.schema.json",
    "schemas/performance-result.schema.json",
    "schemas/provenance.schema.json",
    "schemas/runtime-inputs.schema.json",
    "schemas/saved-data.schema.json",
    "schemas/spike-result.schema.json",
    "tests/conformance/Cargo.toml",
    "tests/fixtures/README.md",
    "tests/fixtures/local/README.md",
    "tests/fixtures/public/README.md",
    "tools/ravel-cli/Cargo.toml",
    "tools/ravel-xtask/Cargo.toml",
];

/// Returns the files that define the minimum public repository scaffold.
#[must_use]
pub fn required_scaffold_paths() -> Vec<String> {
    ROOT_FILES
        .iter()
        .map(|path| (*path).to_owned())
        .chain(
            CRATE_NAMES
                .iter()
                .map(|name| format!("crates/{name}/Cargo.toml")),
        )
        .chain(
            TEST_MOD_NAMES
                .iter()
                .map(|name| format!("test-mods/{name}/Cargo.toml")),
        )
        .chain(
            EXPERIMENT_NAMES
                .iter()
                .map(|name| format!("experiments/{name}/Cargo.toml")),
        )
        .collect()
}

/// Checks the repository's initial public scaffold and tracked-file policy.
#[must_use]
pub fn verify_scaffold(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();

    for relative in required_scaffold_paths() {
        if !root.join(&relative).is_file() {
            violations.push(format!("missing required file: {relative}"));
        }
    }

    for relative in maintained_rust_roots() {
        match fs::read_to_string(root.join(&relative)) {
            Ok(source) if source.contains("#![forbid(unsafe_code)]") => {}
            Ok(_) => violations.push(format!("missing unsafe prohibition: {relative}")),
            Err(error) => violations.push(format!("cannot read {relative}: {error}")),
        }
    }

    verify_ignored_boundaries(root, &mut violations);
    verify_schema_documents(root, &mut violations);
    verify_version_boundary(root, &mut violations);
    verify_experiment_fence(root, &mut violations);
    violations.extend(verify_evidence_records(root));

    match tracked_paths(root) {
        Ok(paths) => {
            violations.extend(verify_tracked_paths(paths.iter().map(String::as_str)));
            violations.extend(verify_tracked_contents(root, &paths));
        }
        Err(error) => violations.push(error),
    }

    violations
}

/// Recomputes every digest a milestone manifest records for its own results.
///
/// The schema constrains the shape of `benchmarks.results[].sha256` and nothing
/// recomputed the value, so recorded digests drifted from the files they name
/// the moment a later change edited one of those files. A manifest whose
/// digests do not bind its own records is an index rather than evidence, and
/// the drift is invisible precisely because every field still validates.
///
/// `benchmarks.results[]` and every `platformEvidence[].baselines[]` digest that
/// names a tracked file are recomputed against the working copy, because a
/// milestone owns those files. `inputs[]` names files outside the evidence
/// directory that go on changing after the milestone closes, so it is
/// recomputed against the revision the manifest pins instead; hashing it here
/// would rewrite history rather than verify it.
#[must_use]
pub fn verify_evidence_records(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    let mut manifests = fs::read_dir(root.join("evidence"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path().join("manifest.json"))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    manifests.sort();

    for manifest in &manifests {
        verify_recorded_digests(root, manifest, &mut violations);
        verify_recorded_baselines(root, manifest, &mut violations);
        verify_recorded_inputs(root, manifest, &mut violations);
        verify_recorded_run(root, manifest, &mut violations);
    }

    violations
}

/// Checks the gate run a milestone manifest records against itself.
///
/// A manifest attests a run it cannot contain. The log is private, and the
/// revision the run was made on is below the tip by construction, because
/// recording a result changes the tree that produced it — so no check can ask a
/// manifest to name the state it sits in. What can be bound is that the numbers
/// agree with each other and with the gate they claim to have passed, and that a
/// manifest recording a run records its result at all. That the pinned revision
/// resolves is already bound by `verify_recorded_inputs`, which hashes every
/// input there and fails when it cannot.
///
/// The milestone gate applies the same arithmetic to the one milestone it is
/// closing. This applies it to every manifest on every scaffold run, because the
/// numbers a closed milestone records are exactly the ones nobody looks at again.
fn verify_recorded_run(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    let label = manifest_path
        .strip_prefix(root)
        .unwrap_or(manifest_path)
        .display()
        .to_string();
    let records_run = match jq_matches(
        manifest_path,
        r#".commands | type == "array" and length > 0"#,
    ) {
        Ok(records_run) => records_run,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let records_tests = match jq_matches(manifest_path, r#".tests | type == "object""#) {
        Ok(records_tests) => records_tests,
        Err(error) => {
            violations.push(error);
            return;
        }
    };

    if records_run && !records_tests {
        violations.push(format!("{label} records a gate run but no test result"));
    }
    if records_run {
        require_manifest_rule(
            manifest_path,
            "[.commands[].exitStatus] | all(. == 0)",
            &format!("{label} records a gate run that did not succeed"),
            violations,
        );
    }
    if !records_tests {
        return;
    }
    for (filter, violation) in [
        (
            ".tests.total == (.tests.passed + .tests.failed + .tests.skipped)",
            "records test counts that do not sum to its total",
        ),
        (
            ".tests.total > 0 and .tests.failed == 0 and .tests.skipped == 0",
            "records no passing test run",
        ),
        (
            ".tests.lineCoveragePercent >= 80",
            "records line coverage below the floor the gate enforces",
        ),
    ] {
        require_manifest_rule(
            manifest_path,
            filter,
            &format!("{label} {violation}"),
            violations,
        );
    }
}

/// Recomputes every input digest against the revision the manifest pins.
///
/// An input digest is a claim about one revision, so it has to be hashed there.
/// `revision.id` is that revision: it names the commit the recorded commands
/// ran on, which is what the milestone was measured against. `revision.changeId`
/// names only the change that carries the manifest, which is revised long after
/// the measurement — M3's was, and not one of its inputs hashes to its recorded
/// value there — so hashing at the change would check the wrong tree, and
/// hashing at `@` would check a tree the milestone never saw.
///
/// The revision is resolved through jj, and a checkout without one is skipped:
/// `revision.id` is frequently a commit no branch reaches, so a plain git tree
/// would report every input unreadable and turn a provenance check into noise.
/// The skip says so on stdout, because a whole provenance check that evaporates
/// in silence reads exactly like one that passed.
fn verify_recorded_inputs(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    let label = manifest_path
        .strip_prefix(root)
        .unwrap_or(manifest_path)
        .display()
        .to_string();
    if !root.join(".jj").exists() {
        println!(
            "- {label}: input digests are not recomputed, because this checkout has no jj \
             repository to resolve the pinned revision in"
        );
        return;
    }
    let revision = match jq_value(manifest_path, ".revision.id") {
        Ok(value) if value != "null" && !value.is_empty() => value,
        Ok(_) => {
            violations.push(format!(
                "{label} records inputs but no revision to hash them at"
            ));
            return;
        }
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let records = match jq_value(manifest_path, r#".inputs[]? | .name + " " + .sha256"#) {
        Ok(records) => records,
        Err(error) => {
            violations.push(error);
            return;
        }
    };

    for record in records.lines().filter(|record| !record.trim().is_empty()) {
        let Some((name, recorded)) = record.rsplit_once(' ') else {
            violations.push(format!("{label} records an input with no digest: {record}"));
            continue;
        };
        match revision_digest(root, &revision, name) {
            Ok(actual) if actual == recorded => {}
            Ok(actual) => violations.push(format!(
                "{label} records input {name} as {recorded} but revision {revision} holds {actual}"
            )),
            Err(error) => violations.push(format!("{label} records input {name}: {error}")),
        }
    }
}

/// Hashes a tracked file as it stood in one revision.
fn revision_digest(root: &Path, revision: &str, name: &str) -> Result<String, String> {
    let output = Command::new("jj")
        .args(["file", "show", "-r", revision, "--", name])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot read it at revision {revision}: {error}"))?;
    if !output.status.success() {
        return Err(format!("cannot read it at revision {revision}"));
    }
    Ok(hex_digest(&output.stdout))
}

/// Compares each recorded result digest against the file it names.
fn verify_recorded_digests(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    let label = manifest_path
        .strip_prefix(root)
        .unwrap_or(manifest_path)
        .display()
        .to_string();
    let records = match jq_value(
        manifest_path,
        r#".benchmarks.results[]? | .name + " " + .sha256"#,
    ) {
        Ok(records) => records,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let Some(directory) = manifest_path.parent() else {
        return;
    };

    for record in records.lines().filter(|record| !record.trim().is_empty()) {
        let Some((name, recorded)) = record.rsplit_once(' ') else {
            violations.push(format!("{label} records a result with no digest: {record}"));
            continue;
        };
        verify_tracked_digest(
            &TrackedDigest {
                label: &label,
                kind: "result",
                manifest_path,
                directory,
                name,
                recorded,
            },
            violations,
        );
    }
}

/// Recomputes every baseline digest a manifest records for a tracked file.
///
/// `benchmarks.results[]` is not the only place a manifest hashes its own
/// records. M0 records a launch summary and a launch validation for each
/// baseline workload, and two of those four digests name tracked evidence files
/// that nothing recomputed, which is the same rot in a second place: correct
/// today, and green on a stale value the moment a later change edits the record.
///
/// A validation digest names a private capture that never enters the repository,
/// so it cannot be recomputed here and is announced as unbound instead of being
/// silently passed over. A name that reads as a path is checked, because a typo
/// in one would otherwise buy the same silence.
fn verify_recorded_baselines(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    let label = manifest_path
        .strip_prefix(root)
        .unwrap_or(manifest_path)
        .display()
        .to_string();
    let records = match jq_value(
        manifest_path,
        r#".platformEvidence[]?.baselines[]? | (.result, .validation)
           | select(. != null) | .name + " " + .sha256"#,
    ) {
        Ok(records) => records,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let Some(directory) = manifest_path.parent() else {
        return;
    };

    for record in records.lines().filter(|record| !record.trim().is_empty()) {
        let Some((name, recorded)) = record.rsplit_once(' ') else {
            violations.push(format!(
                "{label} records a baseline with no digest: {record}"
            ));
            continue;
        };
        if !names_a_path(name) {
            println!(
                "- {label}: the baseline digest recorded for \"{name}\" is not recomputed, \
                 because it names material held outside the repository"
            );
            continue;
        }
        verify_tracked_digest(
            &TrackedDigest {
                label: &label,
                kind: "baseline",
                manifest_path,
                directory,
                name,
                recorded,
            },
            violations,
        );
    }
}

/// Whether a recorded name is a path in the repository rather than prose.
///
/// Every tracked evidence record is named by its file name; the private captures
/// are named by description, and no description is a bare file name.
fn names_a_path(name: &str) -> bool {
    !name.contains(char::is_whitespace) && Path::new(name).extension().is_some()
}

/// One recorded digest and where the manifest that records it says to find it.
struct TrackedDigest<'a> {
    label: &'a str,
    kind: &'a str,
    manifest_path: &'a Path,
    directory: &'a Path,
    name: &'a str,
    recorded: &'a str,
}

/// Compares one recorded digest against the file inside the milestone it names.
fn verify_tracked_digest(record: &TrackedDigest<'_>, violations: &mut Vec<String>) {
    let TrackedDigest {
        label,
        kind,
        manifest_path,
        directory,
        name,
        recorded,
    } = *record;
    if name.is_empty() || name.starts_with('/') || name.split('/').any(|part| part == "..") {
        violations.push(format!(
            "{label} records a {kind} outside its own milestone: {name}"
        ));
        return;
    }
    let path = directory.join(name);
    // A manifest cannot carry its own digest. Writing one changes the file
    // it describes, so a check that admitted the record would only ever
    // pass on a value nobody could compute, which is a check that verifies
    // itself rather than the evidence.
    if path == manifest_path {
        violations.push(format!("{label} records its own digest, which cannot bind"));
        return;
    }
    match file_digest(&path) {
        Ok(actual) if actual == recorded => {}
        Ok(actual) => violations.push(format!(
            "{label} records {name} as {recorded} but it hashes {actual}"
        )),
        Err(error) => violations.push(format!("{label} records {name}: {error}")),
    }
}

/// Hashes a tracked file as it stands in the working copy.
fn file_digest(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read the recorded file: {error}"))?;
    Ok(hex_digest(&bytes))
}

/// Renders the SHA-256 of some bytes the way a manifest records it.
fn hex_digest(bytes: &[u8]) -> String {
    let mut digest = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(digest, "{byte:02x}").expect("writing to a string cannot fail");
    }
    digest
}

/// Checks every recorded architecture spike result against the M1 gate rules.
///
/// An absent spike tree is not a violation; the M1 milestone gate is what
/// requires results to exist. This check only enforces that recorded results are
/// complete, comparable, and free of substituted values.
#[must_use]
pub fn verify_spikes(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    let spikes = root.join("evidence/m1/spikes");

    for (directory, gate) in SPIKE_GATES {
        let gate_root = spikes.join(directory);
        if !gate_root.is_dir() {
            continue;
        }

        let mut results = fs::read_dir(&gate_root)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect::<Vec<_>>();
        results.sort();

        if results.is_empty() {
            violations.push(format!("spike gate {gate} has no recorded result"));
            continue;
        }
        for result in &results {
            verify_spike_result(root, result, gate, &mut violations);
            verify_recorded_change(root, result, &mut violations);
        }
    }

    violations
}

/// Checks that a result names the change that introduced it.
///
/// `trial.revision.id` is a commit, which every jj rewrite abandons, so it cannot
/// be resolved and is not checked. The change id can be, and this is the only
/// reason to prefer it: seven results named the wrong revision for a week because
/// nothing recomputed the claim.
///
/// A checkout with no jj repository cannot resolve a change at all, so the check
/// is skipped there and says so on stdout. It used to return in silence, which is
/// indistinguishable from passing on exactly the machines where the repository's
/// strongest provenance control is absent.
fn verify_recorded_change(root: &Path, path: &Path, violations: &mut Vec<String>) {
    let label = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string();
    if !root.join(".jj").exists() {
        println!(
            "- {label}: the recorded change is not recomputed, because this checkout has no jj \
             repository to resolve it in"
        );
        return;
    }
    let recorded = match jq_value(path, ".trial.revision.changeId") {
        Ok(value) if value != "null" && !value.is_empty() => value,
        // A result with no recorded change is the failure this rule exists for,
        // so it cannot be the branch that returns quietly.
        Ok(_) => {
            violations.push(format!("{label} records no change id"));
            return;
        }
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    match introducing_change(root, &label) {
        Ok(owner) => violations.extend(change_mismatch(&label, &recorded, &owner)),
        Err(error) => violations.push(error),
    }
}

/// Resolves the change that introduced a tracked path.
///
/// A lookup that cannot run is an error, not a pass. This is the repository's
/// strongest provenance control, and a control that fails open when its tool is
/// missing verifies nothing on exactly the machines where it matters. Empty
/// output is not an error: an untracked result has no introducing change yet.
fn introducing_change(root: &Path, label: &str) -> Result<String, String> {
    let output = Command::new("jj")
        .args([
            "log",
            "-r",
            &format!("roots(::@ & files('{label}'))"),
            "--no-graph",
            "-T",
            "change_id",
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot resolve the change that introduced {label}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cannot resolve the change that introduced {label}: jj exited {}",
            output.status
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Checks that a milestone manifest names the change that introduced it.
///
/// The same rule the spike results carry, for the same reason: a commit id is
/// abandoned by every rewrite, so a manifest that records only one cannot be
/// resolved by the reader it is written for.
///
/// As with the spike results, a checkout with no jj repository is skipped and the
/// skip is announced rather than silent.
fn verify_manifest_change(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    let Ok(label) = manifest_path.strip_prefix(root) else {
        return;
    };
    let label = label.display().to_string();
    // Contract fixtures are synthetic by design and describe no real revision;
    // only admitted evidence carries a resolvable provenance claim.
    if !label.starts_with("evidence/") {
        return;
    }
    if !root.join(".jj").exists() {
        println!(
            "- {label}: the recorded change is not recomputed, because this checkout has no jj \
             repository to resolve it in"
        );
        return;
    }
    let owner = match introducing_change(root, &label) {
        Ok(owner) => owner,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    match jq_value(manifest_path, ".revision.changeId") {
        Ok(recorded) if recorded != "null" && !recorded.is_empty() => {
            violations.extend(change_mismatch(&label, &recorded, &owner));
        }
        Ok(_) => violations.push(format!("{label} records no change id")),
        Err(error) => violations.push(error),
    }
}

/// Compares a recorded change against the one that introduced the file.
///
/// An untracked result has no introducing change yet, which is not a violation:
/// the schema still constrains the field, and the next run has a change to check.
fn change_mismatch(label: &str, recorded: &str, owner: &str) -> Option<String> {
    if owner.is_empty() || owner == recorded {
        return None;
    }
    Some(format!(
        "{label} records change {recorded} but was introduced by {owner}"
    ))
}

fn verify_spike_result(root: &Path, path: &Path, gate: &str, violations: &mut Vec<String>) {
    let label = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string();

    match spike_schema_matches(root, path) {
        Ok(true) => {}
        Ok(false) => violations.push(format!("{label} does not conform to the spike schema")),
        Err(error) => violations.push(error),
    }

    let rules = [
        (
            ".schemaVersion == 1 and .protocolVersion == \"spike-v1\"",
            "must record spike protocol version 1",
        ),
        (
            &format!(".gate == \"{gate}\""),
            "must record the gate of the directory it belongs to",
        ),
        (
            r#".candidate.disposition == "rejected"
                or ([.failures[].scenario] | sort
                    == ["callback-failure", "crash", "invalid-input", "timeout",
                        "unavailable-runtime"])"#,
            "must exercise every required failure scenario exactly once",
        ),
        (
            // Recording the scenarios says nothing about what happened in them.
            // A candidate that never once failed closed never demonstrated the
            // property this gate exists to check.
            r#".candidate.disposition == "rejected"
                or ([.failures[] | select(.outcome == "failed-closed")] | length > 0)"#,
            "a measured candidate must record at least one failure that closed",
        ),
        (
            r#"[.memory.bootstrapPeakRss, .memory.retainedRss, .memory.retainedClasses,
                .startup.processToReady]
                | all((.status == "unavailable") or (.value > 0))"#,
            "must not report a measured bootstrap or startup value of zero",
        ),
        (
            r#".candidate.disposition == "rejected"
                or .calls.callbackNanoseconds.status == "measured"
                or (.calls.callbackNanoseconds.reason | length > 0)"#,
            "must measure callback cost or record why it is unavailable",
        ),
        (
            r#".deviations | all(.classification != "blocking")"#,
            "must not carry a blocking deviation",
        ),
        (
            r#".trial.measuredInvocations > 0 or .candidate.disposition == "rejected""#,
            "must measure at least one invocation",
        ),
    ];
    for (filter, requirement) in rules {
        match jq_matches(path, filter) {
            Ok(true) => {}
            Ok(false) => violations.push(format!("{label} {requirement}")),
            Err(error) => violations.push(error),
        }
    }

    verify_redaction(path, &label, violations);
}

fn spike_schema_matches(root: &Path, result: &Path) -> Result<bool, String> {
    let output = Command::new("check-jsonschema")
        .arg("--schemafile")
        .arg(root.join("schemas/spike-result.schema.json"))
        .arg(result)
        .output()
        .map_err(|error| format!("cannot validate spike schema: {error}"))?;
    Ok(output.status.success())
}

/// Keeps measurement-only spike crates out of the production dependency graph.
fn verify_experiment_fence(root: &Path, violations: &mut Vec<String>) {
    let production_manifests = CRATE_NAMES
        .iter()
        .map(|name| format!("crates/{name}/Cargo.toml"))
        .chain(
            TEST_MOD_NAMES
                .iter()
                .map(|name| format!("test-mods/{name}/Cargo.toml")),
        )
        .chain([
            "tests/conformance/Cargo.toml".to_owned(),
            "tools/ravel-cli/Cargo.toml".to_owned(),
            "tools/ravel-xtask/Cargo.toml".to_owned(),
        ]);

    for relative in production_manifests {
        let Ok(manifest) = fs::read_to_string(root.join(&relative)) else {
            continue;
        };
        if manifest.contains("ravel-spike") || manifest.contains("experiments/") {
            violations.push(format!(
                "production crate depends on an experiment: {relative}"
            ));
        }
        if manifest.contains(r#"unsafe_code = "allow""#) {
            violations.push(format!(
                "unsafe code is permitted outside the experiment boundary: {relative}"
            ));
        }
    }

    let mut sources = Vec::new();
    for directory in ["crates", "test-mods", "tools", "tests"] {
        collect_files(&root.join(directory), &mut sources);
    }
    for path in sources {
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        // A bare `contains` would match this rule's own literals, so anchor on
        // the attribute position instead of the substring.
        if fs::read_to_string(&path).is_ok_and(|source| {
            source.lines().map(str::trim_start).any(|line| {
                line.starts_with("#![allow(unsafe_code)")
                    || line.starts_with("#[allow(unsafe_code)")
                    || line.starts_with("#![expect(unsafe_code)")
                    || line.starts_with("#[expect(unsafe_code)")
            })
        }) {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            violations.push(format!(
                "unsafe code is permitted outside the experiment boundary: {}",
                relative.display()
            ));
        }
    }
}

/// Checks a milestone evidence manifest's identity and minimum gate fields.
#[must_use]
pub fn verify_milestone(root: &Path, milestone: &str) -> Vec<String> {
    if !is_valid_milestone_id(milestone) {
        return vec![format!("invalid milestone ID: {milestone}")];
    }

    let relative = format!("evidence/{}/manifest.json", milestone.to_ascii_lowercase());
    let manifest_path = root.join(&relative);
    if !manifest_path.is_file() {
        // A milestone with no manifest is not closed. Say that, rather than
        // reporting a filesystem error against an absolute path.
        return vec![format!(
            "{milestone} is not closed: {relative} does not exist"
        )];
    }

    let mut violations = verify_evidence_manifest(root, &manifest_path, milestone);
    verify_admitted_manifest(&manifest_path, milestone, &mut violations);
    violations
}

/// Checks one evidence manifest against the schema and milestone closure rules.
#[must_use]
pub fn verify_evidence_manifest(root: &Path, manifest_path: &Path, milestone: &str) -> Vec<String> {
    if !is_valid_milestone_id(milestone) {
        return vec![format!("invalid milestone ID: {milestone}")];
    }

    match jq_matches(manifest_path, "type == \"object\"") {
        Ok(true) => {}
        Ok(false) => return vec!["milestone manifest must be a JSON object".to_owned()],
        Err(error) => return vec![error],
    }

    let mut violations = Vec::new();
    match evidence_schema_matches(root, manifest_path) {
        Ok(true) => {}
        Ok(false) => {
            violations.push("milestone manifest does not conform to evidence schema".to_owned());
        }
        Err(error) => violations.push(error),
    }
    verify_redaction(manifest_path, "milestone manifest", &mut violations);
    if !jq_matches(manifest_path, ".schemaVersion == 1").unwrap_or(false) {
        violations.push("manifest schemaVersion must equal 1".to_owned());
    }
    let milestone_filter = format!(".milestone == \"{milestone}\"");
    if !jq_matches(manifest_path, &milestone_filter).unwrap_or(false) {
        violations.push(format!("manifest milestone must equal {milestone}"));
    }

    verify_general_closure(manifest_path, milestone, &mut violations);
    verify_manifest_change(root, manifest_path, &mut violations);
    if milestone == "M0" {
        verify_m0_manifest(manifest_path, &mut violations);
    }
    if milestone == "M1" {
        verify_m1_manifest(root, manifest_path, &mut violations);
    }
    if milestone == "M2" {
        verify_m2_manifest(root, manifest_path, &mut violations);
    }

    violations
}

fn verify_general_closure(manifest_path: &Path, milestone: &str, violations: &mut Vec<String>) {
    let work_package_prefix = format!("{milestone}-W");
    require_manifest_rule(
        manifest_path,
        &format!(".workPackages | length > 0 and all(startswith(\"{work_package_prefix}\"))"),
        "work-package IDs must belong to the manifest milestone",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".revision.dirty == false",
        "milestone revision must be clean",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".inputs | length > 0",
        "milestone must include at least one hashed input",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "([.commands[].exitStatus] + [.platformEvidence[].commands[].exitStatus]) | length > 0 and all(. == 0)",
        "milestone commands must all succeed",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".tests.total == (.tests.passed + .tests.failed + .tests.skipped)",
        "test counts must equal passed + failed + skipped",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".tests.total > 0 and .tests.passed > 0",
        "milestone must record at least one passing test",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".tests.failed == 0",
        "milestone tests must have zero failures",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".tests.skipped == 0",
        "milestone tests must have zero skipped tests",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".tests.lineCoveragePercent != null and .tests.lineCoveragePercent >= 80",
        "milestone line coverage must be at least 80%",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "([.deviations[].classification] + [.risks[].classification]) | all(. != \"blocking\")",
        "milestone must have no blocking deviations or risks",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".review.independent == true and .review.result == \"pass\"",
        "milestone review must be independent and pass",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".review | has(\"record\")",
        "milestone review must include a hashed record",
        violations,
    );
}

fn verify_admitted_manifest(manifest_path: &Path, milestone: &str, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        r#".evidenceKind == "native""#,
        "milestone closure requires native evidence, not a synthetic contract fixture",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#"[.. | strings | ascii_downcase | contains("synthetic")] | any | not"#,
        "native milestone evidence must not contain synthetic placeholders",
        violations,
    );
    if milestone == "M0" {
        require_manifest_rule(
            manifest_path,
            r#".attestations
                | any(
                    .platform == "linux"
                    and .architecture == "x86_64"
                    and (.workloads | sort == ["official-atm11", "vanilla"])
                )"#,
            "M0 closure requires a workload-bound native Linux attestation",
            violations,
        );
    }
}

/// Checks what closing M1 requires beyond the rules every milestone shares.
///
/// M1 decides the architecture, so its closure is about decisions being made on
/// recorded evidence: every gate measured, every decision written down, and the
/// one gate that needs an operator-run launch actually having one.
fn verify_m1_manifest(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M1-W1\", \"M1-W2\", \"M1-W3\", \"M1-W4\", \"M1-W5\", \"M1-W6\", \"M1-W7\", \"M1-W8\"]",
        "M1 manifest must include every work package from M1-W1 through M1-W8 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".benchmarks.protocolVersion == \"spike-v1\" and (.benchmarks.results | length > 0)",
        "M1 manifest must record spike results under the spike-v1 protocol",
        violations,
    );

    // Every gate needs a recorded result and an accepted decision. A gate with
    // no ADR is a measurement nobody committed to.
    for (directory, gate) in SPIKE_GATES {
        let results = root.join("evidence/m1/spikes").join(directory);
        let recorded = fs::read_dir(&results)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
            })
            .count();
        if recorded == 0 {
            violations.push(format!("M1 closure requires a recorded result for {gate}"));
        }
    }
    for decision in M1_DECISIONS {
        if !root.join("docs/decisions").join(decision).is_file() {
            violations.push(format!(
                "M1 closure requires the decision record {decision}"
            ));
        }
    }

    // The review itself is a process artifact and stays out of the repository, so
    // the manifest's hashed record names a document held elsewhere. That is a
    // weaker guarantee than recomputing the hash here, and deliberately so: the
    // shared rules still require the claim, only the file no longer ships.

    // ADG-5 is the one gate whose answer needs the game to actually start.
    let launched = fs::read_dir(root.join("evidence/m1/spikes/adg-5"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "json"))
        .any(|path| jq_matches(&path, ".prismIntegration.stockLaunch == true").unwrap_or(false));
    if !launched {
        violations.push(
            "M1 closure requires an ADG-5 result recording an observed stock-Prism launch"
                .to_owned(),
        );
    }
}

/// Checks what closing M2 requires beyond the rules every milestone shares.
///
/// M2 builds the pre-world loader, so its closure is about the pipeline having
/// been run rather than described. The manifest names the work; the records
/// under `evidence/m2/` are what happened, and each one is asked here for the
/// claim `ROADMAP.md`'s M2 rows make of it. A manifest that says the loader
/// works and a record that does not show it working is the failure this
/// catches.
fn verify_m2_manifest(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M2-W1\", \"M2-W2\", \"M2-W3\", \"M2-W4\", \"M2-W5\", \"M2-W6\", \"M2-W7\", \"M2-W8\"]",
        "M2 manifest must include every work package from M2-W1 through M2-W8 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".benchmarks.protocolVersion == \"m2-loader-budget-v1\" and (.benchmarks.results | length > 0)",
        "M2 manifest must record its results under the m2-loader-budget-v1 protocol",
        violations,
    );
    // M2-W5's launch is operator-run, so a person says it happened. The record
    // it points at is checked below; this is the claim that someone stands
    // behind it.
    require_manifest_rule(
        manifest_path,
        r#".attestations | any(.platform == "linux" and .architecture == "x86_64")"#,
        "M2 closure requires a native Linux attestation for the operator-run launch",
        violations,
    );

    for (file, rule, requirement) in M2_RECORDS {
        let path = root.join("evidence/m2").join(file);
        if !path.is_file() {
            violations.push(format!("M2 closure requires the record evidence/m2/{file}"));
            continue;
        }
        if !jq_matches(&path, rule).unwrap_or(false) {
            violations.push(format!("M2 closure requires {requirement}"));
        }
        // The records carry what a launcher log and a machine produced, so they
        // are held to the same redaction rule as the manifest rather than to
        // the reader's attention. The two generated records have a conformance
        // test of their own; the overhead and launch records have only this.
        verify_redaction(&path, &format!("evidence/m2/{file}"), violations);
    }
}

/// The records M2 cannot close without, and what each one has to say.
const M2_RECORDS: &[(&str, &str, &str)] = &[
    (
        "noop-pipeline.json",
        // `all(.repeatedRunIsIdentical)` would be a truthiness test, and every
        // non-null value but `false` is truthy in jq, so the string "no" would
        // pass it. The comparison is what makes the field mean what it says.
        ".runs | length > 0 and all(.repeatedRunIsIdentical == true)",
        "a no-op pipeline record whose run is identical when repeated",
    ),
    (
        "pre-world-gate.json",
        ".gate.refusesWorldAccess == true and .gate.ranModCode == false",
        "a pre-world gate record showing invalid packages refused before any mod code ran",
    ),
    (
        "loader-overhead.json",
        // A budget is only a budget if each line says which metric it bounds, in
        // what unit, and sits above the measurement it was derived from. A bare
        // positive number bounds nothing, and two lines naming the same metric
        // are a limit and a contradiction rather than two limits.
        r#".budget.limits
            | length > 0
            and all(
                (.metric | type == "string" and length > 0)
                and (.unit | type == "string" and length > 0)
                and .operator == "less-than-or-equal"
                and (.measured | type == "number")
                and .measured > 0
                and (.limit | type == "number")
                and .limit > .measured
            )
            and (map(.metric) | unique | length) == length"#,
        "a loader overhead record whose budget lines each name one metric and a unit, and set an \
         upper limit above the positive measurement that limit was derived from",
    ),
    (
        "prism-launch.json",
        // The rewrite count and the native invocation count are what M2's launch
        // added over M1's, which could show the client started and nothing more.
        // Each count is type-checked before it is compared: jq orders strings
        // above numbers, so an unguarded `> 0` accepts "not measured". The
        // objection has to be present and null, because a missing key reads as
        // null too and would let a record drop its own objection to pass.
        // `stockLauncher` is the operator's word rather than a measurement, so
        // the record labels it attested and the gate asks for it under that name.
        r#".launch.provesStockLaunch == true
            and (.launch | has("objection"))
            and .launch.objection == null
            and .launch.attestation.stockLauncher == true
            and .ravel.reachedGameProcess == true
            and .ravel.rewrite.reported == true
            and .ravel.rewrite.reachedRavel == true
            and (.ravel.rewrite.classesRewritten | type == "number")
            and .ravel.rewrite.classesRewritten > 0
            and (.ravel.rewrite.methodsRewritten | type == "number")
            and .ravel.rewrite.methodsRewritten > 0
            and (.ravel.rewrite.nativeInvocations | type == "number")
            and .ravel.rewrite.nativeInvocations > 0
            and .instance.withoutAnotherLoader == true"#,
        "a launch record showing the official client reaching Ravel from an attested stock Prism, \
         with the transformer's own count of what it rewrote inside that process and of the hook \
         invocations that reached native code, and no other loader present",
    ),
];

/// Decision records M1 cannot close without.
const M1_DECISIONS: &[&str] = &[
    "adg-1-jvm-entry.md",
    "adg-2-mod-execution.md",
    "adg-3-bindings.md",
    "adg-4-state-ownership.md",
    "adg-5-prism-integration.md",
];

fn verify_m0_manifest(manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M0-W1\", \"M0-W2\", \"M0-W3\", \"M0-W4\", \"M0-W5\", \"M0-W6\", \"M0-W7\", \"M0-W8\", \"M0-W9\"]",
        "M0 manifest must include every work package from M0-W1 through M0-W9 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".versions.minecraft == \"26.1.2\" and .versions.neoforge == \"26.1.2.78\" and .versions.fixture == \"ATM11 0.2.1\"",
        "M0 manifest must pin Minecraft 26.1.2, NeoForge 26.1.2.78, and ATM11 0.2.1",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        &m0_platform_filter("linux", "x86_64", "linux-reference-v1"),
        "M0 manifest must include native Linux x86_64 launch evidence",
        violations,
    );
}

fn m0_platform_filter(platform: &str, architecture: &str, hardware_profile: &str) -> String {
    format!(
        r#".platformEvidence | any(
            .platform == "{platform}"
            and .architecture == "{architecture}"
            and .native == true
            and .hardwareProfile == "{hardware_profile}"
            and .versions.minecraft == "26.1.2"
            and .versions.neoforge == "26.1.2.78"
            and .versions.prism == "11.0.3"
            and .versions.fixture == "ATM11 0.2.1"
            and .versions.java == "25.0.1"
            and .versions.rust == "1.95.0"
            and .versions.nix == "2.33.3"
            and .versions.devenv == "2.1.2"
            and (.settings | length > 0)
            and any(.commands[]; .command == "devenv test --no-tui")
            and any(.commands[]; .command == "nix flake check --accept-flake-config")
            and any(.baselines[]; .workload == "vanilla")
            and any(.baselines[]; .workload == "official-atm11")
        )"#
    )
}

/// Refuses a document that carries a local path, a credential, or a secret field.
///
/// Every tracked evidence document is held to this, not just the manifest: a
/// record built from a launcher log or a machine's output is where leakage would
/// actually come from. `subject` names the document in the violation, so a
/// reader is told which file to look at.
fn verify_redaction(document: &Path, subject: &str, violations: &mut Vec<String>) {
    let output = Command::new("jq")
        .args(["--raw-output", ".. | strings"])
        .arg(document)
        .output();
    let lower = output
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).to_ascii_lowercase())
        .unwrap_or_default();
    let contains_local_data = [
        "/home/",
        "/users/",
        r"c:\\users\\",
        // A path segment, not a bare suffix: `net.minecraft.util.StringUtil` is
        // a class name that evidence is supposed to contain.
        "/.minecraft",
        "tests/fixtures/local/",
        "evidence/private/",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern));
    let contains_credential_value = [
        "authorization: bearer",
        "authorization= bearer",
        "access_token=",
        "accesstoken=",
        "minecraft_access_token=",
        "tangled_token=",
        "aws_secret_access_key=",
        "atp_app_password=",
        "ghp_",
        "xoxb-",
        "xoxp-",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern));
    let credential_fields_absent = jq_matches(
        document,
        r#"[paths(strings) as $path
            | select(
                any(
                    $path[];
                    type == "string"
                    and (ascii_downcase | test("token|password|secret|credential|authorization"))
                )
                and (getpath($path) | length > 0)
            )] | length == 0"#,
    )
    .unwrap_or(false);

    if contains_local_data || contains_credential_value || !credential_fields_absent {
        violations.push(format!("{subject} contains sensitive or local data"));
    }
}

fn require_manifest_rule(
    manifest_path: &Path,
    filter: &str,
    violation: &str,
    violations: &mut Vec<String>,
) {
    match jq_matches(manifest_path, filter) {
        Ok(true) => {}
        Ok(false) => violations.push(violation.to_owned()),
        Err(error) => violations.push(error),
    }
}

/// Rejects rights-restricted, secret, and local-runtime paths from a tracked tree.
#[must_use]
pub fn verify_tracked_paths<'a>(paths: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    paths
        .into_iter()
        .filter_map(|path| {
            let normalized = path.replace('\\', "/");
            is_forbidden_tracked_path(&normalized)
                .then(|| format!("forbidden tracked path: {normalized}"))
        })
        .collect()
}

/// Rejects private keys and non-placeholder credential assignments in tracked text.
#[must_use]
pub fn verify_tracked_contents(root: &Path, paths: &[String]) -> Vec<String> {
    let mut violations = Vec::new();
    for relative in paths {
        let Ok(source) = fs::read_to_string(root.join(relative)) else {
            continue;
        };

        let private_key = [
            "PRIVATE KEY-----",
            "RSA PRIVATE KEY-----",
            "OPENSSH PRIVATE KEY-----",
        ]
        .iter()
        .any(|suffix| source.contains(&format!("{}{}", "-----BEGIN ", suffix)));
        let credential = source.lines().any(has_credential_assignment);
        if private_key || credential {
            violations.push(format!(
                "tracked file contains credential material: {relative}"
            ));
        }
    }
    violations
}

fn maintained_rust_roots() -> Vec<String> {
    CRATE_NAMES
        .iter()
        .map(|name| format!("crates/{name}/src/lib.rs"))
        .chain([
            "tests/conformance/src/lib.rs".to_owned(),
            "tools/ravel-cli/src/lib.rs".to_owned(),
            "tools/ravel-cli/src/main.rs".to_owned(),
            "tools/ravel-xtask/src/lib.rs".to_owned(),
            "tools/ravel-xtask/src/main.rs".to_owned(),
        ])
        .chain(
            TEST_MOD_NAMES
                .iter()
                .map(|name| format!("test-mods/{name}/src/lib.rs")),
        )
        .collect()
}

fn verify_ignored_boundaries(root: &Path, violations: &mut Vec<String>) {
    let gitignore_path = root.join(".gitignore");
    let Ok(gitignore) = fs::read_to_string(&gitignore_path) else {
        violations.push("cannot read .gitignore".to_owned());
        return;
    };

    for pattern in ["/tests/fixtures/local/**", "/evidence/private/**", ".env"] {
        if !gitignore.lines().any(|line| line.trim() == pattern) {
            violations.push(format!("missing restricted-content ignore: {pattern}"));
        }
    }
}

/// The metrics `evidence/m2/loader-overhead.json`'s own expiry clause voided,
/// and why each one is not compared.
///
/// The record says its three build limits "describe a linked pack" and that if
/// M3 selected anything but linking they "describe a shape that no longer exists
/// and are remeasured rather than carried". ADG-2 selected source compiled on the
/// player's machine, so the condition fired exactly as written and `ROADMAP.md`
/// records it.
///
/// They are listed here rather than removed from the record because a recorded
/// measurement is not edited after the fact — the clause inside it is what does
/// the voiding. They are listed at all, rather than skipped, because a limit that
/// is quietly not compared is indistinguishable from one that passed, and that is
/// how three breaches of 18x and 29x sat in this repository unremarked.
const VOID_LIMITS: &[(&str, &str)] = &[
    (
        "build.packCompile",
        "voided by the record's own expiry clause when ADG-2 chose source distribution over \
         linking; the build cost of source distribution is evidence/m3/pack-compile.json",
    ),
    (
        "build.relink",
        "voided by the record's own expiry clause when ADG-2 chose source distribution over \
         linking; nothing relinks a pack under the selected mechanism",
    ),
    (
        "build.binaryGrowthPerMod",
        "voided by the record's own expiry clause when ADG-2 chose source distribution over \
         linking; per-mod binary growth describes an artefact the mechanism does not produce",
    ),
];

/// The budget protocol this comparison understands.
///
/// A record declaring a different protocol is refused rather than compared. Two
/// records can carry the same metric names and mean different things by them, and
/// a budget that silently compared across protocols would be the same failure this
/// mechanism exists to end.
const BUDGET_PROTOCOL: &str = "m2-loader-budget-v1";

/// The records whose limits every later run is compared against.
///
/// The budget is their union rather than any one of them, because a limit is
/// set by whichever measurement was addressed to it: M2 bounded what the loader
/// costs, and M3-W6 bounds what profiling costs a tick when it is off. Two
/// records may not state the same metric — a metric with two ceilings has none —
/// and that is checked rather than resolved.
const BUDGET_RECORDS: &[&str] = &[
    "evidence/m2/loader-overhead.json",
    "evidence/m3/profiling-overhead.json",
];

/// What comparing a run against the regression budget found.
///
/// Violations and notes are separate because they answer different questions. A
/// violation stops a milestone; a note says what was *not* checked, which is the
/// half that has no other way to be seen. `budget.enforcement` in the record
/// records that "comparing a later run against them is M3's to build, and until it
/// does the rule above is a sentence rather than a mechanism" — notes are what
/// keep the mechanism from quietly becoming a sentence again.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BudgetReport {
    /// Limits the candidate run exceeded, or could not be compared against.
    pub violations: Vec<String>,
    /// What was deliberately not compared, and why.
    pub notes: Vec<String>,
}

/// One limit as a record states it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Limit {
    metric: String,
    /// The value a measurement must not exceed, as the record spells it.
    ceiling: String,
    measured: String,
    operator: String,
}

/// Compares the `candidates`' measurements against the recorded regression
/// budget.
///
/// The join key is the metric name, and both sides come from the same
/// `budget.limits` shape: a re-run writes a new record carrying its own measured
/// values, so nothing here needs a table mapping a metric to a field. That
/// matters more than it looks — a hand-written path table is a second description
/// of the record that can drift from it, and the drift would show up as a limit
/// silently not compared.
///
/// The candidates are taken together rather than one at a time. The budget spans
/// more than one measurement now, and no single run measures all of it: asking
/// each run for every limit would fail a loader run for not having profiled and
/// a profiling run for not having loaded. A limit is satisfied when some run
/// measured it and came in under it, and no limit may go unmeasured.
///
/// The hardware profile has to match. Each record's own restatement says its
/// limits "hold on linux-reference-v1 only" and that "a budget without the
/// machine it was measured on is not a budget", so a run from another machine is
/// refused rather than compared and passed.
#[must_use]
pub fn verify_budget(root: &Path, candidates: &[&Path]) -> BudgetReport {
    let mut report = BudgetReport::default();
    if candidates.is_empty() {
        report.violations.push(
            "no candidate run was given, and a budget compared against nothing passes \
                   vacuously"
                .to_owned(),
        );
        return report;
    }

    let owned = BUDGET_RECORDS
        .iter()
        .map(|record| root.join(record))
        .collect::<Vec<_>>();
    let baselines = owned
        .iter()
        .map(std::path::PathBuf::as_path)
        .collect::<Vec<_>>();

    for (path, label) in baselines
        .iter()
        .map(|path| (*path, "a budget record"))
        .chain(candidates.iter().map(|path| (*path, "a candidate run")))
    {
        match jq_value(path, ".budget.protocol") {
            Ok(protocol) if protocol == BUDGET_PROTOCOL => {}
            Ok(protocol) => report.violations.push(format!(
                "{label}, {}, declares budget protocol `{protocol}`, and this comparison \
                 understands only `{BUDGET_PROTOCOL}`",
                path.display()
            )),
            Err(error) => report.violations.push(error),
        }
    }
    if !report.violations.is_empty() {
        return report;
    }

    let required = match hardware_profile(&baselines) {
        Ok(profile) => profile,
        Err(error) => {
            report.violations.push(error);
            return report;
        }
    };
    for candidate in candidates {
        match jq_value(candidate, ".identity.hardwareProfile") {
            Ok(actual) if actual == required => {}
            Ok(actual) => report.violations.push(format!(
                "the budget holds on hardware profile `{required}` and {} was measured on \
                 `{actual}`, so it is remeasured rather than compared",
                candidate.display()
            )),
            Err(error) => report.violations.push(error),
        }
    }
    if !report.violations.is_empty() {
        return report;
    }

    let baseline_limits = match union(&baselines, "budget records") {
        Ok(limits) => limits,
        Err(error) => {
            report.violations.push(error);
            return report;
        }
    };
    let candidate_limits = match union(candidates, "candidate runs") {
        Ok(limits) => limits,
        Err(error) => {
            report.violations.push(error);
            return report;
        }
    };

    for limit in &baseline_limits {
        if let Some((_, reason)) = VOID_LIMITS
            .iter()
            .find(|(metric, _)| *metric == limit.metric)
        {
            report
                .notes
                .push(format!("`{}` is not compared: {reason}", limit.metric));
            continue;
        }
        compare(limit, &candidate_limits, &mut report.violations);
    }

    for measured in &candidate_limits {
        if !baseline_limits
            .iter()
            .any(|limit| limit.metric == measured.metric)
        {
            report.notes.push(format!(
                "`{}` is measured by the candidate run and the budget sets no limit for it, so \
                 nothing here can pass or fail it",
                measured.metric
            ));
        }
    }

    report
}

/// Compares one live limit against whatever the candidate measured for it.
///
/// The ceiling and the comparison both come from the budget, and only the
/// measurement comes from the candidate. A run that could restate the operator or
/// the limit it is judged by would be grading itself, which is a softer version of
/// the same silence this whole check exists to end.
fn compare(limit: &Limit, candidate: &[Limit], violations: &mut Vec<String>) {
    let Some(measured) = candidate.iter().find(|entry| entry.metric == limit.metric) else {
        violations.push(format!(
            "the budget limits `{}` and the candidate run does not measure it, so the limit \
             cannot be said to hold",
            limit.metric
        ));
        return;
    };

    // An operator this comparison does not implement is refused rather than
    // assumed. Guessing that an unknown comparison meant "at most" is how a
    // budget starts passing things it was written to stop.
    if limit.operator != "less-than-or-equal" {
        violations.push(format!(
            "`{}` uses comparison `{}`, which this check does not implement",
            limit.metric, limit.operator
        ));
        return;
    }

    match (
        limit.ceiling.parse::<f64>(),
        measured.measured.parse::<f64>(),
    ) {
        (Ok(ceiling), Ok(value)) if value <= ceiling => {}
        (Ok(ceiling), Ok(value)) => violations.push(format!(
            "`{}` measured {value} against a limit of {ceiling}, which is a regression",
            limit.metric
        )),
        _ => violations.push(format!(
            "`{}` has a limit or a measurement that is not a number",
            limit.metric
        )),
    }
}

/// The one hardware profile every budget record holds on.
///
/// Two budget records that hold on different machines are not one budget, and a
/// candidate cannot be on both. Refusing that is cheaper than deciding which
/// half of an incoherent budget a run should be judged by.
fn hardware_profile(baselines: &[&Path]) -> Result<String, String> {
    let mut required: Option<String> = None;
    for baseline in baselines {
        let profile = jq_value(baseline, ".budget.hardwareProfile")?;
        match &required {
            None => required = Some(profile),
            Some(first) if *first == profile => {}
            Some(first) => {
                return Err(format!(
                    "the budget records hold on different hardware profiles, `{first}` and \
                     `{profile}`, so together they are not a budget; {} is the second",
                    baseline.display()
                ));
            }
        }
    }
    required.ok_or_else(|| "no budget record was named".to_owned())
}

/// Reads every limit a set of records states, refusing a metric two of them
/// state.
///
/// A metric with two ceilings has none: which one applies would be decided by
/// the order the files happened to be read in. Refusing is the only answer that
/// does not make the budget depend on that.
fn union(paths: &[&Path], role: &str) -> Result<Vec<Limit>, String> {
    let mut all: Vec<Limit> = Vec::new();
    for path in paths {
        for limit in limits(path)? {
            if all.iter().any(|seen| seen.metric == limit.metric) {
                return Err(format!(
                    "`{}` is stated by two {role}, so it has two ceilings and therefore none; the \
                     second is in {}",
                    limit.metric,
                    path.display()
                ));
            }
            all.push(limit);
        }
    }
    Ok(all)
}

/// Reads every limit a record states, in the order it states them.
fn limits(path: &Path) -> Result<Vec<Limit>, String> {
    let rows = jq_value(
        path,
        r#".budget.limits[] | "\(.metric)\t\(.limit)\t\(.measured)\t\(.operator)""#,
    )?;
    if rows.is_empty() {
        return Err(format!("{} states no budget limits", path.display()));
    }

    rows.lines()
        .map(|row| {
            let mut fields = row.split('\t');
            match (fields.next(), fields.next(), fields.next(), fields.next()) {
                (Some(metric), Some(ceiling), Some(measured), Some(operator)) => Ok(Limit {
                    metric: metric.to_owned(),
                    ceiling: ceiling.to_owned(),
                    measured: measured.to_owned(),
                    operator: operator.to_owned(),
                }),
                _ => Err(format!(
                    "{} states a limit that is not complete",
                    path.display()
                )),
            }
        })
        .collect()
}

fn verify_schema_documents(root: &Path, violations: &mut Vec<String>) {
    for schema in [
        "configuration",
        "configuration-override",
        "evidence",
        "fixture",
        "pack",
        "package",
        "performance-collector",
        "performance-result",
        "provenance",
        "runtime-inputs",
        "saved-data",
        "spike-result",
    ] {
        let relative = format!("schemas/{schema}.schema.json");
        let path = root.join(&relative);
        match jq_matches(
            &path,
            "type == \"object\" and .\"$schema\" == \"https://json-schema.org/draft/2020-12/schema\"",
        ) {
            Ok(true) => {}
            Ok(false) => violations.push(format!("invalid JSON schema document: {relative}")),
            Err(error) => violations.push(error),
        }
    }
}

fn verify_version_boundary(root: &Path, violations: &mut Vec<String>) {
    let crates = root.join("crates");
    if let Ok(entries) = fs::read_dir(&crates) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(suffix) = name.strip_prefix("ravel-minecraft-")
                && suffix != "adapter"
            {
                violations.push(format!("unexpected per-version Minecraft crate: {name}"));
            }
        }
    }

    let versions = fs::read_dir(root.join("minecraft/versions"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    let mut files = Vec::new();
    collect_files(&root.join("crates"), &mut files);
    for path in files {
        if !matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs" | "toml")
        ) {
            continue;
        }
        if fs::read_to_string(&path)
            .is_ok_and(|source| versions.iter().any(|version| source.contains(version)))
        {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            violations.push(format!(
                "hardcoded Minecraft version outside version data: {}",
                relative.display()
            ));
        }
    }
}

fn tracked_paths(root: &Path) -> Result<Vec<String>, String> {
    if root.join(".jj").exists() {
        let output = Command::new("jj")
            .args(["file", "list"])
            .current_dir(root)
            .output()
            .map_err(|error| format!("cannot list tracked files with jj: {error}"))?;
        if !output.status.success() {
            return Err("cannot list tracked files with jj".to_owned());
        }
        return Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_owned)
            .collect());
    }

    let mut files = Vec::new();
    collect_files(root, &mut files);
    Ok(files
        .iter()
        .filter_map(|path| path.strip_prefix(root).ok())
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect())
}

fn collect_files(root: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            if !matches!(
                name.to_str(),
                Some(".git" | ".jj" | ".devenv" | ".direnv" | "target")
            ) {
                collect_files(&path, files);
            }
        } else if path.is_file() {
            files.push(path);
        }
    }
}

fn is_forbidden_tracked_path(path: &str) -> bool {
    if path == "tests/fixtures/local/README.md" {
        return false;
    }

    let lower = path.to_ascii_lowercase();
    let basename = lower.rsplit('/').next().unwrap_or(&lower);
    let components = lower.split('/').collect::<Vec<_>>();
    let forbidden_prefix = [
        "tests/fixtures/local/",
        "evidence/private/",
        "private/",
        "restricted/",
        ".minecraft/",
        "prism-instances/",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix));
    let forbidden_component = components
        .iter()
        .any(|component| matches!(*component, "saves" | "logs" | "crash-reports"));
    let forbidden_evidence_component = lower.starts_with("evidence/")
        && components
            .iter()
            .any(|component| matches!(*component, "raw" | "captures"));
    let forbidden_basename = matches!(
        basename,
        ".env"
            | "credentials.json"
            | "secrets.json"
            | "fabric.mod.json"
            | "quilt.mod.json"
            | "neoforge.mods.toml"
            | "mods.toml"
    ) || (basename.starts_with(".env.") && basename != ".env.example")
        || basename.starts_with("hs_err_pid");
    let forbidden_extension = [
        ".jar",
        ".mrpack",
        ".pem",
        ".p12",
        ".pfx",
        ".key",
        ".keystore",
        ".jfr",
        ".hprof",
        ".trace",
    ]
    .iter()
    .any(|extension| lower.ends_with(extension));

    forbidden_prefix
        || forbidden_component
        || forbidden_evidence_component
        || forbidden_basename
        || forbidden_extension
}

fn is_valid_milestone_id(milestone: &str) -> bool {
    matches!(
        milestone,
        "M0" | "M1"
            | "M2"
            | "M3"
            | "M4"
            | "M5"
            | "M6"
            | "M7"
            | "M8"
            | "M9"
            | "M10"
            | "M11"
            | "M12"
            | "M13"
    )
}

/// Reads one scalar out of a JSON file, for claims that are compared rather than
/// asserted.
fn jq_value(path: &Path, filter: &str) -> Result<String, String> {
    let output = Command::new("jq")
        .args(["-r", filter])
        .arg(path)
        .output()
        .map_err(|error| format!("cannot read JSON with jq: {error}"))?;
    if !output.status.success() {
        return Err(format!("cannot read {filter} from {}", path.display()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn jq_matches(path: &Path, filter: &str) -> Result<bool, String> {
    let output = Command::new("jq")
        .args(["--exit-status", filter])
        .arg(path)
        .output()
        .map_err(|error| format!("cannot validate JSON with jq: {error}"))?;
    Ok(output.status.success())
}

fn evidence_schema_matches(root: &Path, manifest: &Path) -> Result<bool, String> {
    let output = Command::new("check-jsonschema")
        .arg("--schemafile")
        .arg(root.join("schemas/evidence.schema.json"))
        .arg(manifest)
        .output()
        .map_err(|error| format!("cannot validate evidence schema: {error}"))?;
    Ok(output.status.success())
}

fn has_credential_assignment(line: &str) -> bool {
    let Some((name, value)) = line.split_once('=') else {
        return false;
    };
    if !matches!(
        name.trim(),
        "ATP_APP_PASSWORD" | "AWS_SECRET_ACCESS_KEY" | "MINECRAFT_ACCESS_TOKEN" | "TANGLED_TOKEN"
    ) {
        return false;
    }

    let value = value.trim().trim_matches(['\'', '"']);
    !value.is_empty()
        && !value.contains('<')
        && !value.eq_ignore_ascii_case("example")
        && !value.eq_ignore_ascii_case("placeholder")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::{
        change_mismatch, required_scaffold_paths, verify_evidence_records, verify_experiment_fence,
        verify_ignored_boundaries, verify_milestone, verify_scaffold, verify_spikes,
        verify_tracked_contents, verify_tracked_paths, verify_version_boundary,
    };

    /// Writes a milestone manifest recording one result and returns the root.
    fn recorded_result_root(label: &str, result: &str, digest: &str) -> PathBuf {
        let root = temporary_root(label);
        let milestone = root.join("evidence/m9");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(milestone.join("result.json"), result).expect("result should be writable");
        fs::write(
            milestone.join("manifest.json"),
            format!(
                r#"{{"benchmarks":{{"protocolVersion":"v1",
                    "results":[{{"name":"result.json","sha256":"{digest}"}}]}}}}"#
            ),
        )
        .expect("manifest should be writable");
        root
    }

    /// The digest of `{}\n`, which is what the tests below record.
    const EMPTY_OBJECT_DIGEST: &str =
        "ca3d163bab055381827226140568f3bef7eaac187cebd76878e0b63e9e442356";

    #[test]
    fn a_recorded_result_digest_is_recomputed_against_the_file() {
        let root = recorded_result_root("records-match", "{}\n", EMPTY_OBJECT_DIGEST);

        assert!(verify_evidence_records(&root).is_empty());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_recorded_result_digest_that_binds_nothing_is_reported() {
        // The defect this exists for: a manifest validated against its schema
        // while four of its digests named content no tracked file ever held.
        let root = recorded_result_root("records-drift", "{\"changed\": true}\n", &"0".repeat(64));

        let violations = verify_evidence_records(&root);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].starts_with("evidence/m9/manifest.json records result.json as ")
                && violations[0].contains(" but it hashes "),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_manifest_cannot_record_its_own_digest_or_a_result_it_does_not_hold() {
        let root = temporary_root("records-self");
        let milestone = root.join("evidence/m9");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(
            milestone.join("manifest.json"),
            format!(
                r#"{{"benchmarks":{{"protocolVersion":"v1","results":[
                    {{"name":"manifest.json","sha256":"{EMPTY_OBJECT_DIGEST}"}},
                    {{"name":"../m8/borrowed.json","sha256":"{EMPTY_OBJECT_DIGEST}"}},
                    {{"name":"absent.json","sha256":"{EMPTY_OBJECT_DIGEST}"}}]}}}}"#
            ),
        )
        .expect("manifest should be writable");

        let violations = verify_evidence_records(&root);

        assert_eq!(
            violations,
            [
                "evidence/m9/manifest.json records its own digest, which cannot bind".to_owned(),
                "evidence/m9/manifest.json records a result outside its own milestone: \
                 ../m8/borrowed.json"
                    .to_owned(),
                format!(
                    "evidence/m9/manifest.json records absent.json: cannot read the recorded \
                     file: {}",
                    fs::read(milestone.join("absent.json"))
                        .expect_err("absent file cannot be read")
                ),
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// Writes a manifest recording one baseline result and one private
    /// validation, and returns the root.
    fn recorded_baseline_root(label: &str, result: &str, digest: &str) -> PathBuf {
        let root = temporary_root(label);
        let milestone = root.join("evidence/m9");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(milestone.join("launch.json"), result).expect("record should be writable");
        fs::write(
            milestone.join("manifest.json"),
            format!(
                r#"{{"platformEvidence":[{{"baselines":[{{"workload":"vanilla",
                    "result":{{"name":"launch.json","sha256":"{digest}"}},
                    "validation":{{"name":"private launch validation",
                        "sha256":"{}"}}}}]}}]}}"#,
                "0".repeat(64)
            ),
        )
        .expect("manifest should be writable");
        root
    }

    #[test]
    fn a_recorded_baseline_digest_is_recomputed_against_the_tracked_file() {
        // The private validation digest names a capture outside the repository
        // and is announced rather than checked, so it cannot be a violation.
        let root = recorded_baseline_root("baseline-match", "{}\n", EMPTY_OBJECT_DIGEST);

        assert!(verify_evidence_records(&root).is_empty());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_recorded_baseline_digest_that_binds_nothing_is_reported() {
        // The hole this exists for: two M0 baseline digests naming tracked
        // evidence files that no gate recomputed.
        let root = recorded_baseline_root("baseline-drift", "{}\n", &"0".repeat(64));

        let violations = verify_evidence_records(&root);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].starts_with("evidence/m9/manifest.json records launch.json as ")
                && violations[0].ends_with(&format!(" but it hashes {EMPTY_OBJECT_DIGEST}")),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_baseline_naming_a_file_the_milestone_does_not_hold_is_reported() {
        let root = temporary_root("baseline-absent");
        let milestone = root.join("evidence/m9");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(
            milestone.join("manifest.json"),
            format!(
                r#"{{"platformEvidence":[{{"baselines":[{{"workload":"vanilla",
                    "result":{{"name":"absent.json","sha256":"{EMPTY_OBJECT_DIGEST}"}},
                    "validation":{{"name":"../m8/borrowed.json",
                        "sha256":"{EMPTY_OBJECT_DIGEST}"}}}}]}}]}}"#
            ),
        )
        .expect("manifest should be writable");

        let violations = verify_evidence_records(&root);

        assert_eq!(
            violations,
            [
                format!(
                    "evidence/m9/manifest.json records absent.json: cannot read the recorded \
                     file: {}",
                    fs::read(milestone.join("absent.json"))
                        .expect_err("absent file cannot be read")
                ),
                "evidence/m9/manifest.json records a baseline outside its own milestone: \
                 ../m8/borrowed.json"
                    .to_owned(),
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// The digest of `hello\n`, which the pinned-input tests below record.
    const HELLO_DIGEST: &str = "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03";

    /// Initialises a jj repository holding one input and returns it with the
    /// revision that holds it.
    ///
    /// The input is edited afterwards so its recorded content survives only in
    /// the pinned revision, which is the position every closed milestone is in:
    /// a check that hashed the working copy would see the later content.
    fn pinned_input_root(label: &str) -> (PathBuf, String) {
        let root = temporary_root(label);
        let initialised = Command::new("jj")
            .args(["git", "init"])
            .current_dir(&root)
            .output()
            .expect("jj should be available");
        assert!(initialised.status.success(), "{initialised:?}");
        fs::write(root.join("input.txt"), "hello\n").expect("input should be writable");
        let pinned = Command::new("jj")
            .args(["log", "-r", "@", "--no-graph", "-T", "commit_id"])
            .current_dir(&root)
            .output()
            .expect("jj should be available");
        assert!(pinned.status.success(), "{pinned:?}");
        fs::write(root.join("input.txt"), "moved on\n").expect("input should be rewritable");
        (
            root,
            String::from_utf8_lossy(&pinned.stdout).trim().to_owned(),
        )
    }

    /// Records one input against a pinned revision in an M9 manifest.
    fn write_input_manifest(root: &Path, revision: &str, name: &str, digest: &str) {
        let milestone = root.join("evidence/m9");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(
            milestone.join("manifest.json"),
            format!(
                r#"{{"revision":{{"id":"{revision}"}},
                    "inputs":[{{"name":"{name}","sha256":"{digest}"}}]}}"#
            ),
        )
        .expect("manifest should be writable");
    }

    #[test]
    fn an_input_digest_is_recomputed_against_the_revision_the_manifest_pins() {
        let (root, revision) = pinned_input_root("inputs-match");
        write_input_manifest(&root, &revision, "input.txt", HELLO_DIGEST);

        assert!(verify_evidence_records(&root).is_empty());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn an_input_digest_that_binds_nothing_at_the_pinned_revision_is_reported() {
        // The hole this exists for: an input digest that matched the file at no
        // revision at all, recorded beside four that did and read as verified.
        let (root, revision) = pinned_input_root("inputs-drift");
        write_input_manifest(&root, &revision, "input.txt", &"0".repeat(64));

        let violations = verify_evidence_records(&root);

        assert_eq!(
            violations,
            [format!(
                "evidence/m9/manifest.json records input input.txt as {} but revision {revision} \
                 holds {HELLO_DIGEST}",
                "0".repeat(64)
            )]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn an_input_the_pinned_revision_does_not_hold_is_reported() {
        let (root, revision) = pinned_input_root("inputs-absent");
        write_input_manifest(&root, &revision, "absent.txt", HELLO_DIGEST);

        let violations = verify_evidence_records(&root);

        assert_eq!(
            violations,
            [format!(
                "evidence/m9/manifest.json records input absent.txt: cannot read it at revision \
                 {revision}"
            )]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// Writes a manifest recording one gate run and returns the root.
    fn recorded_run_root(label: &str, run: &str) -> PathBuf {
        let root = temporary_root(label);
        let milestone = root.join("evidence/m9");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(milestone.join("manifest.json"), run).expect("manifest should be writable");
        root
    }

    #[test]
    fn a_recorded_run_whose_numbers_agree_is_accepted() {
        let root = recorded_run_root(
            "run-consistent",
            r#"{"commands":[{"exitStatus":0}],
                "tests":{"total":9,"passed":9,"failed":0,"skipped":0,
                    "lineCoveragePercent":85.97}}"#,
        );

        assert!(verify_evidence_records(&root).is_empty());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_recorded_run_whose_counts_do_not_sum_is_reported() {
        // A manifest attests a run nothing can replay, so the arithmetic between
        // its own numbers is the part a gate can hold it to.
        let root = recorded_run_root(
            "run-arithmetic",
            r#"{"commands":[{"exitStatus":0}],
                "tests":{"total":8,"passed":9,"failed":0,"skipped":0,
                    "lineCoveragePercent":85.97}}"#,
        );

        assert_eq!(
            verify_evidence_records(&root),
            ["evidence/m9/manifest.json records test counts that do not sum to its total"]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_recorded_run_with_no_test_result_and_no_coverage_is_reported() {
        let root = recorded_run_root("run-untested", r#"{"commands":[{"exitStatus":1}]}"#);

        assert_eq!(
            verify_evidence_records(&root),
            [
                "evidence/m9/manifest.json records a gate run but no test result",
                "evidence/m9/manifest.json records a gate run that did not succeed",
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_recorded_run_below_the_coverage_floor_is_reported() {
        let root = recorded_run_root(
            "run-coverage",
            r#"{"tests":{"total":9,"passed":9,"failed":0,"skipped":0,
                "lineCoveragePercent":79.9}}"#,
        );

        assert_eq!(
            verify_evidence_records(&root),
            ["evidence/m9/manifest.json records line coverage below the floor the gate enforces"]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn an_absent_spike_tree_is_not_a_violation() {
        let root = temporary_root("spikes-absent");

        assert!(verify_spikes(&root).is_empty());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn spike_results_must_cover_every_failure_scenario() {
        let root = temporary_root("spikes-failures");
        let gate = root.join("evidence/m1/spikes/adg-1");
        fs::create_dir_all(&gate).expect("spike directory should be creatable");
        fs::write(
            gate.join("candidate.json"),
            r#"{"schemaVersion":1,"protocolVersion":"spike-v1","gate":"ADG-1",
                "candidate":{"disposition":"measured"},
                "failures":[{"scenario":"crash"}],
                "memory":{"bootstrapPeakRss":{"status":"measured","value":1},
                    "retainedRss":{"status":"measured","value":1},
                    "retainedClasses":{"status":"measured","value":1}},
                "startup":{"processToReady":{"status":"measured","value":1}},
                "calls":{"callbackNanoseconds":{"status":"measured"}},
                "trial":{"measuredInvocations":1},"deviations":[]}"#,
        )
        .expect("spike result should be writable");

        let violations = verify_spikes(&root);

        assert!(violations.iter().any(|violation| {
            violation.ends_with("must exercise every required failure scenario exactly once")
        }));
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_claimed_stock_launch_without_the_redacted_log_hash_is_refused() {
        let root = temporary_root("spikes-launch-log");
        fs::create_dir_all(root.join("schemas")).expect("schema directory should be creatable");
        fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/spike-result.schema.json"),
            root.join("schemas/spike-result.schema.json"),
        )
        .expect("spike schema should be copied");
        let gate = root.join("evidence/m1/spikes/adg-5");
        fs::create_dir_all(&gate).expect("spike directory should be creatable");
        let recorded = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../evidence/m1/spikes/adg-5/adg5-stock-jvm-arguments.json"),
        )
        .expect("the recorded ADG-5 result should be readable");
        let target = gate.join("candidate.json");

        // The recorded result is the control: only the log artifact's name differs
        // between the two writes, so nothing but the new rule can explain the refusal.
        fs::write(&target, &recorded).expect("spike result should be writable");
        assert!(
            verify_spikes(&root).is_empty(),
            "the recorded result should pass unaltered"
        );

        fs::write(
            &target,
            recorded.replace("redacted-instance-launch.log", "some-other-capture.log"),
        )
        .expect("altered spike result should be writable");
        assert!(
            verify_spikes(&root)
                .iter()
                .any(|violation| violation.ends_with("does not conform to the spike schema")),
            "a stock launch with no redacted log hash should be refused"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn spike_results_must_not_substitute_zero_for_a_missing_measurement() {
        let root = temporary_root("spikes-zero");
        let gate = root.join("evidence/m1/spikes/adg-1");
        fs::create_dir_all(&gate).expect("spike directory should be creatable");
        fs::write(
            gate.join("candidate.json"),
            r#"{"schemaVersion":1,"protocolVersion":"spike-v1","gate":"ADG-1",
                "candidate":{"disposition":"measured"},
                "failures":[{"scenario":"callback-failure"},{"scenario":"crash"},
                    {"scenario":"invalid-input"},{"scenario":"timeout"},
                    {"scenario":"unavailable-runtime"}],
                "memory":{"bootstrapPeakRss":{"status":"measured","value":0},
                    "retainedRss":{"status":"measured","value":1},
                    "retainedClasses":{"status":"measured","value":1}},
                "startup":{"processToReady":{"status":"measured","value":1}},
                "calls":{"callbackNanoseconds":{"status":"measured"}},
                "trial":{"measuredInvocations":1},"deviations":[]}"#,
        )
        .expect("spike result should be writable");

        let violations = verify_spikes(&root);

        assert!(violations.iter().any(|violation| {
            violation.ends_with("must not report a measured bootstrap or startup value of zero")
        }));
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn production_crates_cannot_depend_on_measurement_experiments() {
        let root = temporary_root("fence");
        let crate_root = root.join("crates/ravel-loader");
        fs::create_dir_all(&crate_root).expect("crate directory should be creatable");
        fs::write(
            crate_root.join("Cargo.toml"),
            "[dependencies]\nravel-spike-harness.workspace = true\n",
        )
        .expect("manifest should be writable");
        let mut violations = Vec::new();

        verify_experiment_fence(&root, &mut violations);

        assert_eq!(
            violations,
            ["production crate depends on an experiment: crates/ravel-loader/Cargo.toml"]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_result_naming_a_change_that_did_not_introduce_it_is_reported() {
        // The defect this exists for: seven results named a revision no rewrite
        // had updated, and nothing recomputed the claim until a human read them.
        let wrong = change_mismatch(
            "evidence/m1/spikes/adg-3/adg3-javap-extraction.json",
            "wtvoqolwmrxtpwsxspnlwnsnkoslxxlt",
            "ysvpslstqmmmkkzknzmozqkpnozlworn",
        );
        assert_eq!(
            wrong.as_deref(),
            Some(
                "evidence/m1/spikes/adg-3/adg3-javap-extraction.json records change \
                 wtvoqolwmrxtpwsxspnlwnsnkoslxxlt but was introduced by \
                 ysvpslstqmmmkkzknzmozqkpnozlworn"
            )
        );
        assert_eq!(
            change_mismatch("r.json", "wtvoqolwmrxt", "wtvoqolwmrxt"),
            None
        );
        // An untracked result has no introducing change and is not yet checkable.
        assert_eq!(change_mismatch("r.json", "wtvoqolwmrxt", ""), None);
    }

    #[test]
    fn unsafe_code_cannot_be_permitted_outside_experiments() {
        let root = temporary_root("fence-unsafe");
        let crate_root = root.join("crates/ravel-bootstrap/src");
        fs::create_dir_all(&crate_root).expect("crate directory should be creatable");
        fs::write(
            crate_root.join("lib.rs"),
            "#![allow(unsafe_code)]\npub fn boundary() {}\n",
        )
        .expect("source should be writable");
        let mut violations = Vec::new();

        verify_experiment_fence(&root, &mut violations);

        assert_eq!(
            violations,
            ["unsafe code is permitted outside the experiment boundary: \
                 crates/ravel-bootstrap/src/lib.rs"]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn missing_files_are_reported_by_relative_path() {
        let root = temporary_root("missing");

        let violations = verify_scaffold(&root);

        assert!(
            violations
                .iter()
                .any(|violation| violation == "missing required file: Cargo.toml")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn every_required_path_is_unique() {
        let mut paths = required_scaffold_paths();
        let original_len = paths.len();
        paths.sort_unstable();
        paths.dedup();

        assert_eq!(paths.len(), original_len);
    }

    #[test]
    fn restricted_content_ignores_are_required() {
        let root = temporary_root("ignores");
        fs::write(root.join(".gitignore"), "# intentionally incomplete\n")
            .expect("test gitignore should be writable");
        let mut violations = Vec::new();

        verify_ignored_boundaries(&root, &mut violations);

        assert_eq!(violations.len(), 3);
        assert!(
            violations
                .iter()
                .all(|violation| violation.starts_with("missing restricted-content ignore:"))
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn restricted_tracked_paths_are_rejected() {
        let violations = verify_tracked_paths([
            "tests/fixtures/local/secret.jar",
            "evidence/private/capture.json",
            "evidence/m0/raw/process.log",
            "evidence/m0/captures/frame.trace",
            "config/credentials.json",
            "docs/README.md",
        ]);

        assert_eq!(violations.len(), 5);
        assert!(
            violations
                .iter()
                .all(|violation| violation.starts_with("forbidden tracked path:"))
        );
    }

    #[test]
    fn tracked_credential_material_is_rejected() {
        let root = temporary_root("credentials");
        fs::write(
            root.join("notes.txt"),
            format!("{}{}\n", "-----BEGIN OPENSSH ", "PRIVATE KEY-----"),
        )
        .expect("credential canary should be writable");

        let violations = verify_tracked_contents(&root, &["notes.txt".to_owned()]);

        assert_eq!(
            violations,
            ["tracked file contains credential material: notes.txt"]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn milestone_manifest_must_match_requested_milestone() {
        let root = temporary_root("milestone");
        let evidence = root.join("evidence/m0");
        fs::create_dir_all(&evidence).expect("evidence directory should be creatable");
        fs::write(
            evidence.join("manifest.json"),
            r#"{"schemaVersion":1,"milestone":"M1"}"#,
        )
        .expect("manifest should be writable");

        let violations = verify_milestone(&root, "M0");

        assert!(
            violations
                .iter()
                .any(|violation| violation == "manifest milestone must equal M0")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn malformed_milestone_ids_are_rejected() {
        let root = temporary_root("milestone-id");

        let violations = verify_milestone(&root, "M14");

        assert_eq!(violations, ["invalid milestone ID: M14"]);
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn version_specific_crates_and_production_literals_are_rejected() {
        let root = temporary_root("versions");
        fs::create_dir_all(root.join("minecraft/versions/test-version"))
            .expect("version directory should be creatable");
        fs::create_dir_all(root.join("crates/ravel-minecraft-test-version/src"))
            .expect("crate directory should be creatable");
        fs::write(
            root.join("crates/ravel-minecraft-test-version/src/lib.rs"),
            "const VERSION: &str = \"test-version\";\n",
        )
        .expect("test source should be writable");
        let mut violations = Vec::new();

        verify_version_boundary(&root, &mut violations);

        assert_eq!(violations.len(), 2);
        assert!(
            violations
                .iter()
                .any(|violation| violation.starts_with("unexpected per-version Minecraft crate:"))
        );
        assert!(violations.iter().any(|violation| {
            violation.starts_with("hardcoded Minecraft version outside version data:")
        }));
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    fn temporary_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("ravel-xtask-{label}-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale temporary root should be removable");
        }
        fs::create_dir_all(&root).expect("temporary root should be creatable");
        root
    }
}
