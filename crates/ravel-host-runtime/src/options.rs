use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// The file a player's client keeps its settings in.
const OPTIONS: &str = "options.txt";

/// The key that lists the packs a player has turned on.
const KEY: &str = "resourcePacks:";

/// Ravel's own directory inside an instance, beside `mods` and `host`.
const RAVEL: &str = "ravel";

/// Where a seed is recorded, so it is offered once rather than every launch.
const RECORD: &str = "seeded-packs";

/// The id the game's folder pack source gives Ravel's written pack.
///
/// The prefix is the game's rather than Ravel's: the class that lists a pack
/// directory builds every id it finds by putting `file/` in front of the
/// directory's name, so this is what the pack Ravel writes is called in the list a
/// player's settings hold.
pub const PACK_ID: &str = "file/ravel";

/// What seeding the pack into a player's settings did, or did not do.
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

/// Adds Ravel's pack to a player's enabled list, once, and never again.
///
/// The pack Ravel writes is a folder pack like any other, and a folder pack the
/// player has not enabled is a pack the game finds and does not read. So the first
/// launch that finds a settings file puts the id in the list, and records that it
/// did — after which the list is the player's. Somebody who turns the pack off
/// means it, and a loader that turned it back on every launch would be arguing
/// with them.
///
/// # What is never done
///
/// **No settings file is created.** A file this writes would carry no `version`
/// line, and the game reads that line to decide which of its own migrations to run
/// over the rest, so a one-line file invites the game to migrate a document from a
/// version it never came from. A fresh instance gets its settings from the game on
/// its first run and the seed lands on the next one, which costs a launch and
/// risks nothing that belongs to the player.
///
/// **A list that is not a list is not repaired.** It is somebody else's file in a
/// shape this does not understand, so it is left exactly as it is and the launch
/// continues without the pack enabled. Nothing is recorded either, so an instance
/// whose settings are fixed later is seeded then.
///
/// Every other line is preserved byte for byte, including its line ending, because
/// the rest of that file is the player's keybindings and their video settings and
/// none of it is Ravel's to reformat.
///
/// # Errors
///
/// Returns an [`OptionsError`] when a settings file that exists cannot be read,
/// when it is not UTF-8, or when the rewrite cannot be put in place. Those are a
/// broken instance rather than an unexpected shape, and the launch is failed on
/// them; the rewrite is staged and renamed, so a failed one has edited nothing.
///
/// Reading or writing Ravel's own record of the offer is never one of them. See
/// [`record_offer`].
pub fn seed_pack(game_directory: &Path) -> Result<Seeded, OptionsError> {
    let record = game_directory.join(RAVEL).join(RECORD);
    match offered(&record) {
        Ok(true) => return Ok(Seeded::AlreadyOffered),
        Ok(false) => {}
        // A record that cannot be read is not a reason to refuse a launch, and it
        // is not a reason to edit somebody's settings either. Nothing is touched
        // and the player is told why the pack is not on.
        Err(refused) => return Ok(Seeded::Unrecorded(refused.to_string())),
    }

    let settings = game_directory.join(OPTIONS);
    if !settings.is_file() {
        return Ok(Seeded::NoSettings);
    }
    let text = read(&settings)?;

    let Some(line) = enabled_line(&text) else {
        // No such key at all, which is a settings file the game has not written
        // this option into yet. Appending it states the one thing Ravel has to
        // say and touches nothing that is already there.
        let ending = if text.ends_with('\n') { "" } else { "\n" };
        let appended = format!("{text}{ending}{KEY}{}\n", list(&[PACK_ID.to_owned()]));
        replace(&settings, &appended)?;
        return Ok(record_offer(&record, Seeded::Added));
    };

    let Some(mut enabled) = parse(line.value) else {
        return Ok(Seeded::Unreadable(line.value.to_owned()));
    };
    if enabled.iter().any(|name| name == PACK_ID) {
        return Ok(record_offer(&record, Seeded::Present));
    }

    enabled.push(PACK_ID.to_owned());
    let mut rewritten = String::with_capacity(text.len() + PACK_ID.len() + 4);
    rewritten.push_str(&text[..line.at]);
    rewritten.push_str(KEY);
    rewritten.push_str(&list(&enabled));
    rewritten.push_str(&text[line.end..]);
    replace(&settings, &rewritten)?;
    Ok(record_offer(&record, Seeded::Added))
}

