//! Forwards adapter-generated export paths from Cargo metadata to the bootstrap.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");

    let exports = match std::env::var("DEP_RAVEL_MINECRAFT_ADAPTER_EXPORTS") {
        Ok(exports) => exports,
        Err(error) => {
            println!("cargo::error=the adapter's generated exports are unavailable: {error}");
            std::process::exit(1);
        }
    };

    println!("cargo::rerun-if-changed={exports}");
    println!("cargo::rustc-env=RAVEL_GENERATED_EXPORTS={exports}");
}
