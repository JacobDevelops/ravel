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

use ravel_sdk::config::{
    ConfigurationDeclaration, DeclarationError, LayeredConfiguration, SettingSide,
};
use ravel_sdk::content::{ContentId, ContentIdError};
use ravel_sdk::network::{
    Direction, PacketContract, PacketId, PacketReader, PacketWriter, WireError,
};
use ravel_sdk::{Declared, JsonValue, Side};
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

/// The fourteen feature switches, in declaration order.
pub const SWITCH_KEYS: [&str; 14] = [
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
];

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

/// The seventeen flags, each paired with the switch it reads.
///
/// Three switches carry no flag and four switches carry more than one, which is
/// the shape section four records: the two sets are not in bijection. What is
/// fixed here is the binding — every flag reads the switch it is named for, and
/// the test beside it is the test that says so.
///
/// Which four switches carry a second flag is **inferred**: the specification
/// states the counts and the mis-binding but not the whole table.
pub const FLAG_BINDINGS: [(&str, &str); 17] = [
    ("tinted_stone", FEATURE_TINTED_STONE),
    ("light_block", FEATURE_LIGHT_BLOCK),
    ("fuel_fragments", FEATURE_FUEL_FRAGMENTS),
    ("watering_tools", FEATURE_WATERING_TOOLS),
    ("sieve_meshes", FEATURE_SIEVE_MESHES),
    ("headpieces", FEATURE_HEADPIECES),
    ("combined_tools", FEATURE_COMBINED_TOOLS),
    ("first_tier", FEATURE_FIRST_TIER),
    ("first_tier_armour", FEATURE_FIRST_TIER),
    ("first_tier_shearing", FEATURE_FIRST_TIER),
    ("second_tier", FEATURE_SECOND_TIER),
    ("second_tier_armour", FEATURE_SECOND_TIER),
    ("second_tier_shearing", FEATURE_SECOND_TIER),
    ("sweeping_wide", FEATURE_SWEEPING_WIDE),
    ("sweeping_wide_upper", FEATURE_SWEEPING_WIDE),
    ("sweeping_tall", FEATURE_SWEEPING_TALL),
    ("sweeping_tall_upper", FEATURE_SWEEPING_TALL),
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

/// Tool shapes whose repair costs are halved.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RepairDiscounts {
    /// Whether both costs halve for a tiered digging tool.
    pub digging: bool,
    /// Whether both costs halve for the shearing tool.
    pub shearing: bool,
}

/// The values in force for one launch.
#[derive(Clone, PartialEq)]
pub struct Settings {
    /// Whether the two informational tooltips are drawn.
    pub tooltip_enabled: bool,
    /// Tools the mining-speed tooltip is left off.
    pub tooltip_excluded: Matcher,
    /// The fourteen feature switches, in the order of [`SWITCH_KEYS`].
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

// Keep the established flat diagnostic field names and order.
impl std::fmt::Debug for Settings {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Settings")
            .field("tooltip_enabled", &self.tooltip_enabled)
            .field("tooltip_excluded", &self.tooltip_excluded)
            .field("switches", &self.switches)
            .field("repair_enabled", &self.repair_enabled)
            .field("repair_cost", &self.repair_cost)
            .field("repair_enchantment_cost", &self.repair_enchantment_cost)
            .field(
                "repair_strip_enchantments",
                &self.repair_materials.strip_enchantments,
            )
            .field(
                "repair_disable_defaults",
                &self.repair_materials.disable_defaults,
            )
            .field("repair_override_mode", &self.repair_materials.override_mode)
            .field("repair_cheaper_digging", &self.repair_discounts.digging)
            .field("repair_cheaper_shearing", &self.repair_discounts.shearing)
            .field("repair_blacklist", &self.repair_blacklist)
            .field("repair_overrides", &self.repair_overrides)
            .field("tweak_nearly_destroyed", &self.tweak_nearly_destroyed)
            .field("tweak_ineffective", &self.tweak_ineffective)
            .finish()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            tooltip_enabled: true,
            tooltip_excluded: Matcher::default(),
            switches: [true; 14],
            repair_enabled: true,
            repair_cost: 3,
            repair_enchantment_cost: 1,
            repair_materials: RepairMaterials::default(),
            repair_discounts: RepairDiscounts {
                digging: true,
                shearing: true,
            },
            repair_blacklist: Matcher::default(),
            repair_overrides: Table::default(),
            tweak_nearly_destroyed: true,
            tweak_ineffective: Matcher::default(),
        }
    }
}

