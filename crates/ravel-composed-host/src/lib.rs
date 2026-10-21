#![forbid(unsafe_code)]

//! JVM native host using [`ravel_host_runtime`] for admission and startup.
//! The default `fixture-mods` feature composes the tracked test-mod member list;
//! `--no-default-features` builds an empty composition with the same runtime and hooks.

use jni::EnvUnowned;
use jni::objects::JClass;
use ravel_bootstrap_runtime::Composition;
use ravel_core::Side;
use ravel_host_runtime::Member;
#[cfg(feature = "fixture-mods")]
use ravel_test_mod_audio::AudioMod;
#[cfg(feature = "fixture-mods")]
use ravel_test_mod_client_only::{ClientOnlyMod, work_log};
#[cfg(feature = "fixture-mods")]
use ravel_test_mod_commands::CommandsMod;
#[cfg(feature = "fixture-mods")]
use ravel_test_mod_configuration::ConfigurationMod;
#[cfg(feature = "fixture-mods")]
use ravel_test_mod_networking::NetworkingMod;
#[cfg(feature = "fixture-mods")]
use ravel_test_mod_registries::RegistriesMod;
#[cfg(feature = "fixture-mods")]
use ravel_test_mod_saved_data::SavedDataMod;
#[cfg(feature = "fixture-mods")]
use ravel_test_mod_server_only::{ServerOnlyMod, work_log as server_work_log};

pub use ravel_host_runtime::LaunchError;

// Admit the same manifests the members ship.
#[cfg(feature = "fixture-mods")]
const CONFIGURATION_PACKAGE: &str = include_str!("../../../test-mods/configuration/package.json");
#[cfg(feature = "fixture-mods")]
const SAVED_DATA_PACKAGE: &str = include_str!("../../../test-mods/saved-data/package.json");
#[cfg(feature = "fixture-mods")]
const CLIENT_ONLY_PACKAGE: &str = include_str!("../../../test-mods/client-only/package.json");
#[cfg(feature = "fixture-mods")]
const SERVER_ONLY_PACKAGE: &str = include_str!("../../../test-mods/server-only/package.json");
#[cfg(feature = "fixture-mods")]
const REGISTRIES_PACKAGE: &str = include_str!("../../../test-mods/registries/package.json");
#[cfg(feature = "fixture-mods")]
const COMMANDS_PACKAGE: &str = include_str!("../../../test-mods/commands/package.json");
#[cfg(feature = "fixture-mods")]
const NETWORKING_PACKAGE: &str = include_str!("../../../test-mods/networking/package.json");
#[cfg(feature = "fixture-mods")]
const AUDIO_PACKAGE: &str = include_str!("../../../test-mods/audio/package.json");

#[cfg(feature = "fixture-mods")]
fn members(side: Side) -> Vec<Member> {
    let mut members = vec![
        Member {
            package: CONFIGURATION_PACKAGE.to_owned(),
            value: Box::new(ConfigurationMod::new()),
        },
        Member {
            package: SAVED_DATA_PACKAGE.to_owned(),
            value: Box::new(SavedDataMod::new()),
        },
        Member {
            package: CLIENT_ONLY_PACKAGE.to_owned(),
            value: Box::new(ClientOnlyMod::new(side, &work_log())),
        },
        Member {
            package: REGISTRIES_PACKAGE.to_owned(),
            value: Box::new(RegistriesMod::new()),
        },
        Member {
            package: SERVER_ONLY_PACKAGE.to_owned(),
            value: Box::new(ServerOnlyMod::new(side, &server_work_log())),
        },
    ];
    if ravel_bootstrap_runtime::LEGACY_ADAPTER {
        members.push(Member {
            package: COMMANDS_PACKAGE.to_owned(),
            value: Box::new(CommandsMod::new()),
        });
        members.push(Member {
            package: NETWORKING_PACKAGE.to_owned(),
            value: Box::new(NetworkingMod::new()),
        });
        members.push(Member {
            package: AUDIO_PACKAGE.to_owned(),
            value: Box::new(AudioMod::new()),
        });
    }
    members
}

