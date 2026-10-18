//! The twenty-eight settings of section three, and the flags of section four.
//!
//! The original writes them into two documents: two presentation settings in a
//! client-scoped file, and the other twenty-six — every setting that decides a
//! recipe result and every setting that decides whether a server refuses an
//! interaction — in a per-installation file that is not sent to a joining
//! client. Section 3.5 records the consequence and says in as many words that
//! this is the one place where matching the original exactly would be wrong.
//!
//! So the deciding group is server-authoritative here. Ravel resolves one
//! document per mod with a side on each setting and has no side that means "both
//! servers", so the group is declared common, every side resolves it, and a
//! joining client is then handed the server's answers by [`Decisive`] and adopts
//! them over its own. What a client resolved for itself decides nothing after
//! that payload arrives.
//!
//! Seventeen named flags are bound over fourteen of these booleans. Each flag
//! reads the switch it is named for, which is the second declared change: the
//! original binds one flag to a switch belonging to an unrelated feature.

use ravel_sdk::Declared;
use ravel_sdk::config::{
    ConfigurationDeclaration, DeclarationError, LayeredConfiguration, ValueError,
};
use ravel_sdk::content::{ContentId, ContentIdError};
use ravel_sdk::network::{
    Direction, Packet, PacketContract, PacketId, PacketReader, PacketWriter, WireError,
};
use rvl_042_counterpart::flags::{FlagId, FlagTable};

use crate::matching::Matcher;
use crate::overrides::Table;

/// The key the two informational tooltips are turned on and off under.
pub const TOOLTIP_ENABLED: &str = "tooltip.enabled";
/// The key tools excluded from the mining-speed tooltip are listed under.
pub const TOOLTIP_EXCLUDED: &str = "tooltip.excluded";

/// The key the tinted stone variants are switched under.
pub const FEATURE_TINTED_STONE: &str = "feature.tinted_stone";
/// The key the light-emitting block is switched under.
pub const FEATURE_LIGHT_BLOCK: &str = "feature.light_block";
/// The key the two fuel fragments are switched under.
pub const FEATURE_FUEL_FRAGMENTS: &str = "feature.fuel_fragments";
/// The key the two restorative foods are switched under.
pub const FEATURE_RESTORATIVE_FOODS: &str = "feature.restorative_foods";
/// The key the two watering tools are switched under.
pub const FEATURE_WATERING_TOOLS: &str = "feature.watering_tools";
/// The key the three sieve meshes are switched under.
pub const FEATURE_SIEVE_MESHES: &str = "feature.sieve_meshes";
/// The key the attractor is switched under.
pub const FEATURE_ATTRACTOR: &str = "feature.attractor";
/// The key the two head-worn pieces are switched under.
pub const FEATURE_HEADPIECES: &str = "feature.headpieces";
/// The key the combined mining tools are switched under.
pub const FEATURE_COMBINED_TOOLS: &str = "feature.combined_tools";
/// The key the first added material tier is switched under.
pub const FEATURE_FIRST_TIER: &str = "feature.first_tier";
/// The key the second added material tier is switched under.
pub const FEATURE_SECOND_TIER: &str = "feature.second_tier";
/// The key the right-click crop harvest is switched under.
pub const FEATURE_CROP_HARVEST: &str = "feature.crop_harvest";
/// The key the wider of the two sweeping harvest families is switched under.
pub const FEATURE_SWEEPING_WIDE: &str = "feature.sweeping_wide";
/// The key the taller of the two sweeping harvest families is switched under.
pub const FEATURE_SWEEPING_TALL: &str = "feature.sweeping_tall";

/// The key the grid repair rule is switched under.
pub const REPAIR_ENABLED: &str = "repair.enabled";
/// The key units of material for a whole repair are written under.
pub const REPAIR_COST: &str = "repair.cost";
/// The key the extra units an enchanted tool costs are written under.
pub const REPAIR_ENCHANTMENT_COST: &str = "repair.enchantment_cost";
/// The key stripping enchantments on repair is switched under.
pub const REPAIR_STRIP_ENCHANTMENTS: &str = "repair.strip_enchantments";
/// The key ignoring the host's own repair materials is switched under.
pub const REPAIR_DISABLE_DEFAULTS: &str = "repair.disable_defaults";
/// The key override mode is switched under.
pub const REPAIR_OVERRIDE_MODE: &str = "repair.override_mode";
/// The key halved costs for the digging family are switched under.
pub const REPAIR_CHEAPER_DIGGING: &str = "repair.cheaper_digging";
/// The key halved costs for the shearing tool are switched under.
pub const REPAIR_CHEAPER_SHEARING: &str = "repair.cheaper_shearing";
/// The key tools the repair rule refuses are listed under.
pub const REPAIR_BLACKLIST: &str = "repair.blacklist";
/// The key the repair-material override table is written under.
pub const REPAIR_OVERRIDES: &str = "repair.overrides";

/// The key the nearly-destroyed tool refusal is switched under.
pub const TWEAK_NEARLY_DESTROYED: &str = "tweak.nearly_destroyed";
/// The key the ineffective tools are listed under.
pub const TWEAK_INEFFECTIVE: &str = "tweak.ineffective";

