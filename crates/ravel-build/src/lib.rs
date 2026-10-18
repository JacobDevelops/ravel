#![forbid(unsafe_code)]

//! Plans deterministic host workspaces from installed packages and builds their libraries.
//!
//! Installed mods may reach only the installation-pinned dependency closure of
//! `ravel-sdk`, `serde`, `serde_json`, and `num-traits`; direct internal path
//! dependencies remain restricted to the SDK and other planned mods. Generated
//! hosts may use other installation-pinned dependencies. Player workspaces keep
//! the installation's Rust channel with the minimal profile, not its developer
//! components or cross-compilation targets.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use ravel_core::ModId;
use ravel_core::paths::HOST_DIRECTORY;
use ravel_loader::Discovered;
use ravel_package::Sha256Digest;
use sha2::{Digest, Sha256};

mod compile;
mod error;
mod generate;

pub use compile::build;
pub use error::{BuildRefused, Wanted};

use generate::{host_manifest, host_source, workspace_manifest};

/// The entry function `ravel_sdk::export_mod!` writes into a package.
pub const ENTRY_FUNCTION: &str = "__ravel_entry";

// Workspace-lent dependencies and explicitly versioned author seeds are separate:
// num-traits is admitted without adding a workspace inheritance convention.
const AUTHOR_DEPENDENCIES: &[&str] = &["serde", "serde_json"];
const AUTHOR_SEEDS: &[&str] = &["ravel-sdk", "serde", "serde_json", "num-traits"];

/// What a host will be generated from, and the bytes that generate it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostPlan {
    files: BTreeMap<String, String>,
    installation_lock: String,
    members: Vec<PlannedMember>,
    fingerprint: Sha256Digest,
}

impl HostPlan {
    /// Returns generated files in path order, relative to the instance's `ravel` directory.
    #[must_use]
    pub const fn files(&self) -> &BTreeMap<String, String> {
        &self.files
    }

    /// Returns the installation `Cargo.lock` every instance resolution starts from.
    #[must_use]
    pub fn installation_lock(&self) -> &str {
        &self.installation_lock
    }

    /// Returns the packages this host will be built from, in mod-id order.
    #[must_use]
    pub fn members(&self) -> &[PlannedMember] {
        &self.members
    }

    /// Returns the generated-byte digest, not Cargo's build-cache key.
    #[must_use]
    pub const fn fingerprint(&self) -> &Sha256Digest {
        &self.fingerprint
    }
}

/// One installed package as the generated host will name it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedMember {
    /// The identity its manifest declares.
    pub mod_id: ModId,
    /// The directory it is installed under.
    pub directory: String,
    /// The Cargo package name its own `Cargo.toml` declares.
    pub crate_name: String,
}

/// Plans a host using installed package metadata and the caller's Ravel installation root.
///
/// # Errors
/// Returns a refusal for missing or invalid installation inputs, Cargo manifests, or duplicate crate names.
pub fn plan(discovered: &Discovered, ravel_root: &Path) -> Result<HostPlan, BuildRefused> {
    let mut members = Vec::with_capacity(discovered.packages().len());
    for package in discovered.packages() {
        let directory = package.directory().to_owned();
        let crate_name = crate_name(package.root(), &directory)?;
        if let Some(clash) = members
            .iter()
            .find(|planned: &&PlannedMember| planned.crate_name == crate_name)
        {
            return Err(BuildRefused::DuplicateCrate {
                crate_name,
                directory,
                first: clash.directory.clone(),
            });
        }
        members.push(PlannedMember {
            mod_id: package.manifest().id.clone(),
            directory,
            crate_name,
        });
    }

    let dependencies = installation_dependencies(ravel_root)?;
    let author_dependencies = dependency_entries(&dependencies, AUTHOR_DEPENDENCIES)?;
    let jni_dependency = dependency_entries(&dependencies, &["jni"])?;
    let installation_lock = installation_file(ravel_root, "Cargo.lock")?;
    compile::lock_packages(&installation_lock).map_err(|detail| BuildRefused::Installation {
        file: "Cargo.lock".to_owned(),
        detail,
    })?;
    let mut files = BTreeMap::new();
    files.insert(
        "rust-toolchain.toml".to_owned(),
        player_toolchain(&installation_file(ravel_root, "rust-toolchain.toml")?)?,
    );
    files.insert(
        "Cargo.toml".to_owned(),
        workspace_manifest(&members, ravel_root, &author_dependencies),
    );
    files.insert(
        format!("{HOST_DIRECTORY}/Cargo.toml"),
        host_manifest(&members, ravel_root, &jni_dependency),
    );
    files.insert(
        format!("{HOST_DIRECTORY}/src/lib.rs"),
        host_source(&members),
    );

    let mut digest = Sha256::new();
    for (path, contents) in &files {
        digest.update(path.as_bytes());
        digest.update([0]);
        digest.update(contents.as_bytes());
        digest.update([0]);
    }
    digest.update(b"installation Cargo.lock");
    digest.update([0]);
    digest.update(installation_lock.as_bytes());
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&digest.finalize());
    let fingerprint = Sha256Digest::from_bytes(bytes);

    Ok(HostPlan {
        files,
        installation_lock,
        members,
        fingerprint,
    })
}

