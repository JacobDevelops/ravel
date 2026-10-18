//! Holds launch-supplied packs for world and create-world staging hooks.
//!
//! Repository creation selects packs; create-world initialization must also reload
//! staged data. Persistence admission itself never writes a pack; legacy worlds
//! consume resources and recipes through their adapter instead of data packs.
//! Repeated writes are safe because `write_game_pack` leaves identical trees untouched.

use std::path::{Path, PathBuf};

use ravel_loader::Supplied;

mod writer;

pub(crate) use writer::replaced_paths;
pub use writer::{GamePackError, Written};
use writer::{Half, write_game_pack};

/// Where a client scans for packs a player can enable.
pub const RESOURCE_PACKS: &str = "resourcepacks";
/// Where a world's automatically enabled packs live.
pub const WORLD_DATA_PACKS: &str = "datapacks";
/// The one directory Ravel writes in either place.
pub const GAME_PACK_DIRECTORY: &str = "ravel";

const ASSET_DESCRIPTION: &str = "Assets supplied by the Ravel mods this instance runs.";
const DATA_DESCRIPTION: &str = "Data supplied by the Ravel mods this world was opened with.";

/// The folder-source identifier for [`GAME_PACK_DIRECTORY`].
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
pub fn write_world_pack(world: &Path, supplied: &Supplied) -> Result<Written, GamePackError> {
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
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{write_staged_pack, write_world_pack};
    use ravel_loader::Supplied;

    fn directory(name: &str) -> PathBuf {
        crate::tests::fresh_directory("world-pack", name)
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
