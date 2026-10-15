//! Generates pinned bindings using the shared generator before the adapter compiles.

#[path = "src/generate/mod.rs"]
mod generate;

use std::path::PathBuf;

/// Asks this build to rewrite the pinned locks before it generates.
const REGENERATE: &str = "RAVEL_REGENERATE_LOCKS";

fn main() {
    let manifest = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"),
    );
    let root = manifest
        .ancestors()
        .nth(2)
        .expect("the crate lives under crates/<name>");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));

    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=src/generate");
    println!("cargo::rerun-if-env-changed={REGENERATE}");
    println!("cargo::rerun-if-env-changed={}", generate::SELECTION);

    // Regeneration must precede the build that would otherwise reject stale locks.
    let selected = match generate::selection(std::env::var_os(generate::SELECTION)) {
        Ok(selected) => selected,
        Err(error) => {
            println!("cargo::error=the selected Minecraft version could not be read: {error}");
            std::process::exit(1);
        }
    };

    if std::env::var_os(REGENERATE).is_some() {
        if let Err(error) = generate::regenerate_locks(root, selected.as_deref()) {
            println!("cargo::error=the pinned locks could not be rewritten: {error}");
            std::process::exit(1);
        }
        println!("cargo::warning=rewrote mappings.lock and stubs.lock from the pinned jar");
    }

    let generated = match generate::run(root, &out, selected.as_deref()) {
        Ok(generated) => generated,
        Err(error) => {
            println!("cargo::error=the pinned Minecraft binding could not be generated: {error}");
            std::process::exit(1);
        }
    };

    // Watch pinned jars even when absent locally.
    for path in &generated.watched {
        println!("cargo::rerun-if-changed={}", path.display());
    }

    // Cargo forwards `links` metadata to direct dependants' build scripts.
    println!("cargo::metadata=bindings={}", generated.bindings.display());
    println!("cargo::metadata=exports={}", generated.exports.display());
    println!("cargo::metadata=java={}", generated.java.display());
    println!("cargo::metadata=stubs={}", generated.stubs.display());
    println!("cargo::metadata=screens={}", generated.screens.display());
    println!("cargo::metadata=harness={}", generated.harness.display());
    println!("cargo::metadata=sha256={}", generated.sha256);
}