/// The two settings a client resolves and keeps.
pub const CLIENT_KEYS: [&str; 2] = [TOOLTIP_ENABLED, TOOLTIP_EXCLUDED];

/// A declared feature switch.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Feature {
    /// Tinted stone variants.
    TintedStone,
    /// The light-emitting block.
    LightBlock,
    /// Fuel fragments.
    FuelFragments,
    /// Restorative foods.
    RestorativeFoods,
    /// Watering tools.
    WateringTools,
    /// Sieve meshes.
    SieveMeshes,
    /// The attractor.
    Attractor,
    /// Head-worn pieces.
    Headpieces,
    /// Combined mining tools.
    CombinedTools,
    /// The first material tier.
    FirstTier,
    /// The second material tier.
    SecondTier,
    /// Right-click crop harvesting.
    CropHarvest,
    /// Wide sweeping harvest.
    SweepingWide,
    /// Tall sweeping harvest.
    SweepingTall,
}

impl Feature {
    /// All feature switches, in declaration and wire order.
    pub const ALL: [Self; 14] = [
        Self::TintedStone,
        Self::LightBlock,
        Self::FuelFragments,
        Self::RestorativeFoods,
        Self::WateringTools,
        Self::SieveMeshes,
        Self::Attractor,
        Self::Headpieces,
        Self::CombinedTools,
        Self::FirstTier,
        Self::SecondTier,
        Self::CropHarvest,
        Self::SweepingWide,
        Self::SweepingTall,
    ];

    /// The configuration key for this feature.
    pub const fn key(self) -> &'static str {
        match self {
            Self::TintedStone => FEATURE_TINTED_STONE,
            Self::LightBlock => FEATURE_LIGHT_BLOCK,
            Self::FuelFragments => FEATURE_FUEL_FRAGMENTS,
            Self::RestorativeFoods => FEATURE_RESTORATIVE_FOODS,
            Self::WateringTools => FEATURE_WATERING_TOOLS,
            Self::SieveMeshes => FEATURE_SIEVE_MESHES,
            Self::Attractor => FEATURE_ATTRACTOR,
            Self::Headpieces => FEATURE_HEADPIECES,
            Self::CombinedTools => FEATURE_COMBINED_TOOLS,
            Self::FirstTier => FEATURE_FIRST_TIER,
            Self::SecondTier => FEATURE_SECOND_TIER,
            Self::CropHarvest => FEATURE_CROP_HARVEST,
            Self::SweepingWide => FEATURE_SWEEPING_WIDE,
            Self::SweepingTall => FEATURE_SWEEPING_TALL,
        }
    }

    /// The slot for this feature in declaration and wire order.
    pub const fn index(self) -> usize {
        match self {
            Self::TintedStone => 0,
            Self::LightBlock => 1,
            Self::FuelFragments => 2,
            Self::RestorativeFoods => 3,
            Self::WateringTools => 4,
            Self::SieveMeshes => 5,
            Self::Attractor => 6,
            Self::Headpieces => 7,
            Self::CombinedTools => 8,
            Self::FirstTier => 9,
            Self::SecondTier => 10,
            Self::CropHarvest => 11,
            Self::SweepingWide => 12,
            Self::SweepingTall => 13,
        }
    }
}

/// The twenty-six settings the server decides and a client is told.
pub const DECIDING_KEYS: [&str; 26] = [
    FEATURE_TINTED_STONE,
    FEATURE_LIGHT_BLOCK,
    FEATURE_FUEL_FRAGMENTS,
    FEATURE_RESTORATIVE_FOODS,
    FEATURE_WATERING_TOOLS,
    FEATURE_SIEVE_MESHES,
    FEATURE_ATTRACTOR,
    FEATURE_HEADPIECES,
    FEATURE_COMBINED_TOOLS,
    FEATURE_FIRST_TIER,
    FEATURE_SECOND_TIER,
    FEATURE_CROP_HARVEST,
    FEATURE_SWEEPING_WIDE,
    FEATURE_SWEEPING_TALL,
    REPAIR_ENABLED,
    REPAIR_COST,
    REPAIR_ENCHANTMENT_COST,
    REPAIR_STRIP_ENCHANTMENTS,
    REPAIR_DISABLE_DEFAULTS,
    REPAIR_OVERRIDE_MODE,
    REPAIR_CHEAPER_DIGGING,
    REPAIR_CHEAPER_SHEARING,
    REPAIR_BLACKLIST,
    REPAIR_OVERRIDES,
    TWEAK_NEARLY_DESTROYED,
    TWEAK_INEFFECTIVE,
];

/// The lowest and highest units a whole repair may cost.
pub const COST_RANGE: (u32, u32) = (1, 8);

/// The lowest and highest extra units an enchanted tool may cost.
pub const ENCHANTMENT_COST_RANGE: (u32, u32) = (0, 8);

/// A flag this package publishes over a feature switch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Flag {
    TintedStone,
    LightBlock,
    FuelFragments,
    WateringTools,
    SieveMeshes,
    Headpieces,
    CombinedTools,
    FirstTier,
    FirstTierArmour,
    FirstTierShearing,
    SecondTier,
    SecondTierArmour,
    SecondTierShearing,
    SweepingWide,
    SweepingWideUpper,
    SweepingTall,
    SweepingTallUpper,
}

