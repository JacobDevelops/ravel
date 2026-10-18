use std::fs;

use crate::evidence::closure::verify_arithmetic;
use std::path::PathBuf;

use crate::json::read_json;

use crate::evidence::closure::verify_milestone_figures;
use crate::evidence::m6::{M6_ARITHMETIC, M6_CARRIED_FIGURES, M6_FIGURES, verify_m6_manifest};
use crate::test_support::{ClosureCase, misstate_figure, mutate, temporary_root};

const M6_SUMMARY_HEAD: &str = r#"{"milestone":"M6","workPackage":"M6-W6",
    "protocol":"m6-counterpart-v1","result":"pass","#;

const M6_TWO_COUNTERPARTS: &str = r#""wave":{"frozen":true,"entries":[
        {"id":"e1","reason":"foundation value","dependencies":[]},
        {"id":"e2","reason":"cluster partner","dependencies":["e1"]}]},
    "counterparts":[
        {"id":"c1","entry":"e1","specification":{"accepted":true,
            "structureReviewed":true,"carriesUpstreamIdentifier":false,"route":"source"},
         "observableAreas":6,"declarableOnly":4,"exercised":2,
         "provenance":{"complete":true},"suites":{"total":12,"failed":0}},
        {"id":"c2","entry":"e2","specification":{"accepted":true,
            "structureReviewed":true,"carriesUpstreamIdentifier":false,"route":"observed"},
         "observableAreas":4,"declarableOnly":3,"exercised":1,
         "provenance":{"complete":true},"suites":{"total":8,"failed":0}}],
    "processValidation":{"template":"counterpart-template.md",
        "checklist":"counterpart-checklist.md",
        "evidenceSchema":"counterpart.schema.json","secondCounterpart":"c2"},
    "totals":{"counterparts":2,"observableAreas":10,"declarableOnly":7,"exercised":3},"#;

const M6_ONE_COUNTERPART: &str = r#""wave":{"frozen":true,"entries":[
        {"id":"e1","reason":"foundation value","dependencies":[]}]},
    "counterparts":[
        {"id":"c1","entry":"e1","specification":{"accepted":true,
            "structureReviewed":true,"carriesUpstreamIdentifier":false,"route":"source"},
         "observableAreas":6,"declarableOnly":4,"exercised":2,
         "provenance":{"complete":true},"suites":{"total":12,"failed":0}}],
    "processValidation":{"template":"counterpart-template.md",
        "checklist":"counterpart-checklist.md",
        "evidenceSchema":"counterpart.schema.json","secondCounterpart":"c1"},
    "totals":{"counterparts":1,"observableAreas":6,"declarableOnly":4,"exercised":2},"#;

const M6_SUMMARY_TAIL: &str = r#""cluster":{"scenario":"smallest gameplay cluster","steps":[
        {"name":"fresh-play","result":"pass"},{"name":"save","result":"pass"},
        {"name":"exit","result":"pass"},{"name":"reload","result":"pass"}]},
    "rewriteIntegrity":{"enforcing":true,"lexicon":"lexicon.json",
        "identifierSet":"upstream-identifiers.json","filesScanned":214,
        "artifactsScanned":18,"violations":0,"specificationViolations":0,
        "mutationProof":{"sourceInjection":{"exitStatus":1},
            "artifactInjection":{"exitStatus":1},"cleanRun":{"exitStatus":0}}}}"#;

fn balanced_counterpart_summary() -> String {
    [M6_SUMMARY_HEAD, M6_TWO_COUNTERPARTS, M6_SUMMARY_TAIL].concat()
}

fn balanced_m6_manifest() -> String {
    r#"{"workPackages":["M6-W1","M6-W2","M6-W3","M6-W4","M6-W5","M6-W6"],
        "benchmarks":{"protocolVersion":"m6-counterpart-v1",
            "results":[{"name":"counterpart-summary.json"}]},
        "environment":{"settings":{
            "gateStatus":{
                "M6-W1":{"status":"closed","evidence":"wave.json"},
                "M6-W2":{"status":"closed","evidence":"sdk-gaps.md"},
                "M6-W3":{"status":"closed","evidence":"specifications.md"},
                "M6-W4":{"status":"closed","evidence":"cluster.json"},
                "M6-W5":{"status":"closed","evidence":"process.md"},
                "M6-W6":{"status":"open","evidence":""}},
            "rewriteIntegrity":{"enforcing":true,"filesScanned":214,
                "artifactsScanned":18,"violations":0,
                "sourceInjectionExitStatus":1,"artifactInjectionExitStatus":1},
            "counterparts":{"counterparts":2,"waveEntries":2,"observableAreas":10,
                "declarableOnly":7,"exercised":3}}},
        "attestations":[{"platform":"linux","architecture":"x86_64",
            "workloads":["official-atm11","vanilla"]}],
        "deviations":[{"classification":"minor"}],
        "risks":[{"classification":"minor"}]}"#
        .to_owned()
}

