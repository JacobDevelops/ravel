//! The tiered ground the crops stand on.
//!
//! Five ground blocks, one per numbered tier, each extending the host's tilled
//! ground and each carrying its tier. Four behaviours are stated exactly here:
//! the moisture transition, what falling on it costs, what breaking it drops,
//! and what converting it does.

use rvl_042_counterpart::{Position, Stack};

use crate::model::Tier;

/// The highest moisture the ground holds.
pub const MAXIMUM_MOISTURE: u8 = 7;

/// The fall damage multiplier this ground applies.
///
/// Full damage. The host's own tilled ground turns back into dirt when it is
/// jumped on and this ground does not: it takes the damage path instead and
/// survives, which is the difference a player notices.
pub const FALL_MULTIPLIER: f64 = 1.0;

/// The percentage chance breaking the ground drops the tier's material.
pub const MATERIAL_DROP_PERCENT: u32 = 25;

/// The horizontal reach of the water test, in blocks.
pub const WATER_REACH: i32 = 4;

/// Whether moisture makes this ground fertile.
///
/// Strictly greater than zero, so a value of zero is not fertile.
#[must_use]
pub const fn fertile(moisture: u8) -> bool {
    moisture > 0
}

/// Whether tilled ground counts as near water.
///
/// The original reaches a widened host member for this. It is reimplemented
/// from the positions around the point: any water within four blocks
/// horizontally, at the ground's own height or one above.
pub fn near_water(origin: Position, is_water: &dyn Fn(Position) -> bool) -> bool {
    for x in -WATER_REACH..=WATER_REACH {
        for z in -WATER_REACH..=WATER_REACH {
            for y in 0..=1 {
                if is_water((origin.0 + x, origin.1 + y, origin.2 + z)) {
                    return true;
                }
            }
        }
    }
    false
}

/// The moisture this ground holds after one random tick.
///
/// Drying is gradual and wetting is instantaneous. Ground that is neither near
/// water nor under rain loses exactly one; anything else is set straight to the
/// maximum rather than incremented, so ground becomes fertile one tick after
/// rain rather than seven.
#[must_use]
pub const fn next_moisture(moisture: u8, near_water: bool, raining: bool) -> u8 {
    if !near_water && !raining {
        if moisture > 0 {
            return moisture - 1;
        }
        return moisture;
    }
    if moisture < MAXIMUM_MOISTURE {
        return MAXIMUM_MOISTURE;
    }
    moisture
}

/// What breaking this ground drops.
///
/// A silk-touch break drops the ground itself. Anything else drops one dirt,
/// plus one of the tier's own material on a draw under the percentage. The tool
/// is read from the drop request's optional tool, so a break with no recorded
/// tool takes the second branch.
#[must_use]
pub fn drops(tier: &Tier, silk_touch: bool, dirt: &str, draw: u32) -> Vec<Stack> {
    let Some(ground) = tier.ground.as_deref() else {
        return Vec::new();
    };
    if silk_touch {
        return vec![Stack::new(ground, 1)];
    }
    let mut dropped = vec![Stack::new(dirt, 1)];
    if draw % 100 < MATERIAL_DROP_PERCENT
        && let Some(material) = tier.material.as_deref()
    {
        dropped.push(Stack::new(material, 1));
    }
    dropped
}

/// What right-clicking ground with a tier material did.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Converted {
    /// The ground block now in place, when the conversion happened.
    pub ground: Option<String>,
    /// The moisture the new ground holds.
    pub moisture: u8,
    /// How much of the held material was consumed.
    pub consumed: u32,
    /// The refund popped on top, when a tiered ground was replaced.
    pub refund: Option<Stack>,
}

impl Converted {
    /// Whether the interaction passed straight through.
    #[must_use]
    pub const fn passed_through(&self) -> bool {
        self.ground.is_none()
    }
}

