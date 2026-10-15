//! Sections four and five: the declaration vocabulary and what this entry
//! declares through it.
//!
//! Section 5.1 says which part of the framework is a contract rather than a
//! convenience, and it is three rules: every declaration is separately
//! disableable through the settings document and the framework refuses the
//! switch for one marked otherwise, registration order within a kind is
//! declaration order and a kind is finished before the next is begun, and a
//! declaration's client-side companion is loaded on a client only as a
//! declaration-time split rather than a runtime side test.
//!
//! Ravel's own registry has four kinds, so most of the vocabulary below is a
//! name a dependent declares against rather than something claimed at the
//! barrier. `NOTES.md` says which is which. The three rules are implemented over
//! the whole vocabulary either way, because they are what a dependent observes.

use ravel_sdk::Side;
use ravel_sdk::content::ContentKind;

use core::fmt;

/// The kinds of thing a dependent may declare.
///
/// Thirty-four, in the order section 5.1 lists them.
pub const KINDS: [&str; 34] = [
    "block",
    "block_entity",
    "item",
    "entity",
    "menu",
    "particle",
    "recipe",
    "recipe_kind",
    "sound_event",
    "status_effect",
    "villager_profession",
    "creative_tab",
    "armour_material",
    "enchantment_effect_component_kind",
    "enchantment_entity_effect_kind",
    "loot_condition",
    "loot_function",
    "loot_number_provider",
    "loot_score_provider",
    "loot_data_provider",
    "world_feature",
    "world_structure",
    "structure_piece",
    "trunk_placer",
    "foliage_placer",
    "command_argument_kind",
    "advancement_trigger_kind",
    "data_component_kind",
    "capability_declaration",
    "settings_only",
    "fluid",
    "loot_modifier",
    "biome_modifier",
    "recipe_condition_kind",
];

/// One thing a dependent declares.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    kind: String,
    name: String,
    disableable: bool,
    companion: Option<String>,
}

impl Declaration {
    /// Declares one piece of content, switchable through the settings document.
    #[must_use]
    pub fn new(kind: &str, name: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            name: name.to_owned(),
            disableable: true,
            companion: None,
        }
    }

    /// Marks this declaration as one the framework will not switch off.
    #[must_use]
    pub fn never_disableable(mut self) -> Self {
        self.disableable = false;
        self
    }

    /// Attaches the client-side companion, which is loaded on a client only.
    #[must_use]
    pub fn with_companion(mut self, companion: &str) -> Self {
        self.companion = Some(companion.to_owned());
        self
    }

    /// The kind of thing this declares.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The name it registers under.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether the framework will switch it off.
    #[must_use]
    pub const fn disableable(&self) -> bool {
        self.disableable
    }

    /// The client-side companion, if it declared one.
    #[must_use]
    pub fn companion(&self) -> Option<&str> {
        self.companion.as_deref()
    }

    /// Whether this declaration's companion loads on a side.
    ///
    /// The split is decided here, at declaration time, rather than by a side
    /// test where the companion runs. It is why a library's client-only
    /// registration never runs on a dedicated server.
    #[must_use]
    pub fn companion_loads_on(&self, side: Side) -> bool {
        self.companion.is_some() && side == Side::Client
    }
}

/// What the framework refuses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SwitchRefused {
    /// The declaration is marked as not disableable.
    NotDisableable {
        /// Its kind.
        kind: String,
        /// Its name.
        name: String,
    },
    /// Nothing is declared under that name.
    NoSuchDeclaration {
        /// The name asked for.
        name: String,
    },
    /// The vocabulary does not admit that kind.
    NoSuchKind {
        /// The kind asked for.
        kind: String,
    },
}

impl fmt::Display for SwitchRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotDisableable { kind, name } => write!(
                formatter,
                "the {kind} declared as {name} is not one the framework switches off"
            ),
            Self::NoSuchDeclaration { name } => {
                write!(formatter, "nothing is declared under the name {name}")
            }
            Self::NoSuchKind { kind } => {
                write!(formatter, "the vocabulary admits no kind called {kind}")
            }
        }
    }
}

/// A dependent's declarations, and what the framework registers from them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Declarations {
    declared: Vec<Declaration>,
    switched_off: Vec<String>,
}

