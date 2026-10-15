//! The recipe kinds, the ingredient kinds, the conditions, and the three
//! recipes derived per crop.
//!
//! Ravel publishes no recipe kind, no serialiser, no ingredient kind and no
//! data-pack condition, so nothing here is registered anywhere. What is here is
//! the matching and assembly logic over declared inputs, plus the derivation a
//! dependent's crops are put through, both of which are portable and both of
//! which a dependent observes as arithmetic rather than as registration.

use std::collections::BTreeMap;

use rvl_042_counterpart::Stack;

use crate::model::{Crafted, Crop, CropKind, Tier};

/// Which part of a crop a component selector names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    /// The crop's tier's material item.
    TierMaterial,
    /// The crop's kind's base seed item.
    BaseSeed,
    /// What crafting a seed of the crop consumes.
    Crafted,
}

/// What an ingredient kind needs to resolve itself.
pub trait Catalogue {
    /// Whether an entry belongs to a tag.
    fn tag_holds(&self, tag: &str, entry: &str) -> bool;

    /// The entry a crop's named component resolves to.
    fn component(&self, crop: &str, role: Role) -> Option<String>;

    /// Whether an entry is one of the tilling family of tools.
    fn tilling_tool(&self, entry: &str) -> bool;

    /// The charge a stack holds, as a type identifier and an amount.
    fn charge(&self, stack: &Stack) -> Option<(String, f64)>;
}

/// A declared ingredient.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Ingredient {
    /// One named entry.
    Entry(String),
    /// Any member of a tag.
    Tag(String),
    /// Every tool of the tilling family.
    TillingTool,
    /// A container holding more than nothing.
    ChargedContainer,
    /// A named crop's named component.
    CropComponent {
        /// The crop named.
        crop: String,
        /// Which of its three components is named.
        role: Role,
    },
}

impl Ingredient {
    /// Whether this kind resolves to a fixed set of entries.
    ///
    /// A non-simple ingredient answers a predicate instead, and the cache of
    /// [`crate::cache`] can only reach it through the entries it enumerates.
    #[must_use]
    pub const fn simple(&self) -> bool {
        matches!(self, Self::Entry(_) | Self::Tag(_) | Self::TillingTool)
    }

    /// Whether a stack satisfies this ingredient.
    #[must_use]
    pub fn matches(&self, stack: &Stack, catalogue: &dyn Catalogue) -> bool {
        if stack.is_empty() {
            return false;
        }
        match self {
            Self::Entry(entry) => stack.entry == *entry,
            Self::Tag(tag) => catalogue.tag_holds(tag, &stack.entry),
            Self::TillingTool => catalogue.tilling_tool(&stack.entry),
            Self::ChargedContainer => catalogue
                .charge(stack)
                .is_some_and(|(_, amount)| amount > 0.0),
            Self::CropComponent { crop, role } => catalogue
                .component(crop, *role)
                .is_some_and(|entry| entry == stack.entry),
        }
    }
}

/// Assigns each declared ingredient a distinct offered stack, in any order.
///
/// The count has to be exact: a ring holding more items than the recipe
/// declares does not match, and neither does one holding fewer.
#[must_use]
pub fn assigns(declared: &[Ingredient], offered: &[Stack], catalogue: &dyn Catalogue) -> bool {
    let present: Vec<&Stack> = offered.iter().filter(|stack| !stack.is_empty()).collect();
    if present.len() != declared.len() {
        return false;
    }
    let mut taken = vec![false; present.len()];
    assign_from(declared, &present, &mut taken, 0, catalogue)
}

fn assign_from(
    declared: &[Ingredient],
    present: &[&Stack],
    taken: &mut Vec<bool>,
    at: usize,
    catalogue: &dyn Catalogue,
) -> bool {
    if at == declared.len() {
        return true;
    }
    for index in 0..present.len() {
        if taken[index] || !declared[at].matches(present[index], catalogue) {
            continue;
        }
        taken[index] = true;
        if assign_from(declared, present, taken, at + 1, catalogue) {
            return true;
        }
        taken[index] = false;
    }
    false
}

/// The four-station ritual: an altar item and up to eight ring ingredients.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FourStation {
    /// The recipe's own identifier.
    pub id: String,
    /// What sits on the altar.
    pub altar: Ingredient,
    /// What sits on the ring stations.
    pub ring: Vec<Ingredient>,
    /// What the ritual produces.
    pub result: Stack,
    /// Whether the altar item's item-level data is copied onto the result.
    pub copies_data: bool,
}

impl FourStation {
    /// Whether an altar item and a ring satisfy this recipe.
    #[must_use]
    pub fn matches(&self, altar: &Stack, ring: &[Stack], catalogue: &dyn Catalogue) -> bool {
        self.altar.matches(altar, catalogue) && assigns(&self.ring, ring, catalogue)
    }
}

