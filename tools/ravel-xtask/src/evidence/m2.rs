//! M2 closure rules.

use crate::evidence::closure::{LoadedRecords, RecordDefinition, read_records};
use crate::evidence::rules::Milestone;
use std::path::Path;

use serde_json::Value;

use crate::evidence::closure::{verify_closure_records, verify_manifest_closure};
use crate::evidence::model::admit;
use crate::evidence::rules::require;
use crate::evidence::rules::{Rule, has_exact_benchmark_results, has_exact_work_packages};
use crate::json::array;
use crate::text::read_text;

const BUDGET_RECORD: &str = "evidence/m2/loader-overhead.json";

mod model;

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

const M2_MANIFEST_RULES: &[(Rule, &str)] = &[
    (
        |value| {
            has_exact_work_packages(
                value,
                &[
                    "M2-W1", "M2-W2", "M2-W3", "M2-W4", "M2-W5", "M2-W6", "M2-W7", "M2-W8",
                ],
            )
        },
        "M2 manifest must include every work package from M2-W1 through M2-W8 exactly once",
    ),
    (
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
    ),
    (
        |value| {
            array(value, "/attestations").iter().any(|attestation| {
                attestation["platform"] == "linux" && attestation["architecture"] == "x86_64"
            })
        },
        "M2 closure requires a native Linux attestation for the operator-run launch",
    ),
    (
        |value| !array(value, "/deviations").is_empty() && !array(value, "/risks").is_empty(),
        "M2 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
    ),
];

pub(crate) fn verify_m2_manifest(
    manifest: &Value,
    records: &LoadedRecords,
    violations: &mut Vec<String>,
) {
    verify_manifest_closure(manifest, M2_MANIFEST_RULES, violations);
    for (metric, why) in VOID_LIMITS {
        println!(
            "- evidence/m2/manifest.json: the budget limit {metric} is checked for shape and not \
             for meaning, because it is {why}"
        );
    }
    verify_closure_records(Milestone::M2, records, M2_RECORDS, violations);
}

const M2_RECORDS: &[RecordDefinition] = &[
    RecordDefinition {
        name: "noop-pipeline.json",
        rule: |value| {
            let record: model::Noop = admit(value)?;
            for (index, run) in record.runs.0.iter().enumerate() {
                require(run.repeated_run_is_identical, "/repeatedRunIsIdentical")
                    .map_err(|error| format!("/runs/{index}{error}"))?;
            }
            Ok(())
        },
        description: "a no-op pipeline record whose run is identical when repeated",
        required: false,
    },
    RecordDefinition {
        name: "pre-world-gate.json",
        rule: |value| {
            let record: model::PreWorld = admit(value)?;
            require(record.gate.refuses_world_access, "/gate/refusesWorldAccess")?;
            require(!record.gate.ran_mod_code, "/gate/ranModCode")
        },
        description: "a pre-world gate record showing invalid packages refused before any mod code ran",
        required: false,
    },
    RecordDefinition {
        name: "loader-overhead.json",
        rule: verify_m2_budget_limits,
        description: "a loader overhead record whose budget lines each name one metric and a unit, and set an \
     upper limit above the positive measurement that limit was derived from",
        required: true,
    },
    RecordDefinition {
        name: "prism-launch.json",
        rule: verify_m2_prism_launch,
        description: "a launch record showing the official client reaching Ravel from an attested stock Prism, \
     with the transformer's own count of what it rewrote inside that process and of the hook \
     invocations that reached native code, and no other loader present",
        required: false,
    },
];

fn verify_m2_budget_limits(value: &Value) -> Result<(), String> {
    let record: model::BudgetRecord<'_> = admit(value)?;
    let limits = &record.budget.limits.0;
    require(
        limits.iter().enumerate().all(|(index, limit)| {
            limits[..index]
                .iter()
                .all(|other| other.metric.0 != limit.metric.0)
        }),
        "/budget/limits unique metric",
    )?;
    for (index, limit) in limits.iter().enumerate() {
        let measured = limit.measured;
        require(
            measured.is_finite() && measured > 0.0,
            "/measured must be positive",
        )
        .map_err(|error| format!("/budget/limits/{index}{error}"))?;
        require(
            limit.limit.is_finite() && limit.limit > measured,
            "/limit must exceed measured",
        )
        .map_err(|error| format!("/budget/limits/{index}{error}"))?;
    }
    Ok(())
}

fn verify_m2_prism_launch(value: &Value) -> Result<(), String> {
    let record: model::PrismProbe = admit(value)?;
    let rewrite = &record.ravel.rewrite;
    require(
        record.launch.proves_stock_launch,
        "/launch/provesStockLaunch",
    )?;
    require(
        record.launch.attestation.stock_launcher,
        "/launch/attestation/stockLauncher",
    )?;
    require(
        record.ravel.reached_game_process,
        "/ravel/reachedGameProcess",
    )?;
    require(rewrite.reported, "/ravel/rewrite/reported")?;
    require(rewrite.reached_ravel, "/ravel/rewrite/reachedRavel")?;
    for (count, requirement) in [
        (
            &rewrite.classes_rewritten,
            "/ravel/rewrite/classesRewritten",
        ),
        (
            &rewrite.methods_rewritten,
            "/ravel/rewrite/methodsRewritten",
        ),
        (
            &rewrite.native_invocations,
            "/ravel/rewrite/nativeInvocations",
        ),
    ] {
        require(*count > 0, requirement)?;
    }
    require(
        record.instance.without_another_loader,
        "/instance/withoutAnotherLoader",
    )
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
