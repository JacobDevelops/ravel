//! Where the game keeps its files, as far as this process can tell.
//!
//! Minecraft takes `--gameDir` and the client resolves it into a directory every
//! save, every world and every options file hangs off. Ravel is a JVM agent
//! rather than the launcher, so it has to find that directory rather than be
//! told it, and this module is that answer and the reasons it is what it is.
//!
//! # Nothing in the launch names it, measured three ways
//!
//! - **The command line does not carry it.** On the pinned Prism launch the java
//!   command ends at `org.prismlauncher.EntryPoint` with no program arguments at
//!   all; the launcher feeds the game's arguments to that entry point over
//!   standard input, which is also how it keeps the access token off the process
//!   table. So `sun.java.command` names the entry point and no path.
//! - **No system property names it.** That launch sets `user.language`, the
//!   agent, the library path twice, the heap bounds and the class path, and
//!   nothing else. The game sets exactly one property of its own at startup, and
//!   it is `java.awt.headless`.
//! - **The game's own answer is a field, and a field is not reachable.**
//!   `Minecraft.gameDirectory` is the authority, and reading it is three separate
//!   steps rather than one; see "the upgrade path" below.
//!
//! # So: the working directory, and why that is a footing rather than a guess
//!
//! The pinned client declares `--gameDir` with `defaultsTo(new File("."))`. When
//! the argument is absent the game directory *is* the working directory — not
//! something that resembles it, the same thing, by the option's own default. So
//! this is the game's own resolution rather than an imitation of it.
//!
//! It is wrong in exactly one case: a launcher that passes a `--gameDir`
//! different from the directory it spawned the process in. Ravel cannot detect
//! that case, because under Prism the arguments never reach the process table at
//! all. Prism was *expected* to set the working directory to the instance game
//! root, which makes the two agree, and M3-W7 measured it doing so:
//! `evidence/m3/linux-process-matrix.json` records the game process's own
//! working directory, read from outside the process on a launch with no agent
//! attached, and it is the instance game root.
//!
//! The measurement is stronger than the guard below needs, and deliberately so.
//! A marker sitting in the working directory only proves the client has run
//! there at some point. What was recorded is that the client rewrote
//! `options.txt` **during that launch** — its constructor calls `Options.save()`
//! into the directory it resolved `--gameDir` to, so the file's contents being
//! that launch's is the client's own resolution agreeing with the working
//! directory. The footing is measured rather than expected.
//!
//! That result is Prism 11.0.3's. The undetectable case has not gone away; no
//! launcher Ravel has run under exhibits it.
//!
//! The process's own working directory is read rather than the `user.dir`
//! property: it needs no call into the JVM, and it is the same thing the game's
//! `new File(".")` resolves against, so it tracks the game rather than a snapshot
//! taken when the JVM started.
//!
//! # The upgrade path
//!
//! `Minecraft.gameDirectory` is the authority — `public final java.io.File`,
//! assigned once in the client's constructor from the parsed argument and never
//! reassigned. It is **not one hook away**, and anyone reaching for it should
//! know that before they start rather than after:
//!
//! 1. It needs a `hooks.toml` entry, which is a reviewed inventory change.
//! 2. That alone would not work. **A field entry generates no Rust surface at
//!    all** — the generator skips every non-method hook, and its own test asserts
//!    that a field is not a call target and gets no export. The field entries in
//!    the inventory today are drift-checked declarations, not readable state, so
//!    reading one needs generator machinery that does not exist. That is the open
//!    question in issue #29.
//! 3. ADG-4 forbids it as written: reading through a handle and keeping the
//!    result is a mirror, and a mirror needs a measurement, a staleness argument
//!    and an ADR. A configuration writer holding a path is a copy that stays.
//!
//! # When configuration uses this answer
//!
//! Premain reads player configuration from the working directory while it builds
//! the statically composed set. It also installs the settings store then, but does
//! not run this marker check yet: on a brand-new instance the client constructor
//! has not created `options.txt`. Saving resolves and admits the directory after a
//! screen can exist, when the constructor guarantee above holds.

use std::path::{Path, PathBuf};

/// The file the client writes into its game directory during startup.
///
/// This is the marker that says a directory is the game's rather than merely the
/// one Ravel happened to start in, and it is a real guard rather than a hopeful
/// one because of *when* the game writes it: the client's constructor calls
/// `Options.save()`, so `options.txt` exists long before any screen can be
/// opened, including on a brand-new instance whose first launch this is. A player
/// closing a settings screen has always passed that point.
///
/// The other two obvious markers were considered and are not used. `saves/` does
/// not exist until a world is created, so it would refuse a legitimate first
/// write. `logs/` is worse than useless here: the game's logging writes it
/// relative to the working directory, so it would be present in the working
/// directory *even when that is not the game directory*, which is precisely the
/// case this guard exists to catch.
const MARKER: &str = "options.txt";

/// The game directory, or why this process cannot name one.
///
/// # Errors
///
/// Returns a reason when the working directory cannot be read, or when it holds
/// no sign of being a game directory. Refusing is deliberate: the alternative is
/// writing a player's settings somewhere that is not the game's, silently, and
/// the player then finding their edits gone with nothing having said so.
pub(crate) fn resolve() -> Result<PathBuf, String> {
    let directory = std::env::current_dir()
        .map_err(|error| format!("the working directory cannot be read: {error}"))?;
    admit(&directory).map(|()| directory)
}

/// Whether a directory carries the sign of being the game's.
fn admit(directory: &Path) -> Result<(), String> {
    if directory.join(MARKER).is_file() {
        return Ok(());
    }
    Err(format!(
        "`{}` holds no `{MARKER}` and so is not the game directory; \
         Ravel takes the working directory as the game directory, which is what \
         the game itself does unless it was launched with `--gameDir`",
        directory.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::{MARKER, admit};

    fn directory(name: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "ravel-game-directory-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("a writable temporary directory");
        directory
    }

    #[test]
    fn a_directory_the_client_has_started_in_is_admitted() {
        let game = directory("started");
        std::fs::write(game.join(MARKER), "version:1\n").expect("the marker should be writable");

        assert_eq!(admit(&game), Ok(()));
    }

    #[test]
    fn a_directory_with_no_sign_of_the_game_is_refused_and_says_which_sign() {
        let game = directory("elsewhere");

        let refused = admit(&game).expect_err("a directory with no marker is refused");
        assert!(refused.contains(MARKER), "{refused}");
        assert!(refused.contains(&game.display().to_string()), "{refused}");
    }

    #[test]
    fn a_marker_that_is_a_directory_rather_than_a_file_does_not_admit_it() {
        let game = directory("marker-is-a-directory");
        std::fs::create_dir(game.join(MARKER)).expect("the directory should be creatable");

        assert!(admit(&game).is_err());
    }

    #[test]
    fn the_working_directory_is_what_resolution_answers_with() {
        // The gate runs in the repository, which is not a game directory, so the
        // only thing this can assert without moving the process is that
        // resolution reads the working directory and applies the same admission.
        let cwd = std::env::current_dir().expect("the gate has a working directory");
        assert_eq!(super::resolve().is_ok(), admit(&cwd).is_ok());
    }
}
