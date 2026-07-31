#![forbid(unsafe_code)]

//! JVM entry point binding the generated hook surface to Ravel.

mod dispatch;
mod guard;
mod report;

pub use dispatch::register;
pub use guard::HookError;

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

        let fields = targets
            .iter()
            .filter(|target| !target.kind().is_method())
            .collect::<Vec<_>>();
        assert!(
            fields
                .iter()
                .any(|target| target.kind() == HookKind::InstanceField),
            "the reviewed inventory carries a field, which is not a call target"
        );
        for target in fields {
            assert!(
                !source.contains(target.member()),
                "{} is a field and cannot be called",
                target.id()
            );
        }
    }

    #[test]
    fn no_export_hands_a_game_reference_to_a_hook() {
        let source = include_str!(env!("RAVEL_GENERATED_EXPORTS"));

        assert!(
            !source.contains("JObject"),
            "ADG-4 says a hook carries values, so no export takes a bare object"
        );
        assert!(
            source.contains("HookValue::"),
            "every argument crosses as an owned value"
        );
    }
}
