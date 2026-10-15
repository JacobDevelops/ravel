//! Whether a crop may grow, and the four paths that make it grow.
//!
//! The four gates are evaluated together on every attempt, and an attempt that
//! fails any of them is dropped silently rather than refused: the crop stays at
//! the age it had. The same predicate answers the question asked before a
//! fertilising attempt and the attempt itself, which is what stops fertilising
//! from being a way around the gates.
//!
//! Ticking, block state and the biome lookup are the host's. What is here is
//! the predicate, the column rule and the delay arithmetic, and the aura's
//! radius and per-block draw.

use rvl_042_counterpart::Position;
use rvl_042_counterpart::arithmetic::radial_positions;

use crate::model::{Crop, Tier};

/// The crop exempt from the ground gate, named rather than described.
///
/// The exemption is tested against this one crop rather than against a
/// property, so an extension cannot make its own crop exempt by declaring
/// anything. That is the shipped behaviour and it is what an extension's own
/// crops are written against.
pub const EXEMPT_CROP: &str = "rvl_321_counterpart:tier_one_crop";

/// The ranges the six pulse blocks ship with.
pub const PULSE_RANGES: [u32; 6] = [9, 18, 27, 36, 45, 54];

/// The radius of the cube the armour set's aura reaches.
pub const AURA_RADIUS: i32 = 5;

/// The interval, in ticks, between two aura sweeps.
pub const AURA_INTERVAL: u32 = 20;

/// The probability a block inside the aura's cube is ticked.
pub const AURA_PROBABILITY: f64 = 0.5;

/// What the world says about the place a crop is standing in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Surroundings {
    /// The block directly below the crop.
    pub below: Option<String>,
    /// The block two below the crop.
    pub two_below: Option<String>,
    /// The biome at the crop's own position.
    pub biome: Option<String>,
}

/// Which gate refused a growth attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Blocked {
    /// The block two below is not the declared foundation.
    Foundation,
    /// The block below is not this crop's tier's ground.
    Ground,
    /// The biome is not one this crop declares.
    Biome,
}

/// Whether a crop may take a growth step here.
///
/// # Errors
///
/// Answers with the gate that refused. A caller drops the attempt on any of
/// them; the distinction exists so a test can say which rule fired.
pub fn may_grow(
    crop: &Crop,
    tier: &Tier,
    around: &Surroundings,
    require_effective_ground: bool,
) -> Result<(), Blocked> {
    if let Some(foundation) = crop.foundation.as_deref()
        && around.two_below.as_deref() != Some(foundation)
    {
        return Err(Blocked::Foundation);
    }
    if require_effective_ground
        && crop.id != EXEMPT_CROP
        && !tier.is_effective_ground(around.below.as_deref())
    {
        return Err(Blocked::Ground);
    }
    if !crop.biomes.is_empty() {
        let inside = around
            .biome
            .as_deref()
            .is_some_and(|biome| crop.biomes.iter().any(|declared| declared == biome));
        if !inside {
            return Err(Blocked::Biome);
        }
    }
    Ok(())
}

/// What a fertilising attempt did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fertilised {
    /// One ordinary growth step was taken.
    Stepped,
    /// The crop was set straight to its maximum age.
    Matured,
    /// Nothing happened and nothing was consumed.
    Nothing,
}

/// Applies the weak fertiliser: one ordinary step on a fertilisable tier.
///
/// `cancelled` is the host's fertilising event, fired before anything happens
/// and honoured. A cancelled attempt consumes nothing.
#[must_use]
pub fn weak_fertiliser(tier: &Tier, cancelled: bool) -> Fertilised {
    if cancelled || !tier.fertilisable {
        return Fertilised::Nothing;
    }
    Fertilised::Stepped
}

/// Applies the strong fertiliser: straight to the maximum age.
///
/// A crop of a fertilisable tier matures outright. Anything else that is
/// bonemealable takes the ordinary step. The sapling branch is not reachable
/// here; see the notes beside this package.
#[must_use]
pub fn strong_fertiliser(tier: Option<&Tier>, bonemealable: bool, cancelled: bool) -> Fertilised {
    if cancelled {
        return Fertilised::Nothing;
    }
    match tier {
        Some(tier) if tier.fertilisable => Fertilised::Matured,
        None if bonemealable => Fertilised::Stepped,
        Some(_) | None => Fertilised::Nothing,
    }
}

/// Whether a fertilising attempt consumes the item.
#[must_use]
pub const fn consumes_one(outcome: Fertilised) -> bool {
    !matches!(outcome, Fertilised::Nothing)
}

