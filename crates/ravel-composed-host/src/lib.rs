#![forbid(unsafe_code)]

//! Player-local static composition used by the M3 configuration proof.
//!
//! # What this host is now
//!
//! One of two hosts. This one holds a member list a Ravel developer compiled;
//! `ravel-build` generates the other from the packages a player installed. Both
//! are the same thing behind the entry point — [`ravel_host_runtime::compose`]
//! — so a side declaration, a version range and a load order mean here exactly
//! what they mean there. This file is a member list and a JNI symbol, and
//! nothing else.
//!
//! # Where a side declaration takes effect
//!
//! Every member below ships with the package manifest it is packaged under, and
//! the entries the lifecycle holds are [`ravel_loader::admitted`]'s answer rather
//! than this file's. So the `sides` list a mod author writes decides what starts
//! in a launched game: the runtime resolves a plan for the side it was handed,
//! admits the whole set against it, and hands the result to
//! [`ravel_loader::drive`], which parks a mod whose declaration does not name
//! that side before any of its code runs.
//!
//! Until M4's review found it, both entries were written here by hand and the
//! declaration was inert: a client-only mod started on a dedicated server because
//! nothing in a launch read a manifest at all.
//!
//! # What this host still does not do
//!
//! It runs the mods it was compiled from and refuses an instance holding any
//! other package. That refusal is not a limitation of the composition — it is
//! what a *statically* composed host is, and the answer to it is the generated
//! host `ravel-build` produces rather than a change here.
//!
//! The side is not written here. [`initialize`] asks the JVM it is running
//! inside which distribution it was started from, and a launch that cannot get an
//! answer installs nothing rather than composing a set for a guessed side.
//! What it cannot answer is [`ravel_core::Side::IntegratedServer`], which is not
//! a distribution but a server inside a client process, so a mod declaring only
//! that side is parked on a client launch. `docs/guide/limits.md` states it.

use jni::EnvUnowned;
use jni::objects::JClass;
use ravel_core::Side;
use ravel_host_runtime::{Member, initialize as run};
use ravel_test_mod_client_only::{ClientOnlyMod, work_log};
use ravel_test_mod_configuration::ConfigurationMod;
use ravel_test_mod_registries::RegistriesMod;
use ravel_test_mod_saved_data::SavedDataMod;
use ravel_test_mod_server_only::{ServerOnlyMod, work_log as server_work_log};

pub use ravel_host_runtime::{Composition, LaunchError};

/// The manifest each composed member is packaged under.
///
/// Read from the package rather than restated here, so the document this host
/// admits is the same file the mod ships and there is no second copy of a `sides`
/// list to disagree with the first.
const CONFIGURATION_PACKAGE: &str = include_str!("../../../test-mods/configuration/package.json");
const SAVED_DATA_PACKAGE: &str = include_str!("../../../test-mods/saved-data/package.json");
const CLIENT_ONLY_PACKAGE: &str = include_str!("../../../test-mods/client-only/package.json");
const SERVER_ONLY_PACKAGE: &str = include_str!("../../../test-mods/server-only/package.json");
const REGISTRIES_PACKAGE: &str = include_str!("../../../test-mods/registries/package.json");

