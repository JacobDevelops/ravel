use std::fs;

use super::{
    round_count_restatements, round_count_subjects, verify_accounting,
    verify_review_blocking_counts_for,
};
use crate::evidence::m2::verify_void_limits;
use crate::evidence::rules::milestone_manifests;
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
    let manifests = milestone_manifests(&root).expect("manifests should discover");
    verify_review_blocking_counts_for(&root, &manifests, &mut violations);
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
    let manifests = milestone_manifests(&root).expect("manifests should discover");
    let subjects = round_count_subjects(&manifests);
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
        "it reviews 12 crates before checking the result",
    ] {
        assert_eq!(round_count_restatements(innocent), Vec::new(), "{innocent}");
    }
}

#[test]
fn tracked_evidence_satisfies_all_shared_accounting_checks() {
    let root = crate::test_support::repository_root();
    let mut violations = Vec::new();

    verify_accounting(&root, None, &mut violations);

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn the_void_budget_limits_are_the_ones_the_roadmap_declares() {
    let root = crate::test_support::repository_root();
    let mut violations = Vec::new();

    verify_void_limits(
        &root,
        &crate::json::read_json(&root.join("evidence/m2/loader-overhead.json"))
            .expect("budget should parse"),
        &mut violations,
    );

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn blocking_summary_reads_the_immediate_tally_not_a_later_round_count() {
    let root = temporary_root("blocking-summary-immediate-tally");
    fs::create_dir_all(root.join("evidence/m6")).expect("directory should exist");
    fs::write(root.join("evidence/m6/manifest.json"), "{}").expect("manifest should write");
    fs::write(
        root.join("evidence/m6/review.md"),
        "Summary: Blocking findings: 3 across two rounds.\n\
         ### Round 1 — 2 blocking findings\n### Round 2 — 1 blocking finding\n",
    )
    .expect("review should write");
    let mut violations = Vec::new();
    let manifests = milestone_manifests(&root).expect("manifests should discover");
    verify_review_blocking_counts_for(&root, &manifests, &mut violations);
    assert!(violations.is_empty(), "{violations:?}");
    fs::remove_dir_all(root).expect("temporary root should remove");
}

#[test]
fn discovery_failure_is_reported_once() {
    let root = temporary_root("accounting-empty-manifests");
    fs::create_dir_all(root.join("evidence")).expect("evidence directory should exist");
    fs::write(root.join("ROADMAP.md"), "").expect("roadmap should write");
    let mut violations = Vec::new();
    verify_accounting(&root, None, &mut violations);
    assert_eq!(
        violations,
        ["evidence directory contains no milestone manifests"]
    );
    fs::remove_dir_all(root).expect("temporary root should remove");
}

#[test]
fn discovery_failure_does_not_skip_independent_roadmap_accounting() {
    let root = temporary_root("accounting-empty-manifests-roadmap-drift");
    fs::create_dir_all(root.join("evidence")).expect("evidence directory should exist");
    fs::write(
        root.join("ROADMAP.md"),
        "Five independent reviews have run.",
    )
    .expect("roadmap should write");
    let mut violations = Vec::new();
    verify_accounting(&root, None, &mut violations);
    assert_eq!(violations.len(), 2, "{violations:?}");
    assert_eq!(
        violations[0],
        "evidence directory contains no milestone manifests"
    );
    assert!(violations[1].starts_with("ROADMAP.md restates that 5 review reviews exist;"));
    fs::remove_dir_all(root).expect("temporary root should remove");
}

#[test]
fn selected_milestone_filters_typed_manifest_subjects() {
    let root = temporary_root("accounting-selected-milestone");
    for milestone in ["m6", "m7"] {
        let directory = root.join("evidence").join(milestone);
        fs::create_dir_all(&directory).expect("milestone directory should exist");
        fs::write(directory.join("manifest.json"), "{}").expect("manifest should write");
        fs::write(directory.join("README.md"), "Five reviews have run.")
            .expect("readme should write");
        fs::write(directory.join("review.md"), "").expect("review should write");
    }
    fs::write(root.join("ROADMAP.md"), "").expect("roadmap should write");
    let mut violations = Vec::new();
    verify_accounting(&root, Some(super::Milestone::M6), &mut violations);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(violations[0].starts_with("evidence/m6/README.md"));
    fs::remove_dir_all(root).expect("temporary root should remove");
}
