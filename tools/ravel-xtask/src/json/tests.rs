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

#[test]
fn sums_require_every_operand_and_a_real_row_array() {
    use super::{column_sum, number, sum};
    let value = serde_json::json!({
        "a": 2, "b": 3, "total": 5,
        "rows": [{"count": 2}, {"count": 3}],
        "incomplete": [{"count": 2}, {}],
        "empty": []
    });
    assert_eq!(sum(&value, &["/a", "/b"]), Some(5.0));
    assert_eq!(column_sum(&value, "/rows", "/count"), Some(5.0));
    assert_eq!(sum(&value, &["/a", "/missing"]), None);
    assert_eq!(column_sum(&value, "/incomplete", "/count"), None);
    assert_eq!(column_sum(&value, "/missing", "/count"), None);
    assert_eq!(column_sum(&value, "/a", "/count"), None);
    assert_eq!(column_sum(&value, "/empty", "/count"), Some(0.0));
    assert!(
        !sum(&value, &["/missing"])
            .zip(number(&value, "/missing-total"))
            .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
    );
}
