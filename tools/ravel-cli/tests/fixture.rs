use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ravel_cli::{
    FixtureError, load_fixture_descriptor, reconstruct_fixture, sha256_file, verify_fixture_archive,
};

#[test]
fn pinned_atm11_descriptor_records_the_exact_reference_contract() {
    let descriptor =
        load_fixture_descriptor(&descriptor()).expect("the public descriptor should be valid");

    assert_eq!(descriptor.source.project_id, 1_148_445);
    assert_eq!(descriptor.source.file_id, 8_431_787);
    assert_eq!(descriptor.manifest.minecraft, "26.1.2");
    assert_eq!(descriptor.manifest.mod_loader, "neoforge-26.1.2.78");
    assert_eq!(descriptor.manifest.version, "0.2.1");
    assert_eq!(descriptor.manifest.expected_reference_count, 235);
}

#[test]
fn altered_archive_is_rejected_before_manifest_admission() {
    let root = temporary_root("altered");
    let archive = root.join("fixture.zip");
    fs::write(&archive, b"not the pinned archive").expect("test archive should be writable");

    let error = verify_fixture_archive(&descriptor(), &archive)
        .expect_err("an archive with the wrong hash must be rejected");

    assert!(matches!(error, FixtureError::HashMismatch { .. }));
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn manifest_reference_drift_is_rejected() {
    let root = temporary_root("reference-drift");
    let manifest = root.join("manifest.json");
    fs::write(
        &manifest,
        r#"{
          "minecraft": {
            "version": "26.1.2",
            "modLoaders": [{"id": "neoforge-26.1.2.78", "primary": true}]
          },
          "manifestType": "minecraftModpack",
          "manifestVersion": 1,
          "name": "All the Mods 11",
          "version": "0.2.1",
          "author": "ATMTeam",
          "files": [{"projectID": 1, "fileID": 2, "required": true}],
          "overrides": "overrides"
        }"#,
    )
    .expect("test manifest should be writable");
    let archive = root.join("fixture.zip");
    let status = Command::new("zip")
        .args(["-q", "fixture.zip", "manifest.json"])
        .current_dir(&root)
        .status()
        .expect("zip should be available in the development shell");
    assert!(status.success());
    let actual_hash = sha256_file(&archive).expect("test archive should be hashable");
    let altered_descriptor = root.join("descriptor.json");
    let source = fs::read_to_string(descriptor()).expect("descriptor should be readable");
    let mut value: serde_json::Value =
        serde_json::from_str(&source).expect("descriptor should be JSON");
    value["source"]["sha256"] = actual_hash.into();
    value["manifest"]["sha256"] = sha256_file(&manifest)
        .expect("test manifest should be hashable")
        .into();
    fs::write(
        &altered_descriptor,
        serde_json::to_vec_pretty(&value).expect("descriptor should serialize"),
    )
    .expect("test descriptor should be writable");

    let error = verify_fixture_archive(&altered_descriptor, &archive)
        .expect_err("a manifest with reference drift must be rejected");

    assert!(matches!(
        error,
        FixtureError::ReferenceCountMismatch {
            expected: 235,
            actual: 1
        }
    ));
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn reconstruction_reuses_a_verified_archive_inside_the_ignored_boundary() {
    let root = temporary_root("reconstruct");
    let source_root = root.join("tests/fixtures/local/synthetic/source");
    fs::create_dir_all(&source_root).expect("fixture source root should be creatable");
    let manifest = source_root.join("manifest.json");
    // The file row's ids are placeholders. They were a real member mod's
    // catalogue ids, copied verbatim out of the pack, and nothing here reads
    // them — which made them a real upstream identifier sitting in a literal a
    // reviewer would read as synthetic, with no check anywhere that would say
    // so. Public catalogue ids are not a leak; a fixture that claims to be
    // synthetic and is not is a worse thing to keep than an accurate one.
    fs::write(
        &manifest,
        r#"{
          "minecraft": {
            "version": "26.1.2",
            "modLoaders": [{"id": "neoforge-26.1.2.78", "primary": true}]
          },
          "manifestType": "minecraftModpack",
          "manifestVersion": 1,
          "name": "All the Mods 11",
          "version": "0.2.1",
          "author": "ATMTeam",
          "files": [{"projectID": 1, "fileID": 2, "required": true}],
          "overrides": "overrides"
        }"#,
    )
    .expect("test manifest should be writable");
    let status = Command::new("zip")
        .args(["-q", "fixture.zip", "manifest.json"])
        .current_dir(&source_root)
        .status()
        .expect("zip should be available in the development shell");
    assert!(status.success());
    let archive = source_root.join("fixture.zip");
    let synthetic_descriptor = root.join("descriptor.json");
    let source = fs::read_to_string(descriptor()).expect("descriptor should be readable");
    let mut value: serde_json::Value =
        serde_json::from_str(&source).expect("descriptor should be JSON");
    value["id"] = "synthetic".into();
    value["source"]["fileName"] = "fixture.zip".into();
    value["source"]["sha256"] = sha256_file(&archive)
        .expect("test archive should be hashable")
        .into();
    value["manifest"]["sha256"] = sha256_file(&manifest)
        .expect("test manifest should be hashable")
        .into();
    value["manifest"]["expectedReferenceCount"] = 1.into();
    fs::write(
        &synthetic_descriptor,
        serde_json::to_vec_pretty(&value).expect("descriptor should serialize"),
    )
    .expect("test descriptor should be writable");

    let reconstructed = reconstruct_fixture(&root, &synthetic_descriptor)
        .expect("an existing verified local archive should be reused");

    assert_eq!(reconstructed, archive);
    let output = ravel_cli_command()
        .args([
            "fixture",
            "verify",
            synthetic_descriptor
                .to_str()
                .expect("test descriptor path should be UTF-8"),
            archive.to_str().expect("test archive path should be UTF-8"),
        ])
        .current_dir(&root)
        .output()
        .expect("ravel-cli should execute");
    assert!(output.status.success());

    exercise_fixture_cli_paths(&root, &synthetic_descriptor, &archive);
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn exercise_fixture_cli_paths(root: &Path, synthetic_descriptor: &Path, archive: &Path) {
    let default_descriptor = root.join("tests/fixtures/descriptors/atm11-0.2.1.json");
    fs::create_dir_all(
        default_descriptor
            .parent()
            .expect("default descriptor should have a parent"),
    )
    .expect("default descriptor root should be creatable");
    fs::copy(synthetic_descriptor, &default_descriptor)
        .expect("default descriptor should be copyable");
    let default_archive = root.join("tests/fixtures/local/atm11-0.2.1/source/atm11-0.2.1.zip");
    fs::create_dir_all(
        default_archive
            .parent()
            .expect("default archive should have a parent"),
    )
    .expect("default archive root should be creatable");
    fs::copy(archive, &default_archive).expect("default archive should be copyable");

    for arguments in [
        vec!["fixture", "verify"],
        vec!["fixture", "reconstruct"],
        vec![
            "fixture",
            "reconstruct",
            synthetic_descriptor
                .to_str()
                .expect("test descriptor path should be UTF-8"),
        ],
    ] {
        let output = ravel_cli_command()
            .args(arguments)
            .current_dir(root)
            .output()
            .expect("ravel-cli should execute");
        assert!(output.status.success());
    }

    let output = ravel_cli_command()
        .args([
            "fixture",
            "verify",
            synthetic_descriptor
                .to_str()
                .expect("test descriptor path should be UTF-8"),
            "missing.zip",
        ])
        .current_dir(root)
        .output()
        .expect("ravel-cli should execute");
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn command_line_rejects_unknown_operations_with_usage() {
    let output = ravel_cli_command()
        .arg("unknown")
        .output()
        .expect("ravel-cli should execute");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage:"));
}

#[test]
fn malformed_descriptor_and_archive_inputs_fail_closed() {
    let root = temporary_root("malformed");
    let missing = load_fixture_descriptor(&root.join("missing.json"))
        .expect_err("a missing descriptor must fail");
    assert!(matches!(missing, FixtureError::Io(_)));

    let invalid_descriptor = root.join("invalid.json");
    fs::write(&invalid_descriptor, b"not JSON").expect("invalid descriptor should be writable");
    let invalid = load_fixture_descriptor(&invalid_descriptor)
        .expect_err("invalid descriptor JSON must fail");
    assert!(matches!(invalid, FixtureError::Json(_)));

    let descriptor_source =
        fs::read_to_string(descriptor()).expect("public descriptor should be readable");
    let mut value: serde_json::Value =
        serde_json::from_str(&descriptor_source).expect("descriptor should be JSON");
    let invalid_archive = root.join("invalid.zip");
    fs::write(&invalid_archive, b"not a zip").expect("invalid archive should be writable");
    value["source"]["sha256"] = sha256_file(&invalid_archive)
        .expect("invalid archive should still be hashable")
        .into();
    let matching_descriptor = root.join("matching.json");
    fs::write(
        &matching_descriptor,
        serde_json::to_vec(&value).expect("descriptor should serialize"),
    )
    .expect("matching descriptor should be writable");
    let archive_error = verify_fixture_archive(&matching_descriptor, &invalid_archive)
        .expect_err("an invalid zip must fail");
    assert!(matches!(archive_error, FixtureError::Archive(_)));

    let manifest = root.join("manifest.json");
    fs::write(&manifest, b"not JSON").expect("invalid manifest should be writable");
    let status = Command::new("zip")
        .args(["-q", "parse-error.zip", "manifest.json"])
        .current_dir(&root)
        .status()
        .expect("zip should execute");
    assert!(status.success());
    let archive = root.join("parse-error.zip");
    value["source"]["sha256"] = sha256_file(&archive)
        .expect("archive should be hashable")
        .into();
    value["manifest"]["sha256"] = sha256_file(&manifest)
        .expect("manifest should be hashable")
        .into();
    fs::write(
        &matching_descriptor,
        serde_json::to_vec(&value).expect("descriptor should serialize"),
    )
    .expect("matching descriptor should be writable");
    let parse_error = verify_fixture_archive(&matching_descriptor, &archive)
        .expect_err("invalid manifest JSON must fail");
    assert!(matches!(parse_error, FixtureError::Json(_)));

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn ravel_cli_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ravel-cli"));
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        let profile = PathBuf::from(profile);
        let file_name = profile
            .file_name()
            .expect("coverage profile should have a filename")
            .to_string_lossy();
        command.env(
            "LLVM_PROFILE_FILE",
            profile.with_file_name(format!("child-%p-{file_name}")),
        );
    }
    command
}

fn descriptor() -> PathBuf {
    workspace_root().join("tests/fixtures/descriptors/atm11-0.2.1.json")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("ravel-cli must remain under tools/ravel-cli")
        .to_path_buf()
}

fn temporary_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("ravel-cli-{label}-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("stale temporary root should be removable");
    }
    fs::create_dir_all(&root).expect("temporary root should be creatable");
    root
}
