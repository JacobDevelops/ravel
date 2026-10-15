//! M3 closure rules.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::evidence::accounting::{Side, numbers_before, stated_counts};
use crate::evidence::closure::verify_closure_records;
use crate::evidence::manifest::require_manifest_rule;
use crate::json::{Rule, array, closed_gates, number, read_json, work_packages};

const CAMPAIGN_MATRIX: &str = "evidence/m3/linux-process-matrix.json";
const CAMPAIGN_DOCUMENT: &str = "evidence/m3/README.md";
const PERSISTENCE_HARNESS: &str = "jvm/harness/run-harness.sh";

pub(crate) fn verify_m3_integrity(root: &Path, _: &Value, violations: &mut Vec<String>) {
    verify_campaign_inventory(root, violations);
    verify_harness_lifecycles(root, violations);
}

pub(crate) fn verify_m3_manifest(root: &Path, manifest: &Value, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest,
        |value| {
            work_packages(
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
            let mut names = array(value, "/benchmarks/results")
                .iter()
                .filter_map(|row| row["name"].as_str())
                .collect::<Vec<_>>();
            names.sort_unstable();
            value["benchmarks"]["protocolVersion"] == "m3-prototype-v1"
                && array(value, "/benchmarks/results").len() == 4
                && names
                    == [
                        "class-loader-identity.json",
                        "linux-process-matrix.json",
                        "pack-compile.json",
                        "profiling-overhead.json",
                    ]
        },
        "M3 manifest must record all four M3 results under the m3-prototype-v1 protocol, so that each one is covered by a recomputed digest",
        violations,
    );
    require_manifest_rule(
        manifest,
        |value| closed_gates(value, "M3-W8", 8),
        "M3 closure requires a gateStatus row for every work package, each implementation row closed and naming its evidence",
        violations,
    );
    require_manifest_rule(
        manifest,
        |value| {
            array(value, "/attestations").iter().any(|row| {
                row["platform"] == "linux"
                    && row["architecture"] == "x86_64"
                    && array(row, "/workloads")
                        .iter()
                        .any(|workload| workload == "ravel")
            })
        },
        "M3 closure requires a native Linux attestation for the operator-run Ravel launch",
        violations,
    );
    require_manifest_rule(
        manifest,
        |value| !array(value, "/deviations").is_empty() && !array(value, "/risks").is_empty(),
        "M3 recorded deviations and risks, and an empty finding set satisfies every rule phrased as a refusal, so closure requires them to still be there",
        violations,
    );
    verify_closure_records(root, "M3", "evidence/m3", M3_RECORDS, violations);
}

