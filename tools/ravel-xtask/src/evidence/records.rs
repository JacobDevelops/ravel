//! Recorded artifacts, revisions, digests, and review verdicts.

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::evidence::findings::relative_label;
use crate::evidence::manifest::{milestone_manifests, verify_manifest_integrity};
use crate::json::{Rule, array, number, read_json};
use crate::tracked::tracked_paths;

mod binding;

use binding::{TrackedDigest, names_a_path, verify_review, verify_tracked_digest};

#[derive(Deserialize)]
struct Artifact<'a> {
    name: &'a str,
    sha256: &'a str,
}

#[derive(Deserialize)]
struct NamedCapture<'a> {
    name: &'a str,
}

#[derive(Default, Deserialize)]
struct Benchmarks<'a> {
    #[serde(default, borrow)]
    results: Vec<Artifact<'a>>,
}

#[derive(Deserialize)]
struct Baseline<'a> {
    #[serde(borrow)]
    result: Option<Artifact<'a>>,
    #[serde(borrow)]
    validation: Option<NamedCapture<'a>>,
}

#[derive(Deserialize)]
struct Platform<'a> {
    #[serde(default, borrow)]
    baselines: Vec<Baseline<'a>>,
}

#[derive(Default, Deserialize)]
struct Review<'a> {
    result: Option<&'a str>,
    #[serde(borrow)]
    record: Option<Artifact<'a>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Artifacts<'a> {
    #[serde(default, borrow)]
    inputs: Vec<Artifact<'a>>,
    #[serde(default, borrow)]
    benchmarks: Benchmarks<'a>,
    #[serde(default, borrow)]
    platform_evidence: Vec<Platform<'a>>,
    #[serde(default, borrow)]
    review: Option<Review<'a>>,
}

impl<'a> Artifacts<'a> {
    fn results(&self) -> impl Iterator<Item = &Artifact<'a>> {
        self.benchmarks
            .results
            .iter()
            .chain(self.platform_evidence.iter().flat_map(|platform| {
                platform
                    .baselines
                    .iter()
                    .filter_map(|baseline| baseline.result.as_ref())
            }))
    }
}

/// Recomputes every digest a milestone manifest records for its own results.
#[must_use]
pub fn verify_evidence_records(root: &Path) -> Vec<String> {
    let manifests = match milestone_manifests(root) {
        Ok(manifests) => manifests,
        Err(error) => return vec![error],
    };
    if manifests.is_empty() {
        return vec![
            "evidence/ holds no milestone manifest, and a manifest sweep over nothing passes \
             vacuously"
                .to_owned(),
        ];
    }
    let tracked = match tracked_paths(root) {
        Ok(paths) => paths.into_iter().collect::<BTreeSet<String>>(),
        Err(error) => return vec![error],
    };
    let mut violations = Vec::new();
    for (path, milestone) in manifests {
        let path = root.join(path);
        match read_json(&path) {
            Ok(manifest) => {
                verify_recorded_manifest(root, &path, &manifest, &tracked, &mut violations);
                verify_manifest_integrity(root, &path, milestone, &manifest, &mut violations);
            }
            Err(error) => violations.push(error),
        }
    }
    violations
}

fn verify_recorded_manifest(
    root: &Path,
    path: &Path,
    manifest: &Value,
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let artifacts = match Artifacts::deserialize(manifest) {
        Ok(artifacts) => artifacts,
        Err(error) => {
            violations.push(format!(
                "{} records invalid artifacts: {error}",
                relative_label(root, path)
            ));
            return;
        }
    };
    verify_artifact_digests(root, path, &artifacts, tracked, violations);
    if let Some(review) = &artifacts.review {
        verify_review(root, path, review, tracked, violations);
    }
    verify_inputs(root, path, manifest, &artifacts.inputs, tracked, violations);
    verify_recorded_run(root, path, manifest, violations);
    verify_revisions(root, path, &artifacts, violations);
    verify_admitted_inventory(root, path, &artifacts, tracked, violations);
}

fn verify_recorded_run(root: &Path, path: &Path, manifest: &Value, violations: &mut Vec<String>) {
    let label = relative_label(root, path);
    let records_run = !array(manifest, "/commands").is_empty();
    let records_tests = manifest["tests"].is_object();
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
    if records_run
        && !array(manifest, "/commands")
            .iter()
            .all(|command| command["exitStatus"] == 0)
    {
        violations.push(format!("{label} records a gate run that did not succeed"));
    }
    if records_tests {
        for (rule, violation) in RUN_RULES {
            if !rule(manifest) {
                violations.push(format!("{label} {violation}"));
            }
        }
    }
}

const RUN_RULES: &[(Rule, &str)] = &[
    (
        |value| {
            let tests = &value["tests"];
            let counts = (
                tests["total"].as_u64(),
                tests["passed"].as_u64(),
                tests["failed"].as_u64(),
                tests["skipped"].as_u64(),
            );
            matches!(counts, (Some(total), Some(passed), Some(failed), Some(skipped))
                if passed.checked_add(failed).and_then(|sum| sum.checked_add(skipped)) == Some(total))
        },
        "records test counts that do not sum to its total",
    ),
    (
        |value| {
            number(value, "/tests/total").is_some_and(|total| total > 0.0)
                && value["tests"]["failed"] == 0
                && value["tests"]["skipped"] == 0
        },
        "records no passing test run",
    ),
    (
        |value| {
            number(value, "/tests/lineCoveragePercent").is_some_and(|coverage| coverage >= 80.0)
        },
        "records line coverage below the floor the gate enforces",
    ),
];

