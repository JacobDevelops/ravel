use std::fmt;
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::Path;

use ravel_core::{FailureKind, StructuredFailure};
use sha2::{Digest, Sha256};

use crate::values::{RelativePath, Sha256Digest};
use crate::{Artifact, PackageManifest, Provenance};

/// How much of a file is hashed at a time.
///
/// An artifact is a binary and a package may ship several, so the file is
/// streamed rather than read into memory whole.
const READ_BUFFER_BYTES: usize = 64 * 1024;

/// Verifies one declared artifact against the digest its manifest records.
///
/// `SPEC.md` §10 requires package integrity verification and
/// `docs/architecture/safety-boundary.md` keeps that requirement in place for
/// fully trusted mods: trust decides what a mod may do once it runs, not
/// whether the bytes on disk are the bytes the author published. The check runs
/// before anything is loaded, so a package that fails it never reaches
/// [`ravel_core::LifecycleState::Validated`].
///
/// `root` is the package directory. The artifact path is package-relative,
/// absolute paths and `..` components were refused at parse time, so the file
/// this opens is always inside `root`.
///
/// # Errors
///
/// Returns [`IntegrityError::Missing`] when the file is absent,
/// [`IntegrityError::Unreadable`] when it cannot be read, and
/// [`IntegrityError::Mismatch`] when its contents hash to anything other than
/// the recorded digest, which covers alteration, truncation, and extension
/// alike.
pub fn verify_artifact(root: &Path, artifact: &Artifact) -> Result<(), IntegrityError> {
    verify_file(root, &artifact.path, artifact.sha256)
}

/// Verifies the provenance record against the digest its manifest records.
///
/// `SPEC.md` §3.4 requires provenance metadata, which is only worth recording
/// if it is checked the same way an artifact is.
///
/// # Errors
///
/// Returns the same failures as [`verify_artifact`].
pub fn verify_provenance(root: &Path, provenance: &Provenance) -> Result<(), IntegrityError> {
    verify_file(root, &provenance.record, provenance.sha256)
}

/// Verifies every artifact of every declared target, then the provenance
/// record.
///
/// Every target is checked rather than one selected target. Choosing which
/// target a host would load is an ADG-2 question that M3 answers, and a package
/// claims all of its declared digests are correct regardless.
///
/// # Errors
///
/// Returns the first failure encountered, in declaration order.
pub fn verify_package(root: &Path, manifest: &PackageManifest) -> Result<(), IntegrityError> {
    for target in &manifest.targets {
        for artifact in &target.artifacts {
            verify_artifact(root, artifact)?;
        }
    }
    verify_provenance(root, &manifest.provenance)
}

fn verify_file(
    root: &Path,
    path: &RelativePath,
    declared: Sha256Digest,
) -> Result<(), IntegrityError> {
    let file = path.file_name();
    let actual = digest_file(&root.join(path.as_str()), file)?;
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
    let mut reader = File::open(path).map_err(|error| IntegrityError::from_io(file, &error))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; READ_BUFFER_BYTES];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| IntegrityError::from_io(file, &error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let mut digest = [0_u8; 32];
    digest.copy_from_slice(&hasher.finalize());
    Ok(Sha256Digest::from_bytes(digest))
}

