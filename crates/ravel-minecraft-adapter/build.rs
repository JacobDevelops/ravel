//! Generates pinned bindings using the shared generator before the adapter compiles.
//!
//! Missing Cargo paths or an unexpected crate layout fail the build with an error.

use ravel_minecraft_generate as generate;

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").ok_or("Cargo did not set CARGO_MANIFEST_DIR")?,
    );
    let root = manifest
        .ancestors()
        .nth(2)
        .ok_or("the adapter crate must live under crates/<name>")?;
    let out = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("Cargo did not set OUT_DIR")?);

    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed={}", generate::SELECTION);

    let selected = match generate::selection(std::env::var_os(generate::SELECTION)) {
        Ok(selected) => selected,
        Err(error) => {
            println!("cargo::error=the selected Minecraft version could not be read: {error}");
            std::process::exit(1);
        }
    };

    let generated = match generate::generate_rust(root, &out, selected.as_deref()) {
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
    Ok(())
}