/// Records that the offer was made, and answers `made` when it could be.
///
/// **A record that cannot be written never fails a launch.** It used to, and the
/// failure was permanent in the worst way: the settings had already been edited
/// when the record was attempted, so the launch aborted with the pack switched on
/// and no record of it, and every launch after that reached the same line and
/// aborted the same way. An instance whose `ravel` directory a player cannot write
/// to would never have started again.
///
/// The record is still written after the settings rather than before them. Before
/// would leave a record of an offer that a failed settings write never made, and
/// the pack would then never be switched on at all. After, the worst case is that
/// the offer is made again next launch, which changes nothing while the pack is in
/// the list — and the notice says so, because a player who turns the pack off is
/// owed the warning that it may come back.
fn record_offer(record: &Path, made: Seeded) -> Seeded {
    match write_record(record) {
        Ok(()) => made,
        Err(refused) => Seeded::Unrecorded(refused.to_string()),
    }
}

/// Where one key's value sits in the settings text.
struct Line<'a> {
    /// The byte the key starts at.
    at: usize,
    /// The byte after the value, which is the line ending or the end of the file.
    end: usize,
    /// The value as written, with no line ending.
    value: &'a str,
}

/// Finds the enabled-pack line, if the settings file states one.
///
/// The key has to start a line. A settings file states `incompatibleResourcePacks`
/// too, and a search that took the first occurrence of the name anywhere would
/// rewrite whichever of the two came first.
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

/// Reads a written list of names, or nothing when it is not one.
fn parse(value: &str) -> Option<Vec<String>> {
    let parsed: serde_json::Value = serde_json::from_str(value).ok()?;
    parsed
        .as_array()?
        .iter()
        .map(|name| name.as_str().map(str::to_owned))
        .collect()
}

/// Writes a list of names the way the game writes one.
fn list(names: &[String]) -> String {
    serde_json::to_string(names).expect("a list of strings always serializes")
}

