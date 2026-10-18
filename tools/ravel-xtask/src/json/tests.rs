use std::fs;
use std::path::Path;

use crate::json::{read_json, read_text, redacted_path, relative_label};
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

#[test]
fn text_loading_uses_the_supplied_root_and_keeps_relative_error_labels() {
    let root = temporary_root("text-rooted");
    let relative = Path::new("nested/record.txt");
    fs::create_dir_all(root.join("nested")).expect("directory should be creatable");
    fs::write(root.join(relative), "recorded").expect("record should be writable");
    assert_eq!(
        read_text(&root, relative).expect("record should read"),
        "recorded"
    );
    assert_eq!(
        read_text(&root, &root.join(relative)).expect("absolute record should read"),
        "recorded"
    );
    assert_eq!(
        relative_label(&root, &root.join(relative)),
        "nested/record.txt"
    );
    let error =
        read_text(&root, Path::new("nested/missing.txt")).expect_err("missing record must fail");
    assert!(
        error.starts_with("cannot read nested/missing.txt:"),
        "{error}"
    );
    assert!(!error.contains(&root.display().to_string()), "{error}");
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn text_loading_accepts_paths_already_prefixed_by_a_relative_root() {
    let root = std::path::PathBuf::from(format!(
        "target/xtask-relative-text-root-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("relative root should be creatable");
    let relative = Path::new("record.txt");
    let prefixed = root.join(relative);
    fs::write(&prefixed, "recorded").expect("record should be writable");
    assert_eq!(
        read_text(&root, relative).expect("relative record should read"),
        "recorded"
    );
    assert_eq!(
        read_text(&root, &prefixed).expect("prefixed record should read"),
        "recorded"
    );
    assert_eq!(relative_label(&root, &prefixed), "record.txt");
    let absolute = std::env::current_dir()
        .expect("current directory")
        .join(&prefixed);
    assert_eq!(
        read_text(&root, &absolute).expect("absolute record should read"),
        "recorded"
    );
    let error = read_text(&root, &root.join("missing.txt")).expect_err("missing record must fail");
    assert!(error.starts_with("cannot read missing.txt:"), "{error}");
    fs::remove_dir_all(root).expect("relative root should be removable");
}
