//! Saved architecture spike result validation.

use std::fs;
use std::path::{Path, PathBuf};

use crate::evidence::rules::Rule;
use crate::evidence::rules::verify_redaction_value;
use crate::json::{array, nonempty, number, read_json, schema_matches};
use crate::text::relative_label;

pub(crate) const SPIKE_GATES: &[(&str, &str)] = &[
    ("adg-1", "ADG-1"),
    ("adg-2", "ADG-2"),
    ("adg-3", "ADG-3"),
    ("adg-4", "ADG-4"),
    ("adg-5", "ADG-5"),
];

/// Checks every recorded architecture spike result against the M1 gate rules.
#[must_use]
pub fn verify_spikes(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    for (directory, gate) in SPIKE_GATES {
        if !root.join("evidence/m1/spikes").join(directory).exists() {
            continue;
        }
        let results = spike_result_paths(root, directory, gate, &mut violations);
        if results.is_empty() {
            violations.push(format!("spike gate {gate} has no recorded result"));
        }
        for result in results {
            verify_spike_result(root, &result, gate, &mut violations);
        }
    }
    violations
}

pub(crate) fn spike_result_paths(
    root: &Path,
    directory: &str,
    gate: &str,
    violations: &mut Vec<String>,
) -> Vec<PathBuf> {
    let entries = match fs::read_dir(root.join("evidence/m1/spikes").join(directory)) {
        Ok(entries) => entries,
        Err(error) => {
            violations.push(format!("cannot read spike gate {gate}: {error}"));
            return Vec::new();
        }
    };
    let mut results = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) if entry.path().extension().is_some_and(|kind| kind == "json") => {
                results.push(entry.path());
            }
            Ok(_) => {}
            Err(error) => violations.push(format!("cannot read spike gate {gate} entry: {error}")),
        }
    }
    results.sort();
    results
}

fn verify_spike_result(root: &Path, path: &Path, gate: &str, violations: &mut Vec<String>) {
    let label = relative_label(root, path);
    let document = match read_json(root, path) {
        Ok(document) => document,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    match schema_matches(root, "schemas/spike-result.schema.json", path) {
        Ok(true) => {}
        Ok(false) => violations.push(format!("{label} does not conform to the spike schema")),
        Err(error) => violations.push(error),
    }
    if document["gate"] != gate {
        violations.push(format!(
            "{label} must record the gate of the directory it belongs to"
        ));
    }
    for (rule, requirement) in RULES {
        if !rule(&document) {
            violations.push(format!("{label} {requirement}"));
        }
    }
    verify_redaction_value(&document, &label, violations);
}

const RULES: &[(Rule, &str)] = &[
    (
        |value| value["schemaVersion"] == 1 && value["protocolVersion"] == "spike-v1",
        "must record spike protocol version 1",
    ),
    (
        |value| {
            let failures = array(value, "/failures");
            let mut scenarios = failures
                .iter()
                .filter_map(|failure| failure["scenario"].as_str())
                .collect::<Vec<_>>();
            scenarios.sort_unstable();
            value["candidate"]["disposition"] == "rejected"
                || (failures.len() == 5
                    && scenarios
                        == [
                            "callback-failure",
                            "crash",
                            "invalid-input",
                            "timeout",
                            "unavailable-runtime",
                        ])
        },
        "must exercise every required failure scenario exactly once",
    ),
    (
        |value| {
            value["candidate"]["disposition"] == "rejected"
                || array(value, "/failures")
                    .iter()
                    .any(|failure| failure["outcome"] == "failed-closed")
        },
        "a measured candidate must record at least one failure that closed",
    ),
    (
        |value| {
            [
                "/memory/bootstrapPeakRss",
                "/memory/retainedRss",
                "/memory/retainedClasses",
                "/startup/processToReady",
            ]
            .iter()
            .all(|pointer| {
                value.pointer(pointer).is_some_and(|measurement| {
                    measurement["status"] == "unavailable"
                        || number(measurement, "/value").is_some_and(|number| number > 0.0)
                })
            })
        },
        "must not report a measured bootstrap or startup value of zero",
    ),
    (
        |value| {
            value["candidate"]["disposition"] == "rejected"
                || value["calls"]["callbackNanoseconds"]["status"] == "measured"
                || nonempty(value, "/calls/callbackNanoseconds/reason")
        },
        "must measure callback cost or record why it is unavailable",
    ),
    (
        |value| {
            array(value, "/deviations")
                .iter()
                .all(|deviation| deviation["classification"] != "blocking")
        },
        "must not carry a blocking deviation",
    ),
    (
        |value| {
            value["candidate"]["disposition"] == "rejected"
                || number(value, "/trial/measuredInvocations").is_some_and(|number| number > 0.0)
        },
        "must measure at least one invocation",
    ),
];

#[cfg(test)]
mod tests;
