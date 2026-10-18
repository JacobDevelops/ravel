//! What a player and a pack may change about this entry, and the flags it
//! publishes.
//!
//! The specification declares one common, startup-scoped document with two
//! groupings and eight values. Ravel resolves one document per mod and carries a
//! side on each setting, so the grouping is carried by the key prefix and every
//! setting is declared common. Nothing here travels to a joining client, which
//! is the arrangement the specification records: a client and a server that
//! disagree about these values disagree silently.
//!
//! Five of the eight are published as named flags through the shared library
//! under this entry's own namespace. Two further flags are *read* from the
//! dependency's namespace and are never published here; [`consumed_flags`] names
//! them, and the direction is what [`published_flags`] and that function keep
//! apart.
//!
//! Each setting-key constant matches its key in [`DOCUMENT`].

use ravel_sdk::config::{
    ConfigurationDeclaration, DeclarationError, LayeredConfiguration, SettingSide, ValueError,
};
use ravel_sdk::content::ContentIdError;
use ravel_sdk::{JsonValue, Side};
use rvl_042_counterpart::flags::{FlagId, FlagTable};
use rvl_321_counterpart::settings as depended;

pub const DROPS_SOUL_CHANCE: &str = "drops.soul_chance";
pub const DROPS_SCALE_COUNT: &str = "drops.scale_count";
pub const CROPS_FERTILISABLE: &str = "crops.fertilisable";
pub const FOODS_EFFECT_SECONDS: &str = "foods.effect_seconds";
pub const WORLD_LOWER_SHARD_DEPOSIT: &str = "world.lower_shard_deposit";
pub const WORLD_LOWER_ESSENCE_DEPOSIT: &str = "world.lower_essence_deposit";
pub const WORLD_OUTER_SHARD_DEPOSIT: &str = "world.outer_shard_deposit";
pub const WORLD_OUTER_ESSENCE_DEPOSIT: &str = "world.outer_essence_deposit";

/// Every key this document declares, in declaration order.
pub const KEYS: [&str; 8] = [
    DROPS_SOUL_CHANCE,
    DROPS_SCALE_COUNT,
    CROPS_FERTILISABLE,
    FOODS_EFFECT_SECONDS,
    WORLD_LOWER_SHARD_DEPOSIT,
    WORLD_LOWER_ESSENCE_DEPOSIT,
    WORLD_OUTER_SHARD_DEPOSIT,
    WORLD_OUTER_ESSENCE_DEPOSIT,
];

/// The upper bound the duration setting declares.
pub const MAXIMUM_EFFECT_SECONDS: u32 = 2_147_483_647;

