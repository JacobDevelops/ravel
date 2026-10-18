//! The three declared things a dependent supplies: a tier, a crop kind, a crop.
//!
//! This is data an extension writes, so the defaults are part of the contract
//! rather than an implementation choice. Everything filled in during the adjust
//! pass is optional here and stays empty until that pass runs, which is what
//! makes the two-pass shape of [`crate::registries`] observable rather than
//! merely documented.

use ravel_sdk::content::ContentId;
use rvl_042_counterpart::Colour;

/// The opacity a crop's three colours are packed with.
pub const CROP_OPACITY: u8 = 255;

/// The opacity a creature charge type packs its colour with.
///
/// Two opacity constants in one entry. They are deliberately not unified: a
/// dependent reads a packed value back and would see a different number.
pub const CHARGE_OPACITY: u8 = 204;

/// A named material tier.
#[derive(Clone, Debug, PartialEq)]
pub struct Tier {
    /// The full identifier, namespace and path.
    pub id: ContentId,
    /// The ordinal this tier sorts and computes with.
    pub value: u32,
    /// The tier's own colour.
    pub colour: Colour,
    /// The colour this tier's text is drawn in.
    pub text_colour: Colour,
    /// Whether a crop of this tier answers the fertilisable test.
    pub fertilisable: bool,
    /// Whether a crop of this tier may drop a second produce at all.
    pub second_drop: bool,
    /// The chance a crop of this tier drops a second produce.
    pub second_drop_chance: f64,
    /// The ground block that belongs to this tier, filled in on adjust.
    pub ground: Option<ContentId>,
    /// The material item that belongs to this tier, filled in on adjust.
    pub material: Option<ContentId>,
}

impl Tier {
    /// Declares a tier holding the shipped defaults.
    #[must_use]
    pub fn new(id: ContentId, value: u32) -> Self {
        Self {
            id,
            value,
            colour: Colour::new(CROP_OPACITY, 255, 255, 255),
            text_colour: Colour::new(CROP_OPACITY, 255, 255, 255),
            fertilisable: true,
            second_drop: true,
            second_drop_chance: 0.1,
            ground: None,
            material: None,
        }
    }

    /// Points the tier at the ground and material that belong to it.
    ///
    /// This is the adjust pass. A tier that never reaches it has no ground, and
    /// [`Self::is_effective_ground`] then answers false for every block.
    pub fn adjust(&mut self, ground: ContentId, material: ContentId) {
        self.ground = Some(ground);
        self.material = Some(material);
    }

    /// Whether a block is exactly this tier's own ground.
    ///
    /// Exact identity, with an unfilled ground answering false. There is no
    /// counts-as-any-ground escape at this revision, and adding one would make
    /// every crop drop-eligible everywhere.
    #[must_use]
    pub fn is_effective_ground(&self, block: Option<&ContentId>) -> bool {
        match (self.ground.as_ref(), block) {
            (Some(ground), Some(block)) => ground == block,
            _ => false,
        }
    }
}

/// A crop kind: what a crop's stem looks like and what its seed is crafted from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CropKind {
    /// The full identifier.
    pub id: ContentId,
    /// The stem model this kind's crops are drawn with.
    pub stem: String,
    /// The item crafting a seed of this kind consumes, filled in on adjust.
    pub base_seed: Option<ContentId>,
}

impl CropKind {
    /// Declares a kind holding no base seed yet.
    #[must_use]
    pub fn new(id: ContentId, stem: impl Into<String>) -> Self {
        Self {
            id,
            stem: stem.into(),
            base_seed: None,
        }
    }

    /// Points the kind at the seed its crops are crafted from.
    pub fn adjust(&mut self, base_seed: ContentId) {
        self.base_seed = Some(base_seed);
    }
}

/// What a crop is crafted from: one entry, or any member of a tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Crafted {
    /// A single entry, with optional item-level data narrowing it.
    Entry {
        /// The entry identifier.
        id: ContentId,
        /// The item-level data the entry has to carry, if any.
        data: Option<String>,
    },
    /// Any member of a tag, resolved lazily.
    Tag(ContentId),
    /// Nothing: the third state of section fifteen.
    Nothing,
}

impl Crafted {
    /// Whether this resolves to anything at all.
    ///
    /// A tag with no members resolves empty, which leaves the crop enabled and
    /// registered and only removes its two seed recipes.
    #[must_use]
    pub fn resolves(&self, members: &dyn Fn(&ContentId) -> usize) -> bool {
        match self {
            Self::Entry { .. } => true,
            Self::Tag(tag) => members(tag) > 0,
            Self::Nothing => false,
        }
    }
}

