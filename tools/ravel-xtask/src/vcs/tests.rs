use std::fs;
use std::path::Path;
use std::process::Command;

use crate::test_support::temporary_root;

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("Git runs");
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout)
        .expect("Git output is UTF-8")
        .trim()
        .to_owned()
}

fn commit(root: &Path) -> String {
    git(root, &["add", "."]);
    git(
        root,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-m",
            "fixture",
        ],
    );
    git(root, &["rev-parse", "HEAD"])
}

#[test]
fn git_inventory_preserves_newlines_and_reads_historical_bytes() {
    let root = temporary_root("vcs-git-inventory");
    git(&root, &["init"]);
    fs::write(root.join("tracked\nfile.txt"), "old").expect("historical input should be writable");
    fs::write(root.join(".gitignore"), "ignored.txt\n").expect("ignore rules should be writable");
    let revision = commit(&root);
    fs::write(root.join("tracked\nfile.txt"), "new").expect("input should be rewritable");
    fs::write(root.join("untracked.txt"), "untracked").expect("untracked input should be writable");
    fs::write(root.join("ignored.txt"), "ignored").expect("ignored input should be writable");
    let expected = [".gitignore", "tracked\nfile.txt"];
    assert_eq!(
        super::tracked_files(&root, None).expect("Git inventory should be readable"),
        expected
    );
    assert_eq!(
        super::tracked_files(&root, Some(&revision))
            .expect("historical Git inventory should be readable"),
        expected
    );
    assert_eq!(
        super::revision_bytes(&root, &revision, "tracked\nfile.txt")
            .expect("historical newline path should be readable"),
        b"old"
    );
    assert_eq!(
        super::last_modifying_revision(&root, "tracked\nfile.txt")
            .expect("newline path history should resolve"),
        revision
    );
    fs::remove_dir_all(root).expect("inventory fixture should be removable");
}

#[test]
fn git_revision_resolution_includes_other_branches_but_not_dangling_commits() {
    let root = temporary_root("vcs-git-branches");
    git(&root, &["init"]);
    fs::write(root.join("input.txt"), "base").expect("base input should be writable");
    let base = commit(&root);
    git(&root, &["checkout", "-b", "other"]);
    fs::write(root.join("input.txt"), "other").expect("branch input should be writable");
    let other = commit(&root);
    git(&root, &["checkout", "--detach", &base]);
    assert_eq!(
        super::resolve_revision(&root, "other").expect("other branch should resolve"),
        other
    );
    git(&root, &["branch", "-D", "other"]);
    assert!(super::resolve_revision(&root, &other).is_err());
    fs::remove_dir_all(root).expect("branch fixture should be removable");
}

#[test]
fn git_shallow_checkout_reports_missing_history_instead_of_skipping() {
    let source = temporary_root("vcs-shallow-source");
    let clone = temporary_root("vcs-shallow-clone");
    git(&source, &["init"]);
    fs::write(source.join("input.txt"), "old").expect("historical source input should be writable");
    let old = commit(&source);
    fs::write(source.join("input.txt"), "new").expect("source input should be rewritable");
    let head = commit(&source);
    git(
        &clone,
        &[
            "clone",
            "--depth=1",
            &format!("file://{}", source.display()),
            ".",
        ],
    );
    assert!(!clone.join(".jj").exists());
    assert_eq!(
        super::resolve_revision(&clone, &head).expect("shallow checkout head should resolve"),
        head
    );
    assert!(super::resolve_revision(&clone, &old).is_err());
    assert!(super::revision_bytes(&clone, &old, "input.txt").is_err());
    assert!(super::last_modifying_revision(&clone, "input.txt").is_err());
    jj(&clone, &["git", "init", "--colocate"]);
    assert!(super::last_modifying_revision(&clone, "input.txt").is_err());
    fs::remove_dir_all(source).expect("shallow source fixture should be removable");
    fs::remove_dir_all(clone).expect("shallow clone fixture should be removable");
}

#[test]
fn a_present_jj_workspace_is_not_silently_bypassed() {
    let root = temporary_root("vcs-prefer-jj");
    git(&root, &["init"]);
    fs::write(root.join("input.txt"), "tracked").expect("tracked input should be writable");
    commit(&root);
    fs::create_dir(root.join(".jj")).expect("invalid jj metadata directory should be creatable");
    assert!(super::tracked_files(&root, None).is_err());
    fs::remove_dir_all(root).expect("backend preference fixture should be removable");
}

