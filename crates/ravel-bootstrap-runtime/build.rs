//! Forwards adapter-generated export paths from Cargo metadata to the bootstrap.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(ravel_has_refusable_hooks)");

    let exports = match std::env::var("DEP_RAVEL_MINECRAFT_ADAPTER_EXPORTS") {
        Ok(exports) => exports,
        Err(error) => {
            println!("cargo::error=the adapter's generated exports are unavailable: {error}");
            std::process::exit(1);
        }
    };
    let has_refusable_hooks = match std::env::var("DEP_RAVEL_MINECRAFT_ADAPTER_HAS_REFUSABLE_HOOKS")
    {
        Ok(value) => match value.as_str() {
            "true" => true,
            "false" => false,
            _ => {
                println!("cargo::error=the adapter reported an invalid refusable-hook capability");
                std::process::exit(1);
            }
        },
        Err(error) => {
            println!("cargo::error=the adapter's hook capabilities are unavailable: {error}");
            std::process::exit(1);
        }
    };

    println!("cargo::rerun-if-changed={exports}");
    if has_refusable_hooks {
        println!("cargo::rustc-cfg=ravel_has_refusable_hooks");
    }
    println!("cargo::rustc-env=RAVEL_GENERATED_EXPORTS={exports}");
}
