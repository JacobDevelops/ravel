use std::fs;

use super::{
    round_count_restatements, round_count_subjects, verify_manifest_percentages,
    verify_review_blocking_counts,
};
use crate::evidence::m2::verify_void_limits;
use crate::test_support::temporary_root;

#[test]
fn missing_blocking_counts_in_every_heading_are_reported() {
    let root = temporary_root("review-missing-counts");
    fs::create_dir_all(root.join("evidence/m6")).expect("milestone directory should exist");
    fs::write(root.join("evidence/m6/manifest.json"), "{}").expect("manifest should write");
    fs::write(
        root.join("evidence/m6/review.md"),
        "Summary: zero blocking findings.\n### Round 1\nFAIL\n### Round 2\nPASS\n",
    )
    .expect("review should write");
    let mut violations = Vec::new();
    verify_review_blocking_counts(&root, &mut violations);
    assert_eq!(
        violations
            .iter()
            .filter(|violation| violation.contains("declares no blocking count"))
            .count(),
        2,
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should remove");
}

#[test]
fn review_sweep_discovers_later_milestones() {
    let root = temporary_root("review-dynamic-milestones");
    fs::create_dir_all(root.join("evidence/m9")).expect("milestone directory should exist");
    fs::write(root.join("evidence/m9/manifest.json"), "{}").expect("manifest should write");
    let mut violations = Vec::new();
    let subjects = round_count_subjects(&root, &mut violations);
    assert!(violations.is_empty(), "{violations:?}");
    assert_eq!(subjects[0].0, "evidence/m9/review.md");
    fs::remove_dir_all(root).expect("temporary root should remove");
}

#[test]
fn a_round_count_is_caught_whichever_noun_it_chose_and_whichever_side_it_sat() {
    for restatement in [
        "Five independent reviews have run against this milestone.",
        "The independent rounds recorded here number five in total.",
        "There have been five review iterations.",
        "Five independent rounds have run.",
        "There are 5 verdicts recorded below.",
    ] {
        assert_eq!(
            round_count_restatements(restatement)
                .into_iter()
                .map(|(count, _)| count)
                .collect::<Vec<_>>(),
            [5],
            "{restatement}"
        );
    }

    for innocent in [
        "the profiling overhead rounds to `0.05` and not one digit better",
        "Round 5 returned FAIL with four blocking findings.",
        "every round so far has been an independent one",
    ] {
        assert_eq!(round_count_restatements(innocent), Vec::new(), "{innocent}");
    }
}

#[test]
fn no_tracked_manifest_writes_a_percentage_into_its_prose() {
    let root = std::env::current_dir()
        .expect("the current directory must be available")
        .join("../..");
    let mut violations = Vec::new();

    verify_manifest_percentages(&root, &mut violations);

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn the_void_budget_limits_are_the_ones_the_roadmap_declares() {
    let root = std::env::current_dir()
        .expect("the current directory must be available")
        .join("../..");
    let mut violations = Vec::new();

    verify_void_limits(
        &root,
        &crate::json::read_json(&root.join("evidence/m2/loader-overhead.json"))
            .expect("budget should parse"),
        &mut violations,
    );

    assert_eq!(violations, Vec::<String>::new());
}
