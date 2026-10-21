//! Owns launch-supplied pack publication and its JVM crossings.
//!
//! Repository creation selects packs; create-world initialization must also reload
//! staged data. Persistence admission itself never writes a pack; legacy worlds
//! consume resources and recipes through their adapter instead of data packs.
//! Repeated writes are safe because `write_game_pack` leaves identical trees untouched.

use std::path::{Path, PathBuf};
use std::sync::PoisonError;

use crate::runtime::LoaderRuntime;

use ravel_loader::Supplied;

pub(crate) mod crossings;
mod writer;

pub(crate) use writer::replaced_paths;
pub use writer::{GamePackError, Written};
use writer::{Half, write_game_pack};

/// Where a client scans for packs a player can enable.
pub(crate) const RESOURCE_PACKS: &str = "resourcepacks";
/// Where a world's automatically enabled packs live.
pub(crate) const WORLD_DATA_PACKS: &str = "datapacks";
/// The one directory Ravel writes in either place.
pub(crate) const GAME_PACK_DIRECTORY: &str = "ravel";

const ASSET_DESCRIPTION: &str = "Assets supplied by the Ravel mods this instance runs.";
const DATA_DESCRIPTION: &str = "Data supplied by the Ravel mods this world was opened with.";

/// Result of publishing the selected launch's data pack.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub enum PackOutcome {
    /// No loader runtime is installed, so no publication was attempted.
    RuntimeUnavailable,
    /// Publication was refused; the installed runtime holds a diagnostic notice.
    Refused,
    /// The existing pack already holds the selected launch's content.
    Unchanged,
    /// The selected launch's pack was written.
    Written,
}

/// Writes the selected launch's world data pack before discovery.
///
/// Admission of saved world data is independent of this operation. Filesystem
/// refusals are also recorded as runtime notices; a missing runtime is explicit.
pub fn write_world_datapack(world: &Path) -> PackOutcome {
    crate::runtime::selected().map_or(PackOutcome::RuntimeUnavailable, |runtime| {
        runtime.write_world_pack(world)
    })
}

fn write_staged_datapack(directory: &Path) -> PackOutcome {
    crate::runtime::selected().map_or(PackOutcome::RuntimeUnavailable, |runtime| {
        runtime.write_staged_pack(directory)
    })
}

/// The folder-source identifier for the installed `ravel` resource-pack directory.
#[must_use]
pub fn pack_id() -> &'static str {
    concat!("file/", "ravel")
}

/// The installed world-pack root.
pub(crate) fn installed_pack_root(world: &Path) -> PathBuf {
    world.join(WORLD_DATA_PACKS).join(GAME_PACK_DIRECTORY)
}

// Staging copies the holder's contents, so the pack belongs directly inside it.
pub(crate) fn staged_pack_root(holder: &Path) -> PathBuf {
    holder.join(GAME_PACK_DIRECTORY)
}

/// Writes the launch's supplied assets into the instance resource-pack directory.
///
/// # Errors
///
/// Returns an error if the pack cannot be written.
pub fn write_resource_pack(
    game_directory: &Path,
    supplied: &Supplied,
) -> Result<Written, GamePackError> {
    write_game_pack(
        &game_directory
            .join(RESOURCE_PACKS)
            .join(GAME_PACK_DIRECTORY),
        supplied,
        Half::Assets,
        &resource_pack_metadata(ASSET_DESCRIPTION),
    )
}

/// Writes the launch's supplied data into `world`.
///
/// # Errors
///
/// Returns an error if the pack cannot be written.
fn write_world_pack(world: &Path, supplied: &Supplied) -> Result<Written, GamePackError> {
    write_data_pack(&installed_pack_root(world), supplied)
}

/// Writes the launch's supplied data into a staging holder.
///
/// # Errors
///
/// Returns an error if the pack cannot be written.
pub(crate) fn write_staged_pack(
    holder: &Path,
    supplied: &Supplied,
) -> Result<Written, GamePackError> {
    write_data_pack(&staged_pack_root(holder), supplied)
}

fn write_data_pack(root: &Path, supplied: &Supplied) -> Result<Written, GamePackError> {
    write_game_pack(
        root,
        supplied,
        Half::Data,
        &data_pack_metadata(DATA_DESCRIPTION),
    )
}

/// Renders resource-pack metadata using the pinned version's format.
#[must_use]
fn resource_pack_metadata(description: &str) -> String {
    ravel_minecraft::pack_metadata(
        ravel_minecraft_adapter::pinned::RESOURCE_PACK_FORMAT,
        ravel_minecraft_adapter::pinned::PACK_FORMAT_SPELLING,
        description,
    )
}

