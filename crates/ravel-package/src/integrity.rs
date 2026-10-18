use std::fmt;
use std::fs::{self, File};
use std::io::{ErrorKind, Read};
use std::path::Path;

use ravel_core::{FailureKind, StructuredFailure};
use sha2::{Digest, Sha256};

use crate::manifest::{PackageManifest, Provenance, SourceFile};
use crate::values::{RelativePath, Sha256Digest};

const READ_BUFFER_BYTES: usize = 64 * 1024;

/// Verifies a declared source file's digest beneath the package root.
///
/// # Errors
///
/// Returns an error for missing, unreadable, escaping, or digest-mismatched files.
pub fn verify_source_file(root: &Path, source: &SourceFile) -> Result<(), IntegrityError> {
    let file = source.path.file_name();
    let canonical_root = canonical_root(root, file)?;
    verify_file(&canonical_root, &source.path, source.sha256)
}

/// Verifies the provenance record against the digest its manifest records.
///
/// # Errors
///
/// Returns the same failures as [`verify_source_file`].
pub fn verify_provenance(root: &Path, provenance: &Provenance) -> Result<(), IntegrityError> {
    let file = provenance.record.file_name();
    let canonical_root = canonical_root(root, file)?;
    verify_file(&canonical_root, &provenance.record, provenance.sha256)
}

/// Verifies every declared portable source file, then the provenance record.
///
/// # Errors
///
/// Returns the first failure encountered, in declaration order.
pub fn verify_package(root: &Path, manifest: &PackageManifest) -> Result<(), IntegrityError> {
    let first_file = manifest.source.files.first().map_or_else(
        || manifest.provenance.record.file_name(),
        |source| source.path.file_name(),
    );
    let canonical_root = canonical_root(root, first_file)?;
    for source in &manifest.source.files {
        verify_file(&canonical_root, &source.path, source.sha256)?;
    }
    verify_file(
        &canonical_root,
        &manifest.provenance.record,
        manifest.provenance.sha256,
    )
}

fn canonical_root(root: &Path, file: &str) -> Result<std::path::PathBuf, IntegrityError> {
    fs::canonicalize(root).map_err(|error| IntegrityError::from_io(file, error))
}

fn verify_file(
    root: &Path,
    path: &RelativePath,
    declared: Sha256Digest,
) -> Result<(), IntegrityError> {
    let file = path.file_name();
    let resolved = fs::canonicalize(root.join(path.as_str()))
        .map_err(|error| IntegrityError::from_io(file, error))?;
    if resolved == root || !resolved.starts_with(root) {
        return Err(IntegrityError::OutsidePackage {
            file: file.to_owned(),
        });
    }

    // Canonicalize-then-open still has a residual TOCTOU race if links change here.
    let actual = digest_file(&resolved, file)?;
    if actual == declared {
        return Ok(());
    }
    Err(IntegrityError::Mismatch {
        file: file.to_owned(),
        declared,
        actual,
    })
}

fn digest_file(path: &Path, file: &str) -> Result<Sha256Digest, IntegrityError> {
    let mut reader = File::open(path).map_err(|error| IntegrityError::from_io(file, error))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; READ_BUFFER_BYTES];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| IntegrityError::from_io(file, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let mut digest = [0_u8; 32];
    digest.copy_from_slice(&hasher.finalize());
    Ok(Sha256Digest::from_bytes(digest))
}

/// A declared file is missing, unreadable, outside the package, or has the wrong digest.
#[derive(Debug)]
pub enum IntegrityError {
    /// The manifest declares a file the package does not contain.
    Missing {
        /// The declared file name.
        file: String,
    },
    /// The file exists and could not be read.
    Unreadable {
        /// The declared file name.
        file: String,
        /// What the operating system reported.
        source: std::io::Error,
    },
    /// The declared path resolves outside the package directory.
    OutsidePackage {
        /// The declared file name.
        file: String,
    },
    /// The file's contents hash to something other than the recorded digest.
    Mismatch {
        /// The declared file name.
        file: String,
        /// The digest the manifest records.
        declared: Sha256Digest,
        /// The digest the file actually has.
        actual: Sha256Digest,
    },
}

