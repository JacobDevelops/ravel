//! Handwritten JNI contracts span the bootstrap, composed host, generator, and harness.

use std::collections::BTreeSet;

fn java_names<'a>(source: &'a str, marker: &str) -> BTreeSet<&'a str> {
    source
        .lines()
        .filter_map(|line| line.split_once(marker))
        .filter_map(|(_, declaration)| declaration.split_once('('))
        .filter_map(|(head, _)| head.split_whitespace().last())
        .filter(|name| name.starts_with("ravel") || *name == "nativeInvocations")
        .collect()
}

#[test]
fn handwritten_java_natives_match_the_rust_exports() {
    let generator = include_str!("../../../../crates/ravel-minecraft-generate/src/render/hooks.rs");
    let sources = [
        include_str!("../../../../crates/ravel-bootstrap-runtime/src/content.rs"),
        include_str!("../../../../crates/ravel-bootstrap-runtime/src/invocations.rs"),
        include_str!("../../../../crates/ravel-bootstrap-runtime/src/legacy.rs"),
        include_str!("../../../../crates/ravel-bootstrap-runtime/src/mods.rs"),
        include_str!("../../../../crates/ravel-bootstrap-runtime/src/outward.rs"),
        include_str!("../../../../crates/ravel-bootstrap-runtime/src/packs/crossings.rs"),
        include_str!("../../../../crates/ravel-bootstrap-runtime/src/persistence/crossings.rs"),
        include_str!("../../../../crates/ravel-bootstrap-runtime/src/profiling.rs"),
        include_str!("../../../../crates/ravel-bootstrap-runtime/src/renderer.rs"),
        include_str!("../../../../crates/ravel-bootstrap-runtime/src/renderer/baked.rs"),
        include_str!("../../../../crates/ravel-bootstrap-runtime/src/settings.rs"),
        include_str!("../../../../crates/ravel-composed-host/src/lib.rs"),
    ];
    let exports = sources
        .iter()
        .flat_map(|source| source.lines())
        .filter_map(|line| line.split_once("jni_mangle(\"dev.ravel.jvm.Hook\", \""))
        .map(|(_, tail)| tail.split_once('"').expect("JNI method name").0)
        .collect::<BTreeSet<_>>();
    assert_eq!(java_names(generator, "public static native "), exports);
}

#[test]
fn initialization_harness_implements_the_handwritten_crossings_it_exercises() {
    let generator = include_str!("../../../../crates/ravel-minecraft-generate/src/render/hooks.rs");
    let (_, modern) = generator
        .split_once("fn written_declarations()")
        .expect("modern handwritten declarations");
    let mut expected = java_names(modern, "public static native ");
    // This harness exercises initialization and persistence, not screen readers or rendering.
    for unused in [
        "ravelSettingsDocument",
        "ravelSaveSettings",
        "ravelSettingsCrossings",
        "ravelModSurface",
        "ravelProfilingReport",
        "ravelRendererEvent",
        "ravelRendererAccept",
        "ravelRendererSnapshot",
        "ravelRendererWork",
        "ravelRendererMesh",
        "ravelRendererBaked",
        "ravelRendererBakedLease",
        "ravelRendererBakedResort",
    ] {
        assert!(expected.remove(unused));
    }
    // Legacy semantic natives belong to the legacy harness, not this modern stand-in.
    let harness = include_str!("../../../../jvm/harness/initialization/dev/ravel/jvm/Hook.java");
    assert_eq!(java_names(harness, "public static "), expected);
}

#[test]
fn renderer_semantic_natives_keep_the_shared_bounded_primitive_contract() {
    let generator = include_str!("../../../../crates/ravel-minecraft-generate/src/render/hooks.rs");
    let renderer_names: Vec<_> = generator
        .split("public static native ")
        .skip(1)
        .filter_map(|declaration| declaration.split_once('('))
        .filter_map(|(head, _)| head.split_whitespace().last())
        .filter(|name| name.starts_with("ravelRenderer"))
        .collect();
    assert_eq!(
        renderer_names.len(),
        8,
        "exactly eight renderer declarations"
    );
    assert_eq!(
        renderer_names.into_iter().collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "ravelRendererAccept",
            "ravelRendererEvent",
            "ravelRendererSnapshot",
            "ravelRendererWork",
            "ravelRendererMesh",
            "ravelRendererBaked",
            "ravelRendererBakedLease",
            "ravelRendererBakedResort",
        ]),
        "renderer crossings carry only scalars and bounded primitive batches"
    );
}

#[test]
fn renderer_semantic_native_signatures_share_copied_primitives() {
    let generator = include_str!("../../../../crates/ravel-minecraft-generate/src/render/hooks.rs");
    let (_, shared) = generator
        .split_once("fn written_declarations()")
        .expect("shared handwritten declarations");
    let shared = shared.replace("\\n\\", " ").replace("\\x20", " ");
    let names = java_names(&shared, "public static native ");
    for (name, returned, parameters) in [
        (
            "ravelRendererEvent",
            "long",
            vec!["int", "long", "long", "boolean"],
        ),
        ("ravelRendererAccept", "boolean", vec!["long"]),
        (
            "ravelRendererSnapshot",
            "long[]",
            vec!["long", "long", "int[]", "byte[]", "byte[]"],
        ),
        ("ravelRendererWork", "long[]", vec!["int", "long", "long"]),
        ("ravelRendererMesh", "byte[]", vec!["long", "long"]),
        (
            "ravelRendererBaked",
            "long[]",
            vec!["long", "long[]", "byte[]", "byte[]", "int[]"],
        ),
        (
            "ravelRendererBakedLease",
            "long[]",
            vec!["long", "long", "int"],
        ),
        (
            "ravelRendererBakedResort",
            "int",
            vec!["long", "long", "float[]", "int[]"],
        ),
    ] {
        assert!(
            names.contains(name),
            "{name} must be shared, not legacy-only"
        );
        let marker = format!("public static native {returned} {name}(");
        let (_, tail) = shared
            .split_once(&marker)
            .expect("renderer native return type");
        let (arguments, _) = tail.split_once(')').expect("renderer native parameters");
        assert_eq!(
            arguments
                .split(',')
                .map(|argument| argument.split_whitespace().next().expect("parameter type"))
                .collect::<Vec<_>>(),
            parameters,
            "{name} must carry copied primitives, never SDK renderer objects"
        );
        assert_eq!(shared.matches(&marker).count(), 1, "{name}");
    }
}
