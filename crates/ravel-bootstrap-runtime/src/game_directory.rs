//! Checks the launch directory before persisting client settings.
//!
//! An existing `options.txt` is required before saving; premain reads and
//! world-pack containment use the stored launch path without this client marker.

use std::path::Path;

// The client writes this before screens open; saves/ may not exist on a first launch.
const MARKER: &str = "options.txt";

/// Whether a directory carries the sign of being the game's.
pub(crate) fn admit(directory: &Path) -> Result<(), String> {
    if directory.join(MARKER).is_file() {
        return Ok(());
    }
    Err(format!(
        "`{}` holds no `{MARKER}` and so cannot be confirmed as the client game directory",
        directory.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::{MARKER, admit};

    fn directory(name: &str) -> std::path::PathBuf {
        crate::tests::fresh_directory("game-directory", name)
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
}
