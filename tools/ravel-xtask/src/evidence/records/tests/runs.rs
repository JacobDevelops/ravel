use std::fs;
use std::path::PathBuf;

use crate::test_support::temporary_root;

use super::verify_record_bindings;

fn recorded_run_root(label: &str, run: &str) -> PathBuf {
    let root = temporary_root(label);
    let milestone = root.join("evidence/m9");
    fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
    fs::write(milestone.join("manifest.json"), run).expect("manifest should be writable");
    root
}

#[test]
fn a_recorded_run_whose_numbers_agree_is_accepted() {
    let root = recorded_run_root(
        "run-consistent",
        r#"{"commands":[{"exitStatus":0}],
            "tests":{"total":9,"passed":9,"failed":0,"skipped":0,
                "lineCoveragePercent":85.97}}"#,
    );

    assert_eq!(verify_record_bindings(&root), Vec::<String>::new());
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_recorded_run_whose_counts_do_not_sum_is_reported() {
    let root = recorded_run_root(
        "run-arithmetic",
        r#"{"commands":[{"exitStatus":0}],
            "tests":{"total":8,"passed":9,"failed":0,"skipped":0,
                "lineCoveragePercent":85.97}}"#,
    );

    assert_eq!(
        verify_record_bindings(&root),
        ["evidence/m9/manifest.json records test counts that do not sum to its total"]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_recorded_run_with_no_test_result_and_no_coverage_is_reported() {
    let root = recorded_run_root("run-untested", r#"{"commands":[{"exitStatus":1}]}"#);

    assert_eq!(
        verify_record_bindings(&root),
        [
            "evidence/m9/manifest.json records a gate run but no test result",
            "evidence/m9/manifest.json records a gate run that did not succeed",
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_recorded_run_below_the_coverage_floor_is_reported() {
    let root = recorded_run_root(
        "run-coverage",
        r#"{"tests":{"total":9,"passed":9,"failed":0,"skipped":0,
            "lineCoveragePercent":79.9}}"#,
    );

    assert_eq!(
        verify_record_bindings(&root),
        ["evidence/m9/manifest.json records line coverage below the floor the gate enforces"]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn an_evidence_tree_holding_no_manifest_is_reported() {
    let root = temporary_root("records-none");
    fs::create_dir_all(root.join("evidence")).expect("evidence directory should be creatable");

    let violations = verify_record_bindings(&root);

    assert_eq!(
        violations,
        [
            "evidence/ holds no milestone manifest, and a manifest sweep over nothing passes \
          vacuously"
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_manifest_recording_neither_a_run_nor_a_test_result_is_reported() {
    let root = recorded_run_root("run-silent", r#"{"milestone":"M9"}"#);

    assert_eq!(
        verify_record_bindings(&root),
        [
            "evidence/m9/manifest.json records neither a gate run nor a test result, so there is \
          nothing here a gate can hold it to"
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn an_unparseable_manifest_cannot_answer_a_check() {
    let root = recorded_run_root("records-unparseable", "{not json\n");

    let violations = verify_record_bindings(&root);

    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("cannot parse JSON")),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
