use std::fs;
use std::path::PathBuf;

use ravel_minecraft_generate::{Eligibility, Kind};

use super::reflection::{declared_roles, game_class_literals, verify_persistence_reflection};
use super::screens::SCREEN_JAR_SOURCES;
use super::transformer::declared_hook;
use crate::hooks::hook_inventory;
use crate::scaffold::java_shape::{
    AGENT_JAR_SOURCES, REFLECTION_FILE, TRANSFORMER_SHAPE_RECORD, verify_hook_name_literals,
    verify_screen_templates, verify_transformer_shape,
};
use crate::test_support::{adapter_document, temporary_root, version_directory};
use crate::version_boundary::{VERSION_DATA, verify_version_directories};

#[test]
fn agent_source_selection_shares_the_build_manifest() -> Result<(), String> {
    use super::{AGENT_SOURCE_MANIFEST, agent_source_manifest, compiled_bootstrap_source};
    let sources = agent_source_manifest(AGENT_SOURCE_MANIFEST)?;
    for (group, name) in sources {
        let path = PathBuf::from(format!("jvm/bootstrap/dev/ravel/jvm/{name}.java"));
        assert!(compiled_bootstrap_source(&path, false), "{name}");
        assert_eq!(
            compiled_bootstrap_source(&path, true),
            group == "core",
            "{name}"
        );
        assert!(!compiled_bootstrap_source(
            &path.with_extension("java.in"),
            false
        ));
        let nested = PathBuf::from(format!("jvm/bootstrap/dev/ravel/jvm/nested/{name}.java"));
        assert!(!compiled_bootstrap_source(&nested, false));
        assert!(!compiled_bootstrap_source(&nested, true));
    }
    assert!(compiled_bootstrap_source(
        std::path::Path::new("jvm/bootstrap/dev/ravel/jvm/LegacyContent.java"),
        true
    ));
    for malformed in [
        "",
        "core Boot\n",
        "other Boot\nmodern Content\n",
        "core Boot extra\nmodern Content\n",
        "core Boot\nmodern Boot\n",
    ] {
        assert!(agent_source_manifest(malformed).is_err(), "{malformed}");
    }
    let build = include_str!("../../../../../jvm/build/build-agent.sh");
    assert!(build.contains("jvm/build/agent-sources.txt"));
    assert!(build.contains("-maxdepth 1 -name 'Legacy*.java'"));
    Ok(())
}

