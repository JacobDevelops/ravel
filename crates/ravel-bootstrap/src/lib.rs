#![forbid(unsafe_code)]

//! JVM entry point binding the generated hook surface to Ravel.

mod content;
mod dispatch;
mod game_directory;
mod guard;
mod mods;
pub mod outward;
mod persistence;
mod profiling;
mod report;
mod runtime;
mod settings;
mod side;
mod wire;

pub use dispatch::register;
pub use guard::HookError;
pub use persistence::{OpenOutcome, SaveOutcome, observations, save, stage};
pub use runtime::{LoaderRuntime, environment, install, pre_world_report};
pub use settings::install as install_settings;
pub use side::{SideError, side};

/// The generated JNI exports, one per hook shape.
///
/// ADG-1 chose an exported, JNI-mangled symbol resolved by `System.loadLibrary`
/// and recorded that the export count scales with the hook surface: six
/// inventory entries with heterogeneous descriptors cannot share one
/// `extern "system"` signature, and the single `(int, Object[])` dispatcher that
/// would have avoided that allocates an array on every crossing, which the
/// measured zero-allocation crossing does not. Every declaration here is emitted
/// by the generator with `#[jni::jni_mangle]`, which is what lets this crate
/// keep `#![forbid(unsafe_code)]` verbatim with no relaxation of any kind.
pub mod exports {
    include!(env!("RAVEL_GENERATED_EXPORTS"));
}

#[cfg(test)]
mod tests {
    use ravel_minecraft::{HookKind, VersionAdapter};
    use ravel_minecraft_adapter::PinnedAdapter;

    #[test]
    fn every_callable_target_has_an_exported_symbol_and_no_field_does() {
        let source = include_str!(env!("RAVEL_GENERATED_EXPORTS"));
        let targets = PinnedAdapter.targets();

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

        // Anchored on the constant each export dispatches through, which is the
        // one place a generated export names the target it belongs to.
        //
        // Searching the whole file for a member name asked a question about
        // prose instead: member names are ordinary English, and `dispatcher` —
        // the member behind `command.table` — also appears in this file's own
        // header comment, which no author of an inventory entry controls. The
        // constant is derived from the hook id and written with a trailing
        // comma, so matching it is exact rather than a substring guess, and it
        // says what this test's name promises in both directions: every method
        // dispatches through its own constant, and no field's constant is
        // dispatched through at all.
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

    /// The generator's constant naming, which is what ties an export to the
    /// target it was generated for.
    ///
    /// Duplicated from `render.rs` deliberately: the test needs to derive the
    /// name independently for the comparison to mean anything, and this is the
    /// whole derivation.
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
        // ADG-4's amendment admits a reference that crosses, not one that stays.
        // A `JObject` parameter is owned by the exported frame and reaches a hook
        // only as a borrow, so a reference that does not name its frame has been
        // moved somewhere the borrow checker stopped watching.
        for (index, line) in source.lines().enumerate() {
            assert!(
                !line.contains("JObject") || line.contains("<'local>"),
                "line {} takes a reference without naming the frame: {line}",
                index + 1
            );
        }

        // Checked per export and per parameter rather than by totals. Comparing
        // counts over the whole file lets two exports cancel each other's
        // mistakes, and even within one export it accepts two handles both
        // borrowing `argument0` while `argument1` is silently dropped — the
        // totals agree and a parameter never reached the hook.
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
