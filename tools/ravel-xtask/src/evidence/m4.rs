//! M4 closure rules.

use crate::evidence::closure::{LoadedRecords, RecordDefinition, read_records};
use crate::evidence::rules::Milestone;
use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::evidence::closure::{
    CarriedFigures, verify_closure_records, verify_manifest_closure, verify_milestone_figures,
};
use crate::evidence::model::admit;
use crate::evidence::rules::{
    Rule, all_gates_closed, has_exact_benchmark_results, has_exact_work_packages,
    has_native_attestation, require,
};
use crate::json::{array, number};

mod model;

const M4_MANIFEST_RULES: &[(Rule, &str)] = &[
    (
        |value| {
            has_exact_work_packages(
                value,
                &[
                    "M4-W1", "M4-W2", "M4-W3", "M4-W4", "M4-W5", "M4-W6", "M4-W7", "M4-W8",
                ],
            )
        },
        "M4 manifest must include every work package from M4-W1 through M4-W8 exactly once",
    ),
    (
        |value| {
            has_exact_benchmark_results(
                value,
                "m4-surface-v1",
                &["linux-process-matrix.json", "sdk-surface.json"],
            )
        },
        "M4 manifest must record both M4 results under the m4-surface-v1 protocol, so that each \
         one is covered by a recomputed digest",
    ),
    (
        |value| all_gates_closed(value, "M4-W8", 7),
        "M4 closure requires a gateStatus row for every work package, each implementation row \
         closed and naming its evidence",
    ),
    (
        |value| has_native_attestation(value, "ravel"),
        "M4 closure requires a native Linux attestation for the recorded gate run",
    ),
    (
        |value| !array(value, "/deviations").is_empty() && !array(value, "/risks").is_empty(),
        "M4 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
    ),
];

pub(crate) fn verify_m4_manifest(
    manifest: &Value,
    records: &LoadedRecords,
    violations: &mut Vec<String>,
) {
    verify_manifest_closure(manifest, M4_MANIFEST_RULES, violations);

    verify_closure_records(Milestone::M4, records, M4_RECORDS, violations);
}

const M4_RECORDS: &[RecordDefinition] = &[
    RecordDefinition {
        name: "linux-process-matrix.json",
        rule: |value| {
            let record: model::ProcessMatrix<'_> = admit(value)?;
            crate::evidence::model::require_identity(
                record.work_package.0,
                "M4-W8",
                "/workPackage",
            )?;
            crate::evidence::model::require_identity(
                record.protocol.0,
                "m4-gate-matrix-v1",
                "/protocol",
            )?;
            require(
                record.result == crate::evidence::model::Verdict::Pass,
                "/result must be pass",
            )?;
            crate::evidence::model::require_identity(
                record.platform.os.0,
                "linux",
                "/platform/os",
            )?;
            crate::evidence::model::require_identity(
                record.platform.architecture.0,
                "x86_64",
                "/platform/architecture",
            )?;
            require(
                !record.game_launch.occurred,
                "/gameLaunch/occurred must be false",
            )?;
            require(
                record.gate.exit_status.as_f64() == Some(0.0),
                "/gate/exitStatus",
            )?;
            require(
                record
                    .gate
                    .tests
                    .run
                    .as_f64()
                    .is_some_and(|count| count > 0.0),
                "/gate/tests/run",
            )?;
            for (index, process) in record.processes.0.iter().enumerate() {
                require(
                    process.exit_status.as_f64() == Some(0.0),
                    "/exitStatus must be zero",
                )
                .map_err(|error| format!("/processes/{index}{error}"))?;
            }
            Ok(())
        },
        description: "a Linux process matrix in which every recorded gate process exited zero and says what it \
     does not prove, and which states in a field rather than in prose that no game process \
     was launched for it",
        required: false,
    },
    RecordDefinition {
        name: "sdk-surface.json",
        rule: |value| {
            let record: model::Surface<'_> = admit(value)?;
            let mut packages = record
                .work_packages
                .0
                .iter()
                .map(|row| row.id.0)
                .collect::<Vec<_>>();
            packages.sort_unstable();

            crate::evidence::model::require_identity(
                record.work_package.0,
                "M4-W8",
                "/workPackage",
            )?;
            crate::evidence::model::require_identity(
                record.protocol.0,
                "m4-surface-v1",
                "/protocol",
            )?;
            require(record.work_packages.0.len() == 7, "/workPackages")?;
            require(
                packages
                    == [
                        "M4-W1", "M4-W2", "M4-W3", "M4-W4", "M4-W5", "M4-W6", "M4-W7",
                    ],
                "/workPackages must name M4-W1 through M4-W7 exactly",
            )?;
            Ok(())
        },
        description: "an SDK surface record inventorying what each of M4-W1 through M4-W7 shipped, what proves \
     it, and what it does not prove",
        required: false,
    },
];

