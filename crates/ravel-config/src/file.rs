//! Where a player's own configuration lives on disk, and how it is written.
//!
//! One file per owner at `<game directory>/config/<owner>.json`, which is where
//! the Java ecosystem puts player configuration, what players expect to find and
//! what pack authors expect to ship. `<owner>` is the canonical mod id, so
//! nothing new is named here.
//!
//! The file half lives beside [`ValueDocument`] rather than in the crate that
//! writes it, because the file *is* that document: splitting the shape from the
//! place would mean two crates agreeing on a filename and a layout forever. What
//! does not live here is which directory to use — the game directory is a fact
//! about the running process, so it is a parameter and its resolution belongs to
//! whoever knows the process.
//!
//! # A missing file is not an error
//!
//! It means the player has changed nothing, which
//! [`crate::LayeredConfiguration::resolve`] already handles as an absent layer.
//! Only a file that exists and cannot be used is a failure.

use std::path::{Path, PathBuf};
use std::{fmt, fs, io};

use ravel_core::ModId;

use crate::document::{DocumentError, ValueDocument};

/// The directory under a game directory that holds player configuration.
const DIRECTORY: &str = "config";

/// The suffix a file is written under before it is put in place.
const STAGING_SUFFIX: &str = ".pending";

/// The file one owner's player values live in.
#[must_use]
pub fn path(game_directory: &Path, owner: &ModId) -> PathBuf {
    game_directory.join(DIRECTORY).join(format!("{owner}.json"))
}

/// Reads one owner's player document.
///
/// Returns `Ok(None)` when the player has changed nothing, which is the ordinary
/// case for most mods on most launches.
///
/// # Errors
///
/// Returns [`FileError::Io`] when a file that exists cannot be read,
/// [`FileError::Document`] when it is not a document this crate accepts, and
/// [`FileError::Owner`] when it is a valid document belonging to somebody else.
pub fn read(game_directory: &Path, owner: &ModId) -> Result<Option<ValueDocument>, FileError> {
    read_path(&path(game_directory, owner), owner)
}

/// Reads one owner's values from an explicitly selected document path.
///
/// This is the pack-facing counterpart to [`read`]: a generated composition
/// resolves the relative path pinned by its selected pack manifest, while this
/// crate keeps document parsing and owner validation identical for both layers.
///
/// # Errors
///
/// Returns the same errors as [`read`].
pub fn read_path(file: &Path, owner: &ModId) -> Result<Option<ValueDocument>, FileError> {
    let text = match fs::read_to_string(file) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(FileError::Io(format!(
                "cannot read `{}`: {error}",
                file.display()
            )));
        }
    };

    let document = ValueDocument::parse(&text).map_err(FileError::Document)?;
    if &document.owner != owner {
        // The name on the file and the name inside it disagree, so one of them
        // is a lie and applying either would give a mod somebody else's values.
        return Err(FileError::Owner {
            named: owner.to_string(),
            found: document.owner.to_string(),
        });
    }
    Ok(Some(document))
}

/// Writes one owner's player document, atomically.
///
/// The owner comes from the document rather than the caller, so the name on the
/// file and the name inside it cannot disagree.
///
/// The write goes to a staging file beside the target and is then renamed onto
/// it, the same repair the marker socket uses. A rename within a directory
/// replaces the target in one step, so a crash mid-write leaves either the old
/// file or the new one and never half of either. Truncating in place would mean
/// a crash at the wrong moment costs a player every setting for that mod.
///
/// # Errors
///
/// Returns [`FileError::Io`] when the directory cannot be created or the file
/// cannot be written or put in place.
pub fn write(game_directory: &Path, document: &ValueDocument) -> Result<(), FileError> {
    let file = path(game_directory, &document.owner);
    let directory = game_directory.join(DIRECTORY);
    fs::create_dir_all(&directory).map_err(|error| {
        FileError::Io(format!("cannot create `{}`: {error}", directory.display()))
    })?;

    let mut staging = file.clone().into_os_string();
    staging.push(STAGING_SUFFIX);
    let staging = PathBuf::from(staging);

    let mut text = document.to_json();
    text.push('\n');
    stage(&staging, &file, &text).inspect_err(|_| {
        // A staging file left behind would be written over by the next save, but
        // it would also sit in the player's config directory looking like data.
        let _ = fs::remove_file(&staging);
    })
}