impl Flag {
    /// The published name of this flag.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::TintedStone => "tinted_stone",
            Self::LightBlock => "light_block",
            Self::FuelFragments => "fuel_fragments",
            Self::WateringTools => "watering_tools",
            Self::SieveMeshes => "sieve_meshes",
            Self::Headpieces => "headpieces",
            Self::CombinedTools => "combined_tools",
            Self::FirstTier => "first_tier",
            Self::FirstTierArmour => "first_tier_armour",
            Self::FirstTierShearing => "first_tier_shearing",
            Self::SecondTier => "second_tier",
            Self::SecondTierArmour => "second_tier_armour",
            Self::SecondTierShearing => "second_tier_shearing",
            Self::SweepingWide => "sweeping_wide",
            Self::SweepingWideUpper => "sweeping_wide_upper",
            Self::SweepingTall => "sweeping_tall",
            Self::SweepingTallUpper => "sweeping_tall_upper",
        }
    }
}

/// The seventeen flags, each paired with the switch it reads.
///
/// Three switches carry no flag and four switches carry more than one, which is
/// the shape section four records: the two sets are not in bijection. What is
/// fixed here is the binding — every flag reads the switch it is named for, and
/// the test beside it is the test that says so.
///
/// Which four switches carry a second flag is **inferred**: the specification
/// states the counts and the mis-binding but not the whole table.
pub const FLAG_BINDINGS: [(Flag, Feature); 17] = [
    (Flag::TintedStone, Feature::TintedStone),
    (Flag::LightBlock, Feature::LightBlock),
    (Flag::FuelFragments, Feature::FuelFragments),
    (Flag::WateringTools, Feature::WateringTools),
    (Flag::SieveMeshes, Feature::SieveMeshes),
    (Flag::Headpieces, Feature::Headpieces),
    (Flag::CombinedTools, Feature::CombinedTools),
    (Flag::FirstTier, Feature::FirstTier),
    (Flag::FirstTierArmour, Feature::FirstTier),
    (Flag::FirstTierShearing, Feature::FirstTier),
    (Flag::SecondTier, Feature::SecondTier),
    (Flag::SecondTierArmour, Feature::SecondTier),
    (Flag::SecondTierShearing, Feature::SecondTier),
    (Flag::SweepingWide, Feature::SweepingWide),
    (Flag::SweepingWideUpper, Feature::SweepingWide),
    (Flag::SweepingTall, Feature::SweepingTall),
    (Flag::SweepingTallUpper, Feature::SweepingTall),
];

