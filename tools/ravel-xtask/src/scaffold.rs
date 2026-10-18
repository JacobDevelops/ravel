//! Workspace layout and crate fences.

use std::fs;
use std::path::Path;

use crate::evidence::accounting::{
    verify_manifest_percentages, verify_review_blocking_counts, verify_review_round_counts,
};
use crate::evidence::manifest::verify_evidence_records;
use crate::inventory::{
    verify_example_inventory, verify_game_type_claims, verify_guide_enumerations,
    verify_hook_inventory_document, verify_inventory, verify_test_mod_inventory,
};
use crate::java_shape::reflection::verify_persistence_reflection;
use crate::java_shape::{
    verify_hook_name_literals, verify_menu_row_placement, verify_screen_overrides,
    verify_screen_templates, verify_transformer_shape,
};
use crate::json::read_json;
use crate::text::{read_text, relative_label};
use crate::tracked::{collect_files, tracked_paths, verify_tracked_contents, verify_tracked_paths};
use crate::version_boundary::{
    verify_version_boundary, verify_version_directories, verify_version_readmes,
};
use crate::{CRATE_NAMES, EXAMPLE_NAMES, REGISTRY_MOD_NAMES, TEST_MOD_NAMES};

const ROOT_FILES: &[&str] = &[
    ".cargo/config.toml",
    ".config/nextest.toml",
    ".github/workflows/ci.yml",
    ".gitignore",
    ".markdownlint.yaml",
    "AGENTS.md",
    "CLAUDE.md",
    "Cargo.lock",
    "Cargo.toml",
    "LICENSE-APACHE",
    "LICENSE-MIT",
    "Makefile.toml",
    "README.md",
    "ROADMAP.md",
    "SPEC.md",
    "deny.toml",
    "docs/architecture/m1-measurement-gate.md",
    "evidence/m1/README.md",
    "evidence/m2/README.md",
    "docs/runbooks/darwin-verification.md",
    "jvm/README.md",
    "jvm/bootstrap/dev/ravel/jvm/Agent.java",
    "jvm/bootstrap/dev/ravel/jvm/HookTarget.java",
    "jvm/bootstrap/dev/ravel/jvm/Screens.java",
    "jvm/bootstrap/dev/ravel/jvm/Transformer.java",
    "jvm/build/build-agent.sh",
    "jvm/build/build-screen.sh",
    "jvm/harness/net/minecraft/util/StringUtil.java",
    "jvm/harness/remap/Remap.java",
    "jvm/harness/run-harness.sh",
    "jvm/harness/template/driver/Main.java.in",
    "jvm/harness/template/game/Screen.java.in",
    "jvm/screen/template/RavelModsScreen.java.in",
    "jvm/screen/template/RavelSettingsScreen.java.in",
    "jvm/screen/template/Wire.java.in",
    "minecraft/README.md",
    "mise.toml",
    "rust-toolchain.toml",
    "rustfmt.toml",
    "schemas/configuration-override.schema.json",
    "schemas/configuration.schema.json",
    "schemas/evidence.schema.json",
    "schemas/fixture.schema.json",
    "schemas/pack.schema.json",
    "schemas/package.schema.json",
    "schemas/provenance.schema.json",
    "schemas/runtime-inputs.schema.json",
    "schemas/saved-data.schema.json",
    "schemas/spike-result.schema.json",
    "tests/conformance/Cargo.toml",
    "tests/fixtures/README.md",
    "tests/fixtures/local/README.md",
    "tests/fixtures/public/README.md",
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
        .chain(REGISTRY_MOD_NAMES.iter().flat_map(|name| {
            [
                format!("registry/mods/{name}/Cargo.toml"),
                format!("registry/mods/{name}/src/lib.rs"),
            ]
        }))
        .chain(
            TEST_MOD_NAMES
                .iter()
                .map(|name| format!("test-mods/{name}/Cargo.toml")),
        )
        .chain(
            EXAMPLE_NAMES
                .iter()
                .map(|name| format!("examples/{name}/Cargo.toml")),
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

    for relative in maintained_rust_roots(root, &mut violations) {
        match read_text(root, Path::new(&relative)) {
            Ok(source) if declares_unsafe_prohibition(&source) => {}
            Ok(_) => violations.push(format!("missing unsafe prohibition: {relative}")),
            Err(error) => violations.push(error),
        }
    }

    verify_ignored_boundaries(root, &mut violations);
    let (eligibility, inventories) = verify_version_directories(root, &mut violations);
    verify_hook_inventory_document(root, &inventories, &mut violations);
    verify_review_round_counts(root, &mut violations);
    verify_review_blocking_counts(root, &mut violations);
    verify_manifest_percentages(root, &mut violations);
    verify_persistence_reflection(root, &eligibility, &mut violations);
    verify_hook_name_literals(root, &inventories, &eligibility, &mut violations);
    verify_transformer_shape(root, &inventories, &eligibility, &mut violations);
    verify_menu_row_placement(root, &mut violations);
    verify_screen_templates(root, &mut violations);
    verify_screen_overrides(root, &mut violations);
    verify_version_readmes(root, &inventories, &mut violations);
    verify_guide_enumerations(root, &mut violations);
    verify_schema_documents(root, &mut violations);
    verify_version_boundary(root, &inventories, &mut violations);
    verify_example_inventory(root, &mut violations);
    verify_test_mod_inventory(root, &mut violations);
    verify_inventory(root, "crates", CRATE_NAMES, "CRATE_NAMES", &mut violations);
    verify_inventory(
        root,
        "registry/mods",
        REGISTRY_MOD_NAMES,
        "REGISTRY_MOD_NAMES",
        &mut violations,
    );
    verify_production_fences(root, &mut violations);
    violations.extend(verify_evidence_records(root));

    match tracked_paths(root) {
        Ok(paths) => {
            violations.extend(verify_tracked_paths(paths.iter().map(String::as_str)));
            violations.extend(verify_tracked_contents(root, &paths));
            verify_game_type_claims(root, &paths, &mut violations);
        }
        Err(error) => violations.push(error),
    }

    violations
}

fn declares_unsafe_prohibition(source: &str) -> bool {
    source
        .lines()
        .map(str::trim_start)
        .any(|line| line.starts_with("#![forbid(unsafe_code)]"))
}

fn verify_production_fences(root: &Path, violations: &mut Vec<String>) {
    let production_manifests = CRATE_NAMES
        .iter()
        .map(|name| format!("crates/{name}/Cargo.toml"))
        .chain(
            REGISTRY_MOD_NAMES
                .iter()
                .map(|name| format!("registry/mods/{name}/Cargo.toml")),
        )
        .chain(
            TEST_MOD_NAMES
                .iter()
                .map(|name| format!("test-mods/{name}/Cargo.toml")),
        )
        .chain(
            EXAMPLE_NAMES
                .iter()
                .map(|name| format!("examples/{name}/Cargo.toml")),
        )
        .chain([
            "tests/conformance/Cargo.toml".to_owned(),
            "tools/ravel-xtask/Cargo.toml".to_owned(),
        ]);

    for relative in production_manifests {
        let manifest = match read_text(root, Path::new(&relative)) {
            Ok(manifest) => manifest,
            Err(error) => {
                violations.push(error);
                continue;
            }
        };
        if manifest.contains(r#"unsafe_code = "allow""#) {
            violations.push(format!(
                "unsafe code is permitted in a maintained crate: {relative}"
            ));
        }
    }

    verify_testkit_fence(root, violations);

    let mut sources = Vec::new();
    for directory in ["crates", "registry/mods", "test-mods", "tools", "tests"] {
        if let Err(error) = collect_files(&root.join(directory), &mut sources) {
            violations.push(error);
        }
    }
    for path in sources {
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }

        let source = match read_text(root, &path) {
            Ok(source) => source,
            Err(error) => {
                violations.push(error);
                continue;
            }
        };
        if source.lines().map(str::trim_start).any(|line| {
            line.starts_with("#![allow(unsafe_code)")
                || line.starts_with("#[allow(unsafe_code)")
                || line.starts_with("#![expect(unsafe_code)")
                || line.starts_with("#[expect(unsafe_code)")
        }) {
            let relative = relative_label(root, &path);
            violations.push(format!(
                "unsafe code is permitted in a maintained crate: {relative}"
            ));
        }
    }
}

