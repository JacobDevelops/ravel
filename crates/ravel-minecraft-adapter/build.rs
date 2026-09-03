//! Generates the pinned Minecraft binding before the adapter compiles.
//!
//! The generator is the crate's own module, included here rather than depended
//! on: a build script cannot depend on the library it is building, and ADG-3's
//! logic belongs in one place regardless of which of the two compiles it.

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
    // Selection is a build-time choice, so a build that changes it has to
    // generate again rather than serve the previous version's tree from cache.
    println!("cargo::rerun-if-env-changed={}", generate::SELECTION);

    // Rewriting a lock happens here because the generator refuses to build
    // against a lock it has drifted from, so nothing that depends on this crate
    // could run the writer. Asking for it is an explicit environment variable
    // and a devenv task rather than a flag some ordinary build could set.
    // The version data may hold several versions; this build emits one. Where
    // it holds exactly one, nothing has to be set.
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

    // The client jar is named here even though most machines do not hold it, so
    // that a machine which does notices the moment it changes.
    for path in &generated.watched {
        println!("cargo::rerun-if-changed={}", path.display());
    }

    // `links` in the manifest is what makes Cargo forward these to the build
    // script of every direct dependant, which is how `ravel-bootstrap` finds the
    // exports without reading this crate's `OUT_DIR` by hand.
    println!("cargo::metadata=bindings={}", generated.bindings.display());
    println!("cargo::metadata=exports={}", generated.exports.display());
    println!("cargo::metadata=java={}", generated.java.display());
    println!("cargo::metadata=stubs={}", generated.stubs.display());
    println!("cargo::metadata=screens={}", generated.screens.display());
    println!("cargo::metadata=harness={}", generated.harness.display());
    println!("cargo::metadata=sha256={}", generated.sha256);
}
