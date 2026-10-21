use std::fs;
use std::path::Path;

use crate::scaffold::java_shape::REFLECTION_FILE;
use crate::test_support::{adapter_document, temporary_root, version_directory};
use crate::version_boundary::{
    SWEPT_SCOPE, TOML_VERSION_FILES, VERSION_CLASS_EXCUSES, VERSION_DATA, VERSION_PROSE_FILE,
    verify_version_boundary, verify_version_directories, version_class_names, version_directories,
};

#[test]
fn matrix_eligibility_preserves_cli_spellings() {
    for (eligibility, text) in [
        (super::MatrixEligibility::Live, "live"),
        (super::MatrixEligibility::GenerationOnly, "generation-only"),
        (
            super::MatrixEligibility::Unavailable {
                missing: "mapping.tiny".to_owned(),
            },
            "unavailable: mapping.tiny is absent",
        ),
    ] {
        assert_eq!(eligibility.to_string(), text);
    }
}

#[test]
fn matrix_eligibility_uses_the_declared_mapping_instead_of_its_extension() {
    let root = temporary_root("matrix-mappings");
    version_directory(&root, "1.12.2", &[]);
    let adapter = root.join(format!("{VERSION_DATA}/1.12.2/adapter.toml"));
    fs::write(
        &adapter,
        format!(
            "{}\n[mappingNamespaces]\nruntime = 'official'\nreviewed = 'named'\n\
             [inputs.mappingIoJar]\npath = 'mapping-io.jar'\nsha256 = '0000000000000000000000000000000000000000000000000000000000000000'\n",
            adapter_document("1.12.2", "generation-only", Some("mapping.tiny"))
                .replace("official-proguard-mappings", "feather-tiny-mappings")
        ),
    )
    .expect("adapter should be writable");
    assert_eq!(
        super::matrix_eligibility(&root, "1.12.2").expect("valid eligibility"),
        super::MatrixEligibility::Unavailable {
            missing: "mapping.tiny".to_owned()
        }
    );
    fs::write(root.join("mapping.tiny"), "pinned by generator")
        .expect("mapping should be writable");
    assert_eq!(
        super::matrix_eligibility(&root, "1.12.2").expect("valid eligibility"),
        super::MatrixEligibility::Unavailable {
            missing: "mapping-io.jar".to_owned()
        }
    );
    fs::write(root.join("mapping-io.jar"), "pinned by generator")
        .expect("normalizer should be writable");
    assert_eq!(
        super::matrix_eligibility(&root, "1.12.2").expect("valid eligibility"),
        super::MatrixEligibility::GenerationOnly
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
        assert!(super::matrix_eligibility(&root, "26.1.2").is_err());
    }
    fs::write(&adapter, valid.replace("eligibility = 'live'\n", ""))
        .expect("adapter should be writable");
    assert_eq!(
        super::matrix_eligibility(&root, "26.1.2").expect("default eligibility"),
        super::MatrixEligibility::Live
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
    assert!(super::matrix_eligibility(&root, "26.1.2").is_err());
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

    let (_, inventories) = verify_version_directories(&root, &mut violations);
    verify_version_boundary(&root, &inventories, &mut violations);

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
fn prose_and_adapter_files_have_distinct_validation() {
    let root = temporary_root("version-prose-adapter");
    let version = "26.1.2";
    version_directory(&root, version, &[VERSION_PROSE_FILE]);
    let directory = root.join(VERSION_DATA).join(version);
    let mut violations = Vec::new();
    let (adapters, inventories) = verify_version_directories(&root, &mut violations);
    assert_eq!(adapters.len(), 1);
    assert_eq!(inventories.len(), 1);
    assert_eq!(
        violations,
        [format!(
            "missing required file: {VERSION_DATA}/{version}/{VERSION_PROSE_FILE}"
        )]
    );
    fs::write(directory.join(VERSION_PROSE_FILE), "Not TOML: prose only.")
        .expect("prose should be writable");
    fs::remove_file(directory.join("adapter.toml")).expect("adapter should be removable");
    violations.clear();
    let (adapters, inventories) = verify_version_directories(&root, &mut violations);
    assert!(adapters.is_empty());
    assert!(inventories.is_empty());
    assert_eq!(
        violations,
        [format!(
            "missing required file: {VERSION_DATA}/{version}/adapter.toml"
        )]
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_version_whose_inventory_cannot_be_counted_is_reported() {
    let root = temporary_root("version-counts");
    version_directory(&root, "26.1.2", &[]);
    fs::write(
        root.join(format!("{VERSION_DATA}/26.1.2/hooks.toml")),
        "schemaVersion = 1\nminecraft = \"26.1.2\"\n\n[[hook]]\nid = \"an.id\"\n\
         class = 'net.minecraft.Thing'\nmember = 'member'\ndescriptor = '()V'\n\
         capability = 'Carry the reviewed hook.'\n",
    )
    .expect("version data should be writable");
    let mut violations = Vec::new();

    verify_version_directories(&root, &mut violations);

    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("cannot parse hooks.toml")
            && violations[0].contains("missing field `kind`"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_version_whose_inventory_declares_no_hook_is_reported() {
    let root = temporary_root("version-empty-inventory");
    version_directory(&root, "26.1.2", &[]);
    fs::write(
        root.join(format!("{VERSION_DATA}/26.1.2/hooks.toml")),
        "schemaVersion = 1\nminecraft = \"26.1.2\"\nhook = []\n",
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
    for file in TOML_VERSION_FILES.into_iter().chain(["adapter.toml"]) {
        let source = root.join(format!("{VERSION_DATA}/26.1.2/{file}"));
        let text = fs::read_to_string(source).expect("version data should be readable");
        fs::write(root.join(format!("{VERSION_DATA}/1.21.1/{file}")), text)
            .expect("version data should be writable");
    }
    let mut violations = Vec::new();

    verify_version_directories(&root, &mut violations);

    assert_eq!(
        violations.len(),
        TOML_VERSION_FILES.len() + 1,
        "{violations:?}"
    );
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
    let mut violations = Vec::new();
    let (_, inventories) = verify_version_directories(&root, &mut violations);
    fs::write(directory.join("stubs.toml"), "[stub\n").expect("malformed stubs should be writable");
    version_class_names(&root, &[directory], &inventories, &mut violations);
    assert_eq!(violations.len(), 2, "{violations:?}");
    assert!(
        violations[0].contains("cannot access")
            && violations[0].contains("minecraft/versions/26.1.2/hooks.toml"),
        "{violations:?}"
    );
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
    let (_, inventories) = verify_version_directories(&root, &mut violations);
    let classes = version_class_names(&root, &directories, &inventories, &mut violations);
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
        "schemaVersion = 1\nminecraft = 'test-version'\n[[hook]]\nid = 'an.id'\n\
         kind = 'instance-method'\nmember = 'member'\ncapability = 'Carry the reviewed hook.'\n\
         class = \"com.testgame.Thing\"\n\
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

#[test]
fn version_readmes_follow_the_adapter_inventory_and_its_reviewed_counts() {
    let root = temporary_root("version-readme-counts");
    version_directory(&root, "26.1.2", &[]);
    let directory = root.join(VERSION_DATA).join("26.1.2");
    let hooks =
        fs::read_to_string(directory.join("hooks.toml")).expect("fixture hooks should be readable");
    fs::write(
        directory.join("reviewed-hooks.toml"),
        format!("{hooks}\n[[nonGoal]]\nid = 'other'\nreason = 'Not carried.'\n"),
    )
    .expect("reviewed hooks should be writable");
    fs::write(
        directory.join("adapter.toml"),
        adapter_document("26.1.2", "live", None).replace("/hooks.toml", "/reviewed-hooks.toml"),
    )
    .expect("adapter should be writable");
    let readme = directory.join(VERSION_PROSE_FILE);
    fs::write(
        &readme,
        "carries 1 reviewed hooks and records 1 non-goals\n",
    )
    .expect("README should be writable");
    fs::remove_file(directory.join("hooks.toml")).expect("stray hooks should be removable");
    let mut violations = Vec::new();
    let (_, inventories) = verify_version_directories(&root, &mut violations);
    assert!(violations.is_empty(), "{violations:?}");
    assert_eq!(inventories.len(), 1);
    super::verify_version_readmes(&root, &inventories, &mut violations);
    assert!(violations.is_empty(), "{violations:?}");

    for stale in [
        "carries 2 reviewed hooks and records 1 non-goals\n",
        "carries 1 reviewed hooks and records 0 non-goals\n",
    ] {
        fs::write(&readme, stale).expect("stale README should be writable");
        super::verify_version_readmes(&root, &inventories, &mut violations);
        assert_eq!(
            violations,
            [
                "minecraft/versions/26.1.2/README.md does not state \"carries 1 reviewed hooks \
              and records 1 non-goals\", which is what \
              minecraft/versions/26.1.2/reviewed-hooks.toml declares"
            ]
        );
        violations.clear();
    }
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn version_admission_reports_unreadable_version_directories() {
    let root = temporary_root("version-readme-missing-directory");
    let mut violations = Vec::new();
    let (eligibility, inventories) = verify_version_directories(&root, &mut violations);
    assert!(eligibility.is_empty());
    assert!(inventories.is_empty());
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].starts_with("cannot read minecraft/versions:"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn boundary_classes_follow_the_admitted_inventory_not_stray_default_hooks() {
    let root = temporary_root("version-boundary-inventory-path");
    version_directory(&root, "26.1.2", &[]);
    let directory = root.join(VERSION_DATA).join("26.1.2");
    let hooks = fs::read_to_string(directory.join("hooks.toml"))
        .expect("fixture inventory should be readable");
    fs::write(
        directory.join("reviewed-hooks.toml"),
        hooks.replace("net.minecraft.client.One", "net.minecraft.ReviewedOnly"),
    )
    .expect("selected inventory should be writable");
    fs::write(
        directory.join("adapter.toml"),
        adapter_document("26.1.2", "live", None).replace("/hooks.toml", "/reviewed-hooks.toml"),
    )
    .expect("adapter should be writable");
    fs::remove_file(directory.join("hooks.toml")).expect("default inventory should be removable");
    let source = root.join("crates/ravel-core/src");
    fs::create_dir_all(&source).expect("production source directory should be creatable");
    fs::write(
        source.join("lib.rs"),
        "const REVIEWED: &str = \"net.minecraft.ReviewedOnly\";\n\
         const STRAY: &str = \"net.minecraft.StrayOnly\";\n",
    )
    .expect("production source should be writable");
    let mut violations = Vec::new();
    let (_, inventories) = verify_version_directories(&root, &mut violations);
    assert!(violations.is_empty(), "{violations:?}");
    for stray in [false, true] {
        if stray {
            fs::write(
                directory.join("hooks.toml"),
                hooks.replace("net.minecraft.client.One", "net.minecraft.StrayOnly"),
            )
            .expect("stray default inventory should be writable");
        }
        verify_version_boundary(&root, &inventories, &mut violations);
        assert_eq!(
            violations
                .iter()
                .filter(|message| message
                    .contains("crates/ravel-core/src/lib.rs names net/minecraft/ReviewedOnly"))
                .count(),
            1,
            "{violations:?}"
        );
        assert!(
            !violations
                .iter()
                .any(|message| message.contains("names net/minecraft/StrayOnly")),
            "{violations:?}"
        );
        assert!(
            !violations
                .iter()
                .any(|message| message.contains("cannot access") && message.contains("hooks.toml")),
            "{violations:?}"
        );
        violations.clear();
    }
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn public_fixture_inventory_paths_supply_the_classes_the_boundary_sweeps() {
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let relative = "tests/fixtures/public/versions/99.0.0-synthetic";
    let fixture = checkout.join(relative);
    let mut violations = Vec::new();
    let classes = version_class_names(
        &checkout,
        std::slice::from_ref(&fixture),
        &[],
        &mut violations,
    );
    assert!(violations.is_empty(), "{violations:?}");
    assert!(
        classes.contains(&"org/exampleengine/text/TextRules".to_owned()),
        "{classes:?}"
    );

    let root = temporary_root("version-boundary-public-fixture");
    let relocated = root.join(relative);
    hook_only_public_fixture(&root, &fixture, &relocated);
    assert!(
        !root
            .join("minecraft/versions/99.0.0-synthetic/hooks.toml")
            .exists()
    );
    verify_version_boundary(&root, &[], &mut violations);
    assert_eq!(
        violations
            .iter()
            .filter(|message| message
                .contains("crates/ravel-core/src/lib.rs names org/exampleengine/HookOnlyAdmission"))
            .count(),
        1,
        "{violations:?}"
    );
    assert!(
        !violations
            .iter()
            .any(|message| message.contains("cannot access") || message.contains("cannot parse")),
        "{violations:?}"
    );
    violations.clear();
    let broken_version = "99.1.0-broken";
    version_directory(&root, broken_version, &[]);
    let canonical = root.join(VERSION_DATA).join(broken_version);
    fs::write(canonical.join("hooks.toml"), "[[hook]\n")
        .expect("malformed sibling inventory should be writable");
    let broken = root
        .join("tests/fixtures/public/versions")
        .join(broken_version);
    fs::rename(canonical, &broken).expect("broken fixture should be relocatable");
    let classes = version_class_names(&root, &[broken, relocated], &[], &mut violations);
    assert!(
        classes.contains(&"org/exampleengine/HookOnlyAdmission".to_owned()),
        "{classes:?}"
    );
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("cannot parse hooks.toml"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn hook_only_public_fixture(root: &Path, fixture: &Path, relocated: &Path) {
    fs::create_dir_all(relocated).expect("public fixture directory should be creatable");
    for file in [
        "adapter.toml",
        "hooks.toml",
        "stubs.toml",
        "mappings.lock",
        "reflection.toml",
    ] {
        fs::copy(fixture.join(file), relocated.join(file))
            .expect("real fixture inputs should be copied");
    }
    let hooks = fs::read_to_string(relocated.join("hooks.toml"))
        .expect("copied hook inventory should be readable");
    fs::write(
        relocated.join("hooks.toml"),
        hooks.replace(
            "org.exampleengine.text.TextRules",
            "org.exampleengine.HookOnlyAdmission",
        ),
    )
    .expect("inventory-only class should be writable");
    let source = root.join("crates/ravel-core/src");
    fs::create_dir_all(&source).expect("production source directory should be creatable");
    fs::write(
        source.join("lib.rs"),
        "const FIXTURE_CLASS: &str = \"org.exampleengine.HookOnlyAdmission\";\n",
    )
    .expect("production source should be writable");
}

#[test]
fn version_references_use_complete_descriptor_tokens() {
    let value = toml::Value::Array(vec![
        toml::Value::String("([[Lgame/Level;[I)Lgame/Screen;".to_owned()),
        toml::Value::String("[Lgame/Widget;".to_owned()),
        toml::Value::String("prose mentioning Lgame/NotAType;".to_owned()),
        toml::Value::String("Lgame/Incomplete".to_owned()),
    ]);
    let mut classes = std::collections::BTreeSet::new();
    let mut references = Vec::new();
    super::collect_version_classes(&value, &mut classes, &mut references);
    assert_eq!(references, ["game/Level", "game/Screen", "game/Widget"]);
}

#[test]
fn unreadable_excused_source_does_not_make_its_excuse_stale() {
    let root = temporary_root("version-excuse-unreadable");
    version_directory(&root, "26.1.2", &[]);
    let file = VERSION_CLASS_EXCUSES[0].0;
    let path = root.join(file);
    fs::create_dir_all(path.parent().expect("source has a parent"))
        .expect("source directory should exist");
    fs::write(&path, [0xff]).expect("invalid UTF-8 should write");
    let mut violations = Vec::new();
    verify_version_boundary(&root, &[], &mut violations);
    assert_eq!(
        violations
            .iter()
            .filter(|message| message.starts_with(&format!("cannot read {file}:")))
            .count(),
        1,
        "{violations:?}"
    );
    assert!(
        !violations.iter().any(|message| {
            message.starts_with("stale version-class excuse:") && message.contains(file)
        }),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should remove");
}

#[test]
fn incomplete_crate_walk_does_not_claim_class_excuses_are_stale() {
    let root = temporary_root("version-excuse-walk-failure");
    version_directory(&root, "26.1.2", &[]);
    fs::write(root.join("crates"), "not a directory").expect("walk failure should be writable");
    let mut violations = Vec::new();
    verify_version_boundary(&root, &[], &mut violations);
    assert!(
        violations
            .iter()
            .any(|message| message.starts_with("cannot read crates:")),
        "{violations:?}"
    );
    assert!(
        !violations
            .iter()
            .any(|message| message.starts_with("stale version-class excuse:")),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should remove");
}

#[test]
fn incomplete_version_metadata_keeps_findings_without_stale_excuses() {
    for (file, bytes) in [
        ("hooks.toml", b"[[hook]\n".as_slice()),
        ("stubs.toml", b"broken = [\n".as_slice()),
        ("mappings.lock", b"\xff".as_slice()),
        ("reflection.toml", b"broken = [\n".as_slice()),
    ] {
        let root = temporary_root(&format!("version-boundary-incomplete-{file}"));
        version_directory(&root, "26.1.2", &[]);
        fs::write(root.join(VERSION_DATA).join("26.1.2").join(file), bytes)
            .expect("broken version metadata should be writable");
        let source = root.join("crates/ravel-core/src");
        fs::create_dir_all(&source).expect("source directory should be creatable");
        fs::write(source.join("lib.rs"), "26.1.2 net.minecraft.client.One")
            .expect("source should be writable");
        let mut violations = Vec::new();
        let (_, inventories) = verify_version_directories(&root, &mut violations);
        verify_version_boundary(&root, &inventories, &mut violations);
        assert!(
            violations.iter().any(|message| message.contains(file)),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|message| message.starts_with("hardcoded Minecraft version")),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|message| message.contains("names net/minecraft/client/One")),
            "{violations:?}"
        );
        assert!(
            !violations
                .iter()
                .any(|message| message.starts_with("stale version-class excuse:")),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should remove");
    }
}

#[test]
fn failed_public_hook_admission_does_not_finalize_class_excuses() {
    let root = temporary_root("version-boundary-incomplete-public-hooks");
    version_directory(&root, "99.1.0", &[]);
    let canonical = root.join(VERSION_DATA).join("99.1.0");
    fs::write(canonical.join("hooks.toml"), "[[hook]\n")
        .expect("broken hook inventory should be writable");
    let public = root.join("tests/fixtures/public/versions");
    fs::create_dir_all(&public).expect("public versions directory should be creatable");
    fs::rename(canonical, public.join("99.1.0")).expect("fixture should be relocatable");
    let source = root.join("crates/ravel-core/src");
    fs::create_dir_all(&source).expect("source directory should be creatable");
    fs::write(source.join("lib.rs"), "net.minecraft.client.One")
        .expect("source should be writable");
    let mut violations = Vec::new();
    verify_version_boundary(&root, &[], &mut violations);
    assert!(
        violations
            .iter()
            .any(|message| message.contains("cannot parse hooks.toml"))
    );
    assert!(
        violations
            .iter()
            .any(|message| message.contains("names net/minecraft/client/One")),
        "{violations:?}"
    );
    assert!(
        !violations
            .iter()
            .any(|message| message.starts_with("stale version-class excuse:")),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should remove");
}

#[test]
fn incomplete_walk_still_sweeps_the_files_it_collected() {
    let root = temporary_root("version-boundary-partial-walk");
    let source = root.join("crates/ravel-core/src");
    fs::create_dir_all(&source).expect("source directory should be creatable");
    let path = source.join("lib.rs");
    fs::write(&path, "26.1.2 net.minecraft.client.One").expect("source should be writable");
    let versions = std::collections::BTreeSet::from(["26.1.2".to_owned()]);
    let classes = vec!["net/minecraft/client/One".to_owned()];
    let mut violations = Vec::new();
    super::verify_boundary_files(&root, &[path], &versions, &classes, false, &mut violations);
    assert_eq!(violations.len(), 2, "{violations:?}");
    assert!(
        violations[0].starts_with("hardcoded Minecraft version"),
        "{violations:?}"
    );
    assert!(
        violations[1].contains("names net/minecraft/client/One"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should remove");
}
