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
    altered_fixture("complete-m0.json", needle, replacement)
}

fn altered_m1_fixture(needle: &str, replacement: &str) -> PathBuf {
    altered_fixture("complete-m1.json", needle, replacement)
}

fn altered_fixture(name: &str, needle: &str, replacement: &str) -> PathBuf {
    let source = fs::read_to_string(fixture(name)).expect("complete fixture must be readable");
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

/// A private directory a test can build an evidence root inside.
fn temporary_root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "ravel-evidence-root-{}-{}",
        std::process::id(),
        NEXT_PATH.fetch_add(1, Ordering::Relaxed)
    ))
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
        "milestone recording a review verdict must record a hashed review record for it",
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
fn a_milestone_cannot_close_over_a_record_stating_another_verdict() {
    // The defect this exists for: `review.result` was set to pass while the bound
    // record read `Result: FAIL`, and the milestone gate — the one a milestone is
    // closed by — exited 0 without reading the document the manifest names.
    let root = temporary_root();
    fs::create_dir_all(root.join("schemas")).expect("schema directory should be creatable");
    fs::create_dir_all(root.join("evidence/m0")).expect("evidence directory should be creatable");
    fs::copy(
        repository_root().join("schemas/evidence.schema.json"),
        root.join("schemas/evidence.schema.json"),
    )
    .expect("evidence schema should be copied");
    fs::copy(
        fixture("complete-m0.json"),
        root.join("evidence/m0/manifest.json"),
    )
    .expect("manifest should be copied");
    fs::write(
        root.join("evidence/m0/synthetic-review.md"),
        "Result: FAIL\n",
    )
    .expect("record should be writable");

    let violations = verify_milestone(&root, "M0");

    let expected = "evidence/m0/manifest.json records review.result pass while the review record \
                    it binds states fail";
    assert!(
        violations.iter().any(|violation| violation == expected),
        "missing expected violation `{expected}` in:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(root).expect("evidence root should be removable");
}

#[test]
fn m1_closure_is_refused_without_an_attestation_for_the_operator_run_launch() {
    let manifest = altered_m1_fixture(
        "\"platform\": \"linux\",\n      \"architecture\": \"x86_64\",\n      \"workloads\"",
        "\"platform\": \"macos\",\n      \"architecture\": \"x86_64\",\n      \"workloads\"",
    );

    let violations = verify_evidence_manifest(repository_root(), &manifest, "M1");

    let expected = "M1 closure requires a Linux attestation for the operator-run ADG-5 launch, \
                    which no tracked record can recompute";
    assert!(
        violations.iter().any(|violation| violation == expected),
        "missing expected violation `{expected}` in:\n{}",
        violations.join("\n")
    );
    fs::remove_file(manifest).expect("altered fixture should be removable");
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
fn m1_closure_accepts_the_recorded_prism_launch() {
    // Checked against the real repository, so this test states M1's status: every
    // gate has a result, and ADG-5's result records the launch an operator observed.
    let violations =
        verify_evidence_manifest(repository_root(), &fixture("complete-m1.json"), "M1");

    for unmet in [
        "M1 closure requires an ADG-5 result",
        "M1 closure requires a recorded result",
    ] {
        assert!(
            !violations
                .iter()
                .any(|violation| violation.starts_with(unmet)),
            "{unmet} should be satisfied, got {violations:?}"
        );
    }
}

#[test]
fn m1_closure_is_refused_when_no_result_records_a_launch() {
    // The rule has to still bite, so this drives it with the shape ADG-5's result
    // had before an operator ran a launch.
    let root = temporary_root();
    fs::create_dir_all(root.join("evidence/m1/spikes/adg-5"))
        .expect("spike directory should be creatable");
    fs::write(
        root.join("evidence/m1/spikes/adg-5/unlaunched.json"),
        r#"{"prismIntegration": {"stockLaunch": false}}"#,
    )
    .expect("unlaunched result should be writable");

    let violations = verify_evidence_manifest(&root, &fixture("complete-m1.json"), "M1");

    fs::remove_dir_all(&root).expect("evidence root should be removable");
    assert!(
        violations.iter().any(|violation| violation
            == "M1 closure requires an ADG-5 result recording an observed stock-Prism launch"),
        "expected the outstanding launch to be reported, got {violations:?}"
    );
}

#[test]
fn m1_work_packages_must_cover_the_whole_milestone() {
    let path = fixture("complete-m1.json");
    let source = fs::read_to_string(&path).expect("the M1 fixture must be readable");
    // One work package listed twice and another missing: the mistake a hand-edited
    // manifest actually makes.
    let duplicated = source.replace("\"M1-W8\"", "\"M1-W1\"");
    assert_ne!(source, duplicated, "fixture mutation must replace content");
    let altered = std::env::temp_dir().join(format!("ravel-m1-short-{}.json", std::process::id()));
    fs::write(&altered, duplicated).expect("altered fixture must be writable");

    let violations = verify_evidence_manifest(repository_root(), &altered, "M1");

    assert!(violations.iter().any(|violation| violation
        == "M1 manifest must include every work package from M1-W1 through M1-W8 exactly once"));
    fs::remove_file(altered).expect("altered fixture should be removable");
}

#[test]
fn a_java_class_name_is_not_mistaken_for_a_game_directory() {
    // `net.minecraft.util.StringUtil` is exactly what binding evidence records.
    let path = altered_complete_fixture(
        "schema and verifier test only",
        "resolved net.minecraft.util.StringUtil from the pinned jar",
    );

    let violations = verify_evidence_manifest(repository_root(), &path, "M0");

    assert!(
        !violations
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
    // This replacement targets the frozen M0 fixture's historical devenv command.
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

/// The smallest set of M2 records the gate accepts, so a test can break one.
const VALID_M2_RECORDS: &[(&str, &str)] = &[
    (
        "noop-pipeline.json",
        r#"{"runs": [{"repeatedRunIsIdentical": true}]}"#,
    ),
    (
        "pre-world-gate.json",
        r#"{"gate": {"refusesWorldAccess": true, "ranModCode": false}}"#,
    ),
    (
        "loader-overhead.json",
        r#"{"budget": {"limits": [{"metric": "loader.noopPackage.pipeline.p99",
                                   "unit": "nanoseconds",
                                   "operator": "less-than-or-equal",
                                   "measured": 7084.0,
                                   "limit": 14168.0}]}}"#,
    ),
    (
        "prism-launch.json",
        r#"{"launch": {"provesStockLaunch": true,
                       "objection": null,
                       "attestation": {"stockLauncher": true, "derived": false}},
            "ravel": {"reachedGameProcess": true,
                      "rewrite": {"reported": true,
                                  "reachedRavel": true,
                                  "classesRewritten": 2,
                                  "methodsRewritten": 5,
                                  "nativeInvocations": 144}},
            "instance": {"withoutAnotherLoader": true}}"#,
    ),
];

