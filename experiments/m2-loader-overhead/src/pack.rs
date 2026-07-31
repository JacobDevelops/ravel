//! Measures what a linked pack costs to build.
//!
//! ADG-2 deferred the mod execution model to M3 and named the numbers the
//! distribution question turns on: link time, binary growth, and compile time
//! for a pack-sized set. It measured none of them, because its linked trial
//! linked one mod. This measures all three, and selects nothing: every crate it
//! generates is ordinary safe Rust behind the same `HostMod` contract the
//! in-process no-op package already uses, and nothing here loads a library,
//! starts a guest runtime, spawns a mod process, or exports a foreign symbol.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// Mods in the pack-sized set.
///
/// A large modpack is a few hundred mods; ATM11 0.2.1, the pinned M0 fixture,
/// is that scale. 256 is inside it and is a round number to divide by.
pub const PACK_SIZE: usize = 256;

/// What building a pack cost.
#[derive(Clone, Copy, Debug)]
pub struct BuildCost {
    /// Mods in the measured pack.
    pub pack_size: usize,
    /// Cold build of the host with no mods linked: the shared cost of the
    /// loader and its dependencies.
    pub baseline_build_ms: u64,
    /// Cold build of the host with the whole pack linked.
    pub pack_build_ms: u64,
    /// Rebuilding after one mod's source changed, which is the iterate-on-a-mod
    /// cost ADG-2 put on M2's tooling.
    pub relink_ms: u64,
    /// The host binary with no mods linked.
    pub baseline_bytes: u64,
    /// The host binary with the whole pack linked.
    pub pack_bytes: u64,
}

impl BuildCost {
    /// What the pack itself added to a cold build.
    #[must_use]
    pub const fn pack_compile_ms(&self) -> u64 {
        self.pack_build_ms.saturating_sub(self.baseline_build_ms)
    }

    /// What one mod added to the binary.
    #[must_use]
    pub const fn bytes_per_mod(&self) -> u64 {
        if self.pack_size == 0 {
            return 0;
        }
        self.pack_bytes.saturating_sub(self.baseline_bytes) / self.pack_size as u64
    }

    /// What one mod added to a cold build.
    #[must_use]
    pub const fn compile_ms_per_mod(&self) -> u64 {
        if self.pack_size == 0 {
            return 0;
        }
        self.pack_compile_ms() / self.pack_size as u64
    }
}

/// Builds a pack-sized set and one without mods, and times both.
///
/// # Errors
///
/// Returns a message when a generated workspace cannot be written or cargo
/// refuses to build one.
pub fn measure(root: &Path, work: &Path) -> Result<BuildCost, String> {
    let baseline = generate(root, work, 0)?;
    let (baseline_build_ms, baseline_bytes) = cold_build(&baseline)?;

    let pack = generate(root, work, PACK_SIZE)?;
    let (pack_build_ms, pack_bytes) = cold_build(&pack)?;

    // One mod's source changes and the set is rebuilt. This is the whole cost of
    // editing a mod under a linked model: recompile the crate, relink the host.
    touch(&pack.join("mods/mod-000/src/lib.rs"))?;
    let relink_ms = build(&pack)?;

    Ok(BuildCost {
        pack_size: PACK_SIZE,
        baseline_build_ms,
        pack_build_ms,
        relink_ms,
        baseline_bytes,
        pack_bytes,
    })
}

/// Discards the build directory, builds, and reports the time and binary size.
fn cold_build(workspace: &Path) -> Result<(u64, u64), String> {
    let target = workspace.join("target");
    if target.exists() {
        fs::remove_dir_all(&target)
            .map_err(|error| format!("cannot clear a generated build directory: {error}"))?;
    }
    let elapsed = build(workspace)?;
    let binary = workspace.join("target/release/ravel-pack-host");
    let bytes = fs::metadata(&binary)
        .map_err(|error| format!("the generated host binary is missing: {error}"))?
        .len();
    Ok((elapsed, bytes))
}

/// Runs one release build and returns its wall-clock milliseconds.
fn build(workspace: &Path) -> Result<u64, String> {
    let started = Instant::now();
    let output = Command::new("cargo")
        .args(["build", "--release", "--quiet"])
        .current_dir(workspace)
        .output()
        .map_err(|error| format!("cannot run cargo: {error}"))?;
    let elapsed = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    if !output.status.success() {
        return Err(format!(
            "the generated pack did not build: {}",
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .last()
                .unwrap_or("no output")
        ));
    }
    Ok(elapsed)
}

/// Rewrites a file with one changed byte so cargo rebuilds its crate.
fn touch(path: &Path) -> Result<(), String> {
    let mut text = fs::read_to_string(path)
        .map_err(|error| format!("cannot read a generated mod: {error}"))?;
    text.push_str("\n// edited\n");
    fs::write(path, text).map_err(|error| format!("cannot edit a generated mod: {error}"))
}

