use std::fs;
use std::path::{Path, PathBuf};

use crate::java_shape::REFLECTION_FILE;
use crate::test_support::{temporary_root, version_directory};
use crate::version_boundary::{
    SWEPT_SCOPE, VERSION_CLASS_EXCUSES, VERSION_DATA, VERSION_FILES, VERSION_PROSE_FILE,
    verify_version_boundary, verify_version_directories, version_class_names,
};

#[test]
fn ravel_test_versions_agree_with_the_generator() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let versions = root.join(VERSION_DATA);
    let mut checked = 0;
    for entry in fs::read_dir(&versions).expect("the version data should be readable") {
        let directory = entry
            .expect("a version directory should be readable")
            .path();
        if !directory.is_dir() {
            continue;
        }
        let adapter = fs::read_to_string(directory.join("adapter.toml"))
            .expect("every version declares adapter inputs");
        let translating = adapter.contains("mappingSource = \"official-proguard-mappings\"");

        let declares_mapping = adapter
            .lines()
            .any(|line| line.starts_with("path = \"") && line.ends_with("client.txt\""));
        assert_eq!(
            translating,
            declares_mapping,
            "{} reads a published mapping ({translating}) but declares a client.txt path \
             ({declares_mapping}); ravel-test decides what to skip from that path",
            directory.display()
        );
        checked += 1;
    }
    assert!(checked > 0, "the repository tracks a version");
}

#[test]
fn version_specific_crates_and_production_literals_are_rejected() {
    let root = temporary_root("versions");
    fs::create_dir_all(root.join("minecraft/versions/test-version"))
        .expect("version directory should be creatable");

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
            violation.contains("declares Minecraft 26.1.2 while its directory is named 1.21.1"),
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

    let full = "resource = 34\nresourceMinor = 0\ndata = 48\ndataMinor = 0\n\
                spelling = \"pack-format\"\n";
    let without = |key: &str| -> String {
        let mut kept = full
            .lines()
            .filter(|line| !line.starts_with(key))
            .collect::<Vec<_>>()
            .join("\n");
        kept.push('\n');
        kept
    };
    let mut cases: Vec<(String, String)> = vec![(
        "schemaVersion = 1\nminecraft = \"26.1.2\"\n".to_owned(),
        "declares no `[packFormat]`".to_owned(),
    )];
    for key in [
        "resource = ",
        "resourceMinor",
        "data = ",
        "dataMinor",
        "spelling",
    ] {
        let named = key.trim_end_matches(" = ");
        cases.push((
            format!(
                "schemaVersion = 1\nminecraft = \"26.1.2\"\n\n[packFormat]\n{}",
                without(key)
            ),
            format!("without {named}"),
        ));
    }
    for (written, expected) in &cases {
        fs::write(&adapter, written).expect("version data should be writable");
        let mut violations = Vec::new();

        verify_version_directories(&root, &mut violations);

        assert_eq!(violations.len(), 1, "{written}: {violations:?}");
        assert!(
            violations[0].contains(expected),
            "{written}: {violations:?}"
        );
    }

    fs::write(
        &adapter,
        format!("[packFormat]\n{full}\nschemaVersion = 1\nminecraft = \"26.1.2\"\n"),
    )
    .expect("version data should be writable");
    let mut violations = Vec::new();
    verify_version_directories(&root, &mut violations);
    assert!(
        violations.is_empty(),
        "a table on the first line: {violations:?}"
    );

    version_directory(&root, "26.1.2", &[]);
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
            "minecraft/versions/26.1.2/stubs.toml declares no `minecraft` version, so nothing \
             holds it to the version minecraft/versions/26.1.2 is named for"
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
fn a_reflected_class_name_is_version_data_the_boundary_sweep_reads() {
    let root = temporary_root("version-classes");
    version_directory(&root, "26.1.2", &[]);
    fs::write(
        root.join(format!("{VERSION_DATA}/26.1.2/{REFLECTION_FILE}")),
        "schemaVersion = 1\nminecraft = \"26.1.2\"\n\n[[reflected]]\nclass = \
         \"net.minecraft.resources.Identifier\"\n",
    )
    .expect("version data should be writable");

    let classes = version_class_names(&root);

    assert!(
        classes.contains(&"net/minecraft/resources/Identifier".to_owned()),
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
                .any(|violation| violation.contains("is retired")),
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

        assert_eq!(violations, Vec::<String>::new(), "{label}");
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
