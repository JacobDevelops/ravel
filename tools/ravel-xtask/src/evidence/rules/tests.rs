use super::{has_exact_benchmark_results, has_native_attestation};

#[test]
fn benchmark_results_require_the_exact_named_set_and_protocol() {
    let mut manifest = serde_json::json!({
        "benchmarks": {
            "protocolVersion": "fixture-v1",
            "results": [{"name": "b.json"}, {"name": "a.json"}]
        }
    });
    let expected = ["a.json", "b.json"];
    assert!(has_exact_benchmark_results(
        &manifest,
        "fixture-v1",
        &expected
    ));
    assert!(!has_exact_benchmark_results(
        &manifest, "other-v1", &expected
    ));

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
            !has_exact_benchmark_results(&manifest, "fixture-v1", &expected),
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
    assert!(has_native_attestation(&manifest, "ravel"));
    assert!(!has_native_attestation(&manifest, "official-atm11"));

    manifest["attestations"][0]["workloads"] = serde_json::json!(["official-atm11", "vanilla"]);
    assert!(has_native_attestation(&manifest, "official-atm11"));
    assert!(!has_native_attestation(&manifest, "ravel"));

    manifest["attestations"][0]["architecture"] = serde_json::json!("aarch64");
    assert!(!has_native_attestation(&manifest, "official-atm11"));
    manifest["attestations"][0]["architecture"] = serde_json::json!("x86_64");
    manifest["attestations"][0]["platform"] = serde_json::json!("windows");
    assert!(!has_native_attestation(&manifest, "official-atm11"));
}
