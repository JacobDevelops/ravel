//! M0 closure rules.

use std::path::Path;

use crate::evidence::manifest::require_manifest_rule;

pub(crate) fn verify_m0_manifest(manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M0-W1\", \"M0-W2\", \"M0-W3\", \"M0-W4\", \"M0-W5\", \"M0-W6\", \"M0-W7\", \"M0-W8\", \"M0-W9\"]",
        "M0 manifest must include every work package from M0-W1 through M0-W9 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".versions.minecraft == \"26.1.2\" and .versions.neoforge == \"26.1.2.78\" and .versions.fixture == \"ATM11 0.2.1\"",
        "M0 manifest must pin Minecraft 26.1.2, NeoForge 26.1.2.78, and ATM11 0.2.1",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        &m0_platform_filter("linux", "x86_64", "linux-reference-v1"),
        "M0 manifest must include native Linux x86_64 launch evidence",
        violations,
    );
}

fn m0_platform_filter(platform: &str, architecture: &str, hardware_profile: &str) -> String {
    format!(
        r#".platformEvidence | any(
            .platform == "{platform}"
            and .architecture == "{architecture}"
            and .native == true
            and .hardwareProfile == "{hardware_profile}"
            and .versions.minecraft == "26.1.2"
            and .versions.neoforge == "26.1.2.78"
            and .versions.prism == "11.0.3"
            and .versions.fixture == "ATM11 0.2.1"
            and .versions.java == "25.0.1"
            and .versions.rust == "1.95.0"
            and .versions.nix == "2.33.3"
            and .versions.devenv == "2.1.2"
            and (.settings | length > 0)
            and any(.commands[]; .command == "devenv test --no-tui")
            and any(.commands[]; .command == "nix flake check --accept-flake-config")
            and any(.baselines[]; .workload == "vanilla")
            and any(.baselines[]; .workload == "official-atm11")
        )"#
    )
}
