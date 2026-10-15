//! Recorded artifacts, revisions, digests, and review verdicts.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::evidence::closure::verify_milestone_figures;
use crate::evidence::findings::{relative_label, verify_finding_anchors};
use crate::evidence::m4::{verify_m4_gate_figures, verify_m4_process_matrix, verify_m4_surface};
use crate::evidence::m5::{M5_FIGURES, verify_m5_inventory_arithmetic};
use crate::evidence::manifest::{
    MANIFEST_INVARIANTS, MILESTONE_MANIFESTS, require_manifest_rule, verify_redaction,
};
use crate::json::{evidence_schema_matches, jq_matches, jq_value};
use crate::tracked::tracked_paths;

/// Recomputes every digest a milestone manifest records for its own results.
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

    if manifests.is_empty() {
        violations.push(
            "evidence/ holds no milestone manifest, and a manifest sweep over nothing passes \
             vacuously"
                .to_owned(),
        );
        return violations;
    }

    let tracked = match tracked_paths(root) {
        Ok(paths) => paths.into_iter().collect::<BTreeSet<String>>(),
        Err(error) => {
            violations.push(error);
            return violations;
        }
    };

    for manifest in &manifests {
        verify_recorded_digests(root, manifest, &tracked, &mut violations);
        verify_recorded_baselines(root, manifest, &tracked, &mut violations);
        verify_recorded_review(root, manifest, &tracked, &mut violations);
        verify_recorded_inputs(root, manifest, &tracked, &mut violations);
        verify_recorded_run(root, manifest, &mut violations);
        verify_admitted_revisions(root, manifest, &mut violations);
        verify_admitted_inventory(root, manifest, &tracked, &mut violations);
    }

    for (relative, milestone) in MILESTONE_MANIFESTS {
        let manifest = root.join(relative);
        if manifest.is_file() {
            verify_manifest_invariants(root, &manifest, milestone, &mut violations);
        }
    }

    violations
}

fn verify_manifest_invariants(
    root: &Path,
    manifest_path: &Path,
    milestone: &str,
    violations: &mut Vec<String>,
) {
    let label = relative_label(root, manifest_path);
    match evidence_schema_matches(root, manifest_path) {
        Ok(true) => {}
        Ok(false) => violations.push(format!("{label} does not conform to the evidence schema")),
        Err(error) => violations.push(error),
    }
    verify_redaction(manifest_path, &label, violations);

    for (filter, requirement) in [
        (
            ".schemaVersion == 1".to_owned(),
            "must record schemaVersion 1",
        ),
        (
            format!(".milestone == \"{milestone}\""),
            "must record the milestone of the directory it sits in",
        ),
        (
            format!(".workPackages | length > 0 and all(startswith(\"{milestone}-W\"))"),
            "must carry work-package IDs belonging to its own milestone",
        ),
    ] {
        require_manifest_rule(
            manifest_path,
            &filter,
            &format!("{label} {requirement}"),
            violations,
        );
    }
    for (filter, requirement) in MANIFEST_INVARIANTS {
        require_manifest_rule(
            manifest_path,
            filter,
            &format!("{label} {requirement}"),
            violations,
        );
    }

    verify_finding_anchors(root, manifest_path, milestone, violations);
    if milestone == "M4" {
        verify_m4_surface(root, violations);
        verify_m4_process_matrix(root, violations);
        verify_m4_gate_figures(root, manifest_path, violations);
    }
    if milestone == "M5" {
        verify_m5_inventory_arithmetic(root, violations);
        verify_milestone_figures(root, manifest_path, &M5_FIGURES, violations);
    }
}

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

    if !records_run && !records_tests {
        violations.push(format!(
            "{label} records neither a gate run nor a test result, so there is nothing here a \
             gate can hold it to"
        ));
        return;
    }
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

fn verify_admitted_inventory(
    root: &Path,
    manifest_path: &Path,
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let label = relative_label(root, manifest_path);
    let Some(directory) = manifest_path.parent() else {
        return;
    };
    let prefix = format!("{}/", relative_label(root, directory));
    let held = tracked
        .iter()
        .filter_map(|path| path.strip_prefix(&prefix))
        .filter(|path| {
            Path::new(path)
                .extension()
                .is_some_and(|extension| extension == "json")
                && *path != "manifest.json"
        })
        .collect::<BTreeSet<_>>();
    let named = match jq_value(
        manifest_path,
        r".benchmarks.results[]?.name,
          (.platformEvidence[]?.baselines[]?.result? | select(. != null) | .name)",
    ) {
        Ok(named) => named,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let named = named
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect::<BTreeSet<_>>();

    for record in held.difference(&named) {
        violations.push(format!(
            "{label} names no record {record}, which its own milestone directory tracks; a record \
             no manifest names is a record no digest covers"
        ));
    }
}

const TRIAL_PINS: &str = r#"[paths(objects) as $p
        | select(($p | length) >= 2 and ($p[-1]) == "revision" and ($p[-2]) == "trial")
        | {at: ($p | map(tostring) | join(".")), pin: getpath($p)}]
    | .[]
    | (.at | tojson) + " " + (.pin.dirty | type)"#;