#[cfg(not(feature = "fixture-mods"))]
fn members(_side: Side) -> Vec<Member> {
    Vec::new()
}

/// Constructs and drives the composition selected by the `fixture-mods` build feature.
///
/// # Errors
///
/// Returns an error if package admission, configuration, resources, or lifecycle startup fails.
pub fn compose(game_directory: &std::path::Path, side: Side) -> Result<Composition, LaunchError> {
    ravel_host_runtime::compose(game_directory, side, members(side))
}

/// Starts the selected native host composition before game methods run.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelInitialize")]
pub fn ravel_initialize<'local>(
    env: EnvUnowned<'local>,
    _class: JClass<'local>,
    side: jni::sys::jint,
    profiling_requested: jni::sys::jboolean,
) {
    ravel_host_runtime::enter(env, side, profiling_requested, members);
}

/// Starts a fresh composition for the integrated server's own thread and side.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelInitializeIntegratedServer")]
pub fn ravel_initialize_integrated_server<'local>(env: EnvUnowned<'local>, _class: JClass<'local>) {
    ravel_host_runtime::enter_integrated(env, members);
}

/// Exposes original fixture receipts only to the composed host's live packet test.
#[cfg(feature = "fixture-mods")]
#[jni::jni_mangle("dev.ravel.jvm.LegacyPacketLive", "receipts")]
pub fn legacy_packet_receipts<'local>(
    _env: EnvUnowned<'local>,
    _class: JClass<'local>,
    to_server: jni::sys::jboolean,
) -> jni::sys::jlong {
    let count = if to_server == jni::sys::JNI_FALSE {
        ravel_test_mod_client_only::accepted_packets()
    } else {
        ravel_test_mod_networking::accepted_packets()
    };
    count.try_into().unwrap_or(jni::sys::jlong::MAX)
}

#[cfg(all(test, not(feature = "fixture-mods")))]
mod empty_tests {
    use ravel_core::Side;

    #[test]
    fn the_selected_factory_and_real_composition_are_empty_on_every_side() {
        for side in Side::ALL {
            assert!(super::members(side).is_empty());
            let game = std::env::temp_dir().join(format!(
                "ravel-empty-host-{}-{}",
                std::process::id(),
                side.as_str()
            ));
            if game.exists() {
                std::fs::remove_dir_all(&game).expect("remove stale temporary directory");
            }
            std::fs::create_dir_all(&game).expect("a writable temporary directory");

            let composition = super::compose(&game, side).expect("the empty host should start");
            let machine = composition.loaded.machine();
            assert_eq!(machine.side(), side);
            assert_eq!(machine.order().count(), 0);
            assert_eq!(machine.claimed_keys().count(), 0);
            assert!(composition.configurations.is_empty());
            assert!(composition.installed_packages.is_empty());
            assert!(composition.resources.generated().is_empty());

            std::fs::remove_dir_all(&game).expect("remove temporary directory");
        }
    }
}

#[cfg(all(test, feature = "fixture-mods"))]
mod tests {
    use ravel_bootstrap_runtime::Composition;
    use ravel_config::{LayeredConfiguration, ValueDocument, file};
    use ravel_core::{LifecycleState, ModId, Side};
    use ravel_loader::DiscoveryRefused;
    use serde_json::json;

    use super::{
        CLIENT_ONLY_PACKAGE, CONFIGURATION_PACKAGE, LaunchError, SAVED_DATA_PACKAGE,
        SERVER_ONLY_PACKAGE, compose,
    };

