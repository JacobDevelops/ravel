//! Admits adapter capabilities, mapping configuration, and declared upstreams.
//! File bytes and runtime metadata are verified separately after admission.

use super::{Adapter, Eligibility, GenerateError, MappingSource, PackFormat, Spelling, file_name};

pub(super) fn verify_pinned_sources(adapter: &Adapter) -> Result<(), GenerateError> {
    verify_capabilities(adapter)?;
    verify_mapping_configuration(adapter)?;
    match (
        adapter.mapping_source.needs_mappings(),
        adapter.inputs.client_mappings.is_some(),
    ) {
        (true, false) => {
            return Err(GenerateError::Input(format!(
                "{} reads a published mapping but the adapter inputs declare none",
                adapter.mapping_source.as_str()
            )));
        }
        (false, true) => {
            return Err(GenerateError::Input(format!(
                "{} reads names from the class files but the adapter inputs declare a mapping",
                adapter.mapping_source.as_str()
            )));
        }
        (true, true) | (false, false) => {}
    }

    for input in [
        &adapter.inputs.client_jar,
        &adapter.inputs.version_manifest,
        &adapter.inputs.hook_inventory,
        &adapter.inputs.stub_seeds,
        &adapter.inputs.reflective_surface,
    ]
    .into_iter()
    .chain(adapter.inputs.client_mappings.as_ref())
    .chain(adapter.inputs.server_jar.as_ref())
    .chain(adapter.inputs.data_fixer_jar.as_ref())
    .chain(adapter.inputs.mapping_io_jar.as_ref())
    {
        if let Some(upstream) = &input.upstream
            && !upstream.starts_with("https://")
        {
            return Err(GenerateError::Input(format!(
                "{} names an upstream that is not an https address",
                file_name(&input.path)
            )));
        }
    }
    Ok(())
}

fn verify_mapping_configuration(adapter: &Adapter) -> Result<(), GenerateError> {
    let feather = adapter.mapping_source == MappingSource::FeatherTinyMappings;
    let namespaces = adapter.mapping_namespaces.is_some();
    let normalizer = adapter.inputs.mapping_io_jar.is_some();
    if (feather && (!namespaces || !normalizer)) || (!feather && (namespaces || normalizer)) {
        return Err(GenerateError::Input(
            "feather-tiny-mappings requires mappingNamespaces and mappingIoJar; other sources \
             must not declare either"
                .to_owned(),
        ));
    }
    if let Some(namespaces) = &adapter.mapping_namespaces
        && (namespaces.runtime.is_empty()
            || namespaces.reviewed.is_empty()
            || namespaces.runtime == namespaces.reviewed
            || namespaces
                .runtime
                .chars()
                .any(|value| value.is_whitespace() || value.is_control())
            || namespaces
                .reviewed
                .chars()
                .any(|value| value.is_whitespace() || value.is_control()))
    {
        return Err(GenerateError::Input(
            "mappingNamespaces must name distinct, nonempty runtime and reviewed namespaces \
             without whitespace or control characters"
                .to_owned(),
        ));
    }
    if let Some(normalizer) = &adapter.inputs.mapping_io_jar
        && normalizer.sha256.is_none()
    {
        return Err(GenerateError::Input(
            "mappingIoJar executes code and must pin sha256, not only a byte length".to_owned(),
        ));
    }
    Ok(())
}

