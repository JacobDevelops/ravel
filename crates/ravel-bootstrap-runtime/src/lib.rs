#![forbid(unsafe_code)]

//! Safe hook runtime and JVM crossings linked into Ravel's host libraries.

mod content;
mod dispatch;
mod guard;
mod invocations;
mod legacy;
mod mods;
pub mod outward;
mod packs;
mod persistence;
mod profiling;
mod runtime;
mod settings;
mod side;
mod wire;

pub use guard::HookError;
pub use packs::{
    GAME_PACK_DIRECTORY, RESOURCE_PACKS, WORLD_DATA_PACKS, pack_id, write_resource_pack,
    write_world_pack,
};
pub use persistence::{
    OpenOutcome, SaveOutcome, open_world_data, save_world_data, write_world_datapack,
};
/// Whether declared adapter capabilities select the legacy game surface.
pub use ravel_minecraft_adapter::pinned::LEGACY_ADAPTER;
pub use runtime::{
    Composition, InstallError, environment, install, install_integrated, install_panic_hook,
    integrated_installed, is_installed, pre_world_report, process_launch_context,
};
pub use side::{SideError, side_from_agent_code};

/// JNI exports for native entry points and generated callable hook shapes.
pub mod exports {
    pub use crate::content::ravel_admitted_content;
    #[cfg(test)]
    pub use crate::guard::tests::generated_export;
    pub use crate::invocations::native_invocations;
    pub use crate::legacy::{
        ravel_legacy_command, ravel_legacy_commands, ravel_legacy_event, ravel_legacy_packet,
        ravel_legacy_packet_owner, ravel_legacy_resource, ravel_legacy_resource_paths,
    };
    pub use crate::mods::ravel_mod_surface;
    pub use crate::persistence::crossings::{
        ravel_open_world_persistence, ravel_pack_id, ravel_save_world_persistence,
        ravel_write_staged_datapack, ravel_write_world_datapack,
    };
    pub use crate::profiling::ravel_profiling_report;
    pub use crate::settings::{
        ravel_save_settings, ravel_settings_crossings, ravel_settings_document,
    };
    include!(env!("RAVEL_GENERATED_EXPORTS"));
}

#[cfg(test)]
mod tests {
    use ravel_minecraft::HookKind;
    use ravel_minecraft_adapter::pinned::{LEGACY_ADAPTER, TARGETS};

    /// Creates an empty per-process directory under the system temporary directory, clearing
    /// whatever an earlier run left behind.
    pub(crate) fn fresh_directory(label: &str, name: &str) -> std::path::PathBuf {
        let at = std::env::temp_dir().join(format!("ravel-{label}-{}-{name}", std::process::id()));
        if at.exists() {
            std::fs::remove_dir_all(&at).expect("remove the previous test directory");
        }
        std::fs::create_dir_all(&at).expect("a writable temporary directory");
        at
    }

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

        let fields = targets
            .iter()
            .filter(|target| target.kind() == HookKind::InstanceField)
            .count();
        assert_eq!(
            fields > 0,
            !LEGACY_ADAPTER,
            "modern inventory includes a field to exclude from exports; legacy has no field hooks"
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
