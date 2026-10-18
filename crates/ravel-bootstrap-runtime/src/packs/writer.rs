//! Minecraft resource and data packs, distinct from the modpacks declared by `ravel-pack`.
//! The writer owns both the installed tree and its sibling staging path.

use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use ravel_loader::Supplied;

/// The file every pack declares its format in.
const PACK_METADATA: &str = "pack.mcmeta";

/// The half of a supplied set a pack carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Half {
    /// Everything under `assets/`, which a client reads as a resource pack.
    Assets,
    /// Everything under `data/`, which a world reads as a data pack.
    Data,
}

impl Half {
    /// The path prefix this half owns.
    const fn prefix(self) -> &'static str {
        match self {
            Self::Assets => "assets/",
            Self::Data => "data/",
        }
    }
}

/// Writes one half of `supplied` under `root` using version-specific `metadata`.
///
/// Identical trees are left untouched. Changed trees are staged in a sibling directory,
/// then replace the old pack.
///
/// # Errors
///
/// Returns a [`GamePackError`] for any filesystem refusal; the caller must refuse the launch.
pub(super) fn write_game_pack(
    root: &Path,
    supplied: &Supplied,
    half: Half,
    metadata: &str,
) -> Result<Written, GamePackError> {
    // Declarations include overridden offers; write each path only once.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut wanted: Vec<(String, Vec<u8>)> =
        vec![(PACK_METADATA.to_owned(), metadata.as_bytes().to_vec())];
    seen.insert(PACK_METADATA);
    for resource in supplied.set().declared() {
        let path = &resource.path;
        if !path.as_str().starts_with(half.prefix()) || !seen.insert(path.as_str()) {
            continue;
        }
        // Read the winning supplier's bytes, not this declaration's bytes.
        let Some(bytes) = supplied.bytes(path) else {
            continue;
        };
        wanted.push((path.as_str().to_owned(), bytes.to_vec()));
    }
    wanted.sort_by(|(left, _), (right, _)| left.cmp(right));

    let written = Written {
        root: root.to_path_buf(),
        paths: wanted.iter().map(|(path, _)| path.clone()).collect(),
        changed: !holds(root, &wanted),
    };
    if !written.changed {
        return Ok(written);
    }

    let staged = staging(root);
    if staged.exists() {
        remove(&staged)?;
    }
    create(&staged, &staged)?;
    for (path, bytes) in &wanted {
        let file = staged.join(path);
        if let Some(parent) = file.parent() {
            create(parent, &staged)?;
        }
        write(&file, &staged, bytes)?;
    }

    if root.exists() {
        remove(root)?;
    }
    std::fs::rename(&staged, root).map_err(|source| GamePackError {
        path: root.to_path_buf(),
        action: Action::Replace,
        source,
    })?;
    Ok(written)
}

/// Returns the game pack root and sibling staging directory the writer may replace.
///
/// Callers checking filesystem boundaries must check both paths before writing.
#[must_use]
pub(crate) fn replaced_paths(root: &Path) -> [PathBuf; 2] {
    [root.to_path_buf(), staging(root)]
}

// Stage beside the pack so staging files cannot become pack contents.
fn staging(root: &Path) -> PathBuf {
    let mut name = root.file_name().unwrap_or_default().to_os_string();
    name.push(".pending");
    root.with_file_name(name)
}

// Extra files or unreadable content also require replacement.
fn holds(root: &Path, wanted: &[(String, Vec<u8>)]) -> bool {
    let mut found = Vec::new();
    if !collect(root, root, &mut found) {
        return false;
    }
    found.sort();
    if found.len() != wanted.len() {
        return false;
    }
    for ((path, bytes), on_disk) in wanted.iter().zip(&found) {
        if path != on_disk {
            return false;
        }
        match std::fs::read(root.join(on_disk)) {
            Ok(read) if &read == bytes => {}
            _ => return false,
        }
    }
    true
}

/// Every file below `at`, relative to `root`, or false if it cannot be walked.
fn collect(at: &Path, root: &Path, into: &mut Vec<String>) -> bool {
    let Ok(entries) = std::fs::read_dir(at) else {
        return false;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            return false;
        };
        let path = entry.path();
        if path.is_dir() {
            if !collect(&path, root, into) {
                return false;
            }
        } else {
            let Ok(relative) = path.strip_prefix(root) else {
                return false;
            };
            into.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    true
}

fn remove(path: &Path) -> Result<(), GamePackError> {
    std::fs::remove_dir_all(path).map_err(|source| GamePackError {
        path: path.to_path_buf(),
        action: Action::Replace,
        source,
    })
}

fn create(path: &Path, root: &Path) -> Result<(), GamePackError> {
    std::fs::create_dir_all(path).map_err(|source| GamePackError {
        path: relative(path, root),
        action: Action::Create,
        source,
    })
}

fn write(path: &Path, root: &Path, bytes: &[u8]) -> Result<(), GamePackError> {
    std::fs::write(path, bytes).map_err(|source| GamePackError {
        path: relative(path, root),
        action: Action::Write,
        source,
    })
}

/// The path a message names, shortened to the pack when it sits inside one.
fn relative(path: &Path, root: &Path) -> PathBuf {
    path.strip_prefix(root).map_or_else(
        |_| path.to_path_buf(),
        |inside| Path::new(root.file_name().unwrap_or_default()).join(inside),
    )
}

/// What one written pack holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Written {
    root: PathBuf,
    paths: Vec<String>,
    changed: bool,
}

impl Written {
    /// Returns the directory the pack occupies.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns every file in the pack, in path order, `pack.mcmeta` included.
    #[must_use]
    pub fn paths(&self) -> &[String] {
        &self.paths
    }

    /// Whether a pack was created or its contents changed.
    #[must_use]
    pub const fn changed(&self) -> bool {
        self.changed
    }
}

/// What the filesystem refused while a pack was being written.
#[derive(Debug)]
pub struct GamePackError {
    path: PathBuf,
    action: Action,
    source: io::Error,
}

/// Which step of writing a pack was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    /// The previous pack could not be removed.
    Replace,
    /// A directory inside the pack could not be made.
    Create,
    /// A file could not be written.
    Write,
}

impl fmt::Display for GamePackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let verb = match self.action {
            Action::Replace => "the previous pack at",
            Action::Create => "a directory at",
            Action::Write => "a file at",
        };
        write!(
            formatter,
            "{verb} {} could not be written: {}",
            self.path.display(),
            self.source
        )
    }
}

impl std::error::Error for GamePackError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(test)]
mod tests;
