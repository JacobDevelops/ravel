//! M0 closure rules.

use serde_json::Value;

use crate::evidence::manifest::require_manifest_rule;
use crate::json::array;

pub(crate) fn verify_m0_manifest(manifest: &Value, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest,
        |value| {
            let packages = array(value, "/workPackages");
            let expected = [
                "M0-W1", "M0-W2", "M0-W3", "M0-W4", "M0-W5", "M0-W6", "M0-W7", "M0-W8", "M0-W9",
            ];
            packages.len() == expected.len()
                && expected.iter().all(|name| {
                    packages
                        .iter()
                        .any(|package| package.as_str() == Some(*name))
                })
        },
        "M0 manifest must include every work package from M0-W1 through M0-W9 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest,
        |value| {
            value["versions"]["minecraft"] == "26.1.2"
                && value["versions"]["neoforge"] == "26.1.2.78"
                && value["versions"]["fixture"] == "ATM11 0.2.1"
        },
        "M0 manifest must pin Minecraft 26.1.2, NeoForge 26.1.2.78, and ATM11 0.2.1",
        violations,
    );
    require_manifest_rule(
        manifest,
        |value| {
            array(value, "/platformEvidence")
                .iter()
                .any(m0_platform_matches)
        },
        "M0 manifest must include native Linux x86_64 launch evidence",
        violations,
    );
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

#[cfg(test)]
mod tests;
