//! In-process JSON loading and external schema validation.

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

pub(crate) type Rule = fn(&Value) -> bool;
pub(crate) type Figure = fn(&Value) -> Option<Value>;

pub(crate) fn read_json(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read JSON {}: {error}", redacted_path(path)))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("cannot parse JSON {}: {error}", redacted_path(path)))
}

pub(crate) fn array<'a>(value: &'a Value, pointer: &str) -> &'a [Value] {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

pub(crate) fn number(value: &Value, pointer: &str) -> Option<f64> {
    value.pointer(pointer).and_then(Value::as_f64)
}

pub(crate) fn nonempty(value: &Value, pointer: &str) -> bool {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .is_some_and(|text| !text.is_empty())
}

pub(crate) fn work_packages(value: &Value, expected: &[&str]) -> bool {
    let mut packages = array(value, "/workPackages")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    packages.sort_unstable();
    array(value, "/workPackages").len() == expected.len() && packages == expected
}

pub(crate) fn closed_gates(value: &Value, closure_row: &str, count: usize) -> bool {
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
            == count
        && gates
            .iter()
            .filter(|(id, _)| id.as_str() != closure_row)
            .all(|(_, gate)| gate["status"] == "closed" && nonempty(gate, "/evidence"))
}

pub(crate) fn native_attestation(value: &Value) -> bool {
    array(value, "/attestations").iter().any(|attestation| {
        attestation["platform"] == "linux"
            && attestation["architecture"] == "x86_64"
            && array(attestation, "/workloads")
                .iter()
                .any(|workload| workload == "official-atm11")
    })
}

pub(crate) fn redacted_path(path: &Path) -> String {
    if path.is_relative() {
        return path.display().to_string();
    }
    std::env::current_dir()
        .ok()
        .and_then(|directory| path.strip_prefix(directory).ok().map(Path::to_path_buf))
        .map_or_else(
            || {
                path.file_name().map_or_else(
                    || "an unnamed path".to_owned(),
                    |name| name.to_string_lossy().into_owned(),
                )
            },
            |relative| relative.display().to_string(),
        )
}

pub(crate) fn schema_matches(root: &Path, schema: &str, document: &Path) -> Result<bool, String> {
    let output = Command::new("check-jsonschema")
        .arg("--schemafile")
        .arg(root.join(schema))
        .arg(document)
        .output()
        .map_err(|error| format!("cannot validate JSON schema: {error}"))?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        code => Err(format!(
            "cannot validate {} with {schema}: check-jsonschema exited {code:?}: {}",
            redacted_path(document),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
    }
}

pub(crate) fn evidence_schema_matches(root: &Path, manifest: &Path) -> Result<bool, String> {
    schema_matches(root, "schemas/evidence.schema.json", manifest)
}

#[cfg(test)]
mod tests;
