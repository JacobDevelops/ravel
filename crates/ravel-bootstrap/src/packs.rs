//! The files the installed set supplies, held for the crossings that write them.
//!
//! A launch layers every mod's declaration once, and two places need the result:
//! the host writes the asset half into the instance before the game starts, and
//! the world-open crossing writes the data half into the world being opened. The
//! second of those happens inside a hook, long after the host's own call returned,
//! so the set is installed here rather than passed down a call chain that does not
//! exist.
//!
//! # Three paths ask, because the game reads a world's data at three moments
//!
//! Being on disk and being *loaded* are different things, and a pack has to make
//! both. The game selects packs by scanning a directory, and it loads their
//! contents in a separate step that a path may skip.
//!
//! `world.open-persistence` is the latest of the three. It is handed the world
//! stem, which the game built from a repository it had already configured, so a
//! pack written there is on disk in time for the world's *next* open and the
//! session that wrote it runs without Ravel's data.
//!
//! `world.create-pack-repository` is the seam that gets the pack selected. It is
//! the one call every path that gives a world a repository makes, it carries the
//! world's storage access, and the repository it returns is reloaded and
//! configured — and configuring adds every available pack whose source asks to be
//! added automatically, which a world's own pack does. Opening an existing world
//! reloads the world's data after this, so for that path it is enough on its own.
//!
//! `screen.create-world-init` is the seam that gets the pack *loaded* in a world
//! being created. Creating a world does not reload data: the create-world screen
//! loads recipes, tags and loot tables once when it opens, and the created world
//! is handed those same resources. A pack that appears after that load is
//! selected and unread, which is a world listing the mod's pack and knowing none
//! of its recipes. So the pack is written where the screen's own data load will
//! see it, into the staging directory the screen copies into the new world, and
//! the screen is asked to load again — which is the path the game already runs
//! when a player adds a data pack by hand.
//!
//! All three still ask, and that is deliberate rather than tolerated. Each writes
//! the same tree from the one installed set, and `write_pack` compares the tree on
//! disk with the one it would write and leaves an identical one alone — so a
//! later ask over an unchanged tree writes nothing at all and no crossing has to
//! know about the others. Keeping the latest one means a world admitted through a
//! path the earlier seams never ran on still gets its pack.

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
    write_data_pack(&world.join(WORLD_DATA_PACKS))
}

/// Writes the data half of the installed set into the pack directory `holder`.
///
/// The same tree as [`write_world_pack`] one level higher up, because the
/// create-world screen stages packs in a directory of its own and copies that
/// directory's *contents* into the world it makes. So what goes here is the pack
/// directory itself, and the world ends up with the tree the other crossings
/// would have written.
///
/// # Errors
///
/// Returns whatever the filesystem refused, for the same reason and with the same
/// consequence as [`write_world_pack`].
pub fn write_staged_pack(holder: &Path) -> Result<Option<Written>, ravel_loader::PackError> {
    write_data_pack(holder)
}

/// Puts the data half's pack directory inside `holder`.
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

    /// The staging entry point invents no pack either, and puts none where the
    /// world one would.
    ///
    /// Both halves matter. A directory that is a world's and a directory that is
    /// the create-world screen's differ by exactly one level, and an entry point
    /// that wrote the world's shape into the screen's would land the pack at
    /// `datapacks/datapacks/ravel` in the world the screen copies into.
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
