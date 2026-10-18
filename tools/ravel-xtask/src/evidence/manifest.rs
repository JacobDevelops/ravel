//! Manifest identity, closure dispatch, and redaction.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use ravel_observability::contains_credential;
use ravel_observability::redaction::CREDENTIAL_WORDS;
use serde_json::Value;

use crate::evidence::rules::{Rule, evidence_schema_matches};
use crate::evidence::{findings, m0, m1, m2, m3, m4, m5, m6, m7, records};
use crate::json::{array, number, read_json};
use crate::tracked::tracked_paths;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Milestone {
    M0,
    M1,
    M2,
    M3,
    M4,
    M5,
    M6,
    M7,
    M8,
    M9,
    M10,
    M11,
    M12,
    M13,
}

impl Milestone {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::M0 => "M0",
            Self::M1 => "M1",
            Self::M2 => "M2",
            Self::M3 => "M3",
            Self::M4 => "M4",
            Self::M5 => "M5",
            Self::M6 => "M6",
            Self::M7 => "M7",
            Self::M8 => "M8",
            Self::M9 => "M9",
            Self::M10 => "M10",
            Self::M11 => "M11",
            Self::M12 => "M12",
            Self::M13 => "M13",
        }
    }
}

impl fmt::Display for Milestone {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Milestone {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "M0" => Ok(Self::M0),
            "M1" => Ok(Self::M1),
            "M2" => Ok(Self::M2),
            "M3" => Ok(Self::M3),
            "M4" => Ok(Self::M4),
            "M5" => Ok(Self::M5),
            "M6" => Ok(Self::M6),
            "M7" => Ok(Self::M7),
            "M8" => Ok(Self::M8),
            "M9" => Ok(Self::M9),
            "M10" => Ok(Self::M10),
            "M11" => Ok(Self::M11),
            "M12" => Ok(Self::M12),
            "M13" => Ok(Self::M13),
            _ => Err(format!("invalid milestone ID: {text}")),
        }
    }
}

/// Discovers milestone directories; identity inside each document is checked separately.
pub(crate) fn milestone_manifests(root: &Path) -> Result<Vec<(PathBuf, Milestone)>, String> {
    let entries = fs::read_dir(root.join("evidence"))
        .map_err(|error| format!("cannot read evidence directory: {error}"))?;
    let mut manifests = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read evidence entry: {error}"))?;
        let kind = entry
            .file_type()
            .map_err(|error| format!("cannot inspect evidence entry: {error}"))?;
        if !kind.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with('m') || !name[1..].bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let milestone = name.to_ascii_uppercase().parse::<Milestone>()?;
        let relative = PathBuf::from("evidence")
            .join(name.as_ref())
            .join("manifest.json");
        if !root.join(&relative).is_file() {
            return Err(format!("missing required file: {}", relative.display()));
        }
        manifests.push((relative, milestone));
    }
    if manifests.is_empty() {
        return Err("evidence directory contains no milestone manifests".to_owned());
    }
    manifests.sort_by_key(|(_, milestone)| *milestone);
    Ok(manifests)
}

/// Checks a milestone's closure, including its native-evidence admission policy.
#[must_use]
pub fn verify_milestone(root: &Path, milestone: &str) -> Vec<String> {
    let milestone = match milestone.parse::<Milestone>() {
        Ok(milestone) => milestone,
        Err(error) => return vec![error],
    };
    let relative = format!(
        "evidence/{}/manifest.json",
        milestone.as_str().to_ascii_lowercase()
    );
    let path = root.join(&relative);
    let document = match read_json(&path) {
        Ok(document) => document,
        Err(error) => return vec![error],
    };
    let mut violations = verify_document(root, &path, milestone, &document);
    verify_admitted_manifest(&document, milestone, &mut violations);
    super::accounting::verify_milestone_accounting(root, milestone, &mut violations);
    violations
}

/// Checks one supplied manifest against the schema and closure rules.
#[must_use]
pub fn verify_evidence_manifest(root: &Path, path: &Path, milestone: &str) -> Vec<String> {
    let milestone = match milestone.parse::<Milestone>() {
        Ok(milestone) => milestone,
        Err(error) => return vec![error],
    };
    match read_json(path) {
        Ok(document) => verify_document(root, path, milestone, &document),
        Err(error) => vec![error],
    }
}

