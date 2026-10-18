//! Shared predicates for milestone evidence records.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde_json::Value;

use crate::json::{array, nonempty, schema_matches};

use ravel_core::{CREDENTIAL_WORDS, contains_credential};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Milestone {
    M0,
    M1,
    M2,
    M3,
    M4,
    M5,
    M6,
    M7,
    M8,
    M9,
    M10,
    M11,
    M12,
    M13,
}

impl Milestone {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::M0 => "M0",
            Self::M1 => "M1",
            Self::M2 => "M2",
            Self::M3 => "M3",
            Self::M4 => "M4",
            Self::M5 => "M5",
            Self::M6 => "M6",
            Self::M7 => "M7",
            Self::M8 => "M8",
            Self::M9 => "M9",
            Self::M10 => "M10",
            Self::M11 => "M11",
            Self::M12 => "M12",
            Self::M13 => "M13",
        }
    }

    pub(crate) fn directory(self) -> String {
        format!("evidence/{}", self.as_str().to_ascii_lowercase())
    }
}

impl fmt::Display for Milestone {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Milestone {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "M0" => Ok(Self::M0),
            "M1" => Ok(Self::M1),
            "M2" => Ok(Self::M2),
            "M3" => Ok(Self::M3),
            "M4" => Ok(Self::M4),
            "M5" => Ok(Self::M5),
            "M6" => Ok(Self::M6),
            "M7" => Ok(Self::M7),
            "M8" => Ok(Self::M8),
            "M9" => Ok(Self::M9),
            "M10" => Ok(Self::M10),
            "M11" => Ok(Self::M11),
            "M12" => Ok(Self::M12),
            "M13" => Ok(Self::M13),
            _ => Err(format!("invalid milestone ID: {text}")),
        }
    }
}

/// Discovers milestone directories; identity inside each document is checked separately.
pub(crate) fn milestone_manifests(root: &Path) -> Result<Vec<(PathBuf, Milestone)>, String> {
    let entries = fs::read_dir(root.join("evidence"))
        .map_err(|error| format!("cannot read evidence directory: {error}"))?;
    let mut manifests = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read evidence entry: {error}"))?;
        let kind = entry
            .file_type()
            .map_err(|error| format!("cannot inspect evidence entry: {error}"))?;
        if !kind.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with('m') || !name[1..].bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let milestone = name.to_ascii_uppercase().parse::<Milestone>()?;
        let relative = PathBuf::from("evidence")
            .join(name.as_ref())
            .join("manifest.json");
        crate::text::require_file(root, &relative.to_string_lossy())?;
        manifests.push((relative, milestone));
    }
    if manifests.is_empty() {
        return Err("evidence directory contains no milestone manifests".to_owned());
    }
    manifests.sort_by_key(|(_, milestone)| *milestone);
    Ok(manifests)
}

pub(crate) type RecordRule = fn(&Value) -> Result<(), String>;

pub(crate) fn require(condition: bool, requirement: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(requirement.to_owned())
    }
}

pub(crate) fn unique_by(rows: &[Value], same_key: impl Fn(&Value, &Value) -> bool) -> bool {
    rows.iter()
        .enumerate()
        .all(|(index, row)| rows[..index].iter().all(|other| !same_key(other, row)))
}

pub(crate) fn require_nonempty_array<'a>(
    value: &'a Value,
    pointer: &'static str,
) -> Result<&'a [Value], String> {
    let rows = array(value, pointer);
    require(!rows.is_empty(), pointer)?;
    Ok(rows)
}

pub(crate) fn require_fields(
    value: &Value,
    pointers: &[&'static str],
    predicate: fn(&Value) -> bool,
) -> Result<(), String> {
    for pointer in pointers {
        require(value.pointer(pointer).is_some_and(predicate), pointer)?;
    }
    Ok(())
}

pub(crate) fn require_rows(
    value: &Value,
    pointer: &'static str,
    rule: impl Fn(&Value) -> Result<(), String>,
) -> Result<(), String> {
    for (index, row) in require_nonempty_array(value, pointer)?.iter().enumerate() {
        rule(row).map_err(|requirement| format!("{pointer}/{index}{requirement}"))?;
    }
    Ok(())
}

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
pub(crate) fn any_string(value: &Value, predicate: &impl Fn(&str) -> bool) -> bool {
    match value {
        Value::String(text) => predicate(text),
        Value::Array(values) => values.iter().any(|value| any_string(value, predicate)),
        Value::Object(values) => values.values().any(|value| any_string(value, predicate)),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

pub(crate) fn verify_redaction_value(
    document: &Value,
    subject: &str,
    violations: &mut Vec<String>,
) {
    let sensitive = any_string(document, &|text| {
        let lower = text.to_ascii_lowercase();
        [
            "/home/",
            "/users/",
            ":\\users\\",
            "/.minecraft",
            "tests/fixtures/local/",
            "evidence/private/",
        ]
        .iter()
        .any(|pattern| lower.contains(pattern))
            || contains_credential(text)
    }) || has_credential_field(document, false);
    if sensitive {
        violations.push(format!("{subject} contains sensitive or local data"));
    }
}

fn has_credential_field(value: &Value, sensitive_path: bool) -> bool {
    match value {
        Value::String(text) => sensitive_path && !text.is_empty(),
        Value::Array(values) => values
            .iter()
            .any(|value| has_credential_field(value, sensitive_path)),
        Value::Object(values) => values.iter().any(|(name, value)| {
            let lower = name.to_ascii_lowercase();
            has_credential_field(
                value,
                sensitive_path || CREDENTIAL_WORDS.iter().any(|word| lower.contains(word)),
            )
        }),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

pub(crate) fn require_manifest_rule(
    manifest: &Value,
    rule: impl FnOnce(&Value) -> bool,
    violation: &str,
    violations: &mut Vec<String>,
) {
    if !rule(manifest) {
        violations.push(violation.to_owned());
    }
}

#[cfg(test)]
mod tests;
