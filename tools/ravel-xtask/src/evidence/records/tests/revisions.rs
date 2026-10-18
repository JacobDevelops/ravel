use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize as _;
use sha2::{Digest as _, Sha256};

use crate::evidence::records;
use crate::json::read_json;
use crate::test_support::{RECORDED_RUN, temporary_root};

use super::{HELLO_DIGEST, pinned_input_root, verify_record_bindings};

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
             {revision}: jj: Error: No such path: later.txt"
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

fn verify_admitted_revisions(root: &Path, path: &Path, violations: &mut Vec<String>) {
    let manifest = read_json(root, path).expect("manifest should parse");
    let artifacts = records::Artifacts::deserialize(&manifest).expect("artifacts should parse");
    records::verify_revisions(
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
    let manifest = read_json(root, path).expect("manifest should parse");
    let artifacts = records::Artifacts::deserialize(&manifest).expect("artifacts should parse");
    records::verify_inputs(
        root,
        path,
        &manifest,
        &artifacts.inputs,
        tracked,
        violations,
    );
}

fn git_input_root() -> (PathBuf, String) {
    let root = temporary_root("git-input-digests");
    fs::write(root.join("input.txt"), "hello\n").expect("input should be writable");
    for args in [
        &["init"][..],
        &["add", "input.txt"][..],
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-m",
            "pin input",
        ][..],
    ] {
        let output = Command::new("git")
            .args(args)
            .current_dir(&root)
            .output()
            .expect("Git fixture initialization command should run");
        assert!(output.status.success(), "{output:?}");
    }
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&root)
        .output()
        .expect("Git fixture revision command should run");
    assert!(output.status.success(), "{output:?}");
    let revision = String::from_utf8(output.stdout)
        .expect("Git fixture revision should be UTF-8")
        .trim()
        .to_owned();
    (root, revision)
}

#[test]
fn git_checkout_recomputes_input_digests_without_a_jj_workspace() {
    let (root, revision) = git_input_root();
    assert!(!root.join(".jj").exists());
    fs::write(root.join("input.txt"), "working tree differs\n")
        .expect("working input should be rewritable after pinning");
    let tracked = crate::vcs::tracked_files(&root, None)
        .expect("Git input inventory should be readable")
        .into_iter()
        .collect();
    let path = root.join("evidence/m9/manifest.json");
    let mut violations = Vec::new();
    write_input_manifest(&root, &revision, "input.txt", HELLO_DIGEST);
    verify_recorded_inputs(&root, &path, &tracked, &mut violations);
    assert!(violations.is_empty(), "{violations:?}");
    write_input_manifest(&root, &revision, "input.txt", &"0".repeat(64));
    verify_recorded_inputs(&root, &path, &tracked, &mut violations);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains(&format!("but revision {revision} holds {HELLO_DIGEST}")),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("Git digest fixture should be removable");
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
