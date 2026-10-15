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
        "ravel-bootstrap",
        "ravel-composed-host",
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

fn private_workspace_name() -> String {
    ["ravel-", "registry"].concat()
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
fn a_tracked_path_naming_the_private_workspace_is_refused() {
    let path = format!("docs/{}/notes.md", private_workspace_name());

    let violations = verify_tracked_paths([path.as_str()]);

    assert!(
        violations
            .iter()
            .any(|violation| violation.starts_with("tracked path names the private")),
        "expected the private-workspace path to be refused, got:\n{}",
        violations.join("\n")
    );
}

#[test]
fn a_tracked_file_naming_the_private_workspace_is_refused() {
    let root = temporary_root("named");
    let relative = "docs/architecture/public-private-boundary.md".to_owned();
    fs::create_dir_all(root.join("docs/architecture")).expect("the directory should be creatable");
    fs::write(
        root.join(&relative),
        format!(
            "The private workspace lives at ../{}.\n",
            private_workspace_name()
        ),
    )
    .expect("the document should be writable");

    let violations = verify_tracked_contents(&root, &[relative]);

    assert!(
        violations
            .iter()
            .any(|violation| violation.starts_with("tracked file names the private")),
        "expected the private-workspace reference to be refused, got:\n{}",
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
        "[dependencies]\nravel-core = { path = \"../ravel-core\" }\nother = { path = \"../../tools/ravel-cli\" }\n",
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
