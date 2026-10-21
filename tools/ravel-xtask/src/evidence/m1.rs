//! M1 closure rules.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::evidence::closure::verify_manifest_closure;
use crate::evidence::rules::{Rule, has_exact_work_packages};
use crate::evidence::spikes::{SPIKE_GATES, spike_result_paths};
use crate::json::{array, nonempty, read_json};

const M1_MANIFEST_RULES: &[(Rule, &str)] = &[
    (
        |value| {
            has_exact_work_packages(
                value,
                &[
                    "M1-W1", "M1-W2", "M1-W3", "M1-W4", "M1-W5", "M1-W6", "M1-W7", "M1-W8",
                ],
            )
        },
        "M1 manifest must include every work package from M1-W1 through M1-W8 exactly once",
    ),
    (
        |value| {
            value["benchmarks"]["protocolVersion"] == "spike-v1"
                && !array(value, "/benchmarks/results").is_empty()
        },
        "M1 manifest must record spike results under the spike-v1 protocol",
    ),
    (
        |value| !array(value, "/deviations").is_empty() && !array(value, "/risks").is_empty(),
        "M1 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
    ),
    (
        |value| {
            array(value, "/attestations").iter().any(|attestation| {
                attestation["platform"] == "linux"
                    && attestation["architecture"] == "x86_64"
                    && !array(attestation, "/workloads").is_empty()
            })
        },
        "M1 closure requires a Linux attestation for the operator-run ADG-5 launch, which no \
         tracked record can recompute",
    ),
];

pub(crate) fn verify_m1_manifest(root: &Path, manifest: &Value, violations: &mut Vec<String>) {
    verify_manifest_closure(manifest, M1_MANIFEST_RULES, violations);
    verify_m1_spike_results(root, violations);
}

fn verify_m1_spike_results(root: &Path, violations: &mut Vec<String>) {
    let paths: std::collections::BTreeMap<_, _> = SPIKE_GATES
        .iter()
        .map(|(directory, gate)| {
            let paths = spike_result_paths(root, directory, gate, violations);
            if paths.is_empty() {
                violations.push(format!("M1 closure requires a recorded result for {gate}"));
            }
            (*directory, paths)
        })
        .collect();
    let binding_paths = paths.get("adg-3").map(Vec::as_slice).unwrap_or_default();
    verify_m1_manifest_repeated(root, binding_paths, violations);
    let launch_paths = paths.get("adg-5").map(Vec::as_slice).unwrap_or_default();
    if !spike_records_match(root, launch_paths, violations, |record| {
        record["prismIntegration"]["stockLaunch"] == true
    }) {
        violations.push(
            "M1 closure requires an ADG-5 result recording an observed stock-Prism launch"
                .to_owned(),
        );
    }
}

fn verify_m1_manifest_repeated(root: &Path, paths: &[PathBuf], violations: &mut Vec<String>) {
    let repeated = spike_records_match(root, paths, violations, |record| {
        let binding = &record["bindingGeneration"];
        nonempty(binding, "/outputSha256")
            && binding["repeatedOutputSha256"] == binding["outputSha256"]
            && binding["repeatedRunIsIdentical"] == true
    });
    if !repeated {
        violations.push(
            "M1 closure requires an ADG-3 result recording bindingGeneration outputSha256 and \
             repeatedOutputSha256 as the same digest, which is the reproducibility half of \
             M1-W4's Verification clause"
                .to_owned(),
        );
    }
}

fn spike_records_match(
    root: &Path,
    paths: &[PathBuf],
    violations: &mut Vec<String>,
    predicate: fn(&Value) -> bool,
) -> bool {
    let mut matched = false;
    for path in paths {
        match read_json(root, path) {
            Ok(record) => matched |= predicate(&record),
            Err(error) => violations.push(error),
        }
    }
    matched
}

#[cfg(test)]
mod tests;
