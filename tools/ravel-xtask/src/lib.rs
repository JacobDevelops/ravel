#![forbid(unsafe_code)]

//! Repository automation for Ravel.

use std::fs;
use std::path::Path;
use std::process::Command;

const CRATE_NAMES: &[&str] = &[
    "ravel-core",
    "ravel-package",
    "ravel-resolve",
    "ravel-loader",
    "ravel-sdk",
    "ravel-sdk-macros",
    "ravel-host-api",
    "ravel-bootstrap",
    "ravel-minecraft",
    "ravel-minecraft-adapter",
    "ravel-config",
    "ravel-data",
    "ravel-save",
    "ravel-network",
    "ravel-observability",
    "ravel-pack",
    "ravel-testkit",
];

const TEST_MOD_NAMES: &[&str] = &[
    "commands",
    "configuration",
    "dependency-base",
    "dependency-consumer",
    "events",
    "failure-cases",
    "inter-mod-service",
    "networking",
    "noop",
    "registries",
    "resources",
    "saved-data",
];

const ROOT_FILES: &[&str] = &[
    ".cargo/config.toml",
    ".config/nextest.toml",
    ".envrc",
    ".gitignore",
    ".markdownlint.yaml",
    ".tangled/workflows/ci.yml",
    "Cargo.lock",
    "Cargo.toml",
    "LICENSE-APACHE",
    "LICENSE-MIT",
    "README.md",
    "ROADMAP.md",
    "SPEC.md",
    "deny.toml",
    "devenv.lock",
    "devenv.nix",
    "devenv.yaml",
    "docs/runbooks/darwin-verification.md",
    "flake.lock",
    "flake.nix",
    "minecraft/README.md",
    "minecraft/versions/26.1.2/README.md",
    "nix/ci.nix",
    "nix/common.nix",
    "nix/darwin.nix",
    "nix/linux.nix",
    "rust-toolchain.toml",
    "rustfmt.toml",
    "schemas/configuration.schema.json",
    "schemas/evidence.schema.json",
    "schemas/pack.schema.json",
    "schemas/package.schema.json",
    "schemas/provenance.schema.json",
    "schemas/saved-data.schema.json",
    "tests/conformance/Cargo.toml",
    "tests/fixtures/README.md",
    "tests/fixtures/local/README.md",
    "tests/fixtures/public/README.md",
    "tools/ravel-cli/Cargo.toml",
    "tools/ravel-xtask/Cargo.toml",
];

/// Returns the files that define the minimum public repository scaffold.
#[must_use]
pub fn required_scaffold_paths() -> Vec<String> {
    ROOT_FILES
        .iter()
        .map(|path| (*path).to_owned())
        .chain(
            CRATE_NAMES
                .iter()
                .map(|name| format!("crates/{name}/Cargo.toml")),
        )
        .chain(
            TEST_MOD_NAMES
                .iter()
                .map(|name| format!("test-mods/{name}/Cargo.toml")),
        )
        .collect()
}

/// Checks the repository's initial public scaffold and tracked-file policy.
#[must_use]
pub fn verify_scaffold(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();

    for relative in required_scaffold_paths() {
        if !root.join(&relative).is_file() {
            violations.push(format!("missing required file: {relative}"));
        }
    }

    for relative in maintained_rust_roots() {
        match fs::read_to_string(root.join(&relative)) {
            Ok(source) if source.contains("#![forbid(unsafe_code)]") => {}
            Ok(_) => violations.push(format!("missing unsafe prohibition: {relative}")),
            Err(error) => violations.push(format!("cannot read {relative}: {error}")),
        }
    }

    verify_ignored_boundaries(root, &mut violations);
    verify_schema_documents(root, &mut violations);
    verify_version_boundary(root, &mut violations);

    match tracked_paths(root) {
        Ok(paths) => {
            violations.extend(verify_tracked_paths(paths.iter().map(String::as_str)));
            violations.extend(verify_tracked_contents(root, &paths));
        }
        Err(error) => violations.push(error),
    }

    violations
}

/// Checks a milestone evidence manifest's identity and minimum gate fields.
#[must_use]
pub fn verify_milestone(root: &Path, milestone: &str) -> Vec<String> {
    if !is_valid_milestone_id(milestone) {
        return vec![format!("invalid milestone ID: {milestone}")];
    }

    let manifest_path = root
        .join("evidence")
        .join(milestone.to_ascii_lowercase())
        .join("manifest.json");
    if let Err(error) = fs::metadata(&manifest_path) {
        return vec![format!("cannot read {}: {error}", manifest_path.display())];
    }
    match jq_matches(&manifest_path, "type == \"object\"") {
        Ok(true) => {}
        Ok(false) => return vec!["milestone manifest must be a JSON object".to_owned()],
        Err(error) => return vec![error],
    }

    let mut violations = Vec::new();
    match evidence_schema_matches(root, &manifest_path) {
        Ok(true) => {}
        Ok(false) => {
            violations.push("milestone manifest does not conform to evidence schema".to_owned());
        }
        Err(error) => violations.push(error),
    }
    if !jq_matches(&manifest_path, ".schemaVersion == 1").unwrap_or(false) {
        violations.push("manifest schemaVersion must equal 1".to_owned());
    }
    let milestone_filter = format!(".milestone == \"{milestone}\"");
    if !jq_matches(&manifest_path, &milestone_filter).unwrap_or(false) {
        violations.push(format!("manifest milestone must equal {milestone}"));
    }

    for field in [
        "workPackages",
        "revision",
        "versions",
        "environment",
        "inputs",
        "commands",
        "tests",
        "benchmarks",
        "deviations",
        "risks",
        "review",
        "attestations",
    ] {
        let filter = format!("has(\"{field}\")");
        if !jq_matches(&manifest_path, &filter).unwrap_or(false) {
            violations.push(format!("milestone manifest is missing field: {field}"));
        }
    }

    violations
}

