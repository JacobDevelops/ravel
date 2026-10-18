//! Compares declared Java, pack, and shared-hook contracts with runtime artifacts.
//! Admission and reviewed declaration checks happen outside this artifact-facing module.

use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::{jar, mapping, resolve};

use super::{Adapter, GenerateError, HookSide, Inventory, Spelling, file_name};

/// Checks the declared Java release against an available version manifest.
///
/// # Errors
///
/// Returns an error if the manifest is unreadable or its Java release is missing or inconsistent.
pub(super) fn verify_java_release(root: &Path, adapter: &Adapter) -> Result<(), GenerateError> {
    let path = root.join(&adapter.inputs.version_manifest.path);
    if !path.is_file() {
        return Ok(());
    }
    let text = fs::read_to_string(&path).map_err(|error| {
        GenerateError::Input(format!("{} is unreadable: {error}", path.display()))
    })?;
    if major_version(&text).is_none() && adapter.is_legacy() {
        let manifest: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
            GenerateError::Input(format!("invalid legacy version manifest: {error}"))
        })?;
        if manifest.get("javaVersion").is_none() && manifest.is_object() {
            return Ok(());
        }
    }
    let declared = major_version(&text).ok_or_else(|| {
        GenerateError::Input(format!(
            "{} states no javaVersion.majorVersion, so nothing says what release this version runs",
            file_name(&adapter.inputs.version_manifest.path)
        ))
    })?;
    let source_release = adapter.effective_source_java_release();
    if declared != source_release {
        return Err(GenerateError::Drift(format!(
            "the adapter inputs declare effective source Java release {source_release} but {} states {declared}",
            file_name(&adapter.inputs.version_manifest.path)
        )));
    }
    Ok(())
}

/// Legacy manifests omit Java metadata; the pinned bytecode must prove Java 8.
pub(crate) fn verify_legacy_class_release(archive: &jar::Archive) -> Result<(), GenerateError> {
    let mut maximum = None;
    for name in archive.names().filter(|name| {
        Path::new(name)
            .extension()
            .is_some_and(|ext| ext == "class")
    }) {
        let bytes = archive.entry_prefix::<8>(name)?;
        if bytes[..4] != [0xca, 0xfe, 0xba, 0xbe] {
            return Err(GenerateError::Extraction(format!(
                "{name} has no valid class-file version header"
            )));
        }
        let major = u16::from_be_bytes([bytes[6], bytes[7]]);
        maximum = Some(maximum.map_or(major, |previous: u16| previous.max(major)));
    }
    if maximum != Some(52) {
        return Err(GenerateError::Drift(format!(
            "legacy-1-12 sourceJavaRelease 8 requires maximum class-file major 52; \
             pinned client jar states {maximum:?}"
        )));
    }
    Ok(())
}

/// Checks shared targets against the official server using the shared runtime namespace.
pub(crate) fn verify_server_hooks(
    archive: &jar::Archive,
    inventory: &Inventory,
    mapping: &mapping::Mapping,
    client: &[resolve::Resolved],
) -> Result<(), GenerateError> {
    let shared = Inventory {
        hooks: inventory
            .hooks
            .iter()
            .filter(|hook| hook.side == HookSide::Shared)
            .cloned()
            .collect(),
        schema_version: inventory.schema_version,
        minecraft: inventory.minecraft.clone(),
        non_goals: Vec::new(),
    };
    for target in resolve::from_jar(archive, &shared, mapping)? {
        let matched = client.iter().any(|candidate| {
            candidate.id == target.id
                && candidate.runtime_class == target.runtime_class
                && candidate.runtime_member == target.runtime_member
                && candidate.runtime_descriptor == target.runtime_descriptor
                && candidate.is_static == target.is_static
        });
        if !matched {
            return Err(GenerateError::Drift(format!(
                "server hook {} resolves to a different runtime target than the client",
                target.id
            )));
        }
    }
    Ok(())
}

/// Checks the declared pack formats against the pinned jar.
///
/// # Errors
///
/// Returns an error if the jar's pack formats are missing or inconsistent with the declaration.
pub(crate) fn verify_pack_format(
    adapter: &Adapter,
    archive: &jar::Archive,
) -> Result<(), GenerateError> {
    if adapter.is_legacy() {
        return Ok(());
    }
    let text = jar_text(archive, JAR_VERSION)?;
    let declared = adapter.pack_format;
    let stated = pack_versions(&text).ok_or_else(|| {
        GenerateError::Input(format!(
            "{JAR_VERSION} in the pinned jar states no pack_version, so nothing says which pack \
             formats this version reads"
        ))
    })?;
    let expected = (
        declared.resource,
        declared.resource_minor,
        declared.data,
        declared.data_minor,
    );
    if stated != expected {
        return Err(GenerateError::Drift(format!(
            "the adapter inputs declare packFormat {expected:?} but {JAR_VERSION} in the pinned \
             jar states {stated:?}, as resource major, resource minor, data major, data minor"
        )));
    }
    verify_spelling(declared.spelling, archive)
}