fn verify_document(
    root: &Path,
    path: &Path,
    milestone: Milestone,
    document: &Value,
) -> Vec<String> {
    let mut violations = Vec::new();
    verify_manifest_integrity(root, path, milestone, document, &mut violations);
    for (rule, violation) in MANIFEST_VERDICTS {
        require_manifest_rule(document, rule, violation, &mut violations);
    }
    match tracked_paths(root) {
        Ok(paths) => records::verify_recorded_review_value(
            root,
            path,
            document,
            &paths.into_iter().collect(),
            &mut violations,
        ),
        Err(error) => violations.push(error),
    }
    match milestone {
        Milestone::M0 => m0::verify_m0_manifest(document, &mut violations),
        Milestone::M1 => m1::verify_m1_manifest(root, document, &mut violations),
        Milestone::M2 => m2::verify_m2_manifest(root, document, &mut violations),
        Milestone::M3 => m3::verify_m3_manifest(root, document, &mut violations),
        Milestone::M4 => m4::verify_m4_manifest(root, document, &mut violations),
        Milestone::M5 => m5::verify_m5_manifest(root, document, &mut violations),
        Milestone::M6 => m6::verify_m6_manifest(root, document, &mut violations),
        Milestone::M7 => m7::verify_m7_manifest(root, document, &mut violations),
        Milestone::M8
        | Milestone::M9
        | Milestone::M10
        | Milestone::M11
        | Milestone::M12
        | Milestone::M13 => {}
    }
    violations
}

/// Integrity is checked even for open milestones; review pass/blocking closure verdicts are not.
pub(crate) fn verify_manifest_integrity(
    root: &Path,
    path: &Path,
    milestone: Milestone,
    document: &Value,
    violations: &mut Vec<String>,
) {
    if !document.is_object() {
        violations.push("milestone manifest must be a JSON object".to_owned());
        return;
    }
    match evidence_schema_matches(root, path) {
        Ok(true) => {}
        Ok(false) => {
            violations.push("milestone manifest does not conform to evidence schema".to_owned());
        }
        Err(error) => violations.push(error),
    }
    verify_redaction_value(document, "milestone manifest", violations);
    if document["schemaVersion"] != 1 {
        violations.push("manifest schemaVersion must equal 1".to_owned());
    }
    if document["milestone"] != milestone.as_str() {
        violations.push(format!("manifest milestone must equal {milestone}"));
    }
    let prefix = format!("{milestone}-W");
    require_manifest_rule(
        document,
        |value| {
            !array(value, "/workPackages").is_empty()
                && array(value, "/workPackages")
                    .iter()
                    .all(|id| id.as_str().is_some_and(|id| id.starts_with(&prefix)))
        },
        "work-package IDs must belong to the manifest milestone",
        violations,
    );
    for (rule, violation) in MANIFEST_INVARIANTS {
        require_manifest_rule(document, rule, violation, violations);
    }
    findings::verify_finding_anchors_value(root, path, milestone, document, violations);
    match milestone {
        Milestone::M2 => m2::verify_m2_integrity(root, document, violations),
        Milestone::M3 => m3::verify_m3_integrity(root, document, violations),
        Milestone::M4 => m4::verify_m4_integrity(root, document, violations),
        Milestone::M5 => m5::verify_m5_integrity(root, document, violations),
        Milestone::M6 => m6::verify_m6_integrity(root, document, violations),
        Milestone::M7 => m7::verify_m7_integrity(root, document, violations),
        Milestone::M0
        | Milestone::M1
        | Milestone::M8
        | Milestone::M9
        | Milestone::M10
        | Milestone::M11
        | Milestone::M12
        | Milestone::M13 => {}
    }
}

pub(crate) const MANIFEST_INVARIANTS: &[(Rule, &str)] = &[
    (
        |value| value["revision"]["dirty"] == false,
        "milestone revision must be clean",
    ),
    (
        |value| {
            value.pointer("/review/result").is_some_and(Value::is_null)
                || value
                    .pointer("/review/record")
                    .is_some_and(Value::is_object)
        },
        "milestone recording a review verdict must record a hashed review record for it",
    ),
    (
        |value| !array(value, "/inputs").is_empty(),
        "milestone must include at least one hashed input",
    ),
    (commands_succeed, "milestone commands must all succeed"),
    (
        |value| {
            let tests = &value["tests"];
            match (
                tests["total"].as_u64(),
                tests["passed"].as_u64(),
                tests["failed"].as_u64(),
                tests["skipped"].as_u64(),
            ) {
                (Some(total), Some(passed), Some(failed), Some(skipped)) => {
                    passed
                        .checked_add(failed)
                        .and_then(|sum| sum.checked_add(skipped))
                        == Some(total)
                }
                _ => false,
            }
        },
        "test counts must equal passed + failed + skipped",
    ),
    (
        |value| {
            number(value, "/tests/total").is_some_and(|count| count > 0.0)
                && number(value, "/tests/passed").is_some_and(|count| count > 0.0)
        },
        "milestone must record at least one passing test",
    ),
    (
        |value| value["tests"]["failed"] == 0,
        "milestone tests must have zero failures",
    ),
    (
        |value| value["tests"]["skipped"] == 0,
        "milestone tests must have zero skipped tests",
    ),
    (
        |value| {
            number(value, "/tests/lineCoveragePercent").is_some_and(|coverage| coverage >= 80.0)
        },
        "milestone line coverage must be at least 80%",
    ),
];

