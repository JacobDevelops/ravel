//! The three registries, the window a crop may be registered in, and the order
//! the extension callbacks run in.
//!
//! This is the contract an extension is written against, so the refusal rules
//! and the ordering are the behaviour rather than an internal arrangement. The
//! three tables are insertion-ordered and keyed by identifier, and the refusal
//! rule is deliberately not uniform across them.
//!
//! Nothing here is discovered. The original finds its extensions by scanning
//! every loaded mod's annotation index and installs its tables by a write that
//! goes around the published surface; Ravel has neither, and neither is
//! behaviour a dependent can observe. What a dependent observes is this
//! ordering and these refusals, with this entry's own contribution ordered
//! first.

use std::collections::BTreeMap;
use std::fmt;

use crate::model::{ChargeType, Crop, CropKind, Derived, Tier};

/// Why a registration was refused.
///
/// Every refusal is recorded and skipped. Nothing raises, and the caller is not
/// told, so an extension whose crop collided finds out by the crop's absence.
/// Ravel publishes no logging seam a mod may write to, so the recorded list and
/// the sentence each entry renders as are what stands in for the log: a host
/// that has one writes these, and a test reads them either way.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// A crop whose path collides with a registered crop's, in any namespace.
    CropPath(String),
    /// A tier, kind, modifier or charge type colliding on its full identifier.
    Identifier(String),
    /// A charge type claiming a creature another type already claims.
    Creature(String),
    /// A crop registered outside the window crops may be registered in.
    Closed(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CropPath(id) => {
                write!(formatter, "{id} was refused: a crop already has its path")
            }
            Self::Identifier(id) => write!(formatter, "{id} was refused: it is already registered"),
            Self::Creature(id) => {
                write!(formatter, "{id} was refused: another type claims it")
            }
            Self::Closed(id) => write!(
                formatter,
                "{id} was refused: crops may only be registered inside the window"
            ),
        }
    }
}

/// The suffix each per-crop registration is named with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    /// The crop's block.
    Block,
    /// The crop's material item.
    Material,
    /// The crop's seed item.
    Seed,
}

impl Role {
    /// The fixed suffix this role appends to a crop's path.
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Block => "_crop",
            Self::Material => "_material",
            Self::Seed => "_seeds",
        }
    }
}

/// The identifier a crop's registration takes.
///
/// Always in this entry's namespace, never in the declaring extension's, which
/// is why a crop path is a single global space and why an extension's own
/// recipe documents address its crops here.
#[must_use]
pub fn registered_id(crop: &Crop, role: Role) -> String {
    format!("{}:{}{}", crate::MOD_ID, crop.path(), role.suffix())
}

/// A slotted modifier, which registers on full-identifier collision only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Modifier {
    /// The full identifier.
    pub id: String,
    /// The kinds of target this modifier may be applied to.
    pub kinds: Vec<String>,
    /// The modifier's own tier value.
    pub tier: u32,
}

impl Modifier {
    /// Declares a modifier.
    #[must_use]
    pub fn new(id: &str, tier: u32, kinds: &[&str]) -> Self {
        Self {
            id: id.to_owned(),
            kinds: kinds.iter().map(|kind| (*kind).to_owned()).collect(),
            tier,
        }
    }
}

/// The three registries and the state that decides what may enter them.
#[derive(Debug, Default)]
pub struct Registries {
    crops: Vec<Crop>,
    tiers: Vec<Tier>,
    kinds: Vec<CropKind>,
    modifiers: Vec<Modifier>,
    charges: Vec<ChargeType>,
    claimed: BTreeMap<String, String>,
    open: bool,
    refused: Vec<Refusal>,
}

impl Registries {
    /// Builds the three tables empty, with the crop window closed.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens the window crops may be registered in.
    ///
    /// Opened immediately before extensions are asked for their crops and
    /// closed immediately after, and enforced for crops alone: a modifier and a
    /// charge type have no such window.
    pub const fn open(&mut self) {
        self.open = true;
    }

