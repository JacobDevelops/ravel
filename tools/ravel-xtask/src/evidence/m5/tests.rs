use std::fs;

use crate::evidence::closure::verify_arithmetic;
use crate::json::read_json;

use crate::evidence::m5::M5_ARITHMETIC;
use crate::test_support::{balanced_summary, summary_root};

#[test]
fn a_balanced_inventory_summary_closes_its_own_arithmetic() {
    let root = summary_root("m5-arithmetic-balanced", &balanced_summary());
    let mut violations = Vec::new();

    verify_arithmetic(
        &read_json(&root, &root.join("evidence/m5/inventory-summary.json"))
            .expect("summary should parse"),
        "evidence/m5/inventory-summary.json",
        M5_ARITHMETIC,
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
fn missing_sum_operands_cannot_match_a_missing_total() {
    let (rule, _) = M5_ARITHMETIC[0];
    for record in [
        serde_json::json!({}),
        serde_json::json!({"artifacts": {"catalogued": 2}}),
        serde_json::json!({"artifacts": {"catalogued": 2, "total": 2}}),
        serde_json::json!({"artifacts": {"catalogued": 2, "uncatalogued": 0}}),
    ] {
        assert!(!rule(&record), "{record}");
    }
    assert!(rule(&serde_json::json!({
        "artifacts": {"catalogued": 2, "uncatalogued": 0, "total": 2}
    })));
}

#[test]
fn moving_one_part_of_an_inventory_total_stops_it_summing() {
    let root = summary_root(
        "m5-arithmetic-drift",
        &balanced_summary().replace(r#""prohibited":2"#, r#""prohibited":3"#),
    );
    let mut violations = Vec::new();

    verify_arithmetic(
        &read_json(&root, &root.join("evidence/m5/inventory-summary.json"))
            .expect("summary should parse"),
        "evidence/m5/inventory-summary.json",
        M5_ARITHMETIC,
        &mut violations,
    );

    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("exactly one source-use verdict")),
        "violations:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn an_entry_body_read_is_refused_however_the_record_reads() {
    let root = summary_root(
        "m5-arithmetic-reads",
        &balanced_summary().replace(r#""entryBodyReads":0"#, r#""entryBodyReads":1"#),
    );
    let mut violations = Vec::new();

    verify_arithmetic(
        &read_json(&root, &root.join("evidence/m5/inventory-summary.json"))
            .expect("summary should parse"),
        "evidence/m5/inventory-summary.json",
        M5_ARITHMETIC,
        &mut violations,
    );

    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("metadata-only method excludes")),
        "violations:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn missing_inventory_numbers_do_not_balance_as_nulls() {
    let value = serde_json::json!({"artifacts": {"total": null}});
    assert!(!M5_ARITHMETIC[0].0(&value));
    assert!(!M5_ARITHMETIC[2].0(&value));
}

#[test]
fn a_malformed_inventory_summary_reports_the_parse_failure() {
    let root = summary_root("m5-malformed-summary", "not JSON");
    let mut violations = Vec::new();
    let records = crate::evidence::closure::read_records(
        &root,
        crate::evidence::rules::Milestone::M5,
        super::M5_RECORDS,
        &mut violations,
    );
    super::verify_m5_manifest(&serde_json::json!({}), &records, &mut violations);
    assert!(
        violations
            .iter()
            .filter(|message| {
                message.starts_with("cannot parse JSON")
                    && message.contains("inventory-summary.json")
            })
            .count()
            == 1,
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn complete_inventory_summary() -> serde_json::Value {
    let mut record: serde_json::Value =
        serde_json::from_str(&balanced_summary()).expect("summary should parse");
    record["milestone"] = "M5".into();
    record["workPackage"] = "M5-W8".into();
    record["protocol"] = "m5-inventory-v1".into();
    record["result"] = "pass".into();
    record["disclosure"] = serde_json::json!({
        "readObservation": {"bound": "Metadata observations only"},
        "judgedClaims": [{"claim": "Source-use verdict", "basis": "judged", "decidedBy": "inventory reviewer"}],
    });
    for (field, count) in [
        ("classFiles", 4),
        ("models", 2),
        ("textures", 2),
        ("recipes", 2),
        ("nestedJars", 0),
    ] {
        record["surfaces"][field] = count.into();
    }
    record["boundArtifacts"][0]["sha256"] = "0".repeat(64).into();
    record["boundArtifacts"][0]["class"] = "material".into();
    record["primitiveScan"] = serde_json::json!({"class": "machinery"});
    record["machineryAmendments"] = serde_json::json!([]);
    record
}

#[test]
fn inventory_admission_reports_the_typed_numeric_error() {
    let mut record = complete_inventory_summary();
    assert!(super::verify_inventory_record(&record).is_ok());
    record["reads"]["archiveReads"] = serde_json::Value::Null;
    let error = <serde_json::Number as serde::Deserialize>::deserialize(&serde_json::Value::Null)
        .expect_err("null must not deserialize as a number");
    assert_eq!(
        super::verify_inventory_record(&record),
        Err(format!("/reads/archiveReads: {error}"))
    );
}

#[test]
fn bound_artifact_hashes_report_the_canonical_lowercase_error() {
    let mut record = complete_inventory_summary();
    assert!(super::verify_inventory_record(&record).is_ok());
    record["boundArtifacts"][0]["sha256"] = "A".repeat(64).into();
    let error = ravel_core::Sha256Digest::try_from("A".repeat(64))
        .expect_err("uppercase digest must refuse");
    assert_eq!(
        super::verify_inventory_record(&record),
        Err(format!("/boundArtifacts/0/sha256: {error}"))
    );
}
