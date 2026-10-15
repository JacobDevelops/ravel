//! M4 closure rules.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::evidence::closure::verify_closure_records;
use crate::evidence::manifest::require_manifest_rule;
use crate::inventory::EXAMPLE_NAMES;
use crate::json::jq_value;
use crate::{CRATE_NAMES, TEST_MOD_NAMES};

pub(crate) fn verify_m4_manifest(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M4-W1\", \"M4-W2\", \"M4-W3\", \"M4-W4\", \"M4-W5\", \"M4-W6\", \"M4-W7\", \"M4-W8\"]",
        "M4 manifest must include every work package from M4-W1 through M4-W8 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#".benchmarks.protocolVersion == "m4-surface-v1"
            and ([.benchmarks.results[].name] | sort
                == ["linux-process-matrix.json", "sdk-surface.json"])"#,
        "M4 manifest must record both M4 results under the m4-surface-v1 protocol, so that each \
         one is covered by a recomputed digest",
        violations,
    );

    require_manifest_rule(
        manifest_path,
        r#"(.environment.settings.gateStatus | type == "object")
            and ((.workPackages - (.environment.settings.gateStatus | keys)) == [])
            and ([.environment.settings.gateStatus | to_entries[]
                    | select(.key != "M4-W8") | .value]
                 | length == 7
                 and all(.status == "closed"
                         and (.evidence | type == "string" and length > 0)))"#,
        "M4 closure requires a gateStatus row for every work package, each implementation row \
         closed and naming its evidence",
        violations,
    );

    require_manifest_rule(
        manifest_path,
        r#".attestations | any(
            .platform == "linux"
            and .architecture == "x86_64"
            and (.workloads | index("ravel") != null)
        )"#,
        "M4 closure requires a native Linux attestation for the recorded gate run",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "(.deviations | length) > 0 and (.risks | length) > 0",
        "M4 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
        violations,
    );

    verify_closure_records(root, "M4", "evidence/m4", M4_RECORDS, violations);

    verify_m4_surface(root, violations);
    verify_m4_process_matrix(root, violations);
    verify_m4_gate_figures(root, manifest_path, violations);
}

const M4_RECORDS: &[(&str, &str, &str)] = &[
    (
        "linux-process-matrix.json",
        r#".milestone == "M4"
            and .workPackage == "M4-W8"
            and .protocol == "m4-gate-matrix-v1"
            and .result == "pass"
            and .platform.os == "linux"
            and .platform.architecture == "x86_64"
            and .gameLaunch.occurred == false
            and (.gameLaunch.whyItIsNotHere | type == "string" and length > 0)
            and (.gate.exitStatus | type == "number")
            and .gate.exitStatus == 0
            and (.gate.tests.run | type == "number")
            and .gate.tests.run > 0
            and (.processes | type == "array" and length > 0)
            and (.processes | all(
                (.surface | type == "string" and length > 0)
                and (.command | type == "string" and length > 0)
                and (.exitStatus | type == "number")
                and .exitStatus == 0
                and (.proves | type == "string" and length > 0)
                and (.doesNotProve | type == "string" and length > 0)))"#,
        "a Linux process matrix in which every recorded gate process exited zero and says what it \
         does not prove, and which states in a field rather than in prose that no game process \
         was launched for it",
    ),
    (
        "sdk-surface.json",
        r#".milestone == "M4"
            and .workPackage == "M4-W8"
            and .protocol == "m4-surface-v1"
            and (.inventory.crates | type == "array" and length > 0)
            and (.inventory.testMods | type == "array" and length > 0)
            and (.inventory.examples | type == "array" and length > 0)
            and (.inventory.guideDocuments | type == "array" and length > 0)
            and (.inventory.conformanceSuites | type == "array" and length > 0)
            and ([.workPackages[].id] | sort
                == ["M4-W1", "M4-W2", "M4-W3", "M4-W4", "M4-W5", "M4-W6", "M4-W7"])
            and (.workPackages | all(
                (.shipped | type == "string" and length > 0)
                and (.provenBy | type == "array" and length > 0)
                and (.notProven | type == "string" and length > 0)))"#,
        "an SDK surface record inventorying what each of M4-W1 through M4-W7 shipped, what proves \
         it, and what it does not prove",
    ),
];