impl IntegrityError {
    fn from_io(file: &str, error: std::io::Error) -> Self {
        if error.kind() == ErrorKind::NotFound {
            return Self::Missing {
                file: file.to_owned(),
            };
        }
        Self::Unreadable {
            file: file.to_owned(),
            source: error,
        }
    }

    /// Returns the declared file name the failure is about.
    #[must_use]
    pub fn file(&self) -> &str {
        match self {
            Self::Missing { file }
            | Self::Unreadable { file, .. }
            | Self::OutsidePackage { file }
            | Self::Mismatch { file, .. } => file,
        }
    }
}

impl fmt::Display for IntegrityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { file } => write!(
                formatter,
                "the package declares `{file}` but does not contain it"
            ),
            Self::Unreadable { file, source } => {
                write!(formatter, "`{file}` could not be read: {source}")
            }
            Self::OutsidePackage { file } => {
                write!(formatter, "`{file}` resolves outside the package")
            }
            Self::Mismatch {
                file,
                declared,
                actual,
            } => write!(
                formatter,
                "`{file}` hashes {actual} but the manifest records {declared}"
            ),
        }
    }
}

impl std::error::Error for IntegrityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unreadable { source, .. } => Some(source),
            Self::Missing { .. } | Self::OutsidePackage { .. } | Self::Mismatch { .. } => None,
        }
    }
}

impl StructuredFailure for IntegrityError {
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    // Operating-system error details are untrusted text.
    fn quotes_untrusted_text(&self) -> bool {
        matches!(self, Self::Unreadable { .. })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};

    use ravel_core::{LifecycleState, Redaction, StructuredFailure};
    use sha2::{Digest, Sha256};

    use super::{IntegrityError, verify_package, verify_provenance, verify_source_file};
    use crate::manifest::PackageManifest;
    use crate::test_support::manifest_document;
    use crate::values::Sha256Digest;

    const SOURCE_BYTES: &[u8] = b"pub fn load() {}\n";
    const RECORD_BYTES: &[u8] = b"{\"origin\":\"example\"}";
    const ABSENT_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn digest_of(bytes: &[u8]) -> String {
        let mut digest = [0_u8; 32];
        digest.copy_from_slice(&Sha256::digest(bytes));
        Sha256Digest::from_bytes(digest).to_string()
    }