/// The declaration this counterpart packages.
///
/// The tooltip exclusion list ships empty. The original ships it naming a whole
/// third-party namespace, which is that project's identity and not this
/// package's to carry.
pub const DOCUMENT: &str = r#"{
  "schemaVersion": 1,
  "owner": "rvl_086_counterpart",
  "version": 1,
  "settings": [
    {
      "key": "tooltip.enabled",
      "title": "Informational tooltips",
      "description": "Adds a mining-speed line to a digging tool and an ammunition line to a ranged weapon.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "client",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "tooltip.excluded",
      "title": "Tools without a mining-speed line",
      "description": "Matching expressions, comma separated, naming tools the mining-speed line is left off.",
      "valueSchema": { "type": "string" },
      "default": "",
      "side": "client",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "feature.tinted_stone",
      "title": "Tinted stone variants",
      "description": "Lists the sixteen tinted stone variants in the creative grouping and loads their recipes.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": true,
      "packOverride": "allowed"
    },
    {
      "key": "feature.light_block",
      "title": "Light-emitting block",
      "description": "Lists the light-emitting block in the creative grouping and loads its recipe.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": true,
      "packOverride": "allowed"
    },
    {
      "key": "feature.fuel_fragments",
      "title": "Fuel fragments",
      "description": "Lists the two fuel fragments in the creative grouping and loads their recipes.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": true,
      "packOverride": "allowed"
    },
    {
      "key": "feature.restorative_foods",
      "title": "Restorative foods",
      "description": "Lists the two restorative foods in the creative grouping and loads their recipes.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": true,
      "packOverride": "allowed"
    },
    {
      "key": "feature.watering_tools",
      "title": "Watering tools",
      "description": "Lists the two watering tools in the creative grouping and loads their recipes.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": true,
      "packOverride": "allowed"
    },
    {
      "key": "feature.sieve_meshes",
      "title": "Sieve meshes",
      "description": "Lists the plant fibre and the three sieve meshes in the creative grouping.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": true,
      "packOverride": "allowed"
    },
    {
      "key": "feature.attractor",
      "title": "Attractor",
      "description": "Lists the attractor in the creative grouping and loads its recipe.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": true,
      "packOverride": "allowed"
    },
    {
      "key": "feature.headpieces",
      "title": "Night-vision headpieces",
      "description": "Lists the two head-worn night-vision pieces in the creative grouping.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": true,
      "packOverride": "allowed"
    },
    {
      "key": "feature.combined_tools",
      "title": "Combined mining tools",
      "description": "Lists the eight combined mining tools in the creative grouping.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": true,
      "packOverride": "allowed"
    },
    {
      "key": "feature.first_tier",
      "title": "First added material tier",
      "description": "Lists the first added tier's tools and armour in the creative grouping.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": true,
      "packOverride": "allowed"
    },
    {
      "key": "feature.second_tier",
      "title": "Second added material tier",
      "description": "Lists the second added tier's tools and armour in the creative grouping.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": true,
      "packOverride": "allowed"
    },
    {
      "key": "feature.crop_harvest",
      "title": "Right-click crop harvest",
      "description": "Harvests a mature crop on a bare-handed right click and replants it.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "feature.sweeping_wide",
      "title": "Wider sweeping harvest tools",
      "description": "Lists the eight tools of the wider sweeping harvest family in the creative grouping.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": true,
      "packOverride": "allowed"
    },
    {
      "key": "feature.sweeping_tall",
      "title": "Taller sweeping harvest tools",
      "description": "Lists the eight tools of the taller sweeping harvest family in the creative grouping.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": true,
      "packOverride": "allowed"
    },
    {
      "key": "repair.enabled",
      "title": "Repair a tool in a crafting grid",
      "description": "Mends a damaged tool laid in a crafting grid beside valid repair materials.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "repair.cost",
      "title": "Units for a whole repair",
      "description": "How many units of material mend a tool from broken to whole.",
      "valueSchema": { "type": "integer", "minimum": 1, "maximum": 8 },
      "default": 3,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "repair.enchantment_cost",
      "title": "Extra units for an enchanted tool",
      "description": "Units added to the cost when the tool carries any enchantment.",
      "valueSchema": { "type": "integer", "minimum": 0, "maximum": 8 },
      "default": 1,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "repair.strip_enchantments",
      "title": "Strip enchantments on repair",
      "description": "Removes every enchantment that is not a curse from a repaired tool.",
      "valueSchema": { "type": "boolean" },
      "default": false,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "repair.disable_defaults",
      "title": "Ignore the host's own repair materials",
      "description": "Leaves the override table as the only source of a valid repair material.",
      "valueSchema": { "type": "boolean" },
      "default": false,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "repair.override_mode",
      "title": "Overriding a tool replaces its materials",
      "description": "A tool named anywhere in the override table accepts only the materials named with it.",
      "valueSchema": { "type": "boolean" },
      "default": false,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "repair.cheaper_digging",
      "title": "Halve the cost for digging tools",
      "description": "Halves both repair costs for a tiered digging tool.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "repair.cheaper_shearing",
      "title": "Halve the cost for the shearing tool",
      "description": "Halves both repair costs for the shearing tool.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "repair.blacklist",
      "title": "Tools the repair rule refuses",
      "description": "Matching expressions, comma separated, naming tools a crafting grid will not mend.",
      "valueSchema": { "type": "string" },
      "default": "",
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "repair.overrides",
      "title": "Repair-material overrides",
      "description": "Entries of the form tool=material, optionally followed by an at sign and a multiplier above zero, comma separated and read in order.",
      "valueSchema": { "type": "string" },
      "default": "",
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "tweak.nearly_destroyed",
      "title": "Stop a nearly destroyed tool working",
      "description": "Refuses the last two points of durability on a tool rather than letting it break.",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "tweak.ineffective",
      "title": "Tools made ineffective",
      "description": "Matching expressions, comma separated, naming tools that mine nothing and deal no damage.",
      "valueSchema": { "type": "string" },
      "default": "",
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    }
  ]
}"#;

/// Reads the packaged declaration.
///
/// # Errors
///
/// Returns whatever [`ConfigurationDeclaration::parse`] refuses, which for a
/// compile-time constant is a packaging failure rather than a runtime one.
pub fn declaration() -> Result<ConfigurationDeclaration, DeclarationError> {
    ConfigurationDeclaration::parse(DOCUMENT)
}

/// Material-selection and enchantment handling during repair.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RepairMaterials {
    /// Whether a repair strips every enchantment that is not a curse.
    pub strip_enchantments: bool,
    /// Whether the host's own repair materials are ignored.
    pub disable_defaults: bool,
    /// Whether naming a tool in the table replaces its default materials.
    pub override_mode: bool,
}
impl RepairMaterials {
    fn read(resolved: &LayeredConfiguration) -> Result<Self, ValueError> {
        Ok(Self {
            strip_enchantments: resolved.boolean(REPAIR_STRIP_ENCHANTMENTS)?,
            disable_defaults: resolved.boolean(REPAIR_DISABLE_DEFAULTS)?,
            override_mode: resolved.boolean(REPAIR_OVERRIDE_MODE)?,
        })
    }
}

/// Tool shapes whose repair costs are halved.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RepairDiscounts {
    /// Whether both costs halve for a tiered digging tool.
    pub digging: bool,
    /// Whether both costs halve for the shearing tool.
    pub shearing: bool,
}
impl RepairDiscounts {
    fn read(resolved: &LayeredConfiguration) -> Result<Self, ValueError> {
        Ok(Self {
            digging: resolved.boolean(REPAIR_CHEAPER_DIGGING)?,
            shearing: resolved.boolean(REPAIR_CHEAPER_SHEARING)?,
        })
    }
}

