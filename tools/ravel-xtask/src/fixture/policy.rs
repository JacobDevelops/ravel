//! Public descriptor admission policy, independent of archive and download I/O.

use std::path::{Component, Path};

use super::{FixtureDescriptor, FixtureError};

const ALLOWED_DOWNLOAD_PREFIX: &str = "https://mediafilez.forgecdn.net/";

pub(super) fn verify_descriptor(descriptor: &FixtureDescriptor) -> Result<(), FixtureError> {
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