/// Legacy metadata is an explicit contract, not a guess from missing fields.
fn verify_capabilities(adapter: &Adapter) -> Result<(), GenerateError> {
    if adapter.eligibility == Eligibility::GenerationOnly && !adapter.is_legacy() {
        return Err(GenerateError::Input(
            "generation-only eligibility requires legacy-1-12".to_owned(),
        ));
    }
    if let Some(server) = &adapter.inputs.server_jar
        && server.sha256.is_none()
    {
        return Err(GenerateError::Input(
            "serverJar must pin sha256, not only a byte length".to_owned(),
        ));
    }
    if adapter.is_legacy() {
        let expected = PackFormat {
            resource: 3,
            resource_minor: 0,
            data: 0,
            data_minor: 0,
            spelling: Spelling::PackFormat,
        };
        if adapter.capabilities.len() != 1
            || adapter.java_release != 27
            || adapter.source_java_release != Some(8)
            || adapter.pack_format != expected
            || adapter.inputs.data_fixer_jar.is_some()
        {
            return Err(GenerateError::Input(
                "legacy-1-12 requires sourceJavaRelease 8, javaRelease 27, resource format 3, \
                 no data packs, pack-format spelling, and no data fixer jar; \
                 declare the capability exactly once"
                    .to_owned(),
            ));
        }
    } else if adapter.inputs.data_fixer_jar.is_none() {
        return Err(GenerateError::Input(
            "modern adapter inputs require dataFixerJar; omission requires legacy-1-12".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{Capability, MappingNamespaces};
    use super::{Adapter, MappingSource};
    use crate::test_fixtures::{document, legacy_adapter};
    #[test]
    fn source_release_defaults_to_modern_target_and_legacy_requires_explicit_eight() {
        let modern: Adapter =
            toml::from_str(&document("unobfuscated-class-files", "")).expect("modern inputs");
        assert_eq!(modern.effective_source_java_release(), modern.java_release);
        let legacy = legacy_adapter();
        assert_eq!(legacy.effective_source_java_release(), 8);
        assert_eq!(legacy.java_release, 27);
        for source in [None, Some(7), Some(27)] {
            let mut wrong = legacy.clone();
            wrong.source_java_release = source;
            super::verify_pinned_sources(&wrong)
                .expect_err("legacy source release must be explicit");
        }
    }

    #[test]
    fn generation_only_is_an_explicit_legacy_admission() {
        let mut legacy = legacy_adapter();
        legacy.eligibility = super::Eligibility::GenerationOnly;
        super::verify_pinned_sources(&legacy).expect("legacy generation remains available");
        let mut modern: Adapter =
            toml::from_str(&document("unobfuscated-class-files", "")).expect("modern inputs");
        assert_eq!(modern.eligibility, super::Eligibility::Live);
        modern.eligibility = super::Eligibility::GenerationOnly;
        super::verify_pinned_sources(&modern).expect_err("modern obligations cannot be opted out");
    }

    #[test]
    fn server_jar_requires_hash_and_https_upstream() {
        let mut adapter = legacy_adapter();
        let mut server = adapter.inputs.client_jar.clone();
        server.sha256 = None;
        server.bytes = Some(100);
        adapter.inputs.server_jar = Some(server.clone());
        let error = super::verify_pinned_sources(&adapter).expect_err("length is not identity");
        assert!(
            error.to_string().contains("serverJar must pin sha256"),
            "{error}"
        );
        server.sha256 = Some("0".to_owned());
        server.upstream = Some("http://example.invalid/server.jar".to_owned());
        adapter.inputs.server_jar = Some(server);
        let error = super::verify_pinned_sources(&adapter).expect_err("insecure upstream");
        assert!(
            error.to_string().contains("not an https address"),
            "{error}"
        );
    }

    #[test]
    fn legacy_capability_refuses_inconsistent_metadata_and_a_dfu_pin() {
        let legacy = legacy_adapter();
        let mut wrong = Vec::new();
        let mut adapter = legacy.clone();
        adapter.java_release = 21;
        wrong.push(adapter);
        let mut adapter = legacy.clone();
        adapter.pack_format.data = 1;
        wrong.push(adapter);
        let mut adapter = legacy.clone();
        adapter.pack_format.resource_minor = 1;
        wrong.push(adapter);
        let mut adapter = legacy.clone();
        adapter.pack_format.resource = 4;
        wrong.push(adapter);
        let mut adapter = legacy.clone();
        adapter.pack_format.spelling = super::Spelling::MinMaxFormat;
        wrong.push(adapter);
        let mut adapter = legacy.clone();
        adapter.inputs.data_fixer_jar = Some(adapter.inputs.client_jar.clone());
        wrong.push(adapter);
        let mut adapter = legacy;
        adapter.capabilities.push(Capability::Legacy112);
        wrong.push(adapter);
        for adapter in wrong {
            super::verify_pinned_sources(&adapter).expect_err("legacy metadata is exact");
        }
    }

    #[test]
    fn optional_mapping_io_upstreams_obey_input_admission() {
        let mut adapter = legacy_adapter();
        let mut input = adapter.inputs.client_jar.clone();
        input.upstream = Some("http://example.invalid/input.jar".to_owned());
        adapter.inputs.mapping_io_jar = Some(input);
        adapter.mapping_source = MappingSource::FeatherTinyMappings;
        adapter.inputs.client_mappings = Some(adapter.inputs.client_jar.clone());
        adapter.mapping_namespaces = Some(MappingNamespaces {
            runtime: "official".to_owned(),
            reviewed: "named".to_owned(),
        });
        let error = super::verify_pinned_sources(&adapter).expect_err("upstreams require https");
        assert!(
            error.to_string().contains("not an https address"),
            "{error}"
        );
    }

    #[test]
    fn mapping_configuration_is_required_only_for_the_tiny_source() {
        for source in [
            MappingSource::UnobfuscatedClassFiles,
            MappingSource::OfficialProguardMappings,
            MappingSource::FeatherTinyMappings,
        ] {
            for namespaces in [false, true] {
                for normalizer in [false, true] {
                    let mut adapter = legacy_adapter();
                    adapter.mapping_source = source;
                    adapter.mapping_namespaces = namespaces.then(|| MappingNamespaces {
                        runtime: "official".to_owned(),
                        reviewed: "named".to_owned(),
                    });
                    adapter.inputs.mapping_io_jar =
                        normalizer.then(|| adapter.inputs.client_jar.clone());
                    assert_eq!(
                        super::verify_mapping_configuration(&adapter).is_ok(),
                        if source == MappingSource::FeatherTinyMappings {
                            namespaces && normalizer
                        } else {
                            !namespaces && !normalizer
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn tiny_namespaces_must_be_nonempty_and_distinct_even_without_artifacts() {
        for (runtime, reviewed) in [
            ("", "named"),
            ("official", ""),
            ("named", "named"),
            ("off icial", "named"),
            ("official", " named"),
            ("official\n", "named"),
            ("official", "named\u{7f}"),
        ] {
            let mut adapter = legacy_adapter();
            adapter.mapping_source = MappingSource::FeatherTinyMappings;
            adapter.inputs.client_mappings = Some(adapter.inputs.client_jar.clone());
            adapter.inputs.mapping_io_jar = Some(adapter.inputs.client_jar.clone());
            adapter.mapping_namespaces = Some(MappingNamespaces {
                runtime: runtime.to_owned(),
                reviewed: reviewed.to_owned(),
            });
            let error = super::verify_pinned_sources(&adapter)
                .expect_err("namespace validity does not depend on available artifacts");
            assert!(error.to_string().contains("distinct, nonempty"), "{error}");
        }
    }

    #[test]
    fn a_normalizer_without_a_hash_is_refused_even_when_its_length_is_pinned() {
        let mut adapter = legacy_adapter();
        adapter.mapping_source = MappingSource::FeatherTinyMappings;
        adapter.inputs.client_mappings = Some(adapter.inputs.client_jar.clone());
        adapter.mapping_namespaces = Some(MappingNamespaces {
            runtime: "official".to_owned(),
            reviewed: "named".to_owned(),
        });
        let mut normalizer = adapter.inputs.client_jar.clone();
        normalizer.sha256 = None;
        normalizer.bytes = Some(100);
        adapter.inputs.mapping_io_jar = Some(normalizer);
        let error = super::verify_pinned_sources(&adapter)
            .expect_err("equal length cannot establish executable jar identity");
        assert!(error.to_string().contains("must pin sha256"), "{error}");
    }
}