/// The values in force for one launch.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Whether the two informational tooltips are drawn.
    pub tooltip_enabled: bool,
    /// Tools the mining-speed tooltip is left off.
    pub tooltip_excluded: Matcher,
    /// The fourteen feature switches, in the order of [`Feature::ALL`].
    pub switches: [bool; 14],
    /// Whether a crafting grid mends a tool at all.
    pub repair_enabled: bool,
    /// Units of material for a whole repair.
    pub repair_cost: u32,
    /// Extra units charged when the tool carries any enchantment.
    pub repair_enchantment_cost: u32,
    /// Material-selection and enchantment handling.
    pub repair_materials: RepairMaterials,
    /// Tool-shape cost discounts.
    pub repair_discounts: RepairDiscounts,
    /// Tools a crafting grid refuses to mend.
    pub repair_blacklist: Matcher,
    /// The override table, in the order it was written.
    pub repair_overrides: Table,
    /// Whether the last two points of durability are refused.
    pub tweak_nearly_destroyed: bool,
    /// Tools made ineffective, which is the whole of what governs that rule.
    pub tweak_ineffective: Matcher,
}

impl Default for Settings {
    fn default() -> Self {
        let resolved = LayeredConfiguration::resolve(
            declaration().expect("the packaged declaration is valid"),
            None,
            None,
        )
        .expect("the shipped defaults resolve");
        Self::read(&resolved, crate::HOST_NAMESPACE).expect("declared settings have matching types")
    }
}

impl Settings {
    /// Reads the values in force out of a resolved configuration.
    ///
    /// Returns typed access errors; repair costs retain their declared clamping.
    /// `host_namespace` completes an override selector written without one.
    pub fn read(resolved: &LayeredConfiguration, host_namespace: &str) -> Result<Self, ValueError> {
        let mut switches = [false; 14];
        for feature in Feature::ALL {
            switches[feature.index()] = resolved.boolean(feature.key())?;
        }
        Ok(Self {
            tooltip_enabled: resolved.boolean(TOOLTIP_ENABLED)?,
            tooltip_excluded: Matcher::parse(resolved.text(TOOLTIP_EXCLUDED)?),
            switches,
            repair_enabled: resolved.boolean(REPAIR_ENABLED)?,
            repair_cost: clamp_cost(resolved.integer(REPAIR_COST)?, COST_RANGE),
            repair_enchantment_cost: clamp_cost(
                resolved.integer(REPAIR_ENCHANTMENT_COST)?,
                ENCHANTMENT_COST_RANGE,
            ),
            repair_materials: RepairMaterials::read(resolved)?,
            repair_discounts: RepairDiscounts::read(resolved)?,
            repair_blacklist: Matcher::parse(resolved.text(REPAIR_BLACKLIST)?),
            repair_overrides: Table::parse(resolved.text(REPAIR_OVERRIDES)?, host_namespace),
            tweak_nearly_destroyed: resolved.boolean(TWEAK_NEARLY_DESTROYED)?,
            tweak_ineffective: Matcher::parse(resolved.text(TWEAK_INEFFECTIVE)?),
        })
    }

    /// Whether a declared feature switch is on.
    #[must_use]
    pub const fn is_switched_on(&self, feature: Feature) -> bool {
        self.switches[feature.index()]
    }

    /// The values the server decides and a joining client is told.
    #[must_use]
    pub fn decisive(&self) -> Decisive {
        Decisive {
            switches: self.switches,
            repair_enabled: self.repair_enabled,
            repair_cost: self.repair_cost,
            repair_enchantment_cost: self.repair_enchantment_cost,
            repair_materials: self.repair_materials,
            repair_discounts: self.repair_discounts,
            repair_blacklist: written(&self.repair_blacklist),
            repair_overrides: self
                .repair_overrides
                .rows()
                .iter()
                .map(|row| format!("{}={}@{}", row.tool, row.material, row.multiplier))
                .collect::<Vec<_>>()
                .join(","),
            tweak_nearly_destroyed: self.tweak_nearly_destroyed,
            tweak_ineffective: written(&self.tweak_ineffective),
        }
    }

    /// Adopts the server's answers over whatever this side resolved.
    ///
    /// The two presentation settings are untouched: they decide nothing a server
    /// can disagree with, and they are the player's own.
    pub fn adopt(&mut self, decisive: &Decisive, host_namespace: &str) {
        self.switches = decisive.switches;
        self.repair_enabled = decisive.repair_enabled;
        self.repair_cost = decisive.repair_cost.clamp(COST_RANGE.0, COST_RANGE.1);
        self.repair_enchantment_cost = decisive
            .repair_enchantment_cost
            .clamp(ENCHANTMENT_COST_RANGE.0, ENCHANTMENT_COST_RANGE.1);
        self.repair_materials = decisive.repair_materials;
        self.repair_discounts = decisive.repair_discounts;
        self.repair_blacklist = Matcher::parse(&decisive.repair_blacklist);
        self.repair_overrides = Table::parse(&decisive.repair_overrides, host_namespace);
        self.tweak_nearly_destroyed = decisive.tweak_nearly_destroyed;
        self.tweak_ineffective = Matcher::parse(&decisive.tweak_ineffective);
    }
}

fn clamp_cost(value: i64, (minimum, maximum): (u32, u32)) -> u32 {
    u32::try_from(value).map_or_else(
        |_| if value < 0 { minimum } else { maximum },
        |value| value.clamp(minimum, maximum),
    )
}