fn verify_testkit_fence(root: &Path, violations: &mut Vec<String>) {
    let fenced = CRATE_NAMES
        .iter()
        .filter(|name| **name != "ravel-testkit")
        .map(|name| format!("crates/{name}/Cargo.toml"))
        .chain(
            REGISTRY_MOD_NAMES
                .iter()
                .map(|name| format!("registry/mods/{name}/Cargo.toml")),
        )
        .chain(
            TEST_MOD_NAMES
                .iter()
                .map(|name| format!("test-mods/{name}/Cargo.toml")),
        )
        .chain(
            EXAMPLE_NAMES
                .iter()
                .map(|name| format!("examples/{name}/Cargo.toml")),
        );

    for relative in fenced {
        let manifest = match read_text(root, Path::new(&relative)) {
            Ok(manifest) => manifest,
            Err(error) => {
                violations.push(error);
                continue;
            }
        };
        if manifest.contains("ravel-testkit") {
            violations.push(format!(
                "production crate depends on the test harness: {relative}"
            ));
        }
    }
}

fn maintained_rust_roots(root: &Path, violations: &mut Vec<String>) -> Vec<String> {
    CRATE_NAMES
        .iter()
        .map(|name| format!("crates/{name}/src/lib.rs"))
        .chain(
            REGISTRY_MOD_NAMES
                .iter()
                .map(|name| format!("registry/mods/{name}/src/lib.rs")),
        )
        .chain([
            "tests/conformance/src/lib.rs".to_owned(),
            "tools/ravel-xtask/src/lib.rs".to_owned(),
            "tools/ravel-xtask/src/main.rs".to_owned(),
        ])
        .chain(
            TEST_MOD_NAMES
                .iter()
                .map(|name| format!("test-mods/{name}/src/lib.rs")),
        )
        .chain(
            EXAMPLE_NAMES
                .iter()
                .map(|name| format!("examples/{name}/src/lib.rs")),
        )
        .chain(crate_example_roots(root, violations))
        .collect()
}

