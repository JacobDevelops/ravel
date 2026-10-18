use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use ravel_loader::pack_id;

const OPTIONS: &str = "options.txt";

const KEY: &str = "resourcePacks:";

use ravel_core::paths::{INSTANCE_DIRECTORY, SEEDED_PACKS_FILE};

/// Outcome of offering the resource pack in the player's settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Seeded {
    /// The id was added to the list and the file rewritten.
    Added,
    /// The id is already in the list, so the file was left alone.
    Present,
    /// The seed has been offered before, so the player's choice stands.
    AlreadyOffered,
    /// The instance has no settings file yet, so there is nothing to seed into.
    NoSettings,
    /// The list is not a list of names, so nothing was rewritten.
    Unreadable(String),
    /// The pack was offered and the record of that could not be kept.
    Unrecorded(String),
}

/// Enables Ravel's pack once, then preserves the player's choice.
///
/// Missing settings files are not created; malformed pack lists are left untouched.
/// Other lines and their line endings are preserved byte for byte.
/// Offer-record failures return [`Seeded::Unrecorded`] without failing the launch.
///
/// # Errors
///
/// Returns an [`OptionsError`] if existing settings cannot be read as UTF-8 or replaced.
pub fn seed_pack(game_directory: &Path) -> Result<Seeded, OptionsError> {
    let record = game_directory
        .join(INSTANCE_DIRECTORY)
        .join(SEEDED_PACKS_FILE);
    match offered(&record) {
        Ok(true) => return Ok(Seeded::AlreadyOffered),
        Ok(false) => {}
        // An unreadable offer record must neither fail launch nor change settings.
        Err(refused) => return Ok(Seeded::Unrecorded(refused.to_string())),
    }

    let settings = game_directory.join(OPTIONS);
    if !settings.is_file() {
        return Ok(Seeded::NoSettings);
    }
    let text = read(&settings)?;

    let Some(line) = enabled_line(&text) else {
        let ending = if text.ends_with('\n') { "" } else { "\n" };
        let appended = format!("{text}{ending}{KEY}{}\n", list(&[pack_id().to_owned()]));
        replace(&settings, &appended)?;
        return Ok(record_offer(&record, Seeded::Added));
    };

    let Some(mut enabled) = parse(line.value) else {
        return Ok(Seeded::Unreadable(line.value.to_owned()));
    };
    let id = pack_id();
    if enabled.iter().any(|name| name == id) {
        return Ok(record_offer(&record, Seeded::Present));
    }

    let mut rewritten = String::with_capacity(text.len() + id.len() + 4);
    enabled.push(id.to_owned());
    rewritten.push_str(&text[..line.at]);
    rewritten.push_str(KEY);
    rewritten.push_str(&list(&enabled));
    rewritten.push_str(&text[line.end..]);
    replace(&settings, &rewritten)?;
    Ok(record_offer(&record, Seeded::Added))
}

// Record after the settings write so a failed write does not consume the offer.
fn record_offer(record: &Path, made: Seeded) -> Seeded {
    match write_record(record) {
        Ok(()) => made,
        Err(refused) => Seeded::Unrecorded(refused.to_string()),
    }
}

struct Line<'a> {
    at: usize,
    /// Byte after the value, before its line ending.
    end: usize,
    value: &'a str,
}

// Anchor the key to avoid rewriting `incompatibleResourcePacks` or other suffix matches.
fn enabled_line(text: &str) -> Option<Line<'_>> {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.strip_suffix('\n').unwrap_or(line);
        let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);
        if let Some(value) = trimmed.strip_prefix(KEY) {
            return Some(Line {
                at,
                end: at + trimmed.len(),
                value,
            });
        }
        at += line.len();
    }
    None
}

fn parse(value: &str) -> Option<Vec<String>> {
    let parsed: serde_json::Value = serde_json::from_str(value).ok()?;
    parsed
        .as_array()?
        .iter()
        .map(|name| name.as_str().map(str::to_owned))
        .collect()
}

