//! The regression budget comparison.
//!
//! `evidence/m2/loader-overhead.json`'s `budget.enforcement` recorded that
//! "nothing checks these limits automatically yet" and that "until it does the
//! rule above is a sentence rather than a mechanism". These tests are what make
//! it a mechanism: every one of them drives the real recorded budget rather than
//! a synthetic one, because a comparison proved against a budget written in the
//! same file proves the file.
//!
//! Candidate runs are the real record with one field changed. That is deliberate
//! — a candidate built from scratch would diverge from the shape a re-run
//! actually writes, and the comparison would then be proved against a shape
//! nothing produces.

use std::fs;
use std::path::{Path, PathBuf};

use ravel_xtask::{BudgetReport, verify_budget};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two directories below the repository root")
        .to_path_buf()
}

fn recorded_budget() -> String {
    fs::read_to_string(repository_root().join("evidence/m2/loader-overhead.json"))
        .expect("the M2 loader-overhead record should be readable")
}

/// Writes a candidate run built by replacing `from` with `to` in the record.
///
/// Textual replacement rather than a JSON edit, because this crate has no
/// dependencies and a hand-rolled JSON writer here would be a second description
/// of the record. Every call asserts the substring was present, so a record
/// change that invalidates a test fails it rather than silently testing nothing.
fn candidate(label: &str, edits: &[(&str, &str)]) -> PathBuf {
    let mut document = recorded_budget();
    for (from, to) in edits {
        assert!(
            document.contains(from),
            "the record no longer contains `{from}`, so this test is not testing what it says"
        );
        document = document.replace(from, to);
    }
    let path =
        std::env::temp_dir().join(format!("ravel-budget-{label}-{}.json", std::process::id()));
    fs::write(&path, document).expect("the candidate run should be writable");
    path
}

/// The three metrics the record's expiry clause voided.
const VOID: [&str; 3] = [
    "build.packCompile",
    "build.relink",
    "build.binaryGrowthPerMod",
];

fn notes_mention(report: &BudgetReport, metric: &str) -> bool {
    report.notes.iter().any(|note| note.contains(metric))
}

#[test]
fn the_recorded_run_passes_its_own_budget() {
    // Every limit is its measurement times two, so the record that set the
    // budget is the one run guaranteed to clear it. A comparison that failed
    // here would be failing arithmetic rather than finding a regression.
    let report = verify_budget(
        &repository_root(),
        &repository_root().join("evidence/m2/loader-overhead.json"),
    );

    assert!(
        report.violations.is_empty(),
        "the record that set the budget does not pass it: {:?}",
        report.violations
    );
}

#[test]
fn every_void_limit_is_named_rather_than_skipped() {
    let report = verify_budget(
        &repository_root(),
        &repository_root().join("evidence/m2/loader-overhead.json"),
    );

    // This is the whole point of the mechanism. A limit that is quietly not
    // compared reads exactly like one that passed, and that is how three
    // breaches sat in this repository unremarked.
    for metric in VOID {
        assert!(
            notes_mention(&report, metric),
            "`{metric}` is void and the report does not say so: {:?}",
            report.notes
        );
    }
    assert_eq!(report.notes.len(), VOID.len());

    // And each note says why, not merely that.
    for note in &report.notes {
        assert!(note.contains("expiry clause"), "{note}");
        assert!(note.contains("ADG-2"), "{note}");
    }
}

#[test]
fn a_live_limit_that_is_exceeded_is_a_regression_that_names_both_numbers() {
    // The mod-call limit is 12 ns against a measurement of 5.51.
    let run = candidate("breach", &[("\"measured\": 5.51", "\"measured\": 99.0")]);
    let report = verify_budget(&repository_root(), &run);

    assert_eq!(report.violations.len(), 1, "{:?}", report.violations);
    let violation = &report.violations[0];
    assert!(violation.contains("loader.modCall.p99"), "{violation}");
    assert!(violation.contains("99"), "{violation}");
    assert!(violation.contains("12"), "{violation}");
    assert!(violation.contains("regression"), "{violation}");

    // The void limits are still reported, because a failing run is exactly when
    // a reader most needs to know what was not checked.
    assert_eq!(report.notes.len(), VOID.len());
}

#[test]
fn a_measurement_exactly_at_its_limit_is_not_a_regression() {
    // The rule is "a measurement above its limit", and the operator the record
    // states is `less-than-or-equal`. Equality is the boundary the wording puts
    // on the passing side, so it is asserted rather than left to the reader.
    let run = candidate("boundary", &[("\"measured\": 5.51", "\"measured\": 12.0")]);
    let report = verify_budget(&repository_root(), &run);

    assert!(report.violations.is_empty(), "{:?}", report.violations);
}

#[test]
fn a_run_from_another_machine_is_refused_rather_than_compared() {
    // The record's restatement says the limits "hold on linux-reference-v1
    // only" and that "a budget without the machine it was measured on is not a
    // budget". Comparing anyway would produce a pass that means nothing.
    let run = candidate(
        "elsewhere",
        &[(
            "\"hardwareProfile\": \"linux-reference-v1\",\n    \"hardwareProfileSha256\"",
            "\"hardwareProfile\": \"some-laptop\",\n    \"hardwareProfileSha256\"",
        )],
    );
    let report = verify_budget(&repository_root(), &run);

    assert_eq!(report.violations.len(), 1, "{:?}", report.violations);
    assert!(report.violations[0].contains("some-laptop"));
    assert!(report.violations[0].contains("linux-reference-v1"));
    // Nothing was compared, so nothing is reported as not compared either.
    assert!(report.notes.is_empty());
}

