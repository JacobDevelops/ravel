use std::fs;

use crate::evidence::closure::verify_arithmetic;
use std::path::PathBuf;

use crate::json::read_json;

use crate::evidence::closure::verify_milestone_figures;
use crate::evidence::m7::{
    M7_ARITHMETIC, M7_CARRIED_FIGURES, M7_FIGURES, verify_m7_manifest,
    verify_m7_verified_fixture_total,
};
use crate::test_support::{ClosureCase, misstate_figure, mutate, temporary_root};

fn library_summary(total: u32, correction: &str) -> String {
    format!(
        r#"{{"milestone":"M7","workPackage":"M7-W5","protocol":"m7-library-v1",
            "result":"pass",
            "entries":{{"total":{total},"accepted":{total},"blocked":0,"unstarted":0}},
            "correction":{correction},
            "waves":[{{"index":0,"entries":{first},"dependsOn":[]}},
                {{"index":1,"entries":120,"dependsOn":[0]}},
                {{"index":2,"entries":104,"dependsOn":[0,1]}}],
            "dependencies":{{"resolved":900,"unresolved":0}},
            "registries":{{"resolved":400,"unresolved":0}},
            "provenance":{{"present":{total},"missing":0}},
            "routing":{{"absorbed":40,"remaining":{remaining},
                "runnableToday":{total},"blocked":0}},
            "suites":{{"packages":{{"total":{total},"failed":0}},
                "clusters":{{"total":12,"failed":0}}}}}}"#,
        first = total - 224,
        remaining = total - 40,
    )
}

fn balanced_m7_manifest() -> String {
    r#"{"workPackages":["M7-W0","M7-W1","M7-W2","M7-W3","M7-W4","M7-W5","M7-W6"],
        "benchmarks":{"protocolVersion":"m7-library-v1",
            "results":[{"name":"library-summary.json"}]},
        "environment":{"settings":{
            "gateStatus":{
                "M7-W0":{"status":"closed","evidence":"sdk.md"},
                "M7-W1":{"status":"closed","evidence":"waves.json"},
                "M7-W2":{"status":"closed","evidence":"packages.json"},
                "M7-W3":{"status":"closed","evidence":"clusters.json"},
                "M7-W4":{"status":"closed","evidence":"ledger.json"},
                "M7-W5":{"status":"closed","evidence":"audit.md"},
                "M7-W6":{"status":"open","evidence":""}},
            "library":{"entries":324,"accepted":324,"blocked":0,"unstarted":0,
                "waves":3,"dependenciesUnresolved":0,"registriesUnresolved":0,
                "provenancePresent":324,"absorbed":40,"packageSuites":324,
                "clusterSuites":12}}},
        "attestations":[{"platform":"linux","architecture":"x86_64",
            "workloads":["official-atm11","vanilla"]}],
        "deviations":[{"classification":"minor"}],
        "risks":[{"classification":"minor"}]}"#
        .to_owned()
}

fn m7_root(label: &str, summary: &str, manifest: &str) -> PathBuf {
    let root = temporary_root(label);
    fs::create_dir_all(root.join("evidence/m7")).expect("evidence should be creatable");
    fs::create_dir_all(root.join("evidence/m5")).expect("evidence should be creatable");
    fs::create_dir_all(root.join("tests/fixtures/descriptors"))
        .expect("fixture descriptors should be creatable");
    fs::write(
        root.join("tests/fixtures/descriptors/atm10-tts-2.0.2.json"),
        r#"{"manifest":{"expectedReferenceCount":324}}"#,
    )
    .expect("historical descriptor should be writable");
    fs::write(
        root.join("evidence/m5/inventory-summary.json"),
        r#"{"artifacts":{"total":324}}"#,
    )
    .expect("inventory should be writable");
    fs::write(root.join("evidence/m7/library-summary.json"), summary)
        .expect("summary should be writable");
    fs::write(root.join("evidence/m7/manifest.json"), manifest)
        .expect("manifest should be writable");
    root
}