fn crate_name(root: &Path, directory: &str) -> Result<String, BuildRefused> {
    let path = root.join("Cargo.toml");
    let document = match fs::read_to_string(&path) {
        Ok(document) => document,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(BuildRefused::MissingCargoManifest {
                directory: directory.to_owned(),
            });
        }
        Err(error) => {
            return Err(BuildRefused::CargoManifestIo {
                directory: directory.to_owned(),
                source: error,
            });
        }
    };
    let parsed: toml::Table =
        document
            .parse()
            .map_err(|error: toml::de::Error| BuildRefused::CargoManifest {
                directory: directory.to_owned(),
                detail: error.to_string(),
            })?;
    parsed
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| BuildRefused::CargoManifest {
            directory: directory.to_owned(),
            detail: "no `package.name` is declared".to_owned(),
        })
}

fn player_toolchain(document: &str) -> Result<String, BuildRefused> {
    let invalid = |detail| BuildRefused::Installation {
        file: "rust-toolchain.toml".to_owned(),
        detail,
    };
    let parsed: toml::Table =
        toml::from_str(document).map_err(|error| invalid(error.to_string()))?;
    let channel = parsed
        .get("toolchain")
        .and_then(|toolchain| toolchain.get("channel"))
        .and_then(toml::Value::as_str)
        .filter(|channel| !channel.is_empty())
        .ok_or_else(|| invalid("no toolchain.channel is declared".to_owned()))?;
    let toolchain = toml::Table::from_iter([
        (
            "channel".to_owned(),
            toml::Value::String(channel.to_owned()),
        ),
        (
            "profile".to_owned(),
            toml::Value::String("minimal".to_owned()),
        ),
    ]);
    toml::to_string(&toml::Table::from_iter([(
        "toolchain".to_owned(),
        toml::Value::Table(toolchain),
    )]))
    .map_err(|error| invalid(error.to_string()))
}

fn installation_file(root: &Path, file: &str) -> Result<String, BuildRefused> {
    fs::read_to_string(root.join(file)).map_err(|source| BuildRefused::InstallationIo {
        file: file.to_owned(),
        source,
    })
}

fn installation_dependencies(root: &Path) -> Result<toml::Table, BuildRefused> {
    let document = installation_file(root, "Cargo.toml")?;
    let parsed: toml::Table =
        toml::from_str(&document).map_err(|error| BuildRefused::Installation {
            file: "Cargo.toml".to_owned(),
            detail: error.to_string(),
        })?;
    parsed
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table)
        .cloned()
        .ok_or_else(|| BuildRefused::Installation {
            file: "Cargo.toml".to_owned(),
            detail: "no workspace.dependencies table".to_owned(),
        })
}

fn dependency_entries(
    dependencies: &toml::Table,
    names: &[&str],
) -> Result<toml::Table, BuildRefused> {
    let mut selected = toml::Table::new();
    for name in names {
        let value = dependencies
            .get(*name)
            .ok_or_else(|| BuildRefused::Installation {
                file: "Cargo.toml".to_owned(),
                detail: format!("no workspace dependency `{name}`"),
            })?;
        selected.insert((*name).to_owned(), value.clone());
    }
    Ok(selected)
}

#[cfg(test)]
mod tests;
