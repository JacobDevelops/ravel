#![forbid(unsafe_code)]

//! Shipped package manifests agree with their compiled crates and sources.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use ravel_example_content_mod::ContentMod;
use ravel_example_minimal_mod::MinimalMod;
use ravel_example_stateful_mod::Journal;
use ravel_package::{PackageManifest, verify_package};
use ravel_sdk::{Declared, Mod, Side};

use super::repository_root;

fn declaration<M: Mod + Declared>(host: &M) -> (&'static str, Vec<u32>) {
    let versions = host
        .saved_data_declaration()
        .expect("the saved-data declaration succeeds")
        .map_or_else(Vec::new, |declaration| {
            declaration.versions().map(std::num::NonZero::get).collect()
        });
    (M::declared_id().as_str(), versions)
}

fn test_mod_declarations() -> BTreeMap<&'static str, (&'static str, Vec<u32>)> {
    BTreeMap::from([
        (
            "test-mods/audio",
            declaration(&ravel_test_mod_audio::AudioMod::new()),
        ),
        (
            "test-mods/client-only",
            declaration(&ravel_test_mod_client_only::ClientOnlyMod::new(
                Side::Client,
                &ravel_test_mod_client_only::work_log(),
            )),
        ),
        (
            "test-mods/commands",
            declaration(&ravel_test_mod_commands::CommandsMod::new()),
        ),
        (
            "test-mods/configuration",
            declaration(&ravel_test_mod_configuration::ConfigurationMod::new()),
        ),
        (
            "test-mods/networking",
            declaration(&ravel_test_mod_networking::NetworkingMod::new()),
        ),
        (
            "test-mods/registries",
            declaration(&ravel_test_mod_registries::RegistriesMod::new()),
        ),
        (
            "test-mods/saved-data",
            declaration(&ravel_test_mod_saved_data::SavedDataMod::new()),
        ),
        (
            "test-mods/server-only",
            declaration(&ravel_test_mod_server_only::ServerOnlyMod::new(
                Side::DedicatedServer,
                &ravel_test_mod_server_only::work_log(),
            )),
        ),
        (
            "test-mods/waystone-ledger",
            declaration(&waystone_ledger::WaystoneLedger::new(Side::Client)),
        ),
    ])
}

// Adding a shipped package requires a compiler-derived declaration, not another literal id.
fn declarations() -> BTreeMap<&'static str, (&'static str, Vec<u32>)> {
    let mut declarations = BTreeMap::from([
        ("examples/content-mod", declaration(&ContentMod::new())),
        ("examples/minimal-mod", declaration(&MinimalMod::new())),
        ("examples/stateful-mod", declaration(&Journal::new())),
        (
            "registry/mods/rvl-002-counterpart",
            declaration(&rvl_002_counterpart::Entry::new()),
        ),
        (
            "registry/mods/rvl-025-counterpart",
            declaration(&rvl_025_counterpart::Entry::new(Side::Client)),
        ),
        (
            "registry/mods/rvl-042-counterpart",
            declaration(&rvl_042_counterpart::Entry::new()),
        ),
        (
            "registry/mods/rvl-060-counterpart",
            declaration(&rvl_060_counterpart::Entry::new(
                Side::Client,
                "package-agreement",
            )),
        ),
        (
            "registry/mods/rvl-086-counterpart",
            declaration(&rvl_086_counterpart::Entry::new()),
        ),
        (
            "registry/mods/rvl-120-counterpart",
            declaration(&rvl_120_counterpart::Entry::new(Side::Client)),
        ),
        (
            "registry/mods/rvl-132-counterpart",
            declaration(&rvl_132_counterpart::Entry::new()),
        ),
        (
            "registry/mods/rvl-146-counterpart",
            declaration(&rvl_146_counterpart::Entry::new(Side::Client)),
        ),
        (
            "registry/mods/rvl-180-counterpart",
            declaration(&rvl_180_counterpart::Entry::new(Side::Client)),
        ),
        (
            "registry/mods/rvl-210-counterpart",
            declaration(&rvl_210_counterpart::Entry::new()),
        ),
        (
            "registry/mods/rvl-287-counterpart",
            declaration(&rvl_287_counterpart::Entry::new(Side::Client)),
        ),
        (
            "registry/mods/rvl-321-counterpart",
            declaration(&rvl_321_counterpart::Entry::new()),
        ),
    ]);
    declarations.extend(test_mod_declarations());
    declarations
}

fn packages() -> Vec<String> {
    let mut packages = Vec::new();
    for parent in ["examples", "registry/mods", "test-mods"] {
        for entry in fs::read_dir(repository_root().join(parent)).expect("package directory exists")
        {
            let path = entry.expect("a readable directory entry").path();
            if path.join("package.json").is_file() {
                packages.push(format!(
                    "{parent}/{}",
                    path.file_name()
                        .expect("a named directory")
                        .to_str()
                        .expect("a portable package name")
                ));
            }
        }
    }
    packages.sort();
    packages
}

