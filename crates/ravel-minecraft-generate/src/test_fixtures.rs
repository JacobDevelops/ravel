//! Shared adapter, pinned-version, and reflective-surface fixtures for generator tests.

use std::path::Path;

use crate::inputs::{self, Adapter, Capability, PinnedVersion, Reflected, Reflection};

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

pub(super) fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate lives under crates/<name>")
}

pub(crate) fn document(source: &str, mappings: &str) -> String {
    format!(
        "schemaVersion = 1\n\
         minecraft = \"0.0.0-test\"\n\
         javaRelease = 25\n\
         mappingSource = \"{source}\"\n\
         [packFormat]\nresource = 7\nresourceMinor = 0\ndata = 11\ndataMinor = 0\n\
         spelling = \"pack-format\"\n\
         {mappings}\
         [inputs.clientJar]\npath = \"a\"\nsha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n\
         [inputs.dataFixerJar]\npath = \"b\"\nsha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n\
         [inputs.versionManifest]\npath = \"c\"\nsha256 = \"0\"\n\
         [inputs.hookInventory]\npath = \"d\"\nsha256 = \"0\"\n\
         [inputs.stubSeeds]\npath = \"e\"\nsha256 = \"0\"\n\
         [inputs.reflectiveSurface]\npath = \"h\"\nsha256 = \"0\"\n\
         [inputs.mappingsLock]\npath = \"f\"\nsha256 = \"0\"\n\
         [inputs.stubsLock]\npath = \"g\"\nsha256 = \"0\"\n"
    )
}

pub(crate) fn legacy_adapter() -> Adapter {
    let mut adapter: Adapter =
        toml::from_str(&document("unobfuscated-class-files", "")).expect("a modern adapter");
    adapter.capabilities = vec![Capability::Legacy112];
    adapter.java_release = 27;
    adapter.source_java_release = Some(8);
    adapter.pack_format.resource = 3;
    adapter.pack_format.data = 0;
    adapter.inputs.data_fixer_jar = None;
    adapter
}