fn m6_root(label: &str, summary: &str, manifest: &str) -> PathBuf {
    let root = temporary_root(label);
    fs::create_dir_all(root.join("evidence/m6")).expect("evidence should be creatable");
    fs::write(root.join("evidence/m6/counterpart-summary.json"), summary)
        .expect("summary should be writable");
    fs::write(root.join("evidence/m6/manifest.json"), manifest)
        .expect("manifest should be writable");
    root
}

#[test]
fn a_balanced_counterpart_summary_closes_its_own_arithmetic() {
    let root = m6_root(
        "m6-arithmetic-balanced",
        &balanced_counterpart_summary(),
        &balanced_m6_manifest(),
    );
    let mut violations = Vec::new();

    verify_arithmetic(
        &read_json(&root.join("evidence/m6/counterpart-summary.json"))
            .expect("summary should parse"),
        "evidence/m6/counterpart-summary.json",
        M6_ARITHMETIC,
        &mut violations,
    );

    assert!(
        violations.is_empty(),
        "violations:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn m6_area_arithmetic_mutations(balanced: &str) -> Vec<(usize, String)> {
    vec![
        (
            0,
            mutate(balanced, &[(r#""counterparts":2"#, r#""counterparts":3"#)]),
        ),
        (
            1,
            mutate(
                balanced,
                &[(r#""observableAreas":10"#, r#""observableAreas":11"#)],
            ),
        ),
        (
            2,
            mutate(
                balanced,
                &[(r#""declarableOnly":7"#, r#""declarableOnly":6"#)],
            ),
        ),
        (
            3,
            mutate(balanced, &[(r#""exercised":3"#, r#""exercised":4"#)]),
        ),
        (
            4,
            mutate(
                balanced,
                &[(
                    r#""observableAreas":6,"declarableOnly":4,"exercised":2"#,
                    r#""observableAreas":6,"declarableOnly":4,"exercised":1"#,
                )],
            ),
        ),
        (
            5,
            mutate(
                balanced,
                &[
                    (
                        r#""observableAreas":6,"declarableOnly":4"#,
                        r#""observableAreas":7,"declarableOnly":4"#,
                    ),
                    (r#""observableAreas":10"#, r#""observableAreas":11"#),
                ],
            ),
        ),
        (
            6,
            mutate(
                balanced,
                &[
                    (
                        r#""observableAreas":4,"declarableOnly":3,"exercised":1"#,
                        r#""observableAreas":0,"declarableOnly":0,"exercised":0"#,
                    ),
                    (r#""observableAreas":10"#, r#""observableAreas":6"#),
                    (r#""declarableOnly":7"#, r#""declarableOnly":4"#),
                    (r#""exercised":3"#, r#""exercised":2"#),
                ],
            ),
        ),
    ]
}

fn m6_specification_and_provenance_mutations(balanced: &str) -> Vec<(usize, String)> {
    vec![
        (7, mutate(balanced, &[(r#""id":"e2""#, r#""id":"e9""#)])),
        (
            8,
            mutate(balanced, &[(r#""accepted":true"#, r#""accepted":false"#)]),
        ),
        (
            9,
            mutate(
                balanced,
                &[(
                    r#""carriesUpstreamIdentifier":false"#,
                    r#""carriesUpstreamIdentifier":true"#,
                )],
            ),
        ),
        (
            10,
            mutate(
                balanced,
                &[(
                    r#""structureReviewed":true"#,
                    r#""structureReviewed":false"#,
                )],
            ),
        ),
        (
            11,
            mutate(balanced, &[(r#""complete":true"#, r#""complete":false"#)]),
        ),
        (
            12,
            mutate(
                balanced,
                &[(
                    r#""suites":{"total":12,"failed":0}"#,
                    r#""suites":{"total":12,"failed":1}"#,
                )],
            ),
        ),
    ]
}

fn m6_process_and_integrity_mutations(balanced: &str) -> Vec<(usize, String)> {
    vec![
        (
            13,
            mutate(balanced, &[(M6_TWO_COUNTERPARTS, M6_ONE_COUNTERPART)]),
        ),
        (
            14,
            mutate(
                balanced,
                &[(r#""secondCounterpart":"c2""#, r#""secondCounterpart":"c9""#)],
            ),
        ),
        (
            15,
            mutate(
                balanced,
                &[(
                    r#"{"name":"reload","result":"pass"}"#,
                    r#"{"name":"restart","result":"pass"}"#,
                )],
            ),
        ),
    ]
}

fn m6_integrity_mutations(balanced: &str) -> Vec<(usize, String)> {
    vec![
        (
            16,
            mutate(balanced, &[(r#""enforcing":true"#, r#""enforcing":false"#)]),
        ),
        (
            17,
            mutate(balanced, &[(r#""violations":0,"#, r#""violations":2780,"#)]),
        ),
        (
            18,
            mutate(
                balanced,
                &[(
                    r#""specificationViolations":0"#,
                    r#""specificationViolations":12"#,
                )],
            ),
        ),
        (
            19,
            mutate(
                balanced,
                &[(r#""artifactsScanned":18"#, r#""artifactsScanned":0"#)],
            ),
        ),
        (
            20,
            mutate(
                balanced,
                &[(
                    r#""sourceInjection":{"exitStatus":1}"#,
                    r#""sourceInjection":{"exitStatus":0}"#,
                )],
            ),
        ),
        (
            21,
            mutate(
                balanced,
                &[(
                    r#""artifactInjection":{"exitStatus":1}"#,
                    r#""artifactInjection":{"exitStatus":0}"#,
                )],
            ),
        ),
        (
            22,
            mutate(
                balanced,
                &[(
                    r#""cleanRun":{"exitStatus":0}"#,
                    r#""cleanRun":{"exitStatus":1}"#,
                )],
            ),
        ),
    ]
}

#[test]
fn every_m6_arithmetic_clause_refuses_a_record_that_breaks_it() {
    let balanced = balanced_counterpart_summary();
    let cases: Vec<(usize, String)> = [
        m6_area_arithmetic_mutations(&balanced),
        m6_specification_and_provenance_mutations(&balanced),
        m6_process_and_integrity_mutations(&balanced),
        m6_integrity_mutations(&balanced),
    ]
    .concat();

    let covered: Vec<usize> = cases.iter().map(|(clause, _)| *clause).collect();
    assert_eq!(
        covered,
        (0..M6_ARITHMETIC.len()).collect::<Vec<usize>>(),
        "a clause with no mutation behind it is a clause nothing has seen refuse"
    );

    for (clause, summary) in cases {
        let root = m6_root(
            &format!("m6-arithmetic-{clause}"),
            &summary,
            &balanced_m6_manifest(),
        );
        let mut violations = Vec::new();

        verify_arithmetic(
            &read_json(&root.join("evidence/m6/counterpart-summary.json"))
                .expect("summary should parse"),
            "evidence/m6/counterpart-summary.json",
            M6_ARITHMETIC,
            &mut violations,
        );

        let requirement = M6_ARITHMETIC[clause].1;
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
fn a_complete_m6_record_set_leaves_m6_closure_with_nothing_to_report() {
    let root = m6_root(
        "m6-manifest-balanced",
        &balanced_counterpart_summary(),
        &balanced_m6_manifest(),
    );
    let manifest = root.join("evidence/m6/manifest.json");
    let mut violations = Vec::new();

    verify_m6_manifest(
        &root,
        &read_json(&manifest).expect("manifest should parse"),
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
fn every_m6_closure_clause_refuses_a_manifest_that_breaks_it() {
    let manifest_cases: &[ClosureCase<'_>] = &[
        (
            "work-packages",
            &[(r#","M6-W6"]"#, "]")],
            "M6 manifest must include every work package from M6-W1 through M6-W6 exactly once",
        ),
        (
            "protocol",
            &[(
                r#""protocolVersion":"m6-counterpart-v1""#,
                r#""protocolVersion":"m6-counterpart-v0""#,
            )],
            "M6 manifest must record its counterpart summary under the m6-counterpart-v1",
        ),
        (
            "summary-unnamed",
            &[(
                r#""name":"counterpart-summary.json""#,
                r#""name":"summary.json""#,
            )],
            "M6 manifest must record its counterpart summary under the m6-counterpart-v1",
        ),
        (
            "gate-status",
            &[(
                r#""M6-W3":{"status":"closed""#,
                r#""M6-W3":{"status":"open""#,
            )],
            "M6 closure requires a gateStatus row for every work package",
        ),
        (
            "advisory-check",
            &[(r#""enforcing":true"#, r#""enforcing":false"#)],
            "M6 closure requires the manifest itself to record the rewrite-integrity check",
        ),
        (
            "zero-exit-injection",
            &[(
                r#""artifactInjectionExitStatus":1"#,
                r#""artifactInjectionExitStatus":0"#,
            )],
            "M6 closure requires the manifest itself to record the rewrite-integrity check",
        ),
        (
            "attestation",
            &[(r#""architecture":"x86_64""#, r#""architecture":"aarch64""#)],
            "M6 closure requires a native Linux attestation",
        ),
        (
            "findings",
            &[(
                r#""deviations":[{"classification":"minor"}]"#,
                r#""deviations":[]"#,
            )],
            "recording no deviation or risk deleted the finding rather than the defect",
        ),
    ];

    every_m6_closure_clause_check(manifest_cases);
}

#[test]
fn m6_closure_refuses_a_summary_that_is_absent_or_the_wrong_shape() {
    let root = m6_root(
        "m6-record-shape",
        &mutate(
            &balanced_counterpart_summary(),
            &[(r#""workPackage":"M6-W6""#, r#""workPackage":"M6-W5""#)],
        ),
        &balanced_m6_manifest(),
    );
    let mut violations = Vec::new();

    verify_m6_manifest(
        &root,
        &read_json(&root.join("evidence/m6/manifest.json")).expect("manifest should parse"),
        &mut violations,
    );

    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("M6 closure requires a counterpart summary")),
        "violations:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");

    let root = m6_root(
        "m6-record-absent",
        &balanced_counterpart_summary(),
        &balanced_m6_manifest(),
    );
    fs::remove_file(root.join("evidence/m6/counterpart-summary.json"))
        .expect("summary should be removable");
    let mut violations = Vec::new();

    verify_m6_manifest(
        &root,
        &read_json(&root.join("evidence/m6/manifest.json")).expect("manifest should parse"),
        &mut violations,
    );

    assert!(
        violations.contains(
            &"M6 closure requires the record evidence/m6/counterpart-summary.json".to_owned()
        ),
        "violations:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn every_figure_m6_carries_is_recomputed_from_the_record() {
    for (claimed_at, _) in M6_CARRIED_FIGURES {
        let label = claimed_at.replace('/', "-");
        let root = m6_root(
            &format!("m6-carried{label}"),
            &balanced_counterpart_summary(),
            &balanced_m6_manifest(),
        );
        let manifest = root.join("evidence/m6/manifest.json");
        misstate_figure(&manifest, claimed_at);
        let mut violations = Vec::new();

        verify_milestone_figures(
            &root,
            &read_json(&manifest).expect("manifest should parse"),
            &M6_FIGURES,
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

fn every_m6_closure_clause_check(manifest_cases: &[ClosureCase<'_>]) {
    for (label, replacements, expected) in manifest_cases {
        let root = m6_root(
            &format!("m6-closure-{label}"),
            &balanced_counterpart_summary(),
            &mutate(&balanced_m6_manifest(), replacements),
        );
        let mut violations = Vec::new();

        verify_m6_manifest(
            &root,
            &read_json(&root.join("evidence/m6/manifest.json")).expect("manifest should parse"),
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
fn missing_mutation_exit_status_is_not_a_nonzero_exit() {
    let mut record: serde_json::Value =
        serde_json::from_str(&balanced_counterpart_summary()).expect("summary should parse");
    record["rewriteIntegrity"]["mutationProof"]["sourceInjection"] = serde_json::json!({});
    assert!(!super::counterpart_record(&record));
    assert!(!M6_ARITHMETIC[20].0(&record));
}

#[test]
fn counterpart_schema_refuses_string_counts_and_duplicate_ids() {
    let mut record: serde_json::Value =
        serde_json::from_str(&balanced_counterpart_summary()).expect("summary should parse");
    assert!(super::counterpart_record(&record));
    record["counterparts"][1]["id"] = serde_json::json!("c1");
    assert!(!super::counterpart_record(&record));
    record["counterparts"][1]["id"] = serde_json::json!("c2");
    record["totals"]["observableAreas"] = serde_json::json!("10");
    assert!(!super::counterpart_record(&record));
}
