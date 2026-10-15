//! Saved architecture spike result validation.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::evidence::manifest::verify_redaction;
use crate::json::jq_matches;

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
    let spikes = root.join("evidence/m1/spikes");

    for (directory, gate) in SPIKE_GATES {
        let gate_root = spikes.join(directory);
        if !gate_root.is_dir() {
            continue;
        }

        let mut results = fs::read_dir(&gate_root)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect::<Vec<_>>();
        results.sort();

        if results.is_empty() {
            violations.push(format!("spike gate {gate} has no recorded result"));
            continue;
        }
        for result in &results {
            verify_spike_result(root, result, gate, &mut violations);
        }
    }

    violations
}

fn verify_spike_result(root: &Path, path: &Path, gate: &str, violations: &mut Vec<String>) {
    let label = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string();

    match spike_schema_matches(root, path) {
        Ok(true) => {}
        Ok(false) => violations.push(format!("{label} does not conform to the spike schema")),
        Err(error) => violations.push(error),
    }

    let rules = [
        (
            ".schemaVersion == 1 and .protocolVersion == \"spike-v1\"",
            "must record spike protocol version 1",
        ),
        (
            &format!(".gate == \"{gate}\""),
            "must record the gate of the directory it belongs to",
        ),
        (
            r#".candidate.disposition == "rejected"
                or ([.failures[].scenario] | sort
                    == ["callback-failure", "crash", "invalid-input", "timeout",
                        "unavailable-runtime"])"#,
            "must exercise every required failure scenario exactly once",
        ),
        (
            r#".candidate.disposition == "rejected"
                or ([.failures[] | select(.outcome == "failed-closed")] | length > 0)"#,
            "a measured candidate must record at least one failure that closed",
        ),
        (
            r#"[.memory.bootstrapPeakRss, .memory.retainedRss, .memory.retainedClasses,
                .startup.processToReady]
                | all((.status == "unavailable") or (.value > 0))"#,
            "must not report a measured bootstrap or startup value of zero",
        ),
        (
            r#".candidate.disposition == "rejected"
                or .calls.callbackNanoseconds.status == "measured"
                or (.calls.callbackNanoseconds.reason | length > 0)"#,
            "must measure callback cost or record why it is unavailable",
        ),
        (
            r#".deviations | all(.classification != "blocking")"#,
            "must not carry a blocking deviation",
        ),
        (
            r#".trial.measuredInvocations > 0 or .candidate.disposition == "rejected""#,
            "must measure at least one invocation",
        ),
    ];
    for (filter, requirement) in rules {
        match jq_matches(path, filter) {
            Ok(true) => {}
            Ok(false) => violations.push(format!("{label} {requirement}")),
            Err(error) => violations.push(error),
        }
    }

    verify_redaction(path, &label, violations);
}

fn spike_schema_matches(root: &Path, result: &Path) -> Result<bool, String> {
    let output = Command::new("check-jsonschema")
        .arg("--schemafile")
        .arg(root.join("schemas/spike-result.schema.json"))
        .arg(result)
        .output()
        .map_err(|error| format!("cannot validate spike schema: {error}"))?;
    Ok(output.status.success())
}

#[cfg(test)]
mod tests;
