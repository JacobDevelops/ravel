use std::fs;
use std::path::Path;

use crate::json::{benchmark_results, native_attestation, read_json, redacted_path};
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
fn benchmark_results_require_the_exact_named_set_and_protocol() {
    let mut manifest = serde_json::json!({
        "benchmarks": {
            "protocolVersion": "fixture-v1",
            "results": [{"name": "b.json"}, {"name": "a.json"}]
        }
    });
    let expected = ["a.json", "b.json"];
    assert!(benchmark_results(&manifest, "fixture-v1", &expected));
    assert!(!benchmark_results(&manifest, "other-v1", &expected));

    for results in [
        serde_json::json!([{"name": "a.json"}, {}]),
        serde_json::json!([{"name": "a.json"}, {"name": 2}]),
        serde_json::json!([{"name": "a.json"}, "b.json"]),
        serde_json::json!([{"name": "a.json"}, {"name": "a.json"}]),
        serde_json::json!([{"name": "a.json"}, {"name": "b.json"}, {"name": "c.json"}]),
        serde_json::json!([{"name": "a.json"}]),
        serde_json::Value::Null,
    ] {
        manifest["benchmarks"]["results"] = results;
        assert!(
            !benchmark_results(&manifest, "fixture-v1", &expected),
            "{manifest}"
        );
    }
}

#[test]
fn native_attestation_requires_its_requested_workload_and_native_platform() {
    let mut manifest = serde_json::json!({
        "attestations": [{
            "platform": "linux",
            "architecture": "x86_64",
            "workloads": ["ravel"]
        }]
    });
    assert!(native_attestation(&manifest, "ravel"));
    assert!(!native_attestation(&manifest, "official-atm11"));

    manifest["attestations"][0]["workloads"] = serde_json::json!(["official-atm11", "vanilla"]);
    assert!(native_attestation(&manifest, "official-atm11"));
    assert!(!native_attestation(&manifest, "ravel"));

    manifest["attestations"][0]["architecture"] = serde_json::json!("aarch64");
    assert!(!native_attestation(&manifest, "official-atm11"));
    manifest["attestations"][0]["architecture"] = serde_json::json!("x86_64");
    manifest["attestations"][0]["platform"] = serde_json::json!("windows");
    assert!(!native_attestation(&manifest, "official-atm11"));
}
