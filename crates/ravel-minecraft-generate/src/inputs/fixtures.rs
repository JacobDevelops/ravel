//! Shared tracked-root and adapter fixtures for input responsibility tests.

use std::path::Path;

use super::Adapter;

pub(super) fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate lives under crates/<name>")
}

pub(super) fn document(source: &str, mappings: &str) -> String {
    format!(
        "schemaVersion = 1\n\
         minecraft = \"0.0.0-test\"\n\
         javaRelease = 25\n\
         mappingSource = \"{source}\"\n\
         [packFormat]\nresource = 7\nresourceMinor = 0\ndata = 11\ndataMinor = 0\n\
         spelling = \"pack-format\"\n\
         {mappings}\
         [inputs.clientJar]\npath = \"a\"\nsha256 = \"0\"\n\
         [inputs.dataFixerJar]\npath = \"b\"\nsha256 = \"0\"\n\
         [inputs.versionManifest]\npath = \"c\"\nsha256 = \"0\"\n\
         [inputs.hookInventory]\npath = \"d\"\nsha256 = \"0\"\n\
         [inputs.stubSeeds]\npath = \"e\"\nsha256 = \"0\"\n\
         [inputs.reflectiveSurface]\npath = \"h\"\nsha256 = \"0\"\n\
         [inputs.mappingsLock]\npath = \"f\"\nsha256 = \"0\"\n\
         [inputs.stubsLock]\npath = \"g\"\nsha256 = \"0\"\n"
    )
}

pub(super) fn legacy_adapter() -> Adapter {
    let mut adapter: Adapter =
        toml::from_str(&document("unobfuscated-class-files", "")).expect("a modern adapter");
    adapter.capabilities = vec![super::Capability::Legacy112];
    adapter.java_release = 27;
    adapter.source_java_release = Some(8);
    adapter.pack_format.resource = 3;
    adapter.pack_format.data = 0;
    adapter.inputs.data_fixer_jar = None;
    adapter
}
