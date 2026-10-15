//! The six foods: one per tier, one behaviour, three values that differ.
//!
//! The effect list is strictly cumulative in one fixed order, so tier *n* grants
//! exactly the first *n* effects of that order. What is load-bearing is not the
//! list but the two rules around it: the amplifier is a fixed level II at every
//! tier, and a duration adds to whatever is already running rather than
//! replacing it, with no comparison against the running instance at all.

use std::collections::BTreeMap;

/// The amplifier every effect is applied at, at every tier.
///
/// The tier changes which effects you get, never how strong they are.
pub const AMPLIFIER: u32 = 1;

/// Ticks in one second, which is what turns a configured duration into a
/// duration the host stores.
pub const TICKS_PER_SECOND: u32 = 20;

/// How many foods this entry declares.
pub const COUNT: usize = 6;

/// One of the cumulative effects, in the fixed order the tiers grant them.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Effect {
    /// Granted from tier one.
    Absorption,
    /// Granted from tier two.
    MovementSpeed,
    /// Granted from tier three.
    DamageResistance,
    /// Granted from tier four.
    Regeneration,
    /// Granted from tier five.
    DamageBoost,
    /// Granted from tier six.
    DigSpeed,
}

/// The fixed order the effects accumulate in.
pub const ORDER: [Effect; COUNT] = [
    Effect::Absorption,
    Effect::MovementSpeed,
    Effect::DamageResistance,
    Effect::Regeneration,
    Effect::DamageBoost,
    Effect::DigSpeed,
];

/// One food's declared values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Food {
    /// The tier it sits at, 1 through 6.
    pub tier: u32,
    /// Hunger restored.
    pub nutrition: u32,
    /// The saturation modifier.
    pub saturation: f64,
}

/// The six, lowest tier first.
pub const DECLARED: [Food; COUNT] = [
    Food {
        tier: 1,
        nutrition: 6,
        saturation: 0.3,
    },
    Food {
        tier: 2,
        nutrition: 8,
        saturation: 0.4,
    },
    Food {
        tier: 3,
        nutrition: 10,
        saturation: 0.5,
    },
    Food {
        tier: 4,
        nutrition: 12,
        saturation: 0.6,
    },
    Food {
        tier: 5,
        nutrition: 16,
        saturation: 0.7,
    },
    Food {
        tier: 6,
        nutrition: 20,
        saturation: 0.8,
    },
];

/// Every food is edible at full hunger, and every one renders enchanted.
pub const ALWAYS_EDIBLE: bool = true;

/// The food declared at a tier.
#[must_use]
pub fn at(tier: u32) -> Option<Food> {
    DECLARED.into_iter().find(|food| food.tier == tier)
}

/// The effects a tier grants: the first *n* of the fixed order.
///
/// A tier above the six declared grants the whole order rather than reaching
/// past it, and a tier of zero grants nothing.
#[must_use]
pub fn effects(tier: u32) -> &'static [Effect] {
    let taken = usize::try_from(tier).unwrap_or(COUNT).min(COUNT);
    &ORDER[..taken]
}

/// What one effect's duration becomes after this food is finished.
///
/// The new duration is whatever is already running plus the configured
/// duration in ticks, added unconditionally: nothing compares the running
/// instance's amplifier or duration first. Eating six foods in sequence
/// therefore stacks to six times the configured duration.
///
/// The sum saturates rather than wrapping. A duration that overflowed would
/// come back as a shorter effect than the one already running, which is the one
/// outcome this rule must never produce.
#[must_use]
pub fn extended(running: u32, configured_seconds: u32) -> u32 {
    running.saturating_add(configured_seconds.saturating_mul(TICKS_PER_SECOND))
}

/// What finishing a food does to a player's running effects.
///
/// Server-side only, which the caller decides: the specification guards this on
/// the world not being a client world, and there is no other guard inside it.
#[must_use]
pub fn applied(
    tier: u32,
    configured_seconds: u32,
    running: &BTreeMap<Effect, u32>,
) -> BTreeMap<Effect, (u32, u32)> {
    effects(tier)
        .iter()
        .map(|effect| {
            let held = running.get(effect).copied().unwrap_or(0);
            (*effect, (extended(held, configured_seconds), AMPLIFIER))
        })
        .collect()
}

