//! Holds launch-supplied packs for world and create-world staging hooks.
//!
//! Repository creation selects packs; create-world initialization must also reload
//! staged data. Legacy admission explicitly invokes the pack crossing before opening
//! persistence; persistence admission itself never writes a pack.
//! Repeated writes are safe because `write_game_pack` leaves identical trees untouched.

use std::path::{Path, PathBuf};

use ravel_loader::{Half, Supplied, Written, write_game_pack};

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

/// The installed world-pack root and create-world staging root.
pub(crate) fn world_pack_roots(world: &Path) -> [PathBuf; 2] {
    [
        world.join(WORLD_DATA_PACKS).join(GAME_PACK_DIRECTORY),
        world.join(GAME_PACK_DIRECTORY),
    ]
}

/// Writes the launch's supplied assets into the instance resource-pack directory.
///
/// # Errors
///
/// Returns an error if the pack cannot be written.
pub fn write_resource_pack(
    game_directory: &Path,
    supplied: &Supplied,
) -> Result<Written, ravel_loader::GamePackError> {
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
pub fn write_world_pack(
    world: &Path,
    supplied: &Supplied,
) -> Result<Written, ravel_loader::GamePackError> {
    write_data_pack(&world_pack_roots(world)[0], supplied)
}

/// Writes the launch's supplied data into a staging holder.
///
/// # Errors
///
/// Returns an error if the pack cannot be written.
pub(crate) fn write_staged_pack(
    holder: &Path,
    supplied: &Supplied,
) -> Result<Written, ravel_loader::GamePackError> {
    write_data_pack(&world_pack_roots(holder)[1], supplied)
}

// Staging copies the holder's contents, so the pack belongs directly inside it.
fn write_data_pack(
    root: &Path,
    supplied: &Supplied,
) -> Result<Written, ravel_loader::GamePackError> {
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
        let roots = super::world_pack_roots(&world);
        let supplied = Supplied::default();
        assert_eq!(
            write_world_pack(&world, &supplied)
                .expect("world pack")
                .root(),
            roots[0]
        );
        assert_eq!(
            write_staged_pack(&world, &supplied)
                .expect("staged pack")
                .root(),
            roots[1]
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