fn list(names: &[String]) -> String {
    serde_json::to_string(names).expect("a list of strings always serializes")
}

fn offered(record: &Path) -> Result<bool, OptionsError> {
    match std::fs::read_to_string(record) {
        Ok(text) => {
            let id = pack_id();
            Ok(text.lines().any(|line| line.trim() == id))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(OptionsError {
            path: record.to_path_buf(),
            source,
        }),
    }
}

fn write_record(record: &Path) -> Result<(), OptionsError> {
    let mut recorded = match std::fs::read_to_string(record) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(source) => {
            return Err(OptionsError {
                path: record.to_path_buf(),
                source,
            });
        }
    };
    let id = pack_id();
    if recorded.lines().any(|line| line.trim() == id) {
        return Ok(());
    }
    if !recorded.is_empty() && !recorded.ends_with('\n') {
        recorded.push('\n');
    }
    recorded.push_str(id);
    recorded.push('\n');
    if let Some(parent) = record.parent() {
        std::fs::create_dir_all(parent).map_err(|source| OptionsError {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(record, recorded).map_err(|source| OptionsError {
        path: record.to_path_buf(),
        source,
    })
}

fn read(path: &Path) -> Result<String, OptionsError> {
    std::fs::read_to_string(path).map_err(|source| OptionsError {
        path: path.to_path_buf(),
        source,
    })
}

// Stage beside the original so interruption cannot truncate the player's settings.
fn replace(path: &Path, text: &str) -> Result<(), OptionsError> {
    let staged = path.with_extension("ravel-pending");
    std::fs::write(&staged, text).map_err(|source| OptionsError {
        path: staged.clone(),
        source,
    })?;
    std::fs::rename(&staged, path).map_err(|source| OptionsError {
        path: path.to_path_buf(),
        source,
    })
}

/// Filesystem failure while reading or replacing player settings.
#[derive(Debug)]
pub struct OptionsError {
    path: PathBuf,
    source: io::Error,
}

impl fmt::Display for OptionsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} could not be read or replaced: {}",
            self.path.display(),
            self.source
        )
    }
}

