use std::collections::BTreeMap;
use std::fmt;

use ravel_config::{LayerError, LayeredConfiguration, Rejection, ValueDocument, file};
use ravel_core::{FailureEnvironment, FailureKind, LifecycleState, ModId, StructuredFailure};
use ravel_host_api::{HostError, Mod};

use crate::loaded::LoadedMods;

/// Resolves settings in load order for mods on this side that declare them.
///
/// Rejected player values fall back to lower layers and are reported in [`Configured`].
/// Callback failures are recorded; only required owners stop configuration.
/// The paired view has already refused missing hosts before any declarations or player reads.
/// Player documents are read only after the mod's declaration has passed the ownership check.
///
/// # Errors
///
/// Returns a [`ConfigRefused`] for foreign ownership, unreadable player documents,
/// invalid layers, or a mod's refusal.
pub fn open_configuration<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut LoadedMods<'_, H>,
    packs: &BTreeMap<ModId, ValueDocument>,
    mut players: impl FnMut(&ModId) -> Result<Option<ValueDocument>, file::FileError>,
    environment: &FailureEnvironment,
) -> Result<Configured, ConfigRefused> {
    let (machine, mods) = loaded.parts();
    let mut configured = Configured::default();

    let eligible: Vec<_> = machine
        .eligible()
        .map(|(entry, state)| (entry.mod_id.clone(), state))
        .collect();
    for (mod_id, state) in eligible {
        let instance = crate::loaded::instance_mut(mods, &mod_id);
        let declaration =
            match crate::callback(&mod_id, state, || instance.configuration_declaration()) {
                Ok(Some(declaration)) => declaration,
                Ok(None) => continue,
                Err(source) => {
                    record_failure(machine, environment, source)?;
                    continue;
                }
            };
        if declaration.owner() != &mod_id {
            let refused = ConfigRefused::ForeignDeclaration {
                mod_id: mod_id.clone(),
                owner: declaration.owner().clone(),
            };
            let error = crate::owner_refusal(machine, &mod_id, &refused)
                .map_err(ConfigRefused::Lifecycle)?;
            if machine
                .fail_setup_callback(&error, environment)
                .map_err(ConfigRefused::Lifecycle)?
            {
                return Err(refused);
            }
            continue;
        }

        let player = players(&mod_id).map_err(|source| ConfigRefused::PlayerFile {
            mod_id: mod_id.clone(),
            source,
        })?;
        let resolved =
            LayeredConfiguration::resolve(declaration, packs.get(&mod_id), player.as_ref())
                .map_err(|source| ConfigRefused::Layer {
                    mod_id: mod_id.clone(),
                    source,
                })?;
        for dropped in resolved.rejected() {
            configured.dropped.push((mod_id.clone(), dropped.clone()));
        }
        if let Err(source) = crate::callback(&mod_id, state, || instance.configure(&resolved)) {
            record_failure(machine, environment, source)?;
            continue;
        }
        configured.resolved.push(resolved);
        configured.order.push(mod_id);
    }

    Ok(configured)
}

fn record_failure(
    machine: &mut crate::LifecycleMachine,
    environment: &FailureEnvironment,
    source: HostError,
) -> Result<(), ConfigRefused> {
    if machine
        .fail_setup_callback(&source, environment)
        .map_err(ConfigRefused::Lifecycle)?
    {
        return Err(ConfigRefused::Mod {
            mod_id: source.mod_id().clone(),
            source,
        });
    }
    Ok(())
}

/// Which mods were configured, and what the player asked for and did not get.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Configured {
    order: Vec<ModId>,
    dropped: Vec<(ModId, Rejection)>,
    resolved: Vec<LayeredConfiguration>,
}

impl Configured {
    /// Returns every mod that was handed its settings, in the order it was.
    #[must_use]
    pub fn order(&self) -> &[ModId] {
        &self.order
    }

    /// Returns the settings each configured mod was handed, in the same order.
    #[must_use]
    pub fn resolved(&self) -> &[LayeredConfiguration] {
        &self.resolved
    }

    /// Takes the resolved settings without copying their schemas and values.
    #[must_use]
    pub fn into_resolved(self) -> Vec<LayeredConfiguration> {
        self.resolved
    }