/// The twenty-six deciding settings, as they travel to a joining client.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Decisive {
    /// The fourteen feature switches, in the order of [`Feature::ALL`].
    pub switches: [bool; 14],
    /// Whether a crafting grid mends a tool at all.
    pub repair_enabled: bool,
    /// Units of material for a whole repair.
    pub repair_cost: u32,
    /// Extra units charged when the tool carries any enchantment.
    pub repair_enchantment_cost: u32,
    /// Material-selection and enchantment handling.
    pub repair_materials: RepairMaterials,
    /// Tool-shape cost discounts.
    pub repair_discounts: RepairDiscounts,
    /// The repair blacklist, as written.
    pub repair_blacklist: String,
    /// The override table, as written.
    pub repair_overrides: String,
    /// Whether the last two points of durability are refused.
    pub tweak_nearly_destroyed: bool,
    /// The ineffective list, as written.
    pub tweak_ineffective: String,
}

/// The payload name the deciding settings travel under.
pub const DECISIVE_PACKET: &str = "decisive_settings";

/// The version this entry stamps on every payload it declares.
pub const PACKET_VERSION: u32 = 1;

impl Packet for Decisive {
    fn contract() -> PacketContract {
        PacketContract::new(
            PacketId::new(
                ContentId::new(crate::Entry::declared_id().clone(), DECISIVE_PACKET)
                    .expect("the packaged packet path is legal"),
            ),
            Direction::ToClient,
            PACKET_VERSION,
        )
    }

    fn write(&self, writer: &mut PacketWriter) {
        for switch in self.switches {
            writer.bool(switch);
        }
        writer.bool(self.repair_enabled);
        writer.u32(self.repair_cost);
        writer.u32(self.repair_enchantment_cost);
        writer.bool(self.repair_materials.strip_enchantments);
        writer.bool(self.repair_materials.disable_defaults);
        writer.bool(self.repair_materials.override_mode);
        writer.bool(self.repair_discounts.digging);
        writer.bool(self.repair_discounts.shearing);
        writer.text(&self.repair_blacklist);
        writer.text(&self.repair_overrides);
        writer.bool(self.tweak_nearly_destroyed);
        writer.text(&self.tweak_ineffective);
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        let mut switches = [false; 14];
        for switch in &mut switches {
            *switch = reader.bool()?;
        }
        Ok(Self {
            switches,
            repair_enabled: reader.bool()?,
            repair_cost: reader.u32()?,
            repair_enchantment_cost: reader.u32()?,
            repair_materials: RepairMaterials {
                strip_enchantments: reader.bool()?,
                disable_defaults: reader.bool()?,
                override_mode: reader.bool()?,
            },
            repair_discounts: RepairDiscounts {
                digging: reader.bool()?,
                shearing: reader.bool()?,
            },
            repair_blacklist: reader.text()?.to_owned(),
            repair_overrides: reader.text()?.to_owned(),
            tweak_nearly_destroyed: reader.bool()?,
            tweak_ineffective: reader.text()?.to_owned(),
        })
    }
}

/// Binds the seventeen flags over the switches in force.
///
/// Every flag is a constant read off the settings at the moment of binding,
/// which is what section four's one-time scan wants: the switches are
/// restart-scoped and a flag that changed under a data pack would leave a
/// creative listing disagreeing with the recipes beside it.
///
/// # Errors
///
/// Returns [`ContentIdError`] when the namespace or a declared flag name is invalid.
pub fn publish(
    table: &mut FlagTable,
    settings: &Settings,
    namespace: &str,
) -> Result<(), ContentIdError> {
    for (flag, key) in FLAG_BINDINGS {
        let id = FlagId::new(namespace, flag.name())?;
        table.publish_constant(id, settings.is_switched_on(key));
    }
    Ok(())
}