fn verify_admitted_revisions(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    let label = relative_label(root, manifest_path);
    let Some(directory) = manifest_path.parent() else {
        return;
    };

    let names = match jq_value(
        manifest_path,
        r".benchmarks.results[]?.name,
          (.platformEvidence[]?.baselines[]?.result? | select(. != null) | .name)",
    ) {
        Ok(names) => names,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    for name in names.lines().filter(|name| !name.trim().is_empty()) {
        let record = directory.join(name);

        if !record.is_file() {
            continue;
        }
        match jq_value(&record, TRIAL_PINS) {
            Ok(pins) => {
                for pin in pins.lines().filter(|pin| !pin.trim().is_empty()) {
                    let (at, dirty) = pin
                        .rsplit_once(' ')
                        .expect("TRIAL_PINS always emits a location and type separated by a space");
                    if dirty != "boolean" {
                        violations.push(format!(
                            "{label} admits {name}, whose trial at {at} does not declare whether \
                             the tree it measured was dirty"
                        ));
                    }
                }
            }
            Err(error) => violations.push(error),
        }
    }
}

fn verify_recorded_inputs(
    root: &Path,
    manifest_path: &Path,
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let label = manifest_path
        .strip_prefix(root)
        .unwrap_or(manifest_path)
        .display()
        .to_string();
    let records = match jq_value(manifest_path, r#".inputs[]? | .name + " " + .sha256"#) {
        Ok(records) => records,
        Err(error) => {
            violations.push(error);
            return;
        }
    };

    let mut inputs = Vec::new();
    for record in records.lines().filter(|record| !record.trim().is_empty()) {
        match record.rsplit_once(' ') {
            Some((name, recorded)) => inputs.push((name, recorded)),
            None => violations.push(format!("{label} records an input with no digest: {record}")),
        }
    }

    for (name, _) in &inputs {
        if !tracked.contains(*name) {
            violations.push(format!(
                "{label} records input {name}, which the repository no longer holds, so the \
                 digest binds a path no reader can resolve"
            ));
        }
    }

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
    let resolved = match visible_commit(root, &revision) {
        Ok(commit) if commit == revision => commit,
        Ok(_) => {
            violations.push(format!(
                "{label} revision.id {revision} is not a full commit id"
            ));
            return;
        }
        Err(error) => {
            violations.push(format!("{label} revision.id {revision} {error}"));
            return;
        }
    };

    for (name, recorded) in inputs {
        match revision_digest(root, &resolved, name) {
            Ok(actual) if actual == recorded => {}
            Ok(actual) => violations.push(format!(
                "{label} records input {name} as {recorded} but revision {revision} holds {actual}"
            )),
            Err(error) => violations.push(format!("{label} records input {name}: {error}")),
        }
    }
}

pub(crate) fn visible_commit(root: &Path, revision: &str) -> Result<String, String> {
    let output = Command::new("jj")
        .args([
            "log",
            "--color",
            "never",
            "-r",
            &format!("(::visible_heads() & ({revision})) ~ root()"),
            "--no-graph",
            "-T",
            r#"commit_id ++ "\n""#,
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot be resolved: {error}"))?;
    if !output.status.success() {
        return Err("cannot be resolved by jj".to_owned());
    }
    let resolved = String::from_utf8_lossy(&output.stdout);
    let mut commits = resolved.lines().filter(|line| !line.trim().is_empty());
    let Some(commit) = commits.next() else {
        return Err("names no commit in visible history".to_owned());
    };
    if commits.next().is_some() {
        return Err("names more than one visible commit".to_owned());
    }
    Ok(commit.to_owned())
}

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

fn verify_recorded_digests(
    root: &Path,
    manifest_path: &Path,
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let label = relative_label(root, manifest_path);
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
                root,
                tracked,
                manifest_path,
                directory,
                name,
                recorded,
            },
            violations,
        );
    }
}

