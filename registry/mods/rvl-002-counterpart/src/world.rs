//! World generation: four deposits, two in each of two non-overworld
//! dimensions.
//!
//! Ravel publishes no world feature and no biome modifier, so all four
//! registrations are recorded as host-blocked and the parameters are carried as
//! declared data. What is not blocked is the shaping set: a world this entry
//! deposited into is refused once the entry is gone, rather than opening and
//! generating differently from the chunks already on disk.
//!
//! Every rule here is expressible in the host's own declarative vocabulary. No
//! deposit needs a portable predicate, which is the difference between this
//! section and the dependency's.

use ravel_sdk::world_shaping::WorldShaping;

use crate::settings::{self, Settings};

/// Which non-overworld dimension a deposit sits in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dimension {
    /// The lower dimension.
    Lower,
    /// The outer dimension.
    Outer,
}

/// Which material a deposit yields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ore {
    /// The shard ore, the rarer of the two at every location.
    Shard,
    /// The essence ore.
    Essence,
}

/// The chance an air-exposed placement is discarded, which is none.
pub const AIR_DISCARD: f64 = 0.0;

/// The one generation step all four deposits run at.
pub const STEP: &str = "underground_ores";

/// The placements each deposit uses, in the order it applies them.
pub const PLACEMENTS: [&str; 4] = ["count", "square_spread", "uniform_height", "biome_filter"];

/// One deposit, as declared.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Deposit {
    /// The dimension it sits in.
    pub dimension: Dimension,
    /// The material it yields.
    pub ore: Ore,
    /// The cluster size.
    pub size: u32,
    /// Clusters attempted per chunk.
    pub count: u32,
    /// The lowest absolute height a cluster may sit at.
    pub lowest: i32,
    /// The highest absolute height a cluster may sit at.
    pub highest: i32,
    /// Whether the deposit uses the host's own ore feature unchanged.
    pub host_feature: bool,
    /// The setting key its switch is written under.
    pub switch: &'static str,
    /// The lowest count the ore drops without silk touch.
    pub lowest_drop: u32,
    /// The highest count the ore drops without silk touch.
    pub highest_drop: u32,
}

impl Deposit {
    /// Whether this deposit generates under the settings in force.
    #[must_use]
    pub fn generates(&self, settings: &Settings) -> bool {
        match self.switch {
            settings::WORLD_LOWER_SHARD_DEPOSIT => settings.world_lower.shard,
            settings::WORLD_LOWER_ESSENCE_DEPOSIT => settings.world_lower.essence,
            settings::WORLD_OUTER_SHARD_DEPOSIT => settings.world_outer.shard,
            settings::WORLD_OUTER_ESSENCE_DEPOSIT => settings.world_outer.essence,
            _ => false,
        }
    }

    /// How many of the ore's material a break yields, given a roll in
    /// `[0, highest_drop - lowest_drop]`.
    ///
    /// The roll is bounded against the declared span rather than trusted, and
    /// the span is taken saturating because every field here is public: a
    /// deposit built with its bounds the wrong way round answers its own lowest
    /// count instead of panicking or wrapping.
    #[must_use]
    pub const fn dropped(&self, roll: u32) -> u32 {
        let span = self.highest_drop.saturating_sub(self.lowest_drop);
        let bounded = if roll > span { span } else { roll };
        self.lowest_drop + bounded
    }
}

/// The four, in declaration order.
pub const DEPOSITS: [Deposit; 4] = [
    Deposit {
        dimension: Dimension::Lower,
        ore: Ore::Shard,
        size: 8,
        count: 12,
        lowest: 10,
        highest: 100,
        host_feature: true,
        switch: settings::WORLD_LOWER_SHARD_DEPOSIT,
        lowest_drop: 2,
        highest_drop: 5,
    },
    Deposit {
        dimension: Dimension::Lower,
        ore: Ore::Essence,
        size: 8,
        count: 16,
        lowest: 10,
        highest: 100,
        host_feature: true,
        switch: settings::WORLD_LOWER_ESSENCE_DEPOSIT,
        lowest_drop: 3,
        highest_drop: 5,
    },
    Deposit {
        dimension: Dimension::Outer,
        ore: Ore::Shard,
        size: 8,
        count: 12,
        lowest: 10,
        highest: 100,
        host_feature: true,
        switch: settings::WORLD_OUTER_SHARD_DEPOSIT,
        lowest_drop: 2,
        highest_drop: 5,
    },
    Deposit {
        dimension: Dimension::Outer,
        ore: Ore::Essence,
        size: 8,
        count: 16,
        lowest: 10,
        highest: 100,
        host_feature: true,
        switch: settings::WORLD_OUTER_ESSENCE_DEPOSIT,
        lowest_drop: 3,
        highest_drop: 5,
    },
];

/// Whether a biome modifier admits a location.
///
/// The declared biome tag already restricts a modifier to its dimension, and the
/// modifier re-tests the biome set anyway, so the gate is applied twice. Both
/// tests are here because both are observable: a modifier whose tag matched and
/// whose set did not still declines.
#[must_use]
pub fn admits(deposit: &Deposit, tagged: Dimension, in_set: bool, settings: &Settings) -> bool {
    deposit.generates(settings) && deposit.dimension == tagged && in_set
}

