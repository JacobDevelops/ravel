use std::{
    path::{Path, PathBuf},
    process::Command,
};

use super::{GenerateError, inputs, mapping::Mapping};

pub(super) const NORMALIZER_SOURCE: &str =
    "crates/ravel-minecraft-generate/java/NormalizeMappings.java";

pub(super) fn read(root: &Path, adapter: &inputs::Adapter) -> Result<Mapping, GenerateError> {
    let names = adapter.mapping_namespaces.as_ref().ok_or_else(|| {
        GenerateError::Input(
            "Tiny mappings require explicit runtime and reviewed namespaces".to_owned(),
        )
    })?;
    let artifact = adapter.inputs.client_mappings.as_ref().ok_or_else(|| {
        GenerateError::Input("Tiny mappings require a pinned mapping artifact".to_owned())
    })?;
    let tool = adapter.inputs.mapping_io_jar.as_ref().ok_or_else(|| {
        GenerateError::Input("Tiny mappings require a pinned mapping-io jar".to_owned())
    })?;
    inputs::verify_input(root, artifact)?;
    inputs::verify_input(root, tool)?;
    let source = root.join(NORMALIZER_SOURCE);
    let output = Command::new(java_executable()?)
        .arg("--class-path")
        .arg(root.join(&tool.path))
        .arg(source)
        .arg(root.join(&artifact.path))
        .arg(&names.runtime)
        .arg(&names.reviewed)
        .output()
        .map_err(|error| GenerateError::Input(format!("cannot run mapping-io: {error}")))?;
    if !output.status.success() {
        return Err(GenerateError::Input(format!(
            "mapping-io refused the Tiny mapping: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let normalized = String::from_utf8(output.stdout).map_err(|error| {
        GenerateError::Input(format!("mapping-io emitted invalid UTF-8: {error}"))
    })?;
    Mapping::tiny(&normalized)
}

fn java_executable() -> Result<PathBuf, GenerateError> {
    let home = std::env::var_os("JAVA_HOME")
        .filter(|home| !home.is_empty())
        .ok_or_else(|| {
            GenerateError::Input(
                "mapping-io normalization requires JAVA_HOME to select the configured JDK"
                    .to_owned(),
            )
        })?;
    let java = PathBuf::from(home).join("bin/java");
    let output = Command::new(&java)
        .arg("-version")
        .output()
        .map_err(|error| {
            GenerateError::Input(format!(
                "cannot run mapping-io Java executable {}: {error}",
                inputs::file_name(&java.to_string_lossy())
            ))
        })?;
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(GenerateError::Input(format!(
            "cannot determine mapping-io Java version using {}: {}",
            inputs::file_name(&java.to_string_lossy()),
            diagnostic.trim()
        )));
    }
    let version = diagnostic.split('"').nth(1).ok_or_else(|| {
        GenerateError::Input(format!(
            "unrecognized mapping-io Java version: {diagnostic}"
        ))
    })?;
    let mut components = version.split(['.', '-', '+']);
    let first = components.next().unwrap_or_default();
    let major = if first == "1" {
        components.next().unwrap_or_default()
    } else {
        first
    };
    if major.parse::<u32>().map_or(true, |major| major < 11) {
        return Err(GenerateError::Input(format!(
            "mapping-io normalization requires Java 11 or newer; {} reports {version}; set JAVA_HOME to a supported JDK",
            inputs::file_name(&java.to_string_lossy())
        )));
    }
    Ok(java)
}
