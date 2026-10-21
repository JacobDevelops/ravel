use std::{
    path::{Path, PathBuf},
    process::Command,
};

use super::{GenerateError, Mapping};
use crate::inputs;

pub(crate) const NORMALIZER_SOURCE: &str =
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
    let java = java_executable()?;
    let output = Command::new(&java)
        .arg("--class-path")
        .arg(root.join(&tool.path))
        .arg(source)
        .arg(root.join(&artifact.path))
        .arg(&names.runtime)
        .arg(&names.reviewed)
        .output()
        .map_err(|error| {
            GenerateError::io(
                format!(
                    "mapping-io using {}",
                    inputs::file_name(&java.to_string_lossy())
                ),
                error,
            )
        })?;
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
            GenerateError::io(
                format!(
                    "mapping-io Java executable {}",
                    inputs::file_name(&java.to_string_lossy())
                ),
                error,
            )
        })?;
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(GenerateError::Input(format!(
            "cannot determine mapping-io Java version using {}: {}",
            inputs::file_name(&java.to_string_lossy()),
            diagnostic.trim()
        )));
    }
    ensure_supported_java(&java, &diagnostic)?;
    Ok(java)
}

fn ensure_supported_java(java: &Path, diagnostic: &str) -> Result<(), GenerateError> {
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ensure_supported_java, inputs, read};
    use crate::test_fixtures::document;
    use std::path::Path;

    #[test]
    fn missing_normalizer_inputs_are_refused() {
        let mut adapter: inputs::Adapter =
            toml::from_str(&document("unobfuscated-class-files", ""))
                .expect("complete adapter fixture");
        adapter.mapping_namespaces = Some(inputs::MappingNamespaces {
            runtime: "official".to_owned(),
            reviewed: "named".to_owned(),
        });
        adapter.inputs.client_mappings = Some(adapter.inputs.client_jar.clone());
        adapter.inputs.mapping_io_jar = Some(adapter.inputs.client_jar.clone());
        for (field, expected) in [
            (
                0,
                "Tiny mappings require explicit runtime and reviewed namespaces",
            ),
            (1, "Tiny mappings require a pinned mapping artifact"),
            (2, "Tiny mappings require a pinned mapping-io jar"),
        ] {
            let mut missing = adapter.clone();
            match field {
                0 => missing.mapping_namespaces = None,
                1 => missing.inputs.client_mappings = None,
                2 => missing.inputs.mapping_io_jar = None,
                _ => unreachable!(),
            }
            let error = read(Path::new(""), &missing).expect_err("missing required input");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn normalizer_java_version_requires_a_recognized_supported_major() {
        let java = Path::new("java");
        for version in ["11.0.1", "17.0.1", "25-ea+7", "27"] {
            let diagnostic = format!("openjdk version \"{version}\"");
            ensure_supported_java(java, &diagnostic).expect("supported Java version");
        }
        for version in ["1.8.0_402", "10.0.2", "", "invalid", "1", "1.invalid"] {
            let diagnostic = format!("java version \"{version}\"");
            let error = ensure_supported_java(java, &diagnostic).expect_err("unsupported Java");
            assert!(error.to_string().contains("requires Java 11 or newer"));
        }
        let error = ensure_supported_java(java, "missing version quotes")
            .expect_err("unrecognized version output");
        assert!(
            error
                .to_string()
                .contains("unrecognized mapping-io Java version")
        );
    }
}