    fn directory(name: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "ravel-configuration-host-{}-{name}",
            std::process::id()
        ));
        if directory.exists() {
            std::fs::remove_dir_all(&directory).expect("remove stale temporary directory");
        }
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

    fn settings_of<'a>(
        configurations: &'a [LayeredConfiguration],
        owner: &ModId,
    ) -> &'a LayeredConfiguration {
        configurations
            .iter()
            .find(|resolved| resolved.owner() == owner)
            .expect("this owner declares settings")
    }

    fn registries_owner() -> ModId {
        "registries".parse().expect("the tracked mod id is valid")
    }

    fn client_owner() -> ModId {
        "client_only".parse().expect("the tracked mod id is valid")
    }

    fn server_owner() -> ModId {
        "server_only".parse().expect("the tracked mod id is valid")
    }

    #[test]
    fn the_source_factory_instantiates_every_member_even_when_parked() {
        for side in Side::ALL {
            let supplied = super::members(side);
            assert_eq!(
                supplied.len(),
                if ravel_bootstrap_runtime::LEGACY_ADAPTER {
                    8
                } else {
                    5
                },
                "the actual factory must supply the complete set on {side:?}"
            );
            for owner in [
                owner(),
                saved_owner(),
                client_owner(),
                server_owner(),
                registries_owner(),
            ] {
                assert!(
                    supplied.iter().any(|member| member.value.id() == &owner),
                    "the actual factory omitted {owner} on {side:?}"
                );
            }
        }
    }

    #[test]
    fn the_client_only_member_is_admitted_and_parked_on_a_dedicated_server() {
        let game = directory("dedicated");
        let Composition { loaded, .. } =
            compose(&game, Side::DedicatedServer).expect("the tracked composition should start");
        let machine = loaded.machine();

        assert_eq!(
            machine.state(&client_owner()),
            Ok(LifecycleState::Resolved),
            "a client-only mod started on a dedicated server"
        );
        assert_eq!(machine.state(&owner()), Ok(LifecycleState::Running));
        assert_eq!(machine.state(&saved_owner()), Ok(LifecycleState::Running));
        assert_eq!(
            machine.state(&server_owner()),
            Ok(LifecycleState::Running),
            "a server-only mod was kept off the side it declares"
        );
        assert!(
            machine
                .entries()
                .any(|entry| entry.mod_id == client_owner()),
            "a parked mod is installed rather than absent"
        );
    }

    #[test]
    fn integrated_composition_has_its_own_world_owners_and_legacy_contracts() -> Result<(), String>
    {
        let game = directory("integrated-world-owners");
        let Composition { loaded, .. } =
            compose(&game, Side::IntegratedServer).map_err(|error| error.to_string())?;
        let machine = loaded.machine();
        for owner in [owner(), saved_owner(), server_owner(), registries_owner()] {
            assert_eq!(
                machine.state(&owner),
                Ok(LifecycleState::Running),
                "{owner}"
            );
        }
        assert_eq!(machine.state(&client_owner()), Ok(LifecycleState::Resolved));
        let commands: Vec<_> = machine
            .claimed_keys()
            .filter(|key| key.kind() == ravel_core::ContentKind::Command)
            .map(|key| key.id().to_string())
            .collect();
        if ravel_bootstrap_runtime::LEGACY_ADAPTER {
            assert_eq!(
                commands,
                [
                    "commands:server_probe",
                    "commands:warp",
                    "commands:waypoint"
                ]
            );
            let networking = "networking"
                .parse::<ModId>()
                .map_err(|error| error.to_string())?;
            assert_eq!(machine.state(&networking), Ok(LifecycleState::Running));
        } else {
            assert_eq!(commands, Vec::<String>::new());
        }
        Ok(())
    }

    #[test]
    fn the_server_only_member_is_admitted_and_parked_on_the_client() {
        let game = directory("client");
        let Composition { loaded, .. } =
            compose(&game, Side::Client).expect("the tracked composition should start");
        let machine = loaded.machine();

        assert_eq!(
            machine.state(&server_owner()),
            Ok(LifecycleState::Resolved),
            "a server-only mod started on the client"
        );
        assert_eq!(machine.state(&client_owner()), Ok(LifecycleState::Running));
        assert_eq!(machine.state(&owner()), Ok(LifecycleState::Running));
        assert!(
            machine
                .entries()
                .any(|entry| entry.mod_id == server_owner()),
            "a parked mod is installed rather than absent"
        );
    }

    #[test]
    fn the_shipped_pair_runs_on_opposite_sides_of_every_launch() {
        for side in Side::ALL {
            let game = directory(&format!("pair-{}", side.as_str()));
            let Composition { loaded, .. } =
                compose(&game, side).expect("the tracked composition should start");
            let machine = loaded.machine();

            assert_eq!(
                machine.state(&client_owner()) == Ok(LifecycleState::Running),
                side == Side::Client,
                "client_only on `{}`",
                side.as_str()
            );
            assert_eq!(
                machine.state(&server_owner()) == Ok(LifecycleState::Running),
                side.is_server(),
                "server_only on `{}`",
                side.as_str()
            );
        }
    }

    #[test]
    fn the_selected_composition_drives_every_mod_its_manifests_admit() {
        let game = directory("defaults");
        let Composition {
            loaded,
            configurations,
            ..
        } = compose(&game, Side::Client).expect("the tracked composition should start");
        let machine = loaded.machine();

        for id in [owner(), saved_owner(), client_owner(), registries_owner()] {
            assert_eq!(machine.state(&id), Ok(LifecycleState::Running), "{id}");
        }
        assert_eq!(
            machine.state(&server_owner()),
            Ok(LifecycleState::Resolved),
            "the server-only member is parked on the client"
        );

        let driven: Vec<&ModId> = machine.entries().map(|entry| &entry.mod_id).collect();
        assert_eq!(
            driven.len(),
            if ravel_bootstrap_runtime::LEGACY_ADAPTER {
                8
            } else {
                5
            }
        );
        for id in [
            owner(),
            saved_owner(),
            client_owner(),
            server_owner(),
            registries_owner(),
        ] {
            assert!(driven.contains(&&id), "{id}");
        }

        // Parked members do not contribute configuration declarations.
        let configured: Vec<&ModId> = configurations
            .iter()
            .map(LayeredConfiguration::owner)
            .collect();
        assert_eq!(configured, [&client_owner(), &owner(), &saved_owner()]);
    }

    #[test]
    fn legacy_audio_is_admitted_as_a_real_supplied_vanilla_event_alias() -> Result<(), String> {
        let game = directory("audio-resources");
        let composition = compose(&game, Side::Client).map_err(|error| error.to_string())?;
        let path = "assets/audio/sounds.json"
            .to_owned()
            .try_into()
            .map_err(|error: ravel_core::RelativePathError| error.to_string())?;
        if ravel_bootstrap_runtime::LEGACY_ADAPTER {
            assert_eq!(
                composition.resources.bytes(&path),
                Some(
                    include_bytes!("../../../test-mods/audio/assets/audio/sounds.json").as_slice()
                )
            );
            let audio = "audio"
                .parse()
                .map_err(|error: ravel_core::ModIdError| error.to_string())?;
            assert_eq!(
                composition.loaded.machine().state(&audio),
                Ok(LifecycleState::Running)
            );
        } else {
            assert!(composition.resources.bytes(&path).is_none());
        }
        Ok(())
    }

    #[test]
    fn both_server_compositions_supply_the_recipe_for_their_real_claimed_item() -> Result<(), String>
    {
        let path = "assets/registries/recipes/probe.json"
            .to_owned()
            .try_into()
            .map_err(|error: ravel_core::RelativePathError| error.to_string())?;
        for side in [Side::DedicatedServer, Side::IntegratedServer] {
            let composition =
                compose(&directory(side.as_str()), side).map_err(|error| error.to_string())?;
            assert_eq!(
                composition.resources.bytes(&path),
                Some(
                    include_bytes!(
                        "../../../test-mods/registries/assets/registries/recipes/probe.json"
                    )
                    .as_slice()
                )
            );
            assert!(
                composition
                    .loaded
                    .machine()
                    .claimed_keys()
                    .any(|key| key.kind() == ravel_core::ContentKind::Item
                        && key.id().to_string() == "registries:copper")
            );
        }
        Ok(())
    }

    #[test]
    fn every_shipped_manifest_names_the_value_and_the_versions_it_is_composed_with() {
        let manifests = [
            (CONFIGURATION_PACKAGE, owner()),
            (SAVED_DATA_PACKAGE, saved_owner()),
            (CLIENT_ONLY_PACKAGE, client_owner()),
            (SERVER_ONLY_PACKAGE, server_owner()),
        ];
        let running = ravel_minecraft_adapter::environment();
        for (document, expected) in manifests {
            let manifest =
                ravel_package::PackageManifest::parse(document).expect("a shipped manifest parses");
            assert_eq!(manifest.id, expected);
            manifest
                .compatibility
                .check_ravel(&running.ravel)
                .unwrap_or_else(|refused| panic!("{expected}: {refused}"));
            manifest
                .compatibility
                .check_minecraft(&running.minecraft)
                .unwrap_or_else(|refused| panic!("{expected}: {refused}"));
        }

        let saved = ravel_test_mod_saved_data::SavedDataMod::new();
        let declared: std::collections::BTreeSet<_> = saved
            .declaration()
            .expect("valid declaration")
            .versions()
            .collect();
        let manifest = ravel_package::PackageManifest::parse(SAVED_DATA_PACKAGE)
            .expect("a shipped manifest parses");
        assert_eq!(
            manifest
                .saved_data_versions
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            declared,
            "the manifest and the mod disagree about the versions it can read"
        );
    }

    // Independently computed fixture digests; OTHER_DIGEST matches no fixture.
    const INSTALLED_SOURCE: &[u8] = b"pub fn load() {}\n";
    const INSTALLED_RECORD: &[u8] = br#"{"origin":"example"}"#;
    const SOURCE_DIGEST: &str = "d43fb0c057531a1578825aa660a5cc1b415a10ae8eb1b42994ca7ceda27d6706";
    const RECORD_DIGEST: &str = "5cf5e6735c38fa43af69f9fe2210ca13d9fd75cc98cd9650ee969c0498a28ab0";
    const OTHER_DIGEST: &str = "a126bbfe4662a8d151f8e62afbb278a4a273e01fe0a88fc1545b25d017f19fb3";

    fn install(game: &std::path::Path, id: &str, source_digest: &str) {
        let running = ravel_minecraft_adapter::environment();
        let root = game
            .join(ravel_core::paths::INSTANCE_DIRECTORY)
            .join(ravel_core::paths::PACKAGES_DIRECTORY)
            .join(format!("{id}-1.0.0"));
        std::fs::create_dir_all(root.join("src")).expect("a package directory is creatable");
        std::fs::write(
            root.join(ravel_loader::MANIFEST_FILE),
            format!(
                r#"{{
                  "schemaVersion": 1,
                  "id": "{id}",
                  "version": "1.0.0",
                  "compatibility": {{
                    "ravel": ">={ravel}",
                    "minecraft": ["{minecraft}"]
                  }},
                  "sides": ["client"],
                  "authors": [{{ "name": "Ravel" }}],
                  "license": "MIT",
                  "source": {{
                    "repository": "https://example.invalid/{id}",
                    "files": [{{ "path": "src/lib.rs", "sha256": "{source_digest}" }}]
                  }},
                  "provenance": {{
                    "record": "provenance.json",
                    "sha256": "{RECORD_DIGEST}"
                  }}
                }}"#,
                ravel = running.ravel,
                minecraft = running.minecraft,
            ),
        )
        .expect("manifest written");
        std::fs::write(root.join("src/lib.rs"), INSTALLED_SOURCE).expect("source written");
        std::fs::write(root.join("provenance.json"), INSTALLED_RECORD).expect("record written");
    }

    #[test]
    fn a_package_installed_into_the_instance_that_this_build_cannot_run_refuses_the_launch() {
        let game = directory("installed-stranger");
        install(&game, "stranger_mod", SOURCE_DIGEST);

        let Err(refused) = compose(&game, Side::Client) else {
            panic!("a package no value was compiled from should refuse the launch");
        };
        let LaunchError::NotComposed(mod_id) = &refused else {
            panic!("the refusal should name the installed package: {refused:?}");
        };
        assert_eq!(mod_id.as_str(), "stranger_mod");
        assert!(refused.to_string().contains("stranger_mod"));
    }

    #[test]
    fn an_installed_package_whose_source_is_not_what_it_declares_refuses_the_launch() {
        let game = directory("installed-altered");
        install(&game, "client_only", OTHER_DIGEST);

        let Err(refused) = compose(&game, Side::Client) else {
            panic!("a package whose source was altered should refuse the launch");
        };
        assert!(
            matches!(
                refused,
                LaunchError::Discovery(DiscoveryRefused::Integrity { .. })
            ),
            "{refused:?}"
        );
    }

    #[test]
    fn an_installed_package_this_build_was_compiled_from_launches() {
        let game = directory("installed-known");
        install(&game, "client_only", SOURCE_DIGEST);

        let Composition { loaded, .. } =
            compose(&game, Side::Client).expect("the composition should start");
        let machine = loaded.machine();
        assert_eq!(machine.state(&client_owner()), Ok(LifecycleState::Running));
    }

    #[test]
    fn the_world_owning_mod_carries_the_versions_it_declares() {
        let game = directory("saved-versions");
        let Composition {
            loaded,
            installed_packages,
            ..
        } = compose(&game, Side::Client).expect("the tracked composition should start");
        let machine = loaded.machine();

        assert!(
            machine.order().any(|id| id == &saved_owner()),
            "the world-owning mod is part of the driven load order"
        );
        for package in super::members(Side::Client) {
            let manifest = ravel_package::PackageManifest::parse(&package.package)
                .expect("the member manifest");
            assert_eq!(
                installed_packages.get(&manifest.id),
                Some(&manifest.version),
                "the installed package version survives composition"
            );
        }
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
        let pack_path = file::pack_path(&game, &pack.owner);
        std::fs::create_dir_all(pack_path.parent().expect("the path has a parent"))
            .expect("the pack directory should be writable");
        std::fs::write(&pack_path, pack.to_json()).expect("the pack file should be writable");
        file::write(&game, &player).expect("the player file should be writable");

        let Composition { configurations, .. } =
            compose(&game, Side::Client).expect("the selected pack values should resolve");
        let resolved = settings_of(&configurations, &owner());

        assert_eq!(
            resolved.value("client.enabled"),
            Ok(&json!(true)),
            "allowed leaves the last word to the player"
        );
        assert_eq!(
            resolved.value("integrated.mob_limit"),
            Ok(&json!(40)),
            "locked leaves the last word to the pack"
        );
        assert_eq!(
            resolved.value("dedicated.motd"),
            Ok(&json!("Pack")),
            "required is supplied by and fixed to the pack"
        );
        assert_eq!(resolved.value("common.profile"), Ok(&json!("quiet")));
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

        let Composition { configurations, .. } =
            compose(&game, Side::Client).expect("the player values should resolve");

        assert_eq!(
            settings_of(&configurations, &owner()).value("client.enabled"),
            Ok(&json!(false))
        );
        assert_eq!(
            settings_of(&configurations, &owner()).value("common.profile"),
            Ok(&json!("verbose"))
        );
    }
}
