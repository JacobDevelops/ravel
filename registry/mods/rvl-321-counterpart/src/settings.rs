//! What a player and a pack may change about this entry.
//!
//! The specification declares one common-scoped document in two groupings,
//! twenty settings in all, resolved at startup and never synchronised to a
//! joining client. Ravel resolves one document per mod and carries a side on
//! each setting, so the grouping is carried by the key prefix and every setting
//! is declared common. Nothing here travels, which is what preserves the
//! original arrangement: what a client needs to know arrives through the
//! payloads of section thirteen instead.
//!
//! Fourteen of these are also published as named flags through the shared
//! library, which is what gates the creative listing; [`published_flags`] names
//! them and [`publish`] binds them.
//!
//! Each setting-key constant matches its key in [`DOCUMENT`].

use ravel_sdk::config::{
    ConfigurationDeclaration, DeclarationError, LayeredConfiguration, SettingSide,
};
use ravel_sdk::{JsonValue, Side};
use rvl_042_counterpart::flags::{FlagId, FlagTable};

pub const DROPS_CREATURE_MATERIAL_CHANCE: &str = "drops.creature_material_chance";
pub const RITUAL_CATALYST_USES: &str = "ritual.catalyst_uses";
pub const GROWTH_PULSE_SECONDS: &str = "growth.pulse_seconds";
pub const DROPS_FERTILISER_CHANCE: &str = "drops.fertiliser_chance";
pub const DROPS_SECOND_SEED: &str = "drops.second_seed";
pub const GROWTH_REQUIRE_EFFECTIVE_GROUND: &str = "growth.require_effective_ground";
pub const DROPS_FIRST_BOSS_MATERIAL: &str = "drops.first_boss_material";
pub const DROPS_SECOND_BOSS_MATERIAL: &str = "drops.second_boss_material";
pub const DROPS_FIRST_BOSS_DUST: &str = "drops.first_boss_dust";
pub const DROPS_SECOND_BOSS_DUST: &str = "drops.second_boss_dust";
pub const GROUND_CONVERT_BY_MATERIAL: &str = "ground.convert_by_material";
pub const RECIPES_PLAIN_GRID_SEEDS: &str = "recipes.plain_grid_seeds";
pub const GEAR_TOP_TOOLS_ENCHANTABLE: &str = "gear.top_tools_enchantable";
pub const GEAR_TOP_ARMOUR_UNBREAKABLE: &str = "gear.top_armour_unbreakable";
pub const GEAR_AUTOMATED_WETTING: &str = "gear.automated_wetting";
pub const GEAR_ARMOUR_SET_AURA: &str = "gear.armour_set_aura";
pub const WORLD_FIRST_DEPOSIT: &str = "world.first_deposit";
pub const WORLD_SECOND_DEPOSIT: &str = "world.second_deposit";
pub const WORLD_THIRD_DEPOSIT: &str = "world.third_deposit";
pub const WORLD_RARE_SUBSTITUTION_CHANCE: &str = "world.rare_substitution_chance";

/// Every key this document declares, in declaration order.
pub const KEYS: [&str; 20] = [
    DROPS_CREATURE_MATERIAL_CHANCE,
    RITUAL_CATALYST_USES,
    GROWTH_PULSE_SECONDS,
    DROPS_FERTILISER_CHANCE,
    DROPS_SECOND_SEED,
    GROWTH_REQUIRE_EFFECTIVE_GROUND,
    DROPS_FIRST_BOSS_MATERIAL,
    DROPS_SECOND_BOSS_MATERIAL,
    DROPS_FIRST_BOSS_DUST,
    DROPS_SECOND_BOSS_DUST,
    GROUND_CONVERT_BY_MATERIAL,
    RECIPES_PLAIN_GRID_SEEDS,
    GEAR_TOP_TOOLS_ENCHANTABLE,
    GEAR_TOP_ARMOUR_UNBREAKABLE,
    GEAR_AUTOMATED_WETTING,
    GEAR_ARMOUR_SET_AURA,
    WORLD_FIRST_DEPOSIT,
    WORLD_SECOND_DEPOSIT,
    WORLD_THIRD_DEPOSIT,
    WORLD_RARE_SUBSTITUTION_CHANCE,
];

