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
        &read_json(&root.join("evidence/m5/inventory-summary.json")).expect("summary should parse"),
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
fn moving_one_part_of_an_inventory_total_stops_it_summing() {
    let root = summary_root(
        "m5-arithmetic-drift",
        &balanced_summary().replace(r#""prohibited":2"#, r#""prohibited":3"#),
    );
    let mut violations = Vec::new();

    verify_arithmetic(
        &read_json(&root.join("evidence/m5/inventory-summary.json")).expect("summary should parse"),
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
        &read_json(&root.join("evidence/m5/inventory-summary.json")).expect("summary should parse"),
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
    super::verify_m5_manifest(&root, &serde_json::json!({}), &mut violations);
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
