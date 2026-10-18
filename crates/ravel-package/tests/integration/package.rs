//! Package admission across parsing, on-disk integrity, and host compatibility.

use std::fs;
use std::path::PathBuf;

use ravel_core::StructuredFailure;
use ravel_package::{IntegrityError, PackageManifest, Sha256Digest, verify_package};
use sha2::{Digest, Sha256};

const SOURCE_BYTES: &[u8] = b"pub fn load() {}\n";
const RECORD_BYTES: &[u8] = b"{\"origin\":\"example\"}";

fn digest_of(bytes: &[u8]) -> String {
    let mut digest = [0_u8; 32];
    digest.copy_from_slice(&Sha256::digest(bytes));
    Sha256Digest::from_bytes(digest).to_string()
}

fn manifest_document(source_digest: &str, record_digest: &str) -> String {
    format!(
        r#"{{
  "schemaVersion": 1,
  "id": "example_mod",
  "version": "1.4.0",
  "displayName": "Example Mod",
  "description": "A package that walks the whole admission path.",
  "compatibility": {{ "ravel": ">=0.1, <0.2", "minecraft": ["26.0.0", "26.0.1"] }},
  "sides": ["client", "integrated-server"],
  "dependencies": [
    {{
      "id": "example_library",
      "relationship": "required",
      "version": "^1.0.0",
      "sides": ["client"],
      "order": "after"
    }}
  ],
  "source": {{
    "repository": "https://example.invalid/example_mod",
    "files": [{{ "path": "src/lib.rs", "sha256": "{source_digest}" }}]
  }},
  "resources": ["resources/lang/en.json"],
  "configurationSchemas": ["schemas/config.json"],
  "savedDataVersions": [1],
  "authors": [{{ "name": "Example", "url": "https://example.invalid/example" }}],
  "license": "MIT OR Apache-2.0",
  "provenance": {{ "record": "provenance.json", "sha256": "{record_digest}" }}
}}"#
    )
}

fn package(label: &str) -> (PathBuf, PackageManifest) {
    let root =
        std::env::temp_dir().join(format!("ravel-package-flow-{label}-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("a stale package should be removable");
    }
    fs::create_dir_all(root.join("src")).expect("the source directory");
    fs::write(root.join("src/lib.rs"), SOURCE_BYTES).expect("source written");
    fs::write(root.join("provenance.json"), RECORD_BYTES).expect("record written");

    let manifest = PackageManifest::parse(&manifest_document(
        &digest_of(SOURCE_BYTES),
        &digest_of(RECORD_BYTES),
    ))
    .expect("the fixture manifest should parse");
    (root, manifest)
}

#[test]
fn an_intact_package_passes_identity_integrity_and_compatibility_together() {
    let (root, manifest) = package("intact");

    assert_eq!(manifest.id.as_str(), "example_mod");
    assert_eq!(manifest.source.files[0].path.as_str(), "src/lib.rs");
    verify_package(&root, &manifest).expect("the installed package is intact");
    assert_eq!(
        manifest
            .compatibility
            .check_ravel(&"0.1.4".parse().expect("a valid version")),
        Ok(())
    );
    assert_eq!(manifest.compatibility.check_minecraft("26.0.1"), Ok(()));

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn a_tampered_package_fails_integrity_while_still_parsing() {
    let (root, manifest) = package("tampered");
    fs::write(root.join("src/lib.rs"), b"someone else's bytes").expect("source rewritten");

    let error = verify_package(&root, &manifest).expect_err("the source file was replaced");
    assert!(matches!(error, IntegrityError::Mismatch { .. }), "{error}");
    assert_eq!(error.file(), "lib.rs");
    assert_eq!(error.failure_kind(), None);
    assert!(!error.to_string().contains('/'), "{error}");

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn a_host_outside_the_declared_compatibility_is_refused_before_any_file_is_read() {
    let (root, manifest) = package("incompatible");

    let ravel = manifest
        .compatibility
        .check_ravel(&"0.3.0".parse().expect("a valid version"))
        .expect_err("outside the declared Ravel range");
    let minecraft = manifest
        .compatibility
        .check_minecraft("25.0.0")
        .expect_err("not a declared Minecraft version");

    assert_eq!(
        ravel.failure_kind(),
        Some(ravel_core::FailureKind::IncompatibleVersion)
    );
    assert_eq!(
        minecraft.failure_kind(),
        Some(ravel_core::FailureKind::IncompatibleVersion)
    );
    assert_ne!(ravel.to_string(), minecraft.to_string());
    assert_eq!(
        ravel.lifecycle_state(),
        ravel_core::LifecycleState::Discovered
    );

    fs::remove_dir_all(&root).expect("cleanup");
}