fn valid_m2_record(file: &str) -> &'static str {
    VALID_M2_RECORDS
        .iter()
        .find_map(|(name, body)| (*name == file).then_some(*body))
        .expect("the record must be one M2 closure asks for")
}

/// An evidence root holding that set with one record replaced by a broken one.
fn m2_record_root(broken: &str, body: &str) -> PathBuf {
    let root = temporary_root();
    fs::create_dir_all(root.join("evidence/m2")).expect("evidence directory should be creatable");
    for (file, valid) in VALID_M2_RECORDS {
        let content = if *file == broken { body } else { *valid };
        fs::write(root.join("evidence/m2").join(file), content).expect("record should be writable");
    }
    root
}

fn m2_violations(root: &Path) -> Vec<String> {
    let violations = verify_evidence_manifest(root, &fixture("complete-m2.json"), "M2");
    fs::remove_dir_all(root).expect("evidence root should be removable");
    violations
}

#[test]
fn m2_closure_accepts_the_recorded_loader_evidence() {
    // Checked against the real repository, so this test states M2's status: the
    // pipeline, gate, overhead and launch records all exist, all say what their
    // roadmap rows require, and none of them names a path or a secret.
    let violations =
        verify_evidence_manifest(repository_root(), &fixture("complete-m2.json"), "M2");

    for unmet in ["M2 closure requires", "M2 manifest must", "evidence/m2/"] {
        assert!(
            !violations
                .iter()
                .any(|violation| violation.starts_with(unmet)),
            "{unmet} should be satisfied, got {violations:?}"
        );
    }
}