fn commands_succeed(value: &Value) -> bool {
    let mut commands = array(value, "/commands").iter().chain(
        array(value, "/platformEvidence")
            .iter()
            .flat_map(|platform| array(platform, "/commands")),
    );
    commands.clone().next().is_some() && commands.all(|command| command["exitStatus"] == 0)
}

const MANIFEST_VERDICTS: &[(Rule, &str)] = &[
    (
        |value| {
            array(value, "/deviations")
                .iter()
                .chain(array(value, "/risks"))
                .all(|finding| finding["classification"] != "blocking")
        },
        "milestone must have no blocking deviations or risks",
    ),
    (
        |value| value["review"]["independent"] == true && value["review"]["result"] == "pass",
        "milestone review must be independent and pass",
    ),
];

fn verify_admitted_manifest(document: &Value, milestone: Milestone, violations: &mut Vec<String>) {
    require_manifest_rule(
        document,
        |value| value["evidenceKind"] == "native",
        "milestone closure requires native evidence, not a synthetic contract fixture",
        violations,
    );
    require_manifest_rule(
        document,
        |value| {
            !any_string(value, &|text| {
                text.to_ascii_lowercase().contains("synthetic")
            })
        },
        "native milestone evidence must not contain synthetic placeholders",
        violations,
    );
    if milestone == Milestone::M0 {
        require_manifest_rule(
            document,
            |value| {
                array(value, "/attestations").iter().any(|attestation| {
                    let mut workloads = array(attestation, "/workloads")
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>();
                    workloads.sort_unstable();
                    attestation["platform"] == "linux"
                        && attestation["architecture"] == "x86_64"
                        && array(attestation, "/workloads").len() == 2
                        && workloads == ["official-atm11", "vanilla"]
                })
            },
            "M0 closure requires a workload-bound native Linux attestation",
            violations,
        );
    }
}

fn any_string(value: &Value, predicate: &impl Fn(&str) -> bool) -> bool {
    match value {
        Value::String(text) => predicate(text),
        Value::Array(values) => values.iter().any(|value| any_string(value, predicate)),
        Value::Object(values) => values.values().any(|value| any_string(value, predicate)),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

pub(crate) fn verify_redaction_value(
    document: &Value,
    subject: &str,
    violations: &mut Vec<String>,
) {
    let sensitive = any_string(document, &|text| {
        let lower = text.to_ascii_lowercase();
        [
            "/home/",
            "/users/",
            ":\\users\\",
            "/.minecraft",
            "tests/fixtures/local/",
            "evidence/private/",
        ]
        .iter()
        .any(|pattern| lower.contains(pattern))
            || contains_credential(text)
    }) || credential_field(document, false);
    if sensitive {
        violations.push(format!("{subject} contains sensitive or local data"));
    }
}

fn credential_field(value: &Value, sensitive_path: bool) -> bool {
    match value {
        Value::String(text) => sensitive_path && !text.is_empty(),
        Value::Array(values) => values
            .iter()
            .any(|value| credential_field(value, sensitive_path)),
        Value::Object(values) => values.iter().any(|(name, value)| {
            let lower = name.to_ascii_lowercase();
            credential_field(
                value,
                sensitive_path || CREDENTIAL_WORDS.iter().any(|word| lower.contains(word)),
            )
        }),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

pub(crate) fn require_manifest_rule(
    manifest: &Value,
    rule: impl FnOnce(&Value) -> bool,
    violation: &str,
    violations: &mut Vec<String>,
) {
    if !rule(manifest) {
        violations.push(violation.to_owned());
    }
}

#[cfg(test)]
mod tests;