    /// Closes the window.
    pub const fn close(&mut self) {
        self.open = false;
    }

    /// Whether the crop window is open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Registers a crop, narrowing its derived switches by its extension's.
    ///
    /// The conjunction happens here rather than at derivation time, which is
    /// what makes an extension's opt-out final.
    pub fn register_crop(&mut self, mut crop: Crop, extension: Derived) -> bool {
        if !self.open {
            self.refused.push(Refusal::Closed(crop.id.to_string()));
            return false;
        }
        if self.crops.iter().any(|held| held.path() == crop.path()) {
            self.refused.push(Refusal::CropPath(crop.id.to_string()));
            return false;
        }
        crop.derived = crop.derived.narrowed_by(extension);
        self.crops.push(crop);
        true
    }

    /// Registers a tier, refused on full identifier collision only.
    pub fn register_tier(&mut self, tier: Tier) -> bool {
        if self.tiers.iter().any(|held| held.id == tier.id) {
            self.refused.push(Refusal::Identifier(tier.id.to_string()));
            return false;
        }
        self.tiers.push(tier);
        true
    }

    /// Registers a crop kind, refused on full identifier collision only.
    pub fn register_kind(&mut self, kind: CropKind) -> bool {
        if self.kinds.iter().any(|held| held.id == kind.id) {
            self.refused.push(Refusal::Identifier(kind.id.to_string()));
            return false;
        }
        self.kinds.push(kind);
        true
    }

    /// Registers a slotted modifier, refused on full identifier collision only.
    pub fn register_modifier(&mut self, modifier: Modifier) -> bool {
        if self.modifiers.iter().any(|held| held.id == modifier.id) {
            self.refused.push(Refusal::Identifier(modifier.id));
            return false;
        }
        self.modifiers.push(modifier);
        true
    }

    /// Registers a charge type.
    ///
    /// Refused on its own identifier, and refused again if any creature it
    /// claims is already claimed. Claiming is global and first-wins, so a type
    /// that overlaps an earlier one is dropped whole rather than partly.
    pub fn register_charge(&mut self, charge: ChargeType) -> bool {
        if self.charges.iter().any(|held| held.id == charge.id) {
            self.refused.push(Refusal::Identifier(charge.id));
            return false;
        }
        if let Some(taken) = charge
            .creatures
            .iter()
            .find(|creature| self.claimed.contains_key(*creature))
        {
            self.refused.push(Refusal::Creature(taken.clone()));
            return false;
        }
        for creature in &charge.creatures {
            self.claimed.insert(creature.clone(), charge.id.clone());
        }
        self.charges.push(charge);
        true
    }

    /// Re-sorts the crop table by tier value, stably.
    ///
    /// Run once after block registration. Within a tier the declaration order
    /// survives and across tiers the table ascends, and everything afterwards —
    /// item registration, the creative listing, the derived recipes — reads
    /// this order rather than declaration order.
    pub fn sort_crops(&mut self) {
        let values: BTreeMap<ravel_sdk::content::ContentId, u32> = self
            .tiers
            .iter()
            .map(|tier| (tier.id.clone(), tier.value))
            .collect();
        self.crops
            .sort_by_key(|crop| values.get(&crop.tier).copied().unwrap_or(u32::MAX));
    }

    /// The crop table, in the order everything after block registration reads.
    #[must_use]
    pub fn crops(&self) -> &[Crop] {
        &self.crops
    }

    /// The crop table for the adjust pass.
    pub const fn crops_mut(&mut self) -> &mut Vec<Crop> {
        &mut self.crops
    }

    /// The tier table.
    #[must_use]
    pub fn tiers(&self) -> &[Tier] {
        &self.tiers
    }

    /// The tier table for the adjust pass.
    pub const fn tiers_mut(&mut self) -> &mut Vec<Tier> {
        &mut self.tiers
    }

    /// The named tier, if one is registered under that identifier.
    #[must_use]
    pub fn tier(&self, id: &ravel_sdk::content::ContentId) -> Option<&Tier> {
        self.tiers.iter().find(|tier| &tier.id == id)
    }