impl Declarations {
    /// An empty set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            declared: Vec::new(),
            switched_off: Vec::new(),
        }
    }

    /// Adds one declaration, keeping declaration order.
    ///
    /// # Errors
    ///
    /// Returns [`SwitchRefused::NoSuchKind`] when the kind is outside the
    /// platform vocabulary.
    pub fn declare(&mut self, declaration: Declaration) -> Result<(), SwitchRefused> {
        if !KINDS.contains(&declaration.kind.as_str()) {
            return Err(SwitchRefused::NoSuchKind {
                kind: declaration.kind,
            });
        }
        self.declared.push(declaration);
        Ok(())
    }

    /// Everything declared, in declaration order.
    #[must_use]
    pub fn declared(&self) -> &[Declaration] {
        &self.declared
    }

    /// Switches one declaration off through the settings document.
    ///
    /// # Errors
    ///
    /// Returns [`SwitchRefused::NotDisableable`] when the declaration refuses
    /// the switch, which the framework raises rather than silently ignoring, and
    /// [`SwitchRefused::NoSuchDeclaration`] when nothing carries that name.
    pub fn switch_off(&mut self, name: &str) -> Result<(), SwitchRefused> {
        let Some(declaration) = self
            .declared
            .iter()
            .find(|declaration| declaration.name == name)
        else {
            return Err(SwitchRefused::NoSuchDeclaration {
                name: name.to_owned(),
            });
        };
        if !declaration.disableable {
            return Err(SwitchRefused::NotDisableable {
                kind: declaration.kind.clone(),
                name: declaration.name.clone(),
            });
        }
        if !self.switched_off.iter().any(|held| held == name) {
            self.switched_off.push(name.to_owned());
        }
        Ok(())
    }

    /// Whether a declaration is switched off.
    #[must_use]
    pub fn is_switched_off(&self, name: &str) -> bool {
        self.switched_off.iter().any(|held| held == name)
    }

    /// What the framework registers, in the order it registers it.
    ///
    /// Every declaration of one kind is registered before the next kind begins,
    /// and within a kind the order is declaration order. A declaration that is
    /// switched off is never registered.
    #[must_use]
    pub fn registration_order(&self) -> Vec<&Declaration> {
        let mut ordered = Vec::new();
        for kind in KINDS {
            ordered.extend(
                self.declared
                    .iter()
                    .filter(|declaration| declaration.kind == kind)
                    .filter(|declaration| !self.is_switched_off(&declaration.name)),
            );
        }
        ordered
    }

    /// The companions loaded on a side, in registration order.
    #[must_use]
    pub fn companions_on(&self, side: Side) -> Vec<&str> {
        self.registration_order()
            .into_iter()
            .filter(|declaration| declaration.companion_loads_on(side))
            .filter_map(Declaration::companion)
            .collect()
    }
}

/// The sixteen entries this entry declares of its own, in declaration order.
///
/// Fifteen register and one is the settings-only declaration that registers
/// nothing. No identifier here is the original's: section four states in as many
/// words that the seven storable ones are what the provenance record does not
/// clear and names none of them, so these names are this package's own.
///
/// # Panics
/// Panics if a shipped declaration names a kind outside the vocabulary.
#[must_use]
pub fn own_declarations() -> Declarations {
    let mut declared = Declarations::new();
    let entries: [(&str, &str, Option<&str>); 16] = [
        ("particle", "soft_glow", Some("soft_glow_renderer")),
        (
            "particle",
            "falling_droplet",
            Some("falling_droplet_renderer"),
        ),
        ("data_component_kind", "capacity", None),
        ("data_component_kind", "stored_energy", None),
        ("data_component_kind", "fluid_content", None),
        ("data_component_kind", "embedded_store", None),
        ("advancement_trigger_kind", "screen_opened", None),
        ("advancement_trigger_kind", "item_crafted", None),
        ("advancement_trigger_kind", "first_item_obtained", None),
        ("command_argument_kind", "settings_property", None),
        ("command_argument_kind", "diagnostic_message", None),
        ("command_argument_kind", "enumeration_member", None),
        ("menu", "demonstration_manual", Some("demonstration_screen")),
        ("loot_modifier", "injected_item", None),
        ("capability_declaration", "fluid_store", None),
        ("settings_only", "development_shortcuts", None),
    ];
    for (kind, name, companion) in entries {
        let mut declaration = Declaration::new(kind, name);
        if let Some(companion) = companion {
            declaration = declaration.with_companion(companion);
        }
        // The two argument kinds the command tree needs before it can parse a
        // settings name are not switchable: a tree with a missing argument kind
        // does not build at all.
        if name == "settings_property" || name == "diagnostic_message" {
            declaration = declaration.never_disableable();
        }
        declared
            .declare(declaration)
            .expect("every kind these constants name is in the vocabulary");
    }
    declared
}

