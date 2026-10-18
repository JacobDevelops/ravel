use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use ravel_xtask::FixtureError;
use ravel_xtask::fixture::{
    load_fixture_descriptor, reconstruct_fixture, sha256_file, verify_fixture_archive,
};

#[test]
fn pinned_atm10_tts_descriptor_records_the_exact_reference_contract() {
    let descriptor =
        load_fixture_descriptor(&descriptor()).expect("the public descriptor should be valid");

    assert_eq!(descriptor.source.project_id, 1_298_402);
    assert_eq!(descriptor.source.file_id, 7_854_204);
    assert_eq!(descriptor.manifest.minecraft, "1.21.1");
    assert_eq!(descriptor.manifest.mod_loader, "neoforge-21.1.221");
    assert_eq!(descriptor.manifest.version, "2.0.2");
    assert_eq!(descriptor.manifest.expected_reference_count, 324);
}

#[test]
fn sevtech_archive_accounts_for_references_and_bundled_jars_separately() {
    let root = temporary_root("sevtech-accounting");
    let pinned = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/descriptors/sevtech-ages-3.2.3.json");
    let descriptor = load_fixture_descriptor(&pinned).expect("SevTech descriptor should load");
    assert_eq!(
        descriptor.source.sha256,
        "f872775f7ac0996fdd1bde177a1517aecc43e9209da97406dcbbe3ac842483b8"
    );
    assert_eq!(descriptor.manifest.expected_reference_count, 254);
    assert_eq!(descriptor.manifest.minecraft, "1.12.2");
    assert_eq!(descriptor.manifest.mod_loader, "forge-14.23.5.2860");
    let files: Vec<_> = (1..=254)
        .map(|id| serde_json::json!({"projectID": id, "fileID": id, "required": true}))
        .collect();
    let manifest = serde_json::json!({
        "minecraft": {"version": "1.12.2",
            "modLoaders": [{"id": "forge-14.23.5.2860", "primary": true}]},
        "manifestType": "minecraftModpack", "manifestVersion": 1,
        "name": descriptor.manifest.name, "version": descriptor.manifest.version,
        "author": descriptor.manifest.author, "files": files, "overrides": "overrides"
    });
    let manifest_path = root.join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest).expect("manifest should serialize"),
    )
    .expect("manifest should be writable");
    let mods = root.join("overrides/mods");
    fs::create_dir_all(mods.join("nested")).expect("override directories should be creatable");
    for id in 0..5 {
        fs::write(
            mods.join(format!("bundled-{id}.jar")),
            b"synthetic jar input",
        )
        .expect("jar input should be writable");
    }
    fs::write(mods.join("nested/not-top-level.jar"), b"nested")
        .expect("nested file should be writable");
    fs::write(mods.join("notes.txt"), b"not a jar").expect("notes should be writable");
    let status = Command::new("zip")
        .args(["-qr", "fixture.zip", "manifest.json", "overrides"])
        .current_dir(&root)
        .status()
        .expect("zip should execute");
    assert!(status.success());
    let archive = root.join("fixture.zip");
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(pinned).expect("descriptor should be readable"))
            .expect("descriptor should parse");
    value["source"]["sha256"] = sha256_file(&archive).expect("archive should hash").into();
    value["manifest"]["sha256"] = sha256_file(&manifest_path)
        .expect("manifest should hash")
        .into();
    let synthetic = root.join("descriptor.json");
    fs::write(
        &synthetic,
        serde_json::to_vec(&value).expect("descriptor should serialize"),
    )
    .expect("descriptor should be writable");
    let verified = verify_fixture_archive(&synthetic, &archive).expect("archive should verify");
    assert_eq!(verified.reference_count, 254);
    assert_eq!(verified.override_jar_count, 5);
    fs::write(&archive, b"tampered override content").expect("archive should be writable");
    assert!(matches!(
        verify_fixture_archive(&synthetic, &archive),
        Err(FixtureError::HashMismatch { .. })
    ));
    fs::remove_dir_all(root).expect("temporary root should be removable");
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
            "version": "1.21.1",
            "modLoaders": [{"id": "neoforge-21.1.221", "primary": true}]
          },
          "manifestType": "minecraftModpack",
          "manifestVersion": 1,
          "name": "ATM10 To the Sky",
          "version": "2.0.2",
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
            expected: 324,
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
    fs::write(
        &manifest,
        r#"{
          "minecraft": {
            "version": "1.21.1",
            "modLoaders": [{"id": "neoforge-21.1.221", "primary": true}]
          },
          "manifestType": "minecraftModpack",
          "manifestVersion": 1,
          "name": "ATM10 To the Sky",
          "version": "2.0.2",
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
    let output = xtask_command()
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
        .expect("ravel-xtask should execute");
    assert!(output.status.success());

    exercise_fixture_cli_paths(&root, &synthetic_descriptor, &archive);
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn exercise_fixture_cli_paths(root: &Path, synthetic_descriptor: &Path, archive: &Path) {
    let default_descriptor = root.join("tests/fixtures/descriptors/sevtech-ages-3.2.3.json");
    fs::create_dir_all(
        default_descriptor
            .parent()
            .expect("default descriptor should have a parent"),
    )
    .expect("default descriptor root should be creatable");
    fs::copy(synthetic_descriptor, &default_descriptor)
        .expect("default descriptor should be copyable");
    let default_archive =
        root.join("tests/fixtures/local/sevtech-ages-3.2.3/source/SevTech_Ages_3.2.3.zip");
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
        let output = xtask_command()
            .args(arguments)
            .current_dir(root)
            .output()
            .expect("ravel-xtask should execute");
        assert!(output.status.success());
    }

    let output = xtask_command()
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
        .expect("ravel-xtask should execute");
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn command_line_rejects_unknown_operations_with_usage() {
    let output = xtask_command()
        .arg("unknown")
        .output()
        .expect("ravel-xtask should execute");

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

fn descriptor() -> PathBuf {
    workspace_root().join("tests/fixtures/descriptors/atm10-tts-2.0.2.json")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("ravel-xtask must remain under tools/ravel-xtask")
        .to_path_buf()
}

fn xtask_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ravel-xtask"));
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

fn temporary_root(label: &str) -> PathBuf {
    static NEXT_ROOT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    loop {
        let sequence = NEXT_ROOT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "ravel-xtask-{}-{sequence}-{label}",
            std::process::id()
        ));
        match fs::DirBuilder::new().mode(0o700).create(&root) {
            Ok(()) => return root,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("temporary root should be creatable: {error}"),
        }
    }
}