    fn package(label: &str) -> (PathBuf, PackageManifest) {
        let root =
            std::env::temp_dir().join(format!("ravel-package-{label}-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("a stale package should be removable");
        }
        fs::create_dir_all(root.join("src")).expect("the package should be creatable");
        fs::write(root.join("src/lib.rs"), SOURCE_BYTES).expect("source written");
        fs::write(root.join("provenance.json"), RECORD_BYTES).expect("record written");

        let manifest = PackageManifest::parse(&manifest_document(
            "*",
            r#"["26.0.0"]"#,
            &digest_of(SOURCE_BYTES),
            &digest_of(RECORD_BYTES),
        ))
        .expect("a valid manifest");
        (root, manifest)
    }

    fn source_path(root: &Path) -> PathBuf {
        root.join("src/lib.rs")
    }

    #[cfg(unix)]
    fn external_path(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "ravel-package-external-{label}-{}",
            std::process::id()
        ));
        if path.exists() {
            fs::remove_dir_all(&path).expect("stale external fixture removed");
        }
        path
    }

    #[test]
    fn an_untouched_package_verifies() {
        let (root, manifest) = package("intact");
        verify_package(&root, &manifest).expect("the package is intact");
        verify_source_file(&root, &manifest.source.files[0]).expect("the source is intact");
        verify_provenance(&root, &manifest.provenance).expect("the provenance is intact");
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn an_altered_source_file_is_a_mismatch_naming_both_digests() {
        let (root, manifest) = package("altered");
        let mut altered = SOURCE_BYTES.to_vec();
        altered[0] ^= 0x01;
        fs::write(source_path(&root), &altered).expect("source rewritten");

        let error = verify_package(&root, &manifest).expect_err("the source was altered");
        let IntegrityError::Mismatch {
            file,
            declared,
            actual,
        } = &error
        else {
            panic!("expected a digest mismatch: {error}");
        };
        assert_eq!(file, "lib.rs");
        assert_eq!(*declared, manifest.source.files[0].sha256);
        assert_eq!(
            *actual,
            Sha256Digest::try_from(digest_of(&altered)).expect("a computed digest is well formed")
        );
        assert_eq!(error.file(), "lib.rs");
        assert!(error.to_string().starts_with("`lib.rs` hashes "));
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn a_truncated_or_extended_source_file_is_caught_as_a_mismatch() {
        let (root, manifest) = package("resized");
        for altered in [
            &SOURCE_BYTES[..SOURCE_BYTES.len() - 1],
            &[SOURCE_BYTES, b"!"].concat()[..],
        ] {
            fs::write(source_path(&root), altered).expect("source rewritten");
            let error = verify_package(&root, &manifest).expect_err("the source was resized");
            assert!(matches!(error, IntegrityError::Mismatch { .. }), "{error}");
        }
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn a_source_file_larger_than_one_read_is_hashed_whole() {
        let (root, _) = package("streamed");
        let bytes = b"0123456789abcdef".repeat(16 * 1024);
        fs::write(source_path(&root), &bytes).expect("source rewritten");

        let manifest = PackageManifest::parse(&manifest_document(
            "*",
            r#"["26.0.0"]"#,
            &digest_of(&bytes),
            &digest_of(RECORD_BYTES),
        ))
        .expect("a valid manifest");
        assert!(bytes.len() > super::READ_BUFFER_BYTES);
        verify_package(&root, &manifest).expect("the entire large file matches its digest");
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn a_missing_source_file_is_distinct_from_an_altered_one() {
        let (root, manifest) = package("missing");
        fs::remove_file(source_path(&root)).expect("source removed");

        let error = verify_package(&root, &manifest).expect_err("the source is gone");
        assert!(
            matches!(&error, IntegrityError::Missing { file } if file == "lib.rs"),
            "{error}"
        );
        assert_eq!(
            error.to_string(),
            "the package declares `lib.rs` but does not contain it"
        );
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn a_file_that_cannot_be_read_is_distinct_from_a_missing_one() {
        let (root, manifest) = package("unreadable");
        fs::remove_file(source_path(&root)).expect("source removed");
        fs::create_dir(source_path(&root)).expect("a directory in the source file's place");

        let error = verify_package(&root, &manifest).expect_err("a directory is not source");
        assert!(
            matches!(error, IntegrityError::Unreadable { .. }),
            "{error}"
        );
        assert_eq!(error.file(), "lib.rs");
        assert!(error.to_string().starts_with("`lib.rs` could not be read"));
        assert!(error.quotes_untrusted_text());
        let source = std::error::Error::source(&error)
            .expect("the filesystem error is retained")
            .downcast_ref::<std::io::Error>()
            .expect("the source is the original IO error");
        assert_eq!(source.kind(), std::io::ErrorKind::IsADirectory);
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn an_intermediate_directory_symlink_cannot_escape_the_package() {
        const EXTERNAL_BYTES: &[u8] = b"external source must never be hashed";
        let (root, _) = package("intermediate-symlink");
        let external = external_path("intermediate-symlink");
        fs::create_dir(&external).expect("external directory created");
        fs::write(external.join("lib.rs"), EXTERNAL_BYTES).expect("external source written");
        fs::remove_dir_all(root.join("src")).expect("package source directory removed");
        symlink(&external, root.join("src")).expect("directory symlink created");
        let manifest = PackageManifest::parse(&manifest_document(
            "*",
            r#"["26.0.0"]"#,
            ABSENT_DIGEST,
            &digest_of(RECORD_BYTES),
        ))
        .expect("a valid manifest");

        let error = verify_package(&root, &manifest).expect_err("the source escapes the package");
        assert!(
            matches!(&error, IntegrityError::OutsidePackage { file } if file == "lib.rs"),
            "{error}"
        );
        assert!(!error.to_string().contains(&digest_of(EXTERNAL_BYTES)));
        fs::remove_dir_all(&root).expect("package cleanup");
        fs::remove_dir_all(&external).expect("external cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn a_final_file_symlink_cannot_escape_the_package() {
        const EXTERNAL_BYTES: &[u8] = b"external source must never be hashed";
        let (root, _) = package("file-symlink");
        let external = external_path("file-symlink");
        fs::create_dir(&external).expect("external directory created");
        let external_file = external.join("outside.rs");
        fs::write(&external_file, EXTERNAL_BYTES).expect("external source written");
        fs::remove_file(source_path(&root)).expect("package source removed");
        symlink(&external_file, source_path(&root)).expect("file symlink created");
        let manifest = PackageManifest::parse(&manifest_document(
            "*",
            r#"["26.0.0"]"#,
            ABSENT_DIGEST,
            &digest_of(RECORD_BYTES),
        ))
        .expect("a valid manifest");

        let error = verify_package(&root, &manifest).expect_err("the source escapes the package");
        assert!(
            matches!(&error, IntegrityError::OutsidePackage { file } if file == "lib.rs"),
            "{error}"
        );
        assert!(!error.to_string().contains(&digest_of(EXTERNAL_BYTES)));
        fs::remove_dir_all(&root).expect("package cleanup");
        fs::remove_dir_all(&external).expect("external cleanup");
    }

    #[test]
    fn an_altered_provenance_record_is_caught_on_its_own() {
        let (root, manifest) = package("provenance");
        fs::write(root.join("provenance.json"), b"{}").expect("record rewritten");

        verify_source_file(&root, &manifest.source.files[0]).expect("the source is intact");
        let error =
            verify_provenance(&root, &manifest.provenance).expect_err("the record was altered");
        assert_eq!(error.file(), "provenance.json");
        assert!(matches!(error, IntegrityError::Mismatch { .. }), "{error}");
        let combined = verify_package(&root, &manifest).expect_err("not intact");
        let (
            IntegrityError::Mismatch {
                file,
                declared,
                actual,
            },
            IntegrityError::Mismatch {
                file: combined_file,
                declared: combined_declared,
                actual: combined_actual,
            },
        ) = (&error, &combined)
        else {
            panic!("expected matching digest failures: {error:?}, {combined:?}");
        };
        assert_eq!(file, combined_file);
        assert_eq!(declared, combined_declared);
        assert_eq!(actual, combined_actual);
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn an_integrity_failure_never_names_anything_but_the_file() {
        let (root, manifest) = package("redaction");
        fs::remove_file(source_path(&root)).expect("source removed");

        let error = verify_package(&root, &manifest).expect_err("the source is gone");
        let rendered = error.to_string();
        assert!(!rendered.contains('/'), "{rendered}");
        assert!(!rendered.contains("src"), "{rendered}");
        assert!(
            !rendered.contains(&root.display().to_string()),
            "{rendered}"
        );
        assert_eq!(error.diagnostic().redaction(), Redaction::Raw);
        assert_eq!(error.lifecycle_state(), LifecycleState::Discovered);
        assert_eq!(error.failure_kind(), None);
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn a_package_whose_root_does_not_exist_reports_a_missing_file() {
        let (root, manifest) = package("absent-root");
        fs::remove_dir_all(&root).expect("the package is removed entirely");
        let error = verify_package(&root, &manifest).expect_err("the package root is missing");
        assert!(
            matches!(&error, IntegrityError::Missing { file } if file == "lib.rs"),
            "{error}"
        );
    }

    #[test]
    fn a_digest_recorded_for_other_bytes_is_a_mismatch() {
        let (root, _) = package("declared-only");
        let manifest = PackageManifest::parse(&manifest_document(
            "*",
            r#"["26.0.0"]"#,
            ABSENT_DIGEST,
            &digest_of(RECORD_BYTES),
        ))
        .expect("a valid manifest");
        let error = verify_package(&root, &manifest).expect_err("the digest was never correct");
        assert!(matches!(error, IntegrityError::Mismatch { .. }), "{error}");
        fs::remove_dir_all(&root).expect("cleanup");
    }
}