/// What a reservoir set is missing before an eight-station ritual may advance.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Shortfall {
    /// Each declared material the reservoirs do not cover, and by how much.
    pub missing: Vec<Stack>,
}

impl Shortfall {
    /// Whether the reservoirs cover every declared material.
    #[must_use]
    pub fn covered(&self) -> bool {
        self.missing.is_empty()
    }
}

/// The eight-station ritual: four ingredient stations and four reservoirs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EightStation {
    /// The recipe's own identifier.
    pub id: String,
    /// What sits on the altar.
    pub altar: Ingredient,
    /// The four ingredient stations.
    pub ring: Vec<Ingredient>,
    /// The four material stacks the reservoirs have to cover.
    pub materials: Vec<Stack>,
    /// What the ritual produces.
    pub result: Stack,
}

impl EightStation {
    /// What the reservoirs are short of, by entry and by count.
    #[must_use]
    pub fn shortfall(&self, reservoirs: &[Stack]) -> Shortfall {
        let mut held: BTreeMap<&str, u64> = BTreeMap::new();
        for stack in reservoirs.iter().filter(|stack| !stack.is_empty()) {
            *held.entry(stack.entry.as_str()).or_default() += stack.count;
        }
        let mut missing = Vec::new();
        for declared in &self.materials {
            let have = held.get(declared.entry.as_str()).copied().unwrap_or(0);
            if have < declared.count {
                missing.push(Stack::new(&declared.entry, declared.count - have));
            }
        }
        Shortfall { missing }
    }

    /// Whether the altar, the ring and the reservoirs all satisfy this recipe.
    #[must_use]
    pub fn matches(
        &self,
        altar: &Stack,
        ring: &[Stack],
        reservoirs: &[Stack],
        catalogue: &dyn Catalogue,
    ) -> bool {
        self.altar.matches(altar, catalogue)
            && assigns(&self.ring, ring, catalogue)
            && self.shortfall(reservoirs).covered()
    }

    /// The ring as it is read: materials and ingredients alternating, material
    /// first.
    #[must_use]
    pub fn interleaved(&self) -> Vec<Ringed> {
        let mut ring = Vec::new();
        for index in 0..self.ring.len().max(self.materials.len()) {
            if let Some(material) = self.materials.get(index) {
                ring.push(Ringed::Material(material.entry.clone()));
            }
            if let Some(ingredient) = self.ring.get(index) {
                ring.push(Ringed::Ingredient(ingredient.clone()));
            }
        }
        ring
    }

    /// The ingredients a document encoding writes.
    ///
    /// Four in, four out. The original's document getter hands back the
    /// interleaved eight while its document reader refuses more than four, so a
    /// recipe read from a data pack cannot be written back to one. This
    /// encoding is symmetric in both directions.
    #[must_use]
    pub fn encoded_ingredients(&self) -> &[Ingredient] {
        &self.ring
    }
}

/// One place in an eight-station ritual's ring, as the ring reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Ringed {
    /// A declared material, named by the entry it consumes.
    Material(String),
    /// A ring station's ingredient.
    Ingredient(Ingredient),
}

/// The enchanting recipe: two counted inputs applied to a target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Enchanting {
    /// The recipe's own identifier.
    pub id: String,
    /// The first input and how much of it one level costs.
    pub first: (Ingredient, u64),
    /// The second input and how much of it one level costs.
    pub second: (Ingredient, u64),
    /// The enchantment applied.
    pub enchantment: String,
    /// The highest level that enchantment reaches.
    pub maximum: u32,
}

/// Why an enchanting craft was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refused {
    /// One of the two inputs does not satisfy its ingredient.
    Input,
    /// Neither input covers a single level.
    NoLevel,
    /// The target already carries this enchantment at this level or above.
    NotAnUpgrade,
    /// The target carries an enchantment this one cannot coexist with.
    Incompatible(String),
}

/// What an enchanting craft consumes and produces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Enchanted {
    /// The level applied.
    pub level: u32,
    /// How much of the first input was consumed.
    pub first: u64,
    /// How much of the second input was consumed.
    pub second: u64,
    /// The entry the target became.
    pub target: String,
}

/// The entry a plain book becomes.
pub const PLAIN_BOOK: &str = "minecraft:book";
/// The entry a plain book is enchanted into.
pub const ENCHANTED_BOOK: &str = "minecraft:enchanted_book";

