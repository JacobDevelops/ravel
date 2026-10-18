//! M2 closure rules.

use std::path::Path;

use serde_json::Value;

use crate::evidence::closure::verify_closure_records;
use crate::evidence::manifest::require_manifest_rule;
use crate::evidence::rules::{Rule, has_exact_benchmark_results, has_exact_work_packages};
use crate::json::{array, nonempty, number, read_json, read_text};

const BUDGET_RECORD: &str = "evidence/m2/loader-overhead.json";

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

pub(crate) fn verify_m2_manifest(root: &Path, manifest: &Value, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest,
        |value| {
            has_exact_work_packages(
                value,
                &[
                    "M2-W1", "M2-W2", "M2-W3", "M2-W4", "M2-W5", "M2-W6", "M2-W7", "M2-W8",
                ],
            )
        },
        "M2 manifest must include every work package from M2-W1 through M2-W8 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest,
        |value| {
            has_exact_benchmark_results(
                value,
                "m2-loader-budget-v1",
                &[
                    "loader-overhead.json",
                    "noop-pipeline.json",
                    "pre-world-gate.json",
                    "prism-launch.json",
                ],
            )
        },
        "M2 manifest must record all four M2 results under the m2-loader-budget-v1 protocol, so \
         that each one is covered by a recomputed digest",
        violations,
    );
    require_manifest_rule(
        manifest,
        |value| {
            array(value, "/attestations").iter().any(|attestation| {
                attestation["platform"] == "linux" && attestation["architecture"] == "x86_64"
            })
        },
        "M2 closure requires a native Linux attestation for the operator-run launch",
        violations,
    );
    require_manifest_rule(
        manifest,
        |value| !array(value, "/deviations").is_empty() && !array(value, "/risks").is_empty(),
        "M2 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
        violations,
    );
    for (metric, why) in VOID_LIMITS {
        println!(
            "- evidence/m2/manifest.json: the budget limit {metric} is checked for shape and not \
             for meaning, because it is {why}"
        );
    }
    verify_closure_records(root, "M2", "evidence/m2", M2_RECORDS, violations);
}

const M2_RECORDS: &[(&str, Rule, &str)] = &[
    (
        "noop-pipeline.json",
        |value| {
            let runs = array(value, "/runs");
            !runs.is_empty() && runs.iter().all(|run| run["repeatedRunIsIdentical"] == true)
        },
        "a no-op pipeline record whose run is identical when repeated",
    ),
    (
        "pre-world-gate.json",
        |value| value["gate"]["refusesWorldAccess"] == true && value["gate"]["ranModCode"] == false,
        "a pre-world gate record showing invalid packages refused before any mod code ran",
    ),
    (
        "loader-overhead.json",
        m2_budget_limits,
        "a loader overhead record whose budget lines each name one metric and a unit, and set an \
         upper limit above the positive measurement that limit was derived from",
    ),
    (
        "prism-launch.json",
        m2_prism_launch,
        "a launch record showing the official client reaching Ravel from an attested stock Prism, \
         with the transformer's own count of what it rewrote inside that process and of the hook \
         invocations that reached native code, and no other loader present",
    ),
];

fn m2_budget_limits(value: &Value) -> bool {
    let limits = array(value, "/budget/limits");
    !limits.is_empty()
        && limits.iter().enumerate().all(|(index, limit)| {
            nonempty(limit, "/metric")
                && nonempty(limit, "/unit")
                && limit["operator"] == "less-than-or-equal"
                && number(limit, "/measured").is_some_and(|measured| {
                    measured > 0.0 && number(limit, "/limit").is_some_and(|upper| upper > measured)
                })
                && !limits[..index]
                    .iter()
                    .any(|previous| previous["metric"] == limit["metric"])
        })
}

fn m2_prism_launch(value: &Value) -> bool {
    let launch = &value["launch"];
    let rewrite = &value["ravel"]["rewrite"];
    launch["provesStockLaunch"] == true
        && launch.get("objection").is_some_and(Value::is_null)
        && launch["attestation"]["stockLauncher"] == true
        && value["ravel"]["reachedGameProcess"] == true
        && rewrite["reported"] == true
        && rewrite["reachedRavel"] == true
        && number(rewrite, "/classesRewritten").is_some_and(|count| count > 0.0)
        && number(rewrite, "/methodsRewritten").is_some_and(|count| count > 0.0)
        && number(rewrite, "/nativeInvocations").is_some_and(|count| count > 0.0)
        && value["instance"]["withoutAnotherLoader"] == true
}

pub(crate) fn verify_m2_integrity(root: &Path, _: &Value, violations: &mut Vec<String>) {
    verify_void_limits(root, violations);
}

pub(crate) fn verify_void_limits(root: &Path, violations: &mut Vec<String>) {
    let record = match read_json(&root.join(BUDGET_RECORD)) {
        Ok(record) => record,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let recorded = array(&record, "/budget/limits");
    let roadmap = match read_text(root, Path::new("ROADMAP.md")) {
        Ok(roadmap) => roadmap,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let Some(declaration) = roadmap
        .split("\n\n")
        .find(|paragraph| paragraph.contains("are void"))
    else {
        violations.push(
            "ROADMAP.md declares no limit void, and evidence/m2/loader-overhead.json's expiry \
             clause voided three of its own"
                .to_owned(),
        );
        return;
    };
    for (metric, why) in VOID_LIMITS {
        if !recorded
            .iter()
            .any(|limit| limit["metric"].as_str() == Some(*metric))
        {
            violations.push(format!(
                "{metric} is treated as void and {BUDGET_RECORD} carries no limit for it, so the \
                 void set describes a record that is not there"
            ));
        }
        if !declaration.contains(metric) {
            violations.push(format!(
                "ROADMAP.md's declaration of which limits are void does not name {metric}, which \
                 is {why}; a limit the gate stops comparing and the roadmap stops declaring is \
                 indistinguishable from one that passed"
            ));
        }
    }
    for metric in recorded.iter().filter_map(|limit| limit["metric"].as_str()) {
        if VOID_LIMITS.iter().any(|(void, _)| *void == metric) {
            continue;
        }
        if declaration.contains(metric) {
            violations.push(format!(
                "ROADMAP.md declares {metric} void where it says which of {BUDGET_RECORD}'s limits \
                 are, and that limit still stands"
            ));
        }
    }
}

#[cfg(test)]
mod tests;
