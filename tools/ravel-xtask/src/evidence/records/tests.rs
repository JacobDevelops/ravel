use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize as _;
use sha2::{Digest as _, Sha256};

use crate::json::read_json;
use crate::test_support::{
    PASS_RECORD, PASS_RECORD_DIGEST, RECORDED_RUN, temporary_root, track_fixture_files,
};
use crate::tracked::tracked_paths;

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

const EMPTY_OBJECT_DIGEST: &str =
    "ca3d163bab055381827226140568f3bef7eaac187cebd76878e0b63e9e442356";

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

fn write_admitted(root: &Path, pin: &str, revision: &str) {
    let milestone = root.join("evidence/m9");
    fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
    let record = format!("{{\"trial\":{{\"revision\":{pin}}}}}\n");
    fs::write(milestone.join("record.json"), &record).expect("record should be writable");
    fs::write(
        milestone.join("manifest.json"),
        format!(
            r#"{{{RECORDED_RUN},"revision":{{"id":"{revision}"}},
                "benchmarks":{{"protocolVersion":"v1",
                "results":[{{"name":"record.json","sha256":"{}"}}]}}}}"#,
            crate::hex_digest(&Sha256::digest(record.as_bytes()))
        ),
    )
    .expect("manifest should be writable");
}

#[test]
fn admitted_trial_revisions_require_a_boolean_dirty_state() {
    for (label, pin, accepted) in [
        ("missing", "{}", false),
        ("null", r#"{"dirty":null}"#, false),
        ("string", r#"{"dirty":"false"}"#, false),
        ("number", r#"{"dirty":0}"#, false),
        ("clean", r#"{"dirty":false}"#, true),
        ("dirty", r#"{"dirty":true}"#, true),
    ] {
        let root = temporary_root(&format!("trial-dirty-{label}"));
        write_admitted(&root, pin, "@");
        let mut violations = Vec::new();

        verify_admitted_revisions(
            &root,
            &root.join("evidence/m9/manifest.json"),
            &mut violations,
        );

        if accepted {
            assert_eq!(violations, Vec::<String>::new(), "{label}");
        } else {
            assert_eq!(
                violations,
                [
                    "evidence/m9/manifest.json admits record.json, whose trial at \"trial.revision\" \
                  does not declare whether the tree it measured was dirty"
                ],
                "{label}"
            );
        }
        let record_path = root.join("evidence/m9/record.json");
        let record = fs::read_to_string(&record_path).expect("record should be readable");
        fs::write(&record_path, format!(r#"{{"a\nb":{record}}}"#))
            .expect("nested record should be writable");
        violations.clear();
        verify_admitted_revisions(
            &root,
            &root.join("evidence/m9/manifest.json"),
            &mut violations,
        );
        assert_eq!(violations.len(), usize::from(!accepted), "{label}");
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}

#[test]
fn a_tracked_record_its_manifest_does_not_name_is_reported() {
    let (root, revision) = pinned_input_root("record-unnamed");
    write_admitted(&root, r#"{"dirty":false}"#, &revision);
    fs::write(root.join("evidence/m9/unnamed.json"), "{}\n").expect("record should be writable");

    let violations = verify_record_bindings(&root);

    assert_eq!(
        violations,
        [
            "evidence/m9/manifest.json names no record unnamed.json, which its own milestone \
             directory tracks; a record no manifest names is a record no digest covers"
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

const HELLO_DIGEST: &str = "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03";

fn pinned_input_root(label: &str) -> (PathBuf, String) {
    let root = temporary_root(label);
    let initialised = Command::new("jj")
        .args(["git", "init"])
        .current_dir(&root)
        .output()
        .expect("jj should be available");
    assert!(initialised.status.success(), "{initialised:?}");
    fs::write(root.join("input.txt"), "hello\n").expect("input should be writable");
    let pinned = Command::new("jj")
        .args(["log", "-r", "@", "--no-graph", "-T", "commit_id"])
        .current_dir(&root)
        .output()
        .expect("jj should be available");
    assert!(pinned.status.success(), "{pinned:?}");
    let advanced = Command::new("jj")
        .arg("new")
        .current_dir(&root)
        .output()
        .expect("jj should be available");
    assert!(advanced.status.success(), "{advanced:?}");
    fs::write(root.join("input.txt"), "moved on\n").expect("input should be rewritable");
    (
        root,
        String::from_utf8_lossy(&pinned.stdout).trim().to_owned(),
    )
}

fn write_input_manifest(root: &Path, revision: &str, name: &str, digest: &str) {
    let milestone = root.join("evidence/m9");
    fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
    fs::write(
        milestone.join("manifest.json"),
        format!(
            r#"{{{RECORDED_RUN},"revision":{{"id":"{revision}"}},
                "inputs":[{{"name":"{name}","sha256":"{digest}"}}]}}"#
        ),
    )
    .expect("manifest should be writable");
}

#[test]
fn an_input_digest_is_recomputed_against_the_revision_the_manifest_pins() {
    let (root, revision) = pinned_input_root("inputs-match");
    write_input_manifest(&root, &revision, "input.txt", HELLO_DIGEST);

    assert_eq!(verify_record_bindings(&root), Vec::<String>::new());
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn an_input_digest_that_binds_nothing_at_the_pinned_revision_is_reported() {
    let (root, revision) = pinned_input_root("inputs-drift");
    write_input_manifest(&root, &revision, "input.txt", &"0".repeat(64));

    let violations = verify_record_bindings(&root);

    assert_eq!(
        violations,
        [format!(
            "evidence/m9/manifest.json records input input.txt as {} but revision {revision} \
             holds {HELLO_DIGEST}",
            "0".repeat(64)
        )]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn an_input_the_pinned_revision_does_not_hold_is_reported() {
    let (root, revision) = pinned_input_root("inputs-absent");
    fs::write(root.join("later.txt"), "hello\n").expect("input should be writable");
    write_input_manifest(&root, &revision, "later.txt", HELLO_DIGEST);

    let violations = verify_record_bindings(&root);

    assert_eq!(
        violations,
        [format!(
            "evidence/m9/manifest.json records input later.txt: cannot read it at revision \
             {revision}"
        )]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn an_input_the_repository_no_longer_holds_is_reported() {
    let (root, revision) = pinned_input_root("inputs-deleted");
    fs::remove_file(root.join("input.txt")).expect("input should be removable");
    write_input_manifest(&root, &revision, "input.txt", HELLO_DIGEST);

    let violations = verify_record_bindings(&root);

    assert_eq!(
        violations,
        [
            "evidence/m9/manifest.json records input input.txt, which the repository no longer \
          holds, so the digest binds a path no reader can resolve"
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn an_input_revision_must_name_a_visible_commit_not_a_revset() {
    for revision in ["@", "root()"] {
        let (root, _) = pinned_input_root(&format!("inputs-revset-{revision}"));
        write_input_manifest(&root, revision, "input.txt", HELLO_DIGEST);
        let mut violations = Vec::new();
        verify_recorded_inputs(
            &root,
            &root.join("evidence/m9/manifest.json"),
            &BTreeSet::from(["input.txt".to_owned()]),
            &mut violations,
        );
        assert_eq!(violations.len(), 1, "{revision}: {violations:?}");
        assert!(
            violations[0].starts_with("evidence/m9/manifest.json revision.id "),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}

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

fn recorded_artifact_root(label: &str, file: &str, record: &str, manifest: &str) -> PathBuf {
    let root = temporary_root(label);
    let milestone = root.join("evidence/m9");
    fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
    fs::write(milestone.join(file), record).expect("record should be writable");
    fs::write(milestone.join("manifest.json"), manifest).expect("manifest should be writable");
    root
}

// These fixtures exercise artifact binding without claiming complete milestone integrity.
fn verify_record_bindings(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    let manifests = fs::read_dir(root.join("evidence"))
        .expect("evidence directory should be readable")
        .map(|entry| {
            entry
                .expect("entry should be readable")
                .path()
                .join("manifest.json")
        })
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    if manifests.is_empty() {
        return vec![
            "evidence/ holds no milestone manifest, and a manifest sweep over nothing passes \
             vacuously"
                .to_owned(),
        ];
    }
    track_fixture_files(root);
    let tracked = tracked_paths(root)
        .expect("tracked paths should be readable")
        .into_iter()
        .collect();
    for path in manifests {
        match read_json(&path) {
            Ok(manifest) => {
                super::verify_recorded_manifest(
                    root,
                    &path,
                    &manifest,
                    &tracked,
                    &crate::evidence::closure::LoadedRecords::default(),
                    &mut violations,
                );
            }
            Err(error) => violations.push(error),
        }
    }
    violations
}

fn verify_admitted_revisions(root: &Path, path: &Path, violations: &mut Vec<String>) {
    let manifest = read_json(path).expect("manifest should parse");
    let artifacts = super::Artifacts::deserialize(&manifest).expect("artifacts should parse");
    super::verify_revisions(
        root,
        path,
        &artifacts,
        &crate::evidence::closure::LoadedRecords::default(),
        violations,
    );
}

fn verify_recorded_inputs(
    root: &Path,
    path: &Path,
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let manifest = read_json(path).expect("manifest should parse");
    let artifacts = super::Artifacts::deserialize(&manifest).expect("artifacts should parse");
    super::verify_inputs(
        root,
        path,
        &manifest,
        &artifacts.inputs,
        tracked,
        violations,
    );
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

#[test]
fn a_literal_null_revision_is_resolved_instead_of_treated_as_absent() {
    let (root, _) = pinned_input_root("inputs-literal-null");
    write_input_manifest(&root, "null", "input.txt", HELLO_DIGEST);
    let mut violations = Vec::new();
    verify_recorded_inputs(
        &root,
        &root.join("evidence/m9/manifest.json"),
        &BTreeSet::from(["input.txt".to_owned()]),
        &mut violations,
    );
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].starts_with("evidence/m9/manifest.json revision.id null "),
        "{violations:?}"
    );
    assert!(
        !violations[0].contains("no revision to hash"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn the_full_sweep_validates_m6_integrity_not_only_artifact_binding() {
    let (root, _) = pinned_input_root("records-m6-integrity");
    fs::create_dir_all(root.join("schemas")).expect("schemas should be creatable");
    fs::write(
        root.join("schemas/evidence.schema.json"),
        include_str!("../../../../../schemas/evidence.schema.json"),
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
        super::verify_recorded_review(&root, &path, &BTreeSet::new(), &mut violations);
        assert_eq!(!violations.is_empty(), rejected, "{review}: {violations:?}");
    }
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
