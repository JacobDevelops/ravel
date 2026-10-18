use std::fs;

use crate::scaffold::{
    declares_unsafe_prohibition, maintained_rust_roots, required_scaffold_paths,
    verify_ignored_boundaries, verify_production_fences, verify_scaffold, verify_testkit_fence,
};
use crate::test_support::{adapter_document, temporary_root, version_directory};
use crate::{CRATE_NAMES, REGISTRY_MOD_NAMES};

#[test]
fn unreadable_optional_example_directory_is_not_silently_skipped() {
    let root = temporary_root("example-directory-unreadable");
    let directory = root.join("crates").join(CRATE_NAMES[0]);
    fs::create_dir_all(&directory).expect("crate directory should be creatable");
    fs::write(directory.join("examples"), "not a directory").expect("file should be writable");
    let mut violations = Vec::new();
    maintained_rust_roots(&root, &mut violations);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].starts_with(&format!("cannot read crates/{}/examples:", CRATE_NAMES[0])),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn unsafe_code_cannot_be_permitted_in_maintained_crates() {
    let root = temporary_root("fence-unsafe");
    seed_production_manifests(&root);
    let crate_root = root.join("crates/ravel-bootstrap-runtime/src");
    fs::create_dir_all(&crate_root).expect("crate directory should be creatable");
    fs::write(
        crate_root.join("lib.rs"),
        "#![allow(unsafe_code)]\npub fn boundary() {}\n",
    )
    .expect("source should be writable");
    let mut violations = Vec::new();

    verify_production_fences(&root, &mut violations);

    assert_eq!(
        violations,
        ["unsafe code is permitted in a maintained crate: \
             crates/ravel-bootstrap-runtime/src/lib.rs"]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn missing_files_are_reported_by_relative_path() {
    let root = temporary_root("missing");

    let violations = verify_scaffold(&root);

    assert!(
        violations
            .iter()
            .any(|violation| violation == "missing required file: Cargo.toml")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn invalid_adapters_are_reported_once_by_the_overall_scaffold_check() {
    for (label, adapter, diagnostic) in [
        (
            "scaffold-invalid-toml",
            "not valid TOML".to_owned(),
            "cannot parse adapter.toml",
        ),
        (
            "scaffold-invalid-eligibility",
            adapter_document("26.1.2", "live", None).replace("'live'", "'unsupported'"),
            "unknown variant `unsupported`, expected `live` or `generation-only`",
        ),
        (
            "scaffold-reserved-eligibility",
            adapter_document("26.1.2", "live", None).replace("'live'", "'generation-only'"),
            "generation-only eligibility requires legacy-1-12",
        ),
    ] {
        let root = temporary_root(label);
        version_directory(&root, "26.1.2", &[]);
        fs::write(root.join("minecraft/versions/26.1.2/adapter.toml"), adapter)
            .expect("adapter should be writable");

        let violations = verify_scaffold(&root);

        assert_eq!(
            violations
                .iter()
                .filter(|message| message.contains(diagnostic))
                .count(),
            1,
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}

#[test]
fn malformed_hook_inventories_reach_the_final_scaffold_violations_once() {
    let root = temporary_root("scaffold-malformed-hooks");
    version_directory(&root, "26.1.2", &[]);
    fs::write(
        root.join("minecraft/versions/26.1.2/hooks.toml"),
        "[[hook]\n",
    )
    .expect("malformed inventory should be writable");

    let violations = verify_scaffold(&root);

    assert_eq!(
        violations
            .iter()
            .filter(|message| message.contains("cannot parse hooks.toml"))
            .count(),
        1,
        "{violations:?}"
    );
    assert!(
        !violations
            .iter()
            .any(|message| message.contains("checked against nothing")),
        "{violations:?}"
    );
    assert!(
        !root
            .join(crate::java_shape::TRANSFORMER_SHAPE_RECORD)
            .exists()
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn malformed_inventories_do_not_hide_other_versions_seam_checks_or_records() {
    let root = temporary_root("scaffold-mixed-hooks");
    for (version, kind) in [
        ("26.1.2", "instance-method"),
        ("99.0.0-synthetic", "static-method"),
    ] {
        version_directory(&root, version, &[]);
        fs::write(
            root.join(format!("minecraft/versions/{version}/hooks.toml")),
            format!(
                "schemaVersion = 1\nminecraft = '{version}'\n[[hook]]\n\
                 id = 'world.save-persistence'\nclass = 'net.minecraft.World'\n\
                 member = 'save'\ndescriptor = '(ZZZ)Z'\nkind = '{kind}'\n\
                 capability = 'Save the world.'\n"
            ),
        )
        .expect("reviewed inventory should be writable");
    }
    version_directory(&root, "1.21.1", &[]);
    fs::write(
        root.join("minecraft/versions/1.21.1/hooks.toml"),
        "[[hook]\n",
    )
    .expect("malformed inventory should be writable");

    let violations = verify_scaffold(&root);

    assert_eq!(
        violations
            .iter()
            .filter(|message| message.contains("cannot parse hooks.toml"))
            .count(),
        1,
        "{violations:?}"
    );
    assert_eq!(
        violations
            .iter()
            .filter(|message| message.contains("the transformer loads its receiver"))
            .count(),
        1,
        "{violations:?}"
    );
    assert!(
        violations.iter().any(|message| message.contains(
            "minecraft/versions/26.1.2/README.md does not state \
             \"carries 1 reviewed hooks and records 0 non-goals\""
        )),
        "{violations:?}"
    );
    let record = fs::read_to_string(root.join(crate::java_shape::TRANSFORMER_SHAPE_RECORD))
        .expect("valid inventory should retain its transformer record");
    assert_eq!(
        record,
        "held: Minecraft 26.1.2 against 1 transformer seams\n"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn unreadable_production_text_reports_the_read_failure() {
    let root = temporary_root("fence-unreadable");
    let directory = root.join("crates/ravel-bootstrap-runtime/src");
    fs::create_dir_all(&directory).expect("crate directory should be creatable");
    for path in [
        directory.join("lib.rs"),
        directory.parent().expect("crate root").join("Cargo.toml"),
    ] {
        fs::write(&path, [0xff]).expect("invalid UTF-8 fixture should be writable");
        let mut violations = Vec::new();
        verify_production_fences(&root, &mut violations);
        let relative = crate::text::relative_label(&root, &path);
        assert!(
            violations
                .iter()
                .any(|error| error.starts_with(&format!("cannot read {relative}:"))),
            "{violations:?}"
        );
        fs::remove_file(path).expect("fixture should be removable");
    }
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn every_required_path_is_unique() {
    let mut paths = required_scaffold_paths();
    let original_len = paths.len();
    paths.sort_unstable();
    paths.dedup();

    assert_eq!(paths.len(), original_len);
}

#[test]
fn credential_ignores_are_required() {
    let root = temporary_root("ignores");
    fs::write(root.join(".gitignore"), "# intentionally incomplete\n")
        .expect("test gitignore should be writable");
    let mut violations = Vec::new();

    verify_ignored_boundaries(&root, &mut violations);

    assert_eq!(violations.len(), 1);
    assert!(
        violations
            .iter()
            .all(|violation| violation.starts_with("missing credential ignore:"))
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn milestone_manifests_are_discovered_instead_of_statically_required() {
    assert!(
        required_scaffold_paths()
            .iter()
            .all(|path| !path.starts_with("evidence/") || !path.ends_with("/manifest.json"))
    );
}

#[test]
fn the_unsafe_prohibition_must_be_the_attribute_and_not_a_doc_comment() {
    assert!(declares_unsafe_prohibition("#![forbid(unsafe_code)]\n"));
    assert!(declares_unsafe_prohibition(
        "//! a crate\n\n    #![forbid(unsafe_code)]\n"
    ));
    assert!(!declares_unsafe_prohibition(
        "//! this crate keeps `#![forbid(unsafe_code)]` verbatim\npub fn f() {}\n"
    ));
}

#[test]
fn production_crates_cannot_depend_on_the_test_harness() {
    let root = temporary_root("fence-testkit");
    seed_production_manifests(&root);
    let crate_root = root.join("crates/ravel-loader");
    fs::create_dir_all(&crate_root).expect("crate directory should be creatable");
    fs::write(
        crate_root.join("Cargo.toml"),
        "[dependencies]\nravel-testkit.workspace = true\n",
    )
    .expect("manifest should be writable");
    let mut violations = Vec::new();

    verify_testkit_fence(&root, &mut violations);

    assert_eq!(
        violations,
        ["production crate depends on the test harness: crates/ravel-loader/Cargo.toml"]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn registry_roots_require_an_unsafe_prohibition() {
    let root = temporary_root("registry-roots");
    let maintained = maintained_rust_roots(&root, &mut Vec::new());

    for name in REGISTRY_MOD_NAMES {
        let relative = format!("registry/mods/{name}/src/lib.rs");
        assert!(
            maintained.contains(&relative),
            "{relative} must require an unsafe prohibition"
        );
    }

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn every_registry_mod_obeys_production_fences() {
    let root = temporary_root("registry-fences");

    for name in REGISTRY_MOD_NAMES {
        seed_production_manifests(&root);
        let directory = format!("registry/mods/{name}");
        let manifest = format!("{directory}/Cargo.toml");
        let source = format!("{directory}/src/lib.rs");
        fs::create_dir_all(root.join(&directory).join("src"))
            .expect("crate directory should be creatable");
        fs::write(
            root.join(&manifest),
            "[dependencies]\nravel-testkit.workspace = true\n\
             [lints.rust]\nunsafe_code = \"allow\"\n",
        )
        .expect("manifest should be writable");
        fs::write(
            root.join(&source),
            "#![allow(unsafe_code)]\npub fn boundary() {}\n",
        )
        .expect("source should be writable");
        let mut violations = Vec::new();

        verify_production_fences(&root, &mut violations);

        assert_eq!(
            violations,
            [
                format!("unsafe code is permitted in a maintained crate: {manifest}"),
                format!("production crate depends on the test harness: {manifest}"),
                format!("unsafe code is permitted in a maintained crate: {source}"),
            ]
        );
        fs::remove_dir_all(root.join(directory)).expect("crate directory should be removable");
    }

    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn unlisted_crates_and_registry_mods_cannot_escape_inventory_checks() {
    let root = temporary_root("unlisted-members");
    for directory in ["crates/unlisted", "registry/mods/unlisted"] {
        fs::create_dir_all(root.join(directory)).expect("member directory should be creatable");
        fs::write(
            root.join(directory).join("Cargo.toml"),
            "[package]\nname = \"unlisted\"\n",
        )
        .expect("member manifest should be writable");
    }

    let violations = verify_scaffold(&root);

    for expected in [
        "crates/unlisted is not listed in CRATE_NAMES",
        "registry/mods/unlisted is not listed in REGISTRY_MOD_NAMES",
    ] {
        assert!(
            violations.iter().any(|violation| violation == expected),
            "{violations:?}"
        );
    }
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn seed_production_manifests(root: &std::path::Path) {
    for relative in required_scaffold_paths() {
        if !relative.ends_with("/Cargo.toml") {
            continue;
        }
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("manifest directory"))
            .expect("manifest directory should be creatable");
        fs::write(
            path,
            "[package]\nname = 'fence-fixture'\nversion = '0.0.0'\nedition = '2024'\n\
             [lints.rust]\nunsafe_code = 'forbid'\n",
        )
        .expect("production manifest should be writable");
    }
}
