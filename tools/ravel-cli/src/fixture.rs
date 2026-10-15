//! Fixture descriptor validation, archive verification, and reconstruction.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::FixtureError;

const ALLOWED_DOWNLOAD_PREFIX: &str = "https://mediafilez.forgecdn.net/";

/// Public metadata needed to reconstruct an ignored local fixture.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureDescriptor {
    /// Descriptor format version.
    pub schema_version: u32,
    /// Stable local fixture identifier.
    pub id: String,
    /// Authorized source metadata.
    pub source: FixtureSource,
    /// Expected metadata inside the source archive.
    pub manifest: ExpectedManifest,
}

/// Authorized source and integrity metadata.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureSource {
    /// Source service kind.
    pub kind: String,
    /// `CurseForge` project identifier.
    pub project_id: u64,
    /// `CurseForge` file identifier.
    pub file_id: u64,
    /// Human-readable source page.
    pub page_url: String,
    /// Direct publisher download URL.
    pub download_url: String,
    /// Terms governing the source.
    pub terms_url: String,
    /// Local archive filename.
    pub file_name: String,
    /// Expected archive SHA-256.
    pub sha256: String,
}

/// Expected fields in a `CurseForge` client-pack manifest.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedManifest {
    /// Archive member containing the manifest.
    pub entry: String,
    /// Expected manifest SHA-256.
    pub sha256: String,
    /// `CurseForge` manifest type.
    pub manifest_type: String,
    /// `CurseForge` manifest format version.
    pub manifest_version: u32,
    /// Pack display name.
    pub name: String,
    /// Pack version.
    pub version: String,
    /// Pack author.
    pub author: String,
    /// Minecraft version.
    pub minecraft: String,
    /// Primary mod-loader identifier.
    pub mod_loader: String,
    /// Number of referenced `CurseForge` files.
    pub expected_reference_count: usize,
}

/// Admitted identity fields from a verified fixture archive.
#[derive(Debug, Eq, PartialEq)]
pub struct FixtureVerification {
    /// Minecraft version.
    pub minecraft: String,
    /// Primary mod-loader identifier.
    pub mod_loader: String,
    /// Pack version.
    pub pack_version: String,
    /// Referenced `CurseForge` file count.
    pub reference_count: usize,
}

/// Loads and validates a public fixture descriptor.
///
/// # Errors
///
/// Returns an error when the descriptor cannot be read, parsed, or violates
/// Ravel's fixed source and local-path policy.
pub fn load_fixture_descriptor(path: &Path) -> Result<FixtureDescriptor, FixtureError> {
    let source = fs::read(path)
        .map_err(|error| FixtureError::Io(format!("cannot read {}: {error}", path.display())))?;
    let descriptor: FixtureDescriptor = serde_json::from_slice(&source).map_err(|error| {
        FixtureError::Json(format!(
            "cannot parse fixture descriptor {}: {error}",
            path.display()
        ))
    })?;
    validate_descriptor(&descriptor)?;
    Ok(descriptor)
}

/// Computes the lowercase SHA-256 digest of a file.
///
/// # Errors
///
/// Returns an error when the file cannot be opened or read.
pub fn sha256_file(path: &Path) -> Result<String, FixtureError> {
    let mut file = File::open(path)
        .map_err(|error| FixtureError::Io(format!("cannot open {}: {error}", path.display())))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            FixtureError::Io(format!("cannot hash {}: {error}", path.display()))
        })?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex_digest(&hasher.finalize()))
}

