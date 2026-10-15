//! M1 closure rules.

use std::fs;
use std::path::Path;

use crate::evidence::manifest::require_manifest_rule;
use crate::json::jq_matches;
use crate::spikes::SPIKE_GATES;

pub(crate) fn verify_m1_manifest(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M1-W1\", \"M1-W2\", \"M1-W3\", \"M1-W4\", \"M1-W5\", \"M1-W6\", \"M1-W7\", \"M1-W8\"]",
        "M1 manifest must include every work package from M1-W1 through M1-W8 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".benchmarks.protocolVersion == \"spike-v1\" and (.benchmarks.results | length > 0)",
        "M1 manifest must record spike results under the spike-v1 protocol",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "(.deviations | length) > 0 and (.risks | length) > 0",
        "M1 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
        violations,
    );

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

    verify_m1_manifest_repeated(root, violations);

    require_manifest_rule(
        manifest_path,
        r#".attestations
            | any(.platform == "linux" and .architecture == "x86_64" and (.workloads | length > 0))"#,
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
        .any(|path| jq_matches(&path, ".prismIntegration.stockLaunch == true").unwrap_or(false));
    if !launched {
        violations.push(
            "M1 closure requires an ADG-5 result recording an observed stock-Prism launch"
                .to_owned(),
        );
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
            jq_matches(
                &path,
                r#"(.bindingGeneration.outputSha256 // "") != ""
                   and .bindingGeneration.repeatedOutputSha256 == .bindingGeneration.outputSha256
                   and .bindingGeneration.repeatedRunIsIdentical == true"#,
            )
            .unwrap_or(false)
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