#[test]
fn live_legacy_dispatch_literals_are_held_to_the_legacy_inventory()
-> Result<(), Box<dyn std::error::Error>> {
    let root = reflection_root("legacy-dispatch", "");
    let directory = root.join(VERSION_DATA).join("26.1.2");
    fs::write(
        directory.join("adapter.toml"),
        adapter_document("26.1.2", "generation-only", None).replace("'generation-only'", "'live'"),
    )?;
    fs::write(
        directory.join("hooks.toml"),
        format!(
            "schemaVersion = 1\nminecraft = '26.1.2'\n{}",
            seam("legacy.client-tick", "instance-method", "()V")
        ),
    )?;
    let sources = root.join(AGENT_JAR_SOURCES).join("dev/ravel/jvm");
    let transformer = "boolean modern = target.hookMethod().equals(\"worldOpenPersistence\");\n\
        switch (target.hookMethod()) { case \"legacyClientTick\" -> run(); }\n";
    fs::write(sources.join("Transformer.java"), transformer)?;
    fs::write(
        sources.join("LegacyRuntime.java"),
        "switch (id) { case \"legacyClientTick\" -> run(); }\n",
    )?;
    fs::write(
        sources.join("LegacyPersistence.java"),
        "boolean installed = Agent.isInstalled(\"legacyClientTick\");\n",
    )?;
    let mut violations = Vec::new();
    let (adapters, inventories) = verify_version_directories(&root, &mut violations);
    verify_hook_name_literals(&root, &inventories, &adapters, &mut violations);
    assert!(violations.is_empty(), "{violations:?}");
    for filename in [
        "Transformer.java",
        "LegacyRuntime.java",
        "LegacyPersistence.java",
    ] {
        let path = sources.join(filename);
        let original = fs::read_to_string(&path)?;
        fs::write(&path, original.replace("legacyClientTick", "legacyMissing"))?;
        verify_hook_name_literals(&root, &inventories, &adapters, &mut violations);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains(filename), "{violations:?}");
        assert!(violations[0].contains("legacyMissing"), "{violations:?}");
        violations.clear();
        fs::write(path, original)?;
    }
    let refusal = "if (!id.equals(\"legacyClientTick\")) return false;\n";
    let runtime = sources.join("LegacyRuntime.java");
    fs::write(&runtime, refusal)?;
    verify_hook_name_literals(&root, &inventories, &adapters, &mut violations);
    assert!(violations.is_empty(), "{violations:?}");
    fs::write(
        &runtime,
        refusal.replace("legacyClientTick", "legacyMissing"),
    )?;
    verify_hook_name_literals(&root, &inventories, &adapters, &mut violations);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(violations[0].contains("legacyMissing"), "{violations:?}");
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn inventories_follow_toml_string_and_comment_rules() {
    let root = seam_root(
        "hook-toml-rules",
        "[[hook]] # reviewed\nid = 'screen.open' # hook\nkind = 'instance-method'\n\
         class = 'net.minecraft.Screen'\nmember = 'open'\ncapability = 'Open a screen.'\n\
         descriptor = '(I)V' # signature\n[[nonGoal]]\nid = 'other'\nreason = 'Not carried.'\n",
    );
    let hooks = hook_inventory(&root, "26.1.2").expect("valid hook TOML should parse");
    assert_eq!(
        declared_hook(&hooks, "screen.open"),
        Some((Kind::InstanceMethod, "(I)V"))
    );
    assert_eq!(hooks.non_goals, 1);
    fs::remove_dir_all(root).expect("temporary root should be removable");

    let roles = declared_roles(
        "[[reflected]] # role\nname = 'SCREEN'\nclass = 'net.minecraft.Screen'\n\
         runtime = 'net.minecraft.Screen'\n[[reflected.member]] # member\n\
         descriptor = '(I)V' # signature\n",
        "reflection.toml",
    )
    .expect("valid reflection TOML should parse");
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].name, "SCREEN");
    assert_eq!(roles[0].descriptors, ["(I)V"]);
}

#[test]
fn malformed_hooks_and_missing_ids_are_reported_before_consumers() {
    for (label, text, diagnostic) in [
        ("malformed", "[[hook]\n", "cannot parse hooks.toml"),
        (
            "missing-id",
            "[[hook]]\nkind = 'instance-method'\n",
            "missing field `id`",
        ),
    ] {
        let root = seam_root(label, text);
        fs::create_dir_all(root.join(AGENT_JAR_SOURCES))
            .expect("empty bootstrap source directory should be creatable");
        let mut violations = Vec::new();
        let (eligibility, inventories) = verify_version_directories(&root, &mut violations);
        assert_eq!(eligibility.len(), 1);
        assert_eq!(
            eligibility.get("26.1.2").map(|facts| facts.eligibility),
            Some(Eligibility::Live)
        );
        assert!(inventories.is_empty(), "{inventories:?}");
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains(diagnostic), "{violations:?}");
        verify_hook_name_literals(&root, &inventories, &eligibility, &mut violations);
        verify_transformer_shape(&root, &inventories, &eligibility, &mut violations);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(!root.join(TRANSFORMER_SHAPE_RECORD).exists());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}

#[test]
fn the_agent_jars_game_class_names_are_the_declared_and_the_pinned_ones() {
    let root = crate::test_support::repository_root();
    let mut violations = Vec::new();

    let (eligibility, _) = verify_version_directories(&root, &mut violations);
    verify_persistence_reflection(&root, &eligibility, &mut violations);

    assert_eq!(violations, Vec::<String>::new());
}

fn reflection_root(label: &str, source: &str) -> PathBuf {
    let root = temporary_root(label);
    version_directory(&root, "26.1.2", &[]);
    let sources = root.join(AGENT_JAR_SOURCES).join("dev/ravel/jvm");
    fs::create_dir_all(&sources).expect("agent sources should be creatable");
    fs::write(sources.join("Content.java"), source).expect("agent source should be writable");
    root
}