/// The declaration this counterpart packages.
pub const DOCUMENT: &str = r#"{
  "schemaVersion": 1,
  "owner": "rvl_321_counterpart",
  "version": 1,
  "settings": [
    {
      "key": "drops.creature_material_chance",
      "title": "Chance a defeated pathfinding creature drops the first tier material",
      "valueSchema": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
      "default": 0.2,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "ritual.catalyst_uses",
      "title": "Uses on the basic ritual catalyst",
      "valueSchema": { "type": "integer", "minimum": 10 },
      "default": 1000,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "growth.pulse_seconds",
      "title": "Seconds between growth pulses",
      "valueSchema": { "type": "integer", "minimum": 1 },
      "default": 10,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "drops.fertiliser_chance",
      "title": "Chance a harvested crop yields the weak fertiliser",
      "valueSchema": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
      "default": 0.1,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "drops.second_seed",
      "title": "A second seed may drop",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "growth.require_effective_ground",
      "title": "Crops require their own tier's ground to grow",
      "valueSchema": { "type": "boolean" },
      "default": false,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "drops.first_boss_material",
      "title": "The first boss creature drops tier material",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "drops.second_boss_material",
      "title": "The second boss creature drops tier material",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "drops.first_boss_dust",
      "title": "The first boss creature drops the rare dust",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "drops.second_boss_dust",
      "title": "The second boss creature drops the rare dust",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "ground.convert_by_material",
      "title": "Right-clicking plain tilled ground with a tier material converts it",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "recipes.plain_grid_seeds",
      "title": "Plain-grid seed recipes are generated",
      "valueSchema": { "type": "boolean" },
      "default": false,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "gear.top_tools_enchantable",
      "title": "The two top tool tiers are enchantable at a table",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "gear.top_armour_unbreakable",
      "title": "The two top armour tiers are unbreakable",
      "valueSchema": { "type": "boolean" },
      "default": false,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "gear.automated_wetting",
      "title": "Automated stand-in players may use the wetting tool",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "gear.armour_set_aura",
      "title": "A full top-tier armour set grants its growth aura",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "world.first_deposit",
      "title": "The first ore deposit generates",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "world.second_deposit",
      "title": "The second ore deposit generates",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "world.third_deposit",
      "title": "The third deposit generates",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "world.rare_substitution_chance",
      "title": "Chance the third deposit substitutes the rarer ore",
      "valueSchema": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
      "default": 0.05,
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

/// Drops controlled independently for one boss.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BossDrops {
    /// The boss drops tier material.
    pub material: bool,
    /// The boss drops rare dust.
    pub dust: bool,
}

/// Optional seed and boss drops.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropSettings {
    /// A second seed may drop.
    pub second_seed: bool,
    /// Drops from the first boss.
    pub first_boss: BossDrops,
    /// Drops from the second boss.
    pub second_boss: BossDrops,
}

/// Properties of the two top equipment tiers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopGearSettings {
    /// Tools are enchantable at a table.
    pub tools_enchantable: bool,
    /// Armour is unbreakable.
    pub armour_unbreakable: bool,
}

/// Equipment properties and behaviours.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GearSettings {
    /// Properties of the two top equipment tiers.
    pub top: TopGearSettings,
    /// Automated stand-in players may use the wetting tool.
    pub automated_wetting: bool,
    /// A full top-tier armour set grants its growth aura.
    pub armour_set_aura: bool,
}

/// Which ore deposits generate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DepositSettings {
    /// The first ore deposit generates.
    pub first: bool,
    /// The second ore deposit generates.
    pub second: bool,
    /// The third deposit generates.
    pub third: bool,
}