impl Enchanting {
    /// Applies this recipe to a target.
    ///
    /// The compatibility test is the corrected one: a craft is refused when the
    /// target carries an enchantment this one cannot coexist with. The original
    /// tests the same pairs and refuses the compatible ones, which permits
    /// exactly the pairs that may not coexist.
    ///
    /// # Errors
    ///
    /// Answers with the rule that refused the craft.
    pub fn apply(
        &self,
        first: &Stack,
        second: &Stack,
        target: &Stack,
        held: &BTreeMap<String, u32>,
        compatible: &dyn Fn(&str, &str) -> bool,
        catalogue: &dyn Catalogue,
    ) -> Result<Enchanted, Refused> {
        if !self.first.0.matches(first, catalogue) || !self.second.0.matches(second, catalogue) {
            return Err(Refused::Input);
        }
        let level = first
            .count
            .div_euclid(self.first.1.max(1))
            .min(second.count.div_euclid(self.second.1.max(1)))
            .min(u64::from(self.maximum));
        let level = u32::try_from(level).unwrap_or(self.maximum);
        if level == 0 {
            return Err(Refused::NoLevel);
        }
        for (present, at) in held {
            if present == &self.enchantment {
                if *at >= level {
                    return Err(Refused::NotAnUpgrade);
                }
            } else if !compatible(&self.enchantment, present) {
                return Err(Refused::Incompatible(present.clone()));
            }
        }
        let entry = if target.entry == PLAIN_BOOK {
            ENCHANTED_BOOK.to_owned()
        } else {
            target.entry.clone()
        };
        Ok(Enchanted {
            level,
            first: self.first.1 * u64::from(level),
            second: self.second.1 * u64::from(level),
            target: entry,
        })
    }
}

/// A charge extraction recipe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChargeExtraction {
    /// The recipe's own identifier.
    pub id: String,
    /// What is consumed.
    pub input: Ingredient,
    /// The charge type the container has to hold or be empty of.
    pub charge_type: String,
    /// How much charge one extraction adds.
    pub amount: u64,
    /// How much a full container of this type holds.
    pub requirement: u64,
}

impl ChargeExtraction {
    /// The charge a container holds after one extraction.
    ///
    /// The container has to be empty or already hold this type, and has to have
    /// room. The sum is capped at the type's own requirement.
    ///
    /// # Errors
    ///
    /// Answers `None` when the container is of another type or already full.
    #[must_use]
    pub fn extract(&self, held: Option<(&str, u64)>) -> Option<u64> {
        let current = match held {
            None => 0,
            Some((kind, _)) if kind != self.charge_type => return None,
            Some((_, amount)) => amount,
        };
        if current >= self.requirement {
            return None;
        }
        Some((current + self.amount).min(self.requirement))
    }
}

/// A tilling recipe: a shapeless recipe that returns its tool, one point worse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tilling {
    /// The recipe's own identifier.
    pub id: String,
    /// The ingredients other than the tool.
    pub inputs: Vec<Ingredient>,
    /// The tool the recipe uses and returns.
    pub tool: Ingredient,
    /// What the recipe produces.
    pub result: Stack,
}

impl Tilling {
    /// The tool handed back as a remainder, or nothing when it is destroyed.
    #[must_use]
    pub fn remainder(damage: u32, durability: u32) -> Option<u32> {
        let worn = damage + 1;
        (worn < durability).then_some(worn)
    }
}

/// An emptying recipe: exactly one charged container and nothing else.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Emptying {
    /// The recipe's own identifier.
    pub id: String,
    /// The entry an emptied container becomes.
    pub result: String,
}

impl Emptying {
    /// Whether a grid matches.
    ///
    /// The walk is order-sensitive and the order is worth keeping: a non-empty
    /// slot after a charged container refuses the match, so a container in the
    /// last slot matches and one in the first slot with anything after it does
    /// not.
    #[must_use]
    pub fn matches(&self, grid: &[Stack], catalogue: &dyn Catalogue) -> bool {
        let mut found = false;
        for stack in grid {
            if stack.is_empty() {
                continue;
            }
            if found {
                return false;
            }
            if Ingredient::ChargedContainer.matches(stack, catalogue) {
                found = true;
            }
        }
        found
    }
}

/// The four data-pack conditions, which decide whether a document loads at all.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Condition {
    /// The named crop is enabled.
    CropEnabled(String),
    /// The named modifier is enabled.
    ModifierEnabled(String),
    /// The named crop has a crafting material that resolves.
    CropCrafted(String),
    /// The plain-grid seed recipes are enabled.
    GridSeeds,
}

/// What a condition is answered against.
pub trait Conditions {
    /// Whether the named crop is enabled.
    fn crop_enabled(&self, crop: &str) -> bool;
    /// Whether the named modifier is enabled.
    fn modifier_enabled(&self, modifier: &str) -> bool;
    /// Whether the named crop's crafting material resolves to anything.
    fn crop_crafted(&self, crop: &str) -> bool;
    /// Whether the plain-grid seed recipes are switched on.
    fn grid_seeds(&self) -> bool;
}

