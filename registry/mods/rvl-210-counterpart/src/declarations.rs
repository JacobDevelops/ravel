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
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeclarationKind {
    /// A block.
    Block,
    /// A block entity.
    BlockEntity,
    /// An item.
    Item,
    /// An entity.
    Entity,
    /// A menu.
    Menu,
    /// A particle.
    Particle,
    /// A recipe.
    Recipe,
    /// A recipe kind.
    RecipeKind,
    /// A sound event.
    SoundEvent,
    /// A status effect.
    StatusEffect,
    /// A villager profession.
    VillagerProfession,
    /// A creative tab.
    CreativeTab,
    /// An armour material.
    ArmourMaterial,
    /// An enchantment effect component kind.
    EnchantmentEffectComponentKind,
    /// An enchantment entity effect kind.
    EnchantmentEntityEffectKind,
    /// A loot condition.
    LootCondition,
    /// A loot function.
    LootFunction,
    /// A loot number provider.
    LootNumberProvider,
    /// A loot score provider.
    LootScoreProvider,
    /// A loot data provider.
    LootDataProvider,
    /// A world feature.
    WorldFeature,
    /// A world structure.
    WorldStructure,
    /// A structure piece.
    StructurePiece,
    /// A trunk placer.
    TrunkPlacer,
    /// A foliage placer.
    FoliagePlacer,
    /// A command argument kind.
    CommandArgumentKind,
    /// An advancement trigger kind.
    AdvancementTriggerKind,
    /// A data component kind.
    DataComponentKind,
    /// A capability declaration.
    CapabilityDeclaration,
    /// Settings without registered content.
    SettingsOnly,
    /// A fluid.
    Fluid,
    /// A loot modifier.
    LootModifier,
    /// A biome modifier.
    BiomeModifier,
    /// A recipe condition kind.
    RecipeConditionKind,
}

impl DeclarationKind {
    /// Every kind in section 5.1 registration order.
    pub const ALL: [Self; 34] = [
        Self::Block,
        Self::BlockEntity,
        Self::Item,
        Self::Entity,
        Self::Menu,
        Self::Particle,
        Self::Recipe,
        Self::RecipeKind,
        Self::SoundEvent,
        Self::StatusEffect,
        Self::VillagerProfession,
        Self::CreativeTab,
        Self::ArmourMaterial,
        Self::EnchantmentEffectComponentKind,
        Self::EnchantmentEntityEffectKind,
        Self::LootCondition,
        Self::LootFunction,
        Self::LootNumberProvider,
        Self::LootScoreProvider,
        Self::LootDataProvider,
        Self::WorldFeature,
        Self::WorldStructure,
        Self::StructurePiece,
        Self::TrunkPlacer,
        Self::FoliagePlacer,
        Self::CommandArgumentKind,
        Self::AdvancementTriggerKind,
        Self::DataComponentKind,
        Self::CapabilityDeclaration,
        Self::SettingsOnly,
        Self::Fluid,
        Self::LootModifier,
        Self::BiomeModifier,
        Self::RecipeConditionKind,
    ];

    /// The canonical declaration-document spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::BlockEntity => "block_entity",
            Self::Item => "item",
            Self::Entity => "entity",
            Self::Menu => "menu",
            Self::Particle => "particle",
            Self::Recipe => "recipe",
            Self::RecipeKind => "recipe_kind",
            Self::SoundEvent => "sound_event",
            Self::StatusEffect => "status_effect",
            Self::VillagerProfession => "villager_profession",
            Self::CreativeTab => "creative_tab",
            Self::ArmourMaterial => "armour_material",
            Self::EnchantmentEffectComponentKind => "enchantment_effect_component_kind",
            Self::EnchantmentEntityEffectKind => "enchantment_entity_effect_kind",
            Self::LootCondition => "loot_condition",
            Self::LootFunction => "loot_function",
            Self::LootNumberProvider => "loot_number_provider",
            Self::LootScoreProvider => "loot_score_provider",
            Self::LootDataProvider => "loot_data_provider",
            Self::WorldFeature => "world_feature",
            Self::WorldStructure => "world_structure",
            Self::StructurePiece => "structure_piece",
            Self::TrunkPlacer => "trunk_placer",
            Self::FoliagePlacer => "foliage_placer",
            Self::CommandArgumentKind => "command_argument_kind",
            Self::AdvancementTriggerKind => "advancement_trigger_kind",
            Self::DataComponentKind => "data_component_kind",
            Self::CapabilityDeclaration => "capability_declaration",
            Self::SettingsOnly => "settings_only",
            Self::Fluid => "fluid",
            Self::LootModifier => "loot_modifier",
            Self::BiomeModifier => "biome_modifier",
            Self::RecipeConditionKind => "recipe_condition_kind",
        }
    }
}

