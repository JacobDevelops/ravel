//! M4 closure rules.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::evidence::closure::{CarriedFigures, verify_closure_records, verify_milestone_figures};
use crate::evidence::manifest::require_manifest_rule;
use crate::json::{Rule, array, closed_gates, nonempty, number, read_json, work_packages};
use crate::{CRATE_NAMES, EXAMPLE_NAMES, TEST_MOD_NAMES};

pub(crate) fn verify_m4_manifest(root: &Path, manifest: &Value, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest,
        |value| {
            work_packages(
                value,
                &[
                    "M4-W1", "M4-W2", "M4-W3", "M4-W4", "M4-W5", "M4-W6", "M4-W7", "M4-W8",
                ],
            )
        },
        "M4 manifest must include every work package from M4-W1 through M4-W8 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest,
        |value| {
            let mut names = array(value, "/benchmarks/results")
                .iter()
                .filter_map(|row| row["name"].as_str())
                .collect::<Vec<_>>();
            names.sort_unstable();
            value["benchmarks"]["protocolVersion"] == "m4-surface-v1"
                && array(value, "/benchmarks/results").len() == 2
                && names == ["linux-process-matrix.json", "sdk-surface.json"]
        },
        "M4 manifest must record both M4 results under the m4-surface-v1 protocol, so that each \
         one is covered by a recomputed digest",
        violations,
    );

    require_manifest_rule(
        manifest,
        |value| closed_gates(value, "M4-W8", 7),
        "M4 closure requires a gateStatus row for every work package, each implementation row \
         closed and naming its evidence",
        violations,
    );

    require_manifest_rule(
        manifest,
        |value| {
            array(value, "/attestations").iter().any(|row| {
                row["platform"] == "linux"
                    && row["architecture"] == "x86_64"
                    && array(row, "/workloads")
                        .iter()
                        .any(|workload| workload == "ravel")
            })
        },
        "M4 closure requires a native Linux attestation for the recorded gate run",
        violations,
    );
    require_manifest_rule(
        manifest,
        |value| !array(value, "/deviations").is_empty() && !array(value, "/risks").is_empty(),
        "M4 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
        violations,
    );

    verify_closure_records(root, "M4", "evidence/m4", M4_RECORDS, violations);
}

const M4_RECORDS: &[(&str, Rule, &str)] = &[
    (
        "linux-process-matrix.json",
        |value| {
            value["milestone"] == "M4"
                && value["workPackage"] == "M4-W8"
                && value["protocol"] == "m4-gate-matrix-v1"
                && value["result"] == "pass"
                && value["platform"]["os"] == "linux"
                && value["platform"]["architecture"] == "x86_64"
                && value["gameLaunch"]["occurred"] == false
                && nonempty(value, "/gameLaunch/whyItIsNotHere")
                && number(value, "/gate/exitStatus") == Some(0.0)
                && number(value, "/gate/tests/run").is_some_and(|count| count > 0.0)
                && !array(value, "/processes").is_empty()
                && array(value, "/processes").iter().all(|row| {
                    ["/surface", "/command", "/proves", "/doesNotProve"]
                        .iter()
                        .all(|pointer| nonempty(row, pointer))
                        && number(row, "/exitStatus") == Some(0.0)
                })
        },
        "a Linux process matrix in which every recorded gate process exited zero and says what it \
         does not prove, and which states in a field rather than in prose that no game process \
         was launched for it",
    ),
    (
        "sdk-surface.json",
        |value| {
            let mut packages = array(value, "/workPackages")
                .iter()
                .filter_map(|row| row["id"].as_str())
                .collect::<Vec<_>>();
            packages.sort_unstable();
            value["milestone"] == "M4"
                && value["workPackage"] == "M4-W8"
                && value["protocol"] == "m4-surface-v1"
                && array(value, "/workPackages").len() == 7
                && [
                    "/inventory/crates",
                    "/inventory/testMods",
                    "/inventory/examples",
                    "/inventory/guideDocuments",
                    "/inventory/conformanceSuites",
                ]
                .iter()
                .all(|pointer| !array(value, pointer).is_empty())
                && packages
                    == [
                        "M4-W1", "M4-W2", "M4-W3", "M4-W4", "M4-W5", "M4-W6", "M4-W7",
                    ]
                && array(value, "/workPackages").iter().all(|row| {
                    nonempty(row, "/shipped")
                        && !array(row, "/provenBy").is_empty()
                        && nonempty(row, "/notProven")
                })
        },
        "an SDK surface record inventorying what each of M4-W1 through M4-W7 shipped, what proves \
         it, and what it does not prove",
    ),
];

