//! Materializes a planned workspace, invokes Cargo, and delivers its library.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ravel_core::MINECRAFT_VERSION_SELECTION;

use crate::error::BuildRefused;
use crate::{HOST_LIBRARY, HostPlan};
use ravel_core::paths::{HOST_DIRECTORY, INSTANCE_DIRECTORY};

/// Writes changed generated files, compiles the selected version, and copies the host into the instance.
///
/// # Errors
/// Returns a refusal if writing, running Cargo, compiling, or finding the built library fails.
///
/// # Panics
/// Panics if an internally generated file path violates the workspace-parent invariant.
pub fn build(
    plan: &HostPlan,
    game_directory: &Path,
    cache: &Path,
    minecraft: &str,
) -> Result<PathBuf, BuildRefused> {
    let workspace = game_directory.join(INSTANCE_DIRECTORY);
    for (path, contents) in &plan.files {
        let target = workspace.join(path);
        let parent = target
            .parent()
            .expect("generated relative files joined to the instance workspace have a parent");
        fs::create_dir_all(parent).map_err(|error| BuildRefused::Write {
            file: path.clone(),
            source: error,
        })?;
        // Preserve timestamps so an unchanged workspace stays warm.
        if fs::read_to_string(&target).is_ok_and(|existing| existing == *contents) {
            continue;
        }
        fs::write(&target, contents).map_err(|error| BuildRefused::Write {
            file: path.clone(),
            source: error,
        })?;
    }
    resolve_locked_graph(plan, &workspace, game_directory)?;

    let output = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(workspace.join(HOST_DIRECTORY).join("Cargo.toml"))
        .arg("--target-dir")
        .arg(cache)
        .env(MINECRAFT_VERSION_SELECTION, minecraft)
        .output()
        .map_err(|error| BuildRefused::Toolchain { source: error })?;
    if !output.status.success() {
        return Err(BuildRefused::Compile {
            detail: redact(&String::from_utf8_lossy(&output.stderr), game_directory),
        });
    }

    let name = format!("lib{HOST_LIBRARY}.so");
    let built = cache.join("release").join(&name);
    if !built.is_file() {
        return Err(BuildRefused::MissingArtifact { library: name });
    }
    // The shared cache holds whichever instance built last; deliver an instance-owned copy.
    let delivered = workspace.join(HOST_DIRECTORY).join(&name);
    fs::copy(&built, &delivered).map_err(|error| BuildRefused::Write {
        file: format!("{HOST_DIRECTORY}/{name}"),
        source: error,
    })?;
    Ok(delivered)
}

fn redact(text: &str, game_directory: &Path) -> String {
    let instance = game_directory.display().to_string();
    if instance.is_empty() {
        return text.to_owned();
    }
    text.replace(&instance, "<instance>")
}

// Installed path packages and the generated host are not members of the root
// lockfile. Cargo may add those entries offline; every other identity must be
// vetted by the seed. Mods may directly reach only the SDK and planned members
// through path dependencies. A previous instance lock is never reused,
// because Cargo prunes unused pins from it.
fn resolve_locked_graph(
    plan: &HostPlan,
    workspace: &Path,
    game_directory: &Path,
) -> Result<(), BuildRefused> {
    let lock = workspace.join("Cargo.lock");
    if fs::read_to_string(&lock).ok().as_deref() != Some(plan.installation_lock()) {
        fs::write(&lock, plan.installation_lock()).map_err(|error| BuildRefused::Write {
            file: "Cargo.lock".to_owned(),
            source: error,
        })?;
    }
    let output = Command::new("cargo")
        .args(["metadata", "--offline", "--format-version", "1"])
        .arg("--manifest-path")
        .arg(workspace.join(HOST_DIRECTORY).join("Cargo.toml"))
        .stdout(Stdio::null())
        .output()
        .map_err(|error| BuildRefused::Toolchain { source: error })?;
    if !output.status.success() {
        return Err(BuildRefused::Compile {
            detail: redact(&String::from_utf8_lossy(&output.stderr), game_directory),
        });
    }
    let resolved =
        fs::read_to_string(&lock).map_err(|source| BuildRefused::ResolvedLockIo { source })?;
    let mods = plan
        .members()
        .iter()
        .map(|member| (member.crate_name.as_str(), member.mod_id.as_str()))
        .collect();
    vetted_dependencies(plan.installation_lock(), &resolved, &mods)
}

