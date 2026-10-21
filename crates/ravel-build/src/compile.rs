//! Materializes a planned workspace, invokes Cargo, and delivers its library.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use ravel_core::{MINECRAFT_VERSION_SELECTION, ModId};

use crate::HostPlan;
use crate::error::{BuildRefused, Wanted};
use ravel_core::paths::{HOST_DIRECTORY, HOST_LIBRARY, INSTANCE_DIRECTORY};

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
    let workspace = std::path::absolute(game_directory.join(INSTANCE_DIRECTORY))
        .map_err(|source| BuildRefused::Toolchain { source })?;
    let cache = std::path::absolute(cache).map_err(|source| BuildRefused::Toolchain { source })?;
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

    let output = plan
        .compiler
        .cargo_command()
        .current_dir(&workspace)
        .arg("build")
        .arg("--release")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(workspace.join(HOST_DIRECTORY).join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&cache)
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
    let absolute = std::path::absolute(game_directory).ok();
    let mut redacted = absolute.map_or_else(
        || text.to_owned(),
        |path| text.replace(path.to_string_lossy().as_ref(), "<instance>"),
    );
    let instance = game_directory.to_string_lossy();
    if !instance.is_empty() {
        redacted = redacted.replace(instance.as_ref(), "<instance>");
    }
    redacted
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
    let output = plan
        .compiler
        .cargo_command()
        .current_dir(workspace)
        .args(["metadata", "--offline", "--format-version", "1"])
        .arg("--manifest-path")
        .arg(workspace.join(HOST_DIRECTORY).join("Cargo.toml"))
        .stdout(Stdio::null())
        .output()
        .map_err(|error| BuildRefused::Toolchain { source: error })?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(metadata_refusal(plan, &detail, game_directory));
    }
    let resolved =
        fs::read_to_string(&lock).map_err(|source| BuildRefused::ResolvedLockIo { source })?;
    let mods = plan
        .members()
        .iter()
        .map(|member| (member.crate_name.as_str(), &member.mod_id))
        .collect();
    vetted_dependencies(plan.installation_lock(), &resolved, &mods)
}

pub(super) fn metadata_refusal(
    plan: &HostPlan,
    detail: &str,
    game_directory: &Path,
) -> BuildRefused {
    let mods: Vec<_> = plan
        .members()
        .iter()
        .filter(|member| detail.contains(&format!("package `{} v", member.crate_name)))
        .map(|member| member.mod_id.clone())
        .collect();
    let missing = missing_dependency(detail).filter(|(name, wanted)| {
        !lock_packages(plan.installation_lock()).is_ok_and(|packages| {
            let allowed = author_closure(&packages);
            packages.iter().enumerate().any(|(index, package)| {
                field(package, "name") == Some(*name)
                    && (mods.is_empty() || allowed.contains(&index))
                    && match wanted {
                        Wanted::Unresolved => true,
                        Wanted::Resolved(version) => field(package, "version")
                            .and_then(|value| semver::Version::parse(value).ok())
                            .is_some_and(|candidate| candidate == *version),
                        Wanted::Requirement(requirement) => field(package, "version")
                            .and_then(|value| semver::Version::parse(value).ok())
                            .is_some_and(|candidate| requirement.matches(&candidate)),
                    }
            })
        })
    });
    if let Some((package, version)) = missing {
        return BuildRefused::UnpinnedDependency {
            package: package.to_owned(),
            version,
            mods,
        };
    }
    BuildRefused::Compile {
        detail: redact(detail, game_directory),
    }
}

fn missing_dependency(detail: &str) -> Option<(&str, Wanted)> {
    for line in detail.lines() {
        if let Some(rest) = line.strip_prefix("error: no matching package named `") {
            let (package, _) = rest.split_once("` found")?;
            return Some((package, Wanted::Unresolved));
        }
        if let Some(rest) =
            line.strip_prefix("error: failed to select a version for the requirement `")
        {
            let (requirement, _) = rest.split_once('`')?;
            let (name, version) = requirement.split_once(" = ")?;
            let requirement = semver::VersionReq::parse(version.trim_matches('"')).ok()?;
            return Some((name, Wanted::Requirement(requirement)));
        }
        if detail.contains("--offline was specified")
            && let Some(rest) = line.strip_prefix("error: failed to download `")
        {
            let (identity, _) = rest.split_once('`')?;
            let (name, version) = identity.split_once(" v")?;
            return Some((
                name,
                Wanted::Resolved(semver::Version::parse(version).ok()?),
            ));
        }
    }
    None
}