pub(crate) fn verify_m4_surface(root: &Path, value: &Value, violations: &mut Vec<String>) {
    let paths = match recorded_surface_paths(root) {
        Ok(paths) => paths,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    for (field, directory, suffix) in [
        ("crates", "crates", "/Cargo.toml"),
        ("testMods", "test-mods", "/Cargo.toml"),
        ("examples", "examples", "/Cargo.toml"),
        ("guideDocuments", "docs/guide", ".md"),
        (
            "conformanceSuites",
            "tests/conformance/tests/integration",
            ".rs",
        ),
    ] {
        for entry in array(value, &format!("/inventory/{field}")) {
            let Some(name) = entry.as_str() else {
                violations.push(format!(
                    "evidence/m4/sdk-surface.json names non-string {entry} under inventory.{field}, \
                     which cannot name a repository file"
                ));
                continue;
            };
            let name = name.trim();
            let file = if suffix.starts_with('/') { suffix } else { "" };
            let held = paths.contains(&format!("{directory}/{name}{file}"))
                || (field == "conformanceSuites"
                    && paths.contains(&format!("tests/conformance/tests/{name}")));
            if name.is_empty()
                || name.contains('/')
                || (file.is_empty() && !name.ends_with(suffix))
                || !held
            {
                violations.push(format!(
                    "evidence/m4/sdk-surface.json names {name} under inventory.{field}, which the \
                     inventory revision does not hold"
                ));
            }
        }
    }
}

fn recorded_surface_paths(root: &Path) -> Result<BTreeSet<String>, String> {
    let revision = crate::vcs::last_modifying_revision(root, "evidence/m4/sdk-surface.json")?;
    Ok(crate::vcs::tracked_files(root, Some(&revision))?
        .into_iter()
        .collect())
}

const M4_PROCESS_MATRIX: &str = "evidence/m4/linux-process-matrix.json";

const TASK_COMMAND_PREFIX: &str = "devenv tasks run ";

pub(crate) fn verify_m4_process_matrix(record: &Value, violations: &mut Vec<String>) {
    let recorded = recorded_tasks(record, "processes", violations);
    let outside = recorded_tasks(record, "checksOutsideTheGate", violations);
    for inside in outside.intersection(&recorded) {
        violations.push(format!(
            "{M4_PROCESS_MATRIX} records {inside} under both processes and \
             checksOutsideTheGate; a recorded gate process cannot also be outside that gate"
        ));
    }
    verify_recorded_exit_statuses(record, violations);
}

fn recorded_tasks<'a>(
    record: &'a Value,
    field: &str,
    violations: &mut Vec<String>,
) -> BTreeSet<&'a str> {
    let mut tasks = BTreeSet::new();
    for row in array(record, &format!("/{field}")) {
        let Some(command) = row["command"]
            .as_str()
            .map(str::trim)
            .filter(|text| !text.is_empty())
        else {
            violations.push(format!(
                "{M4_PROCESS_MATRIX} records a row under .{field} with no command"
            ));
            continue;
        };
        match command.strip_prefix(TASK_COMMAND_PREFIX) {
            Some(task) => {
                tasks.insert(task.trim());
            }
            None => violations.push(format!(
                "{M4_PROCESS_MATRIX} records the command `{command}` under .{field}, which does not \
                 name a devenv task; a row nothing can resolve to a task is reconciled against \
                 nothing"
            )),
        }
    }
    tasks
}

fn verify_recorded_exit_statuses(record: &Value, violations: &mut Vec<String>) {
    if !admit::<super::model::Verdict>(&record["result"])
        .is_ok_and(|verdict| verdict == super::model::Verdict::Pass)
    {
        return;
    }
    for field in ["processes", "checksOutsideTheGate"] {
        for row in array(record, &format!("/{field}")) {
            if number(row, "/exitStatus") == Some(0.0) {
                continue;
            }
            let command = row["command"]
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .unwrap_or("<no command>");
            violations.push(format!(
                "{M4_PROCESS_MATRIX} records a nonzero exit status for `{command}` under .{field} \
                 while its own result is pass; a record that reports a failed process and calls \
                 itself green states two things that cannot both be true"
            ));
        }
    }
}

pub(crate) fn verify_m4_integrity(
    root: &Path,
    manifest: &Value,
    violations: &mut Vec<String>,
) -> LoadedRecords {
    let records = read_records(root, Milestone::M4, M4_RECORDS, violations);
    if let Some(surface) = records.get("sdk-surface.json") {
        verify_m4_surface(root, surface, violations);
    }
    if let Some(record) = records.carried(&M4_FIGURES) {
        verify_m4_process_matrix(record, violations);
        verify_milestone_figures(Milestone::M4, record, manifest, &M4_FIGURES, violations);
    }
    records
}

const M4_FIGURES: CarriedFigures = CarriedFigures {
    record: "linux-process-matrix.json",
    subject: "gate run",
    verify: |milestone, record, manifest, carried, violations| {
        let arithmetic: &[(Rule, &str)] = &[];
        crate::evidence::closure::verify_typed_figures(
            milestone,
            record,
            manifest,
            carried,
            M4_CARRIED_FIGURES,
            arithmetic,
            violations,
        );
    },
};

const M4_CARRIED_FIGURES: &[(&str, crate::evidence::rules::Figure)] = &[
    ("/tests/total", |value| {
        value.pointer("/gate/tests/run").cloned()
    }),
    ("/tests/passed", |value| {
        value.pointer("/gate/tests/passed").cloned()
    }),
    ("/tests/failed", |value| {
        value.pointer("/gate/tests/failed").cloned()
    }),
    ("/tests/skipped", |value| {
        value.pointer("/gate/tests/skipped").cloned()
    }),
    ("/tests/lineCoveragePercent", |value| {
        value.pointer("/gate/coverage/linesPercent").cloned()
    }),
];

#[cfg(test)]
mod tests;
