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
//! library, which is what gates the creative listing; [`publish`] binds them.
//!
//! Each setting-key constant matches its key in [`DOCUMENT`].

use ravel_sdk::config::{
    ConfigurationDeclaration, DeclarationError, LayeredConfiguration, ValueError,
};
use ravel_sdk::content::ContentIdError;
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
      "valueSchema": { "type": "integer", "minimum": 10, "maximum": 4294967295 },
      "default": 1000,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "growth.pulse_seconds",
      "title": "Seconds between growth pulses",
      "valueSchema": { "type": "integer", "minimum": 1, "maximum": 4294967295 },
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
#[derive(Clone, Debug, PartialEq)]
pub struct DropSettings {
    /// Chance a defeated pathfinding creature drops the first tier material.
    pub creature_material_chance: f64,
    /// Chance a harvested crop yields the weak fertiliser.
    pub fertiliser_chance: f64,
    /// A second seed may drop.
    pub second_seed: bool,
    /// Drops from the first boss.
    pub first_boss: BossDrops,
    /// Drops from the second boss.
    pub second_boss: BossDrops,
}

/// Equipment properties and behaviours.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GearSettings {
    /// Properties of top-tier equipment.
    pub top: TopGearSettings,
    /// Automated stand-in players may use the wetting tool.
    pub automated_wetting: bool,
    /// A full top-tier armour set grants its growth aura.
    pub armour_set_aura: bool,
}

/// Properties shared by the two top equipment tiers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopGearSettings {
    /// Top-tier tools are enchantable at a table.
    pub tools_enchantable: bool,
    /// Top-tier armour is unbreakable.
    pub armour_unbreakable: bool,
}

/// Which ore deposits generate.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct DepositSettings {
    /// Chance the third deposit substitutes the rarer ore.
    pub rare_substitution_chance: f64,
    /// The first ore deposit generates.
    pub first: bool,
    /// The second ore deposit generates.
    pub second: bool,
    /// The third deposit generates.
    pub third: bool,
}

/// The values in force for one launch.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Uses on the basic ritual catalyst.
    pub ritual_catalyst_uses: u32,
    /// Seconds between growth pulses.
    pub growth_pulse_seconds: u32,
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
}

impl Default for Settings {
    fn default() -> Self {
        let resolved = LayeredConfiguration::resolve(
            declaration().expect("the packaged declaration is valid"),
            None,
            None,
        )
        .expect("the shipped defaults resolve");
        Self::read(&resolved).expect("declared settings have matching types")
    }
}
impl DropSettings {
    fn read(resolved: &LayeredConfiguration) -> Result<Self, ValueError> {
        Ok(Self {
            creature_material_chance: resolved.number(DROPS_CREATURE_MATERIAL_CHANCE)?,
            fertiliser_chance: resolved.number(DROPS_FERTILISER_CHANCE)?,
            second_seed: resolved.boolean(DROPS_SECOND_SEED)?,
            first_boss: BossDrops {
                material: resolved.boolean(DROPS_FIRST_BOSS_MATERIAL)?,
                dust: resolved.boolean(DROPS_FIRST_BOSS_DUST)?,
            },
            second_boss: BossDrops {
                material: resolved.boolean(DROPS_SECOND_BOSS_MATERIAL)?,
                dust: resolved.boolean(DROPS_SECOND_BOSS_DUST)?,
            },
        })
    }
}

impl GearSettings {
    fn read(resolved: &LayeredConfiguration) -> Result<Self, ValueError> {
        Ok(Self {
            top: TopGearSettings {
                tools_enchantable: resolved.boolean(GEAR_TOP_TOOLS_ENCHANTABLE)?,
                armour_unbreakable: resolved.boolean(GEAR_TOP_ARMOUR_UNBREAKABLE)?,
            },
            automated_wetting: resolved.boolean(GEAR_AUTOMATED_WETTING)?,
            armour_set_aura: resolved.boolean(GEAR_ARMOUR_SET_AURA)?,
        })
    }
}

impl DepositSettings {
    fn read(resolved: &LayeredConfiguration) -> Result<Self, ValueError> {
        Ok(Self {
            rare_substitution_chance: resolved.number(WORLD_RARE_SUBSTITUTION_CHANCE)?,
            first: resolved.boolean(WORLD_FIRST_DEPOSIT)?,
            second: resolved.boolean(WORLD_SECOND_DEPOSIT)?,
            third: resolved.boolean(WORLD_THIRD_DEPOSIT)?,
        })
    }
}