/// Renders data-pack metadata using the pinned version's format.
#[must_use]
fn data_pack_metadata(description: &str) -> String {
    ravel_minecraft::pack_metadata(
        ravel_minecraft_adapter::pinned::DATA_PACK_FORMAT,
        ravel_minecraft_adapter::pinned::PACK_FORMAT_SPELLING,
        description,
    )
}
// Reject paths whose tree replacement could overwrite the game directory.
fn admissible_world(world: &Path, game: &Path) -> Result<(), String> {
    if !world.is_absolute() {
        return Err(format!(
            "`{}` is not an absolute path, so it names a directory relative to the game's own",
            world.display()
        ));
    }
    if !world.is_dir() {
        return Err(format!(
            "`{}` is not an existing directory",
            world.display()
        ));
    }
    let world = world
        .canonicalize()
        .map_err(|error| format!("world path cannot be resolved: {error}"))?;
    let game = game
        .canonicalize()
        .map_err(|error| format!("launch directory cannot be resolved: {error}"))?;
    if game.starts_with(&world) {
        return Err(format!(
            "`{}` holds the game directory `{}`, and a pack is written by replacing a tree \
             inside the path it is given",
            world.display(),
            game.display()
        ));
    }
    pack_targets_are_real(&world)
}

// The writer replaces these entries, so a link among them would redirect it outside `world`.
fn pack_targets_are_real(world: &Path) -> Result<(), String> {
    let packs = world.join(WORLD_DATA_PACKS);
    let roots = [installed_pack_root(world), staged_pack_root(world)];
    for path in std::iter::once(packs).chain(roots.iter().flat_map(|root| replaced_paths(root))) {
        if std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(format!(
                "`{}` is a symbolic link, and a pack is written only to real directories \
                 inside the world",
                path.display()
            ));
        }
    }
    Ok(())
}

impl LoaderRuntime {
    pub(crate) fn write_world_pack(&self, world: &Path) -> PackOutcome {
        self.put_pack(world, write_world_pack)
    }

    pub(crate) fn write_staged_pack(&self, directory: &Path) -> PackOutcome {
        self.put_pack(directory, write_staged_pack)
    }

    fn put_pack(
        &self,
        path: &Path,
        write: fn(&Path, &Supplied) -> Result<Written, GamePackError>,
    ) -> PackOutcome {
        let directory = path.display();
        let launch = &self.launch;
        if let Err(refusal) = admissible_world(path, &launch.game_directory) {
            self.inner
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .report
                .note(&format!(
                    "no data pack was written into `{directory}`, so this world reads none of \
                     the mods' recipes or tags: {refusal}"
                ));
            return PackOutcome::Refused;
        }
        match write(path, &launch.supplied) {
            Ok(written) if written.changed() => PackOutcome::Written,
            Ok(_) => PackOutcome::Unchanged,
            Err(error) => {
                self.inner
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .report
                    .note(&format!(
                        "`{directory}` could not be given the mods' data pack, so this world \
                         reads none of their recipes or tags: {error}"
                    ));
                PackOutcome::Refused
            }
        }
    }
}

#[cfg(test)]
mod admission_tests;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{write_staged_pack, write_world_pack};
    use ravel_loader::Supplied;

    fn directory(name: &str) -> PathBuf {
        crate::test_support::fresh_directory("world-pack", name)
    }

    #[test]
    fn the_pack_id_matches_the_bootstrap_destination() {
        assert_eq!(
            super::pack_id(),
            format!("file/{}", super::GAME_PACK_DIRECTORY)
        );
    }

    #[test]
    fn world_and_staged_writers_use_the_shared_roots() {
        let world = directory("shared-roots");
        let supplied = Supplied::default();
        assert_eq!(
            write_world_pack(&world, &supplied)
                .expect("world pack")
                .root(),
            super::installed_pack_root(&world)
        );
        assert_eq!(
            write_staged_pack(&world, &supplied)
                .expect("staged pack")
                .root(),
            super::staged_pack_root(&world)
        );
        std::fs::remove_dir_all(world).expect("remove world");
    }

    // A launch with no declared files still publishes the version's metadata.
    #[test]
    fn an_empty_launch_writes_world_pack_metadata() {
        let world = directory("absent");

        write_world_pack(&world, &Supplied::default()).expect("the pack can be written");

        assert!(world.join("datapacks/ravel/pack.mcmeta").is_file());
        std::fs::remove_dir_all(world).expect("remove the test world");
    }

    #[test]
    fn an_empty_launch_stages_pack_metadata() {
        let staging = directory("unstaged");

        write_staged_pack(&staging, &Supplied::default()).expect("the pack can be written");

        assert!(staging.join("ravel/pack.mcmeta").is_file());
        assert!(!staging.join("datapacks").exists());
        std::fs::remove_dir_all(staging).expect("remove the staging directory");
    }
}
