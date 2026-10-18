//! Tracked-path, dependency-path, and redaction checks.

use std::fs;
use std::path::Path;

use ravel_core::CREDENTIAL_WORDS;

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
        let read = if path.is_symlink() && path.file_name().is_none_or(|name| name != "Cargo.toml")
        {
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
        let credential_value = ravel_core::contains_credential(&source);
        if private_key || credential || credential_value {
            violations.push(format!(
                "tracked file contains credential material: {relative}"
            ));
        }

        if relative.rsplit('/').next() == Some("Cargo.toml") {
            match escaping_path_dependencies(relative, &source) {
                Ok(paths) => violations.extend(paths.into_iter().map(|value| {
                    format!(
                        "tracked manifest depends on a path outside this repository: {relative} -> {value}"
                    )
                })),
                Err(error) => violations.push(error),
            }
        }
    }
    violations
}

fn escaping_path_dependencies(
    relative_manifest: &str,
    source: &str,
) -> Result<Vec<String>, String> {
    let manifest = toml::from_str::<toml::Value>(source).map_err(|error| {
        let context = error.span().map_or_else(String::new, |span| {
            let prefix = &source[..span.start];
            let line = prefix.split('\n').count();
            let column = prefix
                .rfind('\n')
                .map_or(prefix.len() + 1, |index| prefix.len() - index);
            format!(" at line {line}, byte column {column}")
        });
        format!("cannot parse tracked manifest {relative_manifest}: invalid TOML{context}")
    })?;
    let directory = relative_manifest
        .rsplit_once('/')
        .map_or("", |(head, _)| head);
    let mut paths = Vec::new();
    collect_escaping_paths(&manifest, directory, &mut paths);
    Ok(paths)
}

fn collect_escaping_paths(value: &toml::Value, directory: &str, paths: &mut Vec<String>) {
    match value {
        toml::Value::Table(table) => {
            for (key, value) in table {
                if key == "path"
                    && let Some(path) = value.as_str()
                    && resolves_outside_root(directory, path)
                {
                    paths.push(path.to_owned());
                }
                collect_escaping_paths(value, directory, paths);
            }
        }
        toml::Value::Array(values) => {
            for value in values {
                collect_escaping_paths(value, directory, paths);
            }
        }
        toml::Value::String(_)
        | toml::Value::Integer(_)
        | toml::Value::Float(_)
        | toml::Value::Boolean(_)
        | toml::Value::Datetime(_) => {}
    }
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

pub(crate) fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<std::path::PathBuf>,
) -> Result<(), String> {
    let entries = fs::read_dir(directory).map_err(|error| {
        format!(
            "cannot read {}: {error}",
            crate::text::redacted_path(root, directory)
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "cannot read {} entry: {error}",
                crate::text::redacted_path(root, directory)
            )
        })?;
        let path = entry.path();
        if path.ends_with(Path::new("tests/fixtures/local")) {
            continue;
        }
        let kind = fs::metadata(&path)
            .map_err(|error| {
                format!(
                    "cannot inspect {}: {error}",
                    crate::text::redacted_path(root, &path)
                )
            })?
            .file_type();
        if kind.is_dir() {
            let name = entry.file_name();
            if !matches!(name.to_str(), Some(".git" | ".jj" | "target")) {
                collect_files(root, &path, files)?;
            }
        } else if kind.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn is_forbidden_tracked_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let basename = lower.rsplit('/').next().unwrap_or(&lower);
    let forbidden_component = lower.split('/').any(|component| match component {
        "saves" => !is_cargo_rust_source(&lower),
        "logs" | "crash-reports" => true,
        _ => false,
    });
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

fn is_cargo_rust_source(path: &str) -> bool {
    let mut components = path.split('/');
    matches!(
        components.next(),
        Some("crates" | "tools" | "examples" | "test-mods")
    ) && components
        .next()
        .is_some_and(|package| !matches!(package, "" | "." | ".."))
        && components.next() == Some("src")
        && components.all(|component| !matches!(component, "" | "." | ".."))
        && Path::new(path)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
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