#[test]
fn m2_closure_is_refused_when_the_launch_record_does_not_show_ravel_running() {
    // The rule has to still bite, so this drives it with the shape the launch
    // record had while M2-W5's criterion was outstanding: a client that ran and
    // an agent that proved nothing.
    let root = m2_record_root(
        "prism-launch.json",
        r#"{"launch": {"provesStockLaunch": true,
                       "objection": null,
                       "attestation": {"stockLauncher": true}},
            "ravel": {"reachedGameProcess": false,
                      "rewrite": {"reported": false,
                                  "reachedRavel": false,
                                  "classesRewritten": 0,
                                  "methodsRewritten": 0,
                                  "nativeInvocations": -1}},
            "instance": {"withoutAnotherLoader": true}}"#,
    );

    let violations = m2_violations(&root);

    assert!(
        violations.iter().any(|violation| violation.starts_with(
            "M2 closure requires a launch record showing the official client reaching Ravel"
        )),
        "expected the unproven launch to be reported, got {violations:?}"
    );
}

#[test]
fn a_launch_that_only_started_the_client_cannot_close_m2() {
    // M1's launch could prove the JVM accepted both arguments and the client
    // ran. Each field below is one of the things M2 added, removed on its own.
    for (missing, record) in [
        (
            "the rewrite report",
            valid_m2_record("prism-launch.json")
                .replace(r#""reported": true"#, r#""reported": false"#),
        ),
        (
            "the count of rewritten classes",
            valid_m2_record("prism-launch.json")
                .replace(r#""classesRewritten": 2"#, r#""classesRewritten": 0"#),
        ),
        (
            "the hook invocations that reached native code",
            valid_m2_record("prism-launch.json")
                .replace(r#""nativeInvocations": 144"#, r#""nativeInvocations": -1"#),
        ),
        (
            "the attested stock launcher",
            valid_m2_record("prism-launch.json")
                .replace(r#""stockLauncher": true"#, r#""stockLauncher": false"#),
        ),
    ] {
        let root = m2_record_root("prism-launch.json", &record);

        let violations = m2_violations(&root);

        assert!(
            violations.iter().any(|violation| violation.starts_with(
                "M2 closure requires a launch record showing the official client reaching Ravel"
            )),
            "a launch record without {missing} was accepted, got {violations:?}"
        );
    }
}

#[test]
fn a_launch_record_whose_counts_are_not_numbers_cannot_close_m2() {
    // jq orders strings above every number, so an unguarded `> 0` reads
    // "not measured" as a count above zero. A hand-edited record that answers
    // the three counts in words is the shape that would otherwise close M2
    // without anything having been counted.
    for (defect, record) in [
        (
            "every count written in words",
            valid_m2_record("prism-launch.json")
                .replace(
                    r#""classesRewritten": 2"#,
                    r#""classesRewritten": "not measured""#,
                )
                .replace(
                    r#""methodsRewritten": 5"#,
                    r#""methodsRewritten": "not measured""#,
                )
                .replace(
                    r#""nativeInvocations": 144"#,
                    r#""nativeInvocations": "not measured""#,
                ),
        ),
        (
            "one count written in words",
            valid_m2_record("prism-launch.json").replace(
                r#""methodsRewritten": 5"#,
                r#""methodsRewritten": "several""#,
            ),
        ),
        (
            "a count given as a string of digits",
            valid_m2_record("prism-launch.json").replace(
                r#""nativeInvocations": 144"#,
                r#""nativeInvocations": "144""#,
            ),
        ),
        (
            "no objection field at all",
            valid_m2_record("prism-launch.json").replace(r#""objection": null,"#, ""),
        ),
    ] {
        let root = m2_record_root("prism-launch.json", &record);

        let violations = m2_violations(&root);

        assert!(
            violations.iter().any(|violation| violation.starts_with(
                "M2 closure requires a launch record showing the official client reaching Ravel"
            )),
            "a launch record with {defect} was accepted, got {violations:?}"
        );
    }
}

#[test]
fn a_pipeline_record_that_does_not_say_the_rerun_matched_cannot_close_m2() {
    // Every jq value but `false` and `null` is truthy, so the field has to be
    // read as the boolean it claims to be rather than as something present.
    for (defect, record) in [
        (
            "a run that says it differed",
            r#"{"runs": [{"repeatedRunIsIdentical": false}]}"#,
        ),
        (
            "the answer written as a word",
            r#"{"runs": [{"repeatedRunIsIdentical": "no"}]}"#,
        ),
        (
            "one run of two that differed",
            r#"{"runs": [{"repeatedRunIsIdentical": true}, {"repeatedRunIsIdentical": false}]}"#,
        ),
        ("a run that does not answer at all", r#"{"runs": [{}]}"#),
        ("no runs at all", r#"{"runs": []}"#),
    ] {
        let root = m2_record_root("noop-pipeline.json", record);

        let violations = m2_violations(&root);

        assert!(
            violations
                .iter()
                .any(|violation| violation
                    .starts_with("M2 closure requires a no-op pipeline record")),
            "a pipeline record with {defect} was accepted, got {violations:?}"
        );
    }
}

#[test]
fn a_gate_record_that_let_a_world_open_cannot_close_m2() {
    // The gate's whole claim is that the refusal came before any mod code, so
    // either half missing is the failure it exists to catch.
    for (defect, record) in [
        (
            "a world opened anyway",
            r#"{"gate": {"refusesWorldAccess": false, "ranModCode": false}}"#,
        ),
        (
            "mod code that ran first",
            r#"{"gate": {"refusesWorldAccess": true, "ranModCode": true}}"#,
        ),
        (
            "the refusal written as a word",
            r#"{"gate": {"refusesWorldAccess": "yes", "ranModCode": false}}"#,
        ),
        ("no gate at all", "{}"),
    ] {
        let root = m2_record_root("pre-world-gate.json", record);

        let violations = m2_violations(&root);

        assert!(
            violations
                .iter()
                .any(|violation| violation
                    .starts_with("M2 closure requires a pre-world gate record")),
            "a gate record with {defect} was accepted, got {violations:?}"
        );
    }
}

#[test]
fn a_budget_limit_that_bounds_nothing_cannot_close_m2() {
    // A limit is only a limit against a named metric it sits above. Each of
    // these is a budget line that reads like one and is not.
    for (defect, budget) in [
        (
            "a bare positive number",
            r#"{"budget": {"limits": [{"limit": 14168.0}]}}"#,
        ),
        (
            "a limit at or below its own measurement",
            r#"{"budget": {"limits": [{"metric": "loader.noopPackage.pipeline.p99",
                                       "unit": "nanoseconds",
                                       "operator": "less-than-or-equal",
                                       "measured": 7084.0,
                                       "limit": 7084.0}]}}"#,
        ),
        (
            "an unnamed metric",
            r#"{"budget": {"limits": [{"metric": "",
                                       "unit": "nanoseconds",
                                       "operator": "less-than-or-equal",
                                       "measured": 7084.0,
                                       "limit": 14168.0}]}}"#,
        ),
        (
            "a measurement of zero standing in for one never taken",
            r#"{"budget": {"limits": [{"metric": "loader.noopPackage.pipeline.p99",
                                       "unit": "nanoseconds",
                                       "operator": "less-than-or-equal",
                                       "measured": 0,
                                       "limit": 14168.0}]}}"#,
        ),
        (
            "a number the unit does not name",
            r#"{"budget": {"limits": [{"metric": "loader.noopPackage.pipeline.p99",
                                       "operator": "less-than-or-equal",
                                       "measured": 7084.0,
                                       "limit": 14168.0}]}}"#,
        ),
        (
            "two lines claiming the same metric",
            r#"{"budget": {"limits": [{"metric": "loader.noopPackage.pipeline.p99",
                                       "unit": "nanoseconds",
                                       "operator": "less-than-or-equal",
                                       "measured": 7084.0,
                                       "limit": 14168.0},
                                      {"metric": "loader.noopPackage.pipeline.p99",
                                       "unit": "nanoseconds",
                                       "operator": "less-than-or-equal",
                                       "measured": 3000.0,
                                       "limit": 6000.0}]}}"#,
        ),
        ("no limits at all", r#"{"budget": {"limits": []}}"#),
    ] {
        let root = m2_record_root("loader-overhead.json", budget);

        let violations = m2_violations(&root);

        assert!(
            violations
                .iter()
                .any(|violation| violation
                    .starts_with("M2 closure requires a loader overhead record")),
            "a budget with {defect} was accepted, got {violations:?}"
        );
    }
}

