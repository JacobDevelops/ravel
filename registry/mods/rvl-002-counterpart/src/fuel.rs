//! Fuel: twelve burn times, six items and six blocks, from one base.
//!
//! They are not a table. Both are derivations from a single constant, and the
//! factor of nine between an item and its block is the crafting ratio rather
//! than a bonus: a fuel block is crafted from nine fuel items and burns for
//! exactly as long, so compressing fuel is storage and never efficiency.

/// The lowest tier's item burn time, in ticks.
pub const BASE_TICKS: u64 = 2400;

/// How many items a fuel block is crafted from, and the factor its burn time
/// carries.
pub const COMPRESSION: u64 = 9;

/// The tiers this entry derives a pair for.
pub const TIERS: u32 = 6;

/// How long one fuel item of tier `tier` burns, in ticks.
///
/// A tier outside 1 to 6 is not a tier this entry declares a fuel for, and is
/// refused rather than extrapolated: the shift below would overflow long before
/// a caller noticed, and a burn time computed for a tier nothing registers is
/// not a value any consumer should be handed.
#[must_use]
pub fn item_ticks(tier: u32) -> Option<u64> {
    if tier == 0 || tier > TIERS {
        return None;
    }
    BASE_TICKS.checked_mul(1u64.checked_shl(tier - 1)?)
}

/// How long one fuel block of tier `tier` burns, in ticks.
#[must_use]
pub fn block_ticks(tier: u32) -> Option<u64> {
    item_ticks(tier)?.checked_mul(COMPRESSION)
}

/// Every item burn time, lowest tier first.
#[must_use]
pub fn item_series() -> Vec<u64> {
    (1..=TIERS).filter_map(item_ticks).collect()
}

/// Every block burn time, lowest tier first.
#[must_use]
pub fn block_series() -> Vec<u64> {
    (1..=TIERS).filter_map(block_ticks).collect()
}

/// The burn time an entry answers with, whatever is asking.
///
/// The specification records that the value comes from a fixed field and does
/// not vary by consumer or by recipe kind, so this takes no consumer at all.
/// That absence is the rule.
#[must_use]
pub fn burn_time(tier: u32, compressed: bool) -> Option<u64> {
    if compressed {
        block_ticks(tier)
    } else {
        item_ticks(tier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DECLARED_ITEMS: [u64; 6] = [2400, 4800, 9600, 19_200, 38_400, 76_800];
    const DECLARED_BLOCKS: [u64; 6] = [21_600, 43_200, 86_400, 172_800, 345_600, 691_200];

    #[test]
    fn all_twelve_declared_constants_come_out_of_the_derivation_exactly() {
        assert_eq!(item_series(), DECLARED_ITEMS);
        assert_eq!(block_series(), DECLARED_BLOCKS);
    }

    #[test]
    fn each_tier_doubles_the_one_below_it() {
        for tier in 2..=TIERS {
            let below = item_ticks(tier - 1).expect("a declared tier");
            assert_eq!(item_ticks(tier), Some(below * 2));
        }
    }

    #[test]
    fn a_block_burns_for_exactly_what_the_nine_items_it_holds_would() {
        for tier in 1..=TIERS {
            let item = item_ticks(tier).expect("a declared tier");
            assert_eq!(
                block_ticks(tier),
                Some(item * COMPRESSION),
                "compressing fuel is storage, not efficiency"
            );
        }
    }

    #[test]
    fn a_tier_this_entry_declares_no_fuel_for_is_refused_rather_than_extrapolated() {
        assert_eq!(item_ticks(0), None);
        assert_eq!(item_ticks(TIERS + 1), None);
        assert_eq!(item_ticks(u32::MAX), None);
        assert_eq!(block_ticks(0), None);
        assert_eq!(block_ticks(TIERS + 1), None);
        assert_eq!(block_ticks(u32::MAX), None);
    }

    #[test]
    fn the_answer_does_not_vary_by_what_is_asking() {
        for tier in 1..=TIERS {
            assert_eq!(burn_time(tier, false), item_ticks(tier));
            assert_eq!(burn_time(tier, true), block_ticks(tier));
        }
    }
}
