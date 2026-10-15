use std::fs;

use crate::test_support::temporary_root;
use crate::tracked::{parse_tracked_paths, verify_tracked_contents, verify_tracked_paths};

#[test]
fn tracked_credential_paths_are_rejected() {
    let violations = verify_tracked_paths(["config/credentials.json", "docs/README.md"]);

    assert_eq!(violations.len(), 1);
    assert!(
        violations
            .iter()
            .all(|violation| violation.starts_with("forbidden tracked path:"))
    );
}

#[test]
fn tracked_runtime_material_is_rejected() {
    let paths = [
        "hs_err_pid123.log",
        "nested/HS_ERR_PID456.log",
        "dump.hprof",
        "recording.jfr",
        "frame.trace",
        "logs/latest.log",
        "instance/crash-reports/crash.txt",
        "instance/saves/world/level.dat",
        "evidence/m0/raw/process.log",
        "evidence/m0/captures/frame.png",
    ];
    let violations = verify_tracked_paths(paths);

    assert_eq!(violations.len(), paths.len(), "{violations:?}");
}

#[test]
fn non_runtime_material_is_not_a_policy_violation() {
    let violations = verify_tracked_paths([
        "tests/fixtures/local/example.jar",
        "evidence/private/capture.json",
        ".minecraft/assets/example.png",
        "mods/example.jar",
        "pack.mrpack",
        "registry/mods/rvl-042-counterpart/Cargo.toml",
        "registry/fixtures/hidden\nname.rs",
    ]);

    assert!(violations.is_empty(), "violations: {violations:?}");
}

#[test]
fn tracked_credential_material_is_rejected() {
    let root = temporary_root("credentials");
    fs::write(
        root.join("notes.txt"),
        format!("{}{}\n", "-----BEGIN OPENSSH ", "PRIVATE KEY-----"),
    )
    .expect("credential canary should be writable");

    let violations = verify_tracked_contents(&root, &["notes.txt".to_owned()]);

    assert_eq!(
        violations,
        ["tracked file contains credential material: notes.txt"]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn contents_scan_discloses_what_it_does_not_refuse() {
    let root = temporary_root("disclosure");
    fs::write(
        root.join("notes.txt"),
        "cache lives at /home/someone/.local/state/ravel\n",
    )
    .expect("absolute path sample should be writable");

    fs::write(
        root.join("fixture.json"),
        "{\"files\": [{\"projectID\": 1, \"modId\": \"someupstreammod\"}]}\n",
    )
    .expect("identifier sample should be writable");

    let violations =
        verify_tracked_contents(&root, &["notes.txt".to_owned(), "fixture.json".to_owned()]);

    assert_eq!(
        violations,
        Vec::<String>::new(),
        "an absolute home path and an upstream-shaped identifier are disclosed as not refused; \
         if either now fails, update the disclosure on verify_tracked_contents and in \
         evidence/m5/manifest.json rather than this assertion"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_tracked_file_that_is_not_utf8_is_still_scanned_for_credentials() {
    let root = temporary_root("credentials-binary");
    let mut bytes = format!("{}{}\n", "-----BEGIN OPENSSH ", "PRIVATE KEY-----").into_bytes();
    bytes.push(0xff);
    fs::write(root.join("keystore.bin"), bytes).expect("canary should be writable");

    let violations = verify_tracked_contents(&root, &["keystore.bin".to_owned()]);

    assert_eq!(
        violations,
        ["tracked file contains credential material: keystore.bin"]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn nul_delimited_tracked_paths_preserve_newlines() {
    assert_eq!(
        parse_tracked_paths(b"registry/a\nb.rs\0registry/c.rs\0"),
        ["registry/a\nb.rs", "registry/c.rs"]
    );
}

#[test]
fn git_inventory_excludes_untracked_and_ignored_files() {
    let root = temporary_root("git-tracked");
    for args in [
        &["init"][..],
        &["add", "--", ".gitignore", "tracked\nfile.txt"][..],
    ] {
        if args[0] == "add" {
            fs::write(root.join(".gitignore"), "ignored.txt\n").expect("ignore should be writable");
            for name in ["tracked\nfile.txt", "untracked.txt", "ignored.txt"] {
                fs::write(root.join(name), "sample").expect("sample should be writable");
            }
        }
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(&root)
            .output()
            .expect("git should run");
        assert!(output.status.success(), "{output:?}");
    }
    assert_eq!(
        crate::tracked::tracked_paths(&root).expect("git inventory should be readable"),
        [".gitignore", "tracked\nfile.txt"]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn tracked_inventory_requires_version_control() {
    let root = temporary_root("no-vcs");
    fs::write(root.join("untracked.txt"), "sample").expect("sample should be writable");
    assert!(
        crate::tracked::tracked_paths(&root)
            .expect_err("a directory listing is not a tracked inventory")
            .contains("neither .jj nor .git")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn credential_assignments_use_shared_words_without_treating_source_as_environment() {
    for name in [
        "ATP_APP_PASSWORD",
        "AWS_SECRET_ACCESS_KEY",
        "MINECRAFT_ACCESS_TOKEN",
        "TANGLED_TOKEN",
        "AUTHORIZATION",
    ] {
        assert!(super::has_credential_assignment(&format!(
            "{name}=private-value"
        )));
        assert!(!super::has_credential_assignment(&format!(
            "{name}=<placeholder>"
        )));
    }
    assert!(!super::has_credential_assignment(
        "const TOKEN_SEGMENT: usize = 8;"
    ));
    assert!(!super::has_credential_assignment("let token = redacted;"));
}

#[test]
fn broken_git_metadata_is_an_error_instead_of_a_directory_fallback() {
    let root = temporary_root("broken-git");
    fs::write(root.join(".git"), "gitdir: missing\n").expect("git marker should be writable");
    assert!(
        crate::tracked::tracked_paths(&root)
            .expect_err("git failure must not become a disk inventory")
            .contains("cannot list tracked files with git")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
