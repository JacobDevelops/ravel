//! Manifest identity and milestone dispatch.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::evidence::closure::LoadedRecords;
use crate::evidence::rules::{
    Milestone, Rule, any_string, evidence_schema_matches, milestone_manifests,
    require_manifest_rule, verify_redaction_value,
};
use crate::evidence::{findings, m0, m1, m2, m3, m4, m5, m6, m7, records};
use crate::json::{array, number, read_json};

use crate::tracked::tracked_paths;

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
                let records =
                    verify_manifest_integrity(root, &path, milestone, &manifest, &mut violations);
                records::verify_recorded_manifest(
                    root,
                    &path,
                    &manifest,
                    &tracked,
                    &records,
                    &mut violations,
                );
            }
            Err(error) => violations.push(error),
        }
    }
    violations
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
    super::accounting::verify_accounting(root, Some(milestone), &mut violations);
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
    let records = verify_manifest_integrity(root, path, milestone, document, &mut violations);
    if !document.is_object() {
        return violations;
    }
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
        Milestone::M2 => m2::verify_m2_manifest(document, &records, &mut violations),
        Milestone::M3 => m3::verify_m3_manifest(document, &records, &mut violations),
        Milestone::M4 => m4::verify_m4_manifest(document, &records, &mut violations),
        Milestone::M5 => m5::verify_m5_manifest(document, &records, &mut violations),
        Milestone::M6 => m6::verify_m6_manifest(document, &records, &mut violations),
        Milestone::M7 => m7::verify_m7_manifest(document, &records, &mut violations),
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
) -> LoadedRecords {
    if !document.is_object() {
        violations.push("milestone manifest must be a JSON object".to_owned());
        return LoadedRecords::default();
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
        | Milestone::M13 => LoadedRecords::default(),
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
        m0::verify_m0_admission(document, violations);
    }
}

#[cfg(test)]
mod tests;