/// The declaration this counterpart packages.
pub const DOCUMENT: &str = r#"{
  "schemaVersion": 1,
  "owner": "rvl_002_counterpart",
  "version": 1,
  "settings": [
    {
      "key": "drops.soul_chance",
      "title": "Chance a defeated wither-class boss drops the soul item",
      "valueSchema": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
      "default": 0.35,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "drops.scale_count",
      "title": "Scales a defeated dragon-class boss drops",
      "description": "Scales a defeated dragon-class boss drops. Zero turns the drop off.",
      "valueSchema": { "type": "integer", "minimum": 0, "maximum": 64 },
      "default": 8,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "crops.fertilisable",
      "title": "The sixth tier's crops accept the growth accelerant",
      "description": "The sixth tier's crops accept the growth accelerant. Read once, at registration.",
      "valueSchema": { "type": "boolean" },
      "default": false,
      "side": "common",
      "restartRequired": true,
      "packOverride": "allowed"
    },
    {
      "key": "foods.effect_seconds",
      "title": "Seconds of effect each food grants",
      "description": "Seconds of effect each food grants, added to whatever is already running.",
      "valueSchema": { "type": "integer", "minimum": 1, "maximum": 2147483647 },
      "default": 180,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "world.lower_shard_deposit",
      "title": "The lower dimension's shard deposit generates",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "world.lower_essence_deposit",
      "title": "The lower dimension's essence deposit generates",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "world.outer_shard_deposit",
      "title": "The outer dimension's shard deposit generates",
      "valueSchema": { "type": "boolean" },
      "default": true,
      "side": "common",
      "restartRequired": false,
      "packOverride": "allowed"
    },
    {
      "key": "world.outer_essence_deposit",
      "title": "The outer dimension's essence deposit generates",
      "valueSchema": { "type": "boolean" },
      "default": true,
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

/// Which of a dimension's two material deposits generate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deposits {
    /// Whether the shard deposit generates.
    pub shard: bool,
    /// Whether the essence deposit generates.
    pub essence: bool,
}

/// The values in force for one launch.
#[derive(Clone, PartialEq)]
pub struct Settings {
    /// Chance a defeated wither-class boss drops the soul item.
    pub drops_soul_chance: f64,
    /// Scales a defeated dragon-class boss drops.
    pub drops_scale_count: u32,
    /// The sixth tier's crops accept the growth accelerant.
    pub crops_fertilisable: bool,
    /// Seconds of effect each food grants.
    pub foods_effect_seconds: u32,
    /// Deposit generation in the lower dimension.
    pub world_lower: Deposits,
    /// Deposit generation in the outer dimension.
    pub world_outer: Deposits,
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Settings")
            .field("drops_soul_chance", &self.drops_soul_chance)
            .field("drops_scale_count", &self.drops_scale_count)
            .field("crops_fertilisable", &self.crops_fertilisable)
            .field("foods_effect_seconds", &self.foods_effect_seconds)
            .field("world_lower_shard_deposit", &self.world_lower.shard)
            .field("world_lower_essence_deposit", &self.world_lower.essence)
            .field("world_outer_shard_deposit", &self.world_outer.shard)
            .field("world_outer_essence_deposit", &self.world_outer.essence)
            .finish()
    }
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

impl Settings {
    /// Reads the values in force out of a resolved configuration.
    ///
    /// Returns a typed access error if a key is missing or has the wrong kind.
    pub fn read(resolved: &LayeredConfiguration) -> Result<Self, ValueError> {
        Ok(Self {
            drops_soul_chance: resolved
                .number(&DROPS_SOUL_CHANCE.parse().expect("a declared key"))?,
            drops_scale_count: u32::try_from(
                resolved.integer(&DROPS_SCALE_COUNT.parse().expect("a declared key"))?,
            )
            .expect("the declared range fits u32"),
            crops_fertilisable: resolved
                .boolean(&CROPS_FERTILISABLE.parse().expect("a declared key"))?,
            foods_effect_seconds: u32::try_from(
                resolved.integer(&FOODS_EFFECT_SECONDS.parse().expect("a declared key"))?,
            )
            .expect("the declared range fits u32"),
            world_lower: Deposits {
                shard: resolved
                    .boolean(&WORLD_LOWER_SHARD_DEPOSIT.parse().expect("a declared key"))?,
                essence: resolved
                    .boolean(&WORLD_LOWER_ESSENCE_DEPOSIT.parse().expect("a declared key"))?,
            },
            world_outer: Deposits {
                shard: resolved
                    .boolean(&WORLD_OUTER_SHARD_DEPOSIT.parse().expect("a declared key"))?,
                essence: resolved
                    .boolean(&WORLD_OUTER_ESSENCE_DEPOSIT.parse().expect("a declared key"))?,
            },
        })
    }

    /// Whether a value is a legal reading of the named key.
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

type FlagBinding = (&'static str, fn(&Settings) -> bool);

const PUBLISHED: [FlagBinding; 5] = [
    (CROPS_FERTILISABLE, |settings| settings.crops_fertilisable),
    (WORLD_LOWER_SHARD_DEPOSIT, |settings| {
        settings.world_lower.shard
    }),
    (WORLD_LOWER_ESSENCE_DEPOSIT, |settings| {
        settings.world_lower.essence
    }),
    (WORLD_OUTER_SHARD_DEPOSIT, |settings| {
        settings.world_outer.shard
    }),
    (WORLD_OUTER_ESSENCE_DEPOSIT, |settings| {
        settings.world_outer.essence
    }),
];

/// The five settings this entry republishes as named flags.
pub fn published_flags() -> [&'static str; 5] {
    PUBLISHED.map(|(key, _)| key)
}

/// Publishes those flags into the shared table under this entry's namespace.
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

/// The flag identity a setting key of this entry's is published under.
///
/// # Errors
///
/// Returns [`ContentIdError`] when the setting key cannot form a legal flag identity.
pub fn flag_id(key: &str) -> Result<FlagId, ContentIdError> {
    FlagId::new(crate::MOD_ID, &key.replace('.', "_"))
}

/// The two flags this entry reads and never publishes.
///
/// Both live in the dependency's namespace: the tool-enchantability flag that
/// [`crate::tools`] consults and the ground-conversion flag that
/// [`crate::ground`] short-circuits on. Naming them through that package's own
/// [`depended::flag_id`] is what keeps the direction honest — this entry cannot
/// mint a flag in a namespace it does not own.
pub fn consumed_flags() -> Result<Vec<FlagId>, ContentIdError> {
    [
        depended::GEAR_TOP_TOOLS_ENCHANTABLE,
        depended::GROUND_CONVERT_BY_MATERIAL,
    ]
    .into_iter()
    .map(depended::flag_id)
    .collect()
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

    #[test]
    fn a_missing_declared_key_is_returned_instead_of_defaulted() {
        let changed = DOCUMENT.replace(DROPS_SOUL_CHANCE, "drops.renamed");
        let declared = ConfigurationDeclaration::parse(&changed).expect("a legal declaration");
        let resolved = LayeredConfiguration::resolve(declared, None, None)
            .expect("the changed declaration resolves");
        assert!(Settings::read(&resolved).is_err());
    }

    #[test]
    fn typed_reads_apply_numbers_integers_and_boolean_overrides() {
        let document = ravel_sdk::config::ValueDocument::parse(
            &ravel_sdk::json!({
                "schemaVersion": 1,
                "owner": crate::MOD_ID,
                "values": {
                    DROPS_SOUL_CHANCE: 0.75,
                    DROPS_SCALE_COUNT: 0,
                    FOODS_EFFECT_SECONDS: 1,
                    CROPS_FERTILISABLE: false,
                    WORLD_OUTER_ESSENCE_DEPOSIT: false,
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
        assert_eq!(settings.drops_soul_chance, 0.75);
        assert_eq!(settings.drops_scale_count, 0);
        assert_eq!(settings.foods_effect_seconds, 1);
        assert!(!settings.crops_fertilisable);
        assert!(!settings.world_outer.essence);
        assert!(settings.world_lower.shard);
    }

    #[test]
    fn the_document_declares_eight_settings_and_the_shipped_defaults_match_it() {
        let declared = declaration().expect("the packaged document parses");
        assert_eq!(declared.settings().len(), 8);
        for key in KEYS {
            assert!(
                declared
                    .setting(&key.parse().expect("a legal key"))
                    .is_some(),
                "the document declares {key}"
            );
        }
        let shipped = Settings::default();
        assert!((shipped.drops_soul_chance - 0.35).abs() < f64::EPSILON);
        assert_eq!(shipped.drops_scale_count, 8);
        assert!(!shipped.crops_fertilisable);
        assert_eq!(shipped.foods_effect_seconds, 180);
        assert!(shipped.world_lower.shard);
        assert!(shipped.world_lower.essence);
        assert!(shipped.world_outer.shard);
        assert!(shipped.world_outer.essence);
    }

    #[test]
    fn every_bound_the_document_states_is_a_bound_a_value_is_held_to() {
        assert!(Settings::admits(DROPS_SOUL_CHANCE, &JsonValue::from(0.0)));
        assert!(Settings::admits(DROPS_SOUL_CHANCE, &JsonValue::from(1.0)));
        assert!(!Settings::admits(DROPS_SOUL_CHANCE, &JsonValue::from(1.5)));
        assert!(!Settings::admits(DROPS_SOUL_CHANCE, &JsonValue::from(-0.1)));
        assert!(Settings::admits(DROPS_SCALE_COUNT, &JsonValue::from(64)));
        assert!(!Settings::admits(DROPS_SCALE_COUNT, &JsonValue::from(65)));
        assert!(!Settings::admits(FOODS_EFFECT_SECONDS, &JsonValue::from(0)));
        assert!(Settings::admits(FOODS_EFFECT_SECONDS, &JsonValue::from(1)));
        assert!(Settings::admits(
            FOODS_EFFECT_SECONDS,
            &JsonValue::from(MAXIMUM_EFFECT_SECONDS)
        ));
        assert!(!Settings::admits(CROPS_FERTILISABLE, &JsonValue::from(1)));
        assert!(!Settings::admits(
            "nothing.declared",
            &JsonValue::from(true)
        ));
    }

    #[test]
    fn five_flags_are_published_and_the_two_consumed_ones_are_not_in_this_namespace() {
        let mut table = FlagTable::new();
        let settings = Settings::default();
        publish(&mut table, &settings).expect("legal flag bindings");
        assert_eq!(table.published().count(), 5);
        assert_eq!(published_flags().len(), 5);

        let published: Vec<String> = table.published().map(ToString::to_string).collect();
        assert_eq!(published.len(), 5);
        for key in published_flags() {
            let id = flag_id(key).expect("a legal flag name");
            assert!(published.contains(&id.to_string()));
            assert_eq!(id.namespace().as_str(), crate::MOD_ID);
        }

        let consumed = consumed_flags().expect("legal consumed flag bindings");
        assert_eq!(consumed.len(), 2);
        for id in &consumed {
            assert_ne!(
                id.namespace().as_str(),
                crate::MOD_ID,
                "a consumed flag belongs to the entry that declared it"
            );
            assert!(
                !published.contains(&id.to_string()),
                "this entry never publishes a flag it only reads"
            );
        }
    }

    #[test]
    fn a_published_flag_carries_the_value_the_setting_held() {
        let mut table = FlagTable::new();
        let settings = Settings {
            crops_fertilisable: true,
            world_outer: Deposits {
                shard: false,
                essence: true,
            },
            ..Settings::default()
        };
        publish(&mut table, &settings).expect("legal flag bindings");
        let fertilisable = flag_id(CROPS_FERTILISABLE).expect("a legal flag name");
        let deposit = flag_id(WORLD_OUTER_SHARD_DEPOSIT).expect("a legal flag name");
        assert!(table.holds(&fertilisable));
        assert!(table.holds(&deposit), "both are published");
        assert!(table.enabled(&fertilisable));
        assert!(!table.enabled(&deposit), "and one of them is off");
    }

    #[test]
    fn nothing_this_entry_declares_travels_to_a_joining_client() {
        assert!(!travels_to_clients());
        for key in KEYS {
            assert!(resolved_on(key, Side::Client));
            assert!(resolved_on(key, Side::DedicatedServer));
            assert!(resolved_on(key, Side::IntegratedServer));
        }
    }
}