impl Settings {
    /// Reads the values in force out of a resolved configuration.
    ///
    /// A key the resolution does not carry keeps its shipped default, and a
    /// number outside its declared range is clamped rather than refused, so a
    /// hand-edited file still yields a usable set. `host_namespace` completes an
    /// override selector written without one.
    #[must_use]
    pub fn read(resolved: &LayeredConfiguration, host_namespace: &str) -> Self {
        let shipped = Self::default();
        let mut switches = shipped.switches;
        for (slot, key) in SWITCH_KEYS.iter().enumerate() {
            if let Some(value) = boolean(resolved, key) {
                switches[slot] = value;
            }
        }
        Self {
            tooltip_enabled: boolean(resolved, TOOLTIP_ENABLED).unwrap_or(shipped.tooltip_enabled),
            tooltip_excluded: text(resolved, TOOLTIP_EXCLUDED)
                .map_or_else(Matcher::default, |written| Matcher::parse(&written)),
            switches,
            repair_enabled: boolean(resolved, REPAIR_ENABLED).unwrap_or(shipped.repair_enabled),
            repair_cost: bounded(resolved, REPAIR_COST, COST_RANGE).unwrap_or(shipped.repair_cost),
            repair_enchantment_cost: bounded(
                resolved,
                REPAIR_ENCHANTMENT_COST,
                ENCHANTMENT_COST_RANGE,
            )
            .unwrap_or(shipped.repair_enchantment_cost),
            repair_materials: RepairMaterials {
                strip_enchantments: boolean(resolved, REPAIR_STRIP_ENCHANTMENTS)
                    .unwrap_or(shipped.repair_materials.strip_enchantments),
                disable_defaults: boolean(resolved, REPAIR_DISABLE_DEFAULTS)
                    .unwrap_or(shipped.repair_materials.disable_defaults),
                override_mode: boolean(resolved, REPAIR_OVERRIDE_MODE)
                    .unwrap_or(shipped.repair_materials.override_mode),
            },
            repair_discounts: RepairDiscounts {
                digging: boolean(resolved, REPAIR_CHEAPER_DIGGING)
                    .unwrap_or(shipped.repair_discounts.digging),
                shearing: boolean(resolved, REPAIR_CHEAPER_SHEARING)
                    .unwrap_or(shipped.repair_discounts.shearing),
            },
            repair_blacklist: text(resolved, REPAIR_BLACKLIST)
                .map_or_else(Matcher::default, |written| Matcher::parse(&written)),
            repair_overrides: text(resolved, REPAIR_OVERRIDES)
                .map_or_else(Table::default, |w| Table::parse(&w, host_namespace)),
            tweak_nearly_destroyed: boolean(resolved, TWEAK_NEARLY_DESTROYED)
                .unwrap_or(shipped.tweak_nearly_destroyed),
            tweak_ineffective: text(resolved, TWEAK_INEFFECTIVE)
                .map_or_else(Matcher::default, |written| Matcher::parse(&written)),
        }
    }

    /// Whether one feature switch is on, by its key.
    ///
    /// A key that is not one of the fourteen answers false rather than panicking,
    /// because a flag binding is data and a mistyped one should disable a feature
    /// rather than end a launch.
    #[must_use]
    pub fn switch(&self, key: &str) -> bool {
        SWITCH_KEYS
            .iter()
            .position(|declared| *declared == key)
            .is_some_and(|slot| self.switches[slot])
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

/// The twenty-six deciding settings, as they travel to a joining client.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct Decisive {
    /// The fourteen feature switches, in the order of [`SWITCH_KEYS`].
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

// Match the established payload diagnostics, independently of Rust grouping.
impl std::fmt::Debug for Decisive {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Decisive")
            .field("switches", &self.switches)
            .field("repair_enabled", &self.repair_enabled)
            .field("repair_cost", &self.repair_cost)
            .field("repair_enchantment_cost", &self.repair_enchantment_cost)
            .field(
                "repair_strip_enchantments",
                &self.repair_materials.strip_enchantments,
            )
            .field(
                "repair_disable_defaults",
                &self.repair_materials.disable_defaults,
            )
            .field("repair_override_mode", &self.repair_materials.override_mode)
            .field("repair_cheaper_digging", &self.repair_discounts.digging)
            .field("repair_cheaper_shearing", &self.repair_discounts.shearing)
            .field("repair_blacklist", &self.repair_blacklist)
            .field("repair_overrides", &self.repair_overrides)
            .field("tweak_nearly_destroyed", &self.tweak_nearly_destroyed)
            .field("tweak_ineffective", &self.tweak_ineffective)
            .finish()
    }
}

/// The payload name the deciding settings travel under.
pub const DECISIVE_PACKET: &str = "decisive_settings";

/// The version this entry stamps on every payload it declares.
pub const PACKET_VERSION: u32 = 1;

impl Decisive {
    /// The contract these settings travel under.
    ///
    /// # Errors
    ///
    /// Returns [`ContentIdError`] when the declared name is not a legal path,
    /// which for a constant is a packaging failure.
    pub fn contract() -> Result<PacketContract, ContentIdError> {
        contract(DECISIVE_PACKET, Direction::ToClient)
    }

