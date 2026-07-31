//! Points the crate at the exports the adapter's generator wrote.
//!
//! The adapter declares `links` in its manifest, so Cargo forwards its build
//! script's metadata here. That is the whole reason this file exists: the
//! generator runs once, in the crate that owns the version data, and the
//! bootstrap includes what it produced rather than reading another crate's
//! `OUT_DIR` by hand or generating the same thing twice.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");

    let exports = std::env::var("DEP_RAVEL_MINECRAFT_ADAPTER_EXPORTS")
        .expect("the adapter's build script must publish its generated exports");

    println!("cargo::rerun-if-changed={exports}");
    println!("cargo::rustc-env=RAVEL_GENERATED_EXPORTS={exports}");
}
