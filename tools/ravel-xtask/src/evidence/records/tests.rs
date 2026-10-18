use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::json::read_json;
use crate::test_support::{temporary_root, track_fixture_files};

mod artifacts;
mod reviews;
mod revisions;
mod runs;
mod sweep;

const EMPTY_OBJECT_DIGEST: &str =
    "ca3d163bab055381827226140568f3bef7eaac187cebd76878e0b63e9e442356";

const HELLO_DIGEST: &str = "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03";

fn pinned_input_root(label: &str) -> (PathBuf, String) {
    let root = temporary_root(label);
    let initialised = Command::new("jj")
        .args(["git", "init"])
        .current_dir(&root)
        .output()
        .expect("jj should be available");
    assert!(initialised.status.success(), "{initialised:?}");
    fs::write(root.join("input.txt"), "hello\n").expect("input should be writable");
    let pinned = Command::new("jj")
        .args(["log", "-r", "@", "--no-graph", "-T", "commit_id"])
        .current_dir(&root)
        .output()
        .expect("jj should be available");
    assert!(pinned.status.success(), "{pinned:?}");
    let advanced = Command::new("jj")
        .arg("new")
        .current_dir(&root)
        .output()
        .expect("jj should be available");
    assert!(advanced.status.success(), "{advanced:?}");
    fs::write(root.join("input.txt"), "moved on\n").expect("input should be rewritable");
    (
        root,
        String::from_utf8_lossy(&pinned.stdout).trim().to_owned(),
    )
}

fn recorded_artifact_root(label: &str, file: &str, record: &str, manifest: &str) -> PathBuf {
    let root = temporary_root(label);
    let milestone = root.join("evidence/m9");
    fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
    fs::write(milestone.join(file), record).expect("record should be writable");
    fs::write(milestone.join("manifest.json"), manifest).expect("manifest should be writable");
    root
}

// These fixtures exercise artifact binding without claiming complete milestone integrity.
fn verify_record_bindings(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    let manifests = fs::read_dir(root.join("evidence"))
        .expect("evidence directory should be readable")
        .map(|entry| {
            entry
                .expect("entry should be readable")
                .path()
                .join("manifest.json")
        })
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    if manifests.is_empty() {
        return vec![
            "evidence/ holds no milestone manifest, and a manifest sweep over nothing passes \
             vacuously"
                .to_owned(),
        ];
    }
    track_fixture_files(root);
    pin_default_record_inputs(root, &manifests);
    let tracked = crate::vcs::tracked_files(root, None)
        .expect("tracked paths should be readable")
        .into_iter()
        .collect();
    for path in manifests {
        match read_json(root, &path) {
            Ok(manifest) => {
                super::verify_recorded_manifest(
                    root,
                    &path,
                    &manifest,
                    &tracked,
                    &crate::evidence::closure::LoadedRecords::default(),
                    &mut violations,
                );
            }
            Err(error) => violations.push(error),
        }
    }
    violations
}

fn pin_default_record_inputs(root: &Path, manifests: &[PathBuf]) {
    let mut unpinned = Vec::with_capacity(manifests.len());
    for path in manifests {
        if let Ok(manifest) = read_json(root, path)
            && manifest.get("revision").is_none()
            && manifest.get("inputs").is_none()
        {
            unpinned.push((path, manifest));
        }
    }
    if unpinned.is_empty() {
        return;
    }
    let input = "record-fixture-input.txt";
    fs::write(root.join(input), "hello\n").expect("default fixture input should be writable");
    let revision = crate::vcs::resolve_revision(root, "@")
        .expect("default fixture input revision should resolve");
    let advanced = Command::new("jj")
        .args(["new", "-m", "bind record fixture inputs"])
        .current_dir(root)
        .output()
        .expect("jj should preserve the pinned fixture revision");
    assert!(advanced.status.success(), "{advanced:?}");
    for (path, mut manifest) in unpinned {
        manifest["revision"] = serde_json::json!({"id": revision});
        manifest["inputs"] = serde_json::json!([{"name": input, "sha256": HELLO_DIGEST}]);
        fs::write(
            path,
            serde_json::to_vec(&manifest).expect("fixture manifest should serialize"),
        )
        .expect("pinned fixture manifest should be writable");
    }
}