/// The kinds among this entry's own declarations that Ravel's registry claims.
///
/// Ravel's registry admits blocks, items, recipes and commands. Nothing this
/// entry declares is a block, an item or a recipe, so the command tree of
/// section fifteen is the whole of what reaches the barrier.
pub const CLAIMED_KINDS: [ContentKind; 1] = [ContentKind::Command];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vocabulary_is_thirty_four_kinds_named_once_each() {
        assert_eq!(KINDS.len(), 34);
        let mut sorted = KINDS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 34);
    }

    #[test]
    fn a_kind_outside_the_vocabulary_is_refused_at_declaration() {
        let mut declared = Declarations::new();
        assert_eq!(
            declared.declare(Declaration::new("nothing_admits_this", "one")),
            Err(SwitchRefused::NoSuchKind {
                kind: "nothing_admits_this".to_owned()
            })
        );
        assert_eq!(declared.declared().len(), 0);
    }

    #[test]
    fn the_framework_raises_for_a_declaration_marked_as_not_disableable() {
        let mut declared = own_declarations();
        assert_eq!(
            declared.switch_off("settings_property"),
            Err(SwitchRefused::NotDisableable {
                kind: "command_argument_kind".to_owned(),
                name: "settings_property".to_owned(),
            })
        );
        assert!(!declared.is_switched_off("settings_property"));
    }

    #[test]
    fn switching_off_a_name_nothing_declares_is_refused_rather_than_remembered() {
        let mut declared = own_declarations();
        assert_eq!(
            declared.switch_off("never_declared"),
            Err(SwitchRefused::NoSuchDeclaration {
                name: "never_declared".to_owned()
            })
        );
        assert!(!declared.is_switched_off("never_declared"));
    }

    #[test]
    fn a_switched_off_declaration_is_never_registered() {
        let mut declared = own_declarations();
        let before = declared.registration_order().len();
        declared.switch_off("soft_glow").expect("it is switchable");
        let after = declared.registration_order();
        assert_eq!(after.len(), before - 1);
        assert!(
            !after
                .iter()
                .any(|declaration| declaration.name() == "soft_glow"),
            "a switched-off declaration reached the registry"
        );
    }

    #[test]
    fn every_declaration_of_one_kind_is_registered_before_the_next_kind_begins() {
        let declared = own_declarations();
        let kinds: Vec<&str> = declared
            .registration_order()
            .into_iter()
            .map(Declaration::kind)
            .collect();
        let mut seen: Vec<&str> = Vec::new();
        for kind in kinds {
            if seen.last() != Some(&kind) {
                assert!(!seen.contains(&kind), "the kind {kind} was returned to");
                seen.push(kind);
            }
        }
    }

    #[test]
    fn the_order_within_a_kind_is_declaration_order() {
        let declared = own_declarations();
        let components: Vec<&str> = declared
            .registration_order()
            .into_iter()
            .filter(|declaration| declaration.kind() == "data_component_kind")
            .map(Declaration::name)
            .collect();
        assert_eq!(
            components,
            vec![
                "capacity",
                "stored_energy",
                "fluid_content",
                "embedded_store"
            ]
        );
    }

    #[test]
    fn a_companion_loads_on_a_client_and_on_neither_server() {
        let declared = own_declarations();
        assert_eq!(declared.companions_on(Side::Client).len(), 3);
        assert_eq!(declared.companions_on(Side::DedicatedServer).len(), 0);
        assert_eq!(declared.companions_on(Side::IntegratedServer).len(), 0);
    }

    #[test]
    fn this_entry_declares_sixteen_entries_and_fifteen_of_them_register() {
        let declared = own_declarations();
        assert_eq!(declared.declared().len(), 16);
        let registering = declared
            .declared()
            .iter()
            .filter(|declaration| declaration.kind() != "settings_only")
            .count();
        assert_eq!(registering, 15);
        assert_eq!(
            declared
                .declared()
                .iter()
                .filter(|declaration| declaration.kind() == "settings_only")
                .count(),
            1
        );
    }

    #[test]
    fn the_census_of_section_four_is_the_counts_it_states() {
        let declared = own_declarations();
        let count = |kind: &str| {
            declared
                .declared()
                .iter()
                .filter(|declaration| declaration.kind() == kind)
                .count()
        };
        assert_eq!(count("particle"), 2);
        assert_eq!(count("data_component_kind"), 4);
        assert_eq!(count("advancement_trigger_kind"), 3);
        assert_eq!(count("command_argument_kind"), 3);
        assert_eq!(count("menu"), 1);
        assert_eq!(count("loot_modifier"), 1);
        assert_eq!(count("capability_declaration"), 1);
    }

    #[test]
    fn the_seven_storable_kinds_are_the_ones_the_provenance_record_leaves_open() {
        let declared = own_declarations();
        let storable = declared
            .declared()
            .iter()
            .filter(|declaration| {
                matches!(
                    declaration.kind(),
                    "data_component_kind" | "advancement_trigger_kind"
                )
            })
            .count();
        assert_eq!(storable, 7);
    }
}