/// Writes a workspace holding `count` mod crates and one host binary.
///
/// The mods are generated rather than tracked because the measurement is about
/// the count, and 256 hand-written no-op crates would be 256 files saying the
/// same thing. Nothing generated here is tracked, and nothing production reads
/// it.
fn generate(root: &Path, work: &Path, count: usize) -> Result<PathBuf, String> {
    let workspace = work.join(format!("pack-{count}"));
    let mods = workspace.join("mods");
    if mods.exists() {
        fs::remove_dir_all(&mods)
            .map_err(|error| format!("cannot clear generated mods: {error}"))?;
    }
    // The directory exists even when the pack is empty, because the workspace's
    // `mods/*` member glob is an error against a missing directory and matches
    // nothing against an empty one.
    fs::create_dir_all(&mods).map_err(|error| format!("cannot create generated mods: {error}"))?;
    fs::create_dir_all(workspace.join("host/src"))
        .map_err(|error| format!("cannot create a generated workspace: {error}"))?;

    // The repository's lock file, so the generated workspace resolves to the
    // same dependency versions the loader itself was built against.
    let lock = fs::read(root.join("Cargo.lock"))
        .map_err(|error| format!("cannot read the workspace lock file: {error}"))?;
    write(&workspace.join("Cargo.lock"), &lock)?;
    // Cargo refuses a member glob that matches nothing, so an empty pack names
    // only the host.
    let members = if count == 0 {
        "\"host\""
    } else {
        "\"host\", \"mods/*\""
    };
    write(
        &workspace.join("Cargo.toml"),
        WORKSPACE_MANIFEST.replace("MEMBERS", members).as_bytes(),
    )?;

    let mut dependencies = String::new();
    let mut uses = String::new();
    let mut pushes = String::new();
    for index in 0..count {
        let name = format!("mod-{index:03}");
        generate_mod(&mods.join(&name), &name, root)?;
        let _ = writeln!(dependencies, "{name} = {{ path = \"../mods/{name}\" }}");
        let _ = writeln!(uses, "use mod_{index:03}::PackMod as Mod{index:03};");
        let _ = writeln!(
            pushes,
            "    mods.push(Box::new(Mod{index:03}::new()) as Box<dyn HostMod>);"
        );
    }

    write(
        &workspace.join("host/Cargo.toml"),
        manifest(HOST_MANIFEST, root)
            .replace("MOD_DEPENDENCIES", &dependencies)
            .as_bytes(),
    )?;
    write(
        &workspace.join("host/src/main.rs"),
        HOST_SOURCE
            .replace("MOD_USES\n", &uses)
            .replace("MOD_PUSHES\n", &pushes)
            .as_bytes(),
    )?;
    Ok(workspace)
}

/// Writes one generated mod crate.
fn generate_mod(directory: &Path, name: &str, root: &Path) -> Result<(), String> {
    fs::create_dir_all(directory.join("src"))
        .map_err(|error| format!("cannot create a generated mod: {error}"))?;
    write(
        &directory.join("Cargo.toml"),
        manifest(MOD_MANIFEST, root)
            .replace("MOD_NAME", name)
            .as_bytes(),
    )?;
    write(
        &directory.join("src/lib.rs"),
        MOD_SOURCE.replace("MOD_NAME", name).as_bytes(),
    )
}

/// Points a generated manifest's path dependencies at this repository.
fn manifest(template: &str, root: &Path) -> String {
    template
        .replace(
            "CORE_PATH",
            &root.join("crates/ravel-core").display().to_string(),
        )
        .replace(
            "HOST_API_PATH",
            &root.join("crates/ravel-host-api").display().to_string(),
        )
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|error| format!("cannot write a generated file: {error}"))
}

const WORKSPACE_MANIFEST: &str = r#"[workspace]
resolver = "3"
members = [MEMBERS]
"#;

const MOD_MANIFEST: &str = r#"[package]
name = "MOD_NAME"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
ravel-core = { path = "CORE_PATH" }
ravel-host-api = { path = "HOST_API_PATH" }
"#;

const HOST_MANIFEST: &str = r#"[package]
name = "ravel-pack-host"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
ravel-core = { path = "CORE_PATH" }
ravel-host-api = { path = "HOST_API_PATH" }
MOD_DEPENDENCIES"#;

const MOD_SOURCE: &str = r#"#![forbid(unsafe_code)]

use ravel_core::{LifecycleState, ModId};
use ravel_host_api::{HostError, HostMod};

/// The no-op mod body under a generated identity.
pub struct PackMod {
    id: ModId,
    entered: usize,
}

impl PackMod {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: "MOD_NAME".parse().expect("a legal mod id"),
            entered: 0,
        }
    }
}

impl Default for PackMod {
    fn default() -> Self {
        Self::new()
    }
}

impl HostMod for PackMod {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
        self.entered += 1;
        Ok(())
    }
}
"#;

const HOST_SOURCE: &str = r#"use ravel_core::LifecycleState;
use ravel_host_api::HostMod;
MOD_USES

fn main() {
    let mut mods: Vec<Box<dyn HostMod>> = Vec::new();
MOD_PUSHES
    let mut entered = 0usize;
    for value in &mut mods {
        for state in [
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
            LifecycleState::Running,
            LifecycleState::Stopping,
        ] {
            value
                .enter(state)
                .expect("a generated mod accepts every step");
            entered += 1;
        }
    }
    println!("{entered}");
}
"#;
