use std::fs;
use std::path::PathBuf;

use crate::evidence::closure::verify_milestone_figures;
use crate::evidence::m7::{
    M7_ARITHMETIC, M7_CARRIED_FIGURES, M7_FIGURES, verify_m7_library_arithmetic,
    verify_m7_manifest, verify_m7_verified_fixture_total,
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

    verify_m7_library_arithmetic(&root, &mut violations);

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
        (3, library_summary(300, "null")),
        (
            4,
            mutate(
                balanced,
                &[(
                    r#""dependencies":{"resolved":900,"unresolved":0}"#,
                    r#""dependencies":{"resolved":900,"unresolved":3}"#,
                )],
            ),
        ),
        (
            5,
            mutate(
                balanced,
                &[(
                    r#""registries":{"resolved":400,"unresolved":0}"#,
                    r#""registries":{"resolved":400,"unresolved":7}"#,
                )],
            ),
        ),
        (6, mutate(balanced, &[(r#""missing":0"#, r#""missing":2"#)])),
    ]
}

fn m7_wave_routing_and_suite_mutations(balanced: &str) -> Vec<(usize, String)> {
    vec![
        (
            7,
            mutate(
                balanced,
                &[(r#""index":1,"entries":120"#, r#""index":1,"entries":121"#)],
            ),
        ),
        (8, mutate(balanced, &[(r#""index":2"#, r#""index":1"#)])),
        (
            9,
            mutate(
                balanced,
                &[(r#""dependsOn":[0,1]"#, r#""dependsOn":[0,5]"#)],
            ),
        ),
        (
            10,
            mutate(
                balanced,
                &[(
                    r#""index":0,"entries":100,"dependsOn":[]"#,
                    r#""index":0,"entries":100,"dependsOn":[2]"#,
                )],
            ),
        ),
        (
            11,
            mutate(balanced, &[(r#""absorbed":40"#, r#""absorbed":50"#)]),
        ),
        (
            12,
            mutate(
                balanced,
                &[(r#""runnableToday":324"#, r#""runnableToday":323"#)],
            ),
        ),
        (
            13,
            mutate(
                balanced,
                &[(
                    r#""runnableToday":324,"blocked":0"#,
                    r#""runnableToday":0,"blocked":324"#,
                )],
            ),
        ),
        (
            14,
            mutate(
                balanced,
                &[(
                    r#""packages":{"total":324,"failed":0}"#,
                    r#""packages":{"total":324,"failed":3}"#,
                )],
            ),
        ),
        (
            15,
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
        m7_wave_routing_and_suite_mutations(&balanced),
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

        verify_m7_library_arithmetic(&root, &mut violations);

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

        verify_m7_verified_fixture_total(&root, &mut violations);

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
fn a_complete_m7_record_set_leaves_m7_closure_with_nothing_to_report() {
    let root = m7_root(
        "m7-manifest-balanced",
        &library_summary(324, "null"),
        &balanced_m7_manifest(),
    );
    let mut violations = Vec::new();

    verify_m7_manifest(
        &root,
        &root.join("evidence/m7/manifest.json"),
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
    let manifest_cases: &[ClosureCase] = &[
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

        verify_m7_manifest(
            &root,
            &root.join("evidence/m7/manifest.json"),
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

    verify_m7_manifest(
        &root,
        &root.join("evidence/m7/manifest.json"),
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

    verify_m7_manifest(
        &root,
        &root.join("evidence/m7/manifest.json"),
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
        let label = claimed_at.replace('.', "-");
        let root = m7_root(
            &format!("m7-carried{label}"),
            &library_summary(324, "null"),
            &balanced_m7_manifest(),
        );
        let manifest = root.join("evidence/m7/manifest.json");
        misstate_figure(&manifest, claimed_at);
        let mut violations = Vec::new();

        verify_milestone_figures(&root, &manifest, &M7_FIGURES, &mut violations);

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
