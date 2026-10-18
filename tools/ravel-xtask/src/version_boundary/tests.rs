use std::fs;
use std::path::Path;

use crate::java_shape::REFLECTION_FILE;
use crate::test_support::{adapter_document, temporary_root, version_directory};
use crate::version_boundary::{
    SWEPT_SCOPE, VERSION_CLASS_EXCUSES, VERSION_DATA, VERSION_FILES, VERSION_PROSE_FILE,
    verify_version_boundary, verify_version_directories, version_class_names, version_directories,
};

#[test]
fn matrix_eligibility_uses_the_declared_mapping_instead_of_its_extension() {
    let root = temporary_root("matrix-mappings");
    version_directory(&root, "1.12.2", &[]);
    let adapter = root.join(format!("{VERSION_DATA}/1.12.2/adapter.toml"));
    fs::write(
        &adapter,
        format!(
            "{}\n[mappingNamespaces]\nruntime = 'official'\nreviewed = 'named'\n\
             [inputs.mappingIoJar]\npath = 'mapping-io.jar'\nsha256 = '0'\n",
            adapter_document("1.12.2", "generation-only", Some("mapping.tiny"))
                .replace("official-proguard-mappings", "feather-tiny-mappings")
        ),
    )
    .expect("adapter should be writable");
    assert_eq!(
        super::version_matrix(&root, "1.12.2").expect("valid eligibility"),
        "unavailable: mapping.tiny is absent"
    );
    fs::write(root.join("mapping.tiny"), "pinned by generator")
        .expect("mapping should be writable");
    assert_eq!(
        super::version_matrix(&root, "1.12.2").expect("valid eligibility"),
        "unavailable: mapping-io.jar is absent"
    );
    fs::write(root.join("mapping-io.jar"), "pinned by generator")
        .expect("normalizer should be writable");
    assert_eq!(
        super::version_matrix(&root, "1.12.2").expect("valid eligibility"),
        "generation-only"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn modern_versions_cannot_downgrade_or_misspell_eligibility() {
    let root = temporary_root("modern-eligibility");
    version_directory(&root, "26.1.2", &[]);
    let adapter = root.join(format!("{VERSION_DATA}/26.1.2/adapter.toml"));
    let valid = adapter_document("26.1.2", "live", None);
    for eligibility in ["generation-only", "not-run", ""] {
        fs::write(
            &adapter,
            valid.replace("'live'", &format!("'{eligibility}'")),
        )
        .expect("adapter should be writable");
        assert!(super::version_matrix(&root, "26.1.2").is_err());
    }
    fs::write(&adapter, valid.replace("eligibility = 'live'\n", ""))
        .expect("adapter should be writable");
    assert_eq!(
        super::version_matrix(&root, "26.1.2").expect("default eligibility"),
        "live"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_scaffolded_modern_manifest_cannot_opt_out_of_live_coverage() {
    let root = temporary_root("modern-downgrade");
    version_directory(&root, "26.1.2", &[]);
    fs::write(
        root.join(format!("{VERSION_DATA}/26.1.2/adapter.toml")),
        adapter_document("26.1.2", "live", None).replace("'live'", "'generation-only'"),
    )
    .expect("adapter should be writable");
    assert!(super::version_matrix(&root, "26.1.2").is_err());
    let mut violations = Vec::new();
    verify_version_directories(&root, &mut violations);
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("generation-only")),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn version_specific_crates_and_production_literals_are_rejected() {
    let root = temporary_root("versions");
    version_specific_crates_fixture(&root);
    let mut violations = Vec::new();

    verify_version_boundary(&root, &mut violations);

    assert!(
        violations
            .iter()
            .any(|violation| violation.starts_with("unexpected per-version Minecraft crate:")),
        "{violations:?}"
    );
    let swept = violations
        .iter()
        .filter(|violation| violation.contains(SWEPT_SCOPE))
        .collect::<Vec<_>>();
    assert!(
        swept.iter().any(|violation| violation
            .contains("hardcoded Minecraft version outside version data")
            && violation.contains("ravel-minecraft-test-version")),
        "{violations:?}"
    );
    assert!(
        swept
            .iter()
            .any(|violation| violation.contains("names com/testgame/Thing")),
        "a class the data declares is a name no general crate may spell: {violations:?}"
    );
    assert!(
        swept
            .iter()
            .any(|violation| violation.contains("names com/testgame/Other")),
        "a class only a descriptor names is in the set too: {violations:?}"
    );
    assert!(
        !violations
            .iter()
            .any(|violation| violation.contains("java/lang/String")),
        "the platform is not the game: {violations:?}"
    );
    assert_eq!(swept.len(), 3, "{violations:?}");

    let excused = VERSION_CLASS_EXCUSES
        .iter()
        .map(|(_, classes, _)| classes.len())
        .sum::<usize>();
    assert_eq!(
        violations
            .iter()
            .filter(|violation| violation.starts_with("stale version-class excuse:"))
            .count(),
        excused,
        "{violations:?}"
    );
    assert_eq!(violations.len(), 4 + excused, "{violations:?}");
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_version_directory_missing_a_required_file_is_reported() {
    let root = temporary_root("version-files");
    version_directory(&root, "26.1.2", &[]);
    version_directory(&root, "99.0.0-synthetic", &["stubs.lock"]);
    let mut violations = Vec::new();

    verify_version_directories(&root, &mut violations);

    assert_eq!(
        violations,
        ["missing required file: minecraft/versions/99.0.0-synthetic/stubs.lock"]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_version_whose_inventory_cannot_be_counted_is_reported() {
    let root = temporary_root("version-counts");
    version_directory(&root, "26.1.2", &[]);
    fs::write(
        root.join(format!("{VERSION_DATA}/26.1.2/hooks.toml")),
        "schemaVersion = 1\nminecraft = \"26.1.2\"\n\n[[hook]]\nid = \"an.id\"\n",
    )
    .expect("version data should be writable");
    let mut violations = Vec::new();

    verify_version_directories(&root, &mut violations);

    assert_eq!(
        violations,
        [
            "cannot count minecraft/versions/26.1.2/hooks.toml: a `[[hook]]` entry declares no \
          kind or no descriptor"
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_version_whose_inventory_declares_no_hook_is_reported() {
    let root = temporary_root("version-empty-inventory");
    version_directory(&root, "26.1.2", &[]);
    fs::write(
        root.join(format!("{VERSION_DATA}/26.1.2/hooks.toml")),
        "schemaVersion = 1\nminecraft = \"26.1.2\"\n",
    )
    .expect("version data should be writable");
    let mut violations = Vec::new();

    verify_version_directories(&root, &mut violations);

    assert_eq!(
        violations,
        [
            "cannot count minecraft/versions/26.1.2/hooks.toml: the inventory declares no \
             `[[hook]]` entry"
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_version_directory_declaring_another_versions_data_is_reported() {
    let root = temporary_root("version-declaration");
    version_directory(&root, "26.1.2", &[]);
    version_directory(&root, "1.21.1", &[]);
    for file in VERSION_FILES {
        if file == VERSION_PROSE_FILE {
            continue;
        }
        let source = root.join(format!("{VERSION_DATA}/26.1.2/{file}"));
        let text = fs::read_to_string(source).expect("version data should be readable");
        fs::write(root.join(format!("{VERSION_DATA}/1.21.1/{file}")), text)
            .expect("version data should be writable");
    }
    let mut violations = Vec::new();

    verify_version_directories(&root, &mut violations);

    assert_eq!(violations.len(), VERSION_FILES.len() - 1, "{violations:?}");
    for violation in &violations {
        assert!(
            violation.contains("26.1.2") && violation.contains("1.21.1"),
            "{violations:?}"
        );
    }
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_version_declaring_no_pack_format_is_reported() {
    let root = temporary_root("pack-format");
    version_directory(&root, "26.1.2", &[]);
    let adapter = root.join(format!("{VERSION_DATA}/26.1.2/adapter.toml"));

    let valid: toml::Table =
        toml::from_str(&adapter_document("26.1.2", "live", None)).expect("valid typed adapter");
    for key in [
        "packFormat",
        "resource",
        "resourceMinor",
        "data",
        "dataMinor",
        "spelling",
    ] {
        let mut document = valid.clone();
        if key == "packFormat" {
            document.remove(key);
        } else {
            document
                .get_mut("packFormat")
                .expect("format table")
                .as_table_mut()
                .expect("format table")
                .remove(key);
        }
        fs::write(
            &adapter,
            toml::to_string(&document).expect("serializable adapter"),
        )
        .expect("version data should be writable");
        let mut violations = Vec::new();
        verify_version_directories(&root, &mut violations);
        assert_eq!(violations.len(), 1, "{key}: {violations:?}");
        assert!(violations[0].contains(key), "{key}: {violations:?}");
    }
    fs::write(&adapter, adapter_document("26.1.2", "live", None))
        .expect("version data should be writable");
    let mut violations = Vec::new();
    verify_version_directories(&root, &mut violations);
    assert!(violations.is_empty(), "{violations:?}");
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_version_file_declaring_no_version_is_reported() {
    let root = temporary_root("version-undeclared");
    version_directory(&root, "26.1.2", &[]);
    fs::write(
        root.join(format!("{VERSION_DATA}/26.1.2/stubs.toml")),
        "schemaVersion = 1\n",
    )
    .expect("version data should be writable");
    let mut violations = Vec::new();

    verify_version_directories(&root, &mut violations);

    assert_eq!(
        violations,
        [
            "minecraft/versions/26.1.2/stubs.toml declares no `minecraft` version for \
             minecraft/versions/26.1.2"
        ]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_version_tree_holding_no_version_is_not_read_as_agreement() {
    let root = temporary_root("version-empty");
    fs::create_dir_all(root.join(VERSION_DATA)).expect("version tree should be creatable");
    let mut violations = Vec::new();

    verify_version_directories(&root, &mut violations);

    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("holds no version directory"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn unreadable_version_roots_are_reported() {
    let root = temporary_root("version-root-unreadable");
    fs::create_dir_all(root.join("minecraft")).expect("parent should be creatable");
    fs::write(root.join(VERSION_DATA), "not a directory")
        .expect("version root obstruction should be writable");
    let error = version_directories(&root, &[VERSION_DATA])
        .expect_err("a version root must be readable as a directory");
    assert!(error.contains("cannot read minecraft/versions"), "{error}");
    let mut violations = Vec::new();
    verify_version_directories(&root, &mut violations);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(violations[0].contains("cannot read minecraft/versions"));
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn malformed_non_adapter_version_files_are_reported() {
    let root = temporary_root("version-malformed-stubs");
    version_directory(&root, "26.1.2", &[]);
    fs::write(
        root.join(format!("{VERSION_DATA}/26.1.2/stubs.toml")),
        "[stub\n",
    )
    .expect("version data should be writable");
    let mut violations = Vec::new();
    verify_version_directories(&root, &mut violations);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("cannot parse minecraft/versions/26.1.2/stubs.toml as TOML"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn unreadable_and_malformed_class_inventory_files_are_reported() {
    let root = temporary_root("version-class-input-errors");
    version_directory(&root, "26.1.2", &[]);
    let directory = root.join(format!("{VERSION_DATA}/26.1.2"));
    fs::remove_file(directory.join("hooks.toml")).expect("hook file should be removable");
    fs::create_dir(directory.join("hooks.toml")).expect("unreadable hook path should be creatable");
    fs::write(directory.join("stubs.toml"), "[stub\n").expect("malformed stubs should be writable");
    let mut violations = Vec::new();
    version_class_names(&root, &[directory], &mut violations);
    assert_eq!(violations.len(), 2, "{violations:?}");
    assert!(violations[0].contains("cannot read") && violations[0].contains("hooks.toml"));
    assert!(violations[1].contains("cannot parse") && violations[1].contains("stubs.toml"));
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_reflected_class_name_is_version_data_the_boundary_sweep_reads() {
    let root = temporary_root("version-classes");
    version_directory(&root, "26.1.2", &[]);
    fs::write(
        root.join(format!("{VERSION_DATA}/26.1.2/{REFLECTION_FILE}")),
        "schemaVersion = 1\nminecraft = \"26.1.2\"\n\n[[reflected]]\n\
         class = 'net.minecraft.resources.Identifier' # literal string\n\
         [[reflected.member]]\nclass = \"net.minecraft.Screen\" # basic string\n",
    )
    .expect("version data should be writable");

    let directories = version_directories(&root, &[VERSION_DATA])
        .expect("version directories should be readable");
    let mut violations = Vec::new();
    let classes = version_class_names(&root, &directories, &mut violations);
    assert!(violations.is_empty(), "{violations:?}");

    assert!(
        classes.contains(&"net/minecraft/resources/Identifier".to_owned()),
        "{classes:?}"
    );
    assert!(
        classes.contains(&"net/minecraft/Screen".to_owned()),
        "{classes:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_retired_declaration_is_a_violation_in_every_spelling() {
    for (label, declaration) in [
        ("bare", "[harness.notRun]\n"),
        (
            "keyed",
            "[harness.notRun]\npersistence = \"an obstacle; work package: none\"\n",
        ),
        ("spaced", "  [harness.notRun]\npersistence = \"none\"\n"),
        ("dotted", "[harness]\nnotRun.persistence = \"none\"\n"),
        ("inline", "[harness]\nnotRun = { persistence = \"none\" }\n"),
        (
            "quoted",
            "[\"harness\".\"notRun\"]\npersistence = \"none\"\n",
        ),
    ] {
        let root = temporary_root(&format!("scaffold-retired-{label}"));
        retired_declaration_root(&root, declaration);

        let mut violations = Vec::new();
        verify_version_directories(&root, &mut violations);

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("unknown field `harness`")),
            "{label} raised no refusal; violations:\n{}",
            violations.join("\n")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}

#[test]
fn an_unparseable_adapter_is_a_violation_rather_than_a_pass() {
    let root = temporary_root("scaffold-retired-unparseable");
    retired_declaration_root(&root, "[harness\nthis is not toml\n");

    let mut violations = Vec::new();
    verify_version_directories(&root, &mut violations);

    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("adapter.toml")),
        "violations:\n{}",
        violations.join("\n")
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn an_adapter_without_the_retired_table_is_accepted() {
    for (label, declaration) in [
        ("absent", ""),
        (
            "elsewhere",
            "[stubs]\nnotRun = \"nothing to do with the harness\"\n",
        ),
    ] {
        let root = temporary_root(&format!("scaffold-retired-clean-{label}"));
        retired_declaration_root(&root, declaration);

        let mut violations = Vec::new();
        verify_version_directories(&root, &mut violations);

        if declaration.is_empty() {
            assert!(violations.is_empty(), "{label}: {violations:?}");
        } else {
            assert!(
                violations
                    .iter()
                    .any(|error| error.contains("unknown field `stubs`")),
                "{label}: {violations:?}"
            );
        }
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}

fn retired_declaration_root(root: &Path, declaration: &str) {
    version_directory(root, "26.1.2", &[]);
    let adapter = root.join(format!("{VERSION_DATA}/26.1.2/adapter.toml"));
    let mut text = fs::read_to_string(&adapter).expect("adapter inputs should be readable");
    text.push_str(declaration);
    fs::write(&adapter, &text).expect("adapter inputs should be writable");
}

fn version_specific_crates_fixture(root: &Path) {
    version_directory(root, "test-version", &[]);

    fs::write(
        root.join("minecraft/versions/test-version/hooks.toml"),
        "class = \"com.testgame.Thing\"\n\
         descriptor = \"(Lcom/testgame/Other;Ljava/lang/String;)V\"\n",
    )
    .expect("version data should be writable");
    fs::create_dir_all(root.join("crates/ravel-minecraft-test-version/src"))
        .expect("crate directory should be creatable");
    fs::write(
        root.join("crates/ravel-minecraft-test-version/src/lib.rs"),
        "const VERSION: &str = \"test-version\";\n",
    )
    .expect("test source should be writable");
    fs::create_dir_all(root.join("crates/ravel-core/src"))
        .expect("crate directory should be creatable");
    fs::write(
        root.join("crates/ravel-core/src/lib.rs"),
        "// com.testgame.Thing, com/testgame/Other, java/lang/String\n",
    )
    .expect("test source should be writable");
}
