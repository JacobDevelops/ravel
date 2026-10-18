use std::path::PathBuf;

use ravel_bootstrap::compose;
use ravel_config::{EditOutcome, LayeredConfiguration, ValueDocument, ValueSource, file};
use ravel_core::{LifecycleState, Side};
use ravel_test_mod_configuration::ConfigurationMod;
use serde_json::{Value, json};

fn document(values: &Value) -> ValueDocument {
    ValueDocument::parse(
        &json!({
            "schemaVersion": 1,
            "owner": "configuration",
            "values": values,
        })
        .to_string(),
    )
    .expect("a valid value document")
}

fn key(value: &str) -> ravel_config::SettingKey {
    value.parse().expect("a valid setting key")
}

fn directory(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "ravel-configuration-conformance-{}-{name}",
        std::process::id()
    ));
    if directory.exists() {
        std::fs::remove_dir_all(&directory).expect("a stale directory is removable");
    }
    std::fs::create_dir_all(&directory).expect("a writable temporary directory");
    directory
}

#[test]
fn the_tracked_declaration_enforces_sides_validation_and_pack_policy() {
    let pack = document(&json!({
        "client.enabled": false,
        "integrated.mob_limit": 40,
        "dedicated.motd": "Pack",
        "common.profile": "verbose",
    }));
    let player = document(&json!({
        "client.enabled": true,
        "integrated.mob_limit": 10,
        "dedicated.motd": "Player",
        "common.profile": "quiet",
    }));
    let resolved =
        LayeredConfiguration::resolve(ConfigurationMod::declaration(), Some(&pack), Some(&player))
            .expect("the complete pack document resolves");

    assert_eq!(resolved.value(&key("client.enabled")), Some(&json!(true)));
    assert_eq!(
        resolved.source(&key("client.enabled")),
        Some(ValueSource::Player)
    );
    assert_eq!(
        resolved.value(&key("integrated.mob_limit")),
        Some(&json!(40))
    );
    assert_eq!(
        resolved.source(&key("integrated.mob_limit")),
        Some(ValueSource::Pack)
    );
    assert_eq!(resolved.value(&key("dedicated.motd")), Some(&json!("Pack")));
    assert_eq!(
        resolved.source(&key("dedicated.motd")),
        Some(ValueSource::Pack)
    );
    assert_eq!(
        resolved.value(&key("common.profile")),
        Some(&json!("quiet"))
    );

    for (side, specific) in [
        (Side::Client, "client.enabled"),
        (Side::IntegratedServer, "integrated.mob_limit"),
        (Side::DedicatedServer, "dedicated.motd"),
    ] {
        let visible = resolved
            .settings_on(side)
            .into_iter()
            .map(|setting| setting.setting.key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(visible, [specific, "common.profile"]);
    }

    let invalid = document(&json!({ "client.enabled": "yes" }));
    let fallback =
        LayeredConfiguration::resolve(ConfigurationMod::declaration(), None, Some(&invalid))
            .expect("an invalid player value falls back to its default");
    assert_eq!(fallback.value(&key("client.enabled")), Some(&json!(true)));
    assert_eq!(fallback.rejected().len(), 1);

    let incomplete_pack = document(&json!({ "integrated.mob_limit": 40 }));
    assert!(
        LayeredConfiguration::resolve(
            ConfigurationMod::declaration(),
            Some(&incomplete_pack),
            None,
        )
        .is_err(),
        "a present pack must state the required dedicated-server value"
    );
}

#[test]
fn file_edits_take_effect_now_or_after_relaunch_as_declared() {
    let game = directory("file-round-trip");
    let mut resolved = LayeredConfiguration::resolve(ConfigurationMod::declaration(), None, None)
        .expect("author defaults resolve");

    assert_eq!(
        resolved.set(&key("client.enabled"), json!(false), Side::Client),
        Ok(EditOutcome::Now)
    );
    assert_eq!(resolved.value(&key("client.enabled")), Some(&json!(false)));
    assert_eq!(
        resolved.set(&key("common.profile"), json!("verbose"), Side::Client),
        Ok(EditOutcome::NextLaunch)
    );
    assert_eq!(
        resolved.value(&key("common.profile")),
        Some(&json!("quiet"))
    );
    assert!(resolved.restart_pending());

    file::write(&game, &resolved.player_document()).expect("the player document is persisted");
    let reread = file::read(&game, resolved.owner())
        .expect("the player document is readable")
        .expect("the player document exists");
    let relaunched =
        LayeredConfiguration::resolve(ConfigurationMod::declaration(), None, Some(&reread))
            .expect("the persisted player document resolves on relaunch");

    assert_eq!(
        relaunched.value(&key("client.enabled")),
        Some(&json!(false))
    );
    assert_eq!(
        relaunched.value(&key("common.profile")),
        Some(&json!("verbose"))
    );
    assert!(!relaunched.restart_pending());
}

#[test]
fn production_composition_reads_pack_and_player_files_and_drives_the_real_mod() {
    let game = directory("production-compose");
    let pack = document(&json!({
        "client.enabled": true,
        "integrated.mob_limit": 40,
        "dedicated.motd": "Pack",
    }));
    let pack_path = file::pack_path(&game, &pack.owner);
    std::fs::create_dir_all(pack_path.parent().expect("the pack path has a parent"))
        .expect("the pack directory is writable");
    std::fs::write(&pack_path, pack.to_json()).expect("the launch pack file is writable");
    file::write(
        &game,
        &document(&json!({
            "client.enabled": false,
            "integrated.mob_limit": 10,
            "dedicated.motd": "Player",
            "common.profile": "verbose",
        })),
    )
    .expect("the launch player file is writable");

    let ravel_bootstrap_runtime::Composition {
        machine,
        mods,
        configurations,
        ..
    } = compose(&game, Side::Client).expect("the production source composition starts");
    let owner = "configuration"
        .parse()
        .expect("the tracked mod id is valid");

    assert_eq!(machine.state(&owner), Ok(LifecycleState::Running));
    assert!(
        mods.iter().any(|host| host.id() == &owner),
        "the production composition did not drive the configuration mod"
    );
    let resolved = configurations
        .iter()
        .find(|resolved| resolved.owner() == &owner)
        .expect("the configuration mod's settings resolved");
    assert_eq!(resolved.value(&key("client.enabled")), Some(&json!(false)));
    assert_eq!(
        resolved.value(&key("integrated.mob_limit")),
        Some(&json!(40)),
        "the selected pack's locked value wins"
    );
    assert_eq!(
        resolved.value(&key("dedicated.motd")),
        Some(&json!("Pack")),
        "the selected pack supplies the required value"
    );
    assert_eq!(
        resolved.value(&key("common.profile")),
        Some(&json!("verbose"))
    );
}
