use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

#[test]
fn java_generation_is_an_explicit_command() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("xtask lives under tools");
    let started = SystemTime::now();
    let output = Command::new(env!("CARGO_BIN_EXE_ravel-xtask"))
        .args(["generate", "java"])
        .current_dir(root)
        .output()
        .expect("run Java generation");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let hook = root.join("target/ravel-jvm/generated/dev/ravel/jvm/Hook.java");
    assert!(
        hook.metadata()
            .expect("generated Hook.java")
            .modified()
            .expect("Hook.java modification time")
            >= started,
        "the explicit command must rewrite Java even when outputs already exist"
    );
    for relative in [
        "generated/dev/ravel/jvm/Hook.java",
        "generated-stubs",
        "generated-screen",
        "generated-harness/overrides.txt",
    ] {
        assert!(
            root.join("target/ravel-jvm").join(relative).exists(),
            "{relative}"
        );
    }
}
