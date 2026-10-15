use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::java_shape::REFLECTION_FILE;
use crate::version_boundary::{VERSION_DATA, VERSION_FILES, VERSION_PROSE_FILE};

pub(crate) const RECORDED_RUN: &str = r#""commands":[{"exitStatus":0}],
    "tests":{"total":9,"passed":9,"failed":0,"skipped":0,"lineCoveragePercent":85.97}"#;

pub(crate) const PASS_RECORD: &str = "Result: PASS\n";

pub(crate) const PASS_RECORD_DIGEST: &str =
    "d2bcd3b6eb96afdab9d8fa0b8333538ff07cfd0764afc5a3854976209bef21e2";

pub(crate) fn version_directory(root: &Path, version: &str, without: &[&str]) {
    let directory = root.join(format!("{VERSION_DATA}/{version}"));
    fs::create_dir_all(&directory).expect("version directory should be creatable");
    for file in VERSION_FILES {
        if without.contains(&file) {
            continue;
        }
        let text = match file {
            VERSION_PROSE_FILE => format!("# Minecraft {version}\n"),
            "hooks.toml" => format!(
                "schemaVersion = 1\nminecraft = \"{version}\"\n\n[[hook]]\nid = \
                 \"level.tick\"\nkind = \"instance-method\"\nmember = \
                 \"tick\"\ndescriptor = \"()V\"\n"
            ),
            REFLECTION_FILE => format!(
                "schemaVersion = 1\nminecraft = \"{version}\"\n\n[[reflected]]\nname = \
                 \"ONE\"\nclass = \"net.minecraft.client.One\"\nruntime = \
                 \"net.minecraft.client.One\"\n"
            ),
            "adapter.toml" => format!(
                "schemaVersion = 1\nminecraft = \"{version}\"\n\n[packFormat]\n\
                 resource = 1\nresourceMinor = 0\ndata = 2\ndataMinor = 0\n\
                 spelling = \"pack-format\"\n"
            ),
            _ => format!("schemaVersion = 1\nminecraft = \"{version}\"\n"),
        };
        fs::write(directory.join(file), text).expect("version data should be writable");
    }
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

pub(crate) fn misstate_figure(manifest: &Path, path: &str) {
    let output = Command::new("jq")
        .arg(format!("{path} = 987654"))
        .arg(manifest)
        .output()
        .expect("jq should run");
    assert!(output.status.success(), "jq could not set {path}");
    fs::write(manifest, output.stdout).expect("manifest should be writable");
}

pub(crate) fn temporary_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("ravel-xtask-{label}-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("stale temporary root should be removable");
    }
    fs::create_dir_all(&root).expect("temporary root should be creatable");
    root
}