    /// Returns dropped player values in load order, then key order within each mod.
    #[must_use]
    pub fn dropped(&self) -> &[(ModId, Rejection)] {
        &self.dropped
    }
}

/// Why a set's settings were not applied.
#[derive(Debug)]
pub enum ConfigRefused {
    /// The lifecycle refused failure attribution.
    Lifecycle(crate::LifecycleError),
    /// A player's settings document could not be read.
    PlayerFile {
        /// The mod whose document could not be read.
        mod_id: ModId,
        /// The file refusal.
        source: file::FileError,
    },
    /// A mod declared settings owned by another identity.
    ForeignDeclaration {
        /// The mod that declared them.
        mod_id: ModId,
        /// The identity the declaration names as its owner.
        owner: ModId,
    },
    /// A pack or a player document could not be layered onto the declaration.
    Layer {
        /// The mod whose settings they are.
        mod_id: ModId,
        /// What `ravel-config` refused.
        source: LayerError,
    },
    /// A mod refused the settings it was handed.
    Mod {
        /// The mod that refused.
        mod_id: ModId,
        /// What it said.
        source: HostError,
    },
}

impl ConfigRefused {
    /// Returns the mod this refusal is about.
    #[must_use]
    pub const fn mod_id(&self) -> Option<&ModId> {
        match self {
            Self::Lifecycle(_) => None,
            Self::PlayerFile { mod_id, .. }
            | Self::ForeignDeclaration { mod_id, .. }
            | Self::Layer { mod_id, .. }
            | Self::Mod { mod_id, .. } => Some(mod_id),
        }
    }
}

impl fmt::Display for ConfigRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lifecycle(error) => error.fmt(formatter),
            Self::PlayerFile { mod_id, source } => {
                write!(
                    formatter,
                    "`{mod_id}`'s player configuration cannot be read: {source}"
                )
            }
            Self::ForeignDeclaration { mod_id, owner } => write!(
                formatter,
                "`{mod_id}` declared settings owned by `{owner}`, and a mod declares only its own \
                 settings"
            ),
            Self::Layer { mod_id, source } => {
                write!(formatter, "`{mod_id}`'s settings did not layer: {source}")
            }
            Self::Mod { mod_id, source } => {
                write!(formatter, "`{mod_id}` refused its settings: {source}")
            }
        }
    }
}

impl std::error::Error for ConfigRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lifecycle(error) => Some(error),
            Self::PlayerFile { source, .. } => Some(source),
            Self::ForeignDeclaration { .. } => None,
            Self::Layer { source, .. } => Some(source),
            Self::Mod { source, .. } => Some(source),
        }
    }
}

impl StructuredFailure for ConfigRefused {
    fn failure_kind(&self) -> Option<FailureKind> {
        match self {
            Self::Lifecycle(error) => error.failure_kind(),
            Self::Mod { source, .. } => source.failure_kind(),
            Self::PlayerFile { .. } | Self::ForeignDeclaration { .. } | Self::Layer { .. } => None,
        }
    }

    fn lifecycle_state(&self) -> LifecycleState {
        match self {
            Self::Lifecycle(error) => error.lifecycle_state(),
            Self::Mod { source, .. } => source.lifecycle_state(),
            Self::PlayerFile { .. } | Self::ForeignDeclaration { .. } | Self::Layer { .. } => {
                LifecycleState::Discovered
            }
        }
    }

