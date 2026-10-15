use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::evidence::findings::{
    defined_rust_symbols, distinguishable_symbol, named_rust_symbols, sources_naming,
};
use crate::evidence::manifest::verify_milestone;
use crate::evidence::records::visible_commit;
use crate::test_support::{PASS_RECORD, PASS_RECORD_DIGEST, RECORDED_RUN, temporary_root};

fn pinned_record_root(label: &str) -> PathBuf {
    let root = temporary_root(label);
    let initialised = Command::new("jj")
        .args(["git", "init"])
        .current_dir(&root)
        .output()
        .expect("jj should be available");
    assert!(initialised.status.success(), "{initialised:?}");
    root
}

#[test]
fn an_abandoned_commit_is_not_visible_although_jj_still_reads_it() {
    let root = pinned_record_root("visible-abandoned");
    fs::write(root.join("input.txt"), "hello\n").expect("input should be writable");
    let abandoned = visible_commit(&root, "@").expect("the working copy should be visible");
    let described = Command::new("jj")
        .args(["describe", "-m", "moves the change off that commit"])
        .current_dir(&root)
        .output()
        .expect("jj should be available");
    assert!(described.status.success(), "{described:?}");
    let readable = Command::new("jj")
        .args(["file", "show", "-r", &abandoned, "--", "input.txt"])
        .current_dir(&root)
        .output()
        .expect("jj should be available");
    assert!(readable.status.success(), "{readable:?}");
    assert_eq!(
        visible_commit(&root, &abandoned),
        Err("names no commit in visible history".to_owned())
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_missing_or_root_revision_is_not_visible() {
    let root = pinned_record_root("visible-bogus");
    for revision in ["1".repeat(40), "0".repeat(40)] {
        assert!(visible_commit(&root, &revision).is_err());
    }
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn milestone_manifest_must_match_requested_milestone() {
    let root = temporary_root("milestone");
    let evidence = root.join("evidence/m0");
    fs::create_dir_all(&evidence).expect("evidence directory should be creatable");
    fs::write(
        evidence.join("manifest.json"),
        r#"{"schemaVersion":1,"milestone":"M1"}"#,
    )
    .expect("manifest should be writable");

    let violations = verify_milestone(&root, "M0");

    assert!(
        violations
            .iter()
            .any(|violation| violation == "manifest milestone must equal M0")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn milestone_review_root(label: &str, record: &str, digest: &str) -> PathBuf {
    let root = temporary_root(label);
    let milestone = root.join("evidence/m0");
    fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
    fs::write(milestone.join("review.md"), record).expect("record should be writable");
    fs::write(
        milestone.join("manifest.json"),
        format!(
            r#"{{"schemaVersion":1,"milestone":"M0",{RECORDED_RUN},
                "review":{{"reviewer":"an independent reviewer","independent":true,
                "result":"pass","record":{{"name":"review.md","sha256":"{digest}"}}}}}}"#
        ),
    )
    .expect("manifest should be writable");
    root
}

#[test]
fn the_milestone_gate_recomputes_the_review_record_it_closes_on() {
    let substituted = "Result: PASS\nNo round ever ran.\n";
    let hashes = "4f682a1f88a7dbba60f3046cc667ef8fcc6876c6eda74b90dc97a4ddca688091";
    let root = milestone_review_root("milestone-review", substituted, &"0".repeat(64));

    let violations = verify_milestone(&root, "M0");

    assert!(
        violations.iter().any(|violation| {
            violation.starts_with("evidence/m0/manifest.json records review.md as ")
                && violation.ends_with(&format!(" but it hashes {hashes}"))
        }),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn the_milestone_gate_accepts_a_review_record_that_hashes_as_recorded() {
    let root = milestone_review_root("milestone-review-match", PASS_RECORD, PASS_RECORD_DIGEST);

    let violations = verify_milestone(&root, "M0");

    assert!(
        !violations
            .iter()
            .any(|violation| violation.contains("review.md")),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn malformed_milestone_ids_are_rejected() {
    let root = temporary_root("milestone-id");

    let violations = verify_milestone(&root, "M14");

    assert_eq!(violations, ["invalid milestone ID: M14"]);
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_symbol_a_finding_could_not_have_meant_is_not_a_symbol() {
    assert!(!distinguishable_symbol("open"));
    assert!(!distinguishable_symbol("run"));
    assert!(distinguishable_symbol("verify_tracked_contents"));
    assert!(distinguishable_symbol("LoaderRuntime"));
}

#[test]
fn the_tree_decides_what_a_finding_named_rather_than_its_author() {
    let root = temporary_root("m5-defined-symbols");
    fs::write(
        root.join("source.rs"),
        "fn verify_something_narrow() {}\nstruct WidgetHandle;\nfn go() {}\n",
    )
    .expect("source should be writable");
    let sources = vec!["source.rs".to_owned()];

    let defined = defined_rust_symbols(&root, &sources);

    assert!(defined.contains("verify_something_narrow"));
    assert!(defined.contains("WidgetHandle"));
    assert!(
        !defined.contains("go"),
        "a two-letter item is below the floor"
    );
    assert_eq!(
        named_rust_symbols("the WidgetHandle is never built", &defined),
        vec!["WidgetHandle".to_owned()]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn an_anchor_goes_stale_when_a_caller_appears_outside_its_path_set() {
    let root = temporary_root("m5-anchor-stale");
    fs::create_dir_all(root.join("elsewhere")).expect("directory should be creatable");
    fs::write(root.join("source.rs"), "fn verify_something_narrow() {}\n")
        .expect("source should be writable");
    fs::write(
        root.join("elsewhere/caller.rs"),
        "fn call() { verify_something_narrow(); }\n",
    )
    .expect("source should be writable");
    let sources = vec!["source.rs".to_owned(), "elsewhere/caller.rs".to_owned()];

    let naming = sources_naming(&root, &sources, "verify_something_narrow");

    assert_eq!(naming, vec!["source.rs", "elsewhere/caller.rs"]);
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
