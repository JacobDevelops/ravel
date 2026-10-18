use std::fs;
use std::path::PathBuf;

use crate::test_support::{PASS_RECORD, PASS_RECORD_DIGEST, RECORDED_RUN, temporary_root};

use super::{
    EMPTY_OBJECT_DIGEST, pinned_input_root, recorded_artifact_root, verify_record_bindings,
};

fn recorded_result_root(label: &str, result: &str, digest: &str) -> PathBuf {
    recorded_artifact_root(
        label,
        "result.json",
        result,
        &format!(
            r#"{{{RECORDED_RUN},"benchmarks":{{"protocolVersion":"v1",
                "results":[{{"name":"result.json","sha256":"{digest}"}}]}}}}"#
        ),
    )
}

#[test]
fn a_recorded_result_digest_is_recomputed_against_the_file() {
    let root = recorded_result_root("records-match", "{}\n", EMPTY_OBJECT_DIGEST);

    assert_eq!(verify_record_bindings(&root), Vec::<String>::new());
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_recorded_result_digest_that_binds_nothing_is_reported() {
    let root = recorded_result_root("records-drift", "{\"changed\": true}\n", &"0".repeat(64));

    let violations = verify_record_bindings(&root);

    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].starts_with("evidence/m9/manifest.json records result.json as ")
            && violations[0].contains(" but it hashes "),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_manifest_cannot_record_its_own_digest_or_a_result_it_does_not_hold() {
    let root = temporary_root("records-self");
    let milestone = root.join("evidence/m9");
    fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
    fs::write(
        milestone.join("manifest.json"),
        format!(
            r#"{{{RECORDED_RUN},"benchmarks":{{"protocolVersion":"v1","results":[
                {{"name":"manifest.json","sha256":"{EMPTY_OBJECT_DIGEST}"}},
                {{"name":"../m8/borrowed.json","sha256":"{EMPTY_OBJECT_DIGEST}"}},
                {{"name":"absent.json","sha256":"{EMPTY_OBJECT_DIGEST}"}}]}}}}"#
        ),
    )
    .expect("manifest should be writable");

    let violations = verify_record_bindings(&root);

    assert_eq!(
        violations,
        [
            "evidence/m9/manifest.json records its own digest, which cannot bind".to_owned(),
            "evidence/m9/manifest.json records a result outside its own milestone: \
             ../m8/borrowed.json"
                .to_owned(),
            "evidence/m9/manifest.json records a result that the repository does not \
             track, so its digest binds nothing a reader can obtain: absent.json"
                .to_owned(),
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn recorded_baseline_root(label: &str, result: &str, digest: &str) -> PathBuf {
    recorded_artifact_root(
        label,
        "launch.json",
        result,
        &format!(
            r#"{{{RECORDED_RUN},"platformEvidence":[{{"baselines":[
                {{"workload":"vanilla",
                "result":{{"name":"launch.json","sha256":"{digest}"}},
                "validation":{{"name":"private launch validation",
                    "sha256":"{}"}}}}]}}]}}"#,
            "0".repeat(64)
        ),
    )
}

#[test]
fn a_recorded_baseline_digest_is_recomputed_against_the_tracked_file() {
    let root = recorded_baseline_root("baseline-match", "{}\n", EMPTY_OBJECT_DIGEST);

    assert_eq!(verify_record_bindings(&root), Vec::<String>::new());
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_recorded_baseline_digest_that_binds_nothing_is_reported() {
    let root = recorded_baseline_root("baseline-drift", "{}\n", &"0".repeat(64));

    let violations = verify_record_bindings(&root);

    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].starts_with("evidence/m9/manifest.json records launch.json as ")
            && violations[0].ends_with(&format!(" but it hashes {EMPTY_OBJECT_DIGEST}")),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_baseline_naming_a_file_the_milestone_does_not_hold_is_reported() {
    let root = temporary_root("baseline-absent");
    let milestone = root.join("evidence/m9");
    fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
    fs::write(
        milestone.join("manifest.json"),
        format!(
            r#"{{{RECORDED_RUN},"platformEvidence":[{{"baselines":[
                {{"workload":"vanilla",
                "result":{{"name":"absent.json","sha256":"{EMPTY_OBJECT_DIGEST}"}},
                "validation":{{"name":"a private launch validation",
                    "sha256":"{EMPTY_OBJECT_DIGEST}"}}}}]}}]}}"#
        ),
    )
    .expect("manifest should be writable");

    let violations = verify_record_bindings(&root);

    assert_eq!(
        violations,
        [
            "evidence/m9/manifest.json records a baseline that the repository does not track, \
          so its digest binds nothing a reader can obtain: absent.json"
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_baseline_result_named_by_description_is_reported() {
    let root = temporary_root("baseline-prose");
    let milestone = root.join("evidence/m9");
    fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
    fs::write(
        milestone.join("manifest.json"),
        format!(
            r#"{{{RECORDED_RUN},"platformEvidence":[{{"baselines":[
                {{"workload":"vanilla",
                "result":{{"name":"the Linux vanilla launch summary",
                    "sha256":"{EMPTY_OBJECT_DIGEST}"}},
                "validation":{{"name":"a private launch validation",
                    "sha256":"{EMPTY_OBJECT_DIGEST}"}}}}]}}]}}"#
        ),
    )
    .expect("manifest should be writable");

    assert_eq!(
        verify_record_bindings(&root),
        [
            "evidence/m9/manifest.json records a baseline result by description rather than by \
          path, so its digest binds nothing obtainable: the Linux vanilla launch summary"
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_digest_binding_an_untracked_file_is_reported() {
    let (root, revision) = pinned_input_root("records-untracked");
    fs::write(root.join(".gitignore"), "ignored.json\n").expect("ignore should be writable");
    let milestone = root.join("evidence/m9");
    fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
    fs::write(milestone.join("ignored.json"), "{}\n").expect("result should be writable");
    fs::write(
        milestone.join("manifest.json"),
        format!(
            r#"{{{RECORDED_RUN},"revision":{{"id":"{revision}"}},
                "benchmarks":{{"protocolVersion":"v1",
                "results":[{{"name":"ignored.json",
                    "sha256":"{EMPTY_OBJECT_DIGEST}"}}]}}}}"#
        ),
    )
    .expect("manifest should be writable");

    assert_eq!(
        verify_record_bindings(&root),
        [
            "evidence/m9/manifest.json records a result that the repository does not track, so \
          its digest binds nothing a reader can obtain: ignored.json"
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn filenames_with_spaces_bind_results_baselines_and_reviews() {
    let root = recorded_artifact_root(
        "records-spaces",
        "launch result.json",
        "{}\n",
        &format!(
            r#"{{{RECORDED_RUN},"benchmarks":{{"results":[
                {{"name":"launch result.json","sha256":"{EMPTY_OBJECT_DIGEST}"}}]}},
                "platformEvidence":[{{"baselines":[{{"result":{{
                    "name":"launch result.json","sha256":"{EMPTY_OBJECT_DIGEST}"}}}}]}}],
                "review":{{"result":"pass","record":{{
                    "name":"closure review.md","sha256":"{PASS_RECORD_DIGEST}"}}}}}}"#
        ),
    );
    fs::write(root.join("evidence/m9/closure review.md"), PASS_RECORD)
        .expect("review should be writable");
    assert_eq!(verify_record_bindings(&root), Vec::<String>::new());
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
