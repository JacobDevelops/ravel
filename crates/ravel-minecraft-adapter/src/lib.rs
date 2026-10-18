#![forbid(unsafe_code)]

//! Generated version data in [`pinned`] and reviewed hook lookup through [`target`].

use ravel_minecraft::{HookId, HookTarget};

/// The target table generated from the pinned version's reviewed inventory.
pub mod pinned {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

/// Finds a reviewed hook in the generated target table.
#[must_use]
pub fn target(id: HookId) -> Option<&'static HookTarget> {
    pinned::TARGETS.iter().find(|target| target.id() == id)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use ravel_minecraft::{HookId, HookKind};
    use ravel_minecraft_generate::{Kind, hook_inventory};

    use super::{pinned, target};

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
            names.contains(&pinned::MINECRAFT_VERSION.to_owned()),
            "the generated binding should name a tracked version, not {names:?}"
        );
    }

    #[test]
    fn every_reviewed_hook_reached_the_generated_target_table() {
        assert!(
            pinned::TARGETS.len() >= 6,
            "the reviewed inventory carries at least six resolved hook targets"
        );
        // Internal names permit an unnamed package but never use dots.
        assert!(
            pinned::TARGETS
                .iter()
                .all(|target| !target.class().contains('.')),
            "a target names its class the way the JVM writes it"
        );
        assert_eq!(target(HookId::new("absent.hook")), None);
        for reviewed in &pinned::TARGETS {
            assert_eq!(target(reviewed.id()), Some(reviewed));
        }
    }

    #[test]
    fn the_table_carries_the_target_kinds_the_inventory_declares() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("the crate lives under crates/<name>");
        let inventory = hook_inventory(root, pinned::MINECRAFT_VERSION)
            .expect("the selected adapter owns a valid reviewed inventory");
        assert_eq!(pinned::TARGETS.len(), inventory.hooks.len());
        for hook in inventory.hooks {
            let expected = match hook.kind {
                Kind::StaticMethod => HookKind::StaticMethod,
                Kind::InstanceMethod => HookKind::InstanceMethod,
                Kind::StaticField => HookKind::StaticField,
                Kind::InstanceField => HookKind::InstanceField,
            };
            let generated = pinned::TARGETS
                .iter()
                .find(|target| target.id().as_str() == hook.id)
                .expect("every reviewed hook reaches the target table");
            assert_eq!(generated.kind(), expected, "{}", hook.id);
        }
    }
}
