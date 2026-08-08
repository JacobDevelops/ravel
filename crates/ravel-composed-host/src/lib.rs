#![forbid(unsafe_code)]

//! Player-local static composition used by the M3 configuration proof.

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use jni::EnvUnowned;
use jni::objects::JClass;
use ravel_bootstrap_runtime::{HookError, install, install_settings};
use ravel_config::{LayerError, LayeredConfiguration, file};
use ravel_core::{EmptySideSet, Side, SideSet};
use ravel_host_api::HostMod;
use ravel_loader::{DriveError, LifecycleEntry, LifecycleMachine, Requirement, drive};
use ravel_test_mod_configuration::ConfigurationMod;
use ravel_test_mod_saved_data::SavedDataMod;

/// The override path copied from the selected pack manifest into this generated host.
pub const PACK_CONFIGURATION_PATH: &str = "ravel-pack/configuration.json";

/// The driven runtime pieces and resolved settings produced for one launch.
pub type Composition = (
    LifecycleMachine,
    Vec<Box<dyn HostMod>>,
    Vec<LayeredConfiguration>,
);

/// Constructs and drives this source-composed mod set for one launch.
///
/// # Errors
///
/// Returns an error when a selected-pack or player document cannot be read,
/// configuration cannot be resolved or consumed, or the lifecycle cannot reach
/// running.
pub fn compose(game_directory: &Path, side: Side) -> Result<Composition, LaunchError> {
    let mut configuration = ConfigurationMod::new();
    let declaration = ConfigurationMod::declaration();
    let pack_path = game_directory.join(PACK_CONFIGURATION_PATH);
    let pack = file::read_path(&pack_path, &declaration.owner).map_err(LaunchError::PackFile)?;
    let player = file::read(game_directory, &declaration.owner).map_err(LaunchError::PlayerFile)?;
    let resolved = LayeredConfiguration::resolve(declaration, pack.as_ref(), player.as_ref())
        .map_err(LaunchError::Layer)?;
    configuration
        .configure(&resolved)
        .map_err(LaunchError::Configuration)?;

    let entry = LifecycleEntry {
        mod_id: configuration.id().clone(),
        sides: SideSet::new(Side::ALL).map_err(LaunchError::Sides)?,
        requirement: Requirement::Required,
        saved_data_versions: BTreeSet::new(),
    };
    // The world-owning half of the composition. Until this was here no mod in a
    // running build owned world data, so the saved-data round trip could not be
    // observed in a launch at all.
    let saved = SavedDataMod::new();
    let saved_entry = LifecycleEntry {
        mod_id: saved.id().clone(),
        sides: SideSet::new([Side::Client, Side::DedicatedServer]).map_err(LaunchError::Sides)?,
        requirement: Requirement::Required,
        saved_data_versions: saved.declaration().versions().collect(),
    };

    let (machine, mods) = drive(
        side,
        vec![
            (entry, Box::new(configuration) as Box<dyn HostMod>),
            (saved_entry, Box::new(saved)),
        ],
    )
    .map_err(LaunchError::Drive)?;

    Ok((machine, mods, vec![resolved]))
}

fn initialize() -> Result<(), LaunchError> {
    let game_directory = std::env::current_dir().map_err(LaunchError::CurrentDirectory)?;
    let side = Side::Client;
    let (machine, mods, configurations) = compose(&game_directory, side)?;
    install_settings(side, configurations);
    install(machine, mods).map_err(LaunchError::Install)
}

/// Starts the player-local statically composed set before game methods run.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelInitialize")]
pub fn ravel_initialize<'local>(mut env: EnvUnowned<'local>, _class: JClass<'local>) {
    env.with_env(|_| initialize())
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

/// Why the selected source-composed set could not start.
#[derive(Debug)]
pub enum LaunchError {
    /// The JNI frame could not establish its guarded environment.
    Jvm(jni::errors::Error),
    /// The process could not name its launch directory.
    CurrentDirectory(std::io::Error),
    /// The selected pack's override document could not be read.
    PackFile(file::FileError),
    /// The player's existing value document could not be read.
    PlayerFile(file::FileError),
    /// The static composition declared no execution side.
    Sides(EmptySideSet),
    /// The declaration and its layers could not be resolved.
    Layer(LayerError),
    /// The concrete mod could not consume its declared settings.
    Configuration(String),
    /// The lifecycle could not drive the composed value.
    Drive(DriveError),
    /// The driven runtime could not be installed.
    Install(HookError),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Jvm(error) => write!(formatter, "the JVM refused initialization: {error}"),
            Self::CurrentDirectory(error) => {
                write!(formatter, "the launch directory cannot be read: {error}")
            }
            Self::PackFile(error) => {
                write!(formatter, "pack configuration cannot be read: {error}")
            }
            Self::PlayerFile(error) => {
                write!(formatter, "player configuration cannot be read: {error}")
            }
            Self::Sides(error) => write!(formatter, "the composed mod set has no side: {error}"),
            Self::Layer(error) => write!(formatter, "configuration cannot be resolved: {error}"),
            Self::Configuration(error) => {
                write!(
                    formatter,
                    "the configuration mod refused its declaration: {error}"
                )
            }
            Self::Drive(error) => write!(formatter, "the composed mod set cannot start: {error}"),
            Self::Install(error) => write!(
                formatter,
                "the running mod set cannot be installed: {error}"
            ),
        }
    }
}

