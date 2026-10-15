//! Holds launch-supplied packs for world and create-world staging hooks.
//!
//! Repository creation selects packs; create-world initialization must also reload
//! staged data. The world-open fallback covers paths that skip those hooks.
//! Repeated writes are safe because `write_pack` leaves identical trees untouched.

use std::path::Path;
use std::sync::{Mutex, PoisonError};

use ravel_loader::{Half, PACK_DIRECTORY, Supplied, WORLD_DATA_PACKS, Written, write_pack};

static SUPPLIED: Mutex<Option<Supplied>> = Mutex::new(None);

/// Installs the launch's layered mod files for later hooks.
pub fn install(supplied: Supplied) {
    *SUPPLIED.lock().unwrap_or_else(PoisonError::into_inner) = Some(supplied);
}

/// Writes installed data into `world`, or returns `None` if no set is installed.
///
/// # Errors
///
/// Returns an error if the pack cannot be written.
pub fn write_world_pack(world: &Path) -> Result<Option<Written>, ravel_loader::PackError> {
    write_data_pack(&world.join(WORLD_DATA_PACKS))
}

/// Writes installed data into a staging holder, or returns `None` without a set.
///
/// # Errors
///
/// Returns an error if the pack cannot be written.
pub fn write_staged_pack(holder: &Path) -> Result<Option<Written>, ravel_loader::PackError> {
    write_data_pack(holder)
}

// Staging copies the holder's contents, so the pack belongs directly inside it.
fn write_data_pack(holder: &Path) -> Result<Option<Written>, ravel_loader::PackError> {
    let held = SUPPLIED.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(supplied) = held.as_ref() else {
        return Ok(None);
    };
    write_pack(
        &holder.join(PACK_DIRECTORY),
        supplied,
        Half::Data,
        &crate::runtime::data_pack_metadata(Half::Data.description()),
    )
    .map(Some)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{write_staged_pack, write_world_pack};

    fn directory(name: &str) -> PathBuf {
        let at =
            std::env::temp_dir().join(format!("ravel-world-pack-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        std::fs::create_dir_all(&at).expect("a writable temporary directory");
        at
    }

    // Installing a set here would affect every test sharing the process-wide slot.
    #[test]
    fn a_launch_with_no_installed_set_leaves_the_world_alone() {
        let world = directory("absent");

        let written = write_world_pack(&world).expect("nothing to write cannot fail");

        assert!(written.is_none());
        assert!(!world.join("datapacks").exists());
        let _ = std::fs::remove_dir_all(world);
    }

    #[test]
    fn a_launch_with_no_installed_set_stages_nothing() {
        let staging = directory("unstaged");

        let written = write_staged_pack(&staging).expect("nothing to write cannot fail");

        assert!(written.is_none());
        assert!(!staging.join("ravel").exists());
        assert!(!staging.join("datapacks").exists());
        let _ = std::fs::remove_dir_all(staging);
    }
}
