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

const EXPERIMENT_NAMES: &[&str] = &["spike-harness"];

/// Gate directories under `evidence/m1/spikes` and the gate ID they must carry.
const SPIKE_GATES: &[(&str, &str)] = &[
    ("adg-1", "ADG-1"),
    ("adg-2", "ADG-2"),
    ("adg-3", "ADG-3"),
    ("adg-4", "ADG-4"),
    ("adg-5", "ADG-5"),
];

const ROOT_FILES: &[&str] = &[
    ".cargo/config.toml",
    ".config/nextest.toml",
    ".envrc",
    ".gitignore",
    ".markdownlint.yaml",
    ".tangled/workflows/ci.yml",
    "AGENTS.md",
    "CLAUDE.md",
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
    "experiments/README.md",
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
    "schemas/fixture.schema.json",
    "schemas/pack.schema.json",
    "schemas/package.schema.json",
    "schemas/performance-collector.schema.json",
    "schemas/performance-result.schema.json",
    "schemas/provenance.schema.json",
    "schemas/runtime-inputs.schema.json",
    "schemas/saved-data.schema.json",
    "schemas/spike-result.schema.json",
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
        .chain(
            EXPERIMENT_NAMES
                .iter()
                .map(|name| format!("experiments/{name}/Cargo.toml")),
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
    verify_experiment_fence(root, &mut violations);

    match tracked_paths(root) {
        Ok(paths) => {
            violations.extend(verify_tracked_paths(paths.iter().map(String::as_str)));
            violations.extend(verify_tracked_contents(root, &paths));
        }
        Err(error) => violations.push(error),
    }

    violations
}

/// Checks every recorded architecture spike result against the M1 gate rules.
///
/// An absent spike tree is not a violation; the M1 milestone gate is what
/// requires results to exist. This check only enforces that recorded results are
/// complete, comparable, and free of substituted values.
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

    verify_manifest_redaction(path, violations);
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

/// Keeps measurement-only spike crates out of the production dependency graph.
fn verify_experiment_fence(root: &Path, violations: &mut Vec<String>) {
    let production_manifests = CRATE_NAMES
        .iter()
        .map(|name| format!("crates/{name}/Cargo.toml"))
        .chain(
            TEST_MOD_NAMES
                .iter()
                .map(|name| format!("test-mods/{name}/Cargo.toml")),
        )
        .chain([
            "tests/conformance/Cargo.toml".to_owned(),
            "tools/ravel-cli/Cargo.toml".to_owned(),
            "tools/ravel-xtask/Cargo.toml".to_owned(),
        ]);

    for relative in production_manifests {
        let Ok(manifest) = fs::read_to_string(root.join(&relative)) else {
            continue;
        };
        if manifest.contains("ravel-spike") || manifest.contains("experiments/") {
            violations.push(format!(
                "production crate depends on an experiment: {relative}"
            ));
        }
        if manifest.contains(r#"unsafe_code = "allow""#) {
            violations.push(format!(
                "unsafe code is permitted outside the experiment boundary: {relative}"
            ));
        }
    }

    let mut sources = Vec::new();
    for directory in ["crates", "test-mods", "tools", "tests"] {
        collect_files(&root.join(directory), &mut sources);
    }
    for path in sources {
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        // A bare `contains` would match this rule's own literals, so anchor on
        // the attribute position instead of the substring.
        if fs::read_to_string(&path).is_ok_and(|source| {
            source.lines().map(str::trim_start).any(|line| {
                line.starts_with("#![allow(unsafe_code)")
                    || line.starts_with("#[allow(unsafe_code)")
                    || line.starts_with("#![expect(unsafe_code)")
                    || line.starts_with("#[expect(unsafe_code)")
            })
        }) {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            violations.push(format!(
                "unsafe code is permitted outside the experiment boundary: {}",
                relative.display()
            ));
        }
    }
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

    let mut violations = verify_evidence_manifest(root, &manifest_path, milestone);
    verify_admitted_manifest(&manifest_path, milestone, &mut violations);
    violations
}

/// Checks one evidence manifest against the schema and milestone closure rules.
#[must_use]
pub fn verify_evidence_manifest(root: &Path, manifest_path: &Path, milestone: &str) -> Vec<String> {
    if !is_valid_milestone_id(milestone) {
        return vec![format!("invalid milestone ID: {milestone}")];
    }

    match jq_matches(manifest_path, "type == \"object\"") {
        Ok(true) => {}
        Ok(false) => return vec!["milestone manifest must be a JSON object".to_owned()],
        Err(error) => return vec![error],
    }

    let mut violations = Vec::new();
    match evidence_schema_matches(root, manifest_path) {
        Ok(true) => {}
        Ok(false) => {
            violations.push("milestone manifest does not conform to evidence schema".to_owned());
        }
        Err(error) => violations.push(error),
    }
    verify_manifest_redaction(manifest_path, &mut violations);
    if !jq_matches(manifest_path, ".schemaVersion == 1").unwrap_or(false) {
        violations.push("manifest schemaVersion must equal 1".to_owned());
    }
    let milestone_filter = format!(".milestone == \"{milestone}\"");
    if !jq_matches(manifest_path, &milestone_filter).unwrap_or(false) {
        violations.push(format!("manifest milestone must equal {milestone}"));
    }

    verify_general_closure(manifest_path, milestone, &mut violations);
    if milestone == "M0" {
        verify_m0_manifest(manifest_path, &mut violations);
    }

    violations
}

fn verify_general_closure(manifest_path: &Path, milestone: &str, violations: &mut Vec<String>) {
    let work_package_prefix = format!("{milestone}-W");
    require_manifest_rule(
        manifest_path,
        &format!(".workPackages | length > 0 and all(startswith(\"{work_package_prefix}\"))"),
        "work-package IDs must belong to the manifest milestone",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".revision.dirty == false",
        "milestone revision must be clean",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".inputs | length > 0",
        "milestone must include at least one hashed input",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "([.commands[].exitStatus] + [.platformEvidence[].commands[].exitStatus]) | length > 0 and all(. == 0)",
        "milestone commands must all succeed",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".tests.total == (.tests.passed + .tests.failed + .tests.skipped)",
        "test counts must equal passed + failed + skipped",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".tests.total > 0 and .tests.passed > 0",
        "milestone must record at least one passing test",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".tests.failed == 0",
        "milestone tests must have zero failures",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".tests.skipped == 0",
        "milestone tests must have zero skipped tests",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".tests.lineCoveragePercent != null and .tests.lineCoveragePercent >= 80",
        "milestone line coverage must be at least 80%",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "([.deviations[].classification] + [.risks[].classification]) | all(. != \"blocking\")",
        "milestone must have no blocking deviations or risks",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".review.independent == true and .review.result == \"pass\"",
        "milestone review must be independent and pass",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".review | has(\"record\")",
        "milestone review must include a hashed record",
        violations,
    );
}

fn verify_admitted_manifest(manifest_path: &Path, milestone: &str, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        r#".evidenceKind == "native""#,
        "milestone closure requires native evidence, not a synthetic contract fixture",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#"[.. | strings | ascii_downcase | contains("synthetic")] | any | not"#,
        "native milestone evidence must not contain synthetic placeholders",
        violations,
    );
    if milestone == "M0" {
        require_manifest_rule(
            manifest_path,
            r#".attestations
                | any(
                    .platform == "linux"
                    and .architecture == "x86_64"
                    and (.workloads | sort == ["official-atm11", "vanilla"])
                )"#,
            "M0 closure requires a workload-bound native Linux attestation",
            violations,
        );
    }
}

