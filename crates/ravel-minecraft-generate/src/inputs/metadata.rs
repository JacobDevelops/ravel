//! Checks declared source Java releases against available version manifests.

use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::{Adapter, GenerateError, file_name};

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
        GenerateError::io(file_name(&adapter.inputs.version_manifest.path), error)
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
    use crate::test_fixtures::legacy_adapter;

    #[test]
    fn unreadable_manifest_retains_io_source_and_path() {
        use std::error::Error as _;
        let root = std::env::temp_dir().join(format!(
            "ravel-manifest-invalid-utf8-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create temporary manifest directory");
        let path = root.join("version.json");
        std::fs::write(&path, [0xff]).expect("write invalid UTF-8 manifest");
        let mut adapter = legacy_adapter();
        adapter.inputs.version_manifest.path = "version.json".to_owned();
        let error = super::verify_java_release(&root, &adapter)
            .expect_err("an invalid UTF-8 manifest must be refused");
        let source = error
            .source()
            .expect("the manifest error must retain its I/O source")
            .downcast_ref::<std::io::Error>()
            .expect("the manifest error source must be an std::io::Error");
        assert_eq!(source.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("version.json"));
        assert!(!error.to_string().contains("ravel-manifest-invalid-utf8"));
        std::fs::remove_dir_all(root).expect("remove temporary manifest directory");
    }

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
        std::fs::write(&path, "{}").expect("legacy manifest omits javaVersion");
        super::verify_java_release(&root, &adapter)
            .expect("reviewed legacy source release permits offline generation");
        adapter.capabilities.clear();
        super::verify_java_release(&root, &adapter)
            .expect_err("modern manifests cannot omit javaVersion");
        adapter.capabilities.push(Capability::Legacy112);
        std::fs::write(&path, r#"{"javaVersion":{}}"#).expect("malformed Java declaration");
        super::verify_java_release(&root, &adapter)
            .expect_err("malformed declarations are not historical omissions");
        std::fs::remove_file(path).expect("remove temporary manifest");
        std::fs::remove_dir(root).expect("remove temporary directory");
    }

    #[test]
    fn the_java_release_is_read_from_the_java_version_object() {
        let manifest = r#"{
  "downloads": {"client": {"majorVersion": 3, "size": 1}},
  "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21}
}"#;

        assert_eq!(super::major_version(manifest), Some(21));
        assert_eq!(
            super::major_version(r#"{"javaVersion":{"component":"jre-legacy","majorVersion":8}}"#),
            Some(8)
        );
    }

    #[test]
    fn metadata_numbers_are_read_only_from_valid_json_at_the_declared_path() {
        for value in ["21.5", "-21", "65536", "\"21\"", "21garbage"] {
            let java = format!(r#"{{"javaVersion":{{"majorVersion":{value}}}}}"#);
            assert_eq!(super::major_version(&java), None, "{java}");
        }
        let outside = r#"{"javaVersion":{},"elsewhere":{"majorVersion":21}}"#;
        assert_eq!(super::major_version(outside), None);
    }
}
