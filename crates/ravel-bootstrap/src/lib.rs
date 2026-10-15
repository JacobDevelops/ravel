#![forbid(unsafe_code)]

//! Safe hook runtime and JVM crossings linked into Ravel's host libraries.

mod content;
mod dispatch;
mod game_directory;
mod guard;
mod mods;
pub mod outward;
mod packs;
mod persistence;
mod profiling;
mod report;
mod runtime;
mod settings;
mod side;
mod wire;

pub use dispatch::register;
pub use guard::HookError;
pub use packs::write_world_pack;
pub use persistence::{OpenOutcome, SaveOutcome, save, stage};
pub use runtime::{
    LoaderRuntime, data_pack_metadata, environment, install, install_panic_hook, is_installed,
    pre_world_report, resource_pack_metadata,
};
pub use side::{SideError, side};

/// JNI exports for native entry points and generated callable hook shapes.
pub mod exports {
    pub use crate::content::ravel_admitted_content;
    #[cfg(test)]
    pub use crate::guard::tests::generated_export;
    pub use crate::mods::ravel_mod_surface;
    pub use crate::persistence::{
        ravel_open_world_persistence, ravel_pack_id, ravel_save_world_persistence,
        ravel_write_staged_datapack, ravel_write_world_datapack,
    };
    pub use crate::profiling::ravel_profiling_report;
    pub use crate::report::native_invocations;
    pub use crate::settings::{
        ravel_save_settings, ravel_settings_crossings, ravel_settings_document,
    };
    include!(env!("RAVEL_GENERATED_EXPORTS"));
}

#[cfg(test)]
mod tests {
    use ravel_minecraft::HookKind;
    use ravel_minecraft_adapter::pinned::TARGETS;

    #[test]
    fn every_callable_target_has_an_exported_symbol_and_no_field_does() {
        let source = include_str!(env!("RAVEL_GENERATED_EXPORTS"));
        let targets = &TARGETS;

        let callable = targets
            .iter()
            .filter(|target| target.kind().is_method())
            .count();
        assert_eq!(
            source.matches("jni_mangle").count(),
            callable,
            "one export per hook shape, and one only"
        );

        assert!(
            targets
                .iter()
                .any(|target| target.kind() == HookKind::InstanceField),
            "the reviewed inventory carries a field, which is not a call target"
        );

        // Match dispatch constants, not member names that may also occur in prose.
        for target in targets {
            let dispatched = format!("pinned::{},", constant_name(target.id().as_str()));
            assert_eq!(
                source.contains(&dispatched),
                target.kind().is_method(),
                "{} is a {}, so an export {} dispatch through it",
                target.id(),
                if target.kind().is_method() {
                    "method"
                } else {
                    "field"
                },
                if target.kind().is_method() {
                    "must"
                } else {
                    "must not"
                }
            );
        }
    }

    // Derive independently from the generator so this comparison detects drift.
    fn constant_name(id: &str) -> String {
        id.replace(['.', '-'], "_").to_uppercase()
    }

    #[test]
    fn every_reference_an_export_receives_becomes_its_own_borrowed_handle() {
        let source = include_str!(env!("RAVEL_GENERATED_EXPORTS"));

        assert!(
            source.contains("HookValue::"),
            "every argument crosses as a hook value"
        );
        // Every reference parameter must retain the exported frame's lifetime.
        for (index, line) in source.lines().enumerate() {
            assert!(
                !line.contains("JObject") || line.contains("<'local>"),
                "line {} takes a reference without naming the frame: {line}",
                index + 1
            );
        }

        // Per-parameter checks prevent two exports or arguments masking omissions.
        let mut received = 0;
        for export in source.split("#[::jni::jni_mangle").skip(1) {
            let declared = export
                .lines()
                .filter_map(|line| {
                    line.trim_end()
                        .strip_suffix(": ::jni::objects::JObject<'local>,")
                        .map(str::trim)
                })
                .collect::<Vec<_>>();

            for name in &declared {
                assert!(
                    export.contains(&format!("&{name})")),
                    "{name} is received as a reference and never borrowed into a handle"
                );
            }
            assert_eq!(
                declared.len(),
                export.matches("JavaHandle::new(").count(),
                "an export built a handle from something that is not one of its parameters"
            );
            received += declared.len();
        }

        assert_eq!(
            received,
            source.matches("JObject<'local>").count(),
            "every reference in the file is a declared parameter of some export"
        );
    }
}