/// Writes the staging file and renames it onto the target.
fn stage(staging: &Path, file: &Path, text: &str) -> Result<(), FileError> {
    fs::write(staging, text)
        .map_err(|error| FileError::Io(format!("cannot write `{}`: {error}", staging.display())))?;
    fs::rename(staging, file).map_err(|error| {
        FileError::Io(format!("cannot put `{}` in place: {error}", file.display()))
    })
}

/// Why a player's configuration file could not be used.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileError {
    /// The file exists but could not be read, or could not be written.
    Io(String),
    /// The file is not a document this crate accepts.
    Document(DocumentError),
    /// The file is a valid document that belongs to another owner.
    Owner {
        /// The owner the file name claims.
        named: String,
        /// The owner the document inside it claims.
        found: String,
    },
}

impl fmt::Display for FileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(detail) => formatter.write_str(detail),
            Self::Document(error) => write!(formatter, "{error}"),
            Self::Owner { named, found } => write!(
                formatter,
                "`{named}.json` states values for `{found}` rather than `{named}`"
            ),
        }
    }
}

impl std::error::Error for FileError {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{FileError, path, read, write};
    use crate::document::ValueDocument;

    fn owner() -> ravel_core::ModId {
        "example_mod".parse().expect("a valid mod id")
    }

    fn directory(name: &str) -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("ravel-config-file-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("a writable temporary directory");
        directory
    }

    fn document(value: serde_json::Value) -> ValueDocument {
        let mut values = BTreeMap::new();
        values.insert("render.distance".parse().expect("a valid key"), value);
        ValueDocument {
            schema_version: crate::SchemaVersion::CURRENT,
            owner: owner(),
            values,
        }
    }

    #[test]
    fn a_document_is_read_back_exactly_as_it_was_written() {
        let game = directory("round-trip");
        let written = document(serde_json::json!(12));

        write(&game, &written).expect("the document should be writable");

        assert_eq!(
            read(&game, &owner()).expect("the file should be readable"),
            Some(written)
        );
    }

    #[test]
    fn a_player_who_has_changed_nothing_has_no_file_and_that_is_not_an_error() {
        let game = directory("absent");

        assert_eq!(read(&game, &owner()), Ok(None));
    }

    #[test]
    fn a_second_write_replaces_the_first_and_leaves_no_staging_file() {
        let game = directory("replace");

        write(&game, &document(serde_json::json!(4))).expect("the first write should land");
        write(&game, &document(serde_json::json!(16))).expect("the second write should land");

        assert_eq!(
            read(&game, &owner())
                .expect("readable")
                .expect("present")
                .values
                .get(&"render.distance".parse().expect("a valid key")),
            Some(&serde_json::json!(16))
        );
        let left_behind: Vec<_> = std::fs::read_dir(game.join("config"))
            .expect("the config directory should exist")
            .filter_map(|entry| entry.ok().map(|entry| entry.file_name()))
            .filter(|name| name.to_string_lossy().ends_with(super::STAGING_SUFFIX))
            .collect();
        assert!(
            left_behind.is_empty(),
            "a completed write leaves no staging file: {left_behind:?}"
        );
    }

    #[test]
    fn a_file_naming_one_owner_and_stating_another_is_refused_rather_than_applied() {
        let game = directory("mismatched");
        let other: ravel_core::ModId = "other_mod".parse().expect("a valid mod id");
        std::fs::create_dir_all(game.join("config")).expect("a writable config directory");
        std::fs::write(
            path(&game, &owner()),
            ValueDocument::empty(other.clone()).to_json(),
        )
        .expect("the file should be writable");

        assert_eq!(
            read(&game, &owner()),
            Err(FileError::Owner {
                named: owner().to_string(),
                found: other.to_string(),
            })
        );
    }

    #[test]
    fn a_file_that_is_not_a_document_is_refused_rather_than_treated_as_empty() {
        let game = directory("malformed");
        std::fs::create_dir_all(game.join("config")).expect("a writable config directory");
        std::fs::write(path(&game, &owner()), "{ not json").expect("the file should be writable");

        assert!(matches!(read(&game, &owner()), Err(FileError::Document(_))));
    }

    #[test]
    fn a_file_lands_where_the_java_ecosystem_puts_player_configuration() {
        let game = std::path::Path::new("/game");

        assert_eq!(
            path(game, &owner()),
            std::path::Path::new("/game/config/example_mod.json")
        );
    }
}