fn verify_m0_manifest(manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M0-W1\", \"M0-W2\", \"M0-W3\", \"M0-W4\", \"M0-W5\", \"M0-W6\", \"M0-W7\", \"M0-W8\", \"M0-W9\"]",
        "M0 manifest must include every work package from M0-W1 through M0-W9 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".versions.minecraft == \"26.1.2\" and .versions.neoforge == \"26.1.2.78\" and .versions.fixture == \"ATM11 0.2.1\"",
        "M0 manifest must pin Minecraft 26.1.2, NeoForge 26.1.2.78, and ATM11 0.2.1",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        &m0_platform_filter("linux", "x86_64", "linux-reference-v1"),
        "M0 manifest must include native Linux x86_64 launch evidence",
        violations,
    );
}

fn m0_platform_filter(platform: &str, architecture: &str, hardware_profile: &str) -> String {
    format!(
        r#".platformEvidence | any(
            .platform == "{platform}"
            and .architecture == "{architecture}"
            and .native == true
            and .hardwareProfile == "{hardware_profile}"
            and .versions.minecraft == "26.1.2"
            and .versions.neoforge == "26.1.2.78"
            and .versions.prism == "11.0.3"
            and .versions.fixture == "ATM11 0.2.1"
            and .versions.java == "25.0.1"
            and .versions.rust == "1.95.0"
            and .versions.nix == "2.33.3"
            and .versions.devenv == "2.1.2"
            and (.settings | length > 0)
            and any(.commands[]; .command == "devenv test --no-tui")
            and any(.commands[]; .command == "nix flake check --accept-flake-config")
            and any(.baselines[]; .workload == "vanilla")
            and any(.baselines[]; .workload == "official-atm11")
        )"#
    )
}

