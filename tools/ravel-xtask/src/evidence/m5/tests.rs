use std::fs;

use crate::evidence::m5::verify_m5_inventory_arithmetic;
use crate::test_support::{balanced_summary, summary_root};

#[test]
fn a_balanced_inventory_summary_closes_its_own_arithmetic() {
    let root = summary_root("m5-arithmetic-balanced", &balanced_summary());
    let mut violations = Vec::new();

    verify_m5_inventory_arithmetic(&root, &mut violations);

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

    verify_m5_inventory_arithmetic(&root, &mut violations);

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

    verify_m5_inventory_arithmetic(&root, &mut violations);

    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("metadata-only method excludes")),
        "violations:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
