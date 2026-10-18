use std::collections::BTreeMap;
use std::fmt;

use ravel_config::{LayerError, LayeredConfiguration, Rejection, ValueDocument, file};
use ravel_core::{FailureKind, LifecycleState, ModId, Side, StructuredFailure};
use ravel_host_api::{HostError, HostMod};

use crate::lifecycle::LifecycleEntry;

/// Resolves settings in load order for mods on this side that declare them.
///
/// Rejected player values fall back to lower layers and are reported in [`Configured`].
/// A refusal stops configuration; the caller must not start the set.
/// Entries must be supplied in admitted load order. Player documents are read only
/// after the mod's single declaration has passed the ownership check.
///
/// # Errors
///
/// Returns a [`ConfigRefused`] for foreign ownership, unreadable player documents,
/// invalid layers, or a mod's refusal.
pub fn open_configuration<'a>(
    side: Side,
    entries: impl IntoIterator<Item = &'a LifecycleEntry>,
    mods: &mut [&mut (dyn HostMod + '_)],
    packs: &BTreeMap<ModId, ValueDocument>,
    mut players: impl FnMut(&ModId) -> Result<Option<ValueDocument>, file::FileError>,
) -> Result<Configured, ConfigRefused> {
    let mut configured = Configured::default();

    for entry in entries {
        let mod_id = entry.mod_id.clone();
        if !entry.sides.loads_on(side) {
            continue;
        }
        let Some(host) = mods.iter_mut().find(|host| host.id() == &mod_id) else {
            continue;
        };
        let Some(declaration) = crate::callback(&mod_id, LifecycleState::Constructed, || {
            host.configuration_declaration()
        })
        .map_err(|source| ConfigRefused::Mod {
            mod_id: mod_id.clone(),
            source,
        })?
        else {
            continue;
        };
        if declaration.owner() != &mod_id {
            return Err(ConfigRefused::ForeignDeclaration {
                mod_id,
                owner: declaration.owner().clone(),
            });
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
        crate::callback(&mod_id, LifecycleState::Constructed, || {
            host.configure(&resolved)
        })
        .map_err(|source| ConfigRefused::Mod {
            mod_id: mod_id.clone(),
            source,
        })?;
        configured.resolved.push(resolved);
        configured.order.push(mod_id);
    }

    Ok(configured)
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
    pub const fn owner(&self) -> &ModId {
        match self {
            Self::PlayerFile { mod_id, .. }
            | Self::ForeignDeclaration { mod_id, .. }
            | Self::Layer { mod_id, .. }
            | Self::Mod { mod_id, .. } => mod_id,
        }
    }
}

impl fmt::Display for ConfigRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::PlayerFile { source, .. } => Some(source),
            Self::ForeignDeclaration { .. } => None,
            Self::Layer { source, .. } => Some(source),
            Self::Mod { source, .. } => Some(source),
        }
    }
}