fn written(matcher: &Matcher) -> String {
    matcher
        .expressions()
        .iter()
        .map(ToString::to_string)
        .filter(|piece| !piece.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravel_sdk::Side;
    use ravel_sdk::config::SettingSide;
    /// Whether any shipped setting would have to travel for a side to have a value.
    ///
    /// Every setting here is declared client or common, so every side resolves the
    /// whole document and none has to. What travels is [`Decisive`], and it travels
    /// to make the server authoritative rather than because a client holds nothing.
    #[must_use]
    fn resolves_on_every_side() -> bool {
        declaration().is_ok_and(|declaration| {
            declaration
                .settings()
                .iter()
                .all(|setting| matches!(setting.side, SettingSide::Client | SettingSide::Common))
        })
    }

    /// Whether a setting is one a given side resolves at all.
    #[must_use]
    fn resolved_on(key: &str, side: Side) -> bool {
        declaration().is_ok_and(|declaration| {
            declaration
                .setting(key)
                .is_some_and(|setting| setting.side.applies_to(side))
        })
    }

    #[test]
    fn a_missing_declared_key_is_returned_instead_of_defaulted() {
        let changed = DOCUMENT.replace(TOOLTIP_ENABLED, "tooltip.renamed");
        let declared = ConfigurationDeclaration::parse(&changed).expect("a legal declaration");
        let resolved = LayeredConfiguration::resolve(declared, None, None)
            .expect("the changed declaration resolves");
        assert!(Settings::read(&resolved, "example").is_err());
    }

    #[test]
    fn repair_costs_saturate_before_clamping_to_their_declared_range() {
        assert_eq!(clamp_cost(i64::MIN, COST_RANGE), COST_RANGE.0);
        assert_eq!(clamp_cost(i64::MAX, COST_RANGE), COST_RANGE.1);
        assert_eq!(clamp_cost(4, COST_RANGE), 4);
        assert_eq!(clamp_cost(-1, ENCHANTMENT_COST_RANGE), 0);
    }

    #[test]
    fn typed_reads_apply_repair_boundaries_and_text_matchers() {
        let document = ravel_sdk::config::ValueDocument::parse(
            &ravel_sdk::json!({
                "schemaVersion": 1,
                "owner": crate::MOD_ID,
                "values": {
                    REPAIR_COST: COST_RANGE.1,
                    REPAIR_ENCHANTMENT_COST: ENCHANTMENT_COST_RANGE.0,
                    REPAIR_STRIP_ENCHANTMENTS: true,
                    REPAIR_CHEAPER_DIGGING: true,
                    TOOLTIP_EXCLUDED: "example:pick",
                },
            })
            .to_string(),
        )
        .expect("a legal override document");
        let resolved = LayeredConfiguration::resolve(
            declaration().expect("the packaged declaration parses"),
            Some(&document),
            None,
        )
        .expect("the override values resolve");
        let settings =
            Settings::read(&resolved, "example").expect("declared settings have matching types");
        assert_eq!(settings.repair_cost, COST_RANGE.1);
        assert_eq!(settings.repair_enchantment_cost, ENCHANTMENT_COST_RANGE.0);
        assert!(settings.repair_materials.strip_enchantments);
        assert!(settings.repair_discounts.digging);
        assert_eq!(settings.tooltip_excluded, Matcher::parse("example:pick"));
    }

    #[test]
    fn the_packaged_document_parses_and_declares_twenty_eight_settings() {
        let declaration = declaration().expect("the packaged document parses");
        assert_eq!(declaration.settings().len(), 28);
        for key in CLIENT_KEYS.iter().chain(DECIDING_KEYS.iter()) {
            assert!(declaration.setting(key).is_some(), "{key} is declared");
        }
    }

    #[test]
    fn the_two_groups_are_two_and_twenty_six_and_do_not_overlap() {
        for client in CLIENT_KEYS {
            assert!(!DECIDING_KEYS.contains(&client), "{client} is not decided");
        }
    }

    #[test]
    fn every_deciding_setting_is_resolved_on_every_side_and_the_client_pair_is_not() {
        assert!(resolves_on_every_side());
        for key in DECIDING_KEYS {
            for side in [Side::Client, Side::IntegratedServer, Side::DedicatedServer] {
                assert!(resolved_on(key, side), "{key} on {side:?}");
            }
        }
        assert!(resolved_on(TOOLTIP_ENABLED, Side::Client));
        assert!(!resolved_on(TOOLTIP_ENABLED, Side::DedicatedServer));
    }

    #[test]
    fn every_shipped_default_is_the_one_the_specification_states() {
        let shipped = Settings::default();
        assert!(shipped.tooltip_enabled);
        assert!(shipped.tooltip_excluded.is_empty());
        assert_eq!(shipped.switches, [true; 14]);
        assert!(shipped.repair_enabled);
        assert_eq!(shipped.repair_cost, 3);
        assert_eq!(shipped.repair_enchantment_cost, 1);
        assert!(!shipped.repair_materials.strip_enchantments);
        assert!(!shipped.repair_materials.disable_defaults);
        assert!(!shipped.repair_materials.override_mode);
        assert!(shipped.repair_discounts.digging);
        assert!(shipped.repair_discounts.shearing);
        assert!(shipped.repair_blacklist.is_empty());
        assert_eq!(shipped.repair_overrides.rows().len(), 0);
        assert!(shipped.tweak_nearly_destroyed);
        assert!(shipped.tweak_ineffective.is_empty());
    }

    #[test]
    fn the_two_costs_declare_the_ranges_the_specification_constrains_them_to() {
        let declaration = declaration().expect("the packaged document parses");
        let cost = declaration
            .setting(REPAIR_COST)
            .expect("the cost is declared");
        assert_eq!(cost.value_schema.minimum, Some(1.0));
        assert_eq!(cost.value_schema.maximum, Some(8.0));
        let extra = declaration
            .setting(REPAIR_ENCHANTMENT_COST)
            .expect("the extra cost is declared");
        assert_eq!(extra.value_schema.minimum, Some(0.0));
        assert_eq!(extra.value_schema.maximum, Some(8.0));
    }

    #[test]
    fn seventeen_flags_are_bound_over_fourteen_switches_and_three_switches_carry_none() {
        let mut carried: Vec<Feature> = FLAG_BINDINGS.iter().map(|(_, feature)| *feature).collect();
        carried.sort_unstable();
        carried.dedup();
        assert_eq!(carried.len(), 11);
        assert_eq!(Feature::ALL.len() - carried.len(), 3);
    }

    #[test]
    fn a_flag_reads_the_switch_it_is_named_for() {
        for (flag, feature) in FLAG_BINDINGS {
            let key = feature.key();
            let leaf = key.split_once('.').map_or(key, |(_, leaf)| leaf);
            assert!(
                flag.name().starts_with(leaf),
                "the flag {flag:?} reads {key}, which it is not named for"
            );
        }
    }

    #[test]
    fn switching_one_feature_off_disables_only_the_flags_named_for_it() {
        let mut settings = Settings::default();
        settings.switches[Feature::LightBlock.index()] = false;

        let mut table = FlagTable::default();
        publish(&mut table, &settings, "rvl_086_counterpart").expect("legal flag bindings");
        assert_eq!(table.published().count(), 17);
        let named = FlagId::new("rvl_086_counterpart", "light_block").expect("a legal identity");
        let unrelated =
            FlagId::new("rvl_086_counterpart", "watering_tools").expect("a legal identity");
        assert!(!table.enabled(&named));
        assert!(table.enabled(&unrelated));
    }

    #[test]
    fn an_invalid_flag_namespace_is_returned_instead_of_silently_skipped() {
        let mut table = FlagTable::default();
        assert!(publish(&mut table, &Settings::default(), "invalid namespace").is_err());
        assert_eq!(table.published().count(), 0);
    }

    #[test]
    fn switching_the_watering_tools_off_leaves_the_light_block_listed() {
        let mut settings = Settings::default();
        settings.switches[Feature::WateringTools.index()] = false;

        let mut table = FlagTable::default();
        publish(&mut table, &settings, "rvl_086_counterpart").expect("legal flag bindings");
        let light = FlagId::new("rvl_086_counterpart", "light_block").expect("a legal identity");
        assert!(table.enabled(&light));
    }

    #[test]
    fn every_declared_feature_has_a_distinct_slot_and_ships_enabled() {
        for (index, feature) in Feature::ALL.into_iter().enumerate() {
            assert_eq!(feature.index(), index);
            assert!(Settings::default().is_switched_on(feature));
        }
    }

    #[test]
    fn the_deciding_settings_round_trip_over_the_wire() {
        let mut settings = Settings {
            repair_cost: 7,
            repair_enchantment_cost: 4,
            repair_materials: RepairMaterials {
                strip_enchantments: true,
                ..RepairMaterials::default()
            },
            repair_blacklist: Matcher::parse("example:pick,tag:example:tools"),
            repair_overrides: Table::parse("example:pick=example:ingot@2", "host"),
            tweak_ineffective: Matcher::parse("example:*"),
            ..Settings::default()
        };
        settings.switches[Feature::TintedStone.index()] = false;

        let sent = settings.decisive();
        assert_eq!(
            ravel_sdk::network::decode::<Decisive>(&ravel_sdk::network::encode(&sent))
                .expect("the payload round-trips"),
            sent
        );

        let bytes = ravel_sdk::network::encode(&sent);
        assert!(
            ravel_sdk::network::decode::<Decisive>(&bytes[..bytes.len() - 1]).is_err(),
            "a buffer that ends inside a field is refused rather than read short"
        );
    }

    #[test]
    fn the_deciding_payload_travels_to_a_client_under_this_entrys_version() {
        let contract = Decisive::contract();
        assert_eq!(contract.direction(), Direction::ToClient);
        assert_eq!(contract.version(), PACKET_VERSION);
        assert_eq!(contract.id().id().path(), DECISIVE_PACKET);
    }

    #[test]
    fn a_client_adopts_the_servers_answers_and_keeps_its_own_presentation() {
        let server = Settings {
            repair_cost: 8,
            repair_enabled: false,
            tweak_ineffective: Matcher::parse("example:spade"),
            repair_overrides: Table::parse("example:pick=example:ingot@2", "host"),
            ..Settings::default()
        };

        let mut client = Settings {
            tooltip_enabled: false,
            tooltip_excluded: Matcher::parse("example:*"),
            ..Settings::default()
        };
        client.adopt(&server.decisive(), "host");

        assert_eq!(client.repair_cost, 8);
        assert!(!client.repair_enabled);
        assert!(
            client
                .tweak_ineffective
                .matches("example:spade", &|_| false)
        );
        assert_eq!(client.repair_overrides.rows().len(), 1);
        assert!(!client.tooltip_enabled);
        assert!(
            client
                .tooltip_excluded
                .matches("example:anything", &|_| false)
        );
    }

    #[test]
    fn a_cost_outside_its_declared_range_is_clamped_when_it_is_adopted() {
        let mut decisive = Settings::default().decisive();
        decisive.repair_cost = 4_000_000_000;
        decisive.repair_enchantment_cost = 900;
        let mut client = Settings::default();
        client.adopt(&decisive, "host");
        assert_eq!(client.repair_cost, COST_RANGE.1);
        assert_eq!(client.repair_enchantment_cost, ENCHANTMENT_COST_RANGE.1);
    }

    #[test]
    fn an_adopted_table_keeps_the_order_the_server_wrote() {
        let server = Settings {
            repair_overrides: Table::parse(
                "example:pick=example:a@3,example:pick=example:a@9",
                "host",
            ),
            ..Settings::default()
        };
        let mut client = Settings::default();
        client.adopt(&server.decisive(), "host");
        assert_eq!(
            client
                .repair_overrides
                .multiplier("example:pick", "example:a", &|_| false, &|_| false),
            Some(3.0)
        );
    }
}
