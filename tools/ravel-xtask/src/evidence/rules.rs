//! Shared predicates for milestone evidence records.

use std::path::Path;

use serde_json::Value;

use crate::json::{array, nonempty, schema_matches};

pub(crate) type Rule = fn(&Value) -> bool;
pub(crate) type Figure = fn(&Value) -> Option<Value>;

pub(crate) fn has_exact_work_packages(value: &Value, expected: &[&str]) -> bool {
    let mut packages = array(value, "/workPackages")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    packages.sort_unstable();
    array(value, "/workPackages").len() == expected.len() && packages == expected
}

/// Matches the complete result-name set under its recorded protocol.
/// Expected names must be sorted, just like work-package identifiers.
pub(crate) fn has_exact_benchmark_results(
    value: &Value,
    protocol: &str,
    expected: &[&str],
) -> bool {
    let results = array(value, "/benchmarks/results");
    let mut names = results
        .iter()
        .filter_map(|row| row["name"].as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    value["benchmarks"]["protocolVersion"] == protocol
        && results.len() == expected.len()
        && names == expected
}

pub(crate) fn all_gates_closed(value: &Value, closure_row: &str, expected_closed: usize) -> bool {
    let Some(gates) = value
        .pointer("/environment/settings/gateStatus")
        .and_then(Value::as_object)
    else {
        return false;
    };
    array(value, "/workPackages")
        .iter()
        .all(|package| package.as_str().is_some_and(|id| gates.contains_key(id)))
        && gates
            .iter()
            .filter(|(id, _)| id.as_str() != closure_row)
            .count()
            == expected_closed
        && gates
            .iter()
            .filter(|(id, _)| id.as_str() != closure_row)
            .all(|(_, gate)| gate["status"] == "closed" && nonempty(gate, "/evidence"))
}

pub(crate) fn has_native_attestation(value: &Value, workload: &str) -> bool {
    array(value, "/attestations").iter().any(|attestation| {
        attestation["platform"] == "linux"
            && attestation["architecture"] == "x86_64"
            && array(attestation, "/workloads")
                .iter()
                .any(|recorded| recorded == workload)
    })
}

pub(crate) fn evidence_schema_matches(root: &Path, manifest: &Path) -> Result<bool, String> {
    schema_matches(root, "schemas/evidence.schema.json", manifest)
}

#[cfg(test)]
mod tests;