fn verify_manifest_redaction(manifest_path: &Path, violations: &mut Vec<String>) {
    let output = Command::new("jq")
        .args(["--raw-output", ".. | strings"])
        .arg(manifest_path)
        .output();
    let lower = output
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).to_ascii_lowercase())
        .unwrap_or_default();
    let contains_local_data = [
        "/home/",
        "/users/",
        r"c:\\users\\",
        ".minecraft",
        "tests/fixtures/local/",
        "evidence/private/",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern));
    let contains_credential_value = [
        "authorization: bearer",
        "authorization= bearer",
        "access_token=",
        "accesstoken=",
        "minecraft_access_token=",
        "tangled_token=",
        "aws_secret_access_key=",
        "atp_app_password=",
        "ghp_",
        "xoxb-",
        "xoxp-",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern));
    let credential_fields_absent = jq_matches(
        manifest_path,
        r#"[paths(strings) as $path
            | select(
                any(
                    $path[];
                    type == "string"
                    and (ascii_downcase | test("token|password|secret|credential|authorization"))
                )
                and (getpath($path) | length > 0)
            )] | length == 0"#,
    )
    .unwrap_or(false);

    if contains_local_data || contains_credential_value || !credential_fields_absent {
        violations.push("milestone manifest contains sensitive or local data".to_owned());
    }
}

