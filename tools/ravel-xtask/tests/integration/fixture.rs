use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use ravel_xtask::fixture::{
    FixtureError, load_fixture_descriptor, reconstruct_fixture, sha256_file, verify_fixture_archive,
};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

#[test]
fn file_hashing_returns_the_canonical_digest() {
    let root = temporary_root("typed-file-digest");
    let path = root.join("bytes");
    fs::write(&path, b"abc").expect("fixture bytes are writable");
    let digest = sha256_file(&path).expect("fixture bytes hash");
    assert_eq!(
        digest,
        ravel_core::Sha256Digest::try_from(
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_owned()
        )
        .expect("canonical digest")
    );
    fs::remove_dir_all(root).expect("temporary root is removable");
}

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
        descriptor.source.sha256.to_string(),
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
    let archive = root.join("fixture.zip");
    write_archive(
        &archive,
        &[
            (
                "manifest.json",
                &fs::read(&manifest_path).expect("manifest is readable"),
            ),
            ("overrides/mods/bundled-0.jar", b"jar"),
            ("overrides/mods/bundled-1.jar", b"jar"),
            ("overrides/mods/bundled-2.jar", b"jar"),
            ("overrides/mods/bundled-3.jar", b"jar"),
            ("overrides/mods/bundled-4.JAR", b"jar"),
            ("overrides/mods/nested/not-top-level.jar", b"nested"),
            ("overrides/mods/notes.txt", b"not a jar"),
        ],
    );
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(pinned).expect("descriptor should be readable"))
            .expect("descriptor should parse");
    value["source"]["sha256"] =
        serde_json::json!(sha256_file(&archive).expect("archive should hash"));
    value["manifest"]["sha256"] =
        serde_json::json!(sha256_file(&manifest_path).expect("manifest should hash"));
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
    write_archive(
        &archive,
        &[(
            "manifest.json",
            &fs::read(&manifest).expect("manifest is readable"),
        )],
    );
    let actual_hash = sha256_file(&archive).expect("test archive should be hashable");
    let altered_descriptor = root.join("descriptor.json");
    let source = fs::read_to_string(descriptor()).expect("descriptor should be readable");
    let mut value: serde_json::Value =
        serde_json::from_str(&source).expect("descriptor should be JSON");
    value["source"]["sha256"] = actual_hash.to_string().into();
    value["manifest"]["sha256"] = sha256_file(&manifest)
        .expect("test manifest should be hashable")
        .to_string()
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
    let archive = source_root.join("fixture.zip");
    write_archive(
        &archive,
        &[(
            "manifest.json",
            &fs::read(&manifest).expect("manifest is readable"),
        )],
    );
    let synthetic_descriptor = root.join("descriptor.json");
    let source = fs::read_to_string(descriptor()).expect("descriptor should be readable");
    let mut value: serde_json::Value =
        serde_json::from_str(&source).expect("descriptor should be JSON");
    value["id"] = "synthetic".into();
    value["source"]["fileName"] = "fixture.zip".into();
    value["source"]["sha256"] = sha256_file(&archive)
        .expect("test archive should be hashable")
        .to_string()
        .into();
    value["manifest"]["sha256"] = sha256_file(&manifest)
        .expect("test manifest should be hashable")
        .to_string()
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
        .to_string()
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
    let archive = root.join("parse-error.zip");
    write_archive(&archive, &[("manifest.json", b"not JSON")]);
    value["source"]["sha256"] = sha256_file(&archive)
        .expect("archive should be hashable")
        .to_string()
        .into();
    value["manifest"]["sha256"] = sha256_file(&manifest)
        .expect("manifest should be hashable")
        .to_string()
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

fn write_archive(path: &Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).expect("archive is writable");
    let mut writer = ZipWriter::new(file);
    for &(name, content) in entries {
        writer
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
            )
            .expect("archive entry is writable");
        writer
            .write_all(content)
            .expect("archive content is writable");
    }
    writer.finish().expect("archive is complete");
}

fn synthetic_archive(root: &Path, jars: &[(&str, &[u8])]) -> (PathBuf, PathBuf) {
    let manifest = serde_json::to_vec(&serde_json::json!({
        "minecraft": {"version": "1.21.1",
            "modLoaders": [{"id": "neoforge-21.1.221", "primary": true}]},
        "manifestType": "minecraftModpack", "manifestVersion": 1,
        "name": "ATM10 To the Sky", "version": "2.0.2", "author": "ATMTeam",
        "files": [{"projectID": 1, "fileID": 2, "required": true}], "overrides": "overrides"
    }))
    .expect("manifest serializes");
    let manifest_path = root.join("manifest.json");
    fs::write(&manifest_path, &manifest).expect("manifest is writable");
    let archive = root.join("fixture.zip");
    let mut entries = vec![("manifest.json", manifest.as_slice())];
    entries.extend_from_slice(jars);
    write_archive(&archive, &entries);
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(descriptor()).expect("descriptor is readable"))
            .expect("descriptor parses");
    value["source"]["sha256"] = sha256_file(&archive)
        .expect("archive hashes")
        .to_string()
        .into();
    value["manifest"]["sha256"] = sha256_file(&manifest_path)
        .expect("manifest hashes")
        .to_string()
        .into();
    value["manifest"]["expectedReferenceCount"] = 1.into();
    let descriptor = root.join("descriptor.json");
    fs::write(
        &descriptor,
        serde_json::to_vec(&value).expect("descriptor serializes"),
    )
    .expect("descriptor is writable");
    (descriptor, archive)
}