fn verify_admitted_inventory(
    root: &Path,
    path: &Path,
    artifacts: &Artifacts<'_>,
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let label = relative_label(root, path);
    let Some(directory) = path.parent() else {
        return;
    };
    let prefix = format!("{}/", relative_label(root, directory));
    let named: BTreeSet<_> = artifacts.results().map(|artifact| artifact.name).collect();
    for record in tracked.iter().filter_map(|path| path.strip_prefix(&prefix)) {
        if Path::new(record)
            .extension()
            .is_some_and(|extension| extension == "json")
            && record != "manifest.json"
            && !named.contains(record)
        {
            violations.push(format!(
                "{label} names no record {record}, which its own milestone directory tracks; a record \
                 no manifest names is a record no digest covers"
            ));
        }
    }
}

fn verify_revisions(
    root: &Path,
    path: &Path,
    artifacts: &Artifacts<'_>,
    violations: &mut Vec<String>,
) {
    let label = relative_label(root, path);
    let Some(directory) = path.parent() else {
        return;
    };
    for artifact in artifacts.results() {
        let record = directory.join(artifact.name);
        if !record.is_file() {
            continue;
        }
        match read_json(&record) {
            Ok(value) => verify_trial_pins(
                &value,
                &mut Vec::with_capacity(8),
                &label,
                artifact.name,
                violations,
            ),
            Err(error) => violations.push(error),
        }
    }
}

enum TrialLocation<'a> {
    Key(&'a str),
    Index(usize),
}

impl fmt::Display for TrialLocation<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key(key) => formatter.write_str(key),
            Self::Index(index) => write!(formatter, "{index}"),
        }
    }
}

fn verify_trial_pins<'a>(
    value: &'a Value,
    location: &mut Vec<TrialLocation<'a>>,
    label: &str,
    name: &str,
    violations: &mut Vec<String>,
) {
    if value.is_object()
        && location.len() >= 2
        && matches!(location[location.len() - 2], TrialLocation::Key("trial"))
        && matches!(location.last(), Some(TrialLocation::Key("revision")))
        && !value["dirty"].is_boolean()
    {
        let at = Value::String(
            location
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("."),
        );
        violations.push(format!(
            "{label} admits {name}, whose trial at {at} does not declare whether \
             the tree it measured was dirty"
        ));
    }
    match value {
        Value::Object(fields) => {
            for (key, child) in fields {
                location.push(TrialLocation::Key(key));
                verify_trial_pins(child, location, label, name, violations);
                location.pop();
            }
        }
        Value::Array(children) => {
            for (index, child) in children.iter().enumerate() {
                location.push(TrialLocation::Index(index));
                verify_trial_pins(child, location, label, name, violations);
                location.pop();
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn verify_inputs(
    root: &Path,
    path: &Path,
    manifest: &Value,
    inputs: &[Artifact<'_>],
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let label = relative_label(root, path);
    for input in inputs {
        if !tracked.contains(input.name) {
            violations.push(format!(
                "{label} records input {}, which the repository no longer holds, so the \
                 digest binds a path no reader can resolve",
                input.name
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
    let Some(revision) = manifest
        .pointer("/revision/id")
        .and_then(Value::as_str)
        .filter(|revision| !revision.is_empty())
    else {
        violations.push(format!(
            "{label} records inputs but no revision to hash them at"
        ));
        return;
    };
    let resolved = match visible_commit(root, revision) {
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
    for input in inputs {
        let Artifact {
            name,
            sha256: recorded,
        } = input;
        match revision_digest(root, &resolved, name) {
            Ok(actual) if actual == *recorded => {}
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
    Ok(crate::hex_digest(&Sha256::digest(&output.stdout)))
}

fn verify_artifact_digests(
    root: &Path,
    path: &Path,
    artifacts: &Artifacts<'_>,
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let label = relative_label(root, path);
    let Some(directory) = path.parent() else {
        return;
    };
    for artifact in &artifacts.benchmarks.results {
        verify_tracked_digest(
            &TrackedDigest {
                label: &label,
                kind: "result",
                root,
                tracked,
                manifest_path: path,
                directory,
                name: artifact.name,
                recorded: artifact.sha256,
            },
            violations,
        );
    }
    for baseline in artifacts
        .platform_evidence
        .iter()
        .flat_map(|platform| &platform.baselines)
    {
        if let Some(validation) = &baseline.validation {
            println!(
                "- {label}: the baseline validation digest recorded for \"{}\" is not \
                 recomputed, because a launch validation is a private capture that never enters the \
                 repository",
                validation.name
            );
        }
        let Some(artifact) = &baseline.result else {
            continue;
        };
        if !names_a_path(artifact.name) {
            violations.push(format!(
                "{label} records a baseline result by description rather than by path, so its \
                 digest binds nothing obtainable: {}",
                artifact.name
            ));
            continue;
        }
        verify_tracked_digest(
            &TrackedDigest {
                label: &label,
                kind: "baseline",
                root,
                tracked,
                manifest_path: path,
                directory,
                name: artifact.name,
                recorded: artifact.sha256,
            },
            violations,
        );
    }
}

#[cfg(test)]
pub(crate) fn verify_recorded_review(
    root: &Path,
    path: &Path,
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    match read_json(path) {
        Ok(manifest) => verify_recorded_review_value(root, path, &manifest, tracked, violations),
        Err(error) => violations.push(error),
    }
}

pub(crate) fn verify_recorded_review_value(
    root: &Path,
    path: &Path,
    manifest: &Value,
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let Some(value) = manifest.get("review").filter(|value| !value.is_null()) else {
        return;
    };
    match Review::deserialize(value) {
        Ok(review) => verify_review(root, path, &review, tracked, violations),
        Err(error) => violations.push(format!(
            "{} records invalid review metadata: {error}",
            relative_label(root, path)
        )),
    }
}

#[cfg(test)]
mod tests;