/// Converts ground with a held tier material.
///
/// Plain tilled ground becomes that tier's ground carrying its moisture across
/// unchanged, which is what stops a player upgrading under a mature crop from
/// losing growth progress. Tiered ground standing on a different block does the
/// same and additionally refunds one of the outgoing tier's material on a draw
/// under the percentage. Ground already standing on the block the held material
/// resolves to, and anything else, is left alone: two tiers sharing one ground
/// block are the same ground, so converting between them would consume a
/// material to replace a block with itself.
#[must_use]
pub fn convert(
    current: Option<&Tier>,
    plain_tilled: bool,
    held: &Tier,
    moisture: u8,
    enabled: bool,
    draw: u32,
) -> Converted {
    if !enabled {
        return Converted::default();
    }
    let Some(ground) = held.ground.as_deref() else {
        return Converted::default();
    };
    match current {
        None if plain_tilled => Converted {
            ground: Some(ground.to_owned()),
            moisture,
            consumed: 1,
            refund: None,
        },
        Some(outgoing) if outgoing.ground.as_deref() != Some(ground) => {
            let refund = (draw % 100 < MATERIAL_DROP_PERCENT)
                .then(|| outgoing.material.as_deref().map(|item| Stack::new(item, 1)))
                .flatten();
            Converted {
                ground: Some(ground.to_owned()),
                moisture,
                consumed: 1,
                refund,
            }
        }
        _ => Converted::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tier(value: u32) -> Tier {
        let mut tier = Tier::new(&format!("test:tier_{value}"), value);
        tier.adjust(
            &format!("test:ground_{value}"),
            &format!("test:material_{value}"),
        );
        tier
    }

    #[test]
    fn moisture_of_zero_is_not_fertile_and_anything_above_it_is() {
        assert!(!fertile(0));
        assert!(fertile(1));
        assert!(fertile(MAXIMUM_MOISTURE));
    }

    #[test]
    fn drying_takes_one_step_and_wetting_goes_straight_to_the_maximum() {
        assert_eq!(next_moisture(7, false, false), 6);
        assert_eq!(next_moisture(1, false, false), 0);
        assert_eq!(next_moisture(0, false, false), 0);
        assert_eq!(next_moisture(0, true, false), MAXIMUM_MOISTURE);
        assert_eq!(next_moisture(0, false, true), MAXIMUM_MOISTURE);
        assert_eq!(next_moisture(3, true, false), MAXIMUM_MOISTURE);
        assert_eq!(next_moisture(7, true, true), 7);
    }

    #[test]
    fn the_water_test_reaches_four_horizontally_and_one_above() {
        let origin = (0, 64, 0);
        assert!(near_water(origin, &|position| position == (4, 64, 0)));
        assert!(near_water(origin, &|position| position == (0, 65, -4)));
        assert!(!near_water(origin, &|position| position == (5, 64, 0)));
        assert!(!near_water(origin, &|position| position == (0, 66, 0)));
        assert!(!near_water(origin, &|position| position == (0, 63, 0)));
        assert!(!near_water(origin, &|_| false));
    }

    #[test]
    fn a_silk_touch_break_drops_the_ground_and_anything_else_drops_dirt() {
        let tier = tier(2);
        assert_eq!(
            drops(&tier, true, "test:dirt", 0),
            [Stack::new("test:ground_2", 1)]
        );
        assert_eq!(
            drops(&tier, false, "test:dirt", 24),
            [Stack::new("test:dirt", 1), Stack::new("test:material_2", 1)]
        );
        assert_eq!(
            drops(&tier, false, "test:dirt", 25),
            [Stack::new("test:dirt", 1)],
            "the draw is strictly under twenty-five out of a hundred"
        );
        assert_eq!(
            drops(&tier, false, "test:dirt", 125),
            drops(&tier, false, "test:dirt", 25)
        );
    }

    #[test]
    fn plain_tilled_ground_converts_and_carries_its_moisture_across_unchanged() {
        let held = tier(3);
        let answer = convert(None, true, &held, 5, true, 99);
        assert_eq!(answer.ground.as_deref(), Some("test:ground_3"));
        assert_eq!(answer.moisture, 5, "a converted plot does not reset to dry");
        assert_eq!(answer.consumed, 1);
        assert!(answer.refund.is_none());
    }

    #[test]
    fn two_tiers_standing_on_one_ground_block_do_not_convert_into_each_other() {
        let mut side = Tier::new("test:tier_side", 1);
        side.adjust("test:ground_1", "test:material_side");
        let held = tier(1);
        assert_ne!(side.id, held.id);
        assert!(
            convert(Some(&side), false, &held, 7, true, 0).passed_through(),
            "the block is already the one the held material resolves to"
        );
    }

    #[test]
    fn converting_a_different_tier_refunds_the_outgoing_material_a_quarter_of_the_time() {
        let outgoing = tier(1);
        let held = tier(4);
        let refunded = convert(Some(&outgoing), false, &held, 7, true, 24);
        assert_eq!(refunded.refund, Some(Stack::new("test:material_1", 1)));
        assert_eq!(refunded.consumed, 1);
        assert_eq!(refunded.moisture, 7);

        let bare = convert(Some(&outgoing), false, &held, 7, true, 25);
        assert!(bare.refund.is_none());
        assert_eq!(bare.ground.as_deref(), Some("test:ground_4"));
    }

    #[test]
    fn the_same_tier_and_anything_that_is_not_ground_pass_straight_through() {
        let held = tier(2);
        assert!(convert(Some(&tier(2)), false, &held, 4, true, 0).passed_through());
        assert!(convert(None, false, &held, 4, true, 0).passed_through());
    }

    #[test]
    fn the_setting_switches_the_whole_interaction_off() {
        let held = tier(2);
        assert!(convert(None, true, &held, 4, false, 0).passed_through());
    }

    #[test]
    fn falling_on_this_ground_takes_full_damage_rather_than_breaking_it() {
        assert!((FALL_MULTIPLIER - 1.0).abs() < f64::EPSILON);
    }
}
