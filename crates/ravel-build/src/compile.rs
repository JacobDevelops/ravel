//! Materializes a planned workspace, invokes Cargo, and delivers its library.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ravel_core::MINECRAFT_VERSION_SELECTION;

use crate::{BuildRefused, HOST_LIBRARY, HostPlan, WORKSPACE_DIRECTORY};

/// Writes changed generated files, compiles the selected version, and copies the host into the instance.
///
/// # Errors
/// Returns a refusal if writing, running Cargo, compiling, or finding the built library fails.
pub fn build(
    plan: &HostPlan,
    game_directory: &Path,
    cache: &Path,
    minecraft: &str,
) -> Result<PathBuf, BuildRefused> {
    let workspace = game_directory.join(WORKSPACE_DIRECTORY);
    for (path, contents) in &plan.files {
        let target = workspace.join(path);
        let parent = target.parent().ok_or_else(|| BuildRefused::Write {
            file: path.clone(),
            detail: "the generated path has no parent".to_owned(),
        })?;
        fs::create_dir_all(parent).map_err(|error| BuildRefused::Write {
            file: path.clone(),
            detail: error.to_string(),
        })?;
        // Preserve timestamps so an unchanged workspace stays warm.
        if fs::read_to_string(&target).is_ok_and(|existing| existing == *contents) {
            continue;
        }
        fs::write(&target, contents).map_err(|error| BuildRefused::Write {
            file: path.clone(),
            detail: error.to_string(),
        })?;
    }

    let output = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--manifest-path")
        .arg(workspace.join("host/Cargo.toml"))
        .arg("--target-dir")
        .arg(cache)
        .env(MINECRAFT_VERSION_SELECTION, minecraft)
        .output()
        .map_err(|error| BuildRefused::Toolchain {
            detail: error.to_string(),
        })?;
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
    let delivered = workspace.join("host").join(&name);
    fs::copy(&built, &delivered).map_err(|error| BuildRefused::Write {
        file: format!("host/{name}"),
        detail: error.to_string(),
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
