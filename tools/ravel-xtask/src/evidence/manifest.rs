//! Manifest identity, closure dispatch, and redaction.

use std::path::Path;
use std::process::Command;

use crate::evidence::findings::verify_finding_anchors;
use crate::evidence::m0::verify_m0_manifest;
use crate::evidence::m1::verify_m1_manifest;
use crate::evidence::m2::verify_m2_manifest;
use crate::evidence::m3::verify_m3_manifest;
use crate::evidence::m4::verify_m4_manifest;
use crate::evidence::m5::verify_m5_manifest;
use crate::evidence::m6::verify_m6_manifest;
use crate::evidence::m7::verify_m7_manifest;
use crate::evidence::records::verify_recorded_review;
use crate::json::{evidence_schema_matches, jq_matches};
use crate::tracked::tracked_paths;

pub(crate) const MILESTONE_MANIFESTS: &[(&str, &str)] = &[
    ("evidence/m0/manifest.json", "M0"),
    ("evidence/m1/manifest.json", "M1"),
    ("evidence/m2/manifest.json", "M2"),
    ("evidence/m3/manifest.json", "M3"),
    ("evidence/m4/manifest.json", "M4"),
    ("evidence/m5/manifest.json", "M5"),
];

/// Checks a milestone evidence manifest's identity and minimum gate fields.
#[must_use]
pub fn verify_milestone(root: &Path, milestone: &str) -> Vec<String> {
    if !is_valid_milestone_id(milestone) {
        return vec![format!("invalid milestone ID: {milestone}")];
    }

    let relative = format!("evidence/{}/manifest.json", milestone.to_ascii_lowercase());
    let manifest_path = root.join(&relative);
    if !manifest_path.is_file() {
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

    match tracked_paths(root) {
        Ok(paths) => verify_recorded_review(
            root,
            manifest_path,
            &paths.into_iter().collect(),
            &mut violations,
        ),
        Err(error) => violations.push(error),
    }
    verify_finding_anchors(root, manifest_path, milestone, &mut violations);
    if milestone == "M0" {
        verify_m0_manifest(manifest_path, &mut violations);
    }
    if milestone == "M1" {
        verify_m1_manifest(root, manifest_path, &mut violations);
    }
    if milestone == "M2" {
        verify_m2_manifest(root, manifest_path, &mut violations);
    }
    if milestone == "M3" {
        verify_m3_manifest(root, manifest_path, &mut violations);
    }
    if milestone == "M4" {
        verify_m4_manifest(root, manifest_path, &mut violations);
    }
    if milestone == "M5" {
        verify_m5_manifest(root, manifest_path, &mut violations);
    }
    if milestone == "M6" {
        verify_m6_manifest(root, manifest_path, &mut violations);
    }
    if milestone == "M7" {
        verify_m7_manifest(root, manifest_path, &mut violations);
    }

    violations
}

pub(crate) const MANIFEST_INVARIANTS: &[(&str, &str)] = &[
    (
        ".revision.dirty == false",
        "milestone revision must be clean",
    ),
    (
        ".review.result == null or (.review | has(\"record\"))",
        "milestone recording a review verdict must record a hashed review record for it",
    ),
    (
        ".inputs | length > 0",
        "milestone must include at least one hashed input",
    ),
    (
        "([.commands[].exitStatus] + [.platformEvidence[].commands[].exitStatus]) | length > 0 and all(. == 0)",
        "milestone commands must all succeed",
    ),
    (
        ".tests.total == (.tests.passed + .tests.failed + .tests.skipped)",
        "test counts must equal passed + failed + skipped",
    ),
    (
        ".tests.total > 0 and .tests.passed > 0",
        "milestone must record at least one passing test",
    ),
    (
        ".tests.failed == 0",
        "milestone tests must have zero failures",
    ),
    (
        ".tests.skipped == 0",
        "milestone tests must have zero skipped tests",
    ),
    (
        ".tests.lineCoveragePercent != null and .tests.lineCoveragePercent >= 80",
        "milestone line coverage must be at least 80%",
    ),
];

const MANIFEST_VERDICTS: &[(&str, &str)] = &[
    (
        "([.deviations[].classification] + [.risks[].classification]) | all(. != \"blocking\")",
        "milestone must have no blocking deviations or risks",
    ),
    (
        ".review.independent == true and .review.result == \"pass\"",
        "milestone review must be independent and pass",
    ),
];

fn verify_general_closure(manifest_path: &Path, milestone: &str, violations: &mut Vec<String>) {
    let work_package_prefix = format!("{milestone}-W");
    require_manifest_rule(
        manifest_path,
        &format!(".workPackages | length > 0 and all(startswith(\"{work_package_prefix}\"))"),
        "work-package IDs must belong to the manifest milestone",
        violations,
    );
    for (filter, violation) in MANIFEST_INVARIANTS.iter().chain(MANIFEST_VERDICTS) {
        require_manifest_rule(manifest_path, filter, violation, violations);
    }
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

pub(crate) fn verify_redaction(document: &Path, subject: &str, violations: &mut Vec<String>) {
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

pub(crate) fn require_manifest_rule(
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

#[cfg(test)]
mod tests;
