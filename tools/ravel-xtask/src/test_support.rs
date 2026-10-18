use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::scaffold::java_shape::REFLECTION_FILE;
use crate::version_boundary::{TOML_VERSION_FILES, VERSION_DATA, VERSION_PROSE_FILE};

pub(crate) const RECORDED_RUN: &str = r#""commands":[{"exitStatus":0}],
    "tests":{"total":9,"passed":9,"failed":0,"skipped":0,"lineCoveragePercent":85.97}"#;

pub(crate) const PASS_RECORD: &str = "Result: PASS\n";

pub(crate) const PASS_RECORD_DIGEST: &str =
    "d2bcd3b6eb96afdab9d8fa0b8333538ff07cfd0764afc5a3854976209bef21e2";

pub(crate) fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub(crate) fn hook_inventories(
    root: &Path,
    versions: &[String],
) -> Result<Vec<ravel_minecraft_generate::HookInventory>, String> {
    versions
        .iter()
        .map(|version| crate::hooks::hook_inventory(root, version))
        .collect()
}

pub(crate) fn version_directory(root: &Path, version: &str, without: &[&str]) {
    let directory = root.join(format!("{VERSION_DATA}/{version}"));
    fs::create_dir_all(&directory).expect("version directory should be creatable");
    for file in
        TOML_VERSION_FILES
            .into_iter()
            .chain([VERSION_PROSE_FILE, "adapter.toml", "hooks.toml"])
    {
        if without.contains(&file) {
            continue;
        }
        let text = match file {
            VERSION_PROSE_FILE => format!("# Minecraft {version}\n"),
            "hooks.toml" => format!(
                "schemaVersion = 1\nminecraft = \"{version}\"\n\n[[hook]]\nid = \
                 \"level.tick\"\nclass = \"net.minecraft.client.One\"\nkind = \
                 \"instance-method\"\nmember = \"tick\"\ndescriptor = \"()V\"\n\
                 capability = \"Tick the level.\"\n"
            ),
            REFLECTION_FILE => format!(
                "schemaVersion = 1\nminecraft = \"{version}\"\n\n[[reflected]]\nname = \
                 \"ONE\"\nclass = \"net.minecraft.client.One\"\nruntime = \
                 \"net.minecraft.client.One\"\n"
            ),
            "adapter.toml" => adapter_document(version, "live", None),
            _ => format!("schemaVersion = 1\nminecraft = \"{version}\"\n"),
        };
        fs::write(directory.join(file), text).expect("version data should be writable");
    }
}

pub(crate) fn adapter_document(version: &str, eligibility: &str, mapping: Option<&str>) -> String {
    let (legacy, data_fixer, resource, data) = if eligibility == "generation-only" {
        (
            "capabilities = ['legacy-1-12']\nsourceJavaRelease = 8\n",
            "",
            3,
            0,
        )
    } else {
        (
            "",
            "[inputs.dataFixerJar]\npath = 'data-fixer.jar'\nsha256 = '0000000000000000000000000000000000000000000000000000000000000000'\n",
            1,
            2,
        )
    };
    let (source, mappings) = mapping.map_or_else(
        || ("unobfuscated-class-files", String::new()),
        |path| {
            (
                "official-proguard-mappings",
                format!("[inputs.clientMappings]\npath = '{path}'\nsha256 = '0000000000000000000000000000000000000000000000000000000000000000'\n"),
            )
        },
    );
    format!(
        "schemaVersion = 1\nminecraft = '{version}'\neligibility = '{eligibility}'\n\
         javaRelease = 27\nmappingSource = '{source}'\n{legacy}\
         [packFormat]\nresource = {resource}\nresourceMinor = 0\ndata = {data}\ndataMinor = 0\n\
         spelling = 'pack-format'\n{mappings}{data_fixer}\
         [inputs.clientJar]\npath = 'client.jar'\nsha256 = '0000000000000000000000000000000000000000000000000000000000000000'\n\
         [inputs.versionManifest]\npath = 'version.json'\n\
         [inputs.hookInventory]\npath = 'minecraft/versions/{version}/hooks.toml'\n\
         [inputs.stubSeeds]\npath = 'minecraft/versions/{version}/stubs.toml'\n\
         [inputs.reflectiveSurface]\npath = 'minecraft/versions/{version}/reflection.toml'\n\
         [inputs.mappingsLock]\npath = 'minecraft/versions/{version}/mappings.lock'\n\
         [inputs.stubsLock]\npath = 'minecraft/versions/{version}/stubs.lock'\n"
    )
}

pub(crate) fn balanced_summary() -> String {
    r#"{"artifacts":{"total":10,"catalogued":10,"uncatalogued":0,"planned":10},
        "graph":{"nodes":12,"resolvedNodes":11,"blockedNodes":1,"edges":20,
            "resolvedEdges":20,"blockedEdges":0,"clusters":3,"largestCluster":5},
        "rights":{"sourceUse":{"permitted":4,"conditional":3,"prohibited":2,
            "unresolved":1},"publishable":0},
        "surfaces":{"catalogued":10,"unclassified":0},
        "plans":{"planned":10,"readingSource":4,"notReadingSource":6,
            "canonicalIds":10,"dependencyEdges":7},
        "reads":{"producersWatched":2,"archiveReads":99,"entryBodyReads":0},
        "boundArtifacts":[{"name":"a.json","sha256":"0","holds":"x"}]}"#
        .to_owned()
}

pub(crate) fn summary_root(label: &str, summary: &str) -> PathBuf {
    let root = temporary_root(label);
    let milestone = root.join("evidence/m5");
    fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
    fs::write(milestone.join("inventory-summary.json"), summary)
        .expect("summary should be writable");
    root
}

pub(crate) type ClosureCase<'a> = (&'a str, &'a [(&'a str, &'a str)], &'a str);

pub(crate) fn mutate(document: &str, replacements: &[(&str, &str)]) -> String {
    let mut mutated = document.to_owned();
    for (from, to) in replacements {
        assert!(
            mutated.contains(from),
            "the fixture does not carry `{from}`, so replacing it mutates nothing"
        );
        mutated = mutated.replace(from, to);
    }
    mutated
}

pub(crate) fn misstate_figure(root: &Path, manifest: &Path, path: &str) {
    let mut document = crate::json::read_json(root, manifest).expect("manifest should parse");
    *document
        .pointer_mut(path)
        .expect("carried figure should exist") = serde_json::json!(987_654);
    fs::write(
        manifest,
        serde_json::to_vec(&document).expect("manifest should serialize"),
    )
    .expect("manifest should be writable");
}

pub(crate) fn temporary_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("ravel-xtask-{label}-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("stale temporary root should be removable");
    }
    fs::create_dir_all(&root).expect("temporary root should be creatable");
    root
}

pub(crate) fn track_fixture_files(root: &Path) {
    if root.join(".jj").exists() {
        return;
    }
    let result = Command::new("jj")
        .args(["git", "init"])
        .current_dir(root)
        .output()
        .expect("jj should be available");
    assert!(result.status.success(), "{result:?}");
}