/// A declared file is not the file the manifest describes.
///
/// Every variant names a file rather than a path. The failure is about a file
/// inside a package, and the directory that package was installed into says
/// where a player keeps their game, which `SPEC.md` §9.1 requires reports to
/// redact. The `detail` an unreadable file carries comes from the operating
/// system through `std::fs`, which reports the failing operation and never the
/// path it was given.
#[derive(Clone, Debug, Eq, PartialEq)]
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
        detail: String,
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
    fn from_io(file: &str, error: &std::io::Error) -> Self {
        if error.kind() == ErrorKind::NotFound {
            return Self::Missing {
                file: file.to_owned(),
            };
        }
        Self::Unreadable {
            file: file.to_owned(),
            detail: error.to_string(),
        }
    }

    /// Returns the declared file name the failure is about.
    #[must_use]
    pub fn file(&self) -> &str {
        match self {
            Self::Missing { file }
            | Self::Unreadable { file, .. }
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
            Self::Unreadable { file, detail } => {
                write!(formatter, "`{file}` could not be read: {detail}")
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

impl std::error::Error for IntegrityError {}

impl StructuredFailure for IntegrityError {
    /// An integrity failure has no `SPEC.md` §9.2 kind.
    ///
    /// That list names the dependency-graph outcomes reported before world
    /// access. A package whose bytes do not match its manifest fails earlier
    /// than any of them and is reported as itself.
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    /// [`Self::Unreadable`] quotes the operating system, which is text this
    /// project did not write. The other two are written here and name a file.
    fn quotes_untrusted_text(&self) -> bool {
        matches!(self, Self::Unreadable { .. })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use ravel_core::{LifecycleState, Redaction, StructuredFailure};
    use sha2::{Digest, Sha256};

    use super::{IntegrityError, verify_artifact, verify_package, verify_provenance};
    use crate::PackageManifest;
    use crate::values::Sha256Digest;

    const ARTIFACT_BYTES: &[u8] = b"the artifact this package ships";
    const RECORD_BYTES: &[u8] = b"{\"origin\":\"example\"}";
    const ABSENT_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn digest_of(bytes: &[u8]) -> String {
        let mut digest = [0_u8; 32];
        digest.copy_from_slice(&Sha256::digest(bytes));
        Sha256Digest::from_bytes(digest).to_string()
    }

    fn manifest_document(artifact_digest: &str, record_digest: &str) -> String {
        format!(
            r#"{{
              "schemaVersion": 1,
              "id": "example_mod",
              "version": "1.4.0",
              "compatibility": {{ "ravel": "*", "minecraft": ["26.0.0"] }},
              "sides": ["client"],
              "targets": [{{
                "platform": "linux",
                "architecture": "x86_64",
                "artifacts": [
                  {{
                    "path": "artifacts/mod.bin",
                    "role": "module",
                    "sha256": "{artifact_digest}"
                  }}
                ]
              }}],
              "authors": [{{ "name": "Example" }}],
              "license": "MIT",
              "source": "https://example.invalid/example_mod",
              "provenance": {{ "record": "provenance.json", "sha256": "{record_digest}" }}
            }}"#
        )
    }

    /// Builds a package directory whose files match the digests it declares.
    fn package(label: &str) -> (PathBuf, PackageManifest) {
        let root =
            std::env::temp_dir().join(format!("ravel-package-{label}-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("a stale package should be removable");
        }
        fs::create_dir_all(root.join("artifacts")).expect("the package should be creatable");
        fs::write(root.join("artifacts/mod.bin"), ARTIFACT_BYTES).expect("artifact written");
        fs::write(root.join("provenance.json"), RECORD_BYTES).expect("record written");

        let manifest = PackageManifest::parse(&manifest_document(
            &digest_of(ARTIFACT_BYTES),
            &digest_of(RECORD_BYTES),
        ))
        .expect("a valid manifest");
        (root, manifest)
    }

    fn artifact_path(root: &Path) -> PathBuf {
        root.join("artifacts/mod.bin")
    }

    #[test]
    fn an_untouched_package_verifies() {
        let (root, manifest) = package("intact");
        assert_eq!(verify_package(&root, &manifest), Ok(()));
        assert_eq!(
            verify_artifact(&root, &manifest.targets[0].artifacts[0]),
            Ok(())
        );
        assert_eq!(verify_provenance(&root, &manifest.provenance), Ok(()));
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn an_altered_artifact_is_a_mismatch_naming_both_digests() {
        let (root, manifest) = package("altered");
        let mut altered = ARTIFACT_BYTES.to_vec();
        altered[0] ^= 0x01;
        fs::write(artifact_path(&root), &altered).expect("artifact rewritten");

        let error = verify_package(&root, &manifest).expect_err("the artifact was altered");
        assert_eq!(
            error,
            IntegrityError::Mismatch {
                file: "mod.bin".to_owned(),
                declared: manifest.targets[0].artifacts[0].sha256,
                actual: digest_of(&altered)
                    .try_into()
                    .expect("a computed digest is well formed"),
            }
        );
        assert_eq!(error.file(), "mod.bin");
        assert!(error.to_string().starts_with("`mod.bin` hashes "));
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn a_truncated_or_extended_artifact_is_caught_as_a_mismatch() {
        let (root, manifest) = package("resized");
        for altered in [
            &ARTIFACT_BYTES[..ARTIFACT_BYTES.len() - 1],
            &[ARTIFACT_BYTES, b"!"].concat()[..],
        ] {
            fs::write(artifact_path(&root), altered).expect("artifact rewritten");
            let error = verify_package(&root, &manifest).expect_err("the artifact was resized");
            assert!(matches!(error, IntegrityError::Mismatch { .. }), "{error}");
        }
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn an_artifact_larger_than_one_read_is_hashed_whole() {
        let (root, _) = package("streamed");
        let bytes = b"0123456789abcdef".repeat(16 * 1024);
        fs::write(artifact_path(&root), &bytes).expect("artifact rewritten");

        let manifest = PackageManifest::parse(&manifest_document(
            &digest_of(&bytes),
            &digest_of(RECORD_BYTES),
        ))
        .expect("a valid manifest");
        assert!(bytes.len() > super::READ_BUFFER_BYTES);
        assert_eq!(verify_package(&root, &manifest), Ok(()));
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn a_missing_artifact_is_distinct_from_an_altered_one() {
        let (root, manifest) = package("missing");
        fs::remove_file(artifact_path(&root)).expect("artifact removed");

        let error = verify_package(&root, &manifest).expect_err("the artifact is gone");
        assert_eq!(
            error,
            IntegrityError::Missing {
                file: "mod.bin".to_owned()
            }
        );
        assert_eq!(
            error.to_string(),
            "the package declares `mod.bin` but does not contain it"
        );
        assert_ne!(
            error,
            IntegrityError::Mismatch {
                file: "mod.bin".to_owned(),
                declared: manifest.targets[0].artifacts[0].sha256,
                actual: manifest.provenance.sha256,
            }
        );
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn a_file_that_cannot_be_read_is_distinct_from_a_missing_one() {
        let (root, manifest) = package("unreadable");
        fs::remove_file(artifact_path(&root)).expect("artifact removed");
        fs::create_dir(artifact_path(&root)).expect("a directory in the artifact's place");

        let error = verify_package(&root, &manifest).expect_err("a directory is not an artifact");
        assert!(
            matches!(error, IntegrityError::Unreadable { .. }),
            "{error}"
        );
        assert_eq!(error.file(), "mod.bin");
        assert!(error.to_string().starts_with("`mod.bin` could not be read"));
        assert!(
            error.quotes_untrusted_text(),
            "the detail is the operating system's sentence"
        );
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn an_altered_provenance_record_is_caught_on_its_own() {
        let (root, manifest) = package("provenance");
        fs::write(root.join("provenance.json"), b"{}").expect("record rewritten");

        assert_eq!(
            verify_artifact(&root, &manifest.targets[0].artifacts[0]),
            Ok(())
        );
        let error =
            verify_provenance(&root, &manifest.provenance).expect_err("the record was altered");
        assert_eq!(error.file(), "provenance.json");
        assert!(matches!(error, IntegrityError::Mismatch { .. }), "{error}");
        assert_eq!(
            verify_package(&root, &manifest).expect_err("the package is not intact"),
            error
        );
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn an_integrity_failure_never_names_anything_but_the_file() {
        let (root, manifest) = package("redaction");
        fs::remove_file(artifact_path(&root)).expect("artifact removed");

        let error = verify_package(&root, &manifest).expect_err("the artifact is gone");
        let rendered = error.to_string();
        assert!(!rendered.contains('/'), "{rendered}");
        assert!(!rendered.contains("artifacts"), "{rendered}");
        assert!(
            !rendered.contains(&root.display().to_string()),
            "{rendered}"
        );

        assert_eq!(error.diagnostic().redaction(), Redaction::Raw);
        assert_eq!(error.diagnostic().text(), rendered);
        assert_eq!(error.lifecycle_state(), LifecycleState::Discovered);
        assert_eq!(error.failure_kind(), None);
        assert!(
            !error.quotes_untrusted_text(),
            "a missing file is named by this crate"
        );
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn a_package_whose_root_does_not_exist_reports_a_missing_file() {
        let (root, manifest) = package("absent-root");
        fs::remove_dir_all(&root).expect("the package is removed entirely");

        assert_eq!(
            verify_package(&root, &manifest),
            Err(IntegrityError::Missing {
                file: "mod.bin".to_owned()
            })
        );
    }

    #[test]
    fn a_digest_recorded_for_a_file_that_was_never_written_is_a_mismatch() {
        let (root, _) = package("declared-only");
        let manifest =
            PackageManifest::parse(&manifest_document(ABSENT_DIGEST, &digest_of(RECORD_BYTES)))
                .expect("a valid manifest");

        let error = verify_package(&root, &manifest).expect_err("the digest was never correct");
        assert!(matches!(error, IntegrityError::Mismatch { .. }), "{error}");
        fs::remove_dir_all(&root).expect("cleanup");
    }
}