#[test]
fn an_m2_record_naming_a_local_path_cannot_close_m2() {
    // The pipeline and gate records are byte-compared by conformance tests that
    // assert this themselves. These two are not: one is a machine measurement
    // and the other is built from an operator's launcher log, which is where a
    // path or a session argument would come from.
    for file in ["loader-overhead.json", "prism-launch.json"] {
        let leaked = valid_m2_record(file).replacen(
            '{',
            r#"{"capturedFrom": "/home/alice/prism/instances/ravel", "#,
            1,
        );
        let root = m2_record_root(file, &leaked);

        let violations = m2_violations(&root);

        assert!(
            violations.iter().any(|violation| *violation
                == format!("evidence/m2/{file} contains sensitive or local data")),
            "a local path in {file} was accepted, got {violations:?}"
        );
    }
}

#[test]
fn m2_closure_is_refused_when_a_record_is_missing_altogether() {
    let root = temporary_root();
    fs::create_dir_all(root.join("evidence/m2")).expect("evidence directory should be creatable");

    let violations = verify_evidence_manifest(&root, &fixture("complete-m2.json"), "M2");

    fs::remove_dir_all(&root).expect("evidence root should be removable");
    assert!(
        violations
            .iter()
            .any(|violation| violation
                == "M2 closure requires the record evidence/m2/prism-launch.json"),
        "expected the absent record to be reported, got {violations:?}"
    );
}