/// The minutes-and-seconds rendering the hover line carries.
///
/// Minutes are the configured seconds divided by sixty with no upper bound, and
/// seconds are the remainder zero-padded to two digits.
#[must_use]
pub fn duration_text(configured_seconds: u32) -> String {
    format!(
        "{}:{:02}",
        configured_seconds.div_euclid(60),
        configured_seconds % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_six_rows_carry_the_values_the_table_states() {
        assert_eq!(DECLARED.len(), COUNT);
        let nutrition: Vec<u32> = DECLARED.iter().map(|food| food.nutrition).collect();
        assert_eq!(nutrition, [6, 8, 10, 12, 16, 20]);
        for (index, food) in DECLARED.iter().enumerate() {
            let step = 0.1 * f64::from(u32::try_from(index).expect("six rows"));
            let expected = 0.3 + step;
            assert!(
                (food.saturation - expected).abs() < 1e-9,
                "saturation rises by a tenth throughout"
            );
        }
    }

    #[test]
    fn nutrition_rises_by_two_a_tier_through_the_fourth_and_by_four_after_it() {
        for index in 1..4 {
            assert_eq!(DECLARED[index].nutrition - DECLARED[index - 1].nutrition, 2);
        }
        for index in 4..COUNT {
            assert_eq!(DECLARED[index].nutrition - DECLARED[index - 1].nutrition, 4);
        }
    }

    #[test]
    fn a_tier_grants_exactly_the_first_n_effects_of_the_fixed_order() {
        for tier in 1..=6u32 {
            let granted = effects(tier);
            assert_eq!(granted.len(), usize::try_from(tier).expect("six tiers"));
            assert_eq!(granted, &ORDER[..granted.len()]);
        }
        assert_eq!(effects(0).len(), 0);
        assert_eq!(effects(7), &ORDER[..], "nothing reaches past the order");
        assert_eq!(effects(u32::MAX), &ORDER[..]);
    }

    #[test]
    fn every_effect_at_every_tier_is_applied_at_level_two() {
        let running = BTreeMap::new();
        for tier in 1..=6u32 {
            for (_, (_, amplifier)) in applied(tier, 180, &running) {
                assert_eq!(amplifier, AMPLIFIER);
            }
        }
    }

    #[test]
    fn a_duration_adds_to_what_is_running_rather_than_replacing_it() {
        let mut running = BTreeMap::new();
        let mut carried = 0;
        for _ in 0..6 {
            let applied = applied(6, 180, &running);
            carried += 180 * TICKS_PER_SECOND;
            for (effect, (duration, _)) in applied {
                assert_eq!(duration, carried);
                running.insert(effect, duration);
            }
        }
        assert_eq!(
            carried,
            6 * 180 * TICKS_PER_SECOND,
            "six foods stack to six times the configured duration"
        );
    }

    #[test]
    fn a_lower_tier_food_refreshes_a_higher_tier_foods_shared_effects() {
        let mut running = BTreeMap::new();
        for (effect, (duration, _)) in applied(6, 180, &running) {
            running.insert(effect, duration);
        }
        let refreshed = applied(1, 180, &running);
        assert_eq!(refreshed.len(), 1);
        let (duration, amplifier) = refreshed[&Effect::Absorption];
        assert_eq!(duration, 2 * 180 * TICKS_PER_SECOND);
        assert_eq!(amplifier, AMPLIFIER, "at the same strength");
        assert!(
            !refreshed.contains_key(&Effect::DigSpeed),
            "a lower tier grants no effect above its own"
        );
    }

    #[test]
    fn an_extension_that_would_overflow_saturates_rather_than_shortening_the_effect() {
        assert_eq!(extended(u32::MAX, 180), u32::MAX);
        assert_eq!(extended(0, u32::MAX), u32::MAX);
        assert_eq!(extended(10, 0), 10);
    }

    #[test]
    fn the_duration_line_pads_seconds_and_leaves_minutes_unbounded() {
        assert_eq!(duration_text(180), "3:00");
        assert_eq!(duration_text(9), "0:09");
        assert_eq!(duration_text(61), "1:01");
        assert_eq!(duration_text(0), "0:00");
        assert_eq!(duration_text(360_000), "6000:00");
    }
}