impl std::error::Error for OptionsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{INSTANCE_DIRECTORY, OPTIONS, SEEDED_PACKS_FILE, Seeded, pack_id, seed_pack};

    const SURROUNDING: &str = "version:3955\nfov:0.0\nincompatibleResourcePacks:[]\nlang:en_us\n";

    fn directory(name: &str) -> PathBuf {
        let at = std::env::temp_dir().join(format!("ravel-options-{}-{name}", std::process::id()));
        if at.exists() {
            std::fs::remove_dir_all(&at).expect("remove stale temporary directory");
        }
        std::fs::create_dir_all(&at).expect("a writable temporary directory");
        at
    }

    fn write_settings(game: &std::path::Path, text: &str) {
        std::fs::write(game.join(OPTIONS), text).expect("settings should be writable");
    }

    fn settings(game: &std::path::Path) -> String {
        std::fs::read_to_string(game.join(OPTIONS)).expect("settings should be readable")
    }

    #[test]
    fn an_empty_list_gains_the_pack_and_every_other_line_is_untouched() {
        let game = directory("empty-list");
        write_settings(
            &game,
            &format!("{SURROUNDING}resourcePacks:[]\nrenderDistance:12\n"),
        );

        assert_eq!(
            seed_pack(&game).expect("a readable instance"),
            Seeded::Added
        );

        assert_eq!(
            settings(&game),
            format!("{SURROUNDING}resourcePacks:[\"file/ravel\"]\nrenderDistance:12\n")
        );
        std::fs::remove_dir_all(game).expect("remove temporary directory");
    }

    #[test]
    fn an_existing_list_keeps_what_it_held_and_gains_the_pack_at_the_end() {
        let game = directory("existing-list");
        write_settings(&game, "resourcePacks:[\"file/mine\",\"vanilla\"]\n");

        assert_eq!(
            seed_pack(&game).expect("a readable instance"),
            Seeded::Added
        );

        assert_eq!(
            settings(&game),
            "resourcePacks:[\"file/mine\",\"vanilla\",\"file/ravel\"]\n"
        );
        std::fs::remove_dir_all(game).expect("remove temporary directory");
    }

    #[test]
    fn a_pack_the_player_removed_after_the_first_seed_is_not_put_back() {
        let game = directory("player-wins");
        write_settings(&game, "resourcePacks:[]\n");
        assert_eq!(
            seed_pack(&game).expect("a readable instance"),
            Seeded::Added
        );

        write_settings(&game, "resourcePacks:[]\n");

        assert_eq!(
            seed_pack(&game).expect("a readable instance"),
            Seeded::AlreadyOffered
        );
        assert_eq!(settings(&game), "resourcePacks:[]\n");
        assert!(
            std::fs::read_to_string(game.join(INSTANCE_DIRECTORY).join(SEEDED_PACKS_FILE))
                .expect("the record should be readable")
                .contains(pack_id())
        );
        std::fs::remove_dir_all(game).expect("remove temporary directory");
    }

    #[test]
    fn settings_stating_no_pack_list_gain_one_and_nothing_else_moves() {
        let game = directory("no-key");
        write_settings(&game, SURROUNDING);

        assert_eq!(
            seed_pack(&game).expect("a readable instance"),
            Seeded::Added
        );

        assert_eq!(
            settings(&game),
            format!("{SURROUNDING}resourcePacks:[\"file/ravel\"]\n")
        );
        std::fs::remove_dir_all(game).expect("remove temporary directory");
    }

    #[test]
    fn only_a_line_that_starts_with_the_key_is_the_one_rewritten() {
        let game = directory("incompatible");
        let near = "incompatibleResourcePacks:[\"file/old\"]\nlegacyresourcePacks:[\"file/x\"]\n";
        write_settings(&game, &format!("{near}resourcePacks:[]\n"));

        assert_eq!(
            seed_pack(&game).expect("a readable instance"),
            Seeded::Added
        );

        assert_eq!(
            settings(&game),
            format!("{near}resourcePacks:[\"file/ravel\"]\n")
        );
        std::fs::remove_dir_all(game).expect("remove temporary directory");
    }

    #[test]
    fn a_list_that_is_not_a_list_of_names_is_left_alone_and_seeded_again_later() {
        let game = directory("unreadable");
        for written in [
            "resourcePacks:[\n",
            "resourcePacks:[1,2]\n",
            "resourcePacks:\n",
        ] {
            write_settings(&game, written);

            let outcome = seed_pack(&game).expect("an unreadable list is not a broken instance");

            assert!(matches!(outcome, Seeded::Unreadable(_)), "{outcome:?}");
            assert_eq!(settings(&game), written, "the file is untouched");
            assert!(
                !game
                    .join(INSTANCE_DIRECTORY)
                    .join(SEEDED_PACKS_FILE)
                    .exists(),
                "nothing is recorded, so a repaired file is still seeded"
            );
        }

        write_settings(&game, "resourcePacks:[]\n");
        assert_eq!(
            seed_pack(&game).expect("a readable instance"),
            Seeded::Added
        );
        std::fs::remove_dir_all(game).expect("remove temporary directory");
    }

    #[test]
    fn an_instance_with_no_settings_file_is_given_none() {
        let game = directory("no-settings");

        assert_eq!(
            seed_pack(&game).expect("a readable instance"),
            Seeded::NoSettings
        );

        assert!(!game.join(OPTIONS).exists(), "no settings file is invented");
        assert!(
            !game
                .join(INSTANCE_DIRECTORY)
                .join(SEEDED_PACKS_FILE)
                .exists()
        );

        write_settings(&game, "resourcePacks:[]\n");
        assert_eq!(
            seed_pack(&game).expect("a readable instance"),
            Seeded::Added
        );
        std::fs::remove_dir_all(game).expect("remove temporary directory");
    }

    #[test]
    fn a_pack_the_player_already_enabled_is_recorded_without_a_rewrite() {
        let game = directory("already-there");
        let written = "resourcePacks:[\"file/ravel\"]\n";
        write_settings(&game, written);

        assert_eq!(
            seed_pack(&game).expect("a readable instance"),
            Seeded::Present
        );

        assert_eq!(settings(&game), written);
        assert_eq!(
            seed_pack(&game).expect("a readable instance"),
            Seeded::AlreadyOffered
        );
        std::fs::remove_dir_all(game).expect("remove temporary directory");
    }

    #[test]
    fn a_record_that_cannot_be_kept_leaves_the_launch_running_and_does_so_every_time() {
        let game = directory("unrecordable");
        write_settings(&game, "resourcePacks:[]\n");
        // A file blocks the directory regardless of permission handling.
        std::fs::write(game.join(INSTANCE_DIRECTORY), b"not a directory").expect("a writable file");

        for attempt in 1..=2 {
            let outcome = seed_pack(&game).expect("a record Ravel cannot keep is not a refusal");

            assert!(
                matches!(outcome, Seeded::Unrecorded(_)),
                "attempt {attempt}: {outcome:?}"
            );
            assert_eq!(
                settings(&game),
                "resourcePacks:[]\n",
                "attempt {attempt}: a record that cannot be kept edits nothing"
            );
        }
        std::fs::remove_dir_all(game).expect("remove temporary directory");
    }

    // Probe permissions so the test cannot pass without arranging a refused write.
    #[cfg(unix)]
    #[test]
    fn a_record_write_that_is_refused_still_leaves_the_pack_switched_on() {
        use std::os::unix::fs::PermissionsExt;

        let game = directory("readonly-record");
        write_settings(&game, "resourcePacks:[]\n");
        let ravel = game.join(INSTANCE_DIRECTORY);
        std::fs::create_dir_all(&ravel).expect("a writable directory");
        std::fs::set_permissions(&ravel, std::fs::Permissions::from_mode(0o555))
            .expect("permissions are settable");
        assert!(
            std::fs::write(ravel.join("probe"), b"x").is_err(),
            "this system ignores the mode bits, so the case below is not arranged"
        );

        let outcome = seed_pack(&game).expect("a record Ravel cannot keep is not a refusal");

        assert!(matches!(outcome, Seeded::Unrecorded(_)), "{outcome:?}");
        assert_eq!(
            settings(&game),
            "resourcePacks:[\"file/ravel\"]\n",
            "the settings edit happened and stands"
        );
        let again = seed_pack(&game).expect("still not a refusal");
        assert!(matches!(again, Seeded::Unrecorded(_)), "{again:?}");
        assert_eq!(settings(&game), "resourcePacks:[\"file/ravel\"]\n");

        std::fs::set_permissions(&ravel, std::fs::Permissions::from_mode(0o755))
            .expect("permissions are settable");
        std::fs::remove_dir_all(game).expect("remove temporary directory");
    }

    #[test]
    fn carriage_returns_are_preserved_on_every_line_this_does_not_write() {
        let game = directory("crlf");
        write_settings(&game, "fov:0.0\r\nresourcePacks:[]\r\nlang:en_us\r\n");

        assert_eq!(
            seed_pack(&game).expect("a readable instance"),
            Seeded::Added
        );

        assert_eq!(
            settings(&game),
            "fov:0.0\r\nresourcePacks:[\"file/ravel\"]\r\nlang:en_us\r\n"
        );
        std::fs::remove_dir_all(game).expect("remove temporary directory");
    }
}
