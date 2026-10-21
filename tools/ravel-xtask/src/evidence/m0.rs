//! M0 closure rules.

use serde_json::Value;

use crate::evidence::closure::verify_manifest_closure;
use crate::evidence::rules::{Rule, has_exact_work_packages, require_manifest_rule};
use crate::json::array;

const M0_MANIFEST_RULES: &[(Rule, &str)] = &[
    (
        |value| {
            has_exact_work_packages(
                value,
                &[
                    "M0-W1", "M0-W2", "M0-W3", "M0-W4", "M0-W5", "M0-W6", "M0-W7", "M0-W8", "M0-W9",
                ],
            )
        },
        "M0 manifest must include every work package from M0-W1 through M0-W9 exactly once",
    ),
    (
        |value| {
            value["versions"]["minecraft"] == "26.1.2"
                && value["versions"]["neoforge"] == "26.1.2.78"
                && value["versions"]["fixture"] == "ATM11 0.2.1"
        },
        "M0 manifest must pin Minecraft 26.1.2, NeoForge 26.1.2.78, and ATM11 0.2.1",
    ),
    (
        |value| {
            array(value, "/platformEvidence")
                .iter()
                .any(m0_platform_matches)
        },
        "M0 manifest must include native Linux x86_64 launch evidence",
    ),
];

pub(crate) fn verify_m0_manifest(manifest: &Value, violations: &mut Vec<String>) {
    verify_manifest_closure(manifest, M0_MANIFEST_RULES, violations);
}

fn m0_platform_matches(value: &Value) -> bool {
    let versions = &value["versions"];
    value["platform"] == "linux"
        && value["architecture"] == "x86_64"
        && value["native"] == true
        && value["hardwareProfile"] == "linux-reference-v1"
        && versions["minecraft"] == "26.1.2"
        && versions["neoforge"] == "26.1.2.78"
        && versions["prism"] == "11.0.3"
        && versions["fixture"] == "ATM11 0.2.1"
        && versions["java"] == "25.0.1"
        && versions["rust"] == "1.95.0"
        && versions["nix"] == "2.33.3"
        && versions["devenv"] == "2.1.2"
        && value["settings"]
            .as_object()
            .is_some_and(|settings| !settings.is_empty())
        && array(value, "/commands")
            .iter()
            .any(|command| command["command"] == "devenv test --no-tui")
        && array(value, "/commands")
            .iter()
            .any(|command| command["command"] == "nix flake check --accept-flake-config")
        && array(value, "/baselines")
            .iter()
            .any(|baseline| baseline["workload"] == "vanilla")
        && array(value, "/baselines")
            .iter()
            .any(|baseline| baseline["workload"] == "official-atm11")
}

pub(crate) fn verify_m0_admission(document: &Value, violations: &mut Vec<String>) {
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

#[cfg(test)]
mod tests;
