//! What a harvest yields.
//!
//! Crops carry no loot table: the block computes its drops directly, so the
//! result is not overridable from data and is unaffected by fortune, by silk
//! touch and by whatever tool broke it. Everything here is a pure function of
//! the age, the tier, the ground below, the settings in force and the draws,
//! and the draws are supplied by the caller rather than taken from a global
//! source so that every rule with a number can be held to an assertion.

use rvl_042_counterpart::Stack;
use rvl_042_counterpart::arithmetic::{Mature, sweep_harvest};

use crate::model::{Crop, Tier};

/// The further chance a crop standing on its own tier's ground receives.
pub const OWN_GROUND_BONUS: f64 = 0.1;

/// What the block below the crop is.
///
/// A drop request that carries no origin has no block below at all, which is a
/// different thing from a block below that is not tiered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Below {
    /// The tier value of the ground below, when the block below is some tier's.
    pub tiered: Option<u32>,
    /// Whether the block below is this crop's own tier's ground exactly.
    pub own_ground: bool,
}

impl Below {
    /// Reads the block below against the tier table.
    #[must_use]
    pub fn read(block: Option<&str>, tier: &Tier, tiers: &[Tier]) -> Self {
        let tiered = tiers
            .iter()
            .find(|held| held.is_effective_ground(block))
            .map(|held| held.value);
        Self {
            tiered,
            own_ground: tier.is_effective_ground(block),
        }
    }
}

/// The three independent draws a harvest takes.
///
/// The produce draw and the seed draw are compared against the same chance and
/// are still separate draws, so the two second drops are independent rather
/// than correlated. Every comparison is strictly less than.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Draws {
    /// The draw deciding a second produce.
    pub produce: f64,
    /// The draw deciding a second seed.
    pub seed: f64,
    /// The draw deciding the weak fertiliser.
    pub fertiliser: f64,
    /// The draw deciding the first tier crop's bump on an even ground tier.
    pub even_bump: f64,
}

/// What one harvest produced.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Yield {
    /// How much of the crop's produce dropped.
    pub produce: u32,
    /// How many seeds dropped.
    pub seeds: u32,
    /// How much weak fertiliser dropped.
    pub fertiliser: u32,
}

/// The chance a harvest drops a second produce and a second seed.
///
/// The two additions are not exclusive. A crop on its own tier's ground
/// receives both, which with the shipped defaults is 0.2, against 0.1 on
/// another tier's ground and nothing on plain ground.
#[must_use]
pub fn second_drop_chance(crop: &Crop, tier: &Tier, below: Option<Below>) -> f64 {
    if !tier.second_drop {
        return 0.0;
    }
    let Some(below) = below else {
        return 0.0;
    };
    let mut chance = 0.0;
    if below.tiered.is_some() {
        chance += crop.own_or_tier_chance(tier);
    }
    if crop.respects_effective_ground && below.own_ground {
        chance += OWN_GROUND_BONUS;
    }
    chance.clamp(0.0, 1.0)
}

/// What the ordinary crop yields.
///
/// The produce count is set to one before the origin is consulted, so a drop
/// request carrying no origin still yields one produce and one seed at maximum
/// age and skips the three draws alone. Below maximum age the crop yields one
/// seed and nothing else.
#[must_use]
pub fn general(
    crop: &Crop,
    tier: &Tier,
    below: Option<Below>,
    mature: bool,
    second_seed: bool,
    fertiliser_chance: f64,
    draws: Draws,
) -> Yield {
    if !mature {
        return Yield {
            produce: 0,
            seeds: 1,
            fertiliser: 0,
        };
    }
    let mut answer = Yield {
        produce: 1,
        seeds: 1,
        fertiliser: 0,
    };
    if below.is_none() {
        return answer;
    }
    let chance = second_drop_chance(crop, tier, below);
    if draws.produce < chance {
        answer.produce += 1;
    }
    if second_seed && draws.seed < chance {
        answer.seeds += 1;
    }
    if draws.fertiliser < fertiliser_chance {
        answer.fertiliser = 1;
    }
    answer
}

/// What the first tier's own crop yields.
///
/// Its produce is arithmetic over the tier value of the ground below and it
/// drops no fertiliser at all. The half-chance bump on an even value is what
/// makes the progression smooth rather than stepped.
#[must_use]
pub fn tier_one(
    crop: &Crop,
    tier: &Tier,
    below: Option<Below>,
    mature: bool,
    second_seed: bool,
    draws: Draws,
) -> Yield {
    if !mature {
        return Yield {
            produce: 0,
            seeds: 1,
            fertiliser: 0,
        };
    }
    let mut answer = Yield {
        produce: 1,
        seeds: 1,
        fertiliser: 0,
    };
    let Some(below) = below else {
        return answer;
    };
    if let Some(value) = below.tiered {
        answer.produce = produce_from_value(value, draws.even_bump);
    }
    let chance = second_drop_chance(crop, tier, Some(below));
    if second_seed && draws.seed < chance {
        answer.seeds += 1;
    }
    answer
}