/// The three recipe kinds derived per crop, each separately switchable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Derived {
    /// The ritual seed recipe.
    pub ritual_seed: bool,
    /// The plain-grid seed recipe.
    pub grid_seed: bool,
    /// The seed-conversion recipe.
    pub conversion: bool,
}

impl Default for Derived {
    fn default() -> Self {
        Self {
            ritual_seed: true,
            grid_seed: true,
            conversion: true,
        }
    }
}

impl Derived {
    /// Combines a crop's own switches with its extension's by conjunction.
    ///
    /// An extension that opts out cannot be overridden by a crop that opts in,
    /// which is why this is applied at the moment a crop is registered rather
    /// than read separately at derivation time.
    #[must_use]
    pub const fn narrowed_by(self, extension: Self) -> Self {
        Self {
            ritual_seed: self.ritual_seed && extension.ritual_seed,
            grid_seed: self.grid_seed && extension.grid_seed,
            conversion: self.conversion && extension.conversion,
        }
    }
}

/// Which crop components this entry registers itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registration {
    /// Registers a crop block.
    pub block: bool,
    /// Registers a material item.
    pub material: bool,
    /// Registers a seed item.
    pub seed: bool,
}

/// A crop, with the defaults it is declared holding.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Crop {
    /// The full identifier. Its path is the crop's name in the global space.
    pub id: ContentId,
    /// The tier identifier this crop belongs to.
    pub tier: ContentId,
    /// The crop kind identifier this crop belongs to.
    pub kind: ContentId,
    /// What crafting a seed of this crop consumes.
    pub crafted: Crafted,
    /// The crop's own second-drop chance, or `None` to defer to the tier.
    pub second_drop_chance: Option<f64>,
    /// Whether this crop reads the ground below as its own tier's.
    pub respects_effective_ground: bool,
    /// Whether this crop contributes recipes and appears in the listing.
    pub enabled: bool,
    /// The components this entry registers itself.
    pub registration: Registration,
    /// Whether the crop's items are drawn with a glint.
    pub glint: bool,
    /// The three derived recipe kinds.
    pub derived: Derived,
    /// The biomes this crop grows in, empty meaning every biome.
    pub biomes: Vec<String>,
    /// The block that must sit two below the crop, if any.
    pub foundation: Option<ContentId>,
    /// The block supplied for this crop instead of a registered one.
    pub block: Option<ContentId>,
    /// The material item supplied instead of a registered one.
    pub material: Option<ContentId>,
    /// The seed item supplied instead of a registered one.
    pub seed: Option<ContentId>,
    /// The colour packed into the flower.
    pub flower_colour: Colour,
    /// The colour packed into the material.
    pub material_colour: Colour,
    /// The colour packed into the seed.
    pub seed_colour: Colour,
}

impl Crop {
    /// Declares a crop holding every shipped default.
    #[must_use]
    pub fn new(id: ContentId, tier: ContentId, kind: ContentId, crafted: Crafted) -> Self {
        let untinted = Colour::new(CROP_OPACITY, 0, 0, 0);
        Self {
            id,
            tier,
            kind,
            crafted,
            second_drop_chance: None,
            respects_effective_ground: true,
            enabled: true,
            registration: Registration {
                block: true,
                material: true,
                seed: true,
            },
            glint: false,
            derived: Derived::default(),
            biomes: Vec::new(),
            foundation: None,
            block: None,
            material: None,
            seed: None,
            flower_colour: untinted,
            material_colour: untinted,
            seed_colour: untinted,
        }
    }

    /// The crop's path, which is its name in the single global crop space.
    #[must_use]
    pub fn path(&self) -> &str {
        self.id.path()
    }

    /// Writes one colour into all three of a crop's colours.
    ///
    /// A zero colour means do not tint, so a crop cannot ask for pure black.
    pub fn set_colour(&mut self, red: u8, green: u8, blue: u8) {
        let colour = Colour::new(CROP_OPACITY, red, green, blue);
        self.flower_colour = colour;
        self.material_colour = colour;
        self.seed_colour = colour;
    }

    /// Whether a packed colour asks for no tint at all.
    #[must_use]
    pub const fn is_untinted(colour: Colour) -> bool {
        colour.red == 0 && colour.green == 0 && colour.blue == 0
    }

    /// Supplies a block, which stops this entry registering one.
    ///
    /// `also_register` is the caller's escape: a dependent that wants both its
    /// own block and a registration asks for it explicitly.
    pub fn supply_block(&mut self, block: ContentId, also_register: bool) {
        self.block = Some(block);
        self.registration.block = also_register;
    }

