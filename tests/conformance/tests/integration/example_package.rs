#![forbid(unsafe_code)]

//! Example manifests must match compiled identities, versions, saved-data declarations,
//! source-file inventories, and file digests.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ravel_example_content_mod::ContentMod;
use ravel_example_minimal_mod::MinimalMod;
use ravel_example_stateful_mod::Journal;
use ravel_package::{PackageManifest, verify_package};
use ravel_sdk::{Declared, Mod};

const EXAMPLES_DIRECTORY: &str = "examples";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two directories below the repository root")
        .to_path_buf()
}

// New packaged examples must add a compiler-derived identity here.
fn declared_identities() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("content-mod", ContentMod::declared_id().as_str()),
        ("minimal-mod", MinimalMod::declared_id().as_str()),
        ("stateful-mod", Journal::declared_id().as_str()),
    ])
}

fn packaged_examples() -> Vec<String> {
    let mut directories = fs::read_dir(repository_root().join(EXAMPLES_DIRECTORY))
        .expect("the repository has an examples directory")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.join("package.json").is_file())
        .map(|path| {
            path.file_name()
                .expect("a named directory")
                .to_str()
                .expect("an example with a portable name")
                .to_owned()
        })
        .collect::<Vec<_>>();
    directories.sort();
    directories
}

fn package_directory(example: &str) -> PathBuf {
    repository_root().join(EXAMPLES_DIRECTORY).join(example)
}

fn manifest(example: &str) -> PackageManifest {
    let document = fs::read_to_string(package_directory(example).join("package.json"))
        .expect("the example carries a package manifest");
    PackageManifest::parse(&document).expect("the example manifest parses")
}

// Examples currently use flat src directories.
fn source_files_on_disk(example: &str) -> Vec<String> {
    let directory = package_directory(example);
    let mut files = vec!["Cargo.toml".to_owned()];
    let mut sources = fs::read_dir(directory.join("src"))
        .expect("the example has a source directory")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .map(|path| {
            format!(
                "src/{}",
                path.file_name()
                    .expect("a named file")
                    .to_str()
                    .expect("a source file with a portable name")
            )
        })
        .collect::<Vec<_>>();
    sources.sort();
    files.extend(sources);
    files
}

#[test]
fn every_example_that_ships_a_manifest_is_held_to_its_crate() {
    let known = declared_identities();
    for example in packaged_examples() {
        assert!(
            known.contains_key(example.as_str()),
            "`{example}` ships a package manifest that nothing in this file compares to its crate"
        );
    }
}

#[test]
fn the_manifest_names_the_identity_the_crate_declares() {
    let known = declared_identities();
    for example in packaged_examples() {
        let Some(declared) = known.get(example.as_str()) else {
            continue;
        };
        assert_eq!(
            manifest(&example).id.as_str(),
            *declared,
            "`{example}`'s manifest and its `#[declare_mod]` are two statements of one identity"
        );
    }
}

#[test]
fn the_manifest_names_the_version_the_crate_is_built_at() {
    for example in packaged_examples() {
        let document = fs::read_to_string(package_directory(&example).join("Cargo.toml"))
            .expect("the example carries a cargo manifest");
        let cargo: toml::Table = document.parse().expect("the cargo manifest parses");
        let declared = cargo["package"]["version"]
            .as_str()
            .expect("the example pins its own version rather than inheriting one");
        assert_eq!(
            manifest(&example).version.to_string(),
            declared,
            "{example}"
        );
    }
}

#[test]
fn every_source_file_the_example_is_built_from_is_declared_and_verified() {
    for example in packaged_examples() {
        let manifest = manifest(&example);
        let mut declared = manifest
            .source
            .files
            .iter()
            .map(|file| file.path.as_str().to_owned())
            .collect::<Vec<_>>();
        declared.sort();
        assert_eq!(
            declared,
            source_files_on_disk(&example),
            "a source file `{example}`'s manifest does not declare is a source file nothing hashes"
        );
    }
}

#[test]
fn the_declared_digests_are_the_digests_of_the_files_on_disk() {
    for example in packaged_examples() {
        let manifest = manifest(&example);
        verify_package(&package_directory(&example), &manifest)
            .unwrap_or_else(|error| panic!("`{example}` does not verify: {error}"));
    }
}

#[test]
fn a_mod_that_owns_world_data_declares_its_versions_in_its_manifest() {
    let journal = Journal::new();
    let declaration = journal
        .saved_data_declaration()
        .expect("the saved-data declaration succeeds")
        .expect("the stateful example owns world data");
    let declared = declaration
        .versions()
        .map(std::num::NonZero::get)
        .collect::<Vec<_>>();
    let recorded = manifest("stateful-mod")
        .saved_data_versions
        .iter()
        .map(|version| version.get())
        .collect::<Vec<_>>();
    assert_eq!(recorded, declared);
}