fn jj(root: &Path, args: &[&str]) -> String {
    let output = Command::new("jj")
        .args(args)
        .current_dir(root)
        .output()
        .expect("jj fixture command should run");
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout)
        .expect("jj fixture output should be UTF-8")
        .trim()
        .to_owned()
}

#[test]
fn nested_directories_are_not_repository_roots() {
    let root = temporary_root("vcs-nested-root");
    git(&root, &["init"]);
    fs::write(root.join("input.txt"), "tracked").expect("root input should be writable");
    let revision = commit(&root);
    let nested = root.join("nested");
    fs::create_dir(&nested).expect("nested fixture directory should be creatable");
    assert!(super::tracked_files(&nested, None).is_err());
    assert!(super::tracked_files(&nested, Some(&revision)).is_err());
    assert!(super::resolve_revision(&nested, &revision).is_err());
    assert!(super::revision_bytes(&nested, &revision, "input.txt").is_err());
    assert!(super::last_modifying_revision(&nested, "input.txt").is_err());
    fs::remove_dir_all(root).expect("nested root fixture should be removable");
}

#[test]
fn git_history_paths_are_literal_not_globs() {
    let root = temporary_root("vcs-literal-git-path");
    git(&root, &["init"]);
    fs::write(root.join("glob*.txt"), "literal").expect("literal glob path should be writable");
    fs::write(root.join("glob-other.txt"), "other").expect("glob sibling should be writable");
    let literal = commit(&root);
    fs::write(root.join("glob-other.txt"), "changed").expect("glob sibling should be rewritable");
    commit(&root);
    assert_eq!(
        super::last_modifying_revision(&root, "glob*.txt")
            .expect("literal Git glob history should resolve"),
        literal
    );
    fs::remove_dir_all(root).expect("Git literal path fixture should be removable");
}

#[test]
fn jj_revision_paths_are_literal_for_globs_newlines_and_controls() {
    let root = temporary_root("vcs-literal-jj-paths");
    jj(&root, &["git", "init"]);
    for name in [
        "glob*.txt",
        "line\nbreak.txt",
        "control\u{1}.txt",
        "quote\".txt",
        "slash\\.txt",
    ] {
        fs::write(root.join(name), name.as_bytes()).expect("literal jj path should be writable");
    }
    fs::write(root.join("glob-other.txt"), "other").expect("jj glob sibling should be writable");
    let revision = jj(&root, &["log", "-r", "@", "--no-graph", "-T", "commit_id"]);
    jj(&root, &["new"]);
    fs::write(root.join("glob-other.txt"), "changed")
        .expect("jj glob sibling should be rewritable");
    for name in [
        "glob*.txt",
        "line\nbreak.txt",
        "control\u{1}.txt",
        "quote\".txt",
        "slash\\.txt",
    ] {
        assert_eq!(
            super::revision_bytes(&root, &revision, name)
                .expect("literal jj revision bytes should be readable"),
            name.as_bytes()
        );
        assert_eq!(
            super::last_modifying_revision(&root, name)
                .expect("literal jj path history should resolve"),
            revision
        );
    }
    fs::remove_dir_all(root).expect("jj literal path fixture should be removable");
}

#[test]
fn git_conflict_stages_produce_one_tracked_path() {
    let root = temporary_root("vcs-conflict-inventory");
    git(&root, &["init"]);
    fs::write(root.join("input.txt"), "base\n").expect("conflict base should be writable");
    let base = commit(&root);
    git(&root, &["checkout", "-b", "other"]);
    fs::write(root.join("input.txt"), "other\n").expect("other conflict side should be writable");
    commit(&root);
    git(&root, &["checkout", "-b", "current", &base]);
    fs::write(root.join("input.txt"), "current\n")
        .expect("current conflict side should be writable");
    commit(&root);
    let merge = Command::new("git")
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "merge",
            "other",
        ])
        .current_dir(&root)
        .output()
        .expect("conflicting merge command should run");
    assert!(!merge.status.success(), "{merge:?}");
    assert_eq!(
        super::tracked_files(&root, None).expect("conflicted Git index should be readable"),
        ["input.txt"]
    );
    fs::remove_dir_all(root).expect("conflict fixture should be removable");
}