#[test]
fn fixture_verification_needs_no_system_unzip() {
    let root = temporary_root("no-unzip");
    let (descriptor, archive) = synthetic_archive(&root, &[("overrides/mods/bundled.jar", b"jar")]);
    let output = xtask_command()
        .env("PATH", &root)
        .args(["fixture", "verify"])
        .arg(&descriptor)
        .arg(&archive)
        .output()
        .expect("xtask executes");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).expect("temporary root is removable");
}

#[test]
fn duplicate_override_central_entries_are_not_collapsed() {
    let root = temporary_root("duplicate-override");
    let (descriptor, archive) = synthetic_archive(
        &root,
        &[
            ("overrides/mods/first.jar", b"first"),
            ("overrides/mods/other.jar", b"other"),
        ],
    );
    let mut bytes = fs::read(&archive).expect("archive is readable");
    let old = b"overrides/mods/other.jar";
    let positions: Vec<_> = bytes
        .windows(old.len())
        .enumerate()
        .filter_map(|(index, name)| (name == old).then_some(index))
        .collect();
    assert_eq!(
        positions.len(),
        2,
        "local and central names must both change"
    );
    for index in positions {
        bytes[index..index + old.len()].copy_from_slice(b"overrides/mods/first.jar");
    }
    fs::write(&archive, bytes).expect("duplicate archive is writable");
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&descriptor).expect("descriptor is readable"))
            .expect("descriptor parses");
    value["source"]["sha256"] = sha256_file(&archive)
        .expect("archive hashes")
        .to_string()
        .into();
    fs::write(
        &descriptor,
        serde_json::to_vec(&value).expect("descriptor serializes"),
    )
    .expect("descriptor is writable");
    let error = verify_fixture_archive(&descriptor, &archive).expect_err("duplicates must fail");
    assert!(matches!(&error, FixtureError::Archive(detail)
        if detail.contains("duplicate bundled override jar overrides/mods/first.jar")));
    fs::remove_dir_all(root).expect("temporary root is removable");
}

#[test]
fn duplicate_manifests_are_rejected_even_when_one_matches_the_pinned_digest() {
    for conflicting in [false, true] {
        let root = temporary_root("duplicate-manifest");
        let (descriptor, archive) = synthetic_archive(&root, &[]);
        let manifest = fs::read(root.join("manifest.json")).expect("manifest is readable");
        let mut other = manifest.clone();
        if conflicting {
            other.push(b'\n');
        }
        write_archive(
            &archive,
            &[("manifest.json", &manifest), ("otherone.json", &other)],
        );
        rename_archive_entry(&archive, b"otherone.json", b"manifest.json");
        // Bind the digest to whichever duplicate ZipArchive retains. Verification must
        // still reject the raw duplicate central records, identical or conflicting.
        let mut selected = Vec::new();
        zip::ZipArchive::new(fs::File::open(&archive).expect("archive opens"))
            .expect("archive parses")
            .by_name("manifest.json")
            .expect("manifest is present")
            .read_to_end(&mut selected)
            .expect("selected manifest is readable");
        let selected_path = root.join("selected.json");
        fs::write(&selected_path, selected).expect("selected manifest is writable");
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&descriptor).expect("descriptor is readable"))
                .expect("descriptor parses");
        value["source"]["sha256"] = sha256_file(&archive)
            .expect("archive hashes")
            .to_string()
            .into();
        value["manifest"]["sha256"] = sha256_file(&selected_path)
            .expect("manifest hashes")
            .to_string()
            .into();
        fs::write(
            &descriptor,
            serde_json::to_vec(&value).expect("descriptor serializes"),
        )
        .expect("descriptor is writable");
        let error = verify_fixture_archive(&descriptor, &archive)
            .expect_err("duplicate manifests must fail");
        assert!(matches!(&error, FixtureError::Archive(detail)
            if detail.contains("duplicate fixture manifest manifest.json")));
        fs::remove_dir_all(root).expect("temporary root is removable");
    }
}

fn rename_archive_entry(archive: &Path, old: &[u8], new: &[u8]) {
    assert_eq!(
        old.len(),
        new.len(),
        "ZIP header lengths must remain unchanged"
    );
    let mut bytes = fs::read(archive).expect("archive is readable");
    let positions: Vec<_> = bytes
        .windows(old.len())
        .enumerate()
        .filter_map(|(index, name)| (name == old).then_some(index))
        .collect();
    assert_eq!(
        positions.len(),
        2,
        "local and central names must both change"
    );
    for index in positions {
        bytes[index..index + old.len()].copy_from_slice(new);
    }
    fs::write(archive, bytes).expect("archive is writable");
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
