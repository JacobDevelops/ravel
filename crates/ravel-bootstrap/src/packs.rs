//! The files the installed set supplies, held for the crossings that write them.
//!
//! A launch layers every mod's declaration once, and two places need the result:
//! the host writes the asset half into the instance before the game starts, and
//! the world-open crossing writes the data half into the world being opened. The
//! second of those happens inside a hook, long after the host's own call returned,
//! so the set is installed here rather than passed down a call chain that does not
//! exist.
//!
//! # Written twice, and the earlier one is what this session reads
//!
//! `world.open-persistence` is handed the world stem, and the game builds that
//! stem from a pack repository it has already configured. So a pack written from
//! that crossing is on disk in time for the *next* open of that world, and the
//! session that wrote it runs without Ravel's data.
//!
//! `world.create-pack-repository` is the earlier seam that closes it. It is the
//! one call every path that gives a world a repository makes, it is handed the
//! world's storage access, and it returns the repository the game then reloads
//! and configures — and configuring adds every available pack whose source asks
//! to be added automatically, which a world's own pack does. A pack on disk
//! before it returns is enabled for the session that is starting.
//!
//! Which path matters, because the world-open flow's own steps are not on all of
//! them: creating a fresh level makes the access, builds the repository and
//! loads, reaching no open step at all. A seam chosen from that flow covers every
//! session except a new world's first one, which is the session a player is most
//! likely to judge the mods by.
//!
//! Both crossings still ask, and that is deliberate rather than tolerated. Both
//! write the same tree from the one installed set, and `write_pack` compares the
//! tree on disk with the one it would write and leaves an identical one alone —
//! so the second ask writes nothing at all and neither crossing has to know about
//! the other. Keeping the later one means a world admitted through a path the
//! earlier seam never ran on still gets its pack.

use std::path::Path;
use std::sync::{Mutex, PoisonError};

use ravel_loader::{Half, PACK_DIRECTORY, Supplied, WORLD_DATA_PACKS, Written, write_pack};

/// The layered set this launch composed, or nothing before a host installs one.
static SUPPLIED: Mutex<Option<Supplied>> = Mutex::new(None);

/// Installs the files this launch's mods supply.
///
/// Called by a host once, beside the settings it installs, for the same reason:
/// what a hook reaches has to be put somewhere a hook can reach.
pub fn install(supplied: Supplied) {
    *SUPPLIED.lock().unwrap_or_else(PoisonError::into_inner) = Some(supplied);
}

/// Writes the data half of the installed set into the world rooted at `world`.
///
/// Nothing is written for a launch that installed no set, which is every launch
/// before a host called [`install`] and every test that drives a crossing
/// directly. That is an absence rather than a failure: a set nobody supplied has
/// no files, and inventing an empty pack in somebody's world would be a change to
/// a world for no reason.
///
/// # Errors
///
/// Returns whatever the filesystem refused. The caller decides what that costs; it
/// is not this function's to decide whether a world opens.
pub fn write_world_pack(world: &Path) -> Result<Option<Written>, ravel_loader::PackError> {
    let held = SUPPLIED.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(supplied) = held.as_ref() else {
        return Ok(None);
    };
    write_pack(
        &world.join(WORLD_DATA_PACKS).join(PACK_DIRECTORY),
        supplied,
        Half::Data,
        &crate::runtime::data_pack_metadata(Half::Data.description()),
    )
    .map(Some)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::write_world_pack;

    fn directory(name: &str) -> PathBuf {
        let at =
            std::env::temp_dir().join(format!("ravel-world-pack-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        std::fs::create_dir_all(&at).expect("a writable temporary directory");
        at
    }

    /// A launch that installed no set writes nothing rather than an empty pack.
    ///
    /// The process-wide slot is what makes this the only case a unit test here can
    /// state: a test that installed a set would decide what every other test in
    /// this binary sees. The written tree is held to its contents in
    /// `ravel-loader`, over the same writer, and end to end in the conformance
    /// suite.
    #[test]
    fn a_launch_with_no_installed_set_leaves_the_world_alone() {
        let world = directory("absent");

        let written = write_world_pack(&world).expect("nothing to write cannot fail");

        assert!(written.is_none());
        assert!(!world.join("datapacks").exists());
        let _ = std::fs::remove_dir_all(world);
    }
}
