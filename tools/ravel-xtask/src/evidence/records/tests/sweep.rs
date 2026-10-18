use std::fs;

use crate::test_support::{temporary_root, track_fixture_files};

use super::pinned_input_root;

#[test]
fn the_full_sweep_validates_m6_integrity_not_only_artifact_binding() {
    let (root, _) = pinned_input_root("records-m6-integrity");
    fs::create_dir_all(root.join("schemas")).expect("schemas should be creatable");
    fs::write(
        root.join("schemas/evidence.schema.json"),
        include_str!("../../../../../../schemas/evidence.schema.json"),
    )
    .expect("schema should be writable");
    let directory = root.join("evidence/m6");
    fs::create_dir_all(&directory).expect("milestone should be creatable");
    fs::write(
        directory.join("manifest.json"),
        r#"{"milestone":"M6","schemaVersion":2,"revision":{"dirty":true},
            "commands":[{"exitStatus":0}],
            "tests":{"total":1,"passed":1,"failed":0,"skipped":0,"lineCoveragePercent":90},
            "notes":"evidence/private/redaction-canary"}"#,
    )
    .expect("manifest should be writable");
    track_fixture_files(&root);
    let violations = crate::verify_evidence_records(&root);
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("does not conform to evidence schema")),
        "{violations:?}"
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("clean")),
        "{violations:?}"
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("sensitive or local data")),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn the_full_sweep_rejects_unknown_milestone_directories() {
    let root = temporary_root("records-unknown-milestone");
    let directory = root.join("evidence/m99");
    fs::create_dir_all(&directory).expect("directory should be creatable");
    fs::write(directory.join("manifest.json"), "{}\n").expect("manifest should be writable");
    track_fixture_files(&root);
    let violations = crate::verify_evidence_records(&root);
    assert!(!violations.is_empty(), "{violations:?}");
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("milestone")),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
