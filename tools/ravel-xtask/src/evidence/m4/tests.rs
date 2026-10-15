use std::fs;
use std::path::PathBuf;

use crate::evidence::m4::{verify_m4_gate_figures, verify_m4_process_matrix, verify_m4_surface};
use crate::json::read_json;
use crate::test_support::temporary_root;
use crate::{CRATE_NAMES, EXAMPLE_NAMES, TEST_MOD_NAMES};

fn surface_root(label: &str, guide: &[&str], suites: &[&str], record: &str) -> PathBuf {
    let root = temporary_root(label);
    for (directory, files) in [
        ("docs/guide", guide),
        ("tests/conformance/tests/integration", suites),
    ] {
        let directory = root.join(directory);
        fs::create_dir_all(&directory).expect("directory should be creatable");
        for file in files {
            fs::write(directory.join(file), "").expect("file should be writable");
        }
    }
    let milestone = root.join("evidence/m4");
    fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
    fs::write(milestone.join("sdk-surface.json"), record).expect("record should be writable");
    root
}

fn surface_record(guide: &[&str], suites: &[&str]) -> String {
    let quoted = |names: &[&str]| {
        names
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(",")
    };
    format!(
        r#"{{"inventory":{{"crates":[{}],"testMods":[{}],"examples":[{}],
            "guideDocuments":[{}],"conformanceSuites":[{}]}}}}"#,
        quoted(CRATE_NAMES),
        quoted(TEST_MOD_NAMES),
        quoted(EXAMPLE_NAMES),
        quoted(guide),
        quoted(suites),
    )
}