/// The values in force for one launch.
#[derive(Clone, PartialEq)]
pub struct Settings {
    /// Chance a defeated pathfinding creature drops the first tier material.
    pub drops_creature_material_chance: f64,
    /// Uses on the basic ritual catalyst.
    pub ritual_catalyst_uses: u32,
    /// Seconds between growth pulses.
    pub growth_pulse_seconds: u32,
    /// Chance a harvested crop yields the weak fertiliser.
    pub drops_fertiliser_chance: f64,
    /// Optional seed and boss drops.
    pub drops: DropSettings,
    /// Crops require their own tier's ground to grow.
    pub growth_require_effective_ground: bool,
    /// Right-clicking plain tilled ground with a tier material converts it.
    pub ground_convert_by_material: bool,
    /// Plain-grid seed recipes are generated.
    pub recipes_plain_grid_seeds: bool,
    /// Equipment properties and behaviours.
    pub gear: GearSettings,
    /// Which ore deposits generate.
    pub deposits: DepositSettings,
    /// Chance the third deposit substitutes the rarer ore.
    pub world_rare_substitution_chance: f64,
}

// Keep diagnostic fields flat and in their established order.
impl std::fmt::Debug for Settings {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Settings")
            .field(
                "drops_creature_material_chance",
                &self.drops_creature_material_chance,
            )
            .field("ritual_catalyst_uses", &self.ritual_catalyst_uses)
            .field("growth_pulse_seconds", &self.growth_pulse_seconds)
            .field("drops_fertiliser_chance", &self.drops_fertiliser_chance)
            .field("drops_second_seed", &self.drops.second_seed)
            .field(
                "growth_require_effective_ground",
                &self.growth_require_effective_ground,
            )
            .field("drops_first_boss_material", &self.drops.first_boss.material)
            .field(
                "drops_second_boss_material",
                &self.drops.second_boss.material,
            )
            .field("drops_first_boss_dust", &self.drops.first_boss.dust)
            .field("drops_second_boss_dust", &self.drops.second_boss.dust)
            .field(
                "ground_convert_by_material",
                &self.ground_convert_by_material,
            )
            .field("recipes_plain_grid_seeds", &self.recipes_plain_grid_seeds)
            .field(
                "gear_top_tools_enchantable",
                &self.gear.top.tools_enchantable,
            )
            .field(
                "gear_top_armour_unbreakable",
                &self.gear.top.armour_unbreakable,
            )
            .field("gear_automated_wetting", &self.gear.automated_wetting)
            .field("gear_armour_set_aura", &self.gear.armour_set_aura)
            .field("world_first_deposit", &self.deposits.first)
            .field("world_second_deposit", &self.deposits.second)
            .field("world_third_deposit", &self.deposits.third)
            .field(
                "world_rare_substitution_chance",
                &self.world_rare_substitution_chance,
            )
            .finish()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            drops_creature_material_chance: 0.2,
            ritual_catalyst_uses: 1000,
            growth_pulse_seconds: 10,
            drops_fertiliser_chance: 0.1,
            drops: DropSettings {
                second_seed: true,
                first_boss: BossDrops {
                    material: true,
                    dust: true,
                },
                second_boss: BossDrops {
                    material: true,
                    dust: true,
                },
            },
            growth_require_effective_ground: false,
            ground_convert_by_material: true,
            recipes_plain_grid_seeds: false,
            gear: GearSettings {
                top: TopGearSettings {
                    tools_enchantable: true,
                    armour_unbreakable: false,
                },
                automated_wetting: true,
                armour_set_aura: true,
            },
            deposits: DepositSettings {
                first: true,
                second: true,
                third: true,
            },
            world_rare_substitution_chance: 0.05,
        }
    }
}
impl DropSettings {
    fn read(resolved: &LayeredConfiguration, shipped: &Self) -> Self {
        Self {
            second_seed: DROPS_SECOND_SEED
                .parse()
                .ok()
                .and_then(|key| resolved.boolean(&key).ok())
                .unwrap_or(shipped.second_seed),
            first_boss: BossDrops {
                material: DROPS_FIRST_BOSS_MATERIAL
                    .parse()
                    .ok()
                    .and_then(|key| resolved.boolean(&key).ok())
                    .unwrap_or(shipped.first_boss.material),
                dust: DROPS_FIRST_BOSS_DUST
                    .parse()
                    .ok()
                    .and_then(|key| resolved.boolean(&key).ok())
                    .unwrap_or(shipped.first_boss.dust),
            },
            second_boss: BossDrops {
                material: DROPS_SECOND_BOSS_MATERIAL
                    .parse()
                    .ok()
                    .and_then(|key| resolved.boolean(&key).ok())
                    .unwrap_or(shipped.second_boss.material),
                dust: DROPS_SECOND_BOSS_DUST
                    .parse()
                    .ok()
                    .and_then(|key| resolved.boolean(&key).ok())
                    .unwrap_or(shipped.second_boss.dust),
            },
        }
    }
}