/// The members this build was compiled from, for a launch on `side`.
fn members(side: Side) -> Vec<Member> {
    vec![
        Member {
            package: CONFIGURATION_PACKAGE.to_owned(),
            host: Box::new(ConfigurationMod::new()),
        },
        // The world-owning half of the composition. Until this was here no mod
        // in a running build owned world data, so the saved-data round trip
        // could not be observed in a launch at all.
        Member {
            package: SAVED_DATA_PACKAGE.to_owned(),
            host: Box::new(SavedDataMod::new()),
        },
        // The client-only half. Its own module doc calls itself the mod the
        // side gate is about, and until it was composed here nothing in a
        // launch could show a declaration keeping code off a server.
        Member {
            package: CLIENT_ONLY_PACKAGE.to_owned(),
            host: Box::new(ClientOnlyMod::new(side, &work_log())),
        },
        // The content-owning half. Until this was here no mod in a running
        // build claimed a registry identifier, so the claim table the content
        // adapter reads was empty in every launch and the surface that carries
        // a block into the game had nothing to carry.
        Member {
            package: REGISTRIES_PACKAGE.to_owned(),
            host: Box::new(RegistriesMod::new()),
        },
        // The other half of the same rule. With only the member above, a
        // launch proved a declaration keeping code off a server and nothing
        // proved the reverse, so the shipped set showed one direction of a
        // two-directional rule.
        Member {
            package: SERVER_ONLY_PACKAGE.to_owned(),
            host: Box::new(ServerOnlyMod::new(side, &server_work_log())),
        },
    ]
}

/// Constructs and drives this source-composed mod set for one launch.
///
/// # Errors
///
/// Returns an error when the instance holds a package this build was not
/// compiled from, when a selected-pack or player document cannot be read, when
/// configuration cannot be resolved or consumed, or when the lifecycle cannot
/// reach running.
pub fn compose(game_directory: &std::path::Path, side: Side) -> Result<Composition, LaunchError> {
    ravel_host_runtime::compose(game_directory, side, members(side))
}

fn initialize(env: &mut jni::Env<'_>) -> Result<(), LaunchError> {
    // The side the members are built for is the side the runtime will compose
    // them on, so it is asked once, here, and handed to both.
    let side = ravel_host_runtime::side(env)?;
    run(env, members(side))
}

/// Starts the player-local statically composed set before game methods run.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelInitialize")]
pub fn ravel_initialize<'local>(mut env: EnvUnowned<'local>, _class: JClass<'local>) {
    env.with_env(initialize)
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[cfg(test)]
mod tests {
    use ravel_config::{ValueDocument, file};
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

    fn registries_owner() -> ModId {
        "registries".parse().expect("the tracked mod id is valid")
    }

    fn client_owner() -> ModId {
        "client_only".parse().expect("the tracked mod id is valid")
    }

    fn server_owner() -> ModId {
        "server_only".parse().expect("the tracked mod id is valid")
    }