/// The one block a pulse block ticks, out of the column above it.
///
/// The scan starts two blocks above the pulse block and runs to its range plus
/// two. The first bonemealable block found is the only one ticked, so a pulse
/// block under a stack of crops only ever serves the lowest.
#[must_use]
pub fn pulse_target(
    origin: Position,
    range: u32,
    bonemealable: &dyn Fn(Position) -> bool,
) -> Option<Position> {
    let range = i32::try_from(range).unwrap_or(i32::MAX);
    (2..=range.saturating_add(2))
        .map(|step| (origin.0, origin.1.saturating_add(step), origin.2))
        .find(|position| bonemealable(*position))
}

/// The delay, in ticks, before a pulse block schedules itself again.
///
/// The truncation happens before the multiply, so the delay is a whole number
/// of seconds. A configured value of one yields either nothing or a full second
/// depending on the draw, which is visible as a different growth rate from the
/// arithmetic that multiplies first.
#[must_use]
pub fn pulse_delay(seconds: u32, draw: f64) -> u32 {
    let jitter = 0.9 + draw.clamp(0.0, 1.0) * 0.2;
    let whole = num_traits::cast::AsPrimitive::<u32>::as_(f64::from(seconds) * jitter);
    whole.saturating_mul(20)
}

/// The positions the armour set's aura ticks this sweep.
///
/// Every block in the cube that carries the host's crop tag is drawn for
/// separately, so the probability is per block rather than per sweep. A sweep
/// happens once every [`AURA_INTERVAL`] ticks and only while the setting is on
/// and the full set is worn.
pub fn aura_positions(
    standing: Position,
    full_set: bool,
    enabled: bool,
    is_crop: &dyn Fn(Position) -> bool,
    draw: &mut dyn FnMut(Position) -> f64,
) -> Vec<Position> {
    if !full_set || !enabled {
        return Vec::new();
    }
    radial_positions(standing, AURA_RADIUS)
        .into_iter()
        .filter(|position| is_crop(*position))
        .filter(|position| draw(*position) < AURA_PROBABILITY)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Crafted;

    fn tiered() -> Tier {
        let mut tier = Tier::new("test:tier", 1);
        tier.adjust("test:ground", "test:material");
        tier
    }

    fn crop() -> Crop {
        Crop::new("test:crop", "test:tier", "test:kind", Crafted::Nothing)
    }

    #[test]
    fn every_gate_holds_when_nothing_is_declared_and_the_setting_is_off() {
        let around = Surroundings::default();
        assert_eq!(may_grow(&crop(), &tiered(), &around, false), Ok(()));
    }

    #[test]
    fn a_declared_foundation_is_read_two_blocks_below_and_not_one() {
        let mut crop = crop();
        crop.foundation = Some("test:foundation".to_owned());
        let mut around = Surroundings {
            below: Some("test:foundation".to_owned()),
            ..Surroundings::default()
        };
        assert_eq!(
            may_grow(&crop, &tiered(), &around, false),
            Err(Blocked::Foundation)
        );
        around.two_below = Some("test:foundation".to_owned());
        assert_eq!(may_grow(&crop, &tiered(), &around, false), Ok(()));
    }

    #[test]
    fn the_ground_gate_only_applies_when_the_setting_is_on() {
        let around = Surroundings {
            below: Some("test:dirt".to_owned()),
            ..Surroundings::default()
        };
        assert_eq!(may_grow(&crop(), &tiered(), &around, false), Ok(()));
        assert_eq!(
            may_grow(&crop(), &tiered(), &around, true),
            Err(Blocked::Ground)
        );

        let on_own = Surroundings {
            below: Some("test:ground".to_owned()),
            ..Surroundings::default()
        };
        assert_eq!(may_grow(&crop(), &tiered(), &on_own, true), Ok(()));
    }

    #[test]
    fn the_exempt_crop_is_exempt_by_identity_and_nothing_else_is() {
        let around = Surroundings {
            below: Some("test:dirt".to_owned()),
            ..Surroundings::default()
        };
        let mut exempt = crop();
        exempt.id = EXEMPT_CROP.to_owned();
        assert_eq!(may_grow(&exempt, &tiered(), &around, true), Ok(()));

        let mut imitator = crop();
        imitator.id = "other:tier_one_crop".to_owned();
        assert_eq!(
            may_grow(&imitator, &tiered(), &around, true),
            Err(Blocked::Ground),
            "an extension cannot make its own crop exempt"
        );
    }

    #[test]
    fn an_empty_biome_set_means_every_biome_and_a_declared_one_is_exact() {
        let mut crop = crop();
        let mut around = Surroundings {
            biome: Some("test:plains".to_owned()),
            ..Surroundings::default()
        };
        assert_eq!(may_grow(&crop, &tiered(), &around, false), Ok(()));

        crop.biomes = vec!["test:swamp".to_owned()];
        assert_eq!(
            may_grow(&crop, &tiered(), &around, false),
            Err(Blocked::Biome)
        );
        around.biome = Some("test:swamp".to_owned());
        assert_eq!(may_grow(&crop, &tiered(), &around, false), Ok(()));

        around.biome = None;
        assert_eq!(
            may_grow(&crop, &tiered(), &around, false),
            Err(Blocked::Biome)
        );
    }

    #[test]
    fn the_weak_fertiliser_steps_only_on_a_fertilisable_tier_and_honours_a_cancel() {
        let mut tier = tiered();
        assert_eq!(weak_fertiliser(&tier, false), Fertilised::Stepped);
        assert_eq!(weak_fertiliser(&tier, true), Fertilised::Nothing);
        tier.fertilisable = false;
        assert_eq!(weak_fertiliser(&tier, false), Fertilised::Nothing);
        assert!(consumes_one(Fertilised::Stepped));
        assert!(!consumes_one(Fertilised::Nothing));
    }

    #[test]
    fn the_strong_fertiliser_matures_a_crop_and_steps_anything_else() {
        let tier = tiered();
        assert_eq!(
            strong_fertiliser(Some(&tier), false, false),
            Fertilised::Matured
        );
        assert_eq!(strong_fertiliser(None, true, false), Fertilised::Stepped);
        assert_eq!(strong_fertiliser(None, false, false), Fertilised::Nothing);
        assert_eq!(
            strong_fertiliser(Some(&tier), true, true),
            Fertilised::Nothing
        );
    }

    #[test]
    fn a_pulse_block_starts_two_above_itself_and_takes_only_the_first() {
        let origin = (0, 64, 0);
        let at_one_above = |position: Position| position.1 == 65;
        assert_eq!(pulse_target(origin, 9, &at_one_above), None);

        let two_and_three = |position: Position| position.1 == 66 || position.1 == 67;
        assert_eq!(pulse_target(origin, 9, &two_and_three), Some((0, 66, 0)));

        let far = |position: Position| position.1 == 75;
        assert_eq!(pulse_target(origin, 9, &far), Some((0, 75, 0)));
        assert_eq!(pulse_target(origin, 9, &|p: Position| p.1 == 76), None);
    }

    #[test]
    fn six_pulse_ranges_ship_and_each_is_nine_more_than_the_last() {
        assert_eq!(PULSE_RANGES.len(), 6);
        for pair in PULSE_RANGES.windows(2) {
            assert_eq!(pair[1] - pair[0], 9);
        }
    }

    #[test]
    fn the_delay_truncates_before_the_multiply_so_one_second_is_nothing_or_twenty() {
        assert_eq!(pulse_delay(1, 0.0), 0, "0.9 seconds truncates to nothing");
        assert_eq!(pulse_delay(1, 1.0), 20, "1.1 seconds truncates to one");
        assert_eq!(pulse_delay(10, 0.5), 200);
        assert_eq!(pulse_delay(10, 0.0), 180);
        assert_eq!(pulse_delay(10, 1.0), 220);
        assert_eq!(pulse_delay(0, 1.0), 0);
    }

    #[test]
    fn the_aura_draws_per_block_over_a_cube_of_eleven_on_a_side() {
        let mut answers = vec![0.9_f64, 0.1, 0.1];
        let mut draw = move |_: Position| answers.pop().unwrap_or(0.9);
        let is_crop =
            |position: Position| position.1 == 64 && position.0.abs() < 2 && position.2 == 0;
        let ticked = aura_positions((0, 64, 0), true, true, &is_crop, &mut draw);
        assert_eq!(ticked.len(), 2, "two of the three draws fell under a half");

        let mut always = |_: Position| 0.0_f64;
        assert_eq!(
            aura_positions((0, 64, 0), true, true, &|_| true, &mut always).len(),
            11 * 11 * 11
        );
        assert_eq!(
            aura_positions((0, 64, 0), false, true, &|_| true, &mut always).len(),
            0
        );
        assert_eq!(
            aura_positions((0, 64, 0), true, false, &|_| true, &mut always).len(),
            0
        );
    }
}
