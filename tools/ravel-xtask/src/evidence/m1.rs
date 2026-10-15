//! M1 closure rules.

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::evidence::manifest::require_manifest_rule;
use crate::evidence::spikes::SPIKE_GATES;
use crate::json::{array, nonempty, read_json};

pub(crate) fn verify_m1_manifest(root: &Path, manifest: &Value, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest,
        |value| {
            let packages = array(value, "/workPackages");
            let expected = [
                "M1-W1", "M1-W2", "M1-W3", "M1-W4", "M1-W5", "M1-W6", "M1-W7", "M1-W8",
            ];
            packages.len() == expected.len()
                && expected.iter().all(|name| {
                    packages
                        .iter()
                        .any(|package| package.as_str() == Some(*name))
                })
        },
        "M1 manifest must include every work package from M1-W1 through M1-W8 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest,
        |value| {
            value["benchmarks"]["protocolVersion"] == "spike-v1"
                && !array(value, "/benchmarks/results").is_empty()
        },
        "M1 manifest must record spike results under the spike-v1 protocol",
        violations,
    );
    require_manifest_rule(
        manifest,
        |value| !array(value, "/deviations").is_empty() && !array(value, "/risks").is_empty(),
        "M1 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
        violations,
    );

    verify_m1_spike_results(root, violations);
    verify_m1_manifest_repeated(root, violations);

    require_manifest_rule(
        manifest,
        |value| {
            array(value, "/attestations").iter().any(|attestation| {
                attestation["platform"] == "linux"
                    && attestation["architecture"] == "x86_64"
                    && !array(attestation, "/workloads").is_empty()
            })
        },
        "M1 closure requires a Linux attestation for the operator-run ADG-5 launch, which no \
         tracked record can recompute",
        violations,
    );

    let launched = fs::read_dir(root.join("evidence/m1/spikes/adg-5"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "json"))
        .any(|path| {
            read_json(&path).is_ok_and(|record| record["prismIntegration"]["stockLaunch"] == true)
        });
    if !launched {
        violations.push(
            "M1 closure requires an ADG-5 result recording an observed stock-Prism launch"
                .to_owned(),
        );
    }
}

fn verify_m1_spike_results(root: &Path, violations: &mut Vec<String>) {
    for (directory, gate) in SPIKE_GATES {
        let results = root.join("evidence/m1/spikes").join(directory);
        let recorded = fs::read_dir(&results)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
            })
            .count();
        if recorded == 0 {
            violations.push(format!("M1 closure requires a recorded result for {gate}"));
        }
    }
}

fn verify_m1_manifest_repeated(root: &Path, violations: &mut Vec<String>) {
    let repeated = fs::read_dir(root.join("evidence/m1/spikes/adg-3"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "json"))
        .any(|path| {
            read_json(&path).is_ok_and(|record| {
                let binding = &record["bindingGeneration"];
                nonempty(binding, "/outputSha256")
                    && binding["repeatedOutputSha256"] == binding["outputSha256"]
                    && binding["repeatedRunIsIdentical"] == true
            })
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

#[cfg(test)]
mod tests;
