//! M4 closure rules.

use crate::evidence::closure::{LoadedRecords, RecordDefinition, read_records};
use crate::evidence::rules::Milestone;
use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::evidence::closure::{CarriedFigures, verify_closure_records, verify_manifest_closure};
use crate::evidence::model::admit;
use crate::evidence::rules::{
    Rule, all_gates_closed, has_exact_benchmark_results, has_exact_work_packages,
    has_native_attestation, require,
};
use crate::json::array;

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
        rule: |value| verify_process_record(&admit::<model::ProcessMatrix<'_>>(value)?),
        description: "a Linux process matrix in which every recorded gate process exited zero and says what it \
     does not prove, and which states in a field rather than in prose that no game process \
     was launched for it",
        required: false,
    },
    RecordDefinition {
        name: "sdk-surface.json",
        rule: |value| verify_surface_record(&admit::<model::Surface<'_>>(value)?),
        description: "an SDK surface record inventorying what each of M4-W1 through M4-W7 shipped, what proves \
     it, and what it does not prove",
        required: false,
    },
];

#[cfg(test)]
pub(crate) fn verify_m4_surface(root: &Path, value: &Value, violations: &mut Vec<String>) {
    #[derive(serde::Deserialize)]
    struct SurfaceInventory {
        inventory: model::Inventory,
    }
    let record: SurfaceInventory = match admit(value) {
        Ok(record) => record,
        Err(error) => {
            violations.push(format!("evidence/m4/sdk-surface.json: {error}"));
            return;
        }
    };
    verify_surface_inventory(root, &record.inventory, violations);
}

fn verify_surface_inventory(
    root: &Path,
    inventory: &model::Inventory,
    violations: &mut Vec<String>,
) {
    let paths = match recorded_surface_paths(root) {
        Ok(paths) => paths,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    for (field, directory, suffix, entries) in [
        ("crates", "crates", "/Cargo.toml", &inventory.crates.0),
        (
            "testMods",
            "test-mods",
            "/Cargo.toml",
            &inventory.test_mods.0,
        ),
        ("examples", "examples", "/Cargo.toml", &inventory.examples.0),
        (
            "guideDocuments",
            "docs/guide",
            ".md",
            &inventory.guide_documents.0,
        ),
        (
            "conformanceSuites",
            "tests/conformance/tests/integration",
            ".rs",
            &inventory.conformance_suites.0,
        ),
    ] {
        for entry in entries {
            let name = entry.trim();
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

#[cfg(test)]
pub(crate) fn verify_m4_process_matrix(record: &Value, violations: &mut Vec<String>) {
    let record: model::ProcessMatrix<'_> = match admit(record) {
        Ok(record) => record,
        Err(error) => {
            violations.push(format!("{M4_PROCESS_MATRIX}: {error}"));
            return;
        }
    };
    verify_process_tasks(&record, violations);
}

fn verify_process_tasks(record: &model::ProcessMatrix<'_>, violations: &mut Vec<String>) {
    let recorded = recorded_tasks(
        record.processes.0.iter().map(|process| &process.task),
        "processes",
        violations,
    );
    let outside = recorded_tasks(
        record.checks_outside_the_gate.iter(),
        "checksOutsideTheGate",
        violations,
    );
    for inside in outside.intersection(&recorded) {
        violations.push(format!(
            "{M4_PROCESS_MATRIX} records {inside} under both processes and \
             checksOutsideTheGate; a recorded gate process cannot also be outside that gate"
        ));
    }
    verify_recorded_exit_statuses(record, violations);
}

fn recorded_tasks<'a>(
    rows: impl Iterator<Item = &'a model::Task>,
    field: &str,
    violations: &mut Vec<String>,
) -> BTreeSet<&'a str> {
    let mut tasks = BTreeSet::new();
    for row in rows {
        let command = row.command.text().unwrap_or_default().trim();
        if command.is_empty() {
            violations.push(format!(
                "{M4_PROCESS_MATRIX} records a row under .{field} with no command"
            ));
            continue;
        }
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

fn verify_recorded_exit_statuses(record: &model::ProcessMatrix<'_>, violations: &mut Vec<String>) {
    if record.result != super::model::Verdict::Pass {
        return;
    }
    let rows = record
        .processes
        .0
        .iter()
        .map(|process| ("processes", &process.task))
        .chain(
            record
                .checks_outside_the_gate
                .iter()
                .map(|task| ("checksOutsideTheGate", task)),
        );
    for (field, row) in rows {
        if row.exit_status == Some(0) {
            continue;
        }
        let command = if row.command.text().unwrap_or_default().trim().is_empty() {
            "<no command>"
        } else {
            row.command.text().unwrap_or_default().trim()
        };
        violations.push(format!(
            "{M4_PROCESS_MATRIX} records a nonzero exit status for `{command}` under .{field} \
                 while its own result is pass; a record that reports a failed process and calls \
                 itself green states two things that cannot both be true"
        ));
    }
}

pub(crate) fn verify_m4_integrity(
    root: &Path,
    manifest: &Value,
    violations: &mut Vec<String>,
) -> LoadedRecords {
    let mut records = read_records(root, Milestone::M4, M4_RECORDS, violations);
    if let Some(surface) = records.get("sdk-surface.json") {
        let result = match admit::<model::Surface<'_>>(surface) {
            Ok(surface) => {
                verify_surface_inventory(root, &surface.inventory, violations);
                Ok(verify_surface_record(&surface))
            }
            Err(error) => Err(error),
        };
        records.checked(Milestone::M4, &M4_RECORDS[1], result, violations);
    }
    if let Some(record) = records.get(M4_FIGURES.record) {
        let result = match admit::<model::ProcessMatrix<'_>>(record) {
            Ok(record) => {
                verify_process_tasks(&record, violations);
                verify_typed_matrix_figures(&record, manifest, violations);
                Ok(verify_process_record(&record))
            }
            Err(error) => Err(error),
        };
        records.checked(Milestone::M4, &M4_RECORDS[0], result, violations);
    }
    records
}

const M4_FIGURES: CarriedFigures = CarriedFigures {
    record: "linux-process-matrix.json",
    subject: "gate run",
};

#[cfg(test)]
fn verify_m4_figures(
    milestone: Milestone,
    record: &Value,
    manifest: &Value,
    carried: &CarriedFigures,
    violations: &mut Vec<String>,
) {
    match admit::<model::ProcessMatrix<'_>>(record) {
        Ok(record) => verify_gate_figures(milestone, &record.gate, manifest, carried, violations),
        Err(error) => violations.push(format!("{M4_PROCESS_MATRIX}: {error}")),
    }
}