    fn quotes_untrusted_text(&self) -> bool {
        match self {
            Self::Lifecycle(error) => error.quotes_untrusted_text(),
            Self::Mod { source, .. } => source.quotes_untrusted_text(),
            Self::PlayerFile { .. } | Self::ForeignDeclaration { .. } | Self::Layer { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ravel_config::{ConfigurationDeclaration, LayeredConfiguration, ValueDocument};
    use ravel_core::{LifecycleState, ModId, Side, StructuredFailure};
    use ravel_host_api::{HostError, Mod};

    use super::ConfigRefused;
    use crate::lifecycle::{LifecycleEntry, LifecycleMachine};
    use crate::tests::entry;

    fn open_configuration(
        loaded: &mut crate::LoadedMods<'_, &mut dyn Mod>,
        packs: &BTreeMap<ModId, ValueDocument>,
        players: &BTreeMap<ModId, ValueDocument>,
    ) -> Result<super::Configured, ConfigRefused> {
        super::open_configuration(
            loaded,
            packs,
            |owner| Ok(players.get(owner).cloned()),
            &crate::tests::environment(),
        )
    }

    #[test]
    fn foreign_declarations_never_select_a_player_document() {
        let mut host = Settled::new("thief_mod").declaring_for("other_mod");
        let mut machine = machine(vec![entry("thief_mod", &[Side::DedicatedServer])]);
        let refused = super::open_configuration(
            &mut crate::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                .expect("complete host set"),
            &BTreeMap::new(),
            |_| panic!("foreign ownership must be refused before reading player files"),
            &crate::tests::environment(),
        )
        .expect_err("foreign declaration");
        assert!(matches!(refused, ConfigRefused::ForeignDeclaration { .. }));
    }

    #[test]
    fn unreadable_player_documents_stop_configuration() {
        let mut host = Settled::new("first_mod");
        let mut machine = machine(vec![entry("first_mod", &[Side::DedicatedServer])]);
        let refused = super::open_configuration(
            &mut crate::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                .expect("complete host set"),
            &BTreeMap::new(),
            |_| {
                Err(ravel_config::file::FileError::Io {
                    path: "settings.json".into(),
                    source: std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "unreadable settings",
                    ),
                })
            },
            &crate::tests::environment(),
        )
        .expect_err("unreadable player document");
        assert_eq!(refused.mod_id(), Some(&id("first_mod")));
        assert!(matches!(refused, ConfigRefused::PlayerFile { .. }));
        assert!(std::error::Error::source(&refused).is_some());
        assert_eq!(host.applied, None);
    }

    fn id(value: &str) -> ModId {
        value.parse().expect("a legal mod id")
    }

    fn declaration(owner: &str) -> ConfigurationDeclaration {
        ConfigurationDeclaration::parse(&format!(
            r#"{{
              "schemaVersion": 1,
              "owner": "{owner}",
              "version": 1,
              "settings": [
                {{
                  "key": "limit",
                  "valueSchema": {{ "type": "integer" }},
                  "default": 4,
                  "side": "common",
                  "restartRequired": false,
                  "packOverride": "allowed"
                }}
              ]
            }}"#
        ))
        .expect("a legal declaration")
    }

    fn document(owner: &str, value: &str) -> ValueDocument {
        ValueDocument::parse(&format!(
            r#"{{ "schemaVersion": 1, "owner": "{owner}", "values": {{ "limit": {value} }} }}"#
        ))
        .expect("a legal document")
    }

    struct Settled {
        id: ModId,
        declares: Option<ConfigurationDeclaration>,
        applied: Option<i64>,
        deaf: bool,
        refuses_declaration: bool,
    }

    impl Settled {
        fn new(name: &str) -> Self {
            Self {
                id: id(name),
                declares: Some(declaration(name)),
                applied: None,
                deaf: false,
                refuses_declaration: false,
            }
        }

        fn declaring_nothing(mut self) -> Self {
            self.declares = None;
            self
        }

        fn deaf(mut self) -> Self {
            self.deaf = true;
            self
        }

        fn declaring_for(mut self, other: &str) -> Self {
            self.declares = Some(declaration(other));
            self
        }
    }

    impl Mod for Settled {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), ravel_host_api::ModError> {
            Ok(())
        }

        fn configuration_declaration(
            &self,
        ) -> Result<Option<ConfigurationDeclaration>, ravel_host_api::ModError> {
            if self.refuses_declaration {
                return Err(ravel_host_api::ModError::Unsupported);
            }
            Ok(self.declares.clone())
        }

