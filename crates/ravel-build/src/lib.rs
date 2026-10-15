#![forbid(unsafe_code)]

//! Plans deterministic host workspaces from installed packages and builds their libraries.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use ravel_loader::Discovered;
use ravel_package::Sha256Digest;
use sha2::{Digest, Sha256};

mod compile;
mod error;
mod generate;

pub use compile::build;
pub use error::BuildRefused;

use generate::{host_manifest, host_source, workspace_manifest};

/// The instance directory containing the workspace above its installed packages.
pub const WORKSPACE_DIRECTORY: &str = "ravel";

/// The library name the generated host produces, and the JVM loads.
pub const HOST_LIBRARY: &str = "ravel_bootstrap";

/// The entry function `ravel_sdk::export_mod!` writes into a package.
pub const ENTRY_FUNCTION: &str = "__ravel_entry";

/// What a host will be generated from, and the bytes that generate it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostPlan {
    files: BTreeMap<String, String>,
    members: Vec<PlannedMember>,
    fingerprint: String,
}

impl HostPlan {
    /// Returns generated files in path order, relative to the instance's `ravel` directory.
    #[must_use]
    pub const fn files(&self) -> &BTreeMap<String, String> {
        &self.files
    }

    /// Returns the packages this host will be built from, in mod-id order.
    #[must_use]
    pub fn members(&self) -> &[PlannedMember] {
        &self.members
    }

    /// Returns the generated-byte digest, not Cargo's build-cache key.
    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

/// One installed package as the generated host will name it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedMember {
    /// The identity its manifest declares.
    pub mod_id: String,
    /// The directory it is installed under.
    pub directory: String,
    /// The Cargo package name its own `Cargo.toml` declares.
    pub crate_name: String,
}

/// Plans a host using installed package metadata and the caller's Ravel installation root.
///
/// # Errors
/// Returns a refusal for missing or invalid Cargo manifests or duplicate crate names.
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
            mod_id: package.manifest().id.as_str().to_owned(),
            directory,
            crate_name,
        });
    }

    let mut files = BTreeMap::new();
    files.insert(
        "Cargo.toml".to_owned(),
        workspace_manifest(&members, ravel_root),
    );
    files.insert("host/Cargo.toml".to_owned(), host_manifest(&members));
    files.insert("host/src/lib.rs".to_owned(), host_source(&members));

    let mut digest = Sha256::new();
    for (path, contents) in &files {
        digest.update(path.as_bytes());
        digest.update([0]);
        digest.update(contents.as_bytes());
        digest.update([0]);
    }
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&digest.finalize());
    let fingerprint = Sha256Digest::from_bytes(bytes).to_string();

    Ok(HostPlan {
        files,
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
            return Err(BuildRefused::CargoManifest {
                directory: directory.to_owned(),
                detail: error.to_string(),
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

#[cfg(test)]
mod tests;
