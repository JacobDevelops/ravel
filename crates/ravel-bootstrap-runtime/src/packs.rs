//! Holds launch-supplied packs for world and create-world staging hooks.
//!
//! Repository creation selects packs; create-world initialization must also reload
//! staged data. The world-open fallback covers paths that skip those hooks.
//! Repeated writes are safe because `write_game_pack` leaves identical trees untouched.

use std::path::Path;

use ravel_loader::{
    GAME_PACK_DIRECTORY, Half, Supplied, WORLD_DATA_PACKS, Written, write_game_pack,
};

/// Writes the launch's supplied data into `world`.
///
/// # Errors
///
/// Returns an error if the pack cannot be written.
pub fn write_world_pack(
    supplied: &Supplied,
    world: &Path,
) -> Result<Written, ravel_loader::GamePackError> {
    write_data_pack(supplied, &world.join(WORLD_DATA_PACKS))
}

/// Writes the launch's supplied data into a staging holder.
///
/// # Errors
///
/// Returns an error if the pack cannot be written.
pub(crate) fn write_staged_pack(
    supplied: &Supplied,
    holder: &Path,
) -> Result<Written, ravel_loader::GamePackError> {
    write_data_pack(supplied, holder)
}

// Staging copies the holder's contents, so the pack belongs directly inside it.
fn write_data_pack(
    supplied: &Supplied,
    holder: &Path,
) -> Result<Written, ravel_loader::GamePackError> {
    write_game_pack(
        &holder.join(GAME_PACK_DIRECTORY),
        supplied,
        Half::Data,
        &crate::runtime::data_pack_metadata(Half::Data.description()),
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{write_staged_pack, write_world_pack};
    use ravel_loader::Supplied;

    fn directory(name: &str) -> PathBuf {
        let at =
            std::env::temp_dir().join(format!("ravel-world-pack-{}-{name}", std::process::id()));
        if at.exists() {
            std::fs::remove_dir_all(&at).expect("remove the previous test directory");
        }
        std::fs::create_dir_all(&at).expect("a writable temporary directory");
        at
    }

    // A launch with no declared files still publishes the version's metadata.
    #[test]
    fn an_empty_launch_writes_world_pack_metadata() {
        let world = directory("absent");

        write_world_pack(&Supplied::default(), &world).expect("the pack can be written");

        assert!(world.join("datapacks/ravel/pack.mcmeta").is_file());
        std::fs::remove_dir_all(world).expect("remove the test world");
    }

    #[test]
    fn an_empty_launch_stages_pack_metadata() {
        let staging = directory("unstaged");

        write_staged_pack(&Supplied::default(), &staging).expect("the pack can be written");

        assert!(staging.join("ravel/pack.mcmeta").is_file());
        assert!(!staging.join("datapacks").exists());
        std::fs::remove_dir_all(staging).expect("remove the staging directory");
    }
}