impl StructuredFailure for ConfigRefused {
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Constructed
    }

    fn quotes_untrusted_text(&self) -> bool {
        matches!(self, Self::Mod { .. })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ravel_config::{ConfigurationDeclaration, LayeredConfiguration, ValueDocument};
    use ravel_core::{LifecycleState, ModId, Side, StructuredFailure};
    use ravel_host_api::{HostError, HostMod};

    use super::ConfigRefused;
    use crate::lifecycle::{LifecycleEntry, LifecycleMachine};
    use crate::tests::entry;

    fn open_configuration(
        machine: &LifecycleMachine,
        mods: &mut [&mut (dyn HostMod + '_)],
        packs: &BTreeMap<ModId, ValueDocument>,
        players: &BTreeMap<ModId, ValueDocument>,
    ) -> Result<super::Configured, ConfigRefused> {
        super::open_configuration(machine.side(), machine.entries(), mods, packs, |owner| {
            Ok(players.get(owner).cloned())
        })
    }

    #[test]
    fn foreign_declarations_never_select_a_player_document() {
        let mut host = Settled::new("thief_mod").declaring_for("other_mod");
        let entries = [entry("thief_mod", &[Side::DedicatedServer])];
        let refused = super::open_configuration(
            Side::DedicatedServer,
            &entries,
            &mut [&mut host],
            &BTreeMap::new(),
            |_| panic!("foreign ownership must be refused before reading player files"),
        )
        .expect_err("foreign declaration");
        assert!(matches!(refused, ConfigRefused::ForeignDeclaration { .. }));
    }

    #[test]
    fn unreadable_player_documents_stop_configuration() {
        let mut host = Settled::new("first_mod");
        let entries = [entry("first_mod", &[Side::DedicatedServer])];
        let refused = super::open_configuration(
            Side::DedicatedServer,
            &entries,
            &mut [&mut host],
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
        )
        .expect_err("unreadable player document");
        assert_eq!(refused.owner(), &id("first_mod"));
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

    impl HostMod for Settled {
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
        let machine = machine(vec![entry("refusing_mod", &[Side::DedicatedServer])]);
        let refused = open_configuration(
            &machine,
            &mut [&mut host],
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
                    state: LifecycleState::Constructed,
                },
            } if mod_id == id("refusing_mod") && source_mod_id == id("refusing_mod")
        ));
        assert_eq!(host.applied, None);
    }

    #[test]
    fn every_mod_that_declares_settings_is_handed_the_values_in_force() {
        let mut first = Settled::new("first_mod");
        let mut second = Settled::new("second_mod");
        let machine = machine(vec![
            entry("first_mod", &[Side::DedicatedServer]),
            entry("second_mod", &[Side::DedicatedServer]),
        ]);
        let packs = BTreeMap::from([(id("second_mod"), document("second_mod", "9"))]);

        let configured = open_configuration(
            &machine,
            &mut [&mut first, &mut second],
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
        let machine = machine(vec![
            entry("deaf_mod", &[Side::DedicatedServer]),
            entry("later_mod", &[Side::DedicatedServer]),
        ]);

        let refused = open_configuration(
            &machine,
            &mut [&mut deaf, &mut later],
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
        assert_eq!(refused.owner(), &id("deaf_mod"));
        assert_eq!(
            later.applied, None,
            "the walk stopped rather than configuring the rest"
        );
    }

    #[test]
    fn a_mod_that_declares_no_settings_is_not_asked_and_the_set_still_opens() {
        let mut silent = Settled::new("silent_mod").declaring_nothing();
        let machine = machine(vec![entry("silent_mod", &[Side::DedicatedServer])]);

        let configured = open_configuration(
            &machine,
            &mut [&mut silent],
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
        let machine = machine(vec![entry("client_mod", &[Side::Client])]);

        let configured = open_configuration(
            &machine,
            &mut [&mut client],
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
        let machine = machine(vec![entry("first_mod", &[Side::DedicatedServer])]);
        let players = BTreeMap::from([(id("first_mod"), document("first_mod", "\"twelve\""))]);

        let configured = open_configuration(&machine, &mut [&mut host], &BTreeMap::new(), &players)
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
        let machine = machine(vec![entry("thief_mod", &[Side::DedicatedServer])]);

        let refused = open_configuration(
            &machine,
            &mut [&mut thief],
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
        let machine = machine(vec![entry("first_mod", &[Side::DedicatedServer])]);
        let packs = BTreeMap::from([(id("first_mod"), document("second_mod", "9"))]);

        let refused = open_configuration(&machine, &mut [&mut host], &packs, &BTreeMap::new())
            .expect_err("a document belongs to one mod");

        assert!(matches!(refused, ConfigRefused::Layer { .. }));
        assert!(std::error::Error::source(&refused).is_some());
        assert!(!refused.quotes_untrusted_text());
        assert_eq!(host.applied, None);
    }
}