#[test]
fn a_balanced_library_summary_closes_its_own_arithmetic() {
    let root = m7_root(
        "m7-arithmetic-balanced",
        &library_summary(324, "null"),
        &balanced_m7_manifest(),
    );
    let mut violations = Vec::new();

    verify_arithmetic(
        &read_json(&root, &root.join("evidence/m7/library-summary.json"))
            .expect("summary should parse"),
        "evidence/m7/library-summary.json",
        M7_ARITHMETIC,
        &mut violations,
    );

    assert!(
        violations.is_empty(),
        "violations:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn m7_ledger_mutations(balanced: &str) -> Vec<(usize, String)> {
    vec![
        (
            0,
            mutate(balanced, &[(r#""accepted":324"#, r#""accepted":323"#)]),
        ),
        (
            1,
            mutate(
                balanced,
                &[(
                    r#""accepted":324,"blocked":0"#,
                    r#""accepted":319,"blocked":5"#,
                )],
            ),
        ),
        (
            2,
            mutate(
                balanced,
                &[(
                    r#""accepted":324,"blocked":0"#,
                    r#""accepted":319,"blocked":5"#,
                )],
            ),
        ),
        (
            3,
            mutate(
                balanced,
                &[(
                    r#""dependencies":{"resolved":900,"unresolved":0}"#,
                    r#""dependencies":{"resolved":900,"unresolved":3}"#,
                )],
            ),
        ),
        (
            4,
            mutate(
                balanced,
                &[(
                    r#""registries":{"resolved":400,"unresolved":0}"#,
                    r#""registries":{"resolved":400,"unresolved":7}"#,
                )],
            ),
        ),
        (5, mutate(balanced, &[(r#""missing":0"#, r#""missing":2"#)])),
    ]
}

fn m7_wave_and_routing_mutations(balanced: &str) -> Vec<(usize, String)> {
    vec![
        (
            6,
            mutate(
                balanced,
                &[(r#""index":1,"entries":120"#, r#""index":1,"entries":121"#)],
            ),
        ),
        (7, mutate(balanced, &[(r#""index":2"#, r#""index":1"#)])),
        (
            8,
            mutate(
                balanced,
                &[(r#""dependsOn":[0,1]"#, r#""dependsOn":[0,5]"#)],
            ),
        ),
        (
            9,
            mutate(
                balanced,
                &[(
                    r#""index":0,"entries":100,"dependsOn":[]"#,
                    r#""index":0,"entries":100,"dependsOn":[2]"#,
                )],
            ),
        ),
        (
            10,
            mutate(balanced, &[(r#""absorbed":40"#, r#""absorbed":50"#)]),
        ),
        (
            11,
            mutate(
                balanced,
                &[(r#""runnableToday":324"#, r#""runnableToday":323"#)],
            ),
        ),
        (
            12,
            mutate(
                balanced,
                &[(
                    r#""runnableToday":324,"blocked":0"#,
                    r#""runnableToday":0,"blocked":324"#,
                )],
            ),
        ),
    ]
}

fn m7_suite_mutations(balanced: &str) -> Vec<(usize, String)> {
    vec![
        (
            13,
            mutate(
                balanced,
                &[(
                    r#""packages":{"total":324,"failed":0}"#,
                    r#""packages":{"total":324,"failed":3}"#,
                )],
            ),
        ),
        (
            14,
            mutate(
                balanced,
                &[(
                    r#""clusters":{"total":12,"failed":0}"#,
                    r#""clusters":{"total":12,"failed":1}"#,
                )],
            ),
        ),
    ]
}

#[test]
fn every_m7_arithmetic_clause_refuses_a_record_that_breaks_it() {
    let balanced = library_summary(324, "null");
    let cases: Vec<(usize, String)> = [
        m7_ledger_mutations(&balanced),
        m7_wave_and_routing_mutations(&balanced),
        m7_suite_mutations(&balanced),
    ]
    .concat();

    let covered: Vec<usize> = cases.iter().map(|(clause, _)| *clause).collect();
    assert_eq!(
        covered,
        (0..M7_ARITHMETIC.len()).collect::<Vec<usize>>(),
        "a clause with no mutation behind it is a clause nothing has seen refuse"
    );

    for (clause, summary) in cases {
        let root = m7_root(
            &format!("m7-arithmetic-{clause}"),
            &summary,
            &balanced_m7_manifest(),
        );
        let mut violations = Vec::new();

        verify_arithmetic(
            &read_json(&root, &root.join("evidence/m7/library-summary.json"))
                .expect("summary should parse"),
            "evidence/m7/library-summary.json",
            M7_ARITHMETIC,
            &mut violations,
        );

        let requirement = M7_ARITHMETIC[clause].1;
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains(requirement)),
            "clause {clause} was not refused; violations:\n{}",
            violations.join("\n")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}

#[test]
fn m7_audits_the_total_the_inventory_verified_or_a_correction_that_names_it() {
    let corrected = r#"{"from":324,"total":300,"reason":"two entries were one release"}"#;
    let cases: &[(&str, String, bool)] = &[
        ("matching", library_summary(324, "null"), true),
        ("corrected", library_summary(300, corrected), true),
        ("uncorrected", library_summary(300, "null"), false),
        (
            "corrected-from-elsewhere",
            library_summary(
                300,
                r#"{"from":301,"total":300,"reason":"a figure nothing verified"}"#,
            ),
            false,
        ),
    ];

    for (label, summary, reconciles) in cases {
        let root = m7_root(
            &format!("m7-fixture-total-{label}"),
            summary,
            &balanced_m7_manifest(),
        );
        let mut violations = Vec::new();

        verify_m7_verified_fixture_total(
            &root,
            &read_json(&root, &root.join("evidence/m7").join(M7_FIGURES.record))
                .expect("summary should parse"),
            &mut violations,
        );

        assert_eq!(
            violations.is_empty(),
            *reconciles,
            "{label}; violations:\n{}",
            violations.join("\n")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}

#[test]
fn historical_protocol_preserves_reference_arithmetic_and_missing_inventory_behavior() {
    let mut decimal: serde_json::Value =
        serde_json::from_str(&library_summary(324, "null")).expect("summary should parse");
    decimal["entries"]["total"] = serde_json::json!(324.0);
    let root = m7_root(
        "m7-legacy-inventory",
        &library_summary(235, "null"),
        &balanced_m7_manifest(),
    );
    fs::write(
        root.join("evidence/m5/inventory-summary.json"),
        r#"{"artifacts":{"total":235}}"#,
    )
    .expect("inventory should be writable");
    let mut violations = Vec::new();
    verify_arithmetic(
        &read_json(&root, &root.join("evidence/m7/library-summary.json"))
            .expect("summary should parse"),
        "evidence/m7/library-summary.json",
        M7_ARITHMETIC,
        &mut violations,
    );
    verify_m7_verified_fixture_total(
        &root,
        &read_json(&root, &root.join("evidence/m7").join(M7_FIGURES.record))
            .expect("summary should parse"),
        &mut violations,
    );
    assert!(
        !violations.is_empty(),
        "235 without a correction must not close v1"
    );
    fs::write(
        root.join("evidence/m7/library-summary.json"),
        serde_json::to_vec(&decimal).expect("summary should serialize"),
    )
    .expect("summary should be writable");
    fs::remove_file(root.join("evidence/m5/inventory-summary.json"))
        .expect("inventory should be removable");
    violations.clear();
    verify_m7_verified_fixture_total(
        &root,
        &read_json(&root, &root.join("evidence/m7").join(M7_FIGURES.record))
            .expect("summary should parse"),
        &mut violations,
    );
    assert!(violations.is_empty(), "{violations:?}");
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn selected_workload_root(label: &str) -> (PathBuf, serde_json::Value) {
    let root = m7_root(
        label,
        &library_summary(259, "null"),
        &balanced_m7_manifest(),
    );
    let fixture = serde_json::json!({
        "id": "sevtech-ages-3.2.3",
        "artifactSet": {"count": 259, "sha256": "a".repeat(64)}
    });
    let directory = root.join("registry/inventory/sevtech-ages-3.2.3");
    fs::create_dir_all(&directory).expect("inventory directory should be creatable");
    fs::write(
        directory.join("graph.json"),
        serde_json::to_vec(&serde_json::json!({"fixture": fixture}))
            .expect("inventory should serialize"),
    )
    .expect("inventory should be writable");
    let path = root.join("evidence/m7/library-summary.json");
    let mut record = read_json(&root, &path).expect("summary should parse");
    record["fixture"] = fixture;
    record["protocol"] = "m7-library-v2".into();
    (root, record)
}

#[test]
fn selected_workload_uses_its_jar_inventory_not_historical_reference_totals() {
    let (root, mut record) = selected_workload_root("m7-sevtech-total");
    let path = root.join("evidence/m7/library-summary.json");
    for (label, total, digest, accepted) in [
        ("selected", 259, "a", true),
        ("reference-only", 254, "a", false),
        ("historical", 324, "a", false),
        ("tampered-binding", 259, "b", false),
    ] {
        record["entries"]["total"] = total.into();
        record["fixture"]["artifactSet"]["sha256"] = digest.repeat(64).into();
        fs::write(
            &path,
            serde_json::to_vec(&record).expect("summary should serialize"),
        )
        .expect("summary should be writable");
        let mut violations = Vec::new();
        verify_m7_verified_fixture_total(
            &root,
            &read_json(&root, &root.join("evidence/m7").join(M7_FIGURES.record))
                .expect("summary should parse"),
            &mut violations,
        );
        assert_eq!(violations.is_empty(), accepted, "{label}: {violations:?}");
    }
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn selected_workload_rejects_matching_uppercase_artifact_set_digests() {
    let (root, mut record) = selected_workload_root("m7-uppercase-artifact-set");
    record["fixture"]["artifactSet"]["sha256"] = "A".repeat(64).into();
    fs::write(
        root.join("registry/inventory/sevtech-ages-3.2.3/graph.json"),
        serde_json::to_vec(&serde_json::json!({"fixture": record["fixture"]}))
            .expect("inventory should serialize"),
    )
    .expect("inventory should be writable");
    let mut violations = Vec::new();
    verify_m7_verified_fixture_total(&root, &record, &mut violations);
    let error = ravel_core::Sha256Digest::try_from("A".repeat(64))
        .expect_err("uppercase digest must refuse");
    assert_eq!(
        violations,
        [format!(
            "evidence/m7/library-summary.json: invalid library summary: /fixture/artifactSet/sha256: {error}"
        )]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn selected_workload_rejects_an_uppercase_inventory_digest_with_a_valid_record_pin() {
    let (root, record) = selected_workload_root("m7-uppercase-inventory-artifact-set");
    assert!(super::verify_library_record(&record).is_ok());
    let mut inventory = serde_json::json!({"fixture": record["fixture"]});
    inventory["fixture"]["artifactSet"]["sha256"] = "A".repeat(64).into();
    fs::write(
        root.join("registry/inventory/sevtech-ages-3.2.3/graph.json"),
        inventory.to_string(),
    )
    .expect("inventory should be writable");
    let mut violations = Vec::new();
    verify_m7_verified_fixture_total(&root, &record, &mut violations);
    let prefix = "M7 fixture binding and entries.total must match sevtech-ages-3.2.3's verified artifact-set count and digest: failed";
    assert_eq!(
        violations,
        [
            format!("{prefix} fixture.artifactSet.sha256 must be a SHA-256 digest"),
            format!("{prefix} fixture.artifactSet"),
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn selected_workload_compares_the_entire_artifact_set() {
    let (root, baseline) = selected_workload_root("m7-complete-artifact-set-binding");
    for (label, recorded_extra, inventoried_extra, accepted) in [
        ("record-only", Some(1), None, false),
        ("inventory-only", None, Some(1), false),
        ("different", Some(1), Some(2), false),
        ("matching", Some(1), Some(1), true),
    ] {
        let mut record = baseline.clone();
        let mut inventory = serde_json::json!({"fixture": baseline["fixture"]});
        if let Some(extra) = recorded_extra {
            record["fixture"]["artifactSet"]["additionalFigure"] = extra.into();
        }
        if let Some(extra) = inventoried_extra {
            inventory["fixture"]["artifactSet"]["additionalFigure"] = extra.into();
        }
        assert_eq!(super::verify_library_record(&record), Ok(()), "{label}");
        fs::write(
            root.join("registry/inventory/sevtech-ages-3.2.3/graph.json"),
            inventory.to_string(),
        )
        .expect("inventory should be writable");
        let mut violations = Vec::new();
        verify_m7_verified_fixture_total(&root, &record, &mut violations);
        let expected = if accepted {
            Vec::new()
        } else {
            vec![
                "M7 fixture binding and entries.total must match sevtech-ages-3.2.3's verified artifact-set count and digest: failed fixture.artifactSet".to_owned()
            ]
        };
        assert_eq!(violations, expected, "{label}");
    }
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn selected_workload_requires_a_binding_and_matching_protocol() {
    let (root, record) = selected_workload_root("m7-sevtech-protocol");
    let path = root.join("evidence/m7/library-summary.json");
    assert!(super::verify_library_record(&record).is_ok());
    assert!(M7_ARITHMETIC.iter().all(|(rule, _)| rule(&record)));
    fs::write(
        &path,
        serde_json::to_vec(&record).expect("summary should serialize"),
    )
    .expect("summary should be writable");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&balanced_m7_manifest()).expect("manifest should parse");
    let mut violations = Vec::new();
    let records = crate::evidence::closure::read_records(
        &root,
        crate::evidence::rules::Milestone::M7,
        super::M7_RECORDS,
        &mut violations,
    );
    verify_m7_manifest(&manifest, &records, &mut violations);
    assert!(
        violations
            .iter()
            .any(|error| error.contains("same protocol")),
        "{violations:?}"
    );
    manifest["benchmarks"]["protocolVersion"] = "m7-library-v2".into();
    violations.clear();
    let records = crate::evidence::closure::read_records(
        &root,
        crate::evidence::rules::Milestone::M7,
        super::M7_RECORDS,
        &mut violations,
    );
    verify_m7_manifest(&manifest, &records, &mut violations);
    assert!(violations.is_empty(), "{violations:?}");
    let mut fractional = record.clone();
    fractional["entries"]["total"] = serde_json::json!(259.0);
    assert!(super::verify_library_record(&fractional).is_err());
    for (protocol, fixture) in [
        ("m7-library-v2", serde_json::Value::Null),
        ("m7-library-v1", record["fixture"].clone()),
    ] {
        let mut stripped = record.clone();
        stripped["protocol"] = protocol.into();
        stripped["fixture"] = fixture;
        stripped["correction"] = serde_json::json!({
            "from": 324, "total": 259, "reason": "binding removed"
        });
        assert!(super::verify_library_record(&stripped).is_err());
        fs::write(
            &path,
            serde_json::to_vec(&stripped).expect("summary should serialize"),
        )
        .expect("summary should be writable");
        let mut violations = Vec::new();
        verify_m7_verified_fixture_total(
            &root,
            &read_json(&root, &root.join("evidence/m7").join(M7_FIGURES.record))
                .expect("summary should parse"),
            &mut violations,
        );
        assert!(!violations.is_empty(), "{protocol}: {violations:?}");
    }
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_complete_m7_record_set_leaves_m7_closure_with_nothing_to_report() {
    let root = m7_root(
        "m7-manifest-balanced",
        &library_summary(324, "null"),
        &balanced_m7_manifest(),
    );
    let mut violations = Vec::new();

    let records = crate::evidence::closure::read_records(
        &root,
        crate::evidence::rules::Milestone::M7,
        super::M7_RECORDS,
        &mut violations,
    );
    verify_m7_manifest(
        &read_json(&root, &root.join("evidence/m7/manifest.json")).expect("manifest should parse"),
        &records,
        &mut violations,
    );

    assert!(
        violations.is_empty(),
        "violations:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn every_m7_closure_clause_refuses_a_manifest_that_breaks_it() {
    let manifest_cases: &[ClosureCase<'_>] = &[
        (
            "work-packages",
            &[(r#","M7-W6"]"#, "]")],
            "M7 manifest must include every work package from M7-W0 through M7-W6 exactly once",
        ),
        (
            "protocol",
            &[(
                r#""protocolVersion":"m7-library-v1""#,
                r#""protocolVersion":"m7-library-v0""#,
            )],
            "M7 manifest must record its library summary under the m7-library-v1",
        ),
        (
            "gate-status",
            &[(
                r#""M7-W4":{"status":"closed""#,
                r#""M7-W4":{"status":"open""#,
            )],
            "M7 closure requires a gateStatus row for every work package",
        ),
        (
            "attestation",
            &[(r#""architecture":"x86_64""#, r#""architecture":"aarch64""#)],
            "M7 closure requires a native Linux attestation",
        ),
        (
            "findings",
            &[(r#""risks":[{"classification":"minor"}]"#, r#""risks":[]"#)],
            "an empty finding set satisfies every rule phrased as a refusal",
        ),
    ];

    for (label, replacements, expected) in manifest_cases {
        let root = m7_root(
            &format!("m7-closure-{label}"),
            &library_summary(324, "null"),
            &mutate(&balanced_m7_manifest(), replacements),
        );
        let mut violations = Vec::new();

        let records = crate::evidence::closure::read_records(
            &root,
            crate::evidence::rules::Milestone::M7,
            super::M7_RECORDS,
            &mut violations,
        );
        verify_m7_manifest(
            &read_json(&root, &root.join("evidence/m7/manifest.json"))
                .expect("manifest should parse"),
            &records,
            &mut violations,
        );

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains(expected)),
            "{label} was not refused; violations:\n{}",
            violations.join("\n")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}

#[test]
fn m7_closure_refuses_a_summary_that_is_absent_or_the_wrong_shape() {
    let root = m7_root(
        "m7-record-shape",
        &mutate(
            &library_summary(324, "null"),
            &[(r#""result":"pass""#, r#""result":"provisional""#)],
        ),
        &balanced_m7_manifest(),
    );
    let mut violations = Vec::new();

    let records = crate::evidence::closure::read_records(
        &root,
        crate::evidence::rules::Milestone::M7,
        super::M7_RECORDS,
        &mut violations,
    );
    verify_m7_manifest(
        &read_json(&root, &root.join("evidence/m7/manifest.json")).expect("manifest should parse"),
        &records,
        &mut violations,
    );

    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("M7 closure requires a library summary")),
        "violations:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");

    let root = m7_root(
        "m7-record-absent",
        &library_summary(324, "null"),
        &balanced_m7_manifest(),
    );
    fs::remove_file(root.join("evidence/m7/library-summary.json"))
        .expect("summary should be removable");
    let mut violations = Vec::new();

    let records = crate::evidence::closure::read_records(
        &root,
        crate::evidence::rules::Milestone::M7,
        super::M7_RECORDS,
        &mut violations,
    );
    verify_m7_manifest(
        &read_json(&root, &root.join("evidence/m7/manifest.json")).expect("manifest should parse"),
        &records,
        &mut violations,
    );

    assert!(
        violations.contains(
            &"M7 closure requires the record evidence/m7/library-summary.json".to_owned()
        ),
        "violations:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn every_figure_m7_carries_is_recomputed_from_the_record() {
    for (claimed_at, _) in M7_CARRIED_FIGURES {
        let label = claimed_at.replace('/', "-");
        let root = m7_root(
            &format!("m7-carried{label}"),
            &library_summary(324, "null"),
            &balanced_m7_manifest(),
        );
        let manifest = root.join("evidence/m7/manifest.json");
        misstate_figure(&root, &manifest, claimed_at);
        let mut violations = Vec::new();

        verify_milestone_figures(
            crate::evidence::rules::Milestone::M7,
            &read_json(&root, &root.join("evidence/m7").join(M7_FIGURES.record))
                .expect("summary should parse"),
            &read_json(&root, &manifest).expect("manifest should parse"),
            &M7_FIGURES,
            &mut violations,
        );

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains(&format!("records {claimed_at} 987654"))),
            "{claimed_at} was not reconciled; violations:\n{}",
            violations.join("\n")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}

#[test]
fn absent_fixture_totals_are_unreadable_not_matching_nulls() {
    let root = m7_root(
        "m7-absent-totals",
        &library_summary(324, "null"),
        &balanced_m7_manifest(),
    );
    fs::write(root.join("evidence/m5/inventory-summary.json"), "{}")
        .expect("inventory should be writable");
    let mut violations = Vec::new();
    verify_m7_verified_fixture_total(
        &root,
        &read_json(&root, &root.join("evidence/m7").join(M7_FIGURES.record))
            .expect("summary should parse"),
        &mut violations,
    );
    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains("cannot both be read"));
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn an_unparseable_fixture_total_reports_the_parse_error() {
    let root = m7_root(
        "m7-unparseable-total",
        &library_summary(324, "null"),
        &balanced_m7_manifest(),
    );
    fs::write(root.join("evidence/m5/inventory-summary.json"), "not JSON")
        .expect("inventory should be writable");
    let mut violations = Vec::new();
    verify_m7_verified_fixture_total(
        &root,
        &read_json(&root, &root.join("evidence/m7").join(M7_FIGURES.record))
            .expect("summary should parse"),
        &mut violations,
    );
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("inventory-summary.json"),
        "{violations:?}"
    );
    assert!(
        !violations[0].contains("cannot both be read"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn library_schema_refuses_nonnumeric_wave_dependencies() {
    let mut record: serde_json::Value =
        serde_json::from_str(&library_summary(324, "null")).expect("summary should parse");
    assert!(super::verify_library_record(&record).is_ok());
    record["waves"][1]["dependsOn"] = serde_json::json!(["0"]);
    assert!(super::verify_library_record(&record).is_err());
}

#[test]
fn a_malformed_library_summary_reports_the_parse_failure() {
    let root = temporary_root("m7-malformed-summary");
    fs::create_dir_all(root.join("evidence/m7")).expect("evidence directory should be creatable");
    fs::write(root.join("evidence/m7/library-summary.json"), "not JSON")
        .expect("record should be writable");
    let mut violations = Vec::new();
    let records = crate::evidence::closure::read_records(
        &root,
        crate::evidence::rules::Milestone::M7,
        super::M7_RECORDS,
        &mut violations,
    );
    verify_m7_manifest(&serde_json::json!({}), &records, &mut violations);
    assert!(
        violations
            .iter()
            .filter(|message| {
                message.starts_with("cannot parse JSON") && message.contains("library-summary.json")
            })
            .count()
            == 1,
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn v2_shape_failure_is_not_an_arithmetic_failure() {
    let (root, mut record) = selected_workload_root("m7-v2-shape-clause");
    record["fixture"] = serde_json::Value::Null;
    let error =
        <super::model::Fixture<'_> as serde::Deserialize>::deserialize(&serde_json::Value::Null)
            .err()
            .expect("null must not deserialize as a fixture");
    assert_eq!(
        super::verify_library_record(&record),
        Err(format!("invalid library summary: /fixture: {error}"))
    );
    assert!(
        M7_ARITHMETIC.iter().all(|(rule, _)| rule(&record)),
        "v2 protocol shape belongs to the record validator"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn open_milestone_tolerates_absence_but_closure_requires_the_record() {
    let root = m7_root(
        "m7-open-record-absence",
        &library_summary(259, "null"),
        &balanced_m7_manifest(),
    );
    fs::remove_file(root.join("evidence/m7").join(M7_FIGURES.record))
        .expect("summary should be removable");
    let manifest: serde_json::Value =
        serde_json::from_str(&balanced_m7_manifest()).expect("manifest should parse");
    let mut violations = Vec::new();
    let records = super::verify_m7_integrity(&root, &manifest, &mut violations);
    assert!(violations.is_empty(), "{violations:?}");
    verify_m7_manifest(&manifest, &records, &mut violations);
    assert_eq!(
        violations
            .iter()
            .filter(|violation| {
                violation.as_str()
                    == "M7 closure requires the record evidence/m7/library-summary.json"
            })
            .count(),
        1,
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn v2_correction_is_rejected_during_integrity_not_only_closure() {
    let (root, mut record) = selected_workload_root("m7-v2-correction-integrity");
    record["correction"] = serde_json::json!({"from":235,"total":259,"reason":"not allowed in v2"});
    fs::write(
        root.join("evidence/m7").join(M7_FIGURES.record),
        record.to_string(),
    )
    .expect("summary should write");
    let manifest: serde_json::Value =
        serde_json::from_str(&balanced_m7_manifest()).expect("manifest should parse");
    let mut violations = Vec::new();
    super::verify_m7_integrity(&root, &manifest, &mut violations);
    let error = <() as serde::Deserialize>::deserialize(&record["correction"])
        .expect_err("a v2 correction must be null");
    let requirement = format!("invalid library summary: /correction: {error}");
    assert!(
        violations
            .iter()
            .any(|violation| violation.ends_with(&requirement)),
        "{violations:?}"
    );
    assert_eq!(super::verify_library_record(&record), Err(requirement));
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn typed_wave_admission_reports_the_typed_numeric_error() {
    let (root, mut record) = selected_workload_root("m7-wave-row-clause");
    record["waves"][0]["entries"] = serde_json::Value::Null;
    let error = <serde_json::Number as serde::Deserialize>::deserialize(&serde_json::Value::Null)
        .expect_err("null must not deserialize as a number");
    assert_eq!(
        super::verify_library_record(&record),
        Err(format!(
            "invalid library summary: /waves/0/entries: {error}"
        ))
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn integrity_requires_an_explicit_supported_protocol() {
    let mut record: serde_json::Value =
        serde_json::from_str(&library_summary(324, "null")).expect("summary should parse");
    record
        .as_object_mut()
        .expect("summary should be an object")
        .remove("protocol");
    let root = m7_root(
        "m7-open-no-protocol",
        &record.to_string(),
        &balanced_m7_manifest(),
    );
    let manifest: serde_json::Value =
        serde_json::from_str(&balanced_m7_manifest()).expect("manifest should parse");
    let mut violations = Vec::new();
    super::verify_m7_integrity(&root, &manifest, &mut violations);
    assert_eq!(
        violations,
        ["evidence/m7/library-summary.json: invalid library summary: /: missing field `protocol`"]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn legacy_integrity_reports_an_invalid_fixture_once() {
    let mut record: serde_json::Value =
        serde_json::from_str(&library_summary(324, "null")).expect("summary should parse");
    record["fixture"] = serde_json::json!({"id":"not-valid-for-v1"});
    let root = m7_root(
        "m7-v1-fixture-once",
        &record.to_string(),
        &balanced_m7_manifest(),
    );
    let manifest: serde_json::Value =
        serde_json::from_str(&balanced_m7_manifest()).expect("manifest should parse");
    let mut violations = Vec::new();
    super::verify_m7_integrity(&root, &manifest, &mut violations);
    let error = <() as serde::Deserialize>::deserialize(&record["fixture"])
        .expect_err("a v1 fixture must be null");
    assert_eq!(
        violations,
        [format!(
            "evidence/m7/library-summary.json: invalid library summary: /fixture: {error}"
        )]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn historical_fixture_failures_name_the_runtime_descriptor_not_the_evidence_rule() {
    let root = m7_root(
        "m7-runtime-descriptor",
        &library_summary(324, "null"),
        &balanced_m7_manifest(),
    );
    let relative = "tests/fixtures/descriptors/atm10-tts-2.0.2.json";
    let path = root.join(relative);
    assert_eq!(super::legacy_fixture_total(&root), Ok(324.0));
    fs::write(&path, "{").expect("descriptor should be writable");
    let malformed =
        super::legacy_fixture_total(&root).expect_err("malformed runtime descriptor must refuse");
    let reason = serde_json::from_slice::<serde_json::Value>(b"{")
        .expect_err("an unfinished JSON object must refuse");
    assert_eq!(malformed, format!("cannot parse JSON {relative}: {reason}"));
    for source in ["{}", r#"{"manifest":{"expectedReferenceCount":"259"}}"#] {
        fs::write(&path, source).expect("descriptor should be writable");
        assert_eq!(
            super::legacy_fixture_total(&root),
            Err(format!(
                "{relative}: missing numeric /manifest/expectedReferenceCount"
            ))
        );
    }
    fs::write(&path, r#"{"manifest":{"expectedReferenceCount":259}}"#)
        .expect("descriptor should be writable");
    assert_eq!(super::legacy_fixture_total(&root), Ok(259.0));
    let record: serde_json::Value =
        serde_json::from_str(&library_summary(324, "null")).expect("summary should parse");
    let mut violations = Vec::new();
    verify_m7_verified_fixture_total(&root, &record, &mut violations);
    assert_eq!(
        violations,
        ["evidence/m7/library-summary.json audits 324 entries while \
         tests/fixtures/descriptors/atm10-tts-2.0.2.json pins 259; \
         M7-W5 must audit the pinned historical total or an explicit correction"]
    );
    fs::remove_file(&path).expect("descriptor should be removable");
    let missing =
        super::legacy_fixture_total(&root).expect_err("missing runtime descriptor must refuse");
    assert!(
        missing.starts_with(&format!("cannot read JSON {relative}: ")),
        "{missing}"
    );
    violations.clear();
    verify_m7_verified_fixture_total(&root, &record, &mut violations);
    assert_eq!(violations, [missing]);
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