/// Checks the declared metadata spelling against a pack in the pinned jar.
///
/// # Errors
///
/// Returns an error if the jar contains no pack or its metadata uses another spelling.
fn verify_spelling(spelling: Spelling, archive: &jar::Archive) -> Result<(), GenerateError> {
    let Some(name) = archive.names().find(|name| name.ends_with(PACK_METADATA)) else {
        return Err(GenerateError::Input(format!(
            "the pinned jar ships no {PACK_METADATA} of its own, so nothing says which shape a \
             pack this version reads is written in"
        )));
    };
    let name = name.to_owned();
    let text = jar_text(archive, &name)?;
    if !text.contains(spelling.key()) {
        return Err(GenerateError::Drift(format!(
            "the adapter inputs declare packFormat spelling {} but {name} in the pinned jar \
             carries no {}",
            spelling.as_str(),
            spelling.key()
        )));
    }
    Ok(())
}

/// One named entry of the pinned jar, as text.
fn jar_text(archive: &jar::Archive, name: &str) -> Result<String, GenerateError> {
    let bytes = archive.entry(name)?;
    String::from_utf8(bytes).map_err(|error| {
        GenerateError::Input(format!("{name} in the pinned jar is not UTF-8: {error}"))
    })
}

/// The document every pack, Ravel's included, declares its format in.
const PACK_METADATA: &str = "pack.mcmeta";

/// The document inside a client jar that states what the build is.
const JAR_VERSION: &str = "version.json";

/// The resource and data pack formats one jar's `version.json` states.
fn pack_versions(text: &str) -> Option<(u16, u16, u16, u16)> {
    #[derive(Deserialize)]
    struct Version {
        pack_version: PackVersion,
    }
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum PackVersion {
        Joined {
            resource: u16,
            data: u16,
        },
        Split {
            resource_major: u16,
            resource_minor: u16,
            data_major: u16,
            data_minor: u16,
        },
    }
    let version: Version = serde_json::from_str(text).ok()?;
    Some(match version.pack_version {
        PackVersion::Joined { resource, data } => (resource, 0, data, 0),
        PackVersion::Split {
            resource_major,
            resource_minor,
            data_major,
            data_minor,
        } => (resource_major, resource_minor, data_major, data_minor),
    })
}

/// The `javaVersion.majorVersion` one version manifest states.
fn major_version(text: &str) -> Option<u16> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Manifest {
        java_version: JavaVersion,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct JavaVersion {
        major_version: u16,
    }
    let manifest: Manifest = serde_json::from_str(text).ok()?;
    Some(manifest.java_version.major_version)
}