impl From<jni::errors::Error> for LaunchError {
    fn from(error: jni::errors::Error) -> Self {
        Self::Jvm(error)
    }
}

impl std::error::Error for LaunchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Jvm(error) => Some(error),
            Self::CurrentDirectory(error) => Some(error),
            Self::PackFile(error) | Self::PlayerFile(error) => Some(error),
            Self::Sides(error) => Some(error),
            Self::Layer(error) => Some(error),
            Self::Configuration(_) => None,
            Self::Drive(error) => Some(error),
            Self::Install(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use ravel_config::{ValueDocument, file};
    use ravel_core::{LifecycleState, ModId, Side};
    use serde_json::json;

    use super::{PACK_CONFIGURATION_PATH, compose};

    fn directory(name: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "ravel-configuration-host-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("a writable temporary directory");
        directory
    }

    fn owner() -> ModId {
        "configuration"
            .parse()
            .expect("the tracked mod id is valid")
    }

    fn saved_owner() -> ModId {
        "saved_data".parse().expect("the tracked mod id is valid")
    }

    #[test]
    fn the_selected_composition_drives_both_mods() {
        let game = directory("defaults");
        let (machine, mods, configurations) =
            compose(&game, Side::Client).expect("the tracked composition should start");

        assert_eq!(machine.state(&owner()), Ok(LifecycleState::Running));
        assert_eq!(machine.state(&saved_owner()), Ok(LifecycleState::Running));

        let driven: Vec<&ModId> = mods.iter().map(|held| held.id()).collect();
        assert_eq!(driven.len(), 2);
        assert!(driven.contains(&&owner()));
        assert!(driven.contains(&&saved_owner()));

        // Only the configuration mod declares settings, so the resolved
        // configurations stay one even though the composition is now two.
        assert_eq!(configurations.len(), 1);
        assert_eq!(configurations[0].owner(), &owner());
    }

    #[test]
    fn the_world_owning_mod_carries_the_versions_it_declares() {
        let game = directory("saved-versions");
        let (machine, _, _) =
            compose(&game, Side::Client).expect("the tracked composition should start");

        assert!(
            machine.order().any(|id| id == &saved_owner()),
            "the world-owning mod is part of the driven load order"
        );
    }

    #[test]
    fn the_selected_pack_file_applies_per_setting_policies() {
        let game = directory("pack-values");
        let pack = ValueDocument::parse(
            &json!({
                "schemaVersion": 1,
                "owner": "configuration",
                "values": {
                    "client.enabled": false,
                    "integrated.mob_limit": 40,
                    "dedicated.motd": "Pack",
                    "common.profile": "verbose"
                }
            })
            .to_string(),
        )
        .expect("a valid pack document");
        let player = ValueDocument::parse(
            &json!({
                "schemaVersion": 1,
                "owner": "configuration",
                "values": {
                    "client.enabled": true,
                    "integrated.mob_limit": 10,
                    "dedicated.motd": "Player",
                    "common.profile": "quiet"
                }
            })
            .to_string(),
        )
        .expect("a valid player document");
        let pack_path = game.join(PACK_CONFIGURATION_PATH);
        std::fs::create_dir_all(pack_path.parent().expect("the path has a parent"))
            .expect("the pack directory should be writable");
        std::fs::write(&pack_path, pack.to_json()).expect("the pack file should be writable");
        file::write(&game, &player).expect("the player file should be writable");

        let (_, _, configurations) =
            compose(&game, Side::Client).expect("the selected pack values should resolve");
        let resolved = &configurations[0];

        assert_eq!(
            resolved.value(&"client.enabled".parse().expect("a setting key")),
            Some(&json!(true)),
            "allowed leaves the last word to the player"
        );
        assert_eq!(
            resolved.value(&"integrated.mob_limit".parse().expect("a setting key")),
            Some(&json!(40)),
            "locked leaves the last word to the pack"
        );
        assert_eq!(
            resolved.value(&"dedicated.motd".parse().expect("a setting key")),
            Some(&json!("Pack")),
            "required is supplied by and fixed to the pack"
        );
        assert_eq!(
            resolved.value(&"common.profile".parse().expect("a setting key")),
            Some(&json!("quiet"))
        );
    }

    #[test]
    fn a_player_file_changes_the_selected_values() {
        let game = directory("player-values");
        let player = ValueDocument::parse(
            &json!({
                "schemaVersion": 1,
                "owner": "configuration",
                "values": {
                    "client.enabled": false,
                    "common.profile": "verbose"
                }
            })
            .to_string(),
        )
        .expect("a valid player document");
        file::write(&game, &player).expect("the player file should be writable");

        let (_, _, configurations) =
            compose(&game, Side::Client).expect("the player values should resolve");

        assert_eq!(
            configurations[0].value(&"client.enabled".parse().expect("a setting key")),
            Some(&json!(false))
        );
        assert_eq!(
            configurations[0].value(&"common.profile".parse().expect("a setting key")),
            Some(&json!("verbose"))
        );
    }
}