/// Verifies an archive digest and its pinned `CurseForge` manifest.
///
/// # Errors
///
/// Returns an error for descriptor, hash, archive, metadata, count, or
/// duplicate-reference drift.
pub fn verify_fixture_archive(
    descriptor_path: &Path,
    archive_path: &Path,
) -> Result<FixtureVerification, FixtureError> {
    let descriptor = load_fixture_descriptor(descriptor_path)?;
    let archive_hash = sha256_file(archive_path)?;
    require_hash("fixture archive", &descriptor.source.sha256, &archive_hash)?;

    let output = Command::new("unzip")
        .arg("-p")
        .arg(archive_path)
        .arg(&descriptor.manifest.entry)
        .output()
        .map_err(|error| FixtureError::Archive(format!("cannot execute unzip: {error}")))?;
    if !output.status.success() {
        return Err(FixtureError::Archive(format!(
            "cannot read {} from {}",
            descriptor.manifest.entry,
            archive_path.display()
        )));
    }

    let manifest_hash = sha256_bytes(&output.stdout);
    require_hash(
        "fixture manifest",
        &descriptor.manifest.sha256,
        &manifest_hash,
    )?;
    let manifest: CurseForgeManifest = serde_json::from_slice(&output.stdout).map_err(|error| {
        FixtureError::Json(format!(
            "cannot parse {} from {}: {error}",
            descriptor.manifest.entry,
            archive_path.display()
        ))
    })?;

    verify_manifest(&descriptor.manifest, &manifest)
}

/// Downloads and verifies the pinned archive under the ignored fixture root.
/// Existing verified archives are reused; mismatched archives are preserved and rejected.
///
/// # Errors
///
/// Returns an error when paths escape the workspace, the download fails, or
/// verification fails.
pub fn reconstruct_fixture(
    workspace_root: &Path,
    descriptor_path: &Path,
) -> Result<PathBuf, FixtureError> {
    let descriptor = load_fixture_descriptor(descriptor_path)?;
    let source_root = fixture_source_root(workspace_root, &descriptor)?;

    let destination = source_root.join(&descriptor.source.file_name);
    if destination.exists() {
        verify_fixture_archive(descriptor_path, &destination)?;
        return Ok(destination);
    }

    let partial = source_root.join(format!("{}.partial", descriptor.source.file_name));
    if partial.exists() {
        fs::remove_file(&partial).map_err(|error| {
            FixtureError::Io(format!(
                "cannot remove stale partial download {}: {error}",
                partial.display()
            ))
        })?;
    }

    let status = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--proto",
            "=https",
            "--max-redirs",
            "0",
            "--output",
        ])
        .arg(&partial)
        .arg(&descriptor.source.download_url)
        .status()
        .map_err(|error| FixtureError::Download(format!("cannot execute curl: {error}")))?;
    if !status.success() {
        remove_partial(&partial);
        return Err(FixtureError::Download(format!(
            "fixture download failed with {status}"
        )));
    }

    if let Err(error) = verify_fixture_archive(descriptor_path, &partial) {
        remove_partial(&partial);
        return Err(error);
    }
    fs::rename(&partial, &destination).map_err(|error| {
        FixtureError::Io(format!(
            "cannot admit verified fixture archive {}: {error}",
            destination.display()
        ))
    })?;

    Ok(destination)
}

fn fixture_source_root(
    workspace_root: &Path,
    descriptor: &FixtureDescriptor,
) -> Result<PathBuf, FixtureError> {
    let workspace_root = workspace_root.canonicalize().map_err(|error| {
        FixtureError::Io(format!(
            "cannot resolve workspace root {}: {error}",
            workspace_root.display()
        ))
    })?;
    let local_root = workspace_root.join("tests/fixtures/local");
    fs::create_dir_all(&local_root).map_err(|error| {
        FixtureError::Io(format!(
            "cannot create local fixture root {}: {error}",
            local_root.display()
        ))
    })?;
    let local_root = local_root.canonicalize().map_err(|error| {
        FixtureError::Io(format!(
            "cannot resolve local fixture root {}: {error}",
            local_root.display()
        ))
    })?;
    if !local_root.starts_with(&workspace_root) {
        return Err(FixtureError::InvalidDescriptor(
            "local fixture root resolves outside the workspace".to_owned(),
        ));
    }

    let source_root = local_root.join(&descriptor.id).join("source");
    fs::create_dir_all(&source_root).map_err(|error| {
        FixtureError::Io(format!(
            "cannot create fixture source root {}: {error}",
            source_root.display()
        ))
    })?;
    let source_root = source_root.canonicalize().map_err(|error| {
        FixtureError::Io(format!(
            "cannot resolve fixture source root {}: {error}",
            source_root.display()
        ))
    })?;
    if !source_root.starts_with(&local_root) {
        return Err(FixtureError::InvalidDescriptor(
            "fixture source root resolves outside tests/fixtures/local".to_owned(),
        ));
    }
    Ok(source_root)
}

