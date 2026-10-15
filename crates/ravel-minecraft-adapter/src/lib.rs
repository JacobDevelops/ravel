#![forbid(unsafe_code)]

//! Shared adapter runtime for version data and generated bindings.

use ravel_minecraft::{HookId, HookTarget, VersionAdapter};

#[cfg(feature = "generate")]
pub mod generate;

/// The target table generated from the pinned version's reviewed inventory.
///
/// The generator writes this into `OUT_DIR` rather than into the source tree,
/// because its header names the Minecraft version it resolved against and a
/// version string is only allowed to live in version data.
pub mod pinned {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

/// The adapter over the one version this build generated against.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PinnedAdapter;

impl VersionAdapter for PinnedAdapter {
    fn minecraft_version(&self) -> &str {
        pinned::MINECRAFT_VERSION
    }

    fn targets(&self) -> &[HookTarget] {
        &pinned::TARGETS
    }

    fn non_goals(&self) -> &[HookId] {
        &pinned::NON_GOALS
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use ravel_minecraft::{HookId, HookKind, VersionAdapter};

    use super::{PinnedAdapter, pinned};

    #[test]
    fn the_adapter_reports_the_version_its_data_directory_is_named_for() {
        let versions = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("the crate lives under crates/<name>")
            .join("minecraft/versions");
        let names = fs::read_dir(versions)
            .expect("the version data should be readable")
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(
            names.contains(&PinnedAdapter.minecraft_version().to_owned()),
            "the generated binding should name a tracked version, not {names:?}"
        );
    }

    #[test]
    fn every_reviewed_hook_reached_the_generated_target_table() {
        let adapter = PinnedAdapter;

        assert_eq!(adapter.targets().len(), pinned::TARGETS.len());
        assert!(
            adapter.targets().len() >= 6,
            "the reviewed inventory carries at least six resolved hook targets"
        );
        // The JVM writes an internal name with slashes and never with dots. A
        // class in a package therefore carries a slash, and one the version
        // links into the unnamed package carries neither — which is most of the
        // game under an obfuscated version, so asserting a slash was asserting
        // that this build selected an unobfuscated one.
        assert!(
            adapter
                .targets()
                .iter()
                .all(|target| !target.class().contains('.')),
            "a target names its class the way the JVM writes it"
        );
        assert_eq!(adapter.target(HookId::new("absent.hook")), None);
    }

    #[test]
    fn the_table_carries_both_a_method_and_a_field_target() {
        let adapter = PinnedAdapter;

        assert!(
            adapter
                .targets()
                .iter()
                .any(|target| target.kind() == HookKind::StaticMethod)
        );
        assert!(
            adapter
                .targets()
                .iter()
                .any(|target| target.kind() == HookKind::InstanceField),
            "field extraction is what proves the generator reads more than methods"
        );
    }
}