        fn configure(
            &mut self,
            resolved: &LayeredConfiguration,
        ) -> Result<(), ravel_host_api::ModError> {
            if self.deaf {
                return Err(ravel_host_api::ModError::Unsupported);
            }
            self.applied = resolved
                .value(&"limit".parse().expect("a legal key"))
                .and_then(serde_json::Value::as_i64);
            Ok(())
        }
    }

    fn machine(entries: Vec<LifecycleEntry>) -> LifecycleMachine {
        LifecycleMachine::new(Side::DedicatedServer, entries).expect("a legal plan")
    }

    #[test]
    fn configuration_declaration_refusal_stops_admission() {
        let mut host = Settled::new("refusing_mod");
        host.refuses_declaration = true;
        let mut machine = machine(vec![entry("refusing_mod", &[Side::DedicatedServer])]);
        let refused = open_configuration(
            &mut crate::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                .expect("complete host set"),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .expect_err("configuration declaration refusal");
        assert!(matches!(
            refused,
            ConfigRefused::Mod {
                mod_id,
                source: HostError::Unsupported {
                    mod_id: source_mod_id,
                    state: LifecycleState::Discovered,
                },
            } if mod_id == id("refusing_mod") && source_mod_id == id("refusing_mod")
        ));
        assert_eq!(host.applied, None);
    }

    #[test]
    fn failed_optional_owner_never_declares_or_selects_player_settings() {
        let mut host = Settled::new("failed_mod");
        host.refuses_declaration = true;
        let mut planned = entry("failed_mod", &[Side::DedicatedServer]);
        planned.requirement = crate::Requirement::Optional;
        let mut machine = machine(vec![planned]);
        let environment = ravel_core::FailureEnvironment {
            ravel: "0.1.0".parse().expect("valid version"),
            minecraft: "test".to_owned(),
            platform: "test".to_owned(),
            architecture: "test".to_owned(),
        };
        machine
            .fail_callback(
                &HostError::Unsupported {
                    mod_id: id("failed_mod"),
                    state: LifecycleState::Discovered,
                },
                &environment,
                None,
            )
            .expect("optional owner can fail");
        super::open_configuration(
            &mut crate::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                .expect("paired host"),
            &BTreeMap::new(),
            |_| panic!("a failed owner must not read player settings"),
            &crate::tests::environment(),
        )
        .expect("failed owners are skipped");
        assert_eq!(host.applied, None);
    }

    #[test]
    fn every_mod_that_declares_settings_is_handed_the_values_in_force() {
        let mut first = Settled::new("first_mod");
        let mut second = Settled::new("second_mod");
        let mut machine = machine(vec![
            entry("first_mod", &[Side::DedicatedServer]),
            entry("second_mod", &[Side::DedicatedServer]),
        ]);
        let packs = BTreeMap::from([(id("second_mod"), document("second_mod", "9"))]);

        let configured = open_configuration(
            &mut crate::LoadedMods::<&mut dyn Mod>::new(
                &mut machine,
                &mut [&mut first, &mut second],
            )
            .expect("complete host set"),
            &packs,
            &BTreeMap::new(),
        )
        .expect("both mods accept their settings");

        assert_eq!(configured.order(), [id("first_mod"), id("second_mod")]);
        assert_eq!(first.applied, Some(4), "the author's default");
        assert_eq!(second.applied, Some(9), "the pack's override");
        assert_eq!(configured.dropped(), []);
    }

    #[test]
    fn a_mod_that_declares_settings_and_never_reads_them_refuses_the_whole_set() {
        let mut deaf = Settled::new("deaf_mod").deaf();
        let mut later = Settled::new("later_mod");
        let mut machine = machine(vec![
            entry("deaf_mod", &[Side::DedicatedServer]),
            entry("later_mod", &[Side::DedicatedServer]),
        ]);

        let refused = open_configuration(
            &mut crate::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut deaf, &mut later])
                .expect("complete host set"),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .expect_err("settings nothing reads are settings with no effect");

        assert!(matches!(
            refused,
            ConfigRefused::Mod {
                ref mod_id,
                source: HostError::Unsupported { .. },
            } if mod_id == &id("deaf_mod")
        ));
        assert_eq!(refused.mod_id(), Some(&id("deaf_mod")));
        assert_eq!(
            later.applied, None,
            "the walk stopped rather than configuring the rest"
        );
    }

    #[test]
    fn an_optional_configuration_callback_failure_records_and_continues() {
        let mut deaf = Settled::new("deaf_mod").deaf();
        let mut later = Settled::new("later_mod");
        let mut optional = entry("deaf_mod", &[Side::DedicatedServer]);
        optional.requirement = crate::Requirement::Optional;
        let mut machine = machine(vec![optional, entry("later_mod", &[Side::DedicatedServer])]);
        let configured = open_configuration(
            &mut crate::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut deaf, &mut later])
                .expect("complete mod set"),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .expect("an optional failure does not refuse healthy configuration");
        assert_eq!(configured.order(), [id("later_mod")]);
        assert_eq!(later.applied, Some(4));
        assert_eq!(machine.state(&id("deaf_mod")), Ok(LifecycleState::Failed));
        let failure = machine
            .failure(&id("deaf_mod"))
            .expect("tracked")
            .expect("recorded");
        assert_eq!(failure.mod_id.as_ref(), Some(&id("deaf_mod")));
        assert_eq!(failure.environment, crate::tests::environment());
        assert!(machine.halted_by().is_none());
    }

    #[test]
    fn a_mod_that_declares_no_settings_is_not_asked_and_the_set_still_opens() {
        let mut silent = Settled::new("silent_mod").declaring_nothing();
        let mut machine = machine(vec![entry("silent_mod", &[Side::DedicatedServer])]);

        let configured = open_configuration(
            &mut crate::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut silent])
                .expect("complete host set"),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .expect("a mod with nothing to configure configures");

        assert_eq!(configured.order(), []);
        assert_eq!(silent.applied, None);
    }

    #[test]
    fn a_mod_this_side_parks_is_never_handed_settings() {
        let mut client = Settled::new("client_mod");
        let mut machine = machine(vec![entry("client_mod", &[Side::Client])]);

        let configured = open_configuration(
            &mut crate::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut client])
                .expect("complete host set"),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .expect("a parked mod takes no part");

        assert_eq!(configured.order(), []);
        assert_eq!(client.applied, None);
    }

    #[test]
    fn an_illegal_player_value_is_reported_and_the_layer_below_applies() {
        let mut host = Settled::new("first_mod");
        let mut machine = machine(vec![entry("first_mod", &[Side::DedicatedServer])]);
        let players = BTreeMap::from([(id("first_mod"), document("first_mod", "\"twelve\""))]);

        let configured = open_configuration(
            &mut crate::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                .expect("complete host set"),
            &BTreeMap::new(),
            &players,
        )
        .expect("a dropped value is not a refusal");

        assert_eq!(host.applied, Some(4), "the author's default still applies");
        let dropped = configured.dropped();
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].0, id("first_mod"));
        assert_eq!(dropped[0].1.key.as_str(), "limit");
    }

    #[test]
    fn a_mod_declaring_another_mods_settings_is_refused_before_anything_is_layered() {
        let mut thief = Settled::new("thief_mod").declaring_for("first_mod");
        let mut machine = machine(vec![entry("thief_mod", &[Side::DedicatedServer])]);

        let refused = open_configuration(
            &mut crate::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut thief])
                .expect("complete host set"),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .expect_err("a mod declares only its own settings");

        assert!(matches!(
            &refused,
            ConfigRefused::ForeignDeclaration { mod_id, owner }
                if mod_id == &id("thief_mod") && owner == &id("first_mod")
        ));
        let text = refused.to_string();
        assert!(
            text.contains("thief_mod") && text.contains("first_mod"),
            "{text}"
        );
        assert_eq!(thief.applied, None);
    }

    #[test]
    fn a_pack_document_written_for_another_mod_refuses_the_set() {
        let mut host = Settled::new("first_mod");
        let mut machine = machine(vec![entry("first_mod", &[Side::DedicatedServer])]);
        let packs = BTreeMap::from([(id("first_mod"), document("second_mod", "9"))]);

        let refused = open_configuration(
            &mut crate::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                .expect("complete host set"),
            &packs,
            &BTreeMap::new(),
        )
        .expect_err("a document belongs to one mod");

        assert!(matches!(refused, ConfigRefused::Layer { .. }));
        assert!(std::error::Error::source(&refused).is_some());
        assert!(!refused.quotes_untrusted_text());
        assert_eq!(host.applied, None);
    }
}