impl Settings {
    /// Reads the values in force out of a resolved configuration.
    ///
    /// Returns a typed access error if a key is missing or has the wrong kind.
    pub fn read(resolved: &LayeredConfiguration) -> Result<Self, ValueError> {
        Ok(Self {
            ritual_catalyst_uses: u32::try_from(resolved.integer(RITUAL_CATALYST_USES)?)
                .map_err(|_| ValueError::IntegerRange)?,
            growth_pulse_seconds: u32::try_from(resolved.integer(GROWTH_PULSE_SECONDS)?)
                .map_err(|_| ValueError::IntegerRange)?,
            drops: DropSettings::read(resolved)?,
            growth_require_effective_ground: resolved.boolean(GROWTH_REQUIRE_EFFECTIVE_GROUND)?,
            ground_convert_by_material: resolved.boolean(GROUND_CONVERT_BY_MATERIAL)?,
            recipes_plain_grid_seeds: resolved.boolean(RECIPES_PLAIN_GRID_SEEDS)?,
            gear: GearSettings::read(resolved)?,
            deposits: DepositSettings::read(resolved)?,
        })
    }
}

type FlagBinding = (&'static str, fn(&Settings) -> bool);

const PUBLISHED: [FlagBinding; 14] = [
    (DROPS_SECOND_SEED, |settings| settings.drops.second_seed),
    (GROWTH_REQUIRE_EFFECTIVE_GROUND, |settings| {
        settings.growth_require_effective_ground
    }),
    (DROPS_FIRST_BOSS_MATERIAL, |settings| {
        settings.drops.first_boss.material
    }),
    (DROPS_SECOND_BOSS_MATERIAL, |settings| {
        settings.drops.second_boss.material
    }),
    (DROPS_FIRST_BOSS_DUST, |settings| {
        settings.drops.first_boss.dust
    }),
    (DROPS_SECOND_BOSS_DUST, |settings| {
        settings.drops.second_boss.dust
    }),
    (GROUND_CONVERT_BY_MATERIAL, |settings| {
        settings.ground_convert_by_material
    }),
    (RECIPES_PLAIN_GRID_SEEDS, |settings| {
        settings.recipes_plain_grid_seeds
    }),
    (GEAR_TOP_TOOLS_ENCHANTABLE, |settings| {
        settings.gear.top.tools_enchantable
    }),
    (GEAR_TOP_ARMOUR_UNBREAKABLE, |settings| {
        settings.gear.top.armour_unbreakable
    }),
    (GEAR_AUTOMATED_WETTING, |settings| {
        settings.gear.automated_wetting
    }),
    (GEAR_ARMOUR_SET_AURA, |settings| {
        settings.gear.armour_set_aura
    }),
    (WORLD_FIRST_DEPOSIT, |settings| settings.deposits.first),
    (WORLD_SECOND_DEPOSIT, |settings| settings.deposits.second),
];

/// Publishes those flags into the shared table under this entry's namespace.
///
/// A flag is published as a constant rather than a live predicate, because the
/// shared library binds a reference once and a value that moved afterwards
/// would not be seen by an already-bound reference anyway.
///
/// # Errors
///
/// Returns [`ContentIdError`] when a declared flag binding has an invalid identity.
pub fn publish(table: &mut FlagTable, settings: &Settings) -> Result<(), ContentIdError> {
    for (key, value) in PUBLISHED {
        let id = flag_id(key)?;
        table.publish_constant(id, value(settings));
    }
    Ok(())
}

/// The flag identity a setting key is published under.
///
/// # Errors
///
/// Returns [`ContentIdError`] when the setting key cannot form a legal flag identity.
pub fn flag_id(key: &str) -> Result<FlagId, ContentIdError> {
    FlagId::new(crate::MOD_ID, &key.replace('.', "_"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravel_sdk::config::SettingSide;
    use ravel_sdk::json;
    use ravel_sdk::{JsonValue, Side};

    fn admits(key: &str, value: &JsonValue) -> bool {
        let Ok(parsed) = key.parse() else {
            return false;
        };
        declaration().is_ok_and(|declared| {
            declared
                .setting(&parsed)
                .is_some_and(|setting| setting.value_schema.check(value).is_ok())
        })
    }
    fn travels_to_clients() -> bool {
        declaration().is_ok_and(|declared| {
            declared.settings().iter().any(|setting| {
                matches!(
                    setting.side,
                    SettingSide::DedicatedServer | SettingSide::IntegratedServer
                )
            })
        })
    }

    fn resolved_on(key: &str, side: Side) -> bool {
        let Ok(key) = key.parse() else {
            return false;
        };
        declaration().is_ok_and(|declared| {
            declared
                .setting(&key)
                .is_some_and(|setting| setting.side.applies_to(side))
        })
    }

    #[test]
    fn integer_settings_accept_u32_max_and_refuse_larger_values_before_reading() {
        let document = ravel_sdk::config::ValueDocument::parse(
            &json!({
                "schemaVersion": 1,
                "owner": crate::MOD_ID,
                "values": {
                    RITUAL_CATALYST_USES: u32::MAX,
                    GROWTH_PULSE_SECONDS: u32::MAX,
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
        .expect("the maximum representable values resolve");
        let settings = Settings::read(&resolved).expect("the upper bounds fit u32");
        assert_eq!(settings.ritual_catalyst_uses, u32::MAX);
        assert_eq!(settings.growth_pulse_seconds, u32::MAX);
        assert!(!admits(RITUAL_CATALYST_USES, &json!(5_000_000_000_u64)));
        assert!(!admits(GROWTH_PULSE_SECONDS, &json!(5_000_000_000_u64)));
    }

    #[test]
    fn a_widened_declaration_returns_an_integer_range_error() {
        for default in ["\"default\": 1000,", "\"default\": 10,"] {
            let changed = DOCUMENT
                .replace("4294967295", "5000000000")
                .replace(default, "\"default\": 4294967296,");
            let declared = ConfigurationDeclaration::parse(&changed).expect("a legal declaration");
            let resolved = LayeredConfiguration::resolve(declared, None, None)
                .expect("the widened declaration resolves");
            assert_eq!(Settings::read(&resolved), Err(ValueError::IntegerRange));
        }
    }

    #[test]
    fn a_missing_declared_key_is_returned_instead_of_defaulted() {
        let changed = DOCUMENT.replace(DROPS_CREATURE_MATERIAL_CHANCE, "drops.renamed");
        let declared = ConfigurationDeclaration::parse(&changed).expect("a legal declaration");
        let resolved = LayeredConfiguration::resolve(declared, None, None)
            .expect("the changed declaration resolves");
        assert!(Settings::read(&resolved).is_err());
    }

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
        let settings = Settings::read(&resolved).expect("declared settings have matching types");
        assert_eq!(settings.drops.creature_material_chance, 0.75);
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
        assert!((shipped.drops.creature_material_chance - 0.2).abs() < f64::EPSILON);
        assert!((shipped.drops.fertiliser_chance - 0.1).abs() < f64::EPSILON);
        assert!((shipped.deposits.rare_substitution_chance - 0.05).abs() < f64::EPSILON);
        assert_eq!(shipped.ritual_catalyst_uses, 1000);
        assert_eq!(shipped.growth_pulse_seconds, 10);
    }

    #[test]
    fn the_declared_bounds_refuse_a_value_outside_them() {
        assert!(admits(DROPS_FERTILISER_CHANCE, &json!(0.5)));
        assert!(!admits(DROPS_FERTILISER_CHANCE, &json!(1.5)));
        assert!(!admits(DROPS_FERTILISER_CHANCE, &json!(-0.1)));
        assert!(admits(RITUAL_CATALYST_USES, &json!(10)));
        assert!(!admits(RITUAL_CATALYST_USES, &json!(9)));
        assert!(admits(GROWTH_PULSE_SECONDS, &json!(1)));
        assert!(!admits(GROWTH_PULSE_SECONDS, &json!(0)));
        assert!(!admits("nothing.declares.this", &json!(true)));
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
        let expected: std::collections::BTreeSet<_> = PUBLISHED
            .iter()
            .map(|(key, _)| flag_id(key).expect("a legal flag identity"))
            .collect();
        let mut table = FlagTable::new();
        let settings = Settings::default();
        publish(&mut table, &settings).expect("legal flag bindings");
        assert_eq!(
            table
                .published()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>(),
            expected
        );
        for (key, value) in PUBLISHED {
            let id = flag_id(key).expect("a legal flag identity");
            assert_eq!(table.enabled(&id), value(&settings));
        }
    }
}
