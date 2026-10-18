#![forbid(unsafe_code)]

//! Configuration schema and generated-UI test mod.

use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration, SettingKey};
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod};

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "configuration";

/// The declaration packaged with this mod.
pub const DECLARATION: &str = include_str!("../configuration.json");

const ENABLED: &str = "client.enabled";
const PROFILE: &str = "common.profile";

/// A mod whose running behavior records the effective configuration it received.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigurationMod {
    enabled: bool,
    profile: String,
    observations: Vec<String>,
}

impl ConfigurationMod {
    /// Builds the mod with its author defaults.
    ///
    /// # Panics
    ///
    /// Panics if the packaged identity or declaration stops being valid.
    #[must_use]
    pub fn new() -> Self {
        let declaration = Self::declaration();
        let configuration = LayeredConfiguration::resolve(declaration, None, None)
            .expect("the author defaults resolve");
        let mut value = Self {
            enabled: false,
            profile: String::new(),
            observations: Vec::new(),
        };
        value
            .apply(&configuration)
            .expect("the packaged configuration belongs to this mod");
        value
    }

    /// Parses the declaration Ravel installs for this mod.
    ///
    /// # Panics
    ///
    /// Panics if the tracked declaration stops matching its schema.
    #[must_use]
    pub fn declaration() -> ConfigurationDeclaration {
        ConfigurationDeclaration::parse(DECLARATION)
            .expect("the packaged configuration declaration is valid")
    }

    /// Applies one resolved configuration to subsequent behavior.
    ///
    /// # Errors
    ///
    /// Returns a refused [`ModError`] for a different owner or missing typed settings.
    pub fn apply(&mut self, configuration: &LayeredConfiguration) -> Result<(), ModError> {
        if configuration.owner() != Self::declared_id() {
            return Err(ModError::refused(format!(
                "configuration belongs to `{}`, not `{}`",
                configuration.owner(),
                Self::declared_id()
            )));
        }
        self.enabled = configuration
            .boolean(&key(ENABLED))
            .map_err(ModError::from_display)?;
        configuration
            .text(&key(PROFILE))
            .map_err(ModError::from_display)?
            .clone_into(&mut self.profile);
        Ok(())
    }

    /// Returns the behavior observed each time the mod entered `running`.
    #[must_use]
    pub fn observations(&self) -> &[String] {
        &self.observations
    }

    fn behavior(&self) -> String {
        format!(
            "{}:{}",
            if self.enabled { "enabled" } else { "disabled" },
            self.profile
        )
    }
}

impl Default for ConfigurationMod {
    fn default() -> Self {
        Self::new()
    }
}

#[declare_mod(id = "configuration")]
impl Mod for ConfigurationMod {
    /// Records the effective behavior when the mod starts running.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        if state == LifecycleState::Running {
            self.observations.push(self.behavior());
        }
        Ok(())
    }

    /// Declares the settings this mod owns.
    fn configuration_declaration(&self) -> Result<Option<ConfigurationDeclaration>, ModError> {
        ConfigurationDeclaration::parse(DECLARATION)
            .map(Some)
            .map_err(ModError::from_display)
    }

    /// Applies the settings in force for this launch.
    ///
    /// # Errors
    ///
    /// Returns a [`ModError`] when [`ConfigurationMod::apply`] fails.
    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
        Self::apply(self, resolved)
    }
}

fn key(value: &str) -> SettingKey {
    value.parse().expect("a declared setting key")
}

#[cfg(test)]
mod tests {
    use ravel_sdk::config::{
        ConfigurationDeclaration, LayeredConfiguration, PackOverride, SettingSide, ValueDocument,
    };
    use ravel_sdk::{JsonValue, LifecycleState, Mod, ModId, Side, json};

    use super::{ConfigurationMod, MOD_ID, key};

