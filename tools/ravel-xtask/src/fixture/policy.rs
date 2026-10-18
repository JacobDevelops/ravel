//! Public descriptor admission policy, independent of archive and download I/O.

use std::path::{Component, Path};

use super::{FixtureDescriptor, FixtureError};

const ALLOWED_DOWNLOAD_PREFIX: &str = "https://mediafilez.forgecdn.net/";

pub(super) fn validate_descriptor(descriptor: &FixtureDescriptor) -> Result<(), FixtureError> {
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

pub(super) fn is_safe_component(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('.')
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && !value.contains(['/', '\\'])
}
