//! Tracked-path, dependency-path, and redaction checks.

use std::fs;
use std::path::Path;
use std::process::Command;

const PRIVATE_WORKSPACE_NAME: [&str; 2] = ["ravel-", "registry"];

fn private_workspace_name() -> String {
    PRIVATE_WORKSPACE_NAME.concat()
}

/// Rejects secret paths and private workspace names from a tracked tree.
#[must_use]
pub fn verify_tracked_paths<'a>(paths: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let private = private_workspace_name();
    paths
        .into_iter()
        .flat_map(|path| {
            let normalized = path.replace('\\', "/");
            let lower = normalized.to_ascii_lowercase();
            let mut violations = Vec::new();
            if is_forbidden_tracked_path(&normalized) {
                violations.push(format!("forbidden tracked path: {normalized}"));
            }
            if lower.contains(&private) {
                violations.push(format!(
                    "tracked path names the private compatibility workspace: {normalized}"
                ));
            }
            violations
        })
        .collect()
}

/// Rejects private keys, credential assignments, and private-workspace
/// references in tracked text.
/// Home-directory paths and public upstream identifiers are deliberately not refused.
#[must_use]
pub fn verify_tracked_contents(root: &Path, paths: &[String]) -> Vec<String> {
    let private = private_workspace_name();
    let mut violations = Vec::new();
    for relative in paths {
        let path = root.join(relative);
        let read = if path.is_symlink() {
            fs::read_link(&path).map(|target| target.to_string_lossy().into_owned())
        } else {
            fs::read(&path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        };
        let source = match read {
            Ok(source) => source,
            Err(error) => {
                violations.push(format!("cannot scan tracked file {relative}: {error}"));
                continue;
            }
        };

        let private_key = [
            "PRIVATE KEY-----",
            "RSA PRIVATE KEY-----",
            "OPENSSH PRIVATE KEY-----",
        ]
        .iter()
        .any(|suffix| source.contains(&format!("{}{}", "-----BEGIN ", suffix)));
        let credential = source.lines().any(has_credential_assignment);
        if private_key || credential {
            violations.push(format!(
                "tracked file contains credential material: {relative}"
            ));
        }

        if source.to_ascii_lowercase().contains(&private) {
            violations.push(format!(
                "tracked file names the private compatibility workspace: {relative}"
            ));
        }
        if relative.rsplit('/').next() == Some("Cargo.toml") {
            for value in escaping_path_dependencies(relative, &source) {
                violations.push(format!(
                    "tracked manifest depends on a path outside this repository: {relative} -> {value}"
                ));
            }
        }
    }
    violations
}

fn escaping_path_dependencies<'a>(relative_manifest: &str, source: &'a str) -> Vec<&'a str> {
    let directory = relative_manifest
        .rsplit_once('/')
        .map_or("", |(head, _)| head);
    source
        .lines()
        .flat_map(manifest_path_values)
        .filter(|value| resolves_outside_root(directory, value))
        .collect()
}

fn manifest_path_values(line: &str) -> Vec<&str> {
    let mut values = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = line[cursor..].find("path") {
        let at = cursor + offset;
        cursor = at + "path".len();

        if line[..at]
            .chars()
            .next_back()
            .is_some_and(|character| character.is_alphanumeric() || matches!(character, '_' | '-'))
        {
            continue;
        }
        let Some(rest) = line[cursor..].trim_start().strip_prefix('=') else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('"') else {
            continue;
        };
        let Some(end) = rest.find('"') else {
            continue;
        };
        values.push(&rest[..end]);
    }
    values
}

fn resolves_outside_root(directory: &str, value: &str) -> bool {
    let normalized = value.replace('\\', "/");
    if normalized.starts_with('/') || normalized.chars().nth(1) == Some(':') {
        return true;
    }
    let mut stack = directory
        .split('/')
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>();
    for component in normalized.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if stack.pop().is_none() {
                    return true;
                }
            }
            other => stack.push(other),
        }
    }
    false
}

pub(crate) fn tracked_paths(root: &Path) -> Result<Vec<String>, String> {
    if root.join(".jj").exists() {
        let output = Command::new("jj")
            .args(["file", "list"])
            .current_dir(root)
            .output()
            .map_err(|error| format!("cannot list tracked files with jj: {error}"))?;
        if !output.status.success() {
            return Err("cannot list tracked files with jj".to_owned());
        }
        return Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_owned)
            .collect());
    }

    let mut files = Vec::new();
    collect_files(root, &mut files);
    Ok(files
        .iter()
        .filter_map(|path| path.strip_prefix(root).ok())
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect())
}

pub(crate) fn collect_files(root: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            if !matches!(name.to_str(), Some(".git" | ".jj" | "target")) {
                collect_files(&path, files);
            }
        } else if path.is_file() {
            files.push(path);
        }
    }
}

fn is_forbidden_tracked_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let basename = lower.rsplit('/').next().unwrap_or(&lower);
    let forbidden_component = lower
        .split('/')
        .any(|component| matches!(component, "saves" | "logs" | "crash-reports"));
    let forbidden_evidence_component = lower.starts_with("evidence/")
        && lower
            .split('/')
            .any(|component| matches!(component, "raw" | "captures"));
    let forbidden_basename = matches!(basename, ".env" | "credentials.json" | "secrets.json")
        || (basename.starts_with(".env.") && basename != ".env.example")
        || basename.starts_with("hs_err_pid");
    let forbidden_extension = [
        ".pem",
        ".p12",
        ".pfx",
        ".key",
        ".keystore",
        ".jfr",
        ".hprof",
        ".trace",
    ]
    .iter()
    .any(|extension| lower.ends_with(extension));

    forbidden_basename || forbidden_extension || forbidden_component || forbidden_evidence_component
}

fn has_credential_assignment(line: &str) -> bool {
    let Some((name, value)) = line.split_once('=') else {
        return false;
    };

    if !matches!(
        name.trim(),
        "ATP_APP_PASSWORD" | "AWS_SECRET_ACCESS_KEY" | "MINECRAFT_ACCESS_TOKEN" | "TANGLED_TOKEN"
    ) {
        return false;
    }

    let value = value.trim().trim_matches(['\'', '"']);
    !value.is_empty()
        && !value.contains('<')
        && !value.eq_ignore_ascii_case("example")
        && !value.eq_ignore_ascii_case("placeholder")
}

#[cfg(test)]
mod tests;