fn require_manifest_rule(
    manifest_path: &Path,
    filter: &str,
    violation: &str,
    violations: &mut Vec<String>,
) {
    match jq_matches(manifest_path, filter) {
        Ok(true) => {}
        Ok(false) => violations.push(violation.to_owned()),
        Err(error) => violations.push(error),
    }
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
            "tools/ravel-cli/src/lib.rs".to_owned(),
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
        "fixture",
        "pack",
        "package",
        "performance-collector",
        "performance-result",
        "provenance",
        "runtime-inputs",
        "saved-data",
        "spike-result",
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
    let forbidden_evidence_component = lower.starts_with("evidence/")
        && components
            .iter()
            .any(|component| matches!(*component, "raw" | "captures"));
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

    forbidden_prefix
        || forbidden_component
        || forbidden_evidence_component
        || forbidden_basename
        || forbidden_extension
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
        required_scaffold_paths, verify_experiment_fence, verify_ignored_boundaries,
        verify_milestone, verify_scaffold, verify_spikes, verify_tracked_contents,
        verify_tracked_paths, verify_version_boundary,
    };

    #[test]
    fn an_absent_spike_tree_is_not_a_violation() {
        let root = temporary_root("spikes-absent");

        assert!(verify_spikes(&root).is_empty());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn spike_results_must_cover_every_failure_scenario() {
        let root = temporary_root("spikes-failures");
        let gate = root.join("evidence/m1/spikes/adg-1");
        fs::create_dir_all(&gate).expect("spike directory should be creatable");
        fs::write(
            gate.join("candidate.json"),
            r#"{"schemaVersion":1,"protocolVersion":"spike-v1","gate":"ADG-1",
                "candidate":{"disposition":"measured"},
                "failures":[{"scenario":"crash"}],
                "memory":{"bootstrapPeakRss":{"status":"measured","value":1},
                    "retainedRss":{"status":"measured","value":1},
                    "retainedClasses":{"status":"measured","value":1}},
                "startup":{"processToReady":{"status":"measured","value":1}},
                "calls":{"callbackNanoseconds":{"status":"measured"}},
                "trial":{"measuredInvocations":1},"deviations":[]}"#,
        )
        .expect("spike result should be writable");

        let violations = verify_spikes(&root);

        assert!(violations.iter().any(|violation| {
            violation.ends_with("must exercise every required failure scenario exactly once")
        }));
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn spike_results_must_not_substitute_zero_for_a_missing_measurement() {
        let root = temporary_root("spikes-zero");
        let gate = root.join("evidence/m1/spikes/adg-1");
        fs::create_dir_all(&gate).expect("spike directory should be creatable");
        fs::write(
            gate.join("candidate.json"),
            r#"{"schemaVersion":1,"protocolVersion":"spike-v1","gate":"ADG-1",
                "candidate":{"disposition":"measured"},
                "failures":[{"scenario":"callback-failure"},{"scenario":"crash"},
                    {"scenario":"invalid-input"},{"scenario":"timeout"},
                    {"scenario":"unavailable-runtime"}],
                "memory":{"bootstrapPeakRss":{"status":"measured","value":0},
                    "retainedRss":{"status":"measured","value":1},
                    "retainedClasses":{"status":"measured","value":1}},
                "startup":{"processToReady":{"status":"measured","value":1}},
                "calls":{"callbackNanoseconds":{"status":"measured"}},
                "trial":{"measuredInvocations":1},"deviations":[]}"#,
        )
        .expect("spike result should be writable");

        let violations = verify_spikes(&root);

        assert!(violations.iter().any(|violation| {
            violation.ends_with("must not report a measured bootstrap or startup value of zero")
        }));
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn production_crates_cannot_depend_on_measurement_experiments() {
        let root = temporary_root("fence");
        let crate_root = root.join("crates/ravel-loader");
        fs::create_dir_all(&crate_root).expect("crate directory should be creatable");
        fs::write(
            crate_root.join("Cargo.toml"),
            "[dependencies]\nravel-spike-harness.workspace = true\n",
        )
        .expect("manifest should be writable");
        let mut violations = Vec::new();

        verify_experiment_fence(&root, &mut violations);

        assert_eq!(
            violations,
            ["production crate depends on an experiment: crates/ravel-loader/Cargo.toml"]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn unsafe_code_cannot_be_permitted_outside_experiments() {
        let root = temporary_root("fence-unsafe");
        let crate_root = root.join("crates/ravel-bootstrap/src");
        fs::create_dir_all(&crate_root).expect("crate directory should be creatable");
        fs::write(
            crate_root.join("lib.rs"),
            "#![allow(unsafe_code)]\npub fn boundary() {}\n",
        )
        .expect("source should be writable");
        let mut violations = Vec::new();

        verify_experiment_fence(&root, &mut violations);

        assert_eq!(
            violations,
            ["unsafe code is permitted outside the experiment boundary: \
                 crates/ravel-bootstrap/src/lib.rs"]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

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
            "evidence/m0/raw/process.log",
            "evidence/m0/captures/frame.trace",
            "config/credentials.json",
            "docs/README.md",
        ]);

        assert_eq!(violations.len(), 5);
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
