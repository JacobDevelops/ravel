use crate::inventory::{verify_harness_lifecycles, verify_review_round_counts};

#[test]
fn the_tracked_m3_records_leave_the_round_count_to_the_record_that_counts() {
    let root = std::env::current_dir()
        .expect("the current directory must be available")
        .join("../..");
    let mut violations = Vec::new();

    verify_review_round_counts(&root, &mut violations);

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn the_tracked_m3_records_agree_with_the_harnesss_declared_world_lifecycles() {
    let root = std::env::current_dir()
        .expect("the current directory must be available")
        .join("../..");
    let mut violations = Vec::new();

    verify_harness_lifecycles(&root, &mut violations);

    assert_eq!(violations, Vec::<String>::new());
}
