use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use ravel_core::RelativePath;

use crate::validate::ValidatedPack;

impl ValidatedPack {
    /// Writes every file the pack holds under `directory`.
    ///
    /// The directory is the caller's: this creates the intermediate directories a
    /// path needs and writes each file, and it removes nothing. A pack written into
    /// a directory that already holds a different pack is two packs mixed together,
    /// which is a mistake a caller makes and not one this could detect — so the
    /// callers that care hand it an empty directory.
    ///
    /// Writing is where the determinism claim becomes checkable rather than argued.
    /// [`ValidatedPack::files`] is already ordered and deduplicated in memory, and a
    /// same-process comparison of two of those values cannot see a difference that
    /// depends on the process — a hash seed, an environment variable, a locale, a
    /// path baked into a byte. Two directories written by two processes can.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError`] naming the path that could not be created or written.
    /// Nothing is rolled back: a refusal partway leaves the files written so far,
    /// because a half-written directory a caller was told about is safer than one it
    /// was not, and a pack is written into a directory the caller can discard.
    pub fn write(&self, directory: &Path) -> Result<(), WriteError> {
        for file in self.files() {
            let target = directory.join(file.path().as_str());
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|source| WriteError {
                    path: file.path().clone(),
                    source,
                })?;
            }
            fs::write(&target, file.bytes()).map_err(|source| WriteError {
                path: file.path().clone(),
                source,
            })?;
        }
        Ok(())
    }
}

/// Why one of a pack's files could not be written.
#[derive(Debug)]
pub struct WriteError {
    /// The path inside the pack.
    ///
    /// The local path it was being written to is deliberately not kept. A pack is
    /// written on a player's machine and this sentence reaches a report, so the type
    /// holds the part that identifies the file and not the part that identifies the
    /// machine.
    path: RelativePath,
    /// What the filesystem said.
    source: io::Error,
}

impl WriteError {
    /// Returns the path inside the pack that could not be written.
    #[must_use]
    pub const fn path(&self) -> &RelativePath {
        &self.path
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "`{}` could not be written: {}",
            self.path.as_str(),
            self.source
        )
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::WriteError;

    #[test]
    fn a_write_refusal_names_the_pack_relative_path_and_nothing_local() {
        let error = WriteError {
            path: ravel_core::RelativePath::try_from("data/example_pack/x.json".to_owned())
                .expect("a legal relative path"),
            source: std::io::Error::other("the disk said no"),
        };
        let text = error.to_string();
        assert!(text.contains("data/example_pack/x.json"), "{text}");
        assert!(text.contains("the disk said no"), "{text}");
        assert_eq!(error.path().as_str(), "data/example_pack/x.json");
        assert!(std::error::Error::source(&error).is_some());
    }
}