#[test]
fn a_surface_inventory_naming_what_the_repository_holds_is_accepted() {
    let root = surface_root(
        "surface-agrees",
        &["README.md"],
        &["one.rs"],
        &surface_record(&["README.md"], &["one.rs"]),
    );

    let mut violations = Vec::new();
    verify_m4_surface(&root, &mut violations);

    assert!(violations.is_empty(), "{violations:?}");
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_surface_inventory_naming_what_the_repository_lost_is_reported() {
    let root = surface_root(
        "surface-drifted",
        &["README.md"],
        &["one.rs"],
        &surface_record(&["README.md", "removed.md"], &["one.rs", "deleted.rs"]),
    );

    let mut violations = Vec::new();
    verify_m4_surface(&root, &mut violations);

    assert_eq!(
        violations,
        [
            "evidence/m4/sdk-surface.json names removed.md under inventory.guideDocuments, \
             which the repository does not hold",
            "evidence/m4/sdk-surface.json names deleted.rs under inventory.conformanceSuites, \
             which the repository does not hold",
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_surface_inventory_omitting_what_a_later_milestone_added_is_accepted() {
    let root = surface_root(
        "surface-later-additions",
        &["README.md", "added-later.md"],
        &["one.rs", "added_later.rs"],
        &surface_record(&["README.md"], &["one.rs"]),
    );

    let mut violations = Vec::new();
    verify_m4_surface(&root, &mut violations);

    assert!(violations.is_empty(), "{violations:?}");
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn gate_figures_root(label: &str, manifest: &str, matrix: &str) -> PathBuf {
    let root = temporary_root(label);
    let milestone = root.join("evidence/m4");
    fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
    fs::write(milestone.join("manifest.json"), manifest).expect("manifest should be writable");
    fs::write(milestone.join("linux-process-matrix.json"), matrix)
        .expect("record should be writable");
    root
}

#[test]
fn a_manifest_and_a_matrix_reporting_the_same_run_are_accepted() {
    let root = gate_figures_root(
        "gate-figures-agree",
        r#"{"tests":{"total":9,"passed":9,"failed":0,"skipped":0,
            "lineCoveragePercent":85.97}}"#,
        r#"{"gate":{"tests":{"run":9,"passed":9,"failed":0,"skipped":0},
            "coverage":{"linesPercent":85.97}}}"#,
    );

    let mut violations = Vec::new();
    verify_m4_gate_figures(
        &root,
        &read_json(&root.join("evidence/m4/manifest.json")).expect("manifest should parse"),
        &mut violations,
    );

    assert!(violations.is_empty(), "{violations:?}");
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_manifest_and_a_matrix_reporting_different_runs_are_reported() {
    let root = gate_figures_root(
        "gate-figures-differ",
        r#"{"tests":{"total":9,"passed":9,"failed":0,"skipped":0,
            "lineCoveragePercent":85.97}}"#,
        r#"{"gate":{"tests":{"run":8,"passed":9,"failed":0,"skipped":0},
            "coverage":{"linesPercent":85.97}}}"#,
    );

    let mut violations = Vec::new();
    verify_m4_gate_figures(
        &root,
        &read_json(&root.join("evidence/m4/manifest.json")).expect("manifest should parse"),
        &mut violations,
    );

    assert_eq!(
        violations,
        ["evidence/m4/manifest.json records /tests/total 9 while \
             evidence/m4/linux-process-matrix.json derives 8; one gate run produced both"]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn process_matrix_root(label: &str, matrix: &str) -> PathBuf {
    let root = temporary_root(label);
    let milestone = root.join("evidence/m4");
    fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
    fs::write(milestone.join("linux-process-matrix.json"), matrix)
        .expect("record should be writable");
    root
}

#[test]
fn a_historical_process_matrix_is_independent_of_the_current_makefile() {
    let root = process_matrix_root(
        "matrix-historical",
        r#"{"result":"pass","processes":[
            {"command":"devenv tasks run ravel:fmt","exitStatus":0},
            {"command":"devenv tasks run ravel:verify-budget","exitStatus":0}],
            "checksOutsideTheGate":[
                {"command":"devenv tasks run ravel:historical-check","exitStatus":0}]}"#,
    );

    for source in [
        "[tasks.\"ravel:verify\"]\ndependencies = ['ravel:fmt', 'ravel:verify-budget']",
        "[tasks.\"ravel:verify\"]\ndependencies = ['ravel:fmt', 'ravel:registry-verify']",
        "not valid TOML",
    ] {
        fs::write(root.join("Makefile.toml"), source).expect("Makefile should be writable");
        let mut violations = Vec::new();
        verify_m4_process_matrix(&root, &mut violations);

        assert!(violations.is_empty(), "{source}: {violations:?}");
    }
    fs::remove_file(root.join("Makefile.toml")).expect("Makefile should be removable");
    let mut violations = Vec::new();
    verify_m4_process_matrix(&root, &mut violations);

    assert!(violations.is_empty(), "{violations:?}");
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_failed_process_under_a_record_that_calls_itself_a_pass_is_reported() {
    let root = process_matrix_root(
        "matrix-red-row",
        r#"{"result":"pass","processes":[
            {"command":"devenv tasks run ravel:fmt","exitStatus":1}],
            "checksOutsideTheGate":[
                {"command":"devenv tasks run ravel:historical-check","exitStatus":1}]}"#,
    );

    let mut violations = Vec::new();
    verify_m4_process_matrix(&root, &mut violations);

    assert_eq!(
        violations,
        [
            "evidence/m4/linux-process-matrix.json records a nonzero exit status for `devenv \
             tasks run ravel:fmt` under .processes while its own result is pass; a record that \
             reports a failed process and calls itself green states two things that cannot \
             both be true",
            "evidence/m4/linux-process-matrix.json records a nonzero exit status for `devenv \
             tasks run ravel:historical-check` under .checksOutsideTheGate while its own result \
             is pass; a record that reports a failed process and calls itself green states \
             two things that cannot both be true"
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_task_recorded_both_inside_and_outside_the_gate_is_reported() {
    let root = process_matrix_root(
        "matrix-parked-row",
        r#"{"result":"pass",
            "processes":[{"command":"devenv tasks run ravel:fmt","exitStatus":0}],
            "checksOutsideTheGate":[
                {"command":"devenv tasks run ravel:fmt","exitStatus":0}]}"#,
    );

    let mut violations = Vec::new();
    verify_m4_process_matrix(&root, &mut violations);

    assert_eq!(
        violations,
        [
            "evidence/m4/linux-process-matrix.json records ravel:fmt under both processes and \
             checksOutsideTheGate; a recorded gate process cannot also be outside that gate"
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_recorded_command_that_names_no_task_is_reported() {
    let root = process_matrix_root(
        "matrix-bare-command",
        r#"{"result":"pass","processes":[
            {"command":"devenv tasks run ravel:fmt","exitStatus":0},
            {"command":"cargo test","exitStatus":0}],
            "checksOutsideTheGate":[{"command":"cargo check","exitStatus":0}]}"#,
    );

    let mut violations = Vec::new();
    verify_m4_process_matrix(&root, &mut violations);

    assert_eq!(
        violations,
        [
            "evidence/m4/linux-process-matrix.json records the command `cargo test` under \
             .processes, which does not name a devenv task; a row nothing can resolve to a \
             task is reconciled against nothing",
            "evidence/m4/linux-process-matrix.json records the command `cargo check` under \
             .checksOutsideTheGate, which does not name a devenv task; a row nothing can resolve \
             to a task is reconciled against nothing"
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn missing_or_null_gate_figures_cannot_reconcile_as_a_match() {
    for (label, figures) in [
        ("gate-figures-missing", "{}"),
        (
            "gate-figures-null",
            r#"{"total":null,"passed":null,"failed":null,"skipped":null,"lineCoveragePercent":null}"#,
        ),
    ] {
        let root = gate_figures_root(label, &format!(r#"{{"tests":{figures}}}"#), "{}");
        let manifest =
            read_json(&root.join("evidence/m4/manifest.json")).expect("manifest should parse");
        let mut violations = Vec::new();

        verify_m4_gate_figures(&root, &manifest, &mut violations);

        assert_eq!(violations.len(), 5, "{violations:?}");
        for field in [
            "total",
            "passed",
            "failed",
            "skipped",
            "lineCoveragePercent",
        ] {
            assert!(
                violations.iter().any(|violation| violation
                    == &format!(
                        "evidence/m4/manifest.json: cannot compare gate run /tests/{field} with \
                     evidence/m4/linux-process-matrix.json: a required figure is missing or null"
                    )),
                "{violations:?}"
            );
        }
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}
