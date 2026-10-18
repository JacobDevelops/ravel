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
        include_str!("../../../../crates/ravel-bootstrap-runtime/src/persistence/crossings.rs"),
        include_str!("../../../../crates/ravel-bootstrap-runtime/src/profiling.rs"),
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
    // This harness exercises initialization and persistence, not the screen readers.
    for screen in [
        "ravelSettingsDocument",
        "ravelSaveSettings",
        "ravelSettingsCrossings",
        "ravelModSurface",
        "ravelProfilingReport",
    ] {
        assert!(expected.remove(screen));
    }
    // Legacy semantic natives belong to the legacy harness, not this modern stand-in.
    let harness = include_str!("../../../../jvm/harness/initialization/dev/ravel/jvm/Hook.java");
    assert_eq!(java_names(harness, "public static "), expected);
}