impl Condition {
    /// Whether this condition holds.
    #[must_use]
    pub fn holds(&self, against: &dyn Conditions) -> bool {
        match self {
            Self::CropEnabled(crop) => against.crop_enabled(crop),
            Self::ModifierEnabled(modifier) => against.modifier_enabled(modifier),
            Self::CropCrafted(crop) => against.crop_crafted(crop),
            Self::GridSeeds => against.grid_seeds(),
        }
    }
}

/// One recipe derived from a crop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DerivedRecipe {
    /// The four-station ritual producing one seed.
    RitualSeed(FourStation),
    /// The plain-grid seed recipe, gated on a setting that ships off.
    GridSeed {
        /// The recipe's own identifier.
        id: String,
        /// The nine grid positions, reading across then down.
        grid: Vec<Option<Ingredient>>,
        /// What the recipe produces.
        result: Stack,
    },
    /// One seed turned into two of the crop's produce.
    Conversion {
        /// The recipe's own identifier.
        id: String,
        /// The seed consumed.
        input: Ingredient,
        /// What the recipe produces.
        result: Stack,
    },
}

impl DerivedRecipe {
    /// The recipe's own identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::RitualSeed(ritual) => &ritual.id,
            Self::GridSeed { id, .. } | Self::Conversion { id, .. } => id,
        }
    }
}

/// The suffix the ritual seed recipe is named with.
pub const RITUAL_SUFFIX: &str = "_seed_ritual";
/// The suffix the plain-grid seed recipe is named with.
pub const GRID_SUFFIX: &str = "_seed_grid";
/// The suffix the seed-conversion recipe is named with.
pub const CONVERSION_SUFFIX: &str = "_seed_conversion";

/// Derives the three recipes a crop contributes.
///
/// A disabled crop contributes nothing. A crop whose crafting material resolves
/// to nothing contributes no seed recipe and still contributes its conversion
/// recipe, so a seed obtained another way stays usable.
#[must_use]
pub fn derive(
    crop: &Crop,
    tier: &Tier,
    kind: &CropKind,
    grid_seeds_enabled: bool,
    resolves: &dyn Fn(&str) -> usize,
) -> Vec<DerivedRecipe> {
    if !crop.enabled {
        return Vec::new();
    }
    let path = crop.path();
    let seed = crate::registries::registered_id(crop, crate::registries::Role::Seed);
    let produce = crate::registries::registered_id(crop, crate::registries::Role::Material);
    let mut derived = Vec::new();

    let crafted = crafted_ingredient(&crop.crafted, resolves);
    let seedable = crafted.is_some() && tier.material.is_some() && kind.base_seed.is_some();

    if crop.derived.ritual_seed
        && seedable
        && let (Some(crafted), Some(material), Some(base)) = (
            crafted.clone(),
            tier.material.as_deref(),
            kind.base_seed.as_deref(),
        )
    {
        let mut ring = Vec::new();
        for index in 0..8 {
            ring.push(if index % 2 == 0 {
                crafted.clone()
            } else {
                Ingredient::Entry(material.to_owned())
            });
        }
        derived.push(DerivedRecipe::RitualSeed(FourStation {
            id: format!("{}:{path}{RITUAL_SUFFIX}", crate::MOD_ID),
            altar: Ingredient::Entry(base.to_owned()),
            ring,
            result: Stack::new(&seed, 1),
            copies_data: false,
        }));
    }

    if crop.derived.grid_seed
        && grid_seeds_enabled
        && seedable
        && let (Some(crafted), Some(material), Some(base)) =
            (crafted, tier.material.as_deref(), kind.base_seed.as_deref())
    {
        let edge = Ingredient::Entry(material.to_owned());
        let grid = vec![
            Some(crafted.clone()),
            Some(edge.clone()),
            Some(crafted.clone()),
            Some(edge.clone()),
            Some(Ingredient::Entry(base.to_owned())),
            Some(edge.clone()),
            Some(crafted.clone()),
            Some(edge),
            Some(crafted),
        ];
        derived.push(DerivedRecipe::GridSeed {
            id: format!("{}:{path}{GRID_SUFFIX}", crate::MOD_ID),
            grid,
            result: Stack::new(&seed, 1),
        });
    }

    if crop.derived.conversion {
        derived.push(DerivedRecipe::Conversion {
            id: format!("{}:{path}{CONVERSION_SUFFIX}", crate::MOD_ID),
            input: Ingredient::Entry(seed),
            result: Stack::new(&produce, 2),
        });
    }
    derived
}