#[test]
fn m2_work_packages_must_cover_the_whole_milestone() {
    let source =
        fs::read_to_string(fixture("complete-m2.json")).expect("the M2 fixture must be readable");
    let duplicated = source.replace("\"M2-W8\"", "\"M2-W1\"");
    assert_ne!(source, duplicated, "fixture mutation must replace content");
    let altered = std::env::temp_dir().join(format!("ravel-m2-short-{}.json", std::process::id()));
    fs::write(&altered, duplicated).expect("altered fixture must be writable");

    let violations = verify_evidence_manifest(repository_root(), &altered, "M2");

    assert!(violations.iter().any(|violation| violation
        == "M2 manifest must include every work package from M2-W1 through M2-W8 exactly once"));
    fs::remove_file(altered).expect("altered fixture should be removable");
}

/// The real M3 manifest with one string replaced, written outside the repository.
///
/// M3's rules are about the four records under `evidence/m3/` and the manifest's
/// own per-package gate rows, so the base document is the recorded manifest rather
/// than a synthetic fixture: a mutation of the real thing is what the audit used
/// to close M3, and it is what these tests refuse.
fn altered_m3_manifest(needle: &str, replacement: &str) -> PathBuf {
    let source = fs::read_to_string(repository_root().join("evidence/m3/manifest.json"))
        .expect("the M3 manifest must be readable");
    let altered = source.replace(needle, replacement);
    assert_ne!(source, altered, "manifest mutation must replace content");

    let path = std::env::temp_dir().join(format!(
        "ravel-m3-{}-{}.json",
        std::process::id(),
        NEXT_PATH.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, altered).expect("altered manifest must be writable");
    path
}

fn m3_violations(manifest: &Path) -> Vec<String> {
    let violations = verify_evidence_manifest(repository_root(), manifest, "M3");
    fs::remove_file(manifest).expect("altered manifest should be removable");
    violations
}

#[test]
fn m3_closure_accepts_the_recorded_prototype_evidence() {
    // Checked against the real repository, so this test states M3's status: every
    // gate row is closed and names its evidence, all four records exist and say
    // what their roadmap rows require, and none of them names a path or a secret.
    // What M3 does not have is a passing review, which is a verdict rule and not
    // one of these.
    let violations = verify_evidence_manifest(
        repository_root(),
        &repository_root().join("evidence/m3/manifest.json"),
        "M3",
    );

    for unmet in [
        "M3 closure requires",
        "M3 manifest must",
        "M3 recorded",
        "evidence/m3/",
    ] {
        assert!(
            !violations
                .iter()
                .any(|violation| violation.starts_with(unmet)),
            "{unmet} should be satisfied, got {violations:?}"
        );
    }
}

#[test]
fn m3_work_packages_must_cover_the_whole_milestone() {
    // The mutation this refuses deleted eight of the nine work packages.
    let violations = m3_violations(&altered_m3_manifest(
        "    \"M3-W1\",\n    \"M3-W2\",\n    \"M3-W3\",\n    \"M3-W4\",\n    \"M3-W5\",\n    \
         \"M3-W6\",\n    \"M3-W7\",\n",
        "",
    ));

    assert!(
        violations.iter().any(|violation| violation
            == "M3 manifest must include every work package from M3-W0 through M3-W8 exactly once"),
        "{violations:?}"
    );
}

#[test]
fn m3_closure_is_refused_when_its_recorded_results_are_emptied() {
    // Emptying `benchmarks.results` takes all four M3 records out from under the
    // digest binding, which is what left them bound by nothing.
    let source = fs::read_to_string(repository_root().join("evidence/m3/manifest.json"))
        .expect("the M3 manifest must be readable");
    let emptied = jq_edit(&source, ".benchmarks.results = []");

    let violations = m3_violations(&write_temporary(&emptied));

    assert!(
        violations.iter().any(|violation| violation.starts_with(
            "M3 manifest must record all four M3 results under the m3-prototype-v1 protocol"
        )),
        "{violations:?}"
    );
}

#[test]
fn m3_closure_is_refused_when_a_gate_row_is_not_closed() {
    // The mutation this refuses set every `gateStatus` row to "failed" and closed
    // anyway, because nothing read the table.
    let violations = m3_violations(&altered_m3_manifest(
        "\"status\": \"closed\"",
        "\"status\": \"failed\"",
    ));

    assert!(
        violations.iter().any(|violation| violation
            .starts_with("M3 closure requires a gateStatus row for every work package")),
        "{violations:?}"
    );
}

#[test]
fn m3_closure_is_refused_when_the_gate_rows_are_deleted() {
    let source = fs::read_to_string(repository_root().join("evidence/m3/manifest.json"))
        .expect("the M3 manifest must be readable");
    let emptied = jq_edit(&source, ".environment.settings.gateStatus = {}");

    let violations = m3_violations(&write_temporary(&emptied));

    assert!(
        violations.iter().any(|violation| violation
            .starts_with("M3 closure requires a gateStatus row for every work package")),
        "{violations:?}"
    );
}

#[test]
fn m3_closure_is_refused_when_its_findings_are_deleted() {
    // `all(. != "blocking")` is true of an empty array, so deleting the findings
    // satisfied the shared rule. M3 recorded nineteen deviations and five risks.
    let source = fs::read_to_string(repository_root().join("evidence/m3/manifest.json"))
        .expect("the M3 manifest must be readable");
    let emptied = jq_edit(&source, ".deviations = [] | .risks = []");

    let violations = m3_violations(&write_temporary(&emptied));

    assert!(
        violations
            .iter()
            .any(|violation| violation.starts_with("M3 recorded deviations and risks")),
        "{violations:?}"
    );
}

#[test]
fn m3_closure_is_refused_without_the_operator_attestation() {
    let source = fs::read_to_string(repository_root().join("evidence/m3/manifest.json"))
        .expect("the M3 manifest must be readable");
    let emptied = jq_edit(&source, ".attestations = []");

    let violations = m3_violations(&write_temporary(&emptied));

    assert!(
        violations.iter().any(|violation| violation
            == "M3 closure requires a native Linux attestation for the operator-run Ravel launch"),
        "{violations:?}"
    );
}

#[test]
fn m3_closure_is_refused_when_a_record_does_not_show_what_its_row_requires() {
    // The records are copied out and one claim in one of them is turned over, so
    // nothing but the rule under test can explain the refusal.
    let root = temporary_root();
    fs::create_dir_all(root.join("evidence/m3")).expect("evidence directory should be creatable");
    for record in [
        "class-loader-identity.json",
        "pack-compile.json",
        "profiling-overhead.json",
        "linux-process-matrix.json",
    ] {
        fs::copy(
            repository_root().join("evidence/m3").join(record),
            root.join("evidence/m3").join(record),
        )
        .expect("M3 record should be copied");
    }
    let matrix = root.join("evidence/m3/linux-process-matrix.json");
    let source = fs::read_to_string(&matrix).expect("the matrix record must be readable");
    let control = verify_evidence_manifest(
        &root,
        &repository_root().join("evidence/m3/manifest.json"),
        "M3",
    );
    assert!(
        !control
            .iter()
            .any(|violation| violation.starts_with("M3 closure requires a Linux process-matrix")),
        "the copied records are the control and must pass, got {control:?}"
    );
    fs::write(
        &matrix,
        jq_edit(&source, ".instance.withoutAnotherLoader = false"),
    )
    .expect("altered record should be writable");

    let violations = verify_evidence_manifest(
        &root,
        &repository_root().join("evidence/m3/manifest.json"),
        "M3",
    );

    fs::remove_dir_all(&root).expect("evidence root should be removable");
    assert!(
        violations
            .iter()
            .any(|violation| violation.starts_with("M3 closure requires a Linux process-matrix")),
        "{violations:?}"
    );
}

#[test]
fn m3_closure_is_refused_when_a_record_is_missing_altogether() {
    let root = temporary_root();
    fs::create_dir_all(root.join("evidence/m3")).expect("evidence directory should be creatable");

    let violations = verify_evidence_manifest(
        &root,
        &repository_root().join("evidence/m3/manifest.json"),
        "M3",
    );

    fs::remove_dir_all(&root).expect("evidence root should be removable");
    let absent = "M3 closure requires the record evidence/m3/linux-process-matrix.json";
    assert!(
        violations.iter().any(|violation| violation == absent),
        "expected {absent}, got {violations:?}"
    );
}

/// Applies a jq program to a document, for a mutation a string replacement cannot
/// express.
fn jq_edit(source: &str, program: &str) -> String {
    use std::io::Write as _;
    use std::process::{Command, Stdio};

    let mut jq = Command::new("jq")
        .arg(program)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("jq should be available");
    jq.stdin
        .as_mut()
        .expect("jq stdin should be piped")
        .write_all(source.as_bytes())
        .expect("the document should be writable to jq");
    let output = jq.wait_with_output().expect("jq should complete");
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).expect("jq output should be UTF-8")
}

/// Writes a document outside the repository and returns its path.
fn write_temporary(body: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ravel-m3-edit-{}-{}.json",
        std::process::id(),
        NEXT_PATH.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, body).expect("altered manifest must be writable");
    path
}
