use std::fs;
use std::path::Path;

use crate::json::{read_json, redacted_path};
use crate::test_support::temporary_root;

#[test]
fn gate_output_never_names_an_absolute_path() {
    let absolute = std::env::current_dir()
        .expect("the current directory must be available")
        .join("evidence/m0/manifest.json");

    assert_eq!(redacted_path(&absolute), "evidence/m0/manifest.json");
    assert_eq!(
        redacted_path(Path::new("evidence/m0/manifest.json")),
        "evidence/m0/manifest.json"
    );
    assert_eq!(
        redacted_path(Path::new("/somewhere/else/manifest.json")),
        "manifest.json"
    );
}

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

    assert_eq!(read_json(&path).expect("record should parse"), expected);
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn malformed_json_reports_its_location_without_exposing_the_absolute_path() {
    let root = temporary_root("json-malformed");
    let path = root.join("record.json");
    fs::write(&path, "{\n  \"value\": ]\n}").expect("record should be writable");

    let error = read_json(&path).expect_err("malformed JSON must be rejected");

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
    let error = read_json(&root.join("missing.json")).expect_err("missing JSON must be rejected");

    assert!(
        error.starts_with("cannot read JSON missing.json:"),
        "{error}"
    );
    assert!(!error.contains(&root.display().to_string()), "{error}");
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
