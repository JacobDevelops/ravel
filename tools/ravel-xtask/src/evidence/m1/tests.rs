use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};

use super::{verify_m1_manifest, verify_m1_manifest_repeated};
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
    verify_m1_manifest_repeated(&root, &mut violations);
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
    verify_m1_manifest_repeated(&root, &mut violations);
    assert!(
        violations
            .iter()
            .any(|violation| violation.starts_with(REPEATED)),
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
