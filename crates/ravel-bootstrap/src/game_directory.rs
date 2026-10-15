//! Resolves the game directory from the process working directory.
//!
//! Launchers using a different `--gameDir` are not detectable here. An existing
//! `options.txt` is required before saving; premain reads do not use this guard.

use std::path::{Path, PathBuf};

// The client writes this before screens open; saves/ may not exist on a first launch.
const MARKER: &str = "options.txt";

/// Resolves the working directory and checks the client marker.
///
/// # Errors
///
/// Returns a reason if the working directory cannot be read or lacks the marker.
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
        if directory.exists() {
            std::fs::remove_dir_all(&directory).expect("remove the previous test directory");
        }
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
        let cwd = std::env::current_dir().expect("the gate has a working directory");
        assert_eq!(super::resolve(), admit(&cwd).map(|()| cwd));
    }
}
