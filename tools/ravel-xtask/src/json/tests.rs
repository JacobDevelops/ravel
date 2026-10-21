use std::fs;

use crate::json::read_json;
use crate::test_support::temporary_root;

#[test]
fn json_loading_preserves_nested_keys_and_values() {
    let root = temporary_root("json-nested");
    let path = root.join("record.json");
    let expected = serde_json::json!({
        "quoted\"key": {"line\nbreak": [null, true, 42, "literal\\value"]},
        "empty": [],
        "pointer/key": {"~tilde": "preserved"}
    });
    fs::write(&path, expected.to_string()).expect("record should be writable");

    assert_eq!(
        read_json(&root, &path).expect("record should parse"),
        expected
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn malformed_json_reports_its_location_without_exposing_the_absolute_path() {
    let root = temporary_root("json-malformed");
    let path = root.join("record.json");
    fs::write(&path, "{\n  \"value\": ]\n}").expect("record should be writable");

    let error = read_json(&root, &path).expect_err("malformed JSON must be rejected");

    assert!(
        error.starts_with("cannot parse JSON record.json:"),
        "{error}"
    );
    assert!(error.contains("line 2 column 12"), "{error}");
    assert!(!error.contains(&root.display().to_string()), "{error}");
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn unreadable_json_reports_a_redacted_file_name() {
    let root = temporary_root("json-missing");
    let error =
        read_json(&root, &root.join("missing.json")).expect_err("missing JSON must be rejected");

    assert!(
        error.starts_with("cannot read JSON missing.json:"),
        "{error}"
    );
    assert!(!error.contains(&root.display().to_string()), "{error}");
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn nested_json_diagnostics_use_the_supplied_repository_root() {
    let root = temporary_root("json-rooted-diagnostic");
    fs::create_dir_all(root.join("evidence/m7")).expect("record directory should exist");
    let path = root.join("evidence/m7/summary.json");
    let error = read_json(&root, &path).expect_err("missing record must refuse");
    assert!(
        error.starts_with("cannot read JSON evidence/m7/summary.json:"),
        "{error}"
    );
    fs::write(&path, "{").expect("malformed record should write");
    let error = read_json(&root, &path).expect_err("malformed record must refuse");
    assert!(
        error.starts_with("cannot parse JSON evidence/m7/summary.json:"),
        "{error}"
    );
    assert!(!error.contains(&root.display().to_string()), "{error}");
    fs::remove_dir_all(root).expect("temporary root should remove");
}

#[test]
fn figures_require_both_values_and_use_one_total_order() {
    use super::both_equal;
    assert!(both_equal(Some(42.0), Some(42.0)));
    assert!(!both_equal(None, None));
    assert!(!both_equal(Some(42.0), None));
    assert!(!both_equal(None, Some(42.0)));
    assert!(!both_equal(Some(-0.0), Some(0.0)));
}