fn verify_recorded_baselines(
    root: &Path,
    manifest_path: &Path,
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let label = relative_label(root, manifest_path);
    let results = match jq_value(
        manifest_path,
        r#".platformEvidence[]?.baselines[]?.result
           | select(. != null) | .name + " " + .sha256"#,
    ) {
        Ok(results) => results,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let validations = match jq_value(
        manifest_path,
        r".platformEvidence[]?.baselines[]?.validation | select(. != null) | .name",
    ) {
        Ok(validations) => validations,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let Some(directory) = manifest_path.parent() else {
        return;
    };

    for name in validations.lines().filter(|name| !name.trim().is_empty()) {
        println!(
            "- {label}: the baseline validation digest recorded for \"{name}\" is not \
             recomputed, because a launch validation is a private capture that never enters the \
             repository"
        );
    }

    for record in results.lines().filter(|record| !record.trim().is_empty()) {
        let Some((name, recorded)) = record.rsplit_once(' ') else {
            violations.push(format!(
                "{label} records a baseline with no digest: {record}"
            ));
            continue;
        };
        if !names_a_path(name) {
            violations.push(format!(
                "{label} records a baseline result by description rather than by path, so its \
                 digest binds nothing obtainable: {name}"
            ));
            continue;
        }
        verify_tracked_digest(
            &TrackedDigest {
                label: &label,
                kind: "baseline",
                root,
                tracked,
                manifest_path,
                directory,
                name,
                recorded,
            },
            violations,
        );
    }
}

pub(crate) fn verify_recorded_review(
    root: &Path,
    manifest_path: &Path,
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let label = relative_label(root, manifest_path);
    let record = match jq_value(
        manifest_path,
        r#".review.record? | select(. != null) | .name + " " + .sha256"#,
    ) {
        Ok(record) => record,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    if record.is_empty() {
        return;
    }
    let Some(directory) = manifest_path.parent() else {
        return;
    };
    let Some((name, recorded)) = record.rsplit_once(' ') else {
        violations.push(format!(
            "{label} records a review record with no digest: {record}"
        ));
        return;
    };
    if !names_a_path(name) {
        violations.push(format!(
            "{label} records a review record by title rather than by path, so its digest \
             binds nothing obtainable: {name}"
        ));
        return;
    }
    verify_tracked_digest(
        &TrackedDigest {
            label: &label,
            kind: "review record",
            root,
            tracked,
            manifest_path,
            directory,
            name,
            recorded,
        },
        violations,
    );
    verify_recorded_verdict(&label, manifest_path, &directory.join(name), violations);
}

fn verify_recorded_verdict(
    label: &str,
    manifest_path: &Path,
    record: &Path,
    violations: &mut Vec<String>,
) {
    let claimed = match jq_value(manifest_path, r#".review.result? // """#) {
        Ok(claimed) => claimed,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    if claimed.is_empty() || claimed == "null" {
        violations.push(format!(
            "{label} binds a review record but records no review.result for it to agree with"
        ));
        return;
    }

    let Ok(text) = fs::read_to_string(record) else {
        return;
    };
    match text.lines().find_map(stated_verdict) {
        Some(stated) if stated == claimed => {}
        Some(stated) => violations.push(format!(
            "{label} records review.result {claimed} while the review record it binds states \
             {stated}"
        )),
        None => violations.push(format!(
            "{label} binds a review record that states no verdict, so nothing checks that \
             review.result {claimed} is what the review concluded"
        )),
    }
}

fn stated_verdict(line: &str) -> Option<String> {
    let line = line.trim().trim_start_matches(['#', '*', ' ']);
    let value = line
        .strip_prefix("Result:")
        .or_else(|| line.strip_prefix("Result**:"))?;
    let value = value.trim().trim_matches(['*', ' ']).to_ascii_lowercase();
    ["pass", "fail", "not-required"]
        .contains(&value.as_str())
        .then_some(value)
}

fn names_a_path(name: &str) -> bool {
    !name.contains(char::is_whitespace) && Path::new(name).extension().is_some()
}

struct TrackedDigest<'a> {
    label: &'a str,
    kind: &'a str,
    root: &'a Path,
    tracked: &'a BTreeSet<String>,
    manifest_path: &'a Path,
    directory: &'a Path,
    name: &'a str,
    recorded: &'a str,
}

fn verify_tracked_digest(record: &TrackedDigest<'_>, violations: &mut Vec<String>) {
    let TrackedDigest {
        label,
        kind,
        root,
        tracked,
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

    if path == manifest_path {
        violations.push(format!("{label} records its own digest, which cannot bind"));
        return;
    }

    let relative = relative_label(root, &path).replace('\\', "/");
    if !tracked.contains(&relative) {
        violations.push(format!(
            "{label} records a {kind} that the repository does not track, so its digest binds \
             nothing a reader can obtain: {name}"
        ));
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

fn file_digest(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read the recorded file: {error}"))?;
    Ok(hex_digest(&bytes))
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut digest = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(digest, "{byte:02x}").expect("writing to a string cannot fail");
    }
    digest
}

#[cfg(test)]
mod tests;
