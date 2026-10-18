use std::fs;

use crate::evidence::closure::verify_milestone_figures;
use crate::evidence::m5::M5_FIGURES;
use crate::test_support::{balanced_summary, summary_root};

#[test]
fn a_manifest_figure_must_follow_from_the_record_it_summarises() {
    let root = summary_root("m5-carried-figures", &balanced_summary());
    let manifest = root.join("evidence/m5/manifest.json");
    fs::write(
        &manifest,
        r#"{"environment":{"settings":{"inventory":{"artifacts":11,"planned":10,
            "graphNodes":12,"graphEdges":20,"clusters":3,"sourceProhibited":2,
            "sourceUnresolved":1,"publishable":0,"readingSource":4,
            "dependencyEdges":7,"archiveReads":99,"entryBodyReads":0,
            "digestsBound":1}}}}"#,
    )
    .expect("manifest should be writable");
    let mut violations = Vec::new();

    let manifest = crate::json::read_json(&manifest).expect("manifest should parse");
    verify_milestone_figures(
        &crate::json::read_json(&root.join(M5_FIGURES.record)).expect("summary should parse"),
        &manifest,
        &M5_FIGURES,
        &mut violations,
    );

    assert_eq!(
        violations.len(),
        1,
        "violations:\n{}",
        violations.join("\n")
    );
    assert!(
        violations[0].contains("records /environment/settings/inventory/artifacts 11 while"),
        "violation was: {}",
        violations[0]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn missing_and_malformed_records_report_once_before_closure() {
    use super::{RecordDefinition, read_records, verify_closure_records};
    use crate::evidence::rules::Milestone;
    const RECORDS: &[RecordDefinition] =
        &[("inventory-summary.json", |_| Ok(()), "summary", false)];
    for (label, contents) in [("absent", None), ("malformed", Some("not JSON"))] {
        let root = crate::test_support::temporary_root(label);
        fs::create_dir_all(root.join("evidence/m5")).expect("record directory should exist");
        if let Some(contents) = contents {
            fs::write(root.join(M5_FIGURES.record), contents).expect("record should write");
        }
        let mut violations = Vec::new();
        let records = read_records(&root, Milestone::M5, RECORDS, &mut violations);
        verify_closure_records(Milestone::M5, &records, RECORDS, &mut violations);
        assert_eq!(violations.len(), 1, "{label}: {violations:?}");
        assert!(violations[0].contains("inventory-summary.json"));
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}

#[test]
fn closure_uses_the_loaded_record_and_names_the_failed_clause() {
    use super::{RecordDefinition, read_records, verify_closure_records};
    use crate::evidence::rules::{Milestone, require};
    const RECORDS: &[RecordDefinition] = &[(
        "inventory-summary.json",
        |value| require(value["wave"]["frozen"] == true, "wave.frozen"),
        "a frozen wave",
        false,
    )];
    let root = summary_root(
        "cached-record-clause",
        r#"{"milestone":"M5","wave":{"frozen":false}}"#,
    );
    let mut violations = Vec::new();
    let records = read_records(&root, Milestone::M5, RECORDS, &mut violations);
    fs::remove_file(root.join(M5_FIGURES.record)).expect("loaded record should be removable");
    verify_closure_records(Milestone::M5, &records, RECORDS, &mut violations);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("failed wave.frozen"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn record_metadata_requires_files_and_checks_the_selected_milestone() {
    use super::{RecordDefinition, read_records, verify_closure_records};
    use crate::evidence::rules::Milestone;
    const RECORDS: &[RecordDefinition] = &[(
        "custom.json",
        |_| Err("later clause must hold".to_owned()),
        "custom record",
        true,
    )];
    let root = crate::test_support::temporary_root("required-record-metadata");
    fs::create_dir_all(root.join("evidence/m4")).expect("directory should exist");
    let mut violations = Vec::new();
    let records = read_records(&root, Milestone::M4, RECORDS, &mut violations);
    verify_closure_records(Milestone::M4, &records, RECORDS, &mut violations);
    assert_eq!(
        violations,
        ["missing required file: evidence/m4/custom.json"]
    );
    for milestone in [serde_json::json!("M3"), serde_json::Value::Null] {
        fs::write(
            root.join("evidence/m4/custom.json"),
            serde_json::json!({"milestone": milestone}).to_string(),
        )
        .expect("record should write");
        violations.clear();
        let records = read_records(&root, Milestone::M4, RECORDS, &mut violations);
        verify_closure_records(Milestone::M4, &records, RECORDS, &mut violations);
        assert_eq!(
            violations,
            [
                "M4 closure requires custom record: evidence/m4/custom.json: failed /milestone must be M4"
            ]
        );
    }
    fs::remove_dir_all(root).expect("temporary root should remove");
}