impl GearSettings {
    fn read(resolved: &LayeredConfiguration, shipped: &Self) -> Self {
        Self {
            top: TopGearSettings {
                tools_enchantable: GEAR_TOP_TOOLS_ENCHANTABLE
                    .parse()
                    .ok()
                    .and_then(|key| resolved.boolean(&key).ok())
                    .unwrap_or(shipped.top.tools_enchantable),
                armour_unbreakable: GEAR_TOP_ARMOUR_UNBREAKABLE
                    .parse()
                    .ok()
                    .and_then(|key| resolved.boolean(&key).ok())
                    .unwrap_or(shipped.top.armour_unbreakable),
            },
            automated_wetting: GEAR_AUTOMATED_WETTING
                .parse()
                .ok()
                .and_then(|key| resolved.boolean(&key).ok())
                .unwrap_or(shipped.automated_wetting),
            armour_set_aura: GEAR_ARMOUR_SET_AURA
                .parse()
                .ok()
                .and_then(|key| resolved.boolean(&key).ok())
                .unwrap_or(shipped.armour_set_aura),
        }
    }
}

impl DepositSettings {
    fn read(resolved: &LayeredConfiguration, shipped: &Self) -> Self {
        Self {
            first: WORLD_FIRST_DEPOSIT
                .parse()
                .ok()
                .and_then(|key| resolved.boolean(&key).ok())
                .unwrap_or(shipped.first),
            second: WORLD_SECOND_DEPOSIT
                .parse()
                .ok()
                .and_then(|key| resolved.boolean(&key).ok())
                .unwrap_or(shipped.second),
            third: WORLD_THIRD_DEPOSIT
                .parse()
                .ok()
                .and_then(|key| resolved.boolean(&key).ok())
                .unwrap_or(shipped.third),
        }
    }
}

