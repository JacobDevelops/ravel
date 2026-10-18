//! In-process JSON loading and external schema validation.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::text::redacted_path;
use serde_json::Value;

pub(crate) fn read_json(root: &Path, path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read JSON {}: {error}", redacted_path(root, path)))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("cannot parse JSON {}: {error}", redacted_path(root, path)))
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

pub(crate) fn sum(value: &Value, pointers: &[&str]) -> Option<f64> {
    pointers
        .iter()
        .try_fold(0.0, |sum, pointer| Some(sum + number(value, pointer)?))
}

pub(crate) fn column_sum(value: &Value, rows: &str, field: &str) -> Option<f64> {
    value
        .pointer(rows)?
        .as_array()?
        .iter()
        .try_fold(0.0, |sum, row| Some(sum + number(row, field)?))
}

pub(crate) fn nonempty(value: &Value, pointer: &str) -> bool {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .is_some_and(|text| !text.is_empty())
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
            redacted_path(root, document),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
    }
}

#[cfg(test)]
mod tests;