    /// Supplies a material item, which stops this entry registering one.
    pub fn supply_material(&mut self, material: ContentId, also_register: bool) {
        self.material = Some(material);
        self.registration.material = also_register;
    }

    /// Supplies a seed item, which stops this entry registering one.
    pub fn supply_seed(&mut self, seed: ContentId, also_register: bool) {
        self.seed = Some(seed);
        self.registration.seed = also_register;
    }

    /// The second-drop chance in force, resolving an absent crop override.
    #[must_use]
    pub fn own_or_tier_chance(&self, tier: &Tier) -> f64 {
        self.second_drop_chance.unwrap_or(tier.second_drop_chance)
    }
}

/// A creature charge type, which claims creature identifiers globally.
#[derive(Clone, Debug, PartialEq)]
pub struct ChargeType {
    /// The full identifier.
    pub id: ContentId,
    /// The creatures this type claims.
    pub creatures: Vec<ContentId>,
    /// How much charge a full container of this type holds.
    pub requirement: f64,
    /// The type's colour, packed at the charge opacity.
    pub colour: Colour,
}

impl ChargeType {
    /// Declares a charge type, packing its colour at the charge opacity.
    #[must_use]
    pub fn new(id: ContentId, requirement: f64, red: u8, green: u8, blue: u8) -> Self {
        Self {
            id,
            creatures: Vec::new(),
            requirement,
            colour: Colour::new(CHARGE_OPACITY, red, green, blue),
        }
    }

    /// Claims a creature for this type.
    pub fn claim(&mut self, creature: ContentId) {
        self.creatures.push(creature);
    }
}

/// The six tiers this entry ships.
///
/// Five numbered one to five with matching values, and one side tier for the
/// elemental crops carrying value one. The side tier therefore sorts with the
/// first tier, and after the adjust pass shares its ground and material, so an
/// elemental crop is drop-eligible on the first tier's ground.
#[must_use]
pub fn shipped_tiers() -> Vec<Tier> {
    let mut tiers: Vec<Tier> = (1..=5)
        .map(|value| {
            Tier::new(
                format!("{}:tier_{value}", crate::MOD_ID)
                    .parse()
                    .expect("a legal tier identity"),
                value,
            )
        })
        .collect();
    tiers.push(Tier::new(
        format!("{}:tier_side", crate::MOD_ID)
            .parse()
            .expect("a legal tier identity"),
        1,
    ));
    tiers
}