const M3_RECORDS: &[(&str, Rule, &str)] = &[
    (
        "class-loader-identity.json",
        |value| {
            value["milestone"] == "M3"
                && value["observed"]["sameObject"] == true
                && value["conclusion"]["candidateTwoIsAvailable"] == true
                && [
                    "/rewrite/classesRewritten",
                    "/rewrite/methodsRewritten",
                    "/rewrite/nativeInvocations",
                ]
                .iter()
                .all(|pointer| number(value, pointer).is_some_and(|count| count > 0.0))
                && value["rewrite"]["reachedNativeCode"] == true
        },
        "a class-loader identity record showing the loader that resolved a rewritten game class \
         is the system loader, measured in a launch that rewrote classes and reached native code",
    ),
    (
        "pack-compile.json",
        |value| {
            value["milestone"] == "M3"
                && value["gate"] == "ADG-2"
                && value["protocol"] == "m3-pack-compile-v1"
                && [
                    "/compile/repeats",
                    "/compile/baselineMilliseconds",
                    "/compile/marginalColdMillisecondsPerMod",
                    "/disk/toolchainBytes",
                ]
                .iter()
                .all(|pointer| number(value, pointer).is_some_and(|count| count > 0.0))
        },
        "a pack-compile record pricing ADG-2's source distribution with a repeated measurement, \
         a positive marginal per-mod cost, and the toolchain size the decision accepted",
    ),
    (
        "profiling-overhead.json",
        |value| {
            value["milestone"] == "M3"
                && value["workPackage"] == "M3-W6"
                && value["budget"]["protocol"] == "m2-loader-budget-v1"
                && value["budget"]["hardwareProfile"] == "linux-reference-v1"
                && array(value, "/budget/limits").iter().any(|limit| {
                    limit["metric"] == "profiling.off.perTick"
                        && limit["operator"] == "less-than-or-equal"
                        && number(limit, "/measured")
                            .zip(number(limit, "/limit"))
                            .is_some_and(|(measured, bound)| measured > 0.0 && measured <= bound)
                })
                && !array(value, "/collectors/attributionAccuracy").is_empty()
                && value
                    .pointer("/collectors/unsupported")
                    .is_some_and(Value::is_array)
        },
        "a profiling record whose off-tick measurement comes in under the limit M3-W6 fixed, with \
         per-mod attribution accuracy stated and the unsupported metrics named",
    ),
    (
        "linux-process-matrix.json",
        |value| {
            value["milestone"] == "M3"
            && value["workPackage"] == "M3-W7"
            && value["result"] == "pass"
            && ["/instance/withoutAnotherLoader", "/instance/stockComponentsExact",
                "/screens/titleMenuButtonPresent", "/screens/allThreeOpenedInThisCampaign",
                "/screens/settings/persistenceRoundTrip",
                "/commandAndSavedData/vanillaCommandExecutedThroughInput",
                "/commandAndSavedData/relaunchVerifiedExactValue",
                "/worldPersistenceRoundTrip/createSaveExitReload/aggregateByteIdenticalAcrossTheRoundTrip",
                "/worldPersistenceRoundTrip/failClosedOnAnUnreadableWorld/worldRefused",
                "/worldPersistenceRoundTrip/failClosedOnAnUnreadableWorld/integratedServerNeverStarted"]
                .iter().all(|pointer| value.pointer(pointer) == Some(&Value::Bool(true)))
            && ["/instance/otherLoaderComponents", "/instance/modJars",
                "/instance/recognizedLoaderModJars", "/rewrite/transformationFailures"]
                .iter().all(|pointer| number(value, pointer) == Some(0.0))
            && ["/rewrite/classesRewritten", "/rewrite/nativeInvocations"]
                .iter().all(|pointer| number(value, pointer).is_some_and(|count| count > 0.0))
        },
        "a Linux process-matrix record showing every M3 surface passing natively in an instance \
         the scan proves holds no other loader, with the world round trip preserving bytes and \
         refusing a world it cannot read",
    ),
];
struct CampaignInventory {
    classes: usize,
    targets: usize,
    prior: usize,
    launches: usize,
}

pub(crate) fn verify_campaign_inventory(root: &Path, violations: &mut Vec<String>) {
    let matrix = root.join(CAMPAIGN_MATRIX);
    if !matrix.is_file() {
        violations.push(format!("missing required file: {CAMPAIGN_MATRIX}"));
        return;
    }
    let matrix = match read_json(&matrix) {
        Ok(matrix) => matrix,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let Some(derived) = campaign_inventory(&matrix, violations) else {
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
        match matrix_count(&matrix, &format!("/rewrite/{field}")) {
            Ok(recorded) if recorded == expected => {}
            Ok(recorded) => violations.push(format!(
                "{CAMPAIGN_MATRIX} records {field} {recorded} against the {expected} {subject}"
            )),
            Err(error) => violations.push(error),
        }
    }

    match fs::read_to_string(root.join(CAMPAIGN_DOCUMENT)) {
        Ok(document) => violations.extend(campaign_document_violations(&document, &derived)),
        Err(error) => violations.push(format!("cannot read {CAMPAIGN_DOCUMENT}: {error}")),
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
pub(crate) fn verify_harness_lifecycles(root: &Path, violations: &mut Vec<String>) {
    let harness = match fs::read_to_string(root.join(PERSISTENCE_HARNESS)) {
        Ok(harness) => harness,
        Err(error) => {
            violations.push(format!("cannot read {PERSISTENCE_HARNESS}: {error}"));
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

    for relative in [CAMPAIGN_DOCUMENT, CAMPAIGN_MATRIX] {
        match fs::read_to_string(root.join(relative)) {
            Ok(document) => {
                let stated = numbers_before(&document, &["world", "lifecycles"]);
                if stated.is_empty() {
                    violations.push(format!(
                        "{relative} no longer writes a count against `world lifecycles`, so \
                         nothing in it is held to {PERSISTENCE_HARNESS}'s {declared}"
                    ));
                }
                for count in stated {
                    if count != declared {
                        violations.push(format!(
                            "{relative} states {count} world lifecycles where \
                             {PERSISTENCE_HARNESS} declares {declared}"
                        ));
                    }
                }
            }
            Err(error) => violations.push(format!("cannot read {relative}: {error}")),
        }
    }
}

#[cfg(test)]
mod tests;