fn verify_typed_matrix_figures(
    record: &model::ProcessMatrix<'_>,
    manifest: &Value,
    violations: &mut Vec<String>,
) {
    verify_gate_figures(
        Milestone::M4,
        &record.gate,
        manifest,
        &M4_FIGURES,
        violations,
    );
}

fn verify_gate_figures(
    milestone: Milestone,
    gate: &model::Gate,
    manifest: &Value,
    carried: &CarriedFigures,
    violations: &mut Vec<String>,
) {
    crate::evidence::closure::verify_typed_figures::<_, _, fn(&model::Gate) -> bool>(
        milestone,
        gate,
        manifest,
        carried,
        M4_CARRIED_FIGURES,
        &[],
        violations,
    );
}

type GateFigure = fn(&model::Gate) -> Option<Value>;
const M4_CARRIED_FIGURES: &[(&str, GateFigure)] = &[
    ("/tests/total", |gate| Some(Value::from(gate.tests.run))),
    ("/tests/passed", |gate| gate.tests.passed.map(Value::from)),
    ("/tests/failed", |gate| gate.tests.failed.map(Value::from)),
    ("/tests/skipped", |gate| gate.tests.skipped.map(Value::from)),
    ("/tests/lineCoveragePercent", |gate| {
        gate.coverage.as_ref()?.lines_percent.map(Value::from)
    }),
];

#[cfg(test)]
mod tests;

fn verify_process_record(record: &model::ProcessMatrix<'_>) -> Result<(), String> {
    crate::evidence::model::require_identity(record.work_package.0, "M4-W8", "/workPackage")?;
    crate::evidence::model::require_identity(record.protocol.0, "m4-gate-matrix-v1", "/protocol")?;
    require(
        record.result == crate::evidence::model::Verdict::Pass,
        "/result must be pass",
    )?;
    crate::evidence::model::require_identity(record.platform.os.0, "linux", "/platform/os")?;
    crate::evidence::model::require_identity(
        record.platform.architecture.0,
        "x86_64",
        "/platform/architecture",
    )?;
    require(
        !record.game_launch.occurred,
        "/gameLaunch/occurred must be false",
    )?;
    require(record.gate.exit_status == 0, "/gate/exitStatus")?;
    require(record.gate.tests.run > 0, "/gate/tests/run")?;
    for (index, process) in record.processes.0.iter().enumerate() {
        require(
            process.task.exit_status == Some(0),
            "/exitStatus must be zero",
        )
        .map_err(|error| format!("/processes/{index}{error}"))?;
    }
    Ok(())
}

fn verify_surface_record(record: &model::Surface<'_>) -> Result<(), String> {
    let mut packages = record
        .work_packages
        .0
        .iter()
        .map(|row| row.id.0)
        .collect::<Vec<_>>();
    packages.sort_unstable();

    crate::evidence::model::require_identity(record.work_package.0, "M4-W8", "/workPackage")?;
    crate::evidence::model::require_identity(record.protocol.0, "m4-surface-v1", "/protocol")?;
    require(record.work_packages.0.len() == 7, "/workPackages")?;
    require(
        packages
            == [
                "M4-W1", "M4-W2", "M4-W3", "M4-W4", "M4-W5", "M4-W6", "M4-W7",
            ],
        "/workPackages must name M4-W1 through M4-W7 exactly",
    )?;
    Ok(())
}