pub(crate) fn verify_m4_surface(root: &Path, violations: &mut Vec<String>) {
    let record = root.join("evidence/m4/sdk-surface.json");
    if !record.is_file() {
        return;
    }
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
        let recorded = match jq_value(&record, &format!(".inventory.{field}[]?")) {
            Ok(value) => value
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect::<BTreeSet<_>>(),
            Err(error) => {
                violations.push(error);
                continue;
            }
        };
        for extra in recorded.difference(&present) {
            violations.push(format!(
                "evidence/m4/sdk-surface.json names {extra} under inventory.{field}, which the \
                 repository does not hold"
            ));
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

const GATE_DEFINITION: &str = "Makefile.toml";

const M4_PROCESS_MATRIX: &str = "evidence/m4/linux-process-matrix.json";

const TASK_COMMAND_PREFIX: &str = "devenv tasks run ";

pub(crate) fn verify_m4_process_matrix(root: &Path, violations: &mut Vec<String>) {
    let record = root.join(M4_PROCESS_MATRIX);
    if !record.is_file() {
        return;
    }
    let gate = match gate_sub_tasks(root) {
        Ok(gate) => gate,
        Err(error) => {
            violations.push(error);
            return;
        }
    };

    let Some(recorded) = recorded_tasks(&record, ".processes", violations) else {
        return;
    };
    for extra in recorded.difference(&gate) {
        violations.push(format!(
            "{M4_PROCESS_MATRIX} records {extra} under processes, which {GATE_DEFINITION} does not \
             list among the sub-tasks ravel:verify runs; a row the gate never ran took its exit \
             status from somewhere the record does not name"
        ));
    }
    for missing in gate.difference(&recorded) {
        violations.push(format!(
            "{M4_PROCESS_MATRIX} omits {missing} from processes, which {GATE_DEFINITION} lists \
             among the sub-tasks ravel:verify runs; a gate process with no row is how a failing \
             one leaves a record that still reads complete"
        ));
    }

    if let Some(outside) = recorded_tasks(&record, ".checksOutsideTheGate", violations) {
        for inside in outside.intersection(&gate) {
            violations.push(format!(
                "{M4_PROCESS_MATRIX} records {inside} under checksOutsideTheGate while \
                 {GATE_DEFINITION} lists it among the sub-tasks ravel:verify runs; that section is \
                 for checks the gate does not run and is not a way out of the reconciliation"
            ));
        }
    }

    verify_recorded_exit_statuses(&record, violations);
}

fn gate_sub_tasks(root: &Path) -> Result<BTreeSet<String>, String> {
    let source = fs::read_to_string(root.join(GATE_DEFINITION))
        .map_err(|error| format!("cannot read {GATE_DEFINITION}: {error}"))?;
    let parsed = toml::from_str::<toml::Table>(&source)
        .map_err(|error| format!("cannot parse {GATE_DEFINITION} as TOML: {error}"))?;
    let dependencies = parsed
        .get("tasks")
        .and_then(|tasks| tasks.get("ravel:verify"))
        .and_then(|gate| gate.get("dependencies"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| {
            format!("{GATE_DEFINITION} must declare a dependencies array for ravel:verify")
        })?;
    let mut names = BTreeSet::new();
    for dependency in dependencies {
        let name = dependency
            .as_str()
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| {
                format!(
                    "{GATE_DEFINITION} ravel:verify dependencies must be nonempty task-name strings"
                )
            })?;
        names.insert(name.to_owned());
    }
    if names.is_empty() {
        return Err(format!(
            "{GATE_DEFINITION} states no dependencies for ravel:verify, so nothing says which \
             sub-tasks the gate runs and no record of them can be reconciled"
        ));
    }
    Ok(names)
}

fn recorded_tasks(
    record: &Path,
    field: &str,
    violations: &mut Vec<String>,
) -> Option<BTreeSet<String>> {
    let commands = match jq_value(record, &format!("{field}[]?.command")) {
        Ok(commands) => commands,
        Err(error) => {
            violations.push(error);
            return None;
        }
    };
    let mut tasks = BTreeSet::new();
    for command in commands
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        match command.strip_prefix(TASK_COMMAND_PREFIX) {
            Some(task) => {
                tasks.insert(task.trim().to_owned());
            }
            None => violations.push(format!(
                "{M4_PROCESS_MATRIX} records the command `{command}` under {field}, which does not \
                 name a devenv task; a row nothing can resolve to a task is reconciled against \
                 nothing"
            )),
        }
    }
    Some(tasks)
}

fn verify_recorded_exit_statuses(record: &Path, violations: &mut Vec<String>) {
    match jq_value(record, ".result") {
        Ok(result) if result == "pass" => {}
        Ok(_) => return,
        Err(error) => {
            violations.push(error);
            return;
        }
    }
    for field in [".processes", ".checksOutsideTheGate"] {
        let failed = match jq_value(
            record,
            &format!("{field}[]? | select(.exitStatus != 0) | .command"),
        ) {
            Ok(failed) => failed,
            Err(error) => {
                violations.push(error);
                continue;
            }
        };
        for command in failed
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            violations.push(format!(
                "{M4_PROCESS_MATRIX} records a nonzero exit status for `{command}` under {field} \
                 while its own result is pass; a record that reports a failed process and calls \
                 itself green states two things that cannot both be true"
            ));
        }
    }
}

pub(crate) fn verify_m4_gate_figures(
    root: &Path,
    manifest_path: &Path,
    violations: &mut Vec<String>,
) {
    let record = root.join("evidence/m4/linux-process-matrix.json");
    if !record.is_file() {
        return;
    }
    for (claimed_at, measured_at) in [
        (".tests.total", ".gate.tests.run"),
        (".tests.passed", ".gate.tests.passed"),
        (".tests.failed", ".gate.tests.failed"),
        (".tests.skipped", ".gate.tests.skipped"),
        (".tests.lineCoveragePercent", ".gate.coverage.linesPercent"),
    ] {
        let (Ok(claimed), Ok(measured)) = (
            jq_value(manifest_path, claimed_at),
            jq_value(&record, measured_at),
        ) else {
            violations.push(format!(
                "evidence/m4/manifest.json {claimed_at} and evidence/m4/linux-process-matrix.json \
                 {measured_at} cannot both be read, so nothing reconciles them"
            ));
            continue;
        };
        if claimed != measured {
            violations.push(format!(
                "evidence/m4/manifest.json records {claimed_at} {claimed} while \
                 evidence/m4/linux-process-matrix.json records {measured_at} {measured}; they \
                 describe one gate run"
            ));
        }
    }
}

#[cfg(test)]
mod tests;
