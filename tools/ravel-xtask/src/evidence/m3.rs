//! M3 closure rules.

use crate::evidence::closure::{LoadedRecords, read_records};
use crate::evidence::rules::Milestone;
use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::evidence::closure::verify_closure_records;
use crate::evidence::rules::require_manifest_rule;
use crate::evidence::rules::{
    RecordRule, all_gates_closed, has_exact_benchmark_results, has_exact_work_packages,
    has_native_attestation, require,
};
use crate::json::{array, number};
use crate::text::read_text;
use crate::text::{Side, numbers_before, stated_counts};

const CAMPAIGN_MATRIX: &str = "evidence/m3/linux-process-matrix.json";
const CAMPAIGN_DOCUMENT: &str = "evidence/m3/README.md";
const PERSISTENCE_HARNESS: &str = "jvm/harness/run-harness.sh";

pub(crate) fn verify_m3_integrity(
    root: &Path,
    _: &Value,
    violations: &mut Vec<String>,
) -> LoadedRecords {
    let records = read_records(root, Milestone::M3, M3_RECORDS, violations);
    if let Some(record) = records.get("linux-process-matrix.json") {
        verify_campaign_inventory(root, record, violations);
    }
    verify_harness_lifecycles(root, &records, violations);
    records
}

pub(crate) fn verify_m3_manifest(
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
                    "M3-W0", "M3-W1", "M3-W2", "M3-W3", "M3-W4", "M3-W5", "M3-W6", "M3-W7", "M3-W8",
                ],
            )
        },
        "M3 manifest must include every work package from M3-W0 through M3-W8 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest,
        |value| {
            has_exact_benchmark_results(
                value,
                "m3-prototype-v1",
                &[
                    "class-loader-identity.json",
                    "linux-process-matrix.json",
                    "pack-compile.json",
                    "profiling-overhead.json",
                ],
            )
        },
        "M3 manifest must record all four M3 results under the m3-prototype-v1 protocol, so that each one is covered by a recomputed digest",
        violations,
    );
    require_manifest_rule(
        manifest,
        |value| all_gates_closed(value, "M3-W8", 8),
        "M3 closure requires a gateStatus row for every work package, each implementation row closed and naming its evidence",
        violations,
    );
    require_manifest_rule(
        manifest,
        |value| has_native_attestation(value, "ravel"),
        "M3 closure requires a native Linux attestation for the operator-run Ravel launch",
        violations,
    );
    require_manifest_rule(
        manifest,
        |value| !array(value, "/deviations").is_empty() && !array(value, "/risks").is_empty(),
        "M3 recorded deviations and risks, and an empty finding set satisfies every rule phrased as a refusal, so closure requires them to still be there",
        violations,
    );
    verify_closure_records(Milestone::M3, records, M3_RECORDS, violations);
}