pub(super) fn vetted_dependencies(
    seed: &str,
    resolved: &str,
    mods: &BTreeMap<&str, &str>,
) -> Result<(), BuildRefused> {
    let vetted = lock_packages(seed).map_err(|detail| BuildRefused::Installation {
        file: "Cargo.lock".to_owned(),
        detail,
    })?;
    let resolved =
        lock_packages(resolved).map_err(|detail| BuildRefused::ResolvedLock { detail })?;
    for package in &resolved {
        let name = field(package, "name");
        if package.get("source").is_none()
            && (name == "ravel-generated-host" || mods.contains_key(name))
        {
            continue;
        }
        let matches = vetted.iter().any(|candidate| {
            ["name", "version", "source", "checksum"]
                .iter()
                .all(|field| candidate.get(*field) == package.get(*field))
        });
        let forbidden_direct_path = package.get("source").is_none()
            && name != "ravel-sdk"
            && resolved.iter().any(|dependent| {
                mods.contains_key(field(dependent, "name")) && depends_on(dependent, package)
            });
        if !matches || forbidden_direct_path {
            return Err(BuildRefused::UnpinnedDependency {
                package: name.to_owned(),
                version: field(package, "version").to_owned(),
                mods: requiring_mods(&resolved, name, mods),
            });
        }
    }
    Ok(())
}

fn depends_on(dependent: &toml::Value, package: &toml::Value) -> bool {
    dependent
        .get("dependencies")
        .and_then(toml::Value::as_array)
        .is_some_and(|edges| {
            edges.iter().filter_map(toml::Value::as_str).any(|edge| {
                let mut identity = edge.split_whitespace();
                identity.next() == Some(field(package, "name"))
                    && identity
                        .next()
                        .is_none_or(|version| version == field(package, "version"))
                    && identity.next().is_none()
            })
        })
}

fn field<'a>(package: &'a toml::Value, name: &str) -> &'a str {
    package
        .get(name)
        .and_then(toml::Value::as_str)
        .unwrap_or("<unknown>")
}

/// Walks resolved dependency edges backward from `package` to installed mod crates.
fn requiring_mods(
    packages: &[toml::Value],
    package: &str,
    mods: &BTreeMap<&str, &str>,
) -> Vec<String> {
    let mut pending = Vec::with_capacity(packages.len());
    pending.push(package);
    let mut seen = BTreeSet::from([package]);
    let mut found = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if let Some(mod_id) = mods.get(current) {
            found.insert((*mod_id).to_owned());
            continue;
        }
        for dependent in packages {
            let depends = dependent
                .get("dependencies")
                .and_then(toml::Value::as_array)
                .is_some_and(|edges| {
                    edges
                        .iter()
                        .filter_map(toml::Value::as_str)
                        .any(|edge| edge.split_whitespace().next() == Some(current))
                });
            let name = field(dependent, "name");
            if depends && seen.insert(name) {
                pending.push(name);
            }
        }
    }
    found.into_iter().collect()
}

pub(super) fn lock_packages(lock: &str) -> Result<Vec<toml::Value>, String> {
    let mut parsed: toml::Table = toml::from_str(lock).map_err(|error| error.to_string())?;
    match parsed.remove("package") {
        Some(toml::Value::Array(packages)) => Ok(packages),
        _ => Err("the dependency lock has no package list".to_owned()),
    }
}