/// The produce count a ground tier value yields for the first tier's crop.
#[must_use]
pub fn produce_from_value(value: u32, even_bump: f64) -> u32 {
    let base = value.div_ceil(2);
    if value > 1 && value.is_multiple_of(2) && even_bump < 0.5 {
        base + 1
    } else {
        base
    }
}

/// What the harvesting machine keeps out of a broken crop's drops.
///
/// Exactly one item matching the crop's own seed is removed and the rest is
/// kept, and the crop is reset to age zero rather than broken. The withheld
/// seed is the replant, and it is the same rule the shared library's sweeping
/// harvest applies, so the one implementation there decides both.
#[must_use]
pub fn machine_harvest(drops: Vec<Stack>, seed: Option<String>) -> Vec<Stack> {
    let mature = Mature { drops, seed };
    let mut allow = |_| true;
    sweep_harvest((0, 0, 0), 0, &|_| Some(mature.clone()), &mut allow).drops
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Crafted;

    fn tiers() -> Vec<Tier> {
        (1..=5)
            .map(|value| {
                let mut tier = Tier::new(&format!("test:tier_{value}"), value);
                tier.adjust(
                    &format!("test:ground_{value}"),
                    &format!("test:material_{value}"),
                );
                tier
            })
            .collect()
    }

    fn crop() -> Crop {
        Crop::new("test:crop", "test:tier_1", "test:kind", Crafted::Nothing)
    }

    #[test]
    fn the_shipped_defaults_give_a_fifth_on_own_ground_and_a_tenth_elsewhere() {
        let tiers = tiers();
        let crop = crop();
        let own = Below::read(Some("test:ground_1"), &tiers[0], &tiers);
        let other = Below::read(Some("test:ground_3"), &tiers[0], &tiers);
        let plain = Below::read(Some("test:dirt"), &tiers[0], &tiers);

        assert!((second_drop_chance(&crop, &tiers[0], Some(own)) - 0.2).abs() < 1e-9);
        assert!((second_drop_chance(&crop, &tiers[0], Some(other)) - 0.1).abs() < 1e-9);
        assert!(second_drop_chance(&crop, &tiers[0], Some(plain)).abs() < 1e-9);
    }

    #[test]
    fn a_tier_with_second_drops_off_stops_at_zero_and_the_result_is_clamped() {
        let tiers = tiers();
        let mut tier = tiers[0].clone();
        tier.second_drop = false;
        let own = Below::read(Some("test:ground_1"), &tier, &tiers);
        assert!(second_drop_chance(&crop(), &tier, Some(own)).abs() < 1e-9);

        let mut generous = tiers[0].clone();
        generous.second_drop_chance = 5.0;
        let own = Below::read(Some("test:ground_1"), &generous, &tiers);
        assert!((second_drop_chance(&crop(), &generous, Some(own)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_crop_that_does_not_respect_effective_ground_misses_the_second_addition() {
        let tiers = tiers();
        let mut crop = crop();
        crop.respects_effective_ground = false;
        let own = Below::read(Some("test:ground_1"), &tiers[0], &tiers);
        assert!((second_drop_chance(&crop, &tiers[0], Some(own)) - 0.1).abs() < 1e-9);
    }

    #[test]
    fn a_crops_own_chance_replaces_the_tiers_and_the_sentinel_defers() {
        let tiers = tiers();
        let mut crop = crop();
        crop.second_drop_chance = 0.5;
        let other = Below::read(Some("test:ground_3"), &tiers[0], &tiers);
        assert!((second_drop_chance(&crop, &tiers[0], Some(other)) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn an_immature_crop_yields_one_seed_and_nothing_else() {
        let tiers = tiers();
        let below = Below::read(Some("test:ground_1"), &tiers[0], &tiers);
        let answer = general(
            &crop(),
            &tiers[0],
            Some(below),
            false,
            true,
            1.0,
            Draws::default(),
        );
        assert_eq!(
            answer,
            Yield {
                produce: 0,
                seeds: 1,
                fertiliser: 0
            }
        );
    }

    #[test]
    fn a_request_with_no_origin_still_yields_one_of_each_and_skips_the_draws() {
        let tiers = tiers();
        let certain = Draws {
            produce: 0.0,
            seed: 0.0,
            fertiliser: 0.0,
            even_bump: 0.0,
        };
        let answer = general(&crop(), &tiers[0], None, true, true, 1.0, certain);
        assert_eq!(
            answer,
            Yield {
                produce: 1,
                seeds: 1,
                fertiliser: 0
            },
            "the produce is set before the origin is consulted"
        );
    }

    #[test]
    fn the_produce_and_seed_draws_are_separate_against_the_same_chance() {
        let tiers = tiers();
        let below = Below::read(Some("test:ground_1"), &tiers[0], &tiers);
        let split = Draws {
            produce: 0.05,
            seed: 0.5,
            fertiliser: 1.0,
            even_bump: 1.0,
        };
        let answer = general(&crop(), &tiers[0], Some(below), true, true, 0.1, split);
        assert_eq!(answer.produce, 2, "0.05 is under the shipped 0.2");
        assert_eq!(answer.seeds, 1, "0.5 is not, and the draw is its own");
        assert_eq!(answer.fertiliser, 0);
    }

    #[test]
    fn the_second_seed_setting_switches_off_the_seed_draw_alone() {
        let tiers = tiers();
        let below = Below::read(Some("test:ground_1"), &tiers[0], &tiers);
        let certain = Draws::default();
        let with = general(&crop(), &tiers[0], Some(below), true, true, 0.1, certain);
        let without = general(&crop(), &tiers[0], Some(below), true, false, 0.1, certain);
        assert_eq!(with.seeds, 2);
        assert_eq!(without.seeds, 1);
        assert_eq!(with.produce, without.produce);
    }

    #[test]
    fn the_fertiliser_draw_reads_its_own_chance_and_is_strictly_less_than() {
        let tiers = tiers();
        let below = Below::read(Some("test:ground_1"), &tiers[0], &tiers);
        let boundary = Draws {
            produce: 1.0,
            seed: 1.0,
            fertiliser: 0.1,
            even_bump: 1.0,
        };
        let answer = general(&crop(), &tiers[0], Some(below), true, true, 0.1, boundary);
        assert_eq!(
            answer.fertiliser, 0,
            "a draw equal to the chance does not pass"
        );

        let under = Draws {
            fertiliser: 0.09,
            ..boundary
        };
        let answer = general(&crop(), &tiers[0], Some(below), true, true, 0.1, under);
        assert_eq!(answer.fertiliser, 1);
    }

    #[test]
    fn the_first_tiers_crop_reads_the_ground_value_and_bumps_on_an_even_one() {
        let never = 1.0;
        let always = 0.0;
        assert_eq!(produce_from_value(1, always), 1);
        assert_eq!(produce_from_value(2, never), 1);
        assert_eq!(produce_from_value(2, always), 2);
        assert_eq!(
            produce_from_value(3, always),
            2,
            "an odd value takes no bump"
        );
        assert_eq!(produce_from_value(4, never), 2);
        assert_eq!(produce_from_value(4, always), 3);
        assert_eq!(produce_from_value(5, always), 3);
    }

    #[test]
    fn the_first_tiers_crop_drops_no_fertiliser_and_leaves_untiered_ground_at_one() {
        let tiers = tiers();
        let certain = Draws::default();
        let plain = Below::read(Some("test:dirt"), &tiers[0], &tiers);
        let answer = tier_one(&crop(), &tiers[0], Some(plain), true, true, certain);
        assert_eq!(answer.produce, 1);
        assert_eq!(answer.fertiliser, 0);

        let high = Below::read(Some("test:ground_5"), &tiers[0], &tiers);
        let answer = tier_one(&crop(), &tiers[0], Some(high), true, true, certain);
        assert_eq!(answer.produce, 3);
        assert_eq!(answer.fertiliser, 0);
    }

    #[test]
    fn the_first_tiers_crop_with_no_origin_skips_the_arithmetic_and_the_seed_draw() {
        let tiers = tiers();
        let certain = Draws::default();
        let answer = tier_one(&crop(), &tiers[0], None, true, true, certain);
        assert_eq!(
            answer,
            Yield {
                produce: 1,
                seeds: 1,
                fertiliser: 0
            }
        );
    }

    #[test]
    fn the_machine_withholds_exactly_one_seed_and_keeps_the_rest() {
        let drops = vec![Stack::new("test:material", 3), Stack::new("test:seeds", 2)];
        let kept = machine_harvest(drops, Some("test:seeds".to_owned()));
        assert_eq!(
            kept,
            [Stack::new("test:material", 3), Stack::new("test:seeds", 1)]
        );

        let single = machine_harvest(
            vec![Stack::new("test:seeds", 1)],
            Some("test:seeds".to_owned()),
        );
        assert_eq!(single.len(), 0, "the last seed is the replant");

        let unresolved = machine_harvest(vec![Stack::new("test:seeds", 2)], None);
        assert_eq!(unresolved, [Stack::new("test:seeds", 2)]);
    }
}