fn manifest(package: &str) -> PackageManifest {
    let document = fs::read_to_string(repository_root().join(package).join("package.json"))
        .expect("the package carries a manifest");
    PackageManifest::parse(&document).expect("the package manifest parses")
}

fn cargo(package: &str) -> toml::Table {
    fs::read_to_string(repository_root().join(package).join("Cargo.toml"))
        .expect("the package carries a Cargo manifest")
        .parse()
        .expect("the Cargo manifest parses")
}

fn rust_sources(root: &Path, directory: &Path, files: &mut BTreeSet<String>) {
    for entry in fs::read_dir(directory).expect("the source directory is readable") {
        let path = entry.expect("a readable source entry").path();
        if path.is_dir() {
            rust_sources(root, &path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.insert(
                path.strip_prefix(root)
                    .expect("a package source")
                    .to_str()
                    .expect("a portable source path")
                    .to_owned(),
            );
        }
    }
}

#[test]
fn every_shipped_package_is_held_to_its_compiled_declaration() {
    let known = declarations();
    assert_eq!(
        packages(),
        known
            .keys()
            .map(|key| (*key).to_owned())
            .collect::<Vec<_>>()
    );
    for (package, (id, versions)) in known {
        let manifest = manifest(package);
        assert_eq!(manifest.id.as_str(), id, "{package}");
        let recorded: Vec<_> = manifest
            .saved_data_versions
            .iter()
            .map(|version| version.get())
            .collect();
        assert_eq!(
            recorded, versions,
            "{package}: saved-data declarations must agree"
        );
    }
}

#[test]
fn each_manifest_names_the_version_the_crate_is_built_at() {
    for package in packages() {
        let cargo = cargo(&package);
        let version = cargo["package"]["version"]
            .as_str()
            .expect("a pinned package version");
        assert_eq!(manifest(&package).version.to_string(), version, "{package}");
    }
}

#[test]
fn every_compiled_source_is_declared_and_all_declared_files_verify() {
    for package in packages() {
        let root = repository_root().join(&package);
        let manifest = manifest(&package);
        let declared: BTreeSet<_> = manifest
            .source
            .files
            .iter()
            .filter(|file| {
                let path = Path::new(file.path.as_str());
                path == Path::new("Cargo.toml")
                    || path.extension().is_some_and(|extension| extension == "rs")
            })
            .map(|file| file.path.as_str().to_owned())
            .collect();
        let mut sources = BTreeSet::from(["Cargo.toml".to_owned()]);
        rust_sources(&root, &root.join("src"), &mut sources);
        if root.join("tests").is_dir() {
            rust_sources(&root, &root.join("tests"), &mut sources);
        }
        if root.join("build.rs").is_file() {
            sources.insert("build.rs".to_owned());
        }
        assert_eq!(
            declared, sources,
            "{package}: all compiled sources must be hashed"
        );
        verify_package(&root, &manifest)
            .unwrap_or_else(|error| panic!("`{package}` does not verify: {error}"));
    }
}

#[test]
fn registry_cargo_siblings_are_required_resolver_dependencies() {
    let registry: Vec<_> = packages()
        .into_iter()
        .filter(|package| package.starts_with("registry/mods/"))
        .collect();
    let siblings: BTreeMap<_, _> = registry
        .iter()
        .map(|package| {
            let cargo = cargo(package);
            (
                cargo["package"]["name"]
                    .as_str()
                    .expect("a crate name")
                    .to_owned(),
                manifest(package),
            )
        })
        .collect();
    for package in &registry {
        let cargo = cargo(package);
        let document: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(repository_root().join(package).join("package.json"))
                .expect("a package manifest"),
        )
        .expect("a JSON manifest");
        for (name, dependency) in cargo["dependencies"].as_table().expect("dependencies") {
            let name = dependency
                .get("package")
                .and_then(toml::Value::as_str)
                .unwrap_or(name);
            let Some(sibling) = siblings.get(name) else {
                continue;
            };
            let edge = document["dependencies"]
                .as_array()
                .expect("resolver dependencies")
                .iter()
                .find(|edge| edge["id"].as_str() == Some(sibling.id.as_str()))
                .unwrap_or_else(|| {
                    panic!("{package}: Cargo sibling {name} is absent from resolver dependencies")
                });
            assert_eq!(edge["relationship"], "required", "{package}: {name}");
            assert_eq!(edge["order"], "after", "{package}: {name}");
            let range =
                ravel_sdk::VersionRange::parse(edge["version"].as_str().expect("a version range"))
                    .expect("a valid version range");
            assert!(
                range.matches(&sibling.version),
                "{package}: {name} version must satisfy resolver dependency"
            );
        }
    }
}
