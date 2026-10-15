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

fn loader_run() -> PathBuf {
    repository_root().join("evidence/m2/loader-overhead.json")
}

fn profiling_run() -> PathBuf {
    repository_root().join("evidence/m3/profiling-overhead.json")
}

fn recorded_budget() -> String {
    fs::read_to_string(loader_run()).expect("the M2 loader-overhead record should be readable")
}

fn recorded_profiling_budget() -> String {
    fs::read_to_string(profiling_run()).expect("the M3 profiling record should be readable")
}

fn compare(root: &Path, loader: &Path) -> BudgetReport {
    let profiling = profiling_run();
    verify_budget(root, &[loader, &profiling])
}

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
    let report = compare(&repository_root(), &loader_run());

    assert!(
        report.violations.is_empty(),
        "the record that set the budget does not pass it: {:?}",
        report.violations
    );
}

#[test]
fn every_void_limit_is_named_rather_than_skipped() {
    let report = compare(&repository_root(), &loader_run());

    for metric in VOID {
        assert!(
            notes_mention(&report, metric),
            "`{metric}` is void and the report does not say so: {:?}",
            report.notes
        );
    }
    assert_eq!(report.notes.len(), VOID.len());

    for note in &report.notes {
        assert!(note.contains("expiry clause"), "{note}");
        assert!(note.contains("ADG-2"), "{note}");
    }
}

#[test]
fn a_live_limit_that_is_exceeded_is_a_regression_that_names_both_numbers() {
    let run = candidate("breach", &[("\"measured\": 5.51", "\"measured\": 99.0")]);
    let report = compare(&repository_root(), &run);

    assert_eq!(report.violations.len(), 1, "{:?}", report.violations);
    let violation = &report.violations[0];
    assert!(violation.contains("loader.modCall.p99"), "{violation}");
    assert!(violation.contains("99"), "{violation}");
    assert!(violation.contains("12"), "{violation}");
    assert!(violation.contains("regression"), "{violation}");

    assert_eq!(report.notes.len(), VOID.len());
}

#[test]
fn a_measurement_exactly_at_its_limit_is_not_a_regression() {
    let run = candidate("boundary", &[("\"measured\": 5.51", "\"measured\": 12.0")]);
    let report = compare(&repository_root(), &run);

    assert!(report.violations.is_empty(), "{:?}", report.violations);
}

#[test]
fn a_run_from_another_machine_is_refused_rather_than_compared() {
    let run = candidate(
        "elsewhere",
        &[(
            "\"hardwareProfile\": \"linux-reference-v1\",\n    \"hardwareProfileSha256\"",
            "\"hardwareProfile\": \"some-laptop\",\n    \"hardwareProfileSha256\"",
        )],
    );
    let report = compare(&repository_root(), &run);

    assert_eq!(report.violations.len(), 1, "{:?}", report.violations);
    assert!(report.violations[0].contains("some-laptop"));
    assert!(report.violations[0].contains("linux-reference-v1"));

    assert_eq!(report.notes, [] as [std::string::String; 0]);
}

#[test]
fn a_run_that_stopped_measuring_a_live_metric_fails_rather_than_passes() {
    let run = candidate(
        "dropped",
        &[(
            "\"metric\": \"loader.modCall.p99\"",
            "\"metric\": \"loader.modCall.renamed\"",
        )],
    );
    let report = compare(&repository_root(), &run);

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
    let run = candidate(
        "unbudgeted",
        &[(
            "\"metric\": \"loader.modCall.p99\"",
            "\"metric\": \"loader.crossingsPerTick\"",
        )],
    );
    let report = compare(&repository_root(), &run);

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
    let run = candidate(
        "protocol",
        &[(
            "\"protocol\": \"m2-loader-budget-v1\"",
            "\"protocol\": \"m4-loader-budget-v2\"",
        )],
    );
    let report = compare(&repository_root(), &run);

    assert_eq!(report.violations.len(), 1, "{:?}", report.violations);
    assert!(report.violations[0].contains("m4-loader-budget-v2"));
    assert!(report.violations[0].contains("m2-loader-budget-v1"));
    assert_eq!(report.notes, [] as [std::string::String; 0]);
}

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
    fs::create_dir_all(root.join("evidence/m3")).expect("the temporary root should be creatable");
    fs::write(root.join("evidence/m2/loader-overhead.json"), document)
        .expect("the budget record should be writable");

    fs::write(
        root.join("evidence/m3/profiling-overhead.json"),
        recorded_profiling_budget(),
    )
    .expect("the profiling budget record should be writable");
    root
}

