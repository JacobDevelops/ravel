use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use crate::evidence::records;
use crate::test_support::{PASS_RECORD, PASS_RECORD_DIGEST, RECORDED_RUN, temporary_root};

use super::verify_record_bindings;

fn recorded_review_root(label: &str, record: &str, name: &str, digest: &str) -> PathBuf {
    reviewed_root(label, "pass", record, name, digest)
}

fn reviewed_root(label: &str, result: &str, record: &str, name: &str, digest: &str) -> PathBuf {
    let root = temporary_root(label);
    let milestone = root.join("evidence/m9");
    fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
    fs::write(milestone.join("review.md"), record).expect("record should be writable");
    fs::write(
        milestone.join("manifest.json"),
        format!(
            r#"{{{RECORDED_RUN},"review":{{"reviewer":"an independent reviewer",
                "independent":true,
                "result":"{result}","record":{{"name":"{name}","sha256":"{digest}"}}}}}}"#
        ),
    )
    .expect("manifest should be writable");
    root
}

#[test]
fn a_review_record_stating_another_verdict_than_the_manifest_is_reported() {
    for (result, record, digest) in [
        (
            "pass",
            "Result: FAIL\n",
            "5fcc61bd66ab279caaa7ec4d95d65cc532deee32d8a74ce93a04cb0cdd4ded44",
        ),
        ("fail", PASS_RECORD, PASS_RECORD_DIGEST),
    ] {
        let root = reviewed_root("review-verdict", result, record, "review.md", digest);

        let violations = verify_record_bindings(&root);

        let stated = if result == "pass" { "fail" } else { "pass" };
        assert_eq!(
            violations,
            [format!(
                "evidence/m9/manifest.json records review.result {result} while the review \
                 record it binds states {stated}"
            )],
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}

#[test]
fn a_review_record_stating_no_verdict_is_reported() {
    let root = recorded_review_root(
        "review-silent",
        "a record with no verdict\n",
        "review.md",
        "2f3630fd670787672c297ef4a9f7a4acb23eaa18c86ab35bc8940ca3363bde62",
    );

    let violations = verify_record_bindings(&root);

    assert_eq!(
        violations,
        [
            "evidence/m9/manifest.json binds a review record that states no verdict, so \
             nothing checks that review.result pass is what the review concluded"
                .to_owned()
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_recorded_review_digest_is_recomputed_against_the_tracked_record() {
    let root = recorded_review_root("review-match", PASS_RECORD, "review.md", PASS_RECORD_DIGEST);

    assert_eq!(verify_record_bindings(&root), Vec::<String>::new());
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_recorded_review_digest_that_binds_nothing_is_reported() {
    let root = recorded_review_root("review-drift", PASS_RECORD, "review.md", &"0".repeat(64));

    let violations = verify_record_bindings(&root);

    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].starts_with("evidence/m9/manifest.json records review.md as ")
            && violations[0].ends_with(&format!(" but it hashes {PASS_RECORD_DIGEST}")),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_review_record_named_by_title_is_reported() {
    let root = recorded_review_root(
        "review-title",
        PASS_RECORD,
        "M9 independent closure review",
        PASS_RECORD_DIGEST,
    );

    let violations = verify_record_bindings(&root);

    assert_eq!(
        violations,
        [
            "evidence/m9/manifest.json records a review record by title rather than by \
             path, so its digest binds nothing obtainable: M9 independent closure review"
                .to_owned()
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_manifest_recording_no_review_record_is_left_to_the_milestone_gate() {
    let root = temporary_root("review-absent");
    let milestone = root.join("evidence/m9");
    fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
    fs::write(
        milestone.join("manifest.json"),
        format!(
            r#"{{{RECORDED_RUN},"review":{{"reviewer":"an independent reviewer",
                "independent":true,"result":"fail"}}}}"#
        ),
    )
    .expect("manifest should be writable");

    assert_eq!(verify_record_bindings(&root), Vec::<String>::new());
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn absent_and_null_review_records_are_optional_but_malformed_records_are_not() {
    let root = temporary_root("records-review-option");
    let path = root.join("manifest.json");
    for (review, rejected) in [
        ("{}", false),
        (r#"{"record":null}"#, false),
        ("null", false),
        (r#"{"record":{"name":"review.md"}}"#, true),
    ] {
        fs::write(&path, format!(r#"{{"review":{review}}}"#)).expect("manifest should be writable");
        let mut violations = Vec::new();
        records::verify_recorded_review(&root, &path, &BTreeSet::new(), &mut violations);
        assert_eq!(!violations.is_empty(), rejected, "{review}: {violations:?}");
    }
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
