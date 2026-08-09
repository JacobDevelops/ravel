use std::collections::BTreeMap;
use std::fmt;

use ravel_config::{LayerError, LayeredConfiguration, Rejection, ValueDocument};
use ravel_core::{FailureKind, LifecycleState, ModId, StructuredFailure};
use ravel_host_api::{HostError, HostMod};

use crate::lifecycle::LifecycleMachine;

/// Resolves every mod's settings and hands each one the values in force.
///
/// The counterpart of [`crate::open_services`] and [`crate::open_saved_data`], and
/// it exists for the same reason both of those do: a trait method nothing drives
/// over a set is a contract closed in the type and open in every composition. The
/// `Unsupported` default that makes a mod which declared settings and never read
/// them loud is only useful if something asks every such mod.
///
/// ADG-6 puts the layering in data, so this decides nothing about a value. It
/// walks the set in resolved load order, skips the mods this side parks, and for
/// each mod that declares settings resolves the author's defaults under the pack's
/// overrides and the player's file. A mod that declares none is not called: there
/// is no declaration to resolve, and calling it with somebody else's would be the
/// one mistake this walk exists to make impossible.
///
/// # Failure closure
///
/// Nothing is applied partially, in the sense that matters: a refusal stops the
/// walk, so every mod after the offender is never configured and the caller must
/// not start the set. A mod configured before the refusal holds settings for a
/// launch that does not happen, which is the same shape ADG-7 defines for saved
/// data — the mod holds its own state and the session is withheld.
///
/// A dropped player value is not a refusal. The layered result already dropped it
/// and fell back to the layer below, and this reports it so a caller can tell the
/// player rather than deciding for them.
///
/// # Errors
///
/// Returns [`ConfigRefused::ForeignDeclaration`] when a mod declares settings owned
/// by another identity, [`ConfigRefused::Layer`] when a pack or a player document
/// cannot be layered onto the declaration at all, and [`ConfigRefused::Mod`] when a
/// mod refused the settings it was handed — including the default's refusal for a
/// mod that declared settings and never implemented [`HostMod::configure`].
pub fn open_configuration(
    machine: &LifecycleMachine,
    mods: &mut [&mut dyn HostMod],
    packs: &BTreeMap<ModId, ValueDocument>,
    players: &BTreeMap<ModId, ValueDocument>,
) -> Result<Configured, ConfigRefused> {
    let mut configured = Configured::default();

    for mod_id in machine.order().cloned().collect::<Vec<ModId>>() {
        if !machine
            .entry(&mod_id)
            .is_ok_and(|entry| entry.sides.loads_on(machine.side()))
        {
            continue;
        }
        let Some(host) = mods.iter_mut().find(|host| host.id() == &mod_id) else {
            continue;
        };
        let Some(declaration) = host.configuration_declaration() else {
            continue;
        };
        if declaration.owner != mod_id {
            return Err(ConfigRefused::ForeignDeclaration {
                mod_id,
                owner: declaration.owner.clone(),
            });
        }

        let resolved =
            LayeredConfiguration::resolve(declaration, packs.get(&mod_id), players.get(&mod_id))
                .map_err(|source| ConfigRefused::Layer {
                    mod_id: mod_id.clone(),
                    source,
                })?;
        for dropped in resolved.rejected() {
            configured.dropped.push((mod_id.clone(), dropped.clone()));
        }
        host.configure(&resolved)
            .map_err(|source| ConfigRefused::Mod {
                mod_id: mod_id.clone(),
                source,
            })?;
        configured.order.push(mod_id);
    }

    Ok(configured)
}

/// Which mods were configured, and what the player asked for and did not get.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Configured {
    order: Vec<ModId>,
    dropped: Vec<(ModId, Rejection)>,
}

impl Configured {
    /// Returns every mod that was handed its settings, in the order it was.
    #[must_use]
    pub fn order(&self) -> &[ModId] {
        &self.order
    }

    /// Returns every player value that was dropped, with the mod that owns it.
    ///
    /// In the order the walk found them, so a report reads as load order and then
    /// as key order within one mod.
    #[must_use]
    pub fn dropped(&self) -> &[(ModId, Rejection)] {
        &self.dropped
    }
}

/// Why a set's settings were not applied.
#[derive(Clone, Debug, PartialEq)]
pub enum ConfigRefused {
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
            Self::ForeignDeclaration { mod_id, .. }
            | Self::Layer { mod_id, .. }
            | Self::Mod { mod_id, .. } => mod_id,
        }
    }
}

impl fmt::Display for ConfigRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::ForeignDeclaration { .. } => None,
            Self::Layer { source, .. } => Some(source),
            Self::Mod { source, .. } => Some(source),
        }
    }
}

impl StructuredFailure for ConfigRefused {
    /// None of the three is one of `SPEC.md` §9.2's dependency-graph outcomes: the
    /// graph resolved and a document or a mod is what was refused.
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Constructed
    }

    /// A mod's own refusal carries the sentence that mod wrote. A layering refusal
    /// quotes a key and a schema, both of which came out of a declaration this
    /// repository's schema already validated.
    fn quotes_untrusted_text(&self) -> bool {
        matches!(self, Self::Mod { .. })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use ravel_config::{ConfigurationDeclaration, LayeredConfiguration, ValueDocument};
    use ravel_core::{LifecycleState, ModId, Side, SideSet, StructuredFailure};
    use ravel_host_api::{HostError, HostMod};

    use super::{ConfigRefused, open_configuration};
    use crate::lifecycle::{LifecycleEntry, LifecycleMachine, Requirement};

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

    /// A mod that declares one setting and records the value it was handed.
    struct Settled {
        id: ModId,
        declares: Option<ConfigurationDeclaration>,
        applied: Option<i64>,
        deaf: bool,
    }

    impl Settled {
        fn new(name: &str) -> Self {
            Self {
                id: id(name),
                declares: Some(declaration(name)),
                applied: None,
                deaf: false,
            }
        }

        fn declaring_nothing(mut self) -> Self {
            self.declares = None;
            self
        }

        /// Declares settings and never reads them, which is the loud default's case.
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

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn configuration_declaration(&self) -> Option<ConfigurationDeclaration> {
            self.declares.clone()
        }

        fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), HostError> {
            if self.deaf {
                // Exactly what the trait's default does, reached the same way.
                return Err(HostError::Unsupported {
                    mod_id: self.id.clone(),
                    state: LifecycleState::Constructed,
                });
            }
            self.applied = resolved
                .value(&"limit".parse().expect("a legal key"))
                .and_then(serde_json::Value::as_i64);
            Ok(())
        }
    }

    fn entry(name: &str, sides: &[Side]) -> LifecycleEntry {
        LifecycleEntry {
            mod_id: id(name),
            sides: SideSet::new(sides.iter().copied()).expect("a non-empty side set"),
            requirement: Requirement::Required,
            saved_data_versions: BTreeSet::new(),
        }
    }

    fn machine(entries: Vec<LifecycleEntry>) -> LifecycleMachine {
        LifecycleMachine::new(Side::DedicatedServer, entries).expect("a legal plan")
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
        assert!(configured.dropped().is_empty());
    }

    #[test]
    fn a_mod_that_declares_settings_and_never_reads_them_refuses_the_whole_set() {
        // The loud default's case, reached by a walk rather than by a direct call.
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

        assert!(configured.order().is_empty());
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

        assert!(configured.order().is_empty());
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

        assert_eq!(
            refused,
            ConfigRefused::ForeignDeclaration {
                mod_id: id("thief_mod"),
                owner: id("first_mod"),
            }
        );
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