/// The two crop kinds this entry ships, differing only in their base seed.
#[must_use]
pub fn shipped_kinds() -> Vec<CropKind> {
    vec![
        CropKind::new(
            format!("{}:kind_material", crate::MOD_ID)
                .parse()
                .expect("a legal kind identity"),
            "stem_material",
        ),
        CropKind::new(
            format!("{}:kind_creature", crate::MOD_ID)
                .parse()
                .expect("a legal kind identity"),
            "stem_creature",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_tiers_ship_and_the_side_tier_shares_the_first_tiers_ordinal() {
        let tiers = shipped_tiers();
        assert_eq!(tiers.len(), 6);
        assert_eq!(tiers[0].value, 1);
        assert_eq!(tiers[4].value, 5);
        assert_eq!(tiers[5].value, 1, "the side tier carries value one");
    }

    #[test]
    fn a_tier_ships_fertilisable_with_second_drops_at_a_tenth() {
        let tier = Tier::new(("test:tier").parse().expect("a legal tier identity"), 3);
        assert!(tier.fertilisable);
        assert!(tier.second_drop);
        assert!((tier.second_drop_chance - 0.1).abs() < f64::EPSILON);
        assert!(tier.ground.is_none());
        assert!(tier.material.is_none());
    }

    #[test]
    fn effective_ground_is_exact_identity_and_an_unfilled_tier_answers_false() {
        let mut tier = Tier::new(("test:tier").parse().expect("a legal tier identity"), 1);
        let ground = "test:ground".parse().expect("a legal ground identity");
        assert!(!tier.is_effective_ground(Some(&ground)));
        tier.adjust(
            "test:ground".parse().expect("a legal ground identity"),
            "test:material".parse().expect("a legal material identity"),
        );
        assert!(tier.is_effective_ground(Some(&ground)));
        let other = "test:other_ground"
            .parse()
            .expect("a legal ground identity");
        assert!(!tier.is_effective_ground(Some(&other)));
        assert!(!tier.is_effective_ground(None));
    }

    #[test]
    fn a_crop_ships_deferring_to_its_tier_and_registering_all_three_things() {
        let crop = Crop::new(
            ("test:crop").parse().expect("a legal crop identity"),
            ("test:tier").parse().expect("a legal tier identity"),
            ("test:kind").parse().expect("a legal crop kind identity"),
            Crafted::Nothing,
        );
        assert_eq!(crop.second_drop_chance, None);
        assert!(crop.respects_effective_ground);
        assert!(crop.enabled);
        assert!(crop.registration.block && crop.registration.material && crop.registration.seed);
        assert!(!crop.glint);
        assert_eq!(crop.derived, Derived::default());
        assert_eq!(crop.biomes.len(), 0);
    }

    #[test]
    fn an_absent_override_reads_the_tiers_chance_and_an_own_value_wins() {
        let mut tier = Tier::new(("test:tier").parse().expect("a legal tier identity"), 1);
        tier.second_drop_chance = 0.25;
        let mut crop = Crop::new(
            ("test:crop").parse().expect("a legal crop identity"),
            ("test:tier").parse().expect("a legal tier identity"),
            ("test:kind").parse().expect("a legal crop kind identity"),
            Crafted::Nothing,
        );
        assert!((crop.own_or_tier_chance(&tier) - 0.25).abs() < f64::EPSILON);
        crop.second_drop_chance = Some(0.4);
        assert!((crop.own_or_tier_chance(&tier) - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn one_colour_is_written_into_three_and_a_zero_colour_asks_for_no_tint() {
        let mut crop = Crop::new(
            ("test:crop").parse().expect("a legal crop identity"),
            ("test:tier").parse().expect("a legal tier identity"),
            ("test:kind").parse().expect("a legal crop kind identity"),
            Crafted::Nothing,
        );
        assert!(Crop::is_untinted(crop.flower_colour));
        crop.set_colour(10, 20, 30);
        assert_eq!(crop.flower_colour, crop.material_colour);
        assert_eq!(crop.material_colour, crop.seed_colour);
        assert_eq!(crop.flower_colour.opacity, CROP_OPACITY);
        assert!(!Crop::is_untinted(crop.flower_colour));

        crop.set_colour(0, 0, 0);
        assert!(
            Crop::is_untinted(crop.flower_colour),
            "pure black is no tint"
        );
    }

    #[test]
    fn a_charge_type_packs_its_colour_at_the_other_opacity() {
        let charge = ChargeType::new(
            "test:charge".parse().expect("a legal charge identity"),
            100.0,
            10,
            20,
            30,
        );
        assert_eq!(charge.colour.opacity, CHARGE_OPACITY);
        assert_ne!(CHARGE_OPACITY, CROP_OPACITY);
    }

    #[test]
    fn supplying_a_block_stops_the_registration_unless_the_caller_asks() {
        let mut crop = Crop::new(
            ("test:crop").parse().expect("a legal crop identity"),
            ("test:tier").parse().expect("a legal tier identity"),
            ("test:kind").parse().expect("a legal crop kind identity"),
            Crafted::Nothing,
        );
        crop.supply_block(
            "test:own_block".parse().expect("a legal block identity"),
            false,
        );
        assert!(!crop.registration.block);
        crop.supply_material(
            "test:own_material"
                .parse()
                .expect("a legal material identity"),
            true,
        );
        assert!(crop.registration.material);
        crop.supply_seed(
            "test:own_seed".parse().expect("a legal seed identity"),
            false,
        );
        assert!(!crop.registration.seed);
    }

    #[test]
    fn an_extension_opting_out_cannot_be_overridden_by_a_crop_opting_in() {
        let crop = Derived::default();
        let extension = Derived {
            ritual_seed: false,
            grid_seed: true,
            conversion: true,
        };
        let narrowed = crop.narrowed_by(extension);
        assert!(!narrowed.ritual_seed);
        assert!(narrowed.grid_seed);
        assert!(narrowed.conversion);
    }

    #[test]
    fn a_tag_with_no_members_resolves_empty_and_an_entry_always_resolves() {
        let none = |_: &ContentId| 0;
        let some = |_: &ContentId| 3;
        assert!(!Crafted::Tag("test:tag".parse().expect("a legal tag identity")).resolves(&none));
        assert!(Crafted::Tag("test:tag".parse().expect("a legal tag identity")).resolves(&some));
        assert!(
            Crafted::Entry {
                id: "test:entry".parse().expect("a legal entry identity"),
                data: None
            }
            .resolves(&none)
        );
        assert!(!Crafted::Nothing.resolves(&some));
    }

    #[test]
    fn a_crops_path_is_read_without_its_namespace() {
        let crop = Crop::new(
            ("other:the_name").parse().expect("a legal crop identity"),
            ("test:tier").parse().expect("a legal tier identity"),
            ("test:kind").parse().expect("a legal crop kind identity"),
            Crafted::Nothing,
        );
        assert_eq!(crop.path(), "the_name");
    }
}