const M3_RECORDS: &[(&str, RecordRule, &str, bool)] = &[
    (
        "class-loader-identity.json",
        |value| {
            require(
                value["observed"]["sameObject"] == true,
                "/observed/sameObject",
            )?;
            require(
                value["conclusion"]["candidateTwoIsAvailable"] == true,
                "/conclusion/candidateTwoIsAvailable",
            )?;
            for pointer in [
                "/rewrite/classesRewritten",
                "/rewrite/methodsRewritten",
                "/rewrite/nativeInvocations",
            ] {
                require(
                    number(value, pointer).is_some_and(|count| count > 0.0),
                    pointer,
                )?;
            }
            require(
                value["rewrite"]["reachedNativeCode"] == true,
                "/rewrite/reachedNativeCode",
            )?;
            Ok(())
        },
        "a class-loader identity record showing the loader that resolved a rewritten game class \
     is the system loader, measured in a launch that rewrote classes and reached native code",
        false,
    ),
    (
        "pack-compile.json",
        |value| {
            require(value["gate"] == "ADG-2", "/gate must be ADG-2")?;
            require(
                value["protocol"] == "m3-pack-compile-v1",
                "/protocol must be m3-pack-compile-v1",
            )?;
            for pointer in [
                "/compile/repeats",
                "/compile/baselineMilliseconds",
                "/compile/marginalColdMillisecondsPerMod",
                "/disk/toolchainBytes",
            ] {
                require(
                    number(value, pointer).is_some_and(|count| count > 0.0),
                    pointer,
                )?;
            }
            Ok(())
        },
        "a pack-compile record pricing ADG-2's source distribution with a repeated measurement, \
     a positive marginal per-mod cost, and the toolchain size the decision accepted",
        false,
    ),
    (
        "profiling-overhead.json",
        |value| {
            require(
                value["workPackage"] == "M3-W6",
                "/workPackage must be M3-W6",
            )?;
            require(
                value["budget"]["protocol"] == "m2-loader-budget-v1",
                "/budget/protocol must be m2-loader-budget-v1",
            )?;
            require(
                value["budget"]["hardwareProfile"] == "linux-reference-v1",
                "/budget/hardwareProfile must be linux-reference-v1",
            )?;
            profiling_off_tick_limit(value)?;
            require(
                !array(value, "/collectors/attributionAccuracy").is_empty(),
                "/collectors/attributionAccuracy",
            )?;
            require(
                value
                    .pointer("/collectors/unsupported")
                    .is_some_and(Value::is_array),
                "/collectors/unsupported",
            )?;
            Ok(())
        },
        "a profiling record whose off-tick measurement comes in under the limit M3-W6 fixed, with \
     per-mod attribution accuracy stated and the unsupported metrics named",
        false,
    ),
    (
        "linux-process-matrix.json",
        |value| {
            require(
                value["workPackage"] == "M3-W7",
                "/workPackage must be M3-W7",
            )?;
            require(value["result"] == "pass", "/result must be pass")?;
            for pointer in [
                "/instance/withoutAnotherLoader",
                "/instance/stockComponentsExact",
                "/screens/titleMenuButtonPresent",
                "/screens/allThreeOpenedInThisCampaign",
                "/screens/settings/persistenceRoundTrip",
                "/commandAndSavedData/vanillaCommandExecutedThroughInput",
                "/commandAndSavedData/relaunchVerifiedExactValue",
                "/worldPersistenceRoundTrip/createSaveExitReload/aggregateByteIdenticalAcrossTheRoundTrip",
                "/worldPersistenceRoundTrip/failClosedOnAnUnreadableWorld/worldRefused",
                "/worldPersistenceRoundTrip/failClosedOnAnUnreadableWorld/integratedServerNeverStarted",
            ] {
                require(value.pointer(pointer) == Some(&Value::Bool(true)), pointer)?;
            }
            for pointer in [
                "/instance/otherLoaderComponents",
                "/instance/modJars",
                "/instance/recognizedLoaderModJars",
                "/rewrite/transformationFailures",
            ] {
                require(number(value, pointer) == Some(0.0), pointer)?;
            }
            for pointer in ["/rewrite/classesRewritten", "/rewrite/nativeInvocations"] {
                require(
                    number(value, pointer).is_some_and(|count| count > 0.0),
                    pointer,
                )?;
            }
            Ok(())
        },
        "a Linux process-matrix record showing every M3 surface passing natively in an instance \
     the scan proves holds no other loader, with the world round trip preserving bytes and \
     refusing a world it cannot read",
        true,
    ),
];
fn profiling_off_tick_limit(value: &Value) -> Result<(), String> {
    let mut failure = Err("/budget/limits must contain metric profiling.off.perTick".to_owned());
    for (index, limit) in array(value, "/budget/limits")
        .iter()
        .enumerate()
        .filter(|(_, limit)| limit["metric"] == "profiling.off.perTick")
    {
        let checked = (|| {
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
                    .is_some_and(|(measured, bound)| measured <= bound),
                "/limit must not be below measured",
            )
        })();
        if checked.is_ok() {
            return Ok(());
        }
        failure = checked.map_err(|requirement| format!("/budget/limits/{index}{requirement}"));
    }
    failure
}

struct CampaignInventory {
    classes: usize,
    targets: usize,
    prior: usize,
    launches: usize,
}

pub(crate) fn verify_campaign_inventory(root: &Path, matrix: &Value, violations: &mut Vec<String>) {
    let Some(derived) = campaign_inventory(matrix, violations) else {
        return;
    };

    for (field, expected, subject) in [
        (
            "classesInInventory",
            derived.classes,
            "classes its own target inventory enumerates",
        ),
        (
            "callableTargetsInInventory",
            derived.targets,
            "callable targets its own target inventory enumerates",
        ),
    ] {
        match matrix_count(matrix, &format!("/rewrite/{field}")) {
            Ok(recorded) if recorded == expected => {}
            Ok(recorded) => violations.push(format!(
                "{CAMPAIGN_MATRIX} records {field} {recorded} against the {expected} {subject}"
            )),
            Err(error) => violations.push(error),
        }
    }

    match read_text(root, Path::new(CAMPAIGN_DOCUMENT)) {
        Ok(document) => violations.extend(campaign_document_violations(&document, &derived)),
        Err(error) => violations.push(error),
    }
}

fn campaign_inventory(matrix: &Value, violations: &mut Vec<String>) -> Option<CampaignInventory> {
    let Some(targets) = matrix
        .pointer("/rewrite/targetInventory")
        .and_then(Value::as_array)
    else {
        violations.push(format!(
            "{CAMPAIGN_MATRIX} does not record a target inventory array"
        ));
        return None;
    };
    let mut classes = BTreeSet::new();
    let mut methods = BTreeSet::new();
    for target in targets {
        let Some(class) = target["class"].as_str() else {
            violations.push(format!(
                "{CAMPAIGN_MATRIX} records a target without a class string"
            ));
            return None;
        };
        if !classes.insert(class) {
            violations.push(format!(
                "{CAMPAIGN_MATRIX}'s target inventory lists a class more than once"
            ));
            return None;
        }
        let Some(entries) = target["methods"].as_array() else {
            violations.push(format!(
                "{CAMPAIGN_MATRIX} records a target without a methods array"
            ));
            return None;
        };
        for method in entries {
            let Some(method) = method.as_str() else {
                violations.push(format!(
                    "{CAMPAIGN_MATRIX} records a non-string callable target"
                ));
                return None;
            };
            if !methods.insert(method) {
                violations.push(format!(
                    "{CAMPAIGN_MATRIX}'s target inventory lists a callable target more than once"
                ));
                return None;
            }
        }
    }
    match (
        matrix_count(matrix, "/matrixNotCarriedForward/priorCallableTargets"),
        matrix_count(matrix, "/surfaceCampaign/launches"),
    ) {
        (Ok(prior), Ok(launches)) => Some(CampaignInventory {
            classes: classes.len(),
            targets: methods.len(),
            prior,
            launches,
        }),
        (Err(error), _) | (_, Err(error)) => {
            violations.push(error);
            None
        }
    }
}

