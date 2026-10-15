//! jq-backed evidence queries.

use std::path::Path;
use std::process::Command;

pub(crate) fn jq_value(path: &Path, filter: &str) -> Result<String, String> {
    let output = Command::new("jq")
        .args(["-r", filter])
        .arg(path)
        .output()
        .map_err(|error| format!("cannot read JSON with jq: {error}"))?;
    if !output.status.success() {
        return Err(format!("cannot read {filter} from {}", redacted_path(path)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
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

pub(crate) fn jq_matches(path: &Path, filter: &str) -> Result<bool, String> {
    let output = Command::new("jq")
        .args(["--exit-status", filter])
        .arg(path)
        .output()
        .map_err(|error| format!("cannot validate JSON with jq: {error}"))?;
    if output.status.success() {
        return Ok(true);
    }
    match output.status.code() {
        Some(1 | 4) => Ok(false),
        Some(code) => Err(format!(
            "cannot evaluate {filter} against {}: jq exited {code}",
            redacted_path(path)
        )),
        None => Err(format!(
            "cannot evaluate {filter} against {}: jq was terminated by a signal",
            redacted_path(path)
        )),
    }
}

pub(crate) fn evidence_schema_matches(root: &Path, manifest: &Path) -> Result<bool, String> {
    let output = Command::new("check-jsonschema")
        .arg("--schemafile")
        .arg(root.join("schemas/evidence.schema.json"))
        .arg(manifest)
        .output()
        .map_err(|error| format!("cannot validate evidence schema: {error}"))?;
    Ok(output.status.success())
}

#[cfg(test)]
mod tests;