impl Settings {
    /// Reads the values in force out of a resolved configuration.
    ///
    /// A key the resolution does not carry keeps its shipped default, so a
    /// partial resolution still yields a usable set rather than refusing.
    #[must_use]
    pub fn read(resolved: &LayeredConfiguration) -> Self {
        let shipped = Self::default();
        Self {
            drops_creature_material_chance: DROPS_CREATURE_MATERIAL_CHANCE
                .parse()
                .ok()
                .and_then(|key| resolved.number(&key).ok())
                .unwrap_or(shipped.drops_creature_material_chance),
            ritual_catalyst_uses: RITUAL_CATALYST_USES
                .parse()
                .ok()
                .and_then(|key| resolved.integer(&key).ok())
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(shipped.ritual_catalyst_uses),
            growth_pulse_seconds: GROWTH_PULSE_SECONDS
                .parse()
                .ok()
                .and_then(|key| resolved.integer(&key).ok())
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(shipped.growth_pulse_seconds),
            drops_fertiliser_chance: DROPS_FERTILISER_CHANCE
                .parse()
                .ok()
                .and_then(|key| resolved.number(&key).ok())
                .unwrap_or(shipped.drops_fertiliser_chance),
            drops: DropSettings::read(resolved, &shipped.drops),
            growth_require_effective_ground: GROWTH_REQUIRE_EFFECTIVE_GROUND
                .parse()
                .ok()
                .and_then(|key| resolved.boolean(&key).ok())
                .unwrap_or(shipped.growth_require_effective_ground),
            ground_convert_by_material: GROUND_CONVERT_BY_MATERIAL
                .parse()
                .ok()
                .and_then(|key| resolved.boolean(&key).ok())
                .unwrap_or(shipped.ground_convert_by_material),
            recipes_plain_grid_seeds: RECIPES_PLAIN_GRID_SEEDS
                .parse()
                .ok()
                .and_then(|key| resolved.boolean(&key).ok())
                .unwrap_or(shipped.recipes_plain_grid_seeds),
            gear: GearSettings::read(resolved, &shipped.gear),
            deposits: DepositSettings::read(resolved, &shipped.deposits),
            world_rare_substitution_chance: WORLD_RARE_SUBSTITUTION_CHANCE
                .parse()
                .ok()
                .and_then(|key| resolved.number(&key).ok())
                .unwrap_or(shipped.world_rare_substitution_chance),
        }
    }

    /// Whether a value is a legal reading of the named key.
    ///
    /// The pulse interval and the catalyst use count are read live on every use
    /// rather than at resolution, so a value that arrives after startup is held
    /// to the same bounds the document declares.
    #[must_use]
    pub fn admits(key: &str, value: &JsonValue) -> bool {
        let Ok(parsed) = key.parse() else {
            return false;
        };
        declaration().is_ok_and(|declared| {
            declared
                .setting(&parsed)
                .is_some_and(|setting| setting.value_schema.check(value).is_ok())
        })
    }
}

/// The fourteen settings republished as named flags.
///
/// Fourteen is the twelve booleans of the general grouping plus the first two
/// deposit switches. A flag carries a boolean, so the two fractions and the two
/// integers cannot be flags at all, and that leaves exactly fourteen booleans
/// once the third deposit switch is set aside as the one the substitution
/// chance is read beside.
#[must_use]
pub fn published_flags() -> Vec<&'static str> {
    vec![
        DROPS_SECOND_SEED,
        GROWTH_REQUIRE_EFFECTIVE_GROUND,
        DROPS_FIRST_BOSS_MATERIAL,
        DROPS_SECOND_BOSS_MATERIAL,
        DROPS_FIRST_BOSS_DUST,
        DROPS_SECOND_BOSS_DUST,
        GROUND_CONVERT_BY_MATERIAL,
        RECIPES_PLAIN_GRID_SEEDS,
        GEAR_TOP_TOOLS_ENCHANTABLE,
        GEAR_TOP_ARMOUR_UNBREAKABLE,
        GEAR_AUTOMATED_WETTING,
        GEAR_ARMOUR_SET_AURA,
        WORLD_FIRST_DEPOSIT,
        WORLD_SECOND_DEPOSIT,
    ]
}

/// Publishes those flags into the shared table under this entry's namespace.
///
/// A flag is published as a constant rather than a live predicate, because the
/// shared library binds a reference once and a value that moved afterwards
/// would not be seen by an already-bound reference anyway.
pub fn publish(table: &mut FlagTable, settings: &Settings) -> usize {
    let mut published = 0;
    for key in published_flags() {
        let Some(id) = flag_id(key) else {
            continue;
        };
        table.publish_constant(id, flag_value(settings, key));
        published += 1;
    }
    published
}

/// The flag identity a setting key is published under.
#[must_use]
pub fn flag_id(key: &str) -> Option<FlagId> {
    FlagId::new(crate::MOD_ID, &key.replace('.', "_")).ok()
}

