use std::fs;

use std::path::{Path, PathBuf};

use ravel_xtask::{
    required_scaffold_paths, verify_scaffold, verify_tracked_contents, verify_tracked_paths,
};

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
fn scaffold_contract_does_not_require_decision_records() {
    assert!(
        required_scaffold_paths()
            .iter()
            .all(|path| !path.starts_with("docs/decisions/")),
        "decision records are not part of the scaffold contract"
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
        "ravel-bootstrap-runtime",
        "ravel-composed-host",
        "ravel-minecraft",
        "ravel-minecraft-adapter",
        "ravel-minecraft-generate",
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
fn scaffold_contract_covers_every_maintained_test_mod() {
    let required = required_scaffold_paths();

    for test_mod in [
        "audio",
        "client-only",
        "commands",
        "configuration",
        "events",
        "inter-mod-service",
        "interception",
        "networking",
        "noop",
        "optional-service",
        "provision",
        "registries",
        "render-stages",
        "resources",
        "saved-data",
        "server-data",
        "server-only",
        "waystone-ledger",
        "world-shaping",
    ] {
        let manifest = format!("test-mods/{test_mod}/Cargo.toml");
        assert!(
            required.contains(&manifest),
            "missing contract for {manifest}"
        );
    }
    // M4 reads the historical amendment tree, not placeholder crates in the current tree.
    for retired in ["dependency-base", "dependency-consumer", "failure-cases"] {
        let manifest = format!("test-mods/{retired}/Cargo.toml");
        assert!(
            !required.contains(&manifest),
            "retired contract: {manifest}"
        );
        assert!(
            !Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(manifest)
                .exists()
        );
    }
}

#[test]
fn scaffold_contract_covers_every_registry_mod() {
    let required = required_scaffold_paths();

    for name in [
        "rvl-002-counterpart",
        "rvl-025-counterpart",
        "rvl-042-counterpart",
        "rvl-060-counterpart",
        "rvl-086-counterpart",
        "rvl-120-counterpart",
        "rvl-132-counterpart",
        "rvl-146-counterpart",
        "rvl-180-counterpart",
        "rvl-210-counterpart",
        "rvl-287-counterpart",
        "rvl-321-counterpart",
    ] {
        for file in ["Cargo.toml", "src/lib.rs"] {
            let relative = format!("registry/mods/{name}/{file}");
            assert!(
                required.contains(&relative),
                "missing contract for {relative}"
            );
        }
    }
}

fn temporary_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "ravel-boundary-{label}-{}-{}",
        std::process::id(),
        line!()
    ));
    fs::create_dir_all(&root).expect("the temporary root should be creatable");
    root
}

#[test]
fn tracked_registry_paths_and_historical_names_are_allowed() {
    let violations = verify_tracked_paths([
        "registry/mods/rvl-002-counterpart/Cargo.toml",
        "registry/mods/rvl-002-counterpart/src/lib.rs",
        "docs/ravel-registry/notes.md",
    ]);

    assert!(
        violations.is_empty(),
        "registry paths and historical names are allowed:\n{}",
        violations.join("\n")
    );
}

#[test]
fn runtime_directory_names_in_rust_modules_do_not_admit_runtime_data() {
    assert_eq!(
        verify_tracked_paths(["crates/ravel-loader/src/saves/save_data_tests.rs"]),
        Vec::<String>::new()
    );
    let runtime_paths = [
        "crates/ravel-loader/src/saves/level.dat",
        "crates/ravel-loader/saves/world.rs",
        "saves/world.rs",
        "test-mods/example/saves/world.rs",
        "crates/ravel-loader/src/logs/latest.rs",
        "tools/ravel-xtask/src/crash-reports/report.rs",
        "crates/../src/saves/world.rs",
        "crates/ravel-loader/src/../saves/world.rs",
        "crates/ravel-loader/src/./saves/world.rs",
        "crates/ravel-loader/src//saves/world.rs",
    ];
    assert_eq!(
        verify_tracked_paths(runtime_paths),
        runtime_paths.map(|path| format!("forbidden tracked path: {path}"))
    );
}

#[test]
fn tracked_registry_documentation_and_historical_names_are_allowed() {
    let root = temporary_root("named");
    let relative = "docs/architecture/public-private-boundary.md".to_owned();
    fs::create_dir_all(root.join("docs/architecture")).expect("the directory should be creatable");
    fs::write(
        root.join(&relative),
        "Counterpart crates live in registry/mods, imported from ravel-registry.\n",
    )
    .expect("the document should be writable");

    let violations = verify_tracked_contents(&root, &[relative]);

    assert!(
        violations.is_empty(),
        "registry documentation and historical names are allowed:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(&root).expect("the temporary root should be removable");
}

#[test]
fn a_manifest_path_dependency_inside_the_repository_is_allowed() {
    let root = temporary_root("inside");
    let relative = "crates/ravel-loader/Cargo.toml".to_owned();
    fs::create_dir_all(root.join("crates/ravel-loader"))
        .expect("the directory should be creatable");
    fs::write(
        root.join(&relative),
        "[dependencies]\nravel-core = { path = \"../ravel-core\" }\nother = { path = \"../../tools/ravel-xtask\" }\n",
    )
    .expect("the manifest should be writable");

    let violations = verify_tracked_contents(&root, &[relative]);

    assert!(
        violations.is_empty(),
        "a sibling dependency is not an escape:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(&root).expect("the temporary root should be removable");
}

#[test]
fn a_manifest_path_dependency_outside_the_repository_is_refused() {
    let root = temporary_root("outside");
    let relative = "crates/ravel-loader/Cargo.toml".to_owned();
    fs::create_dir_all(root.join("crates/ravel-loader"))
        .expect("the directory should be creatable");
    fs::write(
        root.join(&relative),
        "[dependencies]\ninventory = { path = \"../../../elsewhere/inventory\" }\n",
    )
    .expect("the manifest should be writable");

    let violations = verify_tracked_contents(&root, &[relative]);

    assert!(
        violations
            .iter()
            .any(|violation| violation.starts_with("tracked manifest depends on a path outside")),
        "expected the escaping dependency to be refused, got:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(&root).expect("the temporary root should be removable");
}