    /// Writes the body.
    pub fn write(&self, writer: &mut PacketWriter) {
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

    /// Reads the body.
    ///
    /// # Errors
    ///
    /// Returns [`WireError`] when the bytes do not spell these settings.
    pub fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
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

/// Builds a contract this entry owns, at the one version it declares.
///
/// # Errors
///
/// Returns [`ContentIdError`] when the name is not a legal content path.
pub fn contract(name: &str, direction: Direction) -> Result<PacketContract, ContentIdError> {
    let id = ContentId::new(crate::Entry::declared_id().clone(), name)?;
    Ok(PacketContract::new(
        PacketId::new(id),
        direction,
        PACKET_VERSION,
    ))
}

/// Writes a payload body to bytes.
#[must_use]
pub fn to_bytes(write: &dyn Fn(&mut PacketWriter)) -> Vec<u8> {
    let mut writer = PacketWriter::default();
    write(&mut writer);
    writer.finish()
}

/// Binds the seventeen flags over the switches in force.
///
/// Every flag is a constant read off the settings at the moment of binding,
/// which is what section four's one-time scan wants: the switches are
/// restart-scoped and a flag that changed under a data pack would leave a
/// creative listing disagreeing with the recipes beside it.
pub fn publish(table: &mut FlagTable, settings: &Settings, namespace: &str) -> usize {
    let mut published = 0;
    for (flag, key) in FLAG_BINDINGS {
        let Ok(id) = FlagId::new(namespace, flag) else {
            continue;
        };
        table.publish_constant(id, settings.switch(key));
        published += 1;
    }
    published
}

/// Whether any shipped setting would have to travel for a side to have a value.
///
/// Every setting here is declared client or common, so every side resolves the
/// whole document and none has to. What travels is [`Decisive`], and it travels
/// to make the server authoritative rather than because a client holds nothing.
#[must_use]
pub fn resolves_on_every_side() -> bool {
    declaration().is_ok_and(|declaration| {
        declaration
            .settings
            .iter()
            .all(|setting| matches!(setting.side, SettingSide::Client | SettingSide::Common))
    })
}

/// Whether a setting is one a given side resolves at all.
#[must_use]
pub fn resolved_on(key: &str, side: Side) -> bool {
    let Ok(key) = key.parse() else {
        return false;
    };
    declaration().is_ok_and(|declaration| {
        declaration
            .setting(&key)
            .is_some_and(|setting| setting.side.applies_to(side))
    })
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

fn value<'a>(resolved: &'a LayeredConfiguration, key: &str) -> Option<&'a JsonValue> {
    resolved.value(&key.parse().ok()?)
}

fn boolean(resolved: &LayeredConfiguration, key: &str) -> Option<bool> {
    value(resolved, key)?.as_bool()
}

fn text(resolved: &LayeredConfiguration, key: &str) -> Option<String> {
    Some(value(resolved, key)?.as_str()?.to_owned())
}

fn bounded(resolved: &LayeredConfiguration, key: &str, range: (u32, u32)) -> Option<u32> {
    let read = value(resolved, key)?.as_i64()?;
    let clamped = read.clamp(i64::from(range.0), i64::from(range.1));
    u32::try_from(clamped).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(sent: &Decisive) -> Decisive {
        let bytes = to_bytes(&|writer| sent.write(writer));
        let mut reader = PacketReader::new(&bytes);
        let read = Decisive::read(&mut reader).expect("the payload round-trips");
        reader.finish().expect("the payload is read whole");
        read
    }

    #[test]
    fn the_packaged_document_parses_and_declares_twenty_eight_settings() {
        let declaration = declaration().expect("the packaged document parses");
        assert_eq!(declaration.settings.len(), 28);
        for key in CLIENT_KEYS.iter().chain(DECIDING_KEYS.iter()) {
            let parsed = key.parse().expect("every declared key is legal");
            assert!(declaration.setting(&parsed).is_some(), "{key} is declared");
        }
    }

    #[test]
    fn the_two_groups_are_two_and_twenty_six_and_do_not_overlap() {
        assert_eq!(CLIENT_KEYS.len(), 2);
        assert_eq!(DECIDING_KEYS.len(), 26);
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
            .setting(&REPAIR_COST.parse().expect("a legal key"))
            .expect("the cost is declared");
        assert_eq!(cost.value_schema.minimum, Some(1.0));
        assert_eq!(cost.value_schema.maximum, Some(8.0));
        let extra = declaration
            .setting(&REPAIR_ENCHANTMENT_COST.parse().expect("a legal key"))
            .expect("the extra cost is declared");
        assert_eq!(extra.value_schema.minimum, Some(0.0));
        assert_eq!(extra.value_schema.maximum, Some(8.0));
    }

    #[test]
    fn seventeen_flags_are_bound_over_fourteen_switches_and_three_switches_carry_none() {
        assert_eq!(FLAG_BINDINGS.len(), 17);
        let mut carried: Vec<&str> = FLAG_BINDINGS.iter().map(|(_, key)| *key).collect();
        carried.sort_unstable();
        carried.dedup();
        assert_eq!(carried.len(), 11);
        assert_eq!(SWITCH_KEYS.len() - carried.len(), 3);
    }

    #[test]
    fn a_flag_reads_the_switch_it_is_named_for() {
        for (flag, key) in FLAG_BINDINGS {
            let leaf = key.split_once('.').map_or(key, |(_, leaf)| leaf);
            assert!(
                flag.starts_with(leaf),
                "the flag {flag} reads {key}, which it is not named for"
            );
        }
    }

    #[test]
    fn switching_one_feature_off_disables_only_the_flags_named_for_it() {
        let mut settings = Settings::default();
        let light = SWITCH_KEYS
            .iter()
            .position(|key| *key == FEATURE_LIGHT_BLOCK)
            .expect("the light block has a switch");
        settings.switches[light] = false;

        let mut table = FlagTable::default();
        assert_eq!(publish(&mut table, &settings, "rvl_086_counterpart"), 17);
        let named = FlagId::new("rvl_086_counterpart", "light_block").expect("a legal identity");
        let unrelated =
            FlagId::new("rvl_086_counterpart", "watering_tools").expect("a legal identity");
        assert!(!table.enabled(&named));
        assert!(table.enabled(&unrelated));
    }

    #[test]
    fn switching_the_watering_tools_off_leaves_the_light_block_listed() {
        let mut settings = Settings::default();
        let watering = SWITCH_KEYS
            .iter()
            .position(|key| *key == FEATURE_WATERING_TOOLS)
            .expect("the watering tools have a switch");
        settings.switches[watering] = false;

        let mut table = FlagTable::default();
        publish(&mut table, &settings, "rvl_086_counterpart");
        let light = FlagId::new("rvl_086_counterpart", "light_block").expect("a legal identity");
        assert!(table.enabled(&light));
    }

    #[test]
    fn a_switch_key_that_is_not_one_of_the_fourteen_answers_false() {
        assert!(!Settings::default().switch("feature.nothing_named_this"));
        assert!(Settings::default().switch(FEATURE_ATTRACTOR));
    }

    #[test]
    fn deciding_debug_keeps_the_flat_field_order() {
        let settings = Settings::default().decisive();
        assert_eq!(
            format!("{settings:?}"),
            concat!(
                "Decisive { switches: [true, true, true, true, true, true, true, ",
                "true, true, true, true, true, true, true], repair_enabled: true, ",
                "repair_cost: 3, repair_enchantment_cost: 1, ",
                "repair_strip_enchantments: false, repair_disable_defaults: false, ",
                "repair_override_mode: false, repair_cheaper_digging: true, ",
                "repair_cheaper_shearing: true, repair_blacklist: \"\", ",
                "repair_overrides: \"\", tweak_nearly_destroyed: true, ",
                "tweak_ineffective: \"\" }"
            )
        );
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
        settings.switches[0] = false;

        let sent = settings.decisive();
        assert_eq!(round_trip(&sent), sent);

        let bytes = to_bytes(&|writer| sent.write(writer));
        let mut short = PacketReader::new(&bytes[..bytes.len() - 1]);
        assert!(
            Decisive::read(&mut short).is_err(),
            "a buffer that ends inside a field is refused rather than read short"
        );
    }

    #[test]
    fn the_deciding_payload_travels_to_a_client_under_this_entrys_version() {
        let contract = Decisive::contract().expect("the declared name is legal");
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