fn validate_descriptor(descriptor: &FixtureDescriptor) -> Result<(), FixtureError> {
    if descriptor.schema_version != 1 {
        return Err(FixtureError::InvalidDescriptor(
            "fixture descriptor schemaVersion must equal 1".to_owned(),
        ));
    }
    if !is_safe_component(&descriptor.id) {
        return Err(FixtureError::InvalidDescriptor(
            "fixture descriptor id is not a safe path component".to_owned(),
        ));
    }
    if descriptor.source.kind != "curseforge-client-pack"
        || descriptor.source.project_id == 0
        || descriptor.source.file_id == 0
    {
        return Err(FixtureError::InvalidDescriptor(
            "fixture source must identify a CurseForge client-pack file".to_owned(),
        ));
    }
    if !descriptor
        .source
        .download_url
        .starts_with(ALLOWED_DOWNLOAD_PREFIX)
    {
        return Err(FixtureError::InvalidDescriptor(
            "fixture download URL is outside the allowed CurseForge CDN".to_owned(),
        ));
    }
    if !descriptor
        .source
        .page_url
        .starts_with("https://www.curseforge.com/")
        || !descriptor.source.terms_url.starts_with("https://")
    {
        return Err(FixtureError::InvalidDescriptor(
            "fixture source pages must use the recorded HTTPS origins".to_owned(),
        ));
    }
    if !is_safe_component(&descriptor.source.file_name)
        || !Path::new(&descriptor.source.file_name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
    {
        return Err(FixtureError::InvalidDescriptor(
            "fixture filename must be a local .zip path component".to_owned(),
        ));
    }
    if descriptor.manifest.entry != "manifest.json"
        || descriptor.manifest.expected_reference_count == 0
    {
        return Err(FixtureError::InvalidDescriptor(
            "fixture must pin a non-empty CurseForge manifest.json".to_owned(),
        ));
    }
    for (field, hash) in [
        ("source.sha256", descriptor.source.sha256.as_str()),
        ("manifest.sha256", descriptor.manifest.sha256.as_str()),
    ] {
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(FixtureError::InvalidDescriptor(format!(
                "fixture descriptor {field} must be a 64-character hexadecimal SHA-256"
            )));
        }
    }
    Ok(())
}

fn verify_manifest(
    expected: &ExpectedManifest,
    actual: &CurseForgeManifest,
) -> Result<FixtureVerification, FixtureError> {
    require_metadata(
        "manifestType",
        &expected.manifest_type,
        &actual.manifest_type,
    )?;
    require_metadata(
        "manifestVersion",
        &expected.manifest_version.to_string(),
        &actual.manifest_version.to_string(),
    )?;
    require_metadata("name", &expected.name, &actual.name)?;
    require_metadata("version", &expected.version, &actual.version)?;
    require_metadata("author", &expected.author, &actual.author)?;
    require_metadata(
        "minecraft.version",
        &expected.minecraft,
        &actual.minecraft.version,
    )?;

    let primary_loaders = actual
        .minecraft
        .mod_loaders
        .iter()
        .filter(|loader| loader.primary)
        .collect::<Vec<_>>();
    let actual_loader = match primary_loaders.as_slice() {
        [loader] => &loader.id,
        _ => {
            return Err(FixtureError::MetadataMismatch {
                field: "minecraft.modLoaders.primary",
                expected: expected.mod_loader.clone(),
                actual: format!("{} primary entries", primary_loaders.len()),
            });
        }
    };
    require_metadata("minecraft.modLoaders", &expected.mod_loader, actual_loader)?;

    if actual.files.len() != expected.expected_reference_count {
        return Err(FixtureError::ReferenceCountMismatch {
            expected: expected.expected_reference_count,
            actual: actual.files.len(),
        });
    }
    let mut references = HashSet::with_capacity(actual.files.len());
    for reference in &actual.files {
        if !references.insert((reference.project_id, reference.file_id)) {
            return Err(FixtureError::DuplicateReference {
                project_id: reference.project_id,
                file_id: reference.file_id,
            });
        }
    }

    Ok(FixtureVerification {
        minecraft: actual.minecraft.version.clone(),
        mod_loader: actual_loader.clone(),
        pack_version: actual.version.clone(),
        reference_count: actual.files.len(),
    })
}