#[test]
fn an_unimplemented_comparison_is_refused_rather_than_assumed_to_mean_at_most() {
    let root = root_with_budget(
        "operator",
        &[(
            "\"operator\": \"less-than-or-equal\"",
            "\"operator\": \"within-one-standard-deviation\"",
        )],
    );
    let report = compare(&root, &loader_run());

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
    let run = candidate(
        "restated",
        &[(
            "\"operator\": \"less-than-or-equal\"",
            "\"operator\": \"within-one-standard-deviation\"",
        )],
    );
    let report = compare(&repository_root(), &run);

    assert!(report.violations.is_empty(), "{:?}", report.violations);
    assert_eq!(report.notes.len(), VOID.len());
}

#[test]
fn a_run_that_is_not_a_readable_record_fails_rather_than_reporting_nothing() {
    let missing =
        std::env::temp_dir().join(format!("ravel-budget-absent-{}.json", std::process::id()));
    let _ = fs::remove_file(&missing);

    let report = compare(&repository_root(), &missing);
    assert_ne!(report.violations, [] as [std::string::String; 0]);
    assert_eq!(report.notes, [] as [std::string::String; 0]);
}

#[test]
fn the_normal_mode_profiling_limit_is_part_of_the_same_budget() {
    let report = compare(&repository_root(), &loader_run());

    assert!(report.violations.is_empty(), "{:?}", report.violations);
    assert!(
        !notes_mention(&report, "profiling.off.perTick"),
        "the profiling limit was compared, not noted as unbudgeted: {:?}",
        report.notes
    );
}

#[test]
fn a_profiling_run_over_one_per_cent_of_a_tick_is_a_regression() {
    let mut document = recorded_profiling_budget();
    let measured = document
        .lines()
        .find(|line| line.contains("\"measured\""))
        .expect("the profiling record states a measured value")
        .to_owned();
    document = document.replace(&measured, "      \"measured\": 500000.01,");
    let run = std::env::temp_dir().join(format!(
        "ravel-budget-profiling-breach-{}.json",
        std::process::id()
    ));
    fs::write(&run, document).expect("the candidate run should be writable");

    let report = verify_budget(&repository_root(), &[&loader_run(), &run]);

    let breach = report
        .violations
        .iter()
        .find(|violation| violation.contains("profiling.off.perTick"))
        .unwrap_or_else(|| panic!("expected a profiling regression: {:?}", report.violations));
    assert!(breach.contains("500000"), "{breach}");
    assert!(breach.contains("regression"), "{breach}");
}

#[test]
fn two_budget_records_may_not_set_two_ceilings_for_one_metric() {
    let root = root_with_budget(
        "collision",
        &[(
            "\"metric\": \"loader.modCall.p99\"",
            "\"metric\": \"profiling.off.perTick\"",
        )],
    );
    let report = compare(&root, &loader_run());

    let collision = report
        .violations
        .iter()
        .find(|violation| violation.contains("two ceilings"))
        .unwrap_or_else(|| panic!("expected a collision refusal: {:?}", report.violations));
    assert!(collision.contains("profiling.off.perTick"), "{collision}");
}

#[test]
fn a_comparison_against_no_run_at_all_is_refused_rather_than_passed() {
    let report = verify_budget(&repository_root(), &[]);

    assert_eq!(report.violations.len(), 1, "{:?}", report.violations);
    assert!(report.violations[0].contains("vacuously"));
}