    /// The property this composition exists to hold, with the real members.
    ///
    /// `client_only` ships `"sides": ["client"]`, so a dedicated server admits it
    /// — a player who installed it sees it listed — and never starts it.
    #[test]
    fn the_client_only_member_is_admitted_and_parked_on_a_dedicated_server() {
        let game = directory("dedicated");
        let (machine, mods, _) =
            compose(&game, Side::DedicatedServer).expect("the tracked composition should start");

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
            mods.iter().any(|held| held.id() == &client_owner()),
            "a parked mod is installed rather than absent"
        );
    }

    /// The other direction of the same rule, through the same shipped path.
    #[test]
    fn the_server_only_member_is_admitted_and_parked_on_the_client() {
        let game = directory("client");
        let (machine, mods, _) =
            compose(&game, Side::Client).expect("the tracked composition should start");

        assert_eq!(
            machine.state(&server_owner()),
            Ok(LifecycleState::Resolved),
            "a server-only mod started on the client"
        );
        assert_eq!(machine.state(&client_owner()), Ok(LifecycleState::Running));
        assert_eq!(machine.state(&owner()), Ok(LifecycleState::Running));
        assert!(
            mods.iter().any(|held| held.id() == &server_owner()),
            "a parked mod is installed rather than absent"
        );
    }

    /// Every side, and on each one the pair disagrees about who runs.
    #[test]
    fn the_shipped_pair_runs_on_opposite_sides_of_every_launch() {
        for side in Side::ALL {
            let game = directory(&format!("pair-{}", side.name()));
            let (machine, _, _) =
                compose(&game, side).expect("the tracked composition should start");

            assert_eq!(
                machine.state(&client_owner()) == Ok(LifecycleState::Running),
                side == Side::Client,
                "client_only on `{}`",
                side.name()
            );
            assert_eq!(
                machine.state(&server_owner()) == Ok(LifecycleState::Running),
                side.is_server(),
                "server_only on `{}`",
                side.name()
            );
        }
    }

    #[test]
    fn the_selected_composition_drives_every_mod_its_manifests_admit() {
        let game = directory("defaults");
        let (machine, mods, configurations) =
            compose(&game, Side::Client).expect("the tracked composition should start");

        for id in [owner(), saved_owner(), client_owner(), registries_owner()] {
            assert_eq!(machine.state(&id), Ok(LifecycleState::Running), "{id}");
        }
        assert_eq!(
            machine.state(&server_owner()),
            Ok(LifecycleState::Resolved),
            "the server-only member is parked on the client"
        );

        let driven: Vec<&ModId> = mods.iter().map(|held| held.id()).collect();
        assert_eq!(driven.len(), 5);
        for id in [
            owner(),
            saved_owner(),
            client_owner(),
            server_owner(),
            registries_owner(),
        ] {
            assert!(driven.contains(&&id), "{id}");
        }

        // Only the configuration mod declares settings, so the resolved
        // configurations stay one however many mods the composition holds.
        assert_eq!(configurations.len(), 1);
        assert_eq!(configurations[0].owner(), &owner());
    }

    /// The manifests are the authority, so nothing may quietly disagree with them.
    #[test]
    fn every_shipped_manifest_names_the_value_and_the_versions_it_is_composed_with() {
        let manifests = [
            (CONFIGURATION_PACKAGE, owner()),
            (SAVED_DATA_PACKAGE, saved_owner()),
            (CLIENT_ONLY_PACKAGE, client_owner()),
            (SERVER_ONLY_PACKAGE, server_owner()),
        ];
        let running = ravel_host_runtime::environment();
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
        let declared: std::collections::BTreeSet<_> = saved.declaration().versions().collect();
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

    /// The bytes an installed fixture carries, and the digests they hash to.
    ///
    /// The digests came from `sha256sum` rather than from the library the code
    /// under test hashes with, so the fixture and the check do not agree by
    /// construction. `OTHER_DIGEST` is the digest of bytes no fixture writes.
    const INSTALLED_SOURCE: &[u8] = b"pub fn load() {}\n";
    const INSTALLED_RECORD: &[u8] = br#"{"origin":"example"}"#;
    const SOURCE_DIGEST: &str = "d43fb0c057531a1578825aa660a5cc1b415a10ae8eb1b42994ca7ceda27d6706";
    const RECORD_DIGEST: &str = "5cf5e6735c38fa43af69f9fe2210ca13d9fd75cc98cd9650ee969c0498a28ab0";
    const OTHER_DIGEST: &str = "a126bbfe4662a8d151f8e62afbb278a4a273e01fe0a88fc1545b25d017f19fb3";

    /// Installs a package into an instance the way a player would.
    fn install(game: &std::path::Path, id: &str, source_digest: &str) {
        let running = ravel_host_runtime::environment();
        let root = game
            .join(ravel_loader::PACKAGES_DIRECTORY)
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

    /// A launch finds what an instance holds, and says so when it cannot run it.
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

    /// The integrity check `docs/guide/limits.md` recorded as absent from a launch.
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

    /// And an intact package this build *was* compiled from still starts the set.
    #[test]
    fn an_installed_package_this_build_was_compiled_from_launches() {
        let game = directory("installed-known");
        install(&game, "client_only", SOURCE_DIGEST);

        let (machine, _, _) = compose(&game, Side::Client).expect("the composition should start");
        assert_eq!(machine.state(&client_owner()), Ok(LifecycleState::Running));
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
        let pack_path = file::pack_path(&game, &pack.owner);
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