/// Whether this pack has been offered to this instance before.
fn offered(record: &Path) -> Result<bool, OptionsError> {
    match std::fs::read_to_string(record) {
        Ok(text) => Ok(text.lines().any(|line| line.trim() == PACK_ID)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(OptionsError {
            path: record.to_path_buf(),
            source,
        }),
    }
}

/// Records that this pack has been offered, adding to what is already recorded.
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
    if recorded.lines().any(|line| line.trim() == PACK_ID) {
        return Ok(());
    }
    if !recorded.is_empty() && !recorded.ends_with('\n') {
        recorded.push('\n');
    }
    recorded.push_str(PACK_ID);
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

/// Puts new settings in place through a neighbouring file.
///
/// Written beside the original and renamed over it, so a launch interrupted
/// halfway leaves the player's settings as they were rather than truncated. It is
/// the same staging `ravel-config` uses for a document it owns, and this is a
/// document it does not.
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

/// What the filesystem refused while a player's settings were being read.
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

    use super::{OPTIONS, PACK_ID, RAVEL, RECORD, Seeded, seed_pack};

    /// The lines around the one under test, which every case requires untouched.
    const SURROUNDING: &str = "version:3955\nfov:0.0\nincompatibleResourcePacks:[]\nlang:en_us\n";

    fn directory(name: &str) -> PathBuf {
        let at = std::env::temp_dir().join(format!("ravel-options-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        std::fs::create_dir_all(&at).expect("a writable temporary directory");
        at
    }

    fn write_settings(game: &std::path::Path, text: &str) {
        std::fs::write(game.join(OPTIONS), text).expect("settings should be writable");
    }

    fn settings(game: &std::path::Path) -> String {
        std::fs::read_to_string(game.join(OPTIONS)).expect("settings should be readable")
    }

    /// The ordinary case, and the byte-preservation that makes it safe.
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
        let _ = std::fs::remove_dir_all(game);
    }

    /// A player's own packs keep their order, and Ravel's goes last.
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
        let _ = std::fs::remove_dir_all(game);
    }

    /// The decision this exists for: a player who turns the pack off keeps it off.
    #[test]
    fn a_pack_the_player_removed_after_the_first_seed_is_not_put_back() {
        let game = directory("player-wins");
        write_settings(&game, "resourcePacks:[]\n");
        assert_eq!(
            seed_pack(&game).expect("a readable instance"),
            Seeded::Added
        );

        // The player turns it off.
        write_settings(&game, "resourcePacks:[]\n");

        assert_eq!(
            seed_pack(&game).expect("a readable instance"),
            Seeded::AlreadyOffered
        );
        assert_eq!(settings(&game), "resourcePacks:[]\n");
        assert!(
            std::fs::read_to_string(game.join(RAVEL).join(RECORD))
                .expect("the record should be readable")
                .contains(PACK_ID)
        );
        let _ = std::fs::remove_dir_all(game);
    }

    /// A settings file with no such key gains one line and keeps the rest.
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
        let _ = std::fs::remove_dir_all(game);
    }

    /// The key has to start its line, and the settings file is full of near misses.
    ///
    /// `incompatibleResourcePacks` is the real one, and it differs by a capital,
    /// so on its own it would let a search that matched the name anywhere in a
    /// line pass. The second line is not a key the game writes and it is the one
    /// that matters: it ends in the exact bytes this looks for, so anything short
    /// of anchoring at the start of the line rewrites the wrong line.
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
        let _ = std::fs::remove_dir_all(game);
    }

    /// A list nothing can read is left exactly as it is, and nothing is recorded.
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
                !game.join(RAVEL).join(RECORD).exists(),
                "nothing is recorded, so a repaired file is still seeded"
            );
        }

        // Repaired, and now it seeds.
        write_settings(&game, "resourcePacks:[]\n");
        assert_eq!(
            seed_pack(&game).expect("a readable instance"),
            Seeded::Added
        );
        let _ = std::fs::remove_dir_all(game);
    }

    /// An instance with no settings yet is left with none, and is seeded later.
    #[test]
    fn an_instance_with_no_settings_file_is_given_none() {
        let game = directory("no-settings");

        assert_eq!(
            seed_pack(&game).expect("a readable instance"),
            Seeded::NoSettings
        );

        assert!(!game.join(OPTIONS).exists(), "no settings file is invented");
        assert!(!game.join(RAVEL).join(RECORD).exists());

        write_settings(&game, "resourcePacks:[]\n");
        assert_eq!(
            seed_pack(&game).expect("a readable instance"),
            Seeded::Added
        );
        let _ = std::fs::remove_dir_all(game);
    }

    /// A player who enabled it themselves is recorded, not written to.
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
        let _ = std::fs::remove_dir_all(game);
    }

    /// A record Ravel cannot keep never fails a launch, and never fails it twice.
    ///
    /// This is the shape the first version had: the settings were edited and the
    /// record written after, and a record that could not be written aborted the
    /// launch — with the pack already switched on, so every launch after it
    /// reached the same line and aborted identically. An instance whose `ravel`
    /// directory had turned read-only would never have started again.
    ///
    /// Two calls, because one proves nothing about a failure that repeats.
    #[test]
    fn a_record_that_cannot_be_kept_leaves_the_launch_running_and_does_so_every_time() {
        let game = directory("unrecordable");
        write_settings(&game, "resourcePacks:[]\n");
        // A file where Ravel's own directory belongs, so nothing under it can be
        // read or written and the failure needs no permission bits to arrange.
        std::fs::write(game.join(RAVEL), b"not a directory").expect("a writable file");

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
        let _ = std::fs::remove_dir_all(game);
    }

    /// The half of the same rule where the settings were already edited.
    ///
    /// The record is written after the settings, so this is the case the old code
    /// aborted on. It needs the directory to exist and refuse a write, which is
    /// permission bits and nothing else — and the probe below is what stops the
    /// test passing on a system that ignores them.
    #[cfg(unix)]
    #[test]
    fn a_record_write_that_is_refused_still_leaves_the_pack_switched_on() {
        use std::os::unix::fs::PermissionsExt;

        let game = directory("readonly-record");
        write_settings(&game, "resourcePacks:[]\n");
        let ravel = game.join(RAVEL);
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
        // And the next launch finds the pack already there rather than adding it
        // twice, so repeating the offer costs nothing.
        let again = seed_pack(&game).expect("still not a refusal");
        assert!(matches!(again, Seeded::Unrecorded(_)), "{again:?}");
        assert_eq!(settings(&game), "resourcePacks:[\"file/ravel\"]\n");

        std::fs::set_permissions(&ravel, std::fs::Permissions::from_mode(0o755))
            .expect("permissions are settable");
        let _ = std::fs::remove_dir_all(game);
    }

    /// Windows line endings survive, because the file is spliced rather than rebuilt.
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
        let _ = std::fs::remove_dir_all(game);
    }
}