#[test]
fn generation_only_reflection_does_not_invent_modern_bootstrap_roles() {
    let root = reflection_root("generation-reflection", "String one = GameNames.ONE;\n");
    version_directory(&root, "1.12.2", &[]);
    fs::write(
        root.join(format!("{VERSION_DATA}/1.12.2/adapter.toml")),
        adapter_document("1.12.2", "generation-only", None),
    )
    .expect("adapter should be writable");
    let reflection = root.join(format!("{VERSION_DATA}/1.12.2/{REFLECTION_FILE}"));
    let valid = "minecraft = '1.12.2'\n[[reflected]]\nname = 'LEGACY_STORAGE'\nclass = 'legacy.Storage'\n\
                 runtime = 'abc'\n[[reflected.member]]\ndescriptor = '()V'\n";
    fs::write(&reflection, valid).expect("reflection should be writable");
    let mut violations = Vec::new();
    let (eligibility, _) = verify_version_directories(&root, &mut violations);
    verify_persistence_reflection(&root, &eligibility, &mut violations);
    assert!(violations.is_empty(), "{violations:?}");
    fs::write(&reflection, valid.replace("()V", "not-a-descriptor"))
        .expect("reflection should be writable");
    verify_persistence_reflection(&root, &eligibility, &mut violations);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("invalid descriptor"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn live_legacy_reflection_uses_the_compiled_adapter_closure()
-> Result<(), Box<dyn std::error::Error>> {
    let root = reflection_root(
        "live-legacy-reflection",
        "String one = GameNames.ONE;\nObject read = GameMembers.ONE_READ;\n",
    );
    let version = "0.0.0-test";
    version_directory(&root, version, &[]);
    let directory = root.join(VERSION_DATA).join(version);
    fs::write(
        directory.join("adapter.toml"),
        adapter_document(version, "generation-only", None).replace("'generation-only'", "'live'"),
    )?;
    let source = root.join(AGENT_JAR_SOURCES).join("dev/ravel/jvm");
    fs::write(
        source.join("Agent.java"),
        "String shared = GameNames.SHARED;\n",
    )?;
    let modern = root.join(VERSION_DATA).join("26.1.2").join(REFLECTION_FILE);
    add_shared_reflection_role(&modern)?;
    fs::write(source.join("Outward.java"), "String one = GameNames.ONE;\n")?;
    fs::write(
        source.join("LegacyContent.java"),
        "Class<?> role = LegacyNames.className(\"LEGACY_ONLY\");\n\
         Object read = LegacyAccess.call(\"LEGACY_ONLY\", \"read\", null);\n",
    )?;
    let reflection = directory.join(REFLECTION_FILE);
    let roles = "minecraft = '0.0.0-test'\n[[reflected]]\nname = 'LEGACY_ONLY'\nclass = 'legacy.Storage'\n\
                 runtime = 'abc'\n[[reflected.member]]\nname = 'read'\ndescriptor = '()V'\n\
                 [[reflected]]\nname = 'SHARED'\nclass = 'shared.Entry'\nruntime = 'shared.Entry'\n";
    fs::write(&reflection, roles)?;
    let mut violations = Vec::new();
    let (adapters, _) = verify_version_directories(&root, &mut violations);
    assert!(adapters[version].legacy_adapter);
    assert_eq!(adapters[version].eligibility, Eligibility::Live);
    verify_persistence_reflection(&root, &adapters, &mut violations);
    assert!(violations.is_empty(), "{violations:?}");

    fs::write(
        source.join("LegacyContent.java"),
        "Class<?> missing = LegacyNames.className(\"MISSING\");\n\
         Object read = LegacyAccess.call(\"LEGACY_ONLY\", \"missing\", null);\n",
    )?;
    verify_persistence_reflection(&root, &adapters, &mut violations);
    assert_eq!(violations.len(), 2, "{violations:?}");
    assert!(
        violations
            .iter()
            .all(|message| message.contains("LegacyContent.java"))
    );
    violations.clear();
    fs::write(source.join("LegacyContent.java"), "")?;
    fs::write(
        &reflection,
        roles.replace("name = 'SHARED'", "name = 'UNUSED_SHARED'"),
    )?;
    verify_persistence_reflection(&root, &adapters, &mut violations);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(violations[0].contains("Agent.java"), "{violations:?}");
    violations.clear();
    fs::write(&reflection, roles.replace("()V", "not-a-descriptor"))?;
    verify_persistence_reflection(&root, &adapters, &mut violations);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("invalid descriptor"),
        "{violations:?}"
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

fn add_shared_reflection_role(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(
        path,
        fs::read_to_string(path)?
            + "\n[[reflected]]\nname = 'SHARED'\n\
        class = 'shared.Entry'\nruntime = 'shared.Entry'\n",
    )?;
    Ok(())
}

#[test]
fn generation_only_reflection_validates_fields_as_single_non_void_types() {
    let root = reflection_root(
        "generation-reflection-fields",
        "String one = GameNames.ONE;\n",
    );
    version_directory(&root, "1.12.2", &[]);
    fs::write(
        root.join(format!("{VERSION_DATA}/1.12.2/adapter.toml")),
        adapter_document("1.12.2", "generation-only", None),
    )
    .expect("adapter should be writable");
    let reflection = root.join(format!("{VERSION_DATA}/1.12.2/{REFLECTION_FILE}"));
    let mut violations = Vec::new();
    let (eligibility, _) = verify_version_directories(&root, &mut violations);
    assert!(violations.is_empty(), "{violations:?}");
    for (descriptor, valid) in [
        ("Lnet/minecraft/util/Id2ObjectBiMap;", true),
        ("Z", true),
        ("B", true),
        ("C", true),
        ("S", true),
        ("I", true),
        ("J", true),
        ("F", true),
        ("D", true),
        ("[I", true),
        ("[[Lnet/minecraft/block/Block;", true),
        ("", false),
        ("V", false),
        ("II", false),
        ("Lnet/minecraft/util/Id2ObjectBiMap", false),
        ("L;", false),
        ("[", false),
        ("[[", false),
        ("[V", false),
        ("[Lnet/minecraft/block/Block", false),
        ("Ljava/lang/Object;I", false),
        ("I)V", false),
    ] {
        fs::write(
            &reflection,
            format!(
                "minecraft = '1.12.2'\n[[reflected]]\nname = 'LEGACY_BLOCK'\n\
                 class = 'net.minecraft.block.Block'\nruntime = 'aow'\n\
                 [[reflected.member]]\nname = 'STATE_REGISTRY'\ndescriptor = '{descriptor}'\n"
            ),
        )
        .expect("reflection should be writable");
        violations.clear();
        verify_persistence_reflection(&root, &eligibility, &mut violations);
        if valid {
            assert!(violations.is_empty(), "{descriptor}: {violations:?}");
        } else {
            assert_eq!(violations.len(), 1, "{descriptor}: {violations:?}");
            assert!(
                violations[0].contains("invalid descriptor"),
                "{descriptor}: {violations:?}"
            );
        }
    }
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_hand_written_game_literal_in_the_agent_sources_is_refused() {
    let root = reflection_root(
        "version-reflection-literal",
        "String identifier = \"net.minecraft.resources.Identifier\";\n    String one = \
         GameNames.ONE;\n",
    );
    let mut violations = Vec::new();

    let (eligibility, _) = verify_version_directories(&root, &mut violations);
    verify_persistence_reflection(&root, &eligibility, &mut violations);

    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0]
            .contains("spells the game class net.minecraft.resources.Identifier as a literal"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_role_a_version_does_not_declare_is_refused() {
    let root = reflection_root(
        "version-reflection-role",
        "String one = GameNames.ONE;\n    String other = GameNames.STORAGE;\n",
    );
    let mut violations = Vec::new();

    let (eligibility, _) = verify_version_directories(&root, &mut violations);
    verify_persistence_reflection(&root, &eligibility, &mut violations);

    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("resolves the game class GameNames.STORAGE"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_declared_role_no_source_resolves_is_refused() {
    let root = reflection_root("version-reflection-unused", "String nothing = null;\n");
    let mut violations = Vec::new();

    let (eligibility, _) = verify_version_directories(&root, &mut violations);
    verify_persistence_reflection(&root, &eligibility, &mut violations);

    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("declares the role ONE for net.minecraft.client.One"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_role_named_only_inside_an_unreached_members_descriptor_is_refused() {
    let root = reflection_root(
        "version-reflection-descriptor",
        "String one = GameNames.ONE;\n",
    );
    fs::write(
        root.join(format!("{VERSION_DATA}/26.1.2/{REFLECTION_FILE}")),
        "schemaVersion = 1\nminecraft = \"26.1.2\"\n\n             [[reflected]]\nname = \"ONE\"\nclass = \"net.minecraft.client.One\"\nruntime =              \"net.minecraft.client.One\"\n\n             [[reflected.member]]\nname = \"takes\"\ndescriptor =              \"(Lnet/minecraft/client/Two;)V\"\n\n             [[reflected]]\nname = \"TWO\"\nclass = \"net.minecraft.client.Two\"\nruntime =              \"net.minecraft.client.Two\"\n\n             [[reflected.member]]\nname = \"passes\"\ndescriptor =              \"(Lnet/minecraft/client/Three;)V\"\n\n             [[reflected]]\nname = \"THREE\"\nclass =              \"net.minecraft.client.Three\"\nruntime = \"net.minecraft.client.Three\"\n",
    )
    .expect("version data should be writable");
    let mut violations = Vec::new();

    let (eligibility, _) = verify_version_directories(&root, &mut violations);
    verify_persistence_reflection(&root, &eligibility, &mut violations);

    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("declares the role THREE for net.minecraft.client.Three"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_reflected_table_missing_a_name_is_refused_rather_than_skipped() {
    let root = reflection_root(
        "version-reflection-partial",
        "String one = GameNames.ONE;\n",
    );
    fs::write(
        root.join(format!("{VERSION_DATA}/26.1.2/{REFLECTION_FILE}")),
        "schemaVersion = 1\nminecraft = \"26.1.2\"\n\n[[reflected]]\nname = \"ONE\"\nclass = \
         \"net.minecraft.client.One\"\n",
    )
    .expect("version data should be writable");
    let mut violations = Vec::new();

    let (eligibility, _) = verify_version_directories(&root, &mut violations);
    verify_persistence_reflection(&root, &eligibility, &mut violations);

    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("cannot parse")
                && violation.contains("missing field `runtime`")),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_version_without_a_reflection_file_disarms_no_check_silently() {
    let root = temporary_root("version-reflection-missing");
    version_directory(&root, "26.1.2", &[REFLECTION_FILE]);
    let mut violations = Vec::new();

    let (eligibility, _) = verify_version_directories(&root, &mut violations);
    verify_persistence_reflection(&root, &eligibility, &mut violations);

    assert!(
        violations
            .iter()
            .any(|violation| violation
                .contains("cannot read minecraft/versions/26.1.2/reflection.toml")),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_hook_name_no_version_declares_is_refused() {
    let root = reflection_root(
        "version-hook-name",
        "boolean seam = target.hookMethod().equals(\"worldSave\");\n    String one = \
         GameNames.ONE;\n",
    );
    let mut violations = Vec::new();

    let (eligibility, inventories) = verify_version_directories(&root, &mut violations);
    verify_hook_name_literals(&root, &inventories, &eligibility, &mut violations);

    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("matches the generated hook worldSave"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_hook_name_every_version_declares_is_accepted() {
    let root = reflection_root(
        "version-hook-name-declared",
        "boolean seam = target.hookMethod().equals(\"levelTick\");\n    String one = \
         GameNames.ONE;\n",
    );
    let mut violations = Vec::new();

    let (eligibility, inventories) = verify_version_directories(&root, &mut violations);
    verify_hook_name_literals(&root, &inventories, &eligibility, &mut violations);

    assert_eq!(violations, Vec::<String>::new());
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn live_legacy_capability_hooks_are_not_modern_transformer_seams() {
    let root = reflection_root("legacy-not-bootstrap", "");
    fs::write(
        root.join(AGENT_JAR_SOURCES)
            .join("dev/ravel/jvm/Transformer.java"),
        "boolean seam = target.hookMethod().equals(\"worldOpenPersistence\");\n",
    )
    .unwrap_or_else(|error| panic!("shared transformer should be writable: {error}"));
    fs::write(
        root.join(format!("{VERSION_DATA}/26.1.2/hooks.toml")),
        format!(
            "schemaVersion = 1\nminecraft = '26.1.2'\n{}",
            declared_seams()
        ),
    )
    .expect("modern hooks should be writable");
    version_directory(&root, "1.12.2", &[]);
    fs::write(
        root.join(format!("{VERSION_DATA}/1.12.2/adapter.toml")),
        adapter_document("1.12.2", "generation-only", None).replace("'generation-only'", "'live'"),
    )
    .expect("legacy adapter should be writable");
    let hooks = root.join(format!("{VERSION_DATA}/1.12.2/hooks.toml"));
    fs::write(
        &hooks,
        format!(
            "schemaVersion = 1\nminecraft = '1.12.2'\n{}",
            seam("world.open-persistence", "instance-method", "()I")
        ),
    )
    .expect("legacy hooks should be writable");
    let mut violations = Vec::new();
    let (eligibility, inventories) = verify_version_directories(&root, &mut violations);
    verify_hook_name_literals(&root, &inventories, &eligibility, &mut violations);
    verify_transformer_shape(&root, &inventories, &eligibility, &mut violations);
    assert!(violations.is_empty(), "{violations:?}");
    let held = fs::read_to_string(root.join("target/ravel-jvm/transformer-shape.txt"))
        .expect("transformer shape record should exist");
    assert!(held.contains("Minecraft 26.1.2"));
    assert!(!held.contains("Minecraft 1.12.2"));

    fs::write(&hooks, "[[hook]\n").expect("legacy hooks should be writable");
    let (eligibility, inventories) = verify_version_directories(&root, &mut violations);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("cannot parse hooks.toml"),
        "{violations:?}"
    );
    assert_eq!(eligibility.len(), 2);
    assert_eq!(
        eligibility.get("26.1.2").map(|facts| facts.eligibility),
        Some(Eligibility::Live)
    );
    assert_eq!(
        eligibility.get("1.12.2").map(|facts| facts.eligibility),
        Some(Eligibility::Live)
    );
    assert_eq!(inventories.len(), 1);
    assert_eq!(inventories[0].version, "26.1.2");
    verify_hook_name_literals(&root, &inventories, &eligibility, &mut violations);
    verify_transformer_shape(&root, &inventories, &eligibility, &mut violations);
    assert_eq!(violations.len(), 1, "{violations:?}");
    let retained = fs::read_to_string(root.join(TRANSFORMER_SHAPE_RECORD))
        .expect("valid modern transformer record should remain");
    assert_eq!(retained, held);
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn shape_checks_share_validated_adapter_facts_without_rereading_adapters() {
    let root = reflection_root("shared-eligibility", "String one = GameNames.ONE;\n");
    fs::write(
        root.join(format!("{VERSION_DATA}/26.1.2/hooks.toml")),
        format!(
            "schemaVersion = 1\nminecraft = '26.1.2'\n{}",
            declared_seams()
        ),
    )
    .expect("hooks should be writable");
    let mut violations = Vec::new();
    let (eligibility, inventories) = verify_version_directories(&root, &mut violations);
    fs::write(
        root.join(format!("{VERSION_DATA}/26.1.2/adapter.toml")),
        "not valid TOML",
    )
    .expect("adapter should be writable");

    verify_persistence_reflection(&root, &eligibility, &mut violations);
    verify_hook_name_literals(&root, &inventories, &eligibility, &mut violations);
    verify_transformer_shape(&root, &inventories, &eligibility, &mut violations);

    assert!(violations.is_empty(), "{violations:?}");
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn invalid_adapters_are_reported_once_before_all_shape_checks() {
    for (label, adapter, diagnostic) in [
        ("malformed-adapter", "not valid TOML".to_owned(), "TOML"),
        (
            "invalid-eligibility",
            adapter_document("1.12.2", "generation-only", None)
                .replace("'generation-only'", "'unsupported'"),
            "unsupported",
        ),
        (
            "wrong-adapter-version",
            adapter_document("26.1.2", "live", None),
            "1.12.2",
        ),
    ] {
        let root = reflection_root(label, "String one = GameNames.ONE;\n");
        fs::write(
            root.join(format!("{VERSION_DATA}/26.1.2/hooks.toml")),
            format!(
                "schemaVersion = 1\nminecraft = '26.1.2'\n{}",
                declared_seams()
            ),
        )
        .expect("hooks should be writable");
        version_directory(&root, "1.12.2", &[]);
        fs::write(
            root.join(format!("{VERSION_DATA}/1.12.2/adapter.toml")),
            adapter,
        )
        .expect("adapter should be writable");
        let mut violations = Vec::new();
        let (eligibility, inventories) = verify_version_directories(&root, &mut violations);

        verify_persistence_reflection(&root, &eligibility, &mut violations);
        verify_hook_name_literals(&root, &inventories, &eligibility, &mut violations);
        verify_transformer_shape(&root, &inventories, &eligibility, &mut violations);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains(diagnostic), "{violations:?}");
        assert!(!eligibility.contains_key("1.12.2"));
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}

fn seam_root(label: &str, seams: &str) -> PathBuf {
    let root = temporary_root(label);
    version_directory(&root, "26.1.2", &[]);
    fs::write(
        root.join(format!("{VERSION_DATA}/26.1.2/hooks.toml")),
        format!("schemaVersion = 1\nminecraft = \"26.1.2\"\n{seams}"),
    )
    .expect("version data should be writable");
    root
}

fn seam(id: &str, kind: &str, descriptor: &str) -> String {
    let title_menu_row = if id == "screen.title-menu-options" && descriptor.ends_with(")V") {
        "titleMenuRow = 1\n"
    } else {
        ""
    };
    format!(
        "\n[[hook]]\nid = \"{id}\"\nclass = \"net.minecraft.client.One\"\nmember = \
         \"one\"\nkind = \"{kind}\"\ndescriptor = \"{descriptor}\"\n\
         capability = \"Carry the reviewed seam.\"\n{title_menu_row}"
    )
}

fn declared_seams() -> String {
    seam(
        "world.open-persistence",
        "instance-method",
        "(Lnet/minecraft/One;Lnet/minecraft/Two;Lnet/minecraft/Three;Z)V",
    ) + &seam("world.save-persistence", "instance-method", "(ZZZ)Z")
        + &seam("screen.title-menu-options", "instance-method", "(II)V")
}

#[test]
fn a_seam_whose_loaded_parameter_moved_is_refused() {
    let root = seam_root(
        "transformer-shape-parameter",
        &(seam(
            "world.open-persistence",
            "instance-method",
            "(Lnet/minecraft/One;Lnet/minecraft/Two;Lnet/minecraft/Three;)V",
        ) + &seam("world.save-persistence", "instance-method", "(ZZZ)Z")
            + &seam("screen.title-menu-options", "instance-method", "(II)V")),
    );
    let mut violations = Vec::new();

    let (eligibility, inventories) = verify_version_directories(&root, &mut violations);
    verify_transformer_shape(&root, &inventories, &eligibility, &mut violations);

    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("the transformer loads a boolean there"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_seam_returning_what_no_sequence_is_written_for_is_refused() {
    let root = seam_root(
        "transformer-shape-return",
        &(seam(
            "world.open-persistence",
            "instance-method",
            "(Lnet/minecraft/One;Lnet/minecraft/Two;Lnet/minecraft/Three;Z)V",
        ) + &seam("world.save-persistence", "instance-method", "(ZZZ)Z")
            + &seam(
                "screen.title-menu-options",
                "instance-method",
                "(II)Ljava/lang/String;",
            )),
    );
    let mut violations = Vec::new();

    let (eligibility, inventories) = verify_version_directories(&root, &mut violations);
    verify_transformer_shape(&root, &inventories, &eligibility, &mut violations);

    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("injects at its return for an int or nothing"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_seam_declared_static_where_the_receiver_is_loaded_is_refused() {
    let root = seam_root(
        "transformer-shape-static",
        &(seam(
            "world.open-persistence",
            "instance-method",
            "(Lnet/minecraft/One;Lnet/minecraft/Two;Lnet/minecraft/Three;Z)V",
        ) + &seam("world.save-persistence", "static-method", "(ZZZ)Z")
            + &seam("screen.title-menu-options", "instance-method", "(II)V")),
    );
    let mut violations = Vec::new();

    let (eligibility, inventories) = verify_version_directories(&root, &mut violations);
    verify_transformer_shape(&root, &inventories, &eligibility, &mut violations);

    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("the transformer loads its receiver"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn the_shapes_the_transformer_injects_against_are_recorded_by_version() {
    let root = seam_root("transformer-shape-held", &declared_seams());
    let mut violations = Vec::new();

    let (eligibility, inventories) = verify_version_directories(&root, &mut violations);
    verify_transformer_shape(&root, &inventories, &eligibility, &mut violations);

    assert_eq!(violations, Vec::<String>::new());
    let record = fs::read_to_string(root.join(TRANSFORMER_SHAPE_RECORD))
        .expect("the record should be written");
    assert_eq!(
        record,
        "held: Minecraft 26.1.2 against 3 transformer seams\n"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_run_no_version_names_a_seam_in_is_refused() {
    let root = seam_root(
        "transformer-shape-vacuous",
        &seam("level.tick", "instance-method", "()V"),
    );
    let mut violations = Vec::new();

    let (eligibility, inventories) = verify_version_directories(&root, &mut violations);
    verify_transformer_shape(&root, &inventories, &eligibility, &mut violations);

    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("checked against nothing"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

fn screen_root(label: &str, template: &str, mapping_source: &str) -> PathBuf {
    let root = temporary_root(label);
    version_directory(&root, "26.1.2", &[]);
    fs::write(
        root.join(format!("{VERSION_DATA}/26.1.2/adapter.toml")),
        adapter_document("26.1.2", "live", None)
            .replace("unobfuscated-class-files", mapping_source),
    )
    .expect("version data should be writable");
    let templates = root.join(SCREEN_JAR_SOURCES);
    fs::create_dir_all(&templates).expect("screen templates should be creatable");
    fs::write(templates.join("Content.java.in"), template)
        .expect("screen template should be writable");
    root
}

#[test]
fn a_template_that_spells_a_game_name_is_refused() {
    let root = screen_root(
        "screen-spelled",
        "import net.minecraft.client.One;\n",
        "unobfuscated-class-files",
    );
    let mut violations = Vec::new();

    verify_screen_templates(&root, &mut violations);

    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("spells the game name net.minecraft.client.One"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_template_that_names_a_game_class_in_a_placeholder_is_accepted() {
    let root = screen_root(
        "screen-placeholder",
        "{{import net.minecraft.client.One}}\nOne one = ({{net.minecraft.client.One}}) held;\n",
        "official-proguard-mappings",
    );
    let mut violations = Vec::new();

    verify_screen_templates(&root, &mut violations);

    assert_eq!(violations, Vec::<String>::new());
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_screen_template_directory_with_nothing_in_it_is_refused() {
    let root = temporary_root("screen-empty");
    fs::create_dir_all(root.join(SCREEN_JAR_SOURCES)).expect("creatable");
    let mut violations = Vec::new();

    verify_screen_templates(&root, &mut violations);

    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("holds no screen template"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}

#[test]
fn a_game_class_name_counts_from_a_literal_and_not_from_prose() {
    let source = "\
        /**\n\
         * Resolves \"net.minecraft.world.level.storage.LevelResource\" in prose,\n\
         * and mentions net.minecraft.client.Minecraft without quoting it.\n\
         */\n\
        private static final String STORAGE = \"net.minecraft.world.level.storage.SavedDataStorage\";\n\
        // A commented-out \"net.minecraft.util.datafix.DataFixers\".\n\
        return target.internalClass().equals(\"net/minecraft/client/Minecraft\");\n\
        String other = \"dev.ravel.jvm.screen.RavelWorldData\";\n";

    assert_eq!(
        game_class_literals(source),
        [
            "net.minecraft.world.level.storage.SavedDataStorage",
            "net.minecraft.client.Minecraft"
        ]
    );
}

#[test]
fn screen_overrides_preserve_version_discovery_errors() {
    let root = temporary_root("screen-version-error");
    let mut violations = Vec::new();
    super::verify_screen_overrides(&root, &mut violations);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].starts_with("cannot read minecraft/versions:"),
        "{violations:?}"
    );
    fs::remove_dir_all(root).expect("temporary root should be removable");
}
