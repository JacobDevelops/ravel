use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use ravel_xtask::{verify_evidence_manifest, verify_milestone};

static NEXT_PATH: AtomicUsize = AtomicUsize::new(0);

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("xtask must remain under tools/ravel-xtask")
}

fn fixture(name: &str) -> std::path::PathBuf {
    repository_root()
        .join("tests/fixtures/public/evidence")
        .join(name)
}

fn altered_complete_fixture(needle: &str, replacement: &str) -> PathBuf {
    let source =
        fs::read_to_string(fixture("complete-m0.json")).expect("complete fixture must be readable");
    let altered = source.replace(needle, replacement);
    assert_ne!(source, altered, "fixture mutation must replace content");

    let path = std::env::temp_dir().join(format!(
        "ravel-evidence-{}-{}.json",
        std::process::id(),
        NEXT_PATH.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, altered).expect("altered fixture must be writable");
    path
}

fn actual_milestone_violations(manifest: &Path) -> Vec<String> {
    let root = std::env::temp_dir().join(format!(
        "ravel-native-evidence-{}-{}",
        std::process::id(),
        NEXT_PATH.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(root.join("schemas")).expect("schema directory should be creatable");
    fs::create_dir_all(root.join("evidence/m0")).expect("evidence directory should be creatable");
    fs::copy(
        repository_root().join("schemas/evidence.schema.json"),
        root.join("schemas/evidence.schema.json"),
    )
    .expect("evidence schema should be copied");
    fs::copy(manifest, root.join("evidence/m0/manifest.json")).expect("manifest should be copied");

    let violations = verify_milestone(&root, "M0");

    fs::remove_dir_all(root).expect("evidence root should be removable");
    violations
}

#[test]
fn deliberately_incomplete_m0_bundle_is_rejected() {
    let violations =
        verify_evidence_manifest(repository_root(), &fixture("incomplete-m0.json"), "M0");

    for expected in [
        "M0 manifest must include every work package from M0-W1 through M0-W9 exactly once",
        "milestone revision must be clean",
        "milestone commands must all succeed",
        "test counts must equal passed + failed + skipped",
        "milestone tests must have zero failures",
        "milestone tests must have zero skipped tests",
        "milestone line coverage must be at least 80%",
        "milestone must have no blocking deviations or risks",
        "milestone review must be independent and pass",
        "milestone review must include a hashed record",
        "M0 manifest must include native Linux x86_64 launch evidence",
    ] {
        assert!(
            violations.iter().any(|violation| violation == expected),
            "missing expected violation `{expected}` in:\n{}",
            violations.join("\n")
        );
    }
}

#[test]
fn synthetic_complete_m0_bundle_passes() {
    let violations =
        verify_evidence_manifest(repository_root(), &fixture("complete-m0.json"), "M0");

    assert!(
        violations.is_empty(),
        "synthetic M0 fixture violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn synthetic_contract_fixture_cannot_close_the_real_milestone() {
    let violations = actual_milestone_violations(&fixture("complete-m0.json"));

    for expected in [
        "milestone closure requires native evidence, not a synthetic contract fixture",
        "native milestone evidence must not contain synthetic placeholders",
    ] {
        assert!(
            violations.iter().any(|violation| violation == expected),
            "missing expected violation `{expected}` in:\n{}",
            violations.join("\n")
        );
    }
}

#[test]
fn m0_requires_a_linux_workload_attestation() {
    let path = altered_complete_fixture(
        r#""attestations": [
    {
      "platform": "linux",
      "architecture": "x86_64",
      "workloads": ["vanilla", "official-atm11"],
      "statement": "synthetic Linux launch attestation",
      "attestedBy": "synthetic reviewer",
      "attestedAt": "2026-01-01T00:00:00Z"
    }
  ]"#,
        r#""attestations": []"#,
    );

    let violations = actual_milestone_violations(&path);

    assert!(violations.iter().any(|violation| {
        violation == "M0 closure requires a workload-bound native Linux attestation"
    }));
    fs::remove_file(path).expect("altered fixture should be removable");
}

#[test]
fn malformed_hash_is_rejected_by_the_evidence_schema() {
    let violations =
        verify_evidence_manifest(repository_root(), &fixture("invalid-hash-m0.json"), "M0");

    assert!(
        violations
            .iter()
            .any(|violation| violation == "milestone manifest does not conform to evidence schema")
    );
}

#[test]
fn identifying_local_paths_are_rejected() {
    let path = altered_complete_fixture(
        "schema and verifier test only",
        "/home/alice/.minecraft synthetic capture",
    );

    let violations = verify_evidence_manifest(repository_root(), &path, "M0");

    assert!(
        violations
            .iter()
            .any(|violation| violation == "milestone manifest contains sensitive or local data")
    );
    fs::remove_file(path).expect("altered fixture should be removable");
}

#[test]
fn empty_test_runs_cannot_close_a_milestone() {
    let path = altered_complete_fixture(
        r#""total": 10,
    "passed": 10"#,
        r#""total": 0,
    "passed": 0"#,
    );

    let violations = verify_evidence_manifest(repository_root(), &path, "M0");

    assert!(
        violations
            .iter()
            .any(|violation| violation == "milestone must record at least one passing test")
    );
    fs::remove_file(path).expect("altered fixture should be removable");
}

#[test]
fn credential_fields_are_rejected() {
    let path = altered_complete_fixture(
        r#""purpose": "schema and verifier test only""#,
        r#""minecraftAccessToken": "synthetic-secret""#,
    );

    let violations = verify_evidence_manifest(repository_root(), &path, "M0");

    assert!(
        violations
            .iter()
            .any(|violation| violation == "milestone manifest contains sensitive or local data")
    );
    fs::remove_file(path).expect("altered fixture should be removable");
}

#[test]
fn escaped_paths_and_credentials_in_arrays_are_rejected() {
    for replacement in [
        r#""purpose": "\u002fhome\u002falice\u002fproject""#,
        r#""tokens": ["synthetic-launcher-token"]"#,
    ] {
        let path =
            altered_complete_fixture(r#""purpose": "schema and verifier test only""#, replacement);

        let violations = verify_evidence_manifest(repository_root(), &path, "M0");

        assert!(
            violations.iter().any(
                |violation| violation == "milestone manifest contains sensitive or local data"
            ),
            "redaction bypass was accepted: {replacement}"
        );
        fs::remove_file(path).expect("altered fixture should be removable");
    }
}

#[test]
fn authorization_bearer_values_are_rejected() {
    let path = altered_complete_fixture(
        "devenv tasks run ravel:verify",
        "curl -H Authorization: Bearer synthetic-launcher-token",
    );

    let violations = verify_evidence_manifest(repository_root(), &path, "M0");

    assert!(
        violations
            .iter()
            .any(|violation| violation == "milestone manifest contains sensitive or local data")
    );
    fs::remove_file(path).expect("altered fixture should be removable");
}

#[test]
fn m0_versions_are_pinned() {
    let path = altered_complete_fixture(r#""minecraft": "26.1.2""#, r#""minecraft": "26.1.3""#);

    let violations = verify_evidence_manifest(repository_root(), &path, "M0");

    assert!(violations.iter().any(|violation| {
        violation == "M0 manifest must pin Minecraft 26.1.2, NeoForge 26.1.2.78, and ATM11 0.2.1"
    }));
    fs::remove_file(path).expect("altered fixture should be removable");
}

#[test]
fn m0_platform_nix_version_is_pinned() {
    let path = altered_complete_fixture(r#""nix": "2.33.3""#, r#""nix": "2.34.0""#);

    let violations = verify_evidence_manifest(repository_root(), &path, "M0");

    assert!(violations.iter().any(
        |violation| violation == "M0 manifest must include native Linux x86_64 launch evidence"
    ));
    fs::remove_file(path).expect("altered fixture should be removable");
}

#[test]
fn every_platform_record_requires_its_own_versions_and_settings() {
    let path = altered_complete_fixture(
        r#"      "settings": {
        "purpose": "synthetic native Linux context"
      },
"#,
        "",
    );

    let violations = verify_evidence_manifest(repository_root(), &path, "M0");

    assert!(
        violations
            .iter()
            .any(|violation| violation == "milestone manifest does not conform to evidence schema")
    );
    fs::remove_file(path).expect("altered fixture should be removable");
}