fn matrix_count(matrix: &Value, pointer: &str) -> Result<usize, String> {
    matrix
        .pointer(pointer)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("{CAMPAIGN_MATRIX} does not record a whole number at `{pointer}`"))
}

fn campaign_document_violations(document: &str, derived: &CampaignInventory) -> Vec<String> {
    let mut violations = Vec::new();
    let document = document.split_whitespace().collect::<Vec<_>>().join(" ");
    let document = document.as_str();
    for (phrase, side, expected, subject) in [
        (
            " callable targets",
            Side::Before,
            derived.targets,
            "callable targets",
        ),
        (
            " classes in the reviewed inventory",
            Side::Before,
            derived.classes,
            "classes in the reviewed inventory",
        ),
        (
            "moved from ",
            Side::After,
            derived.prior,
            "callable targets before the count moved",
        ),
    ] {
        let stated = stated_counts(document, phrase, side);
        if stated.is_empty() {
            violations.push(format!(
                "{CAMPAIGN_DOCUMENT} no longer writes a count against `{phrase}`, so nothing in it \
                 is held to {CAMPAIGN_MATRIX}'s {expected} {subject}"
            ));
            continue;
        }
        for count in stated {
            if count != expected {
                violations.push(format!(
                    "{CAMPAIGN_DOCUMENT} states {count} {subject} where {CAMPAIGN_MATRIX} records \
                     {expected}"
                ));
            }
        }
    }

    let stated = numbers_before(document, &["surface", "launches"]);
    if stated.is_empty() {
        violations.push(format!(
            "{CAMPAIGN_DOCUMENT} no longer writes a count against `surface launches`, so nothing \
             in it is held to {CAMPAIGN_MATRIX}'s {} launches in the surface campaign",
            derived.launches
        ));
    }
    for count in stated {
        if count != derived.launches {
            violations.push(format!(
                "{CAMPAIGN_DOCUMENT} states {count} surface launches where {CAMPAIGN_MATRIX} \
                 records {}",
                derived.launches
            ));
        }
    }
    violations
}
pub(crate) fn verify_harness_lifecycles(
    root: &Path,
    records: &LoadedRecords,
    violations: &mut Vec<String>,
) {
    let harness = match read_text(root, Path::new(PERSISTENCE_HARNESS)) {
        Ok(harness) => harness,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let declared = harness
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("persistence_lifecycles_declared="))
        .and_then(|value| value.parse::<usize>().ok());
    let Some(declared) = declared else {
        violations.push(format!(
            "{PERSISTENCE_HARNESS} no longer declares `persistence_lifecycles_declared`, so no \
             record's world-lifecycle count is held to it"
        ));
        return;
    };

    match read_text(root, Path::new(CAMPAIGN_DOCUMENT)) {
        Ok(text) => verify_lifecycle_counts(
            CAMPAIGN_DOCUMENT,
            &numbers_before(&text, &["world", "lifecycles"]),
            declared,
            violations,
        ),
        Err(error) => violations.push(error),
    }
    if let Some(record) = records.get("linux-process-matrix.json") {
        verify_lifecycle_counts(
            CAMPAIGN_MATRIX,
            &matrix_lifecycle_counts(record),
            declared,
            violations,
        );
    }
}

fn verify_lifecycle_counts(
    relative: &str,
    stated: &[usize],
    declared: usize,
    violations: &mut Vec<String>,
) {
    if stated.is_empty() {
        violations.push(format!(
            "{relative} no longer writes a count against `world lifecycles`, so \
             nothing in it is held to {PERSISTENCE_HARNESS}'s {declared}"
        ));
    }
    for count in stated {
        if *count != declared {
            violations.push(format!(
                "{relative} states {count} world lifecycles where \
                 {PERSISTENCE_HARNESS} declares {declared}"
            ));
        }
    }
}

fn matrix_lifecycle_counts(record: &Value) -> Vec<usize> {
    let mut counts = Vec::new();
    collect_lifecycle_counts(record, &mut counts);
    counts
}

fn collect_lifecycle_counts(value: &Value, counts: &mut Vec<usize>) {
    match value {
        Value::String(text) => counts.extend(numbers_before(text, &["world", "lifecycles"])),
        Value::Array(values) => {
            for value in values {
                collect_lifecycle_counts(value, counts);
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                collect_lifecycle_counts(value, counts);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

#[cfg(test)]
mod tests;
