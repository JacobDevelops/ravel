use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};

use super::{verify_m1_manifest, verify_m1_manifest_repeated};
use crate::evidence::spikes::spike_result_paths;
use crate::test_support::temporary_root;

const REPEATED: &str = "M1 closure requires an ADG-3 result";
const STOCK_LAUNCH: &str =
    "M1 closure requires an ADG-5 result recording an observed stock-Prism launch";

fn spike_root(label: &str, gate: &str, record: &Value) -> PathBuf {
    let root = temporary_root(label);
    let directory = root.join("evidence/m1/spikes").join(gate);
    fs::create_dir_all(&directory).expect("spike directory should be creatable");
    fs::write(directory.join("result.json"), record.to_string())
        .expect("record should be writable");
    root
}

fn repeated_violations(label: &str, binding: Value) -> Vec<String> {
    let mut record = json!({});
    record["bindingGeneration"] = binding;
    let root = spike_root(label, "adg-3", &record);
    let mut violations = Vec::new();
    let paths = spike_result_paths(&root, "adg-3", "ADG-3", &mut violations);
    verify_m1_manifest_repeated(&root, &paths, &mut violations);
    fs::remove_dir_all(root).expect("temporary root should be removable");
    violations
}

#[test]
fn an_identical_repeated_binding_digest_is_accepted() {
    let violations = repeated_violations(
        "m1-repeated-identical",
        json!({"outputSha256": "abc", "repeatedOutputSha256": "abc", "repeatedRunIsIdentical": true}),
    );
    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn a_different_missing_or_null_repeated_digest_is_refused() {
    for (label, binding) in [
        (
            "m1-repeated-different",
            json!({"outputSha256": "abc", "repeatedOutputSha256": "def", "repeatedRunIsIdentical": true}),
        ),
        (
            "m1-repeated-missing",
            json!({"outputSha256": "abc", "repeatedRunIsIdentical": true}),
        ),
        (
            "m1-repeated-null",
            json!({"outputSha256": null, "repeatedOutputSha256": null, "repeatedRunIsIdentical": true}),
        ),
        (
            "m1-repeated-flag-null",
            json!({"outputSha256": "abc", "repeatedOutputSha256": "abc", "repeatedRunIsIdentical": null}),
        ),
    ] {
        let violations = repeated_violations(label, binding);
        assert!(
            violations
                .iter()
                .any(|violation| violation.starts_with(REPEATED)),
            "{label}: {violations:?}"
        );
    }
}

#[test]
fn an_unparseable_repeated_record_is_refused() {
    let root = temporary_root("m1-repeated-unparseable");
    let directory = root.join("evidence/m1/spikes/adg-3");
    fs::create_dir_all(&directory).expect("spike directory should be creatable");
    fs::write(directory.join("result.json"), "not JSON").expect("record should be writable");
    let mut violations = Vec::new();
    let paths = spike_result_paths(&root, "adg-3", "ADG-3", &mut violations);
    verify_m1_manifest_repeated(&root, &paths, &mut violations);
    assert!(
        violations
            .iter()
            .any(|violation| violation.starts_with(REPEATED)),
        "{violations:?}"
    );
    assert!(
        violations
            .iter()
            .any(|message| message.starts_with("cannot parse JSON")),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn stock_launch_refused(label: &str, record: &Value) -> bool {
    let root = spike_root(label, "adg-5", record);
    let mut violations = Vec::new();
    verify_m1_manifest(&root, &json!({}), &mut violations);
    fs::remove_dir_all(root).expect("temporary root should be removable");
    violations.iter().any(|violation| violation == STOCK_LAUNCH)
}

#[test]
fn only_a_recorded_true_stock_launch_satisfies_adg_5() {
    assert!(!stock_launch_refused(
        "m1-stock-true",
        &json!({"prismIntegration": {"stockLaunch": true}})
    ));
    assert!(stock_launch_refused(
        "m1-stock-false",
        &json!({"prismIntegration": {"stockLaunch": false}})
    ));
    assert!(stock_launch_refused(
        "m1-stock-null",
        &json!({"prismIntegration": {"stockLaunch": null}})
    ));
    assert!(stock_launch_refused(
        "m1-stock-missing",
        &json!({"prismIntegration": {}})
    ));
    assert!(stock_launch_refused(
        "m1-stock-string",
        &json!({"prismIntegration": {"stockLaunch": "true"}})
    ));
}

#[test]
fn a_matching_spike_does_not_hide_another_records_parse_error() {
    let root = spike_root(
        "m1-repeated-mixed",
        "adg-3",
        &json!({
            "bindingGeneration": {"outputSha256": "abc", "repeatedOutputSha256": "abc",
                "repeatedRunIsIdentical": true}
        }),
    );
    fs::write(
        root.join("evidence/m1/spikes/adg-3/z-malformed.json"),
        "not JSON",
    )
    .expect("malformed record should be writable");
    let mut violations = Vec::new();
    let paths = spike_result_paths(&root, "adg-3", "ADG-3", &mut violations);
    verify_m1_manifest_repeated(&root, &paths, &mut violations);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].starts_with("cannot parse JSON"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_missing_spike_directory_reports_its_read_error() {
    let root = temporary_root("m1-repeated-missing-directory");
    let mut violations = Vec::new();
    let paths = spike_result_paths(&root, "adg-3", "ADG-3", &mut violations);
    verify_m1_manifest_repeated(&root, &paths, &mut violations);
    assert!(
        violations
            .iter()
            .any(|message| message.starts_with("cannot read spike gate ADG-3:")),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn missing_spike_directories_are_enumerated_once_per_milestone_check() {
    let root = temporary_root("m1-missing-directories-once");
    let mut violations = Vec::new();
    verify_m1_manifest(&root, &json!({}), &mut violations);
    for gate in ["ADG-3", "ADG-5"] {
        let prefix = format!("cannot read spike gate {gate}:");
        assert_eq!(
            violations
                .iter()
                .filter(|message| message.starts_with(&prefix))
                .count(),
            1,
            "{violations:?}"
        );
    }
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