    /// The crop kind table.
    #[must_use]
    pub fn kinds(&self) -> &[CropKind] {
        &self.kinds
    }

    /// The crop kind table for the adjust pass.
    pub const fn kinds_mut(&mut self) -> &mut Vec<CropKind> {
        &mut self.kinds
    }

    /// The named crop kind, if one is registered under that identifier.
    #[must_use]
    pub fn kind(&self, id: &ravel_sdk::content::ContentId) -> Option<&CropKind> {
        self.kinds.iter().find(|kind| &kind.id == id)
    }

    /// The slotted modifier table.
    #[must_use]
    pub fn modifiers(&self) -> &[Modifier] {
        &self.modifiers
    }

    /// The creature charge type table.
    #[must_use]
    pub fn charges(&self) -> &[ChargeType] {
        &self.charges
    }

    /// Everything that was refused, in the order it was refused.
    #[must_use]
    pub fn refused(&self) -> &[Refusal] {
        &self.refused
    }
}

/// What an extension contributes, and when.
///
/// The two-pass shape is the contract: an extension may reach for another
/// extension's contribution in an adjust callback and must not in a register
/// callback, because nothing guarantees the other extension has run yet.
pub trait Extension {
    /// The extension's own namespace.
    fn namespace(&self) -> &str;

    /// The three switches this extension opts its crops out of.
    fn derived(&self) -> Derived {
        Derived::default()
    }

    /// Reads the settings in force. Run once, in the first construction pass.
    fn configure(&mut self, _settings: &crate::settings::Settings) {}

    /// Contributes charge types, immediately after this extension configures.
    fn register_charges(&mut self, _registries: &mut Registries) {}

    /// Adjusts charge types, in a second pass over every extension.
    fn adjust_charges(&mut self, _registries: &mut Registries) {}

    /// Contributes crops, inside the open window.
    fn register_crops(&mut self, _registries: &mut Registries) {}

    /// Adjusts crops, during item registration.
    fn adjust_crops(&mut self, _registries: &mut Registries) {}

    /// Contributes slotted modifiers, during item registration.
    fn register_modifiers(&mut self, _registries: &mut Registries) {}

    /// Adjusts slotted modifiers, after the modifier items are created.
    fn adjust_modifiers(&mut self, _registries: &mut Registries) {}
}

/// A step the ordered run took, in the order it took it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Step {
    /// An extension read the settings.
    Configure(String),
    /// An extension contributed charge types.
    RegisterCharges(String),
    /// An extension adjusted charge types.
    AdjustCharges(String),
    /// The crop window opened.
    Open,
    /// An extension contributed crops.
    RegisterCrops(String),
    /// A default block was created for every crop that wanted one and had none.
    DefaultBlocks,
    /// The crop table was re-sorted by tier value.
    Sort,
    /// The crop window closed.
    Close,
    /// The crop material and seed items were created.
    CropItems,
    /// An extension adjusted crops.
    AdjustCrops(String),
    /// An extension contributed slotted modifiers.
    RegisterModifiers(String),
    /// The modifier items were created.
    ModifierItems,
    /// An extension adjusted slotted modifiers.
    AdjustModifiers(String),
}

/// Runs the construction pass: configure and charge types interleaved, then a
/// second pass adjusting charge types.
pub fn construct(
    registries: &mut Registries,
    extensions: &mut [&mut dyn Extension],
    settings: &crate::settings::Settings,
) -> Vec<Step> {
    let mut taken = Vec::new();
    for extension in extensions.iter_mut() {
        let name = extension.namespace().to_owned();
        extension.configure(settings);
        taken.push(Step::Configure(name.clone()));
        extension.register_charges(registries);
        taken.push(Step::RegisterCharges(name));
    }
    for extension in extensions.iter_mut() {
        let name = extension.namespace().to_owned();
        extension.adjust_charges(registries);
        taken.push(Step::AdjustCharges(name));
    }
    taken
}