/// One thing a dependent declares.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    kind: DeclarationKind,
    name: String,
    disableable: bool,
    companion: Option<String>,
}

impl Declaration {
    /// Declares one piece of content, switchable through the settings document.
    #[must_use]
    pub fn new(kind: DeclarationKind, name: &str) -> Self {
        Self {
            kind,
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
    pub const fn kind(&self) -> DeclarationKind {
        self.kind
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
        }
    }
}

impl std::error::Error for SwitchRefused {}

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
    pub fn declare(&mut self, declaration: Declaration) {
        self.declared.push(declaration);
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
                kind: declaration.kind.as_str().to_owned(),
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
        for kind in DeclarationKind::ALL {
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
#[must_use]
pub fn own_declarations() -> Declarations {
    let mut declared = Declarations::new();
    let entries: [(DeclarationKind, &str, Option<&str>); 16] = [
        (
            DeclarationKind::Particle,
            "soft_glow",
            Some("soft_glow_renderer"),
        ),
        (
            DeclarationKind::Particle,
            "falling_droplet",
            Some("falling_droplet_renderer"),
        ),
        (DeclarationKind::DataComponentKind, "capacity", None),
        (DeclarationKind::DataComponentKind, "stored_energy", None),
        (DeclarationKind::DataComponentKind, "fluid_content", None),
        (DeclarationKind::DataComponentKind, "embedded_store", None),
        (
            DeclarationKind::AdvancementTriggerKind,
            "screen_opened",
            None,
        ),
        (
            DeclarationKind::AdvancementTriggerKind,
            "item_crafted",
            None,
        ),
        (
            DeclarationKind::AdvancementTriggerKind,
            "first_item_obtained",
            None,
        ),
        (
            DeclarationKind::CommandArgumentKind,
            "settings_property",
            None,
        ),
        (
            DeclarationKind::CommandArgumentKind,
            "diagnostic_message",
            None,
        ),
        (
            DeclarationKind::CommandArgumentKind,
            "enumeration_member",
            None,
        ),
        (
            DeclarationKind::Menu,
            "demonstration_manual",
            Some("demonstration_screen"),
        ),
        (DeclarationKind::LootModifier, "injected_item", None),
        (DeclarationKind::CapabilityDeclaration, "fluid_store", None),
        (DeclarationKind::SettingsOnly, "development_shortcuts", None),
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
        declared.declare(declaration);
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
        let kinds: Vec<DeclarationKind> = declared
            .registration_order()
            .into_iter()
            .map(Declaration::kind)
            .collect();
        let mut seen: Vec<DeclarationKind> = Vec::new();
        for kind in kinds {
            if seen.last() != Some(&kind) {
                assert!(!seen.contains(&kind), "the kind {kind:?} was returned to");
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
            .filter(|declaration| declaration.kind() == DeclarationKind::DataComponentKind)
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
            .filter(|declaration| declaration.kind() != DeclarationKind::SettingsOnly)
            .count();
        assert_eq!(registering, 15);
        assert_eq!(
            declared
                .declared()
                .iter()
                .filter(|declaration| declaration.kind() == DeclarationKind::SettingsOnly)
                .count(),
            1
        );
    }

    #[test]
    fn the_census_of_section_four_is_the_counts_it_states() {
        let declared = own_declarations();
        let count = |kind: DeclarationKind| {
            declared
                .declared()
                .iter()
                .filter(|declaration| declaration.kind() == kind)
                .count()
        };
        assert_eq!(count(DeclarationKind::Particle), 2);
        assert_eq!(count(DeclarationKind::DataComponentKind), 4);
        assert_eq!(count(DeclarationKind::AdvancementTriggerKind), 3);
        assert_eq!(count(DeclarationKind::CommandArgumentKind), 3);
        assert_eq!(count(DeclarationKind::Menu), 1);
        assert_eq!(count(DeclarationKind::LootModifier), 1);
        assert_eq!(count(DeclarationKind::CapabilityDeclaration), 1);
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
                    DeclarationKind::DataComponentKind | DeclarationKind::AdvancementTriggerKind
                )
            })
            .count();
        assert_eq!(storable, 7);
    }
}
