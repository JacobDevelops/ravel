use std::fs;

use crate::evidence::manifest::MILESTONE_MANIFESTS;
use crate::scaffold::{
    declares_unsafe_prohibition, required_scaffold_paths, verify_experiment_fence,
    verify_ignored_boundaries, verify_scaffold, verify_testkit_fence,
};
use crate::test_support::temporary_root;

#[test]
fn production_crates_cannot_depend_on_measurement_experiments() {
    let root = temporary_root("fence");
    let crate_root = root.join("crates/ravel-loader");
    fs::create_dir_all(&crate_root).expect("crate directory should be creatable");
    fs::write(
        crate_root.join("Cargo.toml"),
        "[dependencies]\nravel-spike-harness.workspace = true\n",
    )
    .expect("manifest should be writable");
    let mut violations = Vec::new();

    verify_experiment_fence(&root, &mut violations);

    assert_eq!(
        violations,
        ["production crate depends on an experiment: crates/ravel-loader/Cargo.toml"]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn unsafe_code_cannot_be_permitted_outside_experiments() {
    let root = temporary_root("fence-unsafe");
    let crate_root = root.join("crates/ravel-bootstrap/src");
    fs::create_dir_all(&crate_root).expect("crate directory should be creatable");
    fs::write(
        crate_root.join("lib.rs"),
        "#![allow(unsafe_code)]\npub fn boundary() {}\n",
    )
    .expect("source should be writable");
    let mut violations = Vec::new();

    verify_experiment_fence(&root, &mut violations);

    assert_eq!(
        violations,
        ["unsafe code is permitted outside the experiment boundary: \
             crates/ravel-bootstrap/src/lib.rs"]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn missing_files_are_reported_by_relative_path() {
    let root = temporary_root("missing");

    let violations = verify_scaffold(&root);

    assert!(
        violations
            .iter()
            .any(|violation| violation == "missing required file: Cargo.toml")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn every_required_path_is_unique() {
    let mut paths = required_scaffold_paths();
    let original_len = paths.len();
    paths.sort_unstable();
    paths.dedup();

    assert_eq!(paths.len(), original_len);
}

#[test]
fn credential_ignores_are_required() {
    let root = temporary_root("ignores");
    fs::write(root.join(".gitignore"), "# intentionally incomplete\n")
        .expect("test gitignore should be writable");
    let mut violations = Vec::new();

    verify_ignored_boundaries(&root, &mut violations);

    assert_eq!(violations.len(), 1);
    assert!(
        violations
            .iter()
            .all(|violation| violation.starts_with("missing credential ignore:"))
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn every_milestone_manifest_is_required_by_name() {
    let required = required_scaffold_paths();

    for (relative, _) in MILESTONE_MANIFESTS {
        assert!(
            required.iter().any(|path| path == relative),
            "{relative} must be required by name"
        );
    }
}

#[test]
fn the_unsafe_prohibition_must_be_the_attribute_and_not_a_doc_comment() {
    assert!(declares_unsafe_prohibition("#![forbid(unsafe_code)]\n"));
    assert!(declares_unsafe_prohibition(
        "//! a crate\n\n    #![forbid(unsafe_code)]\n"
    ));
    assert!(!declares_unsafe_prohibition(
        "//! this crate keeps `#![forbid(unsafe_code)]` verbatim\npub fn f() {}\n"
    ));
}

#[test]
fn production_crates_cannot_depend_on_the_test_harness() {
    let root = temporary_root("fence-testkit");
    let crate_root = root.join("crates/ravel-loader");
    fs::create_dir_all(&crate_root).expect("crate directory should be creatable");
    fs::write(
        crate_root.join("Cargo.toml"),
        "[dependencies]\nravel-testkit.workspace = true\n",
    )
    .expect("manifest should be writable");
    let mut violations = Vec::new();

    verify_testkit_fence(&root, &mut violations);

    assert_eq!(
        violations,
        ["production crate depends on the test harness: crates/ravel-loader/Cargo.toml"]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