#[cfg(test)]
mod tests {
    use super::super::Capability;
    use super::super::fixtures::{document, legacy_adapter};
    use super::super::sha256;
    use super::Adapter;
    #[test]
    fn manifest_is_compared_with_source_release_not_adapter_target() {
        let root =
            std::env::temp_dir().join(format!("ravel-source-java-release-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("temporary manifest directory");
        let mut adapter = legacy_adapter();
        adapter.inputs.version_manifest.path = "version.json".to_owned();
        let path = root.join("version.json");
        std::fs::write(&path, r#"{"javaVersion":{"majorVersion":8}}"#)
            .expect("legacy source manifest");
        super::verify_java_release(&root, &adapter)
            .expect("Java 8 source does not require a Java 8 adapter");
        std::fs::write(&path, r#"{"javaVersion":{"majorVersion":27}}"#)
            .expect("mismatched manifest");
        let error = super::verify_java_release(&root, &adapter)
            .expect_err("target release does not excuse source drift");
        assert!(
            error.to_string().contains("source Java release 8"),
            "{error}"
        );
        adapter.source_java_release = None;
        super::verify_java_release(&root, &adapter)
            .expect("without override the manifest uses javaRelease");
        adapter = legacy_adapter();
        adapter.inputs.version_manifest.path = "version.json".to_owned();
        adapter.inputs.client_jar.path = "client.jar".to_owned();
        let jar_path = root.join("client.jar");
        std::fs::write(&path, "{}").expect("legacy manifest omits javaVersion");
        super::verify_java_release(&root, &adapter)
            .expect("reviewed legacy source release permits offline generation");
        for (major, accepted) in [(51_u16, false), (52, true), (53, false)] {
            let mut header = vec![0xca, 0xfe, 0xba, 0xbe, 0, 0];
            header.extend(major.to_be_bytes());
            let bytes = super::jar::fixture::stored("Game.class", &header);
            adapter.inputs.client_jar.sha256 = Some(sha256(&bytes));
            std::fs::write(&jar_path, bytes).expect("pinned class version fixture");
            let archive = super::jar::Archive::open(&jar_path).expect("class-version fixture");
            assert_eq!(
                super::verify_legacy_class_release(&archive).is_ok(),
                accepted
            );
        }
        adapter.capabilities.clear();
        super::verify_java_release(&root, &adapter)
            .expect_err("modern manifests cannot omit javaVersion");
        adapter.capabilities.push(Capability::Legacy112);
        std::fs::write(&path, r#"{"javaVersion":{}}"#).expect("malformed Java declaration");
        super::verify_java_release(&root, &adapter)
            .expect_err("malformed declarations are not historical omissions");
        std::fs::remove_file(jar_path).expect("remove temporary client jar");
        std::fs::remove_file(path).expect("remove temporary manifest");
        std::fs::remove_dir(root).expect("remove temporary directory");
    }

    #[test]
    fn only_explicit_client_only_hooks_can_be_absent_from_the_server() {
        let text = "schemaVersion = 1\nminecraft = \"test\"\n\
                    [[hook]]\nid = \"test\"\nclass = \"example.Client\"\n\
                    member = \"tick\"\ndescriptor = \"()V\"\n\
                    kind = \"instance-method\"\ncapability = \"test\"\n";
        let mut inventory: super::Inventory = toml::from_str(text).expect("shared default");
        let archive = super::jar::fixture::parse(super::jar::fixture::stored("unused", b""))
            .expect("server with no client class");
        let mapping = crate::mapping::Mapping::Identity;
        super::verify_server_hooks(&archive, &inventory, &mapping, &[])
            .expect_err("shared missing class is an error");
        inventory.hooks[0].side = super::HookSide::ClientOnly;
        super::verify_server_hooks(&archive, &inventory, &mapping, &[])
            .expect("explicit client-only classification permits absence");
        assert!(toml::from_str::<super::Inventory>(&format!("{text}side = \"client\"\n")).is_err());
    }

    #[test]
    fn the_java_release_is_read_from_the_java_version_object() {
        let manifest = r#"{
  "downloads": {"client": {"majorVersion": 3, "size": 1}},
  "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21}
}"#;

        assert_eq!(super::major_version(manifest), Some(21));
    }

    #[test]
    fn both_spellings_of_a_jars_pack_version_are_read_and_neither_reads_the_other() {
        let joined = r#"{"data": 9, "pack_version": {"resource": 34, "data": 48}}"#;
        assert_eq!(super::pack_versions(joined), Some((34, 0, 48, 0)));

        let split = r#"{"pack_version": {"resource_major": 84, "resource_minor": 0,
          "data_major": 101, "data_minor": 1}, "world_version": 4790}"#;
        assert_eq!(super::pack_versions(split), Some((84, 0, 101, 1)));

        assert_eq!(super::pack_versions(r#"{"world_version": 4790}"#), None);
        assert_eq!(
            super::pack_versions(r#"{"pack_version": {"data": 48}}"#),
            None
        );
    }

    #[test]
    fn metadata_numbers_are_read_only_from_valid_json_at_the_declared_path() {
        let nested = r#"{"pack_version":{"note":{"resource":1},"data":48,"resource":34}}"#;
        assert_eq!(super::pack_versions(nested), Some((34, 0, 48, 0)));
        for value in ["21.5", "-21", "65536", "\"21\"", "21garbage"] {
            let java = format!(r#"{{"javaVersion":{{"majorVersion":{value}}}}}"#);
            assert_eq!(super::major_version(&java), None, "{java}");
            let packs = format!(r#"{{"pack_version":{{"resource":{value},"data":48}}}}"#);
            assert_eq!(super::pack_versions(&packs), None, "{packs}");
        }
        let outside = r#"{"javaVersion":{},"elsewhere":{"majorVersion":21}}"#;
        assert_eq!(super::major_version(outside), None);
    }

    #[test]
    fn only_explicit_legacy_capability_accepts_a_jar_without_pack_version_metadata() {
        let archive = super::jar::fixture::parse(super::jar::fixture::stored("unused", b""))
            .expect("an archive without version metadata");
        super::verify_pack_format(&legacy_adapter(), &archive)
            .expect("legacy pack constants do not pretend to come from jar metadata");
        let modern: Adapter =
            toml::from_str(&document("unobfuscated-class-files", "")).expect("a modern adapter");
        super::verify_pack_format(&modern, &archive)
            .expect_err("modern jars must declare their pack formats");
        assert_eq!(
            super::major_version(r#"{"javaVersion":{"component":"jre-legacy","majorVersion":8}}"#),
            Some(8)
        );
    }
}