#[test]
fn a_run_that_stopped_measuring_a_live_metric_fails_rather_than_passes() {
    // A limit whose metric is absent cannot be said to hold. Treating a missing
    // measurement as a pass is the same silence the void limits are named to
    // avoid, and it is the more dangerous of the two because it looks green.
    let run = candidate(
        "dropped",
        &[(
            "\"metric\": \"loader.modCall.p99\"",
            "\"metric\": \"loader.modCall.renamed\"",
        )],
    );
    let report = verify_budget(&repository_root(), &run);

    let missing = report
        .violations
        .iter()
        .find(|violation| violation.contains("does not measure it"))
        .unwrap_or_else(|| {
            panic!(
                "expected a missing-metric violation: {:?}",
                report.violations
            )
        });
    assert!(missing.contains("loader.modCall.p99"), "{missing}");
}

#[test]
fn a_metric_the_budget_does_not_limit_is_reported_as_unbudgeted() {
    // This is the shape the two undecided limits arrive in — profiling overhead
    // and crossings per tick are numbers nobody has chosen yet. A run that
    // measures one must not read as having passed a limit that does not exist.
    let run = candidate(
        "unbudgeted",
        &[(
            "\"metric\": \"loader.modCall.p99\"",
            "\"metric\": \"loader.crossingsPerTick\"",
        )],
    );
    let report = verify_budget(&repository_root(), &run);

    assert!(
        notes_mention(&report, "loader.crossingsPerTick"),
        "{:?}",
        report.notes
    );
    let note = report
        .notes
        .iter()
        .find(|note| note.contains("loader.crossingsPerTick"))
        .expect("the note was just asserted to exist");
    assert!(note.contains("no limit"), "{note}");
    assert!(note.contains("pass or fail"), "{note}");
}

#[test]
fn a_run_declaring_another_budget_protocol_is_refused_before_anything_is_compared() {
    // Two records can carry the same metric names and mean different things by
    // them. Comparing across protocols would be the same silent-pass failure
    // this mechanism exists to end, one level up.
    let run = candidate(
        "protocol",
        &[(
            "\"protocol\": \"m2-loader-budget-v1\"",
            "\"protocol\": \"m4-loader-budget-v2\"",
        )],
    );
    let report = verify_budget(&repository_root(), &run);

    assert_eq!(report.violations.len(), 1, "{:?}", report.violations);
    assert!(report.violations[0].contains("m4-loader-budget-v2"));
    assert!(report.violations[0].contains("m2-loader-budget-v1"));
    assert!(report.notes.is_empty());
}

/// Writes a temporary repository root whose budget record has `edits` applied.
///
/// The operator and the limit come from the *budget*, never from the candidate,
/// so a test about either has to change the baseline rather than the run. That
/// asymmetry is the point: a run that could restate the rule it is judged by
/// would not be judged at all.
fn root_with_budget(label: &str, edits: &[(&str, &str)]) -> PathBuf {
    let mut document = recorded_budget();
    for (from, to) in edits {
        assert!(
            document.contains(from),
            "the record no longer contains `{from}`, so this test is not testing what it says"
        );
        document = document.replace(from, to);
    }
    let root =
        std::env::temp_dir().join(format!("ravel-budget-root-{label}-{}", std::process::id()));
    fs::create_dir_all(root.join("evidence/m2")).expect("the temporary root should be creatable");
    fs::write(root.join("evidence/m2/loader-overhead.json"), document)
        .expect("the budget record should be writable");
    root
}

#[test]
fn an_unimplemented_comparison_is_refused_rather_than_assumed_to_mean_at_most() {
    // Guessing that an unknown operator meant "at most" is how a budget starts
    // passing the thing it was written to stop.
    let root = root_with_budget(
        "operator",
        &[(
            "\"operator\": \"less-than-or-equal\"",
            "\"operator\": \"within-one-standard-deviation\"",
        )],
    );
    let report = verify_budget(
        &root,
        &repository_root().join("evidence/m2/loader-overhead.json"),
    );

    let refused = report
        .violations
        .iter()
        .find(|violation| violation.contains("does not implement"))
        .unwrap_or_else(|| panic!("expected an operator refusal: {:?}", report.violations));
    assert!(
        refused.contains("within-one-standard-deviation"),
        "{refused}"
    );
}

#[test]
fn a_candidate_cannot_restate_the_rule_it_is_judged_by() {
    // The candidate declares an operator this check does not implement, and the
    // budget still governs. A run that could soften its own comparison would be
    // grading itself.
    let run = candidate(
        "restated",
        &[(
            "\"operator\": \"less-than-or-equal\"",
            "\"operator\": \"within-one-standard-deviation\"",
        )],
    );
    let report = verify_budget(&repository_root(), &run);

    assert!(report.violations.is_empty(), "{:?}", report.violations);
    assert_eq!(report.notes.len(), VOID.len());
}

#[test]
fn a_run_that_is_not_a_readable_record_fails_rather_than_reporting_nothing() {
    let missing =
        std::env::temp_dir().join(format!("ravel-budget-absent-{}.json", std::process::id()));
    let _ = fs::remove_file(&missing);

    let report = verify_budget(&repository_root(), &missing);
    assert!(!report.violations.is_empty());
    assert!(report.notes.is_empty());
}