/// Runs block registration: the window opens, crops arrive, defaults are made,
/// the table is sorted, the window closes.
pub fn register_blocks(
    registries: &mut Registries,
    extensions: &mut [&mut dyn Extension],
) -> Vec<Step> {
    let mut taken = vec![Step::Open];
    registries.open();
    for extension in extensions.iter_mut() {
        let name = extension.namespace().to_owned();
        extension.register_crops(registries);
        taken.push(Step::RegisterCrops(name));
    }
    for crop in registries.crops_mut() {
        if crop.registration.block && crop.block.is_none() {
            crop.block = Some(registered_id(crop, Role::Block));
        }
    }
    taken.push(Step::DefaultBlocks);
    registries.sort_crops();
    taken.push(Step::Sort);
    registries.close();
    taken.push(Step::Close);
    taken
}

/// Runs item registration: crop items, then crop adjustment, then modifiers.
pub fn register_items(
    registries: &mut Registries,
    extensions: &mut [&mut dyn Extension],
) -> Vec<Step> {
    let mut taken = vec![Step::CropItems];
    for extension in extensions.iter_mut() {
        let name = extension.namespace().to_owned();
        extension.adjust_crops(registries);
        taken.push(Step::AdjustCrops(name));
    }
    for extension in extensions.iter_mut() {
        let name = extension.namespace().to_owned();
        extension.register_modifiers(registries);
        taken.push(Step::RegisterModifiers(name));
    }
    taken.push(Step::ModifierItems);
    for extension in extensions.iter_mut() {
        let name = extension.namespace().to_owned();
        extension.adjust_modifiers(registries);
        taken.push(Step::AdjustModifiers(name));
    }
    taken
}

