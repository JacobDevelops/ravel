use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::evidence::findings::{
    defined_rust_symbols, distinguishable_symbol, named_rust_symbols, sources_naming,
};
use crate::evidence::manifest::verify_milestone;
use crate::evidence::records::visible_commit;
use crate::evidence::rules::{Milestone, verify_redaction_value};
use crate::test_support::{
    PASS_RECORD, PASS_RECORD_DIGEST, RECORDED_RUN, temporary_root, track_fixture_files,
};

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
    track_fixture_files(&root);
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
fn milestone_ids_round_trip_through_the_typed_identity() {
    for (text, expected) in [
        ("M0", Milestone::M0),
        ("M1", Milestone::M1),
        ("M2", Milestone::M2),
        ("M3", Milestone::M3),
        ("M4", Milestone::M4),
        ("M5", Milestone::M5),
        ("M6", Milestone::M6),
        ("M7", Milestone::M7),
        ("M8", Milestone::M8),
        ("M9", Milestone::M9),
        ("M10", Milestone::M10),
        ("M11", Milestone::M11),
        ("M12", Milestone::M12),
        ("M13", Milestone::M13),
    ] {
        let parsed = text
            .parse::<Milestone>()
            .expect("known milestone should parse");
        assert_eq!(parsed, expected);
        assert_eq!(parsed.as_str(), text);
        assert_eq!(parsed.to_string(), text);
    }
    for text in [
        "", "M", "m0", "M00", "M01", "M14", "M-1", " M0", "M0 ", "../M0",
    ] {
        assert_eq!(
            text.parse::<Milestone>(),
            Err(format!("invalid milestone ID: {text}"))
        );
    }
}

#[test]
fn redaction_rejects_sensitive_values_nested_in_objects_and_arrays() {
    for value in [
        serde_json::json!({"outer": [{"inner": "/home/operator/instance"}]}),
        serde_json::json!({"outer": [{"inner": "tests/fixtures/local/private.json"}]}),
        serde_json::json!({"outer": [{"inner": r"C:\Users\operator\instance"}]}),
        serde_json::json!({"outer": [{"inner": r"D:\Users\operator\instance"}]}),
        serde_json::json!({"outer": [{"inner": "D:/Users/operator/instance"}]}),
        serde_json::json!({"outer": [{"inner": "evidence/private/capture.log"}]}),
        serde_json::json!({"outer": [{"inner": {"password": (["synthetic", "value"].join("-"))}}]}),
        serde_json::json!({"outer": {"accessToken": [{"nested": "secret"}]}}),
    ] {
        let mut violations = Vec::new();
        verify_redaction_value(&value, "record", &mut violations);
        assert_eq!(
            violations,
            ["record contains sensitive or local data"],
            "{value}"
        );
    }
}

#[test]
fn redaction_uses_the_shared_credential_detector_for_ordinary_strings() {
    for text in [
        [
            "eyJhbGciOiJIUzI1NiJ9",
            "eyJzdWIiOiJmaXh0dXJlIn0",
            "signature",
        ]
        .join("."),
        ["ghp", "fixture-value"].join("_"),
    ] {
        assert!(ravel_core::contains_credential(&text));
        let mut violations = Vec::new();
        verify_redaction_value(
            &serde_json::json!({"outer": [{"inner": text}]}),
            "record",
            &mut violations,
        );
        assert_eq!(violations, ["record contains sensitive or local data"]);
    }
}

#[test]
fn redaction_preserves_safe_nested_values_and_empty_credential_fields() {
    let value = serde_json::json!({
        "outer": [{"inner": "evidence/m4/manifest.json"}, null, false, 42],
        "password": {"empty": ["", null, false, 0]},
        "escaped\"key": {"line\nbreak": "public evidence"},
        "description": "The token and password fields are empty; access_token is an identifier.",
    });
    let original = value.clone();
    let mut violations = Vec::new();

    verify_redaction_value(&value, "record", &mut violations);

    assert!(violations.is_empty(), "{violations:?}");
    assert_eq!(value, original);
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

#[test]
fn evidence_sweep_reports_a_malformed_closure_record_only_once() {
    let root = temporary_root("sweep-malformed-closure");
    let directory = root.join("evidence/m5");
    fs::create_dir_all(&directory).expect("evidence directory should be creatable");
    fs::write(directory.join("inventory-summary.json"), "not JSON").expect("record should write");
    fs::write(directory.join("manifest.json"), serde_json::json!({
        "schemaVersion": 1, "milestone": "M5",
        "benchmarks": {"results": [{"name": "inventory-summary.json", "sha256": "0".repeat(64)}]},
    }).to_string()).expect("manifest should write");
    track_fixture_files(&root);
    let violations = super::verify_evidence_records(&root);
    let parse_errors = violations
        .iter()
        .filter(|violation| {
            violation.starts_with("cannot parse JSON")
                && violation.contains("inventory-summary.json")
        })
        .count();
    assert_eq!(parse_errors, 1, "{violations:?}");
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_nonobject_manifest_does_not_misreport_existing_records_as_missing() {
    let root = temporary_root("nonobject-manifest-records");
    let directory = root.join("evidence/m5");
    fs::create_dir_all(&directory).expect("evidence directory should be creatable");
    let path = directory.join("manifest.json");
    fs::write(&path, "[]").expect("manifest should write");
    fs::write(directory.join("inventory-summary.json"), "{}")
        .expect("existing record should write");
    let violations = super::verify_evidence_manifest(&root, &path, "M5");
    assert_eq!(violations, ["milestone manifest must be a JSON object"]);
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
