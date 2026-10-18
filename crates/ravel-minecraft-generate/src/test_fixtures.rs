//! Shared pinned-version and reflective-surface fixtures for generator tests.

use std::path::Path;

use crate::inputs::{self, PinnedVersion, Reflected, Reflection};

/// Selects a tracked version whose class files already use reviewed names.
pub(crate) fn identity_version() -> PinnedVersion {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate lives under crates/<name>");
    inputs::tracked_version_where(root, |adapter| !adapter.mapping_source.needs_mappings())
}

/// Binds invented reflection roles without introducing member translations.
pub(crate) fn reflection(bound: &[(&str, &str)]) -> Reflection {
    Reflection {
        schema_version: 1,
        minecraft: String::from("0.0.0"),
        classes: bound
            .iter()
            .map(|(name, class)| Reflected {
                name: (*name).to_owned(),
                class: (*class).to_owned(),
                runtime: (*class).to_owned(),
                members: Vec::new(),
            })
            .collect(),
    }
}