fn flag_value(settings: &Settings, key: &str) -> bool {
    match key {
        DROPS_SECOND_SEED => settings.drops.second_seed,
        GROWTH_REQUIRE_EFFECTIVE_GROUND => settings.growth_require_effective_ground,
        DROPS_FIRST_BOSS_MATERIAL => settings.drops.first_boss.material,
        DROPS_SECOND_BOSS_MATERIAL => settings.drops.second_boss.material,
        DROPS_FIRST_BOSS_DUST => settings.drops.first_boss.dust,
        DROPS_SECOND_BOSS_DUST => settings.drops.second_boss.dust,
        GROUND_CONVERT_BY_MATERIAL => settings.ground_convert_by_material,
        RECIPES_PLAIN_GRID_SEEDS => settings.recipes_plain_grid_seeds,
        GEAR_TOP_TOOLS_ENCHANTABLE => settings.gear.top.tools_enchantable,
        GEAR_TOP_ARMOUR_UNBREAKABLE => settings.gear.top.armour_unbreakable,
        GEAR_AUTOMATED_WETTING => settings.gear.automated_wetting,
        GEAR_ARMOUR_SET_AURA => settings.gear.armour_set_aura,
        WORLD_FIRST_DEPOSIT => settings.deposits.first,
        WORLD_SECOND_DEPOSIT => settings.deposits.second,
        _ => false,
    }
}

/// Whether any shipped setting would have to travel to a joining client.
#[must_use]
pub fn travels_to_clients() -> bool {
    declaration().is_ok_and(|declared| {
        declared.settings().iter().any(|setting| {
            matches!(
                setting.side,
                SettingSide::DedicatedServer | SettingSide::IntegratedServer
            )
        })
    })
}