/// The identifiers a whole registration run produces, in the order it produces
/// them: every crop block, then every material item, then every seed item.
///
/// All materials come before all seeds across the catalogue, not crop by crop,
/// and the order is the sorted one rather than the declaration one.
#[must_use]
pub fn registered_order(registries: &Registries) -> Vec<String> {
    let mut ids: Vec<String> = registries
        .crops()
        .iter()
        .filter(|crop| crop.registration.block)
        .map(|crop| registered_id(crop, Role::Block))
        .collect();
    ids.extend(
        registries
            .crops()
            .iter()
            .filter(|crop| crop.registration.material)
            .map(|crop| registered_id(crop, Role::Material)),
    );
    ids.extend(
        registries
            .crops()
            .iter()
            .filter(|crop| crop.registration.seed)
            .map(|crop| registered_id(crop, Role::Seed)),
    );
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Crafted;
    use crate::settings::Settings;

    fn crop(id: &str, tier: &str) -> Crop {
        Crop::new(
            (id).parse().expect("a legal crop identity"),
            (tier).parse().expect("a legal tier identity"),
            ("test:kind").parse().expect("a legal crop kind identity"),
            Crafted::Nothing,
        )
    }

    #[derive(Default)]
    struct Recorder {
        name: String,
        crops: Vec<(String, String)>,
    }

    impl Extension for Recorder {
        fn namespace(&self) -> &str {
            &self.name
        }

        fn register_crops(&mut self, registries: &mut Registries) {
            for (id, tier) in &self.crops {
                registries.register_crop(crop(id, tier), Derived::default());
            }
        }
    }

    #[test]
    fn a_crop_registered_outside_the_window_is_refused() {
        let mut registries = Registries::new();
        assert!(!registries.register_crop(crop("a:one", "t:1"), Derived::default()));
        assert_eq!(registries.crops().len(), 0);
        assert_eq!(
            registries.refused(),
            [Refusal::Closed("a:one".to_owned())].as_slice()
        );

        registries.open();
        assert!(registries.register_crop(crop("a:one", "t:1"), Derived::default()));
        registries.close();
        assert!(!registries.register_crop(crop("a:two", "t:1"), Derived::default()));
    }

    #[test]
    fn a_crop_path_collides_across_namespaces_and_a_tier_does_not() {
        let mut registries = Registries::new();
        registries.open();
        assert!(registries.register_crop(crop("a:same", "t:1"), Derived::default()));
        assert!(
            !registries.register_crop(crop("b:same", "t:1"), Derived::default()),
            "two namespaces cannot both carry one crop path"
        );
        assert_eq!(registries.crops().len(), 1);

        assert!(
            registries.register_tier(Tier::new("a:same".parse().expect("a legal identity"), 1))
        );
        assert!(
            registries.register_tier(Tier::new("b:same".parse().expect("a legal identity"), 2)),
            "a tier collides on the full identifier only"
        );
        assert!(
            !registries.register_tier(Tier::new("a:same".parse().expect("a legal identity"), 3))
        );
        assert_eq!(registries.tiers().len(), 2);
    }

    #[test]
    fn a_charge_type_claiming_a_taken_creature_is_dropped_whole() {
        let mut registries = Registries::new();
        let mut first = ChargeType::new("a:one", 100.0, 0, 0, 0);
        first.claim("test:creature");
        assert!(registries.register_charge(first));

        let mut second = ChargeType::new("b:two", 100.0, 0, 0, 0);
        second.claim("test:other");
        second.claim("test:creature");
        assert!(!registries.register_charge(second));
        assert_eq!(registries.charges().len(), 1);
        assert_eq!(
            registries.refused().last(),
            Some(&Refusal::Creature("test:creature".to_owned())),
            "claiming is global and first wins"
        );

        let mut third = ChargeType::new("c:three", 100.0, 0, 0, 0);
        third.claim("test:other");
        assert!(third.creatures.len() == 1 && registries.register_charge(third));
    }

    #[test]
    fn a_modifier_and_a_kind_collide_on_the_full_identifier_only() {
        let mut registries = Registries::new();
        assert!(registries.register_modifier(Modifier::new("a:m", 1, &["tool"])));
        assert!(registries.register_modifier(Modifier::new("b:m", 1, &["tool"])));
        assert!(!registries.register_modifier(Modifier::new("a:m", 2, &["tool"])));
        assert!(registries.register_kind(CropKind::new(
            "a:k".parse().expect("a legal identity"),
            "stem"
        )));
        assert!(!registries.register_kind(CropKind::new(
            "a:k".parse().expect("a legal identity"),
            "other"
        )));
        assert_eq!(registries.modifiers().len(), 2);
        assert_eq!(registries.kinds().len(), 1);
    }

    #[test]
    fn the_sort_is_stable_and_ascending_by_tier_value() {
        let mut registries = Registries::new();
        registries.register_tier(Tier::new(
            ("t:high").parse().expect("a legal tier identity"),
            5,
        ));
        registries.register_tier(Tier::new(
            ("t:low").parse().expect("a legal tier identity"),
            1,
        ));
        registries.open();
        for (id, tier) in [
            ("a:first_high", "t:high"),
            ("a:first_low", "t:low"),
            ("a:second_high", "t:high"),
            ("a:second_low", "t:low"),
        ] {
            registries.register_crop(crop(id, tier), Derived::default());
        }
        registries.sort_crops();
        let order: Vec<String> = registries
            .crops()
            .iter()
            .map(|crop| crop.id.to_string())
            .collect();
        assert_eq!(
            order,
            [
                "a:first_low",
                "a:second_low",
                "a:first_high",
                "a:second_high"
            ],
            "ascending across tiers, declaration order within one"
        );
    }

    #[test]
    fn every_material_id_precedes_every_seed_id_and_all_are_in_this_namespace() {
        let mut registries = Registries::new();
        registries.register_tier(Tier::new(
            ("t:1").parse().expect("a legal tier identity"),
            1,
        ));
        registries.open();
        registries.register_crop(crop("other:alpha", "t:1"), Derived::default());
        registries.register_crop(crop("other:beta", "t:1"), Derived::default());
        registries.close();

        let ids = registered_order(&registries);
        assert_eq!(ids.len(), 6);
        let seeds = ids
            .iter()
            .position(|id| id.ends_with("_seeds"))
            .expect("a seed id");
        let materials = ids
            .iter()
            .rposition(|id| id.ends_with("_material"))
            .expect("a material id");
        assert!(materials < seeds, "all materials, then all seeds");
        assert!(
            ids.iter().all(|id| id.starts_with(crate::MOD_ID)),
            "a contributed crop registers in this entry's namespace"
        );
    }

    #[test]
    fn the_construction_pass_interleaves_configure_and_charges_then_adjusts() {
        let mut registries = Registries::new();
        let mut first = Recorder {
            name: "first".to_owned(),
            crops: Vec::new(),
        };
        let mut second = Recorder {
            name: "second".to_owned(),
            crops: Vec::new(),
        };
        let mut extensions: Vec<&mut dyn Extension> = vec![&mut first, &mut second];
        let taken = construct(&mut registries, &mut extensions, &Settings::default());
        assert_eq!(
            taken,
            [
                Step::Configure("first".to_owned()),
                Step::RegisterCharges("first".to_owned()),
                Step::Configure("second".to_owned()),
                Step::RegisterCharges("second".to_owned()),
                Step::AdjustCharges("first".to_owned()),
                Step::AdjustCharges("second".to_owned()),
            ]
        );
    }

    #[test]
    fn block_registration_opens_makes_defaults_sorts_and_closes_in_that_order() {
        let mut registries = Registries::new();
        registries.register_tier(Tier::new(
            ("t:1").parse().expect("a legal tier identity"),
            1,
        ));
        let mut one = Recorder {
            name: "one".to_owned(),
            crops: vec![("a:only".to_owned(), "t:1".to_owned())],
        };
        let mut extensions: Vec<&mut dyn Extension> = vec![&mut one];
        let taken = register_blocks(&mut registries, &mut extensions);
        assert_eq!(
            taken,
            [
                Step::Open,
                Step::RegisterCrops("one".to_owned()),
                Step::DefaultBlocks,
                Step::Sort,
                Step::Close,
            ]
        );
        assert!(!registries.is_open());
        assert_eq!(
            registries.crops()[0].block.as_deref(),
            Some("rvl_321_counterpart:only_crop"),
            "a crop that wanted a block and brought none is given one"
        );
    }

    #[test]
    fn item_registration_adjusts_crops_before_it_asks_for_modifiers() {
        let mut registries = Registries::new();
        let mut one = Recorder {
            name: "one".to_owned(),
            crops: Vec::new(),
        };
        let mut extensions: Vec<&mut dyn Extension> = vec![&mut one];
        let taken = register_items(&mut registries, &mut extensions);
        assert_eq!(
            taken,
            [
                Step::CropItems,
                Step::AdjustCrops("one".to_owned()),
                Step::RegisterModifiers("one".to_owned()),
                Step::ModifierItems,
                Step::AdjustModifiers("one".to_owned()),
            ]
        );
    }

    #[test]
    fn an_extension_opt_out_is_applied_at_the_moment_the_crop_is_registered() {
        let mut registries = Registries::new();
        registries.open();
        let mut wants_all = crop("a:one", "t:1");
        wants_all.derived = Derived::default();
        registries.register_crop(
            wants_all,
            Derived {
                ritual_seed: false,
                grid_seed: true,
                conversion: true,
            },
        );
        assert!(!registries.crops()[0].derived.ritual_seed);
    }

    #[test]
    fn each_role_carries_its_own_fixed_suffix() {
        let one = crop("other:name", "t:1");
        assert_eq!(
            registered_id(&one, Role::Block),
            "rvl_321_counterpart:name_crop"
        );
        assert_eq!(
            registered_id(&one, Role::Material),
            "rvl_321_counterpart:name_material"
        );
        assert_eq!(
            registered_id(&one, Role::Seed),
            "rvl_321_counterpart:name_seeds"
        );
    }
}
