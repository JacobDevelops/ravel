//! Forwards adapter-generated export paths from Cargo metadata to the bootstrap.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");

    let exports = std::env::var("DEP_RAVEL_MINECRAFT_ADAPTER_EXPORTS")
        .expect("the adapter's build script must publish its generated exports");

    println!("cargo::rerun-if-changed={exports}");
    println!("cargo::rustc-env=RAVEL_GENERATED_EXPORTS={exports}");
}