fn crate_example_roots(root: &Path, violations: &mut Vec<String>) -> Vec<String> {
    let mut roots = Vec::new();
    for name in CRATE_NAMES {
        let directory = root.join("crates").join(name).join("examples");
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                violations.push(format!("cannot read crates/{name}/examples: {error}"));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    violations.push(format!("cannot read crates/{name}/examples entry: {error}"));
                    continue;
                }
            };
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                let file = entry.file_name().to_string_lossy().into_owned();
                roots.push(format!("crates/{name}/examples/{file}"));
            }
        }
    }
    roots.sort();
    roots
}

fn verify_ignored_boundaries(root: &Path, violations: &mut Vec<String>) {
    let gitignore_path = root.join(".gitignore");
    let gitignore = match read_text(root, &gitignore_path) {
        Ok(text) => text,
        Err(error) => {
            violations.push(error);
            return;
        }
    };

    if !gitignore.lines().any(|line| line.trim() == ".env") {
        violations.push("missing credential ignore: .env".to_owned());
    }
}

fn verify_schema_documents(root: &Path, violations: &mut Vec<String>) {
    for schema in [
        "configuration",
        "configuration-override",
        "evidence",
        "fixture",
        "pack",
        "package",
        "provenance",
        "runtime-inputs",
        "saved-data",
        "spike-result",
    ] {
        let relative = format!("schemas/{schema}.schema.json");
        let path = root.join(&relative);
        match read_json(&path) {
            Ok(document)
                if document.is_object()
                    && document["$schema"] == "https://json-schema.org/draft/2020-12/schema" => {}
            Ok(_) => violations.push(format!("invalid JSON schema document: {relative}")),
            Err(error) => violations.push(error),
        }
    }
}

#[cfg(test)]
mod tests;