pub(crate) fn verify_m4_surface(root: &Path, violations: &mut Vec<String>) {
    let record = root.join("evidence/m4/sdk-surface.json");
    if !record.is_file() {
        return;
    }
    let value = match read_json(&record) {
        Ok(value) => value,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let present = [
        ("crates", listed_names(CRATE_NAMES)),
        ("testMods", listed_names(TEST_MOD_NAMES)),
        ("examples", listed_names(EXAMPLE_NAMES)),
        ("guideDocuments", files_named(root, "docs/guide", "md")),
        (
            "conformanceSuites",
            files_named(root, "tests/conformance/tests/integration", "rs"),
        ),
    ];
    for (field, present) in present {
        for entry in array(&value, &format!("/inventory/{field}")) {
            let Some(name) = entry.as_str() else {
                violations.push(format!(
                    "evidence/m4/sdk-surface.json names non-string {entry} under inventory.{field}, \
                     which cannot name a repository file"
                ));
                continue;
            };
            let name = name.trim();
            if !name.is_empty() && !present.contains(name) {
                violations.push(format!(
                    "evidence/m4/sdk-surface.json names {name} under inventory.{field}, which the \
                     repository does not hold"
                ));
            }
        }
    }
}

fn listed_names(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn files_named(root: &Path, directory: &str, extension: &str) -> BTreeSet<String> {
    let Ok(entries) = fs::read_dir(root.join(directory)) else {
        return BTreeSet::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| {
            Path::new(name)
                .extension()
                .is_some_and(|found| found == extension)
        })
        .collect()
}

const M4_PROCESS_MATRIX: &str = "evidence/m4/linux-process-matrix.json";

const TASK_COMMAND_PREFIX: &str = "devenv tasks run ";

pub(crate) fn verify_m4_process_matrix(root: &Path, violations: &mut Vec<String>) {
    let path = root.join(M4_PROCESS_MATRIX);
    if !path.is_file() {
        return;
    }
    let record = match read_json(&path) {
        Ok(record) => record,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let recorded = recorded_tasks(&record, "processes", violations);
    let outside = recorded_tasks(&record, "checksOutsideTheGate", violations);
    for inside in outside.intersection(&recorded) {
        violations.push(format!(
            "{M4_PROCESS_MATRIX} records {inside} under both processes and \
             checksOutsideTheGate; a recorded gate process cannot also be outside that gate"
        ));
    }
    verify_recorded_exit_statuses(&record, violations);
}

fn recorded_tasks<'a>(
    record: &'a Value,
    field: &str,
    violations: &mut Vec<String>,
) -> BTreeSet<&'a str> {
    let mut tasks = BTreeSet::new();
    for row in array(record, &format!("/{field}")) {
        let command = row["command"].as_str().unwrap_or("null").trim();
        if command.is_empty() {
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

fn verify_recorded_exit_statuses(record: &Value, violations: &mut Vec<String>) {
    if record["result"] != "pass" {
        return;
    }
    for field in ["processes", "checksOutsideTheGate"] {
        for row in array(record, &format!("/{field}")) {
            if number(row, "/exitStatus") == Some(0.0) {
                continue;
            }
            let command = row["command"].as_str().unwrap_or("null").trim();
            if !command.is_empty() {
                violations.push(format!(
                    "{M4_PROCESS_MATRIX} records a nonzero exit status for `{command}` under .{field} \
                     while its own result is pass; a record that reports a failed process and calls \
                     itself green states two things that cannot both be true"
                ));
            }
        }
    }
}

pub(crate) fn verify_m4_integrity(root: &Path, manifest: &Value, violations: &mut Vec<String>) {
    verify_m4_surface(root, violations);
    verify_m4_process_matrix(root, violations);
    verify_m4_gate_figures(root, manifest, violations);
}

pub(crate) fn verify_m4_gate_figures(root: &Path, manifest: &Value, violations: &mut Vec<String>) {
    verify_milestone_figures(root, manifest, &M4_FIGURES, violations);
}

const M4_FIGURES: CarriedFigures = CarriedFigures {
    manifest: "evidence/m4/manifest.json",
    record: M4_PROCESS_MATRIX,
    subject: "gate run",
    figures: &[
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
    ],
};

#[cfg(test)]
mod tests;