/// This entry declared into a world's shaping set.
///
/// # Errors
///
/// Returns [`ravel_sdk::ModIdError`] when the entry's own identity is not a
/// legal mod id, which for a constant is a packaging failure.
pub fn shaping(into: &mut WorldShaping) -> Result<(), ravel_sdk::ModIdError> {
    into.declare(crate::MOD_ID.parse()?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deposit_built_with_its_bounds_reversed_still_answers_inside_them() {
        let reversed = Deposit {
            lowest_drop: 5,
            highest_drop: 2,
            ..DEPOSITS[0]
        };
        for roll in 0..16 {
            assert_eq!(reversed.dropped(roll), reversed.lowest_drop);
        }
    }

    #[test]
    fn four_deposits_sit_two_to_a_dimension_and_one_ore_each() {
        assert_eq!(DEPOSITS.len(), 4);
        for dimension in [Dimension::Lower, Dimension::Outer] {
            let held: Vec<&Deposit> = DEPOSITS
                .iter()
                .filter(|deposit| deposit.dimension == dimension)
                .collect();
            assert_eq!(held.len(), 2);
            assert_ne!(held[0].ore, held[1].ore);
        }
    }

    #[test]
    fn every_deposit_carries_the_parameters_the_table_states() {
        for deposit in &DEPOSITS {
            assert_eq!(deposit.size, 8);
            assert_eq!((deposit.lowest, deposit.highest), (10, 100));
            assert!(deposit.host_feature, "there is no substitution rule here");
        }
        assert!((AIR_DISCARD - 0.0).abs() < f64::EPSILON);
        assert_eq!(PLACEMENTS.len(), 4);
        assert_eq!(STEP, "underground_ores");
    }

    #[test]
    fn the_shard_deposits_are_rarer_than_the_essence_deposits_at_every_location() {
        for dimension in [Dimension::Lower, Dimension::Outer] {
            let shard = DEPOSITS
                .iter()
                .find(|held| held.dimension == dimension && held.ore == Ore::Shard)
                .expect("a shard deposit");
            let essence = DEPOSITS
                .iter()
                .find(|held| held.dimension == dimension && held.ore == Ore::Essence)
                .expect("an essence deposit");
            assert_eq!((shard.count, essence.count), (12, 16));
            assert!(shard.count < essence.count);
        }
    }

    #[test]
    fn all_four_use_the_same_absolute_height_bounds_in_two_unlike_dimensions() {
        let bounds: Vec<(i32, i32)> = DEPOSITS
            .iter()
            .map(|deposit| (deposit.lowest, deposit.highest))
            .collect();
        assert_eq!(
            bounds
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            1
        );
    }

    #[test]
    fn each_switch_gates_exactly_its_own_deposit() {
        for deposit in &DEPOSITS {
            let mut settings = Settings::default();
            assert!(deposit.generates(&settings));
            settings.world_lower.shard = false;
            settings.world_lower.essence = false;
            settings.world_outer.shard = false;
            settings.world_outer.essence = false;
            assert!(!deposit.generates(&settings));
        }

        let settings = Settings {
            world_outer: settings::Deposits {
                shard: true,
                essence: false,
            },
            ..Settings::default()
        };
        let generating = DEPOSITS
            .iter()
            .filter(|deposit| deposit.generates(&settings))
            .count();
        assert_eq!(generating, 3);
    }

    #[test]
    fn every_switch_a_deposit_names_is_a_key_the_document_declares() {
        for deposit in &DEPOSITS {
            assert!(
                settings::KEYS.contains(&deposit.switch),
                "the switch is a declared setting"
            );
            assert!(
                settings::published_flags().contains(&deposit.switch),
                "and a published flag"
            );
        }
    }

    #[test]
    fn the_biome_gate_is_applied_twice_and_both_tests_can_decline() {
        let settings = Settings::default();
        let deposit = DEPOSITS[0];
        assert!(admits(&deposit, Dimension::Lower, true, &settings));
        assert!(
            !admits(&deposit, Dimension::Lower, false, &settings),
            "the re-test declines where the tag admitted"
        );
        assert!(!admits(&deposit, Dimension::Outer, true, &settings));
    }

    #[test]
    fn an_ore_drops_inside_its_declared_range_whatever_the_roll_is() {
        for deposit in &DEPOSITS {
            for roll in [0, 1, 2, 3, 4, 100, u32::MAX] {
                let dropped = deposit.dropped(roll);
                assert!(dropped >= deposit.lowest_drop);
                assert!(dropped <= deposit.highest_drop);
            }
            assert_eq!(deposit.dropped(0), deposit.lowest_drop);
        }
        let shard = DEPOSITS[0];
        let essence = DEPOSITS[1];
        assert_eq!((shard.lowest_drop, shard.highest_drop), (2, 5));
        assert_eq!((essence.lowest_drop, essence.highest_drop), (3, 5));
    }

    #[test]
    fn a_world_this_entry_deposited_into_names_it_in_its_shaping_set() {
        let mut declared = WorldShaping::default();
        assert!(declared.is_empty());
        shaping(&mut declared).expect("a legal mod id");
        shaping(&mut declared).expect("a legal mod id");
        assert_eq!(declared.mods().len(), 1, "a set holds a member once");
        assert_eq!(declared.mods()[0].as_str(), crate::MOD_ID);
    }
}