pub(super) fn vetted_dependencies(
    seed: &str,
    resolved: &str,
    mods: &BTreeMap<&str, &ModId>,
) -> Result<(), BuildRefused> {
    let vetted = lock_packages(seed).map_err(|detail| BuildRefused::Installation {
        file: "Cargo.lock".to_owned(),
        detail,
    })?;
    let resolved =
        lock_packages(resolved).map_err(|detail| BuildRefused::ResolvedLock { detail })?;
    let allowed = author_closure(&vetted);
    let mod_graph = dependency_closure(&resolved, |package| {
        package.get("source").is_none()
            && field(package, "name").is_some_and(|name| mods.contains_key(name))
    });
    for (index, package) in resolved.iter().enumerate() {
        let name = field(package, "name").expect("lock package names are validated");
        if package.get("source").is_none()
            && (name == "ravel-generated-host" || mods.contains_key(name))
        {
            continue;
        }
        let pinned = vetted.iter().position(|candidate| {
            ["name", "version", "source", "checksum"]
                .iter()
                .all(|field| candidate.get(*field) == package.get(*field))
        });
        let forbidden_direct_path = package.get("source").is_none()
            && name != "ravel-sdk"
            && resolved.iter().any(|dependent| {
                dependent.get("source").is_none()
                    && field(dependent, "name").is_some_and(|name| mods.contains_key(name))
                    && depends_on(dependent, package)
            });
        if pinned.is_none()
            || (mod_graph.contains(&index) && !pinned.is_some_and(|index| allowed.contains(&index)))
            || forbidden_direct_path
        {
            let version = field(package, "version")
                .and_then(|value| semver::Version::parse(value).ok())
                .expect("lock package versions are validated");
            return Err(BuildRefused::UnpinnedDependency {
                package: name.to_owned(),
                version: Wanted::Resolved(version),
                mods: requiring_mods(&resolved, index, mods),
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
                identity.next() == field(package, "name")
                    && identity
                        .next()
                        .is_none_or(|version| Some(version) == field(package, "version"))
                    && identity.next().is_none_or(|source| {
                        source
                            .strip_prefix('(')
                            .and_then(|value| value.strip_suffix(')'))
                            == field(package, "source")
                    })
                    && identity.next().is_none()
            })
        })
}

fn field<'a>(package: &'a toml::Value, name: &str) -> Option<&'a str> {
    package.get(name).and_then(toml::Value::as_str)
}

fn author_closure(packages: &[toml::Value]) -> BTreeSet<usize> {
    dependency_closure(packages, |package| {
        field(package, "name")
            .is_some_and(|name| name == "ravel-sdk" || crate::AUTHOR_DEPENDENCIES.contains(&name))
    })
}

fn dependency_closure(
    packages: &[toml::Value],
    is_seed: impl Fn(&toml::Value) -> bool,
) -> BTreeSet<usize> {
    let mut pending = Vec::with_capacity(packages.len());
    pending.extend(
        packages
            .iter()
            .enumerate()
            .filter_map(|(index, package)| is_seed(package).then_some(index)),
    );
    let mut seen: BTreeSet<_> = pending.iter().copied().collect();
    while let Some(current) = pending.pop() {
        for (index, package) in packages.iter().enumerate() {
            if depends_on(&packages[current], package) && seen.insert(index) {
                pending.push(index);
            }
        }
    }
    seen
}

/// Walks resolved dependency identities backward to installed mod crates.
fn requiring_mods(
    packages: &[toml::Value],
    package: usize,
    mods: &BTreeMap<&str, &ModId>,
) -> Vec<ModId> {
    let mut pending = Vec::with_capacity(packages.len());
    pending.push(package);
    let mut seen = BTreeSet::from([package]);
    let mut found = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if packages[current].get("source").is_none()
            && let Some(mod_id) = field(&packages[current], "name").and_then(|name| mods.get(name))
        {
            found.insert((*mod_id).clone());
        }
        for (index, dependent) in packages.iter().enumerate() {
            if depends_on(dependent, &packages[current]) && seen.insert(index) {
                pending.push(index);
            }
        }
    }
    found.into_iter().collect()
}

pub(super) fn lock_packages(lock: &str) -> Result<Vec<toml::Value>, String> {
    let mut parsed: toml::Table = toml::from_str(lock).map_err(|error| error.to_string())?;
    match parsed.remove("package") {
        Some(toml::Value::Array(packages)) => {
            for package in &packages {
                field(package, "name")
                    .filter(|name| !name.is_empty())
                    .ok_or_else(|| "a dependency lock package has no name".to_owned())?;
                field(package, "version")
                    .and_then(|version| semver::Version::parse(version).ok())
                    .ok_or_else(|| "a dependency lock package has no valid version".to_owned())?;
            }
            Ok(packages)
        }
        _ => Err("the dependency lock has no package list".to_owned()),
    }
}
