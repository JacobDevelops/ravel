use std::fs;
use std::path::PathBuf;

use crate::java_shape::{
    AGENT_JAR_SOURCES, REFLECTION_FILE, SCREEN_JAR_SOURCES, TRANSFORMER_SHAPE_RECORD,
    declared_hook, declared_roles, game_class_literals, parse_hook_inventory,
    verify_hook_name_literals, verify_persistence_reflection, verify_screen_templates,
    verify_transformer_shape,
};
use crate::test_support::{temporary_root, version_directory};
use crate::version_boundary::{VERSION_DATA, hook_inventories};

#[test]
fn inventories_follow_toml_string_and_comment_rules() {
    let hooks = parse_hook_inventory(
        "[[hook]] # reviewed\nid = 'screen.open' # hook\nkind = 'instance-method'\n\
         descriptor = '(I)V' # signature\n[[nonGoal]]\nid = 'other'\n",
    )
    .expect("valid hook TOML should parse");
    assert_eq!(
        declared_hook(&hooks, "screen.open"),
        Some(("instance-method", "(I)V"))
    );
    assert_eq!(hooks.non_goals.len(), 1);

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
fn malformed_hooks_and_missing_ids_are_reported_by_both_consumers() {
    for (label, text, diagnostic) in [
        ("malformed", "[[hook]\n", "cannot parse hook inventory"),
        (
            "missing-id",
            "[[hook]]\nkind = 'instance-method'\n",
            "missing field `id`",
        ),
    ] {
        let root = seam_root(label, text);
        let inventories = hook_inventories(&root);
        let mut violations = Vec::new();
        verify_hook_name_literals(&root, &inventories, &mut violations);
        assert!(
            violations
                .iter()
                .any(|message| message.contains(diagnostic)),
            "{violations:?}"
        );
        violations.clear();
        verify_transformer_shape(&root, &inventories, &mut violations);
        assert!(
            violations
                .iter()
                .any(|message| message.contains(diagnostic)),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}

#[test]
fn the_agent_jars_game_class_names_are_the_declared_and_the_pinned_ones() {
    let root = std::env::current_dir()
        .expect("the current directory must be available")
        .join("../..");
    let mut violations = Vec::new();

    verify_persistence_reflection(&root, &mut violations);

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
fn a_hand_written_game_literal_in_the_agent_sources_is_refused() {
    let root = reflection_root(
        "version-reflection-literal",
        "String identifier = \"net.minecraft.resources.Identifier\";\n    String one = \
         GameNames.ONE;\n",
    );
    let mut violations = Vec::new();

    verify_persistence_reflection(&root, &mut violations);

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

    verify_persistence_reflection(&root, &mut violations);

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

    verify_persistence_reflection(&root, &mut violations);

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

    verify_persistence_reflection(&root, &mut violations);

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

    verify_persistence_reflection(&root, &mut violations);

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

    verify_persistence_reflection(&root, &mut violations);

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
    let inventories = hook_inventories(&root);
    let mut violations = Vec::new();

    verify_hook_name_literals(&root, &inventories, &mut violations);

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
    let inventories = hook_inventories(&root);
    let mut violations = Vec::new();

    verify_hook_name_literals(&root, &inventories, &mut violations);

    assert_eq!(violations, Vec::<String>::new());
    fs::remove_dir_all(root).expect("temporary root should be removable");
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
    format!(
        "\n[[hook]]\nid = \"{id}\"\nclass = \"net.minecraft.client.One\"\nmember = \
         \"one\"\nkind = \"{kind}\"\ndescriptor = \"{descriptor}\"\n"
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
    let inventories = hook_inventories(&root);
    let mut violations = Vec::new();

    verify_transformer_shape(&root, &inventories, &mut violations);

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
    let inventories = hook_inventories(&root);
    let mut violations = Vec::new();

    verify_transformer_shape(&root, &inventories, &mut violations);

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
    let inventories = hook_inventories(&root);
    let mut violations = Vec::new();

    verify_transformer_shape(&root, &inventories, &mut violations);

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
    let inventories = hook_inventories(&root);
    let mut violations = Vec::new();

    verify_transformer_shape(&root, &inventories, &mut violations);

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
    let inventories = hook_inventories(&root);
    let mut violations = Vec::new();

    verify_transformer_shape(&root, &inventories, &mut violations);

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
        format!(
            "schemaVersion = 1\nminecraft = \"26.1.2\"\nmappingSource = \
             \"{mapping_source}\"\n"
        ),
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
