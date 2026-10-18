//! Tracked-path, dependency-path, and redaction checks.

use std::fs;
use std::path::Path;
use std::process::Command;

use ravel_observability::redaction::CREDENTIAL_WORDS;

/// Rejects secret and runtime paths from a tracked tree.
#[must_use]
pub fn verify_tracked_paths<'a>(paths: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    paths
        .into_iter()
        .filter_map(|path| {
            let normalized = path.replace('\\', "/");
            is_forbidden_tracked_path(&normalized)
                .then(|| format!("forbidden tracked path: {normalized}"))
        })
        .collect()
}

/// Rejects private keys, credential assignments and credential-shaped values in tracked text.
/// Home-directory paths and public upstream identifiers are deliberately not refused.
#[must_use]
pub fn verify_tracked_contents(root: &Path, paths: &[String]) -> Vec<String> {
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
        // Rust sources contain deliberate redaction test vectors; assignment and key checks still apply.
        let rust_source = Path::new(relative)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"));
        let credential_value = !rust_source && ravel_observability::contains_credential(&source);
        if private_key || credential || credential_value {
            violations.push(format!(
                "tracked file contains credential material: {relative}"
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

/// Lists version-controlled files, preferring jj and otherwise using Git's index.
/// Trees without repository metadata are refused rather than treated as fully tracked.
pub(crate) fn tracked_paths(root: &Path) -> Result<Vec<String>, String> {
    if root.join(".jj").exists() {
        let output = Command::new("jj")
            .args(["file", "list", "-T", "path ++ \"\\0\""])
            .current_dir(root)
            .output()
            .map_err(|error| format!("cannot list tracked files with jj: {error}"))?;
        if !output.status.success() {
            return Err("cannot list tracked files with jj".to_owned());
        }
        return Ok(parse_tracked_paths(&output.stdout));
    }

    if !root.join(".git").exists() {
        return Err("cannot list tracked files: repository has neither .jj nor .git".to_owned());
    }
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot list tracked files with git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cannot list tracked files with git: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(parse_tracked_paths(&output.stdout))
}

fn parse_tracked_paths(output: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(output)
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect()
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

    let name = name.trim();
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        || !CREDENTIAL_WORDS
            .iter()
            .any(|word| name.split('_').any(|part| part.eq_ignore_ascii_case(word)))
    {
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