    fn document(values: &JsonValue) -> ValueDocument {
        ValueDocument::parse(
            &json!({
                "schemaVersion": 1,
                "owner": MOD_ID,
                "values": values
            })
            .to_string(),
        )
        .expect("a valid value document")
    }

    #[test]
    fn the_mod_is_packaged_under_a_valid_declaration_for_its_identity() {
        let configuration = ConfigurationMod::new();
        let declaration = ConfigurationMod::declaration();

        assert_eq!(configuration.id(), &MOD_ID.parse::<ModId>().expect("an id"));
        assert_eq!(declaration.owner(), configuration.id());
        assert_eq!(declaration.settings().len(), 4);
        assert_eq!(ConfigurationMod::default(), configuration);
    }

    #[test]
    fn the_declaration_covers_every_side_and_override_policy() {
        let declaration = ConfigurationMod::declaration();
        let sides = declaration
            .settings()
            .iter()
            .map(|setting| setting.side)
            .collect::<Vec<_>>();
        let policies = declaration
            .settings()
            .iter()
            .map(|setting| setting.pack_override)
            .collect::<Vec<_>>();

        assert_eq!(
            sides,
            [
                SettingSide::Client,
                SettingSide::IntegratedServer,
                SettingSide::DedicatedServer,
                SettingSide::Common,
            ]
        );
        assert_eq!(
            policies,
            [
                PackOverride::Allowed,
                PackOverride::Locked,
                PackOverride::Required,
                PackOverride::Allowed,
            ]
        );
        assert!(
            declaration
                .settings()
                .last()
                .expect("common setting")
                .restart_required
        );
    }

    #[test]
    fn each_process_side_sees_its_own_setting_and_the_common_one() {
        let configuration =
            LayeredConfiguration::resolve(ConfigurationMod::declaration(), None, None)
                .expect("defaults resolve");

        for (side, specific) in [
            (Side::Client, "client.enabled"),
            (Side::IntegratedServer, "integrated.mob_limit"),
            (Side::DedicatedServer, "dedicated.motd"),
        ] {
            let keys = configuration
                .settings_on(side)
                .iter()
                .map(|resolved| resolved.setting.key.as_str())
                .collect::<Vec<_>>();
            assert_eq!(keys, [specific, "common.profile"]);
        }
    }

    #[test]
    fn invalid_values_are_rejected_before_the_mod_observes_them() {
        let player = document(&json!({ "client.enabled": "yes" }));
        let configuration =
            LayeredConfiguration::resolve(ConfigurationMod::declaration(), None, Some(&player))
                .expect("an invalid player value falls back");

        assert_eq!(configuration.rejected().len(), 1);
        assert!(
            configuration
                .boolean(&key("client.enabled"))
                .expect("the setting is a boolean")
        );
    }

    #[test]
    fn effective_changes_produce_different_running_behavior() {
        let player = document(&json!({
            "client.enabled": false,
            "common.profile": "verbose"
        }));
        let configuration =
            LayeredConfiguration::resolve(ConfigurationMod::declaration(), None, Some(&player))
                .expect("player values resolve");
        let mut configured = ConfigurationMod::new();
        configured.apply(&configuration).expect("same owner");
        configured
            .enter(LifecycleState::Running)
            .expect("configuration mod runs");

        assert_eq!(configured.observations(), ["disabled:verbose"]);
    }

    #[test]
    fn a_configuration_for_another_owner_is_refused() {
        let declaration = ConfigurationDeclaration::parse(
            r#"{
              "schemaVersion": 1,
              "owner": "other_mod",
              "version": 1,
              "settings": []
            }"#,
        )
        .expect("valid declaration");
        let configuration = LayeredConfiguration::resolve(declaration, None, None)
            .expect("empty declaration resolves");

        let error = ConfigurationMod::new()
            .apply(&configuration)
            .expect_err("the owner differs");
        assert!(error.to_string().contains("other_mod"), "{error}");
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(ConfigurationMod::new());