/// Rejects rights-restricted, secret, and local-runtime paths from a tracked tree.
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

/// Rejects private keys and non-placeholder credential assignments in tracked text.
#[must_use]
pub fn verify_tracked_contents(root: &Path, paths: &[String]) -> Vec<String> {
    let mut violations = Vec::new();
    for relative in paths {
        let Ok(source) = fs::read_to_string(root.join(relative)) else {
            continue;
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
    }
    violations
}

fn maintained_rust_roots() -> Vec<String> {
    CRATE_NAMES
        .iter()
        .map(|name| format!("crates/{name}/src/lib.rs"))
        .chain([
            "tests/conformance/src/lib.rs".to_owned(),
            "tools/ravel-cli/src/main.rs".to_owned(),
            "tools/ravel-xtask/src/lib.rs".to_owned(),
            "tools/ravel-xtask/src/main.rs".to_owned(),
        ])
        .chain(
            TEST_MOD_NAMES
                .iter()
                .map(|name| format!("test-mods/{name}/src/lib.rs")),
        )
        .collect()
}

fn verify_ignored_boundaries(root: &Path, violations: &mut Vec<String>) {
    let gitignore_path = root.join(".gitignore");
    let Ok(gitignore) = fs::read_to_string(&gitignore_path) else {
        violations.push("cannot read .gitignore".to_owned());
        return;
    };

    for pattern in ["/tests/fixtures/local/**", "/evidence/private/**", ".env"] {
        if !gitignore.lines().any(|line| line.trim() == pattern) {
            violations.push(format!("missing restricted-content ignore: {pattern}"));
        }
    }
}

fn verify_schema_documents(root: &Path, violations: &mut Vec<String>) {
    for schema in [
        "configuration",
        "evidence",
        "pack",
        "package",
        "provenance",
        "saved-data",
    ] {
        let relative = format!("schemas/{schema}.schema.json");
        let path = root.join(&relative);
        match jq_matches(
            &path,
            "type == \"object\" and .\"$schema\" == \"https://json-schema.org/draft/2020-12/schema\"",
        ) {
            Ok(true) => {}
            Ok(false) => violations.push(format!("invalid JSON schema document: {relative}")),
            Err(error) => violations.push(error),
        }
    }
}

fn verify_version_boundary(root: &Path, violations: &mut Vec<String>) {
    let crates = root.join("crates");
    if let Ok(entries) = fs::read_dir(&crates) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(suffix) = name.strip_prefix("ravel-minecraft-")
                && suffix != "adapter"
            {
                violations.push(format!("unexpected per-version Minecraft crate: {name}"));
            }
        }
    }

    let versions = fs::read_dir(root.join("minecraft/versions"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    let mut files = Vec::new();
    collect_files(&root.join("crates"), &mut files);
    for path in files {
        if !matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs" | "toml")
        ) {
            continue;
        }
        if fs::read_to_string(&path)
            .is_ok_and(|source| versions.iter().any(|version| source.contains(version)))
        {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            violations.push(format!(
                "hardcoded Minecraft version outside version data: {}",
                relative.display()
            ));
        }
    }
}

fn tracked_paths(root: &Path) -> Result<Vec<String>, String> {
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

fn collect_files(root: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            if !matches!(
                name.to_str(),
                Some(".git" | ".jj" | ".devenv" | ".direnv" | "target")
            ) {
                collect_files(&path, files);
            }
        } else if path.is_file() {
            files.push(path);
        }
    }
}

fn is_forbidden_tracked_path(path: &str) -> bool {
    if path == "tests/fixtures/local/README.md" {
        return false;
    }

    let lower = path.to_ascii_lowercase();
    let basename = lower.rsplit('/').next().unwrap_or(&lower);
    let components = lower.split('/').collect::<Vec<_>>();
    let forbidden_prefix = [
        "tests/fixtures/local/",
        "evidence/private/",
        "private/",
        "restricted/",
        ".minecraft/",
        "prism-instances/",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix));
    let forbidden_component = components
        .iter()
        .any(|component| matches!(*component, "saves" | "logs" | "crash-reports"));
    let forbidden_basename = matches!(
        basename,
        ".env"
            | "credentials.json"
            | "secrets.json"
            | "fabric.mod.json"
            | "quilt.mod.json"
            | "neoforge.mods.toml"
            | "mods.toml"
    ) || (basename.starts_with(".env.") && basename != ".env.example")
        || basename.starts_with("hs_err_pid");
    let forbidden_extension = [
        ".jar",
        ".mrpack",
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

    forbidden_prefix || forbidden_component || forbidden_basename || forbidden_extension
}