fn require_hash(artifact: &'static str, expected: &str, actual: &str) -> Result<(), FixtureError> {
    if expected.eq_ignore_ascii_case(actual) {
        Ok(())
    } else {
        Err(FixtureError::HashMismatch {
            artifact,
            expected: expected.to_owned(),
            actual: actual.to_owned(),
        })
    }
}

fn require_metadata(field: &'static str, expected: &str, actual: &str) -> Result<(), FixtureError> {
    if expected == actual {
        Ok(())
    } else {
        Err(FixtureError::MetadataMismatch {
            field,
            expected: expected.to_owned(),
            actual: actual.to_owned(),
        })
    }
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex_digest(&Sha256::digest(bytes))
}

fn hex_digest(digest: &[u8]) -> String {
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn is_safe_component(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('.')
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && !value.contains(['/', '\\'])
}

fn remove_partial(path: &Path) {
    if let Err(error) = fs::remove_file(path)
        && error.kind() != io::ErrorKind::NotFound
    {
        eprintln!(
            "warning: cannot remove partial fixture {}: {error}",
            path.display()
        );
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeManifest {
    minecraft: MinecraftManifest,
    manifest_type: String,
    manifest_version: u32,
    name: String,
    version: String,
    author: String,
    files: Vec<CurseForgeReference>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MinecraftManifest {
    version: String,
    mod_loaders: Vec<ModLoader>,
}

#[derive(Debug, Deserialize)]
struct ModLoader {
    id: String,
    primary: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeReference {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::{
        CurseForgeManifest, CurseForgeReference, FixtureDescriptor, FixtureError,
        MinecraftManifest, ModLoader, is_safe_component, reconstruct_fixture, remove_partial,
        sha256_file, validate_descriptor, verify_manifest,
    };

    #[test]
    fn descriptor_policy_rejects_each_unsafe_boundary() {
        let mut descriptor = valid_descriptor();
        descriptor.schema_version = 2;
        assert_invalid(&descriptor, "schemaVersion");

        let mut descriptor = valid_descriptor();
        descriptor.id = "../escape".to_owned();
        assert_invalid(&descriptor, "safe path component");

        let mut descriptor = valid_descriptor();
        descriptor.source.project_id = 0;
        assert_invalid(&descriptor, "CurseForge client-pack");

        let mut descriptor = valid_descriptor();
        descriptor.source.download_url = "https://example.invalid/fixture.zip".to_owned();
        assert_invalid(&descriptor, "allowed CurseForge CDN");

        let mut descriptor = valid_descriptor();
        descriptor.source.page_url = "http://www.curseforge.com/project".to_owned();
        assert_invalid(&descriptor, "HTTPS origins");

        let mut descriptor = valid_descriptor();
        descriptor.source.file_name = "fixture.jar".to_owned();
        assert_invalid(&descriptor, "local .zip");

        let mut descriptor = valid_descriptor();
        descriptor.manifest.expected_reference_count = 0;
        assert_invalid(&descriptor, "non-empty");

        let mut descriptor = valid_descriptor();
        descriptor.source.sha256 = "not-a-digest".to_owned();
        assert_invalid(&descriptor, "source.sha256");
    }

    #[test]
    fn manifest_policy_reports_identity_loader_and_duplicate_drift() {
        let expected = valid_descriptor().manifest;
        let mut manifest = valid_manifest();
        manifest.name = "Changed".to_owned();
        assert!(matches!(
            verify_manifest(&expected, &manifest),
            Err(FixtureError::MetadataMismatch { field: "name", .. })
        ));

        let mut manifest = valid_manifest();
        manifest.minecraft.mod_loaders.push(ModLoader {
            id: "another-loader".to_owned(),
            primary: true,
        });
        assert!(matches!(
            verify_manifest(&expected, &manifest),
            Err(FixtureError::MetadataMismatch {
                field: "minecraft.modLoaders.primary",
                ..
            })
        ));

        let mut manifest = valid_manifest();
        manifest.files.push(CurseForgeReference {
            project_id: 1,
            file_id: 2,
        });
        let mut expected = expected;
        expected.expected_reference_count = 2;
        assert_eq!(
            verify_manifest(&expected, &manifest),
            Err(FixtureError::DuplicateReference {
                project_id: 1,
                file_id: 2
            })
        );
    }

    #[test]
    fn safe_components_exclude_hidden_and_nested_paths() {
        assert!(is_safe_component("fixture.zip"));
        assert!(!is_safe_component(""));
        assert!(!is_safe_component(".hidden"));
        assert!(!is_safe_component("../escape"));
        assert!(!is_safe_component("nested/file.zip"));
        assert!(!is_safe_component(r"nested\file.zip"));
    }

    #[test]
    fn removing_an_absent_partial_is_idempotent() {
        remove_partial(Path::new("/definitely/not/a/ravel/partial"));
    }

    #[test]
    fn reconstruction_admits_an_existing_verified_fixture() {
        let root = temporary_root("unit-reconstruct");
        let source_root = root.join("tests/fixtures/local/fixture/source");
        fs::create_dir_all(&source_root).expect("fixture root should be creatable");
        let manifest = source_root.join("manifest.json");
        fs::write(
            &manifest,
            serde_json::to_vec(&serde_json::json!({
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
            }))
            .expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
        let status = Command::new("zip")
            .args(["-q", "fixture.zip", "manifest.json"])
            .current_dir(&source_root)
            .status()
            .expect("zip should execute");
        assert!(status.success());
        let archive = source_root.join("fixture.zip");
        let descriptor = root.join("descriptor.json");
        let mut value =
            serde_json::to_value(valid_descriptor()).expect("descriptor should serialize");
        value["source"]["sha256"] = sha256_file(&archive)
            .expect("archive should be hashable")
            .into();
        value["manifest"]["sha256"] = sha256_file(&manifest)
            .expect("manifest should be hashable")
            .into();
        fs::write(
            &descriptor,
            serde_json::to_vec(&value).expect("descriptor should serialize"),
        )
        .expect("descriptor should be writable");

        let admitted =
            reconstruct_fixture(&root, &descriptor).expect("verified fixture should be admitted");

        assert_eq!(admitted, archive);
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    fn assert_invalid(descriptor: &FixtureDescriptor, expected_message: &str) {
        let error = validate_descriptor(descriptor)
            .expect_err("the altered descriptor should violate fixture policy");
        assert!(error.to_string().contains(expected_message));
    }

    fn valid_descriptor() -> FixtureDescriptor {
        serde_json::from_value(serde_json::json!({
            "schemaVersion": 1,
            "id": "fixture",
            "source": {
                "kind": "curseforge-client-pack",
                "projectId": 1,
                "fileId": 2,
                "pageUrl": "https://www.curseforge.com/project",
                "downloadUrl": "https://mediafilez.forgecdn.net/files/1/2/fixture.zip",
                "termsUrl": "https://support.curseforge.com/terms",
                "fileName": "fixture.zip",
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            },
            "manifest": {
                "entry": "manifest.json",
                "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "manifestType": "minecraftModpack",
                "manifestVersion": 1,
                "name": "All the Mods 11",
                "version": "0.2.1",
                "author": "ATMTeam",
                "minecraft": "26.1.2",
                "modLoader": "neoforge-26.1.2.78",
                "expectedReferenceCount": 1
            }
        }))
        .expect("test descriptor should deserialize")
    }

    fn valid_manifest() -> CurseForgeManifest {
        CurseForgeManifest {
            minecraft: MinecraftManifest {
                version: "26.1.2".to_owned(),
                mod_loaders: vec![ModLoader {
                    id: "neoforge-26.1.2.78".to_owned(),
                    primary: true,
                }],
            },
            manifest_type: "minecraftModpack".to_owned(),
            manifest_version: 1,
            name: "All the Mods 11".to_owned(),
            version: "0.2.1".to_owned(),
            author: "ATMTeam".to_owned(),
            files: vec![CurseForgeReference {
                project_id: 1,
                file_id: 2,
            }],
        }
    }

    fn temporary_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("ravel-cli-{label}-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale temporary root should be removable");
        }
        fs::create_dir_all(&root).expect("temporary root should be creatable");
        root
    }
}
