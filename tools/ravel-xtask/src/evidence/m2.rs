//! M2 closure rules.

use crate::evidence::closure::{LoadedRecords, read_records};
use crate::evidence::rules::Milestone;
use std::path::Path;

use serde_json::Value;

use crate::evidence::closure::verify_closure_records;
use crate::evidence::rules::{RecordRule, require, require_manifest_rule, require_rows, unique_by};
use crate::evidence::rules::{has_exact_benchmark_results, has_exact_work_packages};
use crate::json::{array, nonempty, number};
use crate::text::read_text;

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

pub(crate) fn verify_m2_manifest(
    manifest: &Value,
    records: &LoadedRecords,
    violations: &mut Vec<String>,
) {
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
    verify_closure_records(Milestone::M2, records, M2_RECORDS, violations);
}

const M2_RECORDS: &[(&str, RecordRule, &str, bool)] = &[
    (
        "noop-pipeline.json",
        |value| {
            require_rows(value, "/runs", |run| {
                require(
                    run["repeatedRunIsIdentical"] == true,
                    "/repeatedRunIsIdentical",
                )
            })
        },
        "a no-op pipeline record whose run is identical when repeated",
        false,
    ),
    (
        "pre-world-gate.json",
        |value| {
            require(
                value["gate"]["refusesWorldAccess"] == true,
                "/gate/refusesWorldAccess",
            )?;
            require(value["gate"]["ranModCode"] == false, "/gate/ranModCode")
        },
        "a pre-world gate record showing invalid packages refused before any mod code ran",
        false,
    ),
    (
        "loader-overhead.json",
        m2_budget_limits,
        "a loader overhead record whose budget lines each name one metric and a unit, and set an \
     upper limit above the positive measurement that limit was derived from",
        true,
    ),
    (
        "prism-launch.json",
        m2_prism_launch,
        "a launch record showing the official client reaching Ravel from an attested stock Prism, \
     with the transformer's own count of what it rewrote inside that process and of the hook \
     invocations that reached native code, and no other loader present",
        false,
    ),
];

fn m2_budget_limits(value: &Value) -> Result<(), String> {
    let limits = array(value, "/budget/limits");
    require(
        unique_by(limits, |left, right| left["metric"] == right["metric"]),
        "/budget/limits unique metric",
    )?;
    require_rows(value, "/budget/limits", |limit| {
        require(nonempty(limit, "/metric"), "/metric")?;
        require(nonempty(limit, "/unit"), "/unit")?;
        require(
            limit["operator"] == "less-than-or-equal",
            "/operator must be less-than-or-equal",
        )?;
        require(
            number(limit, "/measured").is_some_and(|measured| measured > 0.0),
            "/measured must be positive",
        )?;
        require(
            number(limit, "/measured")
                .zip(number(limit, "/limit"))
                .is_some_and(|(measured, upper)| upper > measured),
            "/limit must exceed measured",
        )?;

        Ok(())
    })?;
    Ok(())
}

fn m2_prism_launch(value: &Value) -> Result<(), String> {
    let launch = &value["launch"];
    let rewrite = &value["ravel"]["rewrite"];
    require(
        launch["provesStockLaunch"] == true,
        "/launch/provesStockLaunch",
    )?;
    require(
        launch.get("objection").is_some_and(Value::is_null),
        "/launch/objection",
    )?;
    require(
        launch["attestation"]["stockLauncher"] == true,
        "/launch/attestation/stockLauncher",
    )?;
    require(
        value["ravel"]["reachedGameProcess"] == true,
        "/ravel/reachedGameProcess",
    )?;
    require(rewrite["reported"] == true, "/ravel/rewrite/reported")?;
    require(
        rewrite["reachedRavel"] == true,
        "/ravel/rewrite/reachedRavel",
    )?;
    require(
        number(rewrite, "/classesRewritten").is_some_and(|count| count > 0.0),
        "/ravel/rewrite/classesRewritten",
    )?;
    require(
        number(rewrite, "/methodsRewritten").is_some_and(|count| count > 0.0),
        "/ravel/rewrite/methodsRewritten",
    )?;
    require(
        number(rewrite, "/nativeInvocations").is_some_and(|count| count > 0.0),
        "/ravel/rewrite/nativeInvocations",
    )?;
    require(
        value["instance"]["withoutAnotherLoader"] == true,
        "/instance/withoutAnotherLoader",
    )?;
    Ok(())
}

pub(crate) fn verify_m2_integrity(
    root: &Path,
    _: &Value,
    violations: &mut Vec<String>,
) -> LoadedRecords {
    let records = read_records(root, Milestone::M2, M2_RECORDS, violations);
    if let Some(record) = records.get("loader-overhead.json") {
        verify_void_limits(root, record, violations);
    }
    records
}

pub(crate) fn verify_void_limits(root: &Path, record: &Value, violations: &mut Vec<String>) {
    let recorded = array(record, "/budget/limits");
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