fn is_valid_milestone_id(milestone: &str) -> bool {
    matches!(
        milestone,
        "M0" | "M1"
            | "M2"
            | "M3"
            | "M4"
            | "M5"
            | "M6"
            | "M7"
            | "M8"
            | "M9"
            | "M10"
            | "M11"
            | "M12"
            | "M13"
    )
}

fn jq_matches(path: &Path, filter: &str) -> Result<bool, String> {
    let output = Command::new("jq")
        .args(["--exit-status", filter])
        .arg(path)
        .output()
        .map_err(|error| format!("cannot validate JSON with jq: {error}"))?;
    Ok(output.status.success())
}

fn evidence_schema_matches(root: &Path, manifest: &Path) -> Result<bool, String> {
    let output = Command::new("check-jsonschema")
        .arg("--schemafile")
        .arg(root.join("schemas/evidence.schema.json"))
        .arg(manifest)
        .output()
        .map_err(|error| format!("cannot validate evidence schema: {error}"))?;
    Ok(output.status.success())
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
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{
        required_scaffold_paths, verify_ignored_boundaries, verify_milestone, verify_scaffold,
        verify_tracked_contents, verify_tracked_paths, verify_version_boundary,
    };

    #[test]
    fn missing_files_are_reported_by_relative_path() {
        let root = temporary_root("missing");

        let violations = verify_scaffold(&root);

        assert!(
            violations
                .iter()
                .any(|violation| violation == "missing required file: Cargo.toml")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn every_required_path_is_unique() {
        let mut paths = required_scaffold_paths();
        let original_len = paths.len();
        paths.sort_unstable();
        paths.dedup();

        assert_eq!(paths.len(), original_len);
    }

    #[test]
    fn restricted_content_ignores_are_required() {
        let root = temporary_root("ignores");
        fs::write(root.join(".gitignore"), "# intentionally incomplete\n")
            .expect("test gitignore should be writable");
        let mut violations = Vec::new();

        verify_ignored_boundaries(&root, &mut violations);

        assert_eq!(violations.len(), 3);
        assert!(
            violations
                .iter()
                .all(|violation| violation.starts_with("missing restricted-content ignore:"))
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn restricted_tracked_paths_are_rejected() {
        let violations = verify_tracked_paths([
            "tests/fixtures/local/secret.jar",
            "evidence/private/capture.json",
            "config/credentials.json",
            "docs/README.md",
        ]);

        assert_eq!(violations.len(), 3);
        assert!(
            violations
                .iter()
                .all(|violation| violation.starts_with("forbidden tracked path:"))
        );
    }

    #[test]
    fn tracked_credential_material_is_rejected() {
        let root = temporary_root("credentials");
        fs::write(
            root.join("notes.txt"),
            format!("{}{}\n", "-----BEGIN OPENSSH ", "PRIVATE KEY-----"),
        )
        .expect("credential canary should be writable");

        let violations = verify_tracked_contents(&root, &["notes.txt".to_owned()]);

        assert_eq!(
            violations,
            ["tracked file contains credential material: notes.txt"]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn milestone_manifest_must_match_requested_milestone() {
        let root = temporary_root("milestone");
        let evidence = root.join("evidence/m0");
        fs::create_dir_all(&evidence).expect("evidence directory should be creatable");
        fs::write(
            evidence.join("manifest.json"),
            r#"{"schemaVersion":1,"milestone":"M1"}"#,
        )
        .expect("manifest should be writable");

        let violations = verify_milestone(&root, "M0");

        assert!(
            violations
                .iter()
                .any(|violation| violation == "manifest milestone must equal M0")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn malformed_milestone_ids_are_rejected() {
        let root = temporary_root("milestone-id");

        let violations = verify_milestone(&root, "M14");

        assert_eq!(violations, ["invalid milestone ID: M14"]);
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn version_specific_crates_and_production_literals_are_rejected() {
        let root = temporary_root("versions");
        fs::create_dir_all(root.join("minecraft/versions/test-version"))
            .expect("version directory should be creatable");
        fs::create_dir_all(root.join("crates/ravel-minecraft-test-version/src"))
            .expect("crate directory should be creatable");
        fs::write(
            root.join("crates/ravel-minecraft-test-version/src/lib.rs"),
            "const VERSION: &str = \"test-version\";\n",
        )
        .expect("test source should be writable");
        let mut violations = Vec::new();

        verify_version_boundary(&root, &mut violations);

        assert_eq!(violations.len(), 2);
        assert!(
            violations
                .iter()
                .any(|violation| violation.starts_with("unexpected per-version Minecraft crate:"))
        );
        assert!(violations.iter().any(|violation| {
            violation.starts_with("hardcoded Minecraft version outside version data:")
        }));
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    fn temporary_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("ravel-xtask-{label}-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale temporary root should be removable");
        }
        fs::create_dir_all(&root).expect("temporary root should be creatable");
        root
    }
}