fn crafted_ingredient(crafted: &Crafted, resolves: &dyn Fn(&str) -> usize) -> Option<Ingredient> {
    match crafted {
        Crafted::Entry { id, .. } => Some(Ingredient::Entry(id.clone())),
        Crafted::Tag(tag) if resolves(tag) > 0 => Some(Ingredient::Tag(tag.clone())),
        Crafted::Tag(_) | Crafted::Nothing => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Table {
        tags: BTreeMap<String, Vec<String>>,
        components: BTreeMap<String, String>,
        tools: Vec<String>,
        charges: BTreeMap<String, (String, f64)>,
    }

    impl Catalogue for Table {
        fn tag_holds(&self, tag: &str, entry: &str) -> bool {
            self.tags
                .get(tag)
                .is_some_and(|members| members.iter().any(|held| held == entry))
        }

        fn component(&self, crop: &str, role: Role) -> Option<String> {
            self.components.get(&format!("{crop}/{role:?}")).cloned()
        }

        fn tilling_tool(&self, entry: &str) -> bool {
            self.tools.iter().any(|tool| tool == entry)
        }

        fn charge(&self, stack: &Stack) -> Option<(String, f64)> {
            self.charges.get(&stack.entry).cloned()
        }
    }

    fn tiers() -> Tier {
        let mut tier = Tier::new("test:tier_1", 1);
        tier.adjust("test:ground", "test:material");
        tier
    }

    fn kind() -> CropKind {
        let mut kind = CropKind::new("test:kind", "stem");
        kind.adjust("test:base_seed");
        kind
    }

    fn crop() -> Crop {
        Crop::new(
            "other:alpha",
            "test:tier_1",
            "test:kind",
            Crafted::Entry {
                id: "test:crafted".to_owned(),
                data: None,
            },
        )
    }

    #[test]
    fn only_the_three_set_kinds_declare_themselves_simple() {
        assert!(Ingredient::Entry("a:b".to_owned()).simple());
        assert!(Ingredient::Tag("a:b".to_owned()).simple());
        assert!(Ingredient::TillingTool.simple());
        assert!(!Ingredient::ChargedContainer.simple());
        assert!(
            !Ingredient::CropComponent {
                crop: "a:b".to_owned(),
                role: Role::TierMaterial
            }
            .simple()
        );
    }

    #[test]
    fn a_ring_matches_in_any_order_and_the_count_has_to_be_exact() {
        let table = Table::default();
        let declared = vec![
            Ingredient::Entry("a:one".to_owned()),
            Ingredient::Entry("a:two".to_owned()),
        ];
        let reversed = vec![Stack::new("a:two", 1), Stack::new("a:one", 1)];
        assert!(assigns(&declared, &reversed, &table));

        let padded = vec![
            Stack::new("a:two", 1),
            Stack::new("a:one", 1),
            Stack::new("a:three", 1),
        ];
        assert!(!assigns(&declared, &padded, &table), "too many refuses");
        assert!(!assigns(&declared, &[Stack::new("a:one", 1)], &table));
    }

    #[test]
    fn one_offered_stack_cannot_satisfy_two_declared_ingredients() {
        let mut table = Table::default();
        table
            .tags
            .insert("a:tag".to_owned(), vec!["a:one".to_owned()]);
        let declared = vec![
            Ingredient::Tag("a:tag".to_owned()),
            Ingredient::Entry("a:one".to_owned()),
        ];
        assert!(!assigns(&declared, &[Stack::new("a:one", 1)], &table));
        assert!(assigns(
            &declared,
            &[Stack::new("a:one", 1), Stack::new("a:one", 1)],
            &table
        ));
    }

    #[test]
    fn the_reservoirs_report_what_they_are_short_of_and_by_how_much() {
        let recipe = EightStation {
            id: "test:ritual".to_owned(),
            altar: Ingredient::Entry("a:altar".to_owned()),
            ring: vec![Ingredient::Entry("a:one".to_owned())],
            materials: vec![Stack::new("a:fuel", 10), Stack::new("a:dust", 4)],
            result: Stack::new("a:result", 1),
        };
        let short = recipe.shortfall(&[Stack::new("a:fuel", 6)]);
        assert!(!short.covered());
        assert_eq!(
            short.missing,
            [Stack::new("a:fuel", 4), Stack::new("a:dust", 4)],
            "the shortfall is reported in the order the recipe declares"
        );

        let full = recipe.shortfall(&[
            Stack::new("a:fuel", 6),
            Stack::new("a:fuel", 6),
            Stack::new("a:dust", 4),
        ]);
        assert!(full.covered(), "two partial reservoirs add up");
    }

    #[test]
    fn the_eight_station_ring_reads_a_material_first_and_alternates() {
        let recipe = EightStation {
            id: "test:ritual".to_owned(),
            altar: Ingredient::Entry("a:altar".to_owned()),
            ring: vec![
                Ingredient::Entry("a:one".to_owned()),
                Ingredient::Entry("a:two".to_owned()),
            ],
            materials: vec![Stack::new("a:fuel", 1), Stack::new("a:dust", 1)],
            result: Stack::new("a:result", 1),
        };
        assert_eq!(
            recipe.interleaved(),
            [
                Ringed::Material("a:fuel".to_owned()),
                Ringed::Ingredient(Ingredient::Entry("a:one".to_owned())),
                Ringed::Material("a:dust".to_owned()),
                Ringed::Ingredient(Ingredient::Entry("a:two".to_owned())),
            ],
            "the alternation is the ring's own materials and ingredients"
        );
        assert_eq!(
            recipe.encoded_ingredients().len(),
            2,
            "the document encoding writes what its reader accepts"
        );
    }

    #[test]
    fn the_enchanting_level_is_the_smaller_of_the_two_inputs_and_is_capped() {
        let recipe = Enchanting {
            id: "test:enchanting".to_owned(),
            first: (Ingredient::Entry("a:one".to_owned()), 4),
            second: (Ingredient::Entry("a:two".to_owned()), 2),
            enchantment: "test:sharpness".to_owned(),
            maximum: 5,
        };
        let table = Table::default();
        let held = BTreeMap::new();
        let always = |_: &str, _: &str| true;

        let answer = recipe
            .apply(
                &Stack::new("a:one", 12),
                &Stack::new("a:two", 20),
                &Stack::new("a:sword", 1),
                &held,
                &always,
                &table,
            )
            .expect("both inputs cover three levels");
        assert_eq!(answer.level, 3);
        assert_eq!(answer.first, 12);
        assert_eq!(answer.second, 6);
        assert_eq!(answer.target, "a:sword");

        let capped = recipe
            .apply(
                &Stack::new("a:one", 400),
                &Stack::new("a:two", 400),
                &Stack::new("a:sword", 1),
                &held,
                &always,
                &table,
            )
            .expect("a level exists");
        assert_eq!(capped.level, 5);
    }

    #[test]
    fn a_plain_book_becomes_an_enchanted_one_and_too_little_input_is_refused() {
        let recipe = Enchanting {
            id: "test:enchanting".to_owned(),
            first: (Ingredient::Entry("a:one".to_owned()), 4),
            second: (Ingredient::Entry("a:two".to_owned()), 2),
            enchantment: "test:sharpness".to_owned(),
            maximum: 5,
        };
        let table = Table::default();
        let held = BTreeMap::new();
        let always = |_: &str, _: &str| true;

        let book = recipe
            .apply(
                &Stack::new("a:one", 4),
                &Stack::new("a:two", 2),
                &Stack::new(PLAIN_BOOK, 1),
                &held,
                &always,
                &table,
            )
            .expect("one level");
        assert_eq!(book.target, ENCHANTED_BOOK);

        assert_eq!(
            recipe.apply(
                &Stack::new("a:one", 3),
                &Stack::new("a:two", 2),
                &Stack::new(PLAIN_BOOK, 1),
                &held,
                &always,
                &table
            ),
            Err(Refused::NoLevel)
        );
    }

    #[test]
    fn the_compatibility_test_refuses_on_incompatibility_rather_than_on_compatibility() {
        let recipe = Enchanting {
            id: "test:enchanting".to_owned(),
            first: (Ingredient::Entry("a:one".to_owned()), 1),
            second: (Ingredient::Entry("a:two".to_owned()), 1),
            enchantment: "test:sharpness".to_owned(),
            maximum: 5,
        };
        let table = Table::default();
        let mut held = BTreeMap::new();
        held.insert("test:looting".to_owned(), 2);

        let compatible = |_: &str, _: &str| true;
        assert!(
            recipe
                .apply(
                    &Stack::new("a:one", 3),
                    &Stack::new("a:two", 3),
                    &Stack::new("a:sword", 1),
                    &held,
                    &compatible,
                    &table
                )
                .is_ok(),
            "a compatible enchantment already present does not refuse the craft"
        );

        let incompatible = |_: &str, _: &str| false;
        assert_eq!(
            recipe.apply(
                &Stack::new("a:one", 3),
                &Stack::new("a:two", 3),
                &Stack::new("a:sword", 1),
                &held,
                &incompatible,
                &table
            ),
            Err(Refused::Incompatible("test:looting".to_owned()))
        );
    }

    #[test]
    fn the_same_enchantment_at_the_level_or_above_is_refused_as_no_upgrade() {
        let recipe = Enchanting {
            id: "test:enchanting".to_owned(),
            first: (Ingredient::Entry("a:one".to_owned()), 1),
            second: (Ingredient::Entry("a:two".to_owned()), 1),
            enchantment: "test:sharpness".to_owned(),
            maximum: 5,
        };
        let table = Table::default();
        let always = |_: &str, _: &str| true;
        let mut held = BTreeMap::new();
        held.insert("test:sharpness".to_owned(), 3);

        assert_eq!(
            recipe.apply(
                &Stack::new("a:one", 3),
                &Stack::new("a:two", 3),
                &Stack::new("a:sword", 1),
                &held,
                &always,
                &table
            ),
            Err(Refused::NotAnUpgrade)
        );
        assert!(
            recipe
                .apply(
                    &Stack::new("a:one", 4),
                    &Stack::new("a:two", 4),
                    &Stack::new("a:sword", 1),
                    &held,
                    &always,
                    &table
                )
                .is_ok()
        );
    }

    #[test]
    fn a_charge_extraction_refuses_another_type_and_a_full_container_and_caps() {
        let recipe = ChargeExtraction {
            id: "test:extraction".to_owned(),
            input: Ingredient::Entry("a:one".to_owned()),
            charge_type: "test:charge".to_owned(),
            amount: 30,
            requirement: 100,
        };
        assert_eq!(recipe.extract(None), Some(30));
        assert_eq!(recipe.extract(Some(("test:charge", 80))), Some(100));
        assert_eq!(recipe.extract(Some(("test:charge", 100))), None);
        assert_eq!(recipe.extract(Some(("test:other", 0))), None);
    }

    #[test]
    fn a_tilling_recipe_returns_its_tool_one_point_worse_and_drops_a_destroyed_one() {
        assert_eq!(Tilling::remainder(0, 10), Some(1));
        assert_eq!(Tilling::remainder(8, 10), Some(9));
        assert_eq!(Tilling::remainder(9, 10), None);
    }

    #[test]
    fn an_emptying_recipe_matches_a_container_last_and_refuses_one_with_anything_after() {
        let mut table = Table::default();
        table
            .charges
            .insert("a:container".to_owned(), ("test:charge".to_owned(), 5.0));
        let recipe = Emptying {
            id: "test:emptying".to_owned(),
            result: "a:empty".to_owned(),
        };

        let last = vec![
            Stack::default(),
            Stack::default(),
            Stack::new("a:container", 1),
        ];
        assert!(recipe.matches(&last, &table));

        let first = vec![Stack::new("a:container", 1), Stack::new("a:other", 1)];
        assert!(!recipe.matches(&first, &table));
        assert!(!recipe.matches(&[Stack::default()], &table));

        let reversed = vec![Stack::new("a:other", 1), Stack::new("a:container", 1)];
        assert!(
            recipe.matches(&reversed, &table),
            "the walk only refuses what follows a container, which is the rule as specified"
        );

        let two = vec![Stack::new("a:container", 1), Stack::new("a:container", 1)];
        assert!(!recipe.matches(&two, &table), "one container, not two");
    }

    #[test]
    fn an_uncharged_container_is_not_a_charged_one() {
        let mut table = Table::default();
        table
            .charges
            .insert("a:container".to_owned(), ("test:charge".to_owned(), 0.0));
        assert!(!Ingredient::ChargedContainer.matches(&Stack::new("a:container", 1), &table));
    }

    #[test]
    fn a_crop_component_resolves_through_the_catalogue_rather_than_by_name() {
        let mut table = Table::default();
        table
            .components
            .insert("a:crop/TierMaterial".to_owned(), "test:material".to_owned());
        let ingredient = Ingredient::CropComponent {
            crop: "a:crop".to_owned(),
            role: Role::TierMaterial,
        };
        assert!(ingredient.matches(&Stack::new("test:material", 1), &table));
        assert!(!ingredient.matches(&Stack::new("test:other", 1), &table));
    }

    #[test]
    fn a_disabled_crop_derives_nothing_at_all() {
        let mut crop = crop();
        crop.enabled = false;
        assert_eq!(derive(&crop, &tiers(), &kind(), true, &|_| 1).len(), 0);
    }

    #[test]
    fn an_enabled_crop_derives_the_ritual_and_the_conversion_and_not_the_grid() {
        let derived = derive(&crop(), &tiers(), &kind(), false, &|_| 1);
        assert_eq!(derived.len(), 2);
        assert_eq!(
            derived[0].id(),
            format!("{}:alpha{RITUAL_SUFFIX}", crate::MOD_ID)
        );
        assert_eq!(
            derived[1].id(),
            format!("{}:alpha{CONVERSION_SUFFIX}", crate::MOD_ID)
        );

        let with_grid = derive(&crop(), &tiers(), &kind(), true, &|_| 1);
        assert_eq!(with_grid.len(), 3);
        assert_eq!(
            with_grid[1].id(),
            format!("{}:alpha{GRID_SUFFIX}", crate::MOD_ID)
        );
    }

    #[test]
    fn the_ritual_ring_alternates_the_crafting_material_first_and_produces_one_seed() {
        let derived = derive(&crop(), &tiers(), &kind(), false, &|_| 1);
        let DerivedRecipe::RitualSeed(ritual) = &derived[0] else {
            panic!("the first derived recipe is the ritual");
        };
        assert_eq!(ritual.ring.len(), 8);
        assert_eq!(ritual.ring[0], Ingredient::Entry("test:crafted".to_owned()));
        assert_eq!(
            ritual.ring[1],
            Ingredient::Entry("test:material".to_owned())
        );
        assert_eq!(ritual.altar, Ingredient::Entry("test:base_seed".to_owned()));
        assert_eq!(
            ritual.result,
            Stack::new("rvl_321_counterpart:alpha_seeds", 1)
        );
        assert!(!ritual.copies_data);
    }

    #[test]
    fn a_grid_seed_recipe_centres_the_base_seed_with_material_edges() {
        let derived = derive(&crop(), &tiers(), &kind(), true, &|_| 1);
        let DerivedRecipe::GridSeed { grid, .. } = &derived[1] else {
            panic!("the second derived recipe is the grid one");
        };
        assert_eq!(grid.len(), 9);
        assert_eq!(
            grid[4].as_ref(),
            Some(&Ingredient::Entry("test:base_seed".to_owned()))
        );
        assert_eq!(
            grid[1].as_ref(),
            Some(&Ingredient::Entry("test:material".to_owned()))
        );
        assert_eq!(
            grid[0].as_ref(),
            Some(&Ingredient::Entry("test:crafted".to_owned()))
        );
    }

    #[test]
    fn a_crafting_material_that_resolves_to_nothing_keeps_only_the_conversion() {
        let mut crop = crop();
        crop.crafted = Crafted::Tag("test:empty_tag".to_owned());
        let derived = derive(&crop, &tiers(), &kind(), true, &|_| 0);
        assert_eq!(derived.len(), 1);
        assert_eq!(
            derived[0].id(),
            format!("{}:alpha{CONVERSION_SUFFIX}", crate::MOD_ID)
        );

        let resolving = derive(&crop, &tiers(), &kind(), true, &|_| 2);
        assert_eq!(resolving.len(), 3);
    }

    #[test]
    fn a_tier_with_no_material_and_a_kind_with_no_base_seed_derive_no_seed_recipe() {
        let bare_tier = Tier::new("test:tier_1", 1);
        assert_eq!(derive(&crop(), &bare_tier, &kind(), true, &|_| 1).len(), 1);

        let bare_kind = CropKind::new("test:kind", "stem");
        assert_eq!(derive(&crop(), &tiers(), &bare_kind, true, &|_| 1).len(), 1);
    }

    #[test]
    fn a_seed_conversion_recipe_turns_one_seed_into_two_produce() {
        let derived = derive(&crop(), &tiers(), &kind(), false, &|_| 1);
        let DerivedRecipe::Conversion { input, result, .. } = &derived[1] else {
            panic!("the last derived recipe is the conversion");
        };
        assert_eq!(
            input,
            &Ingredient::Entry("rvl_321_counterpart:alpha_seeds".to_owned())
        );
        assert_eq!(result, &Stack::new("rvl_321_counterpart:alpha_material", 2));
    }

    struct Answers {
        enabled: bool,
        grid: bool,
    }

    impl Conditions for Answers {
        fn crop_enabled(&self, _crop: &str) -> bool {
            self.enabled
        }
        fn modifier_enabled(&self, _modifier: &str) -> bool {
            self.enabled
        }
        fn crop_crafted(&self, _crop: &str) -> bool {
            self.enabled
        }
        fn grid_seeds(&self) -> bool {
            self.grid
        }
    }

    #[test]
    fn each_of_the_four_conditions_reads_its_own_question() {
        let against = Answers {
            enabled: true,
            grid: false,
        };
        assert!(Condition::CropEnabled("a:crop".to_owned()).holds(&against));
        assert!(Condition::ModifierEnabled("a:mod".to_owned()).holds(&against));
        assert!(Condition::CropCrafted("a:crop".to_owned()).holds(&against));
        assert!(!Condition::GridSeeds.holds(&against));
    }
}
