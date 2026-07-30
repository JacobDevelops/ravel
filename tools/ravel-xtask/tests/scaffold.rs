use std::path::Path;

use ravel_xtask::{required_scaffold_paths, verify_scaffold};

#[test]
fn required_scaffold_is_complete() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("xtask must remain under tools/ravel-xtask");

    let violations = verify_scaffold(root);

    assert!(
        violations.is_empty(),
        "scaffold violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn scaffold_contract_covers_every_planned_crate() {
    let required = required_scaffold_paths();

    for crate_name in [
        "ravel-core",
        "ravel-package",
        "ravel-resolve",
        "ravel-loader",
        "ravel-sdk",
        "ravel-sdk-macros",
        "ravel-host-api",
        "ravel-bootstrap",
        "ravel-minecraft",
        "ravel-minecraft-adapter",
        "ravel-config",
        "ravel-data",
        "ravel-save",
        "ravel-network",
        "ravel-observability",
        "ravel-pack",
        "ravel-testkit",
    ] {
        let manifest = format!("crates/{crate_name}/Cargo.toml");
        assert!(
            required.contains(&manifest),
            "missing contract for {manifest}"
        );
    }
}

#[test]
fn scaffold_contract_covers_every_original_test_mod() {
    let required = required_scaffold_paths();

    for test_mod in [
        "commands",
        "configuration",
        "dependency-base",
        "dependency-consumer",
        "events",
        "failure-cases",
        "inter-mod-service",
        "networking",
        "noop",
        "registries",
        "resources",
        "saved-data",
    ] {
        let manifest = format!("test-mods/{test_mod}/Cargo.toml");
        assert!(
            required.contains(&manifest),
            "missing contract for {manifest}"
        );
    }
}
