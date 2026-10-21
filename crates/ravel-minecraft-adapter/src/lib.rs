#![forbid(unsafe_code)]

//! Generated version data, admission environment identity, and reviewed hook lookup.

use ravel_minecraft::{HookId, HookTarget};

/// The target table generated from the pinned version's reviewed inventory.
pub mod pinned {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

/// Returns the pinned runtime environment for admission and failure reports.
///
/// # Panics
/// Panics if this crate's package version is not valid semver.
#[must_use]
pub fn environment() -> ravel_core::FailureEnvironment {
    ravel_core::FailureEnvironment {
        ravel: env!("CARGO_PKG_VERSION")
            .parse()
            .expect("every crate in this workspace carries a semver version"),
        minecraft: pinned::MINECRAFT_VERSION.to_owned(),
        platform: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
    }
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
    fn admission_environment_identifies_the_selected_adapter_and_process() {
        let environment = super::environment();

        assert_eq!(environment.ravel.to_string(), env!("CARGO_PKG_VERSION"));
        assert_eq!(environment.minecraft, pinned::MINECRAFT_VERSION);
        assert_eq!(environment.platform, std::env::consts::OS);
        assert_eq!(environment.architecture, std::env::consts::ARCH);
    }

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