/// Whether a setting is one a given side resolves at all.
#[must_use]
pub fn resolved_on(key: &str, side: Side) -> bool {
    let Ok(key) = key.parse() else {
        return false;
    };
    declaration().is_ok_and(|declared| {
        declared
            .setting(&key)
            .is_some_and(|setting| setting.side.applies_to(side))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravel_sdk::json;

    #[test]
    fn typed_reads_apply_scalars_and_each_group_of_switches() {
        let document = ravel_sdk::config::ValueDocument::parse(
            &json!({
                "schemaVersion": 1,
                "owner": crate::MOD_ID,
                "values": {
                    DROPS_CREATURE_MATERIAL_CHANCE: 0.75,
                    RITUAL_CATALYST_USES: 10,
                    GROWTH_PULSE_SECONDS: 1,
                    DROPS_SECOND_SEED: false,
                    DROPS_FIRST_BOSS_MATERIAL: false,
                    DROPS_SECOND_BOSS_DUST: false,
                    GEAR_TOP_ARMOUR_UNBREAKABLE: true,
                    GEAR_AUTOMATED_WETTING: false,
                    WORLD_SECOND_DEPOSIT: false,
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
        let settings = Settings::read(&resolved);
        assert_eq!(settings.drops_creature_material_chance, 0.75);
        assert_eq!(settings.ritual_catalyst_uses, 10);
        assert_eq!(settings.growth_pulse_seconds, 1);
        assert!(!settings.drops.second_seed);
        assert!(!settings.drops.first_boss.material);
        assert!(!settings.drops.second_boss.dust);
        assert!(settings.gear.top.armour_unbreakable);
        assert!(!settings.gear.automated_wetting);
        assert!(!settings.deposits.second);
        assert!(settings.deposits.first);
    }

    #[test]
    fn the_packaged_document_declares_twenty_settings_and_every_named_key() {
        let declared = declaration().expect("the packaged document parses");
        assert_eq!(declared.settings().len(), 20);
        for key in KEYS {
            let parsed = key.parse().expect("every declared key is legal");
            assert!(declared.setting(&parsed).is_some(), "declares {key}");
        }
    }

    #[test]
    fn sixteen_settings_are_general_and_four_are_world() {
        let world = KEYS.iter().filter(|key| key.starts_with("world.")).count();
        assert_eq!(world, 4);
        assert_eq!(KEYS.len() - world, 16);
    }

    #[test]
    fn the_two_settings_that_ship_off_are_the_ground_gate_and_the_plain_grid_recipes() {
        let shipped = Settings::default();
        assert!(!shipped.growth_require_effective_ground);
        assert!(!shipped.recipes_plain_grid_seeds);
        assert!(!shipped.gear.top.armour_unbreakable);
        assert!(shipped.drops.second_seed);
        assert!(shipped.ground_convert_by_material);
    }

    #[test]
    fn the_shipped_numbers_are_the_ones_the_document_states() {
        let shipped = Settings::default();
        assert!((shipped.drops_creature_material_chance - 0.2).abs() < f64::EPSILON);
        assert!((shipped.drops_fertiliser_chance - 0.1).abs() < f64::EPSILON);
        assert!((shipped.world_rare_substitution_chance - 0.05).abs() < f64::EPSILON);
        assert_eq!(shipped.ritual_catalyst_uses, 1000);
        assert_eq!(shipped.growth_pulse_seconds, 10);
    }

    #[test]
    fn the_declared_bounds_refuse_a_value_outside_them() {
        assert!(Settings::admits(DROPS_FERTILISER_CHANCE, &json!(0.5)));
        assert!(!Settings::admits(DROPS_FERTILISER_CHANCE, &json!(1.5)));
        assert!(!Settings::admits(DROPS_FERTILISER_CHANCE, &json!(-0.1)));
        assert!(Settings::admits(RITUAL_CATALYST_USES, &json!(10)));
        assert!(!Settings::admits(RITUAL_CATALYST_USES, &json!(9)));
        assert!(Settings::admits(GROWTH_PULSE_SECONDS, &json!(1)));
        assert!(!Settings::admits(GROWTH_PULSE_SECONDS, &json!(0)));
        assert!(!Settings::admits("nothing.declares.this", &json!(true)));
    }

    #[test]
    fn nothing_declared_here_has_to_travel_to_a_joining_client() {
        assert!(!travels_to_clients());
        assert!(resolved_on(GROWTH_PULSE_SECONDS, Side::Client));
        assert!(resolved_on(GROWTH_PULSE_SECONDS, Side::DedicatedServer));
        assert!(!resolved_on("nothing.declares.this", Side::Client));
    }

    #[test]
    fn fourteen_flags_are_published_and_each_carries_its_settings_value() {
        assert_eq!(
            published_flags(),
            [
                DROPS_SECOND_SEED,
                GROWTH_REQUIRE_EFFECTIVE_GROUND,
                DROPS_FIRST_BOSS_MATERIAL,
                DROPS_SECOND_BOSS_MATERIAL,
                DROPS_FIRST_BOSS_DUST,
                DROPS_SECOND_BOSS_DUST,
                GROUND_CONVERT_BY_MATERIAL,
                RECIPES_PLAIN_GRID_SEEDS,
                GEAR_TOP_TOOLS_ENCHANTABLE,
                GEAR_TOP_ARMOUR_UNBREAKABLE,
                GEAR_AUTOMATED_WETTING,
                GEAR_ARMOUR_SET_AURA,
                WORLD_FIRST_DEPOSIT,
                WORLD_SECOND_DEPOSIT,
            ],
            "each of the fourteen is pinned by name, so a substitution is not a count"
        );
        let mut table = FlagTable::new();
        let settings = Settings::default();
        assert_eq!(publish(&mut table, &settings), 14);

        let on = flag_id(DROPS_SECOND_SEED).expect("a legal flag identity");
        let off = flag_id(RECIPES_PLAIN_GRID_SEEDS).expect("a legal flag identity");
        assert!(table.enabled(&on));
        assert!(!table.enabled(&off));
        assert_eq!(table.published().count(), 14);
    }
}
