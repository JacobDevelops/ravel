//! Player files at `config/<owner>.json` and pack overrides at `ravel/config/<owner>.json`.
//! Missing files represent absent layers; writes use a staging file and rename.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::{fmt, fs, io};

use ravel_core::ModId;

use crate::document::{DocumentError, ValueDocument};

/// The directory under a game directory that holds player configuration.
const DIRECTORY: &str = "config";

/// The directory a written pack keeps its settings layer in, under the pack root.
pub const PACK_DIRECTORY: &str = "ravel/config";

/// The suffix a file is written under before it is put in place.
const STAGING_SUFFIX: &str = ".pending";

/// The file one owner's player values live in.
#[must_use]
pub fn path(game_directory: &Path, owner: &ModId) -> PathBuf {
    game_directory.join(DIRECTORY).join(format!("{owner}.json"))
}

/// The file one owner's pack values live in, under a written pack's root.
#[must_use]
pub fn pack_path(root: &Path, owner: &ModId) -> PathBuf {
    root.join(PACK_DIRECTORY).join(format!("{owner}.json"))
}

/// Reads every pack settings document a written pack holds, keyed by owner.
///
/// A missing pack configuration directory returns an empty map.
///
/// # Errors
///
/// Returns the same errors as [`read`], plus [`FileError::Name`] for a JSON file
/// whose name is not a mod id and so names no owner at all.
pub fn read_pack(root: &Path) -> Result<BTreeMap<ModId, ValueDocument>, FileError> {
    let directory = root.join(PACK_DIRECTORY);
    let listing = match fs::read_dir(&directory) {
        Ok(listing) => listing,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => {
            return Err(FileError::Io(format!(
                "cannot read `{}`: {error}",
                directory.display()
            )));
        }
    };

    let mut documents = BTreeMap::new();
    for entry in listing {
        let file = entry
            .map_err(|error| {
                FileError::Io(format!("cannot read `{}`: {error}", directory.display()))
            })?
            .path();
        if file.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let name = file
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let owner = file
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| stem.parse::<ModId>().ok())
            .ok_or(FileError::Name { file: name })?;
        if let Some(document) = read_path(&file, &owner)? {
            documents.insert(owner, document);
        }
    }
    Ok(documents)
}

/// Reads one owner's player document.
///
/// Returns `Ok(None)` for a missing file.
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
/// Both pack and player layers use the same parsing and owner validation.
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
        return Err(FileError::Owner {
            named: owner.to_string(),
            found: document.owner.to_string(),
        });
    }
    Ok(Some(document))
}

/// Writes one owner's player document, atomically.
///
/// The document owner determines the filename; staging and rename avoid partial writes.
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
        // Cleanup is best-effort; preserve the original write failure.
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
    /// The file's own name is not a mod id, so it states nobody's values.
    Name {
        /// The file name, which is the whole of what is wrong with it.
        file: String,
    },
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
            Self::Name { file } => write!(
                formatter,
                "`{file}` is named for no mod, and a values file is named for its owner"
            ),
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
    fn every_pack_document_is_read_back_under_the_owner_its_file_is_named_for() {
        let root = directory("pack-layer");
        let other: ravel_core::ModId = "other_mod".parse().expect("a valid mod id");
        std::fs::create_dir_all(root.join(super::PACK_DIRECTORY))
            .expect("a writable pack directory");
        for owner in [owner(), other.clone()] {
            std::fs::write(
                super::pack_path(&root, &owner),
                ValueDocument::empty(owner).to_json(),
            )
            .expect("the file should be writable");
        }
        // Not a document, and not read as one: a pack directory holds Ravel's own
        // files and this is what says the reader takes only the ones it named.
        std::fs::write(
            root.join(super::PACK_DIRECTORY).join("notes.txt"),
            "ignored",
        )
        .expect("the file should be writable");

        let documents = super::read_pack(&root).expect("the pack layer should be readable");

        assert_eq!(
            documents.keys().cloned().collect::<Vec<_>>(),
            [owner(), other],
            "every document, keyed by its owner, and nothing the reader was not named for"
        );
    }

    #[test]
    fn a_pack_that_states_no_overrides_has_no_directory_and_that_is_not_an_error() {
        let root = directory("pack-absent");

        assert_eq!(super::read_pack(&root), Ok(BTreeMap::new()));
    }

    #[test]
    fn a_pack_document_naming_one_owner_and_stating_another_refuses_the_whole_layer() {
        let root = directory("pack-mismatched");
        let other: ravel_core::ModId = "other_mod".parse().expect("a valid mod id");
        std::fs::create_dir_all(root.join(super::PACK_DIRECTORY))
            .expect("a writable pack directory");
        std::fs::write(
            super::pack_path(&root, &owner()),
            ValueDocument::empty(other.clone()).to_json(),
        )
        .expect("the file should be writable");

        assert_eq!(
            super::read_pack(&root),
            Err(FileError::Owner {
                named: owner().to_string(),
                found: other.to_string(),
            })
        );
    }

    #[test]
    fn a_pack_document_whose_name_is_not_a_mod_id_states_nobodys_values() {
        let root = directory("pack-unnamed");
        std::fs::create_dir_all(root.join(super::PACK_DIRECTORY))
            .expect("a writable pack directory");
        std::fs::write(
            root.join(super::PACK_DIRECTORY).join("Not A Mod.json"),
            ValueDocument::empty(owner()).to_json(),
        )
        .expect("the file should be writable");

        assert_eq!(
            super::read_pack(&root),
            Err(FileError::Name {
                file: "Not A Mod.json".to_owned(),
            })
        );
    }

    #[test]
    fn a_file_lands_where_the_java_ecosystem_puts_player_configuration() {
        let game = std::path::Path::new("/game");

        assert_eq!(
            path(game, &owner()),
            std::path::Path::new("/game/config/example_mod.json")
        );
        assert_eq!(
            super::pack_path(game, &owner()),
            std::path::Path::new("/game/ravel/config/example_mod.json")
        );
        assert!(
            FileError::Name {
                file: "Not A Mod.json".to_owned(),
            }
            .to_string()
            .contains("Not A Mod.json")
        );
    }
}
