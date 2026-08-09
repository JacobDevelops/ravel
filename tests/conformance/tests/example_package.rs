#![forbid(unsafe_code)]

//! The minimal example's package, held to the crate beside it.
//!
//! `examples/minimal-mod` is the model an author copies, and a package is two
//! statements of one mod: `package.json` tells the loader who the mod is, and
//! `#[declare_mod]` tells the compiler. Nothing in the workspace compares them —
//! a mismatch produces a mod whose content is namespaced under a name the loader
//! never heard of — so for the one package an author is told to copy, this file
//! compares them.
//!
//! It also recomputes the two things a hand-written manifest goes stale about:
//! the digests, and the file list. A digest is checked by `verify_package`, which
//! is the same call the loader makes. The file list is checked here because
//! `verify_package` cannot: it verifies every file the manifest declares and has
//! no opinion about a source file the manifest forgot, so a new module added to
//! the example would ship unverified and nothing would say so.

use std::fs;
use std::path::{Path, PathBuf};

use ravel_example_minimal_mod::MinimalMod;
use ravel_package::{PackageManifest, verify_package};
use ravel_sdk::Declared;

const PACKAGE_DIRECTORY: &str = "examples/minimal-mod";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two directories below the repository root")
        .to_path_buf()
}

fn package_directory() -> PathBuf {
    repository_root().join(PACKAGE_DIRECTORY)
}

fn manifest() -> PackageManifest {
    let document = fs::read_to_string(package_directory().join("package.json"))
        .expect("the example carries a package manifest");
    PackageManifest::parse(&document).expect("the example manifest parses")
}

/// Every source file the crate is built from, package-relative.
///
/// `Cargo.toml` plus every Rust source under `src/`, which is the whole of what
/// this example is. A package that named a directory would need a walk; this one
/// is flat by construction, and a nested module would show up here as a file the
/// listing does not contain rather than being silently skipped.
fn source_files_on_disk() -> Vec<String> {
    let directory = package_directory();
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
fn the_manifest_names_the_identity_the_crate_declares() {
    assert_eq!(
        manifest().id.as_str(),
        MinimalMod::declared_id().as_str(),
        "the example's manifest and its `#[declare_mod]` are two statements of one identity"
    );
}

#[test]
fn the_manifest_names_the_version_the_crate_is_built_at() {
    let document = fs::read_to_string(package_directory().join("Cargo.toml"))
        .expect("the example carries a cargo manifest");
    let cargo: toml::Table = document.parse().expect("the cargo manifest parses");
    let declared = cargo["package"]["version"]
        .as_str()
        .expect("the example pins its own version rather than inheriting one");
    assert_eq!(manifest().version.to_string(), declared);
}

#[test]
fn every_source_file_the_example_is_built_from_is_declared_and_verified() {
    let manifest = manifest();
    let mut declared = manifest
        .source
        .files
        .iter()
        .map(|file| file.path.as_str().to_owned())
        .collect::<Vec<_>>();
    declared.sort();
    assert_eq!(
        declared,
        source_files_on_disk(),
        "a source file the manifest does not declare is a source file nothing hashes"
    );
}

#[test]
fn the_declared_digests_are_the_digests_of_the_files_on_disk() {
    verify_package(&package_directory(), &manifest())
        .expect("the example's own package verifies, or an author copies a stale one");
}
