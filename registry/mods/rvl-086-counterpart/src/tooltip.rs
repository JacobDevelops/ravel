//! The portable half of section 14.1's two informational tooltips.
//!
//! Inserting a line into an assembled tooltip and recolouring one that is
//! already there are both host-blocked: Ravel publishes no tooltip-assembly
//! surface. What is portable is what the two lines would say — the speed a
//! digging tool is drawn as mining at, and the count of ammunition a ranged
//! weapon can reach — and that is what is here.

use ravel_sdk::content::ContentId;
use rvl_042_counterpart::inventory::Stack;

use crate::repair::Shape;
use crate::settings::Settings;

/// Whether a digging tool gets a mining-speed line at all.
///
/// The line is drawn on a digging tool that the exclusion list does not name,
/// and only when the tooltips are switched on. The exclusion list is one of the
/// three settings that share the matching language.
#[must_use]
pub fn shows_mining_speed(
    settings: &Settings,
    entry: &str,
    shape: Shape,
    member_of: &dyn Fn(&str) -> bool,
) -> bool {
    settings.tooltip_enabled
        && shape == Shape::Digging
        && !settings.tooltip_excluded.matches(entry, member_of)
}

/// The speed the mining-speed line reads.
///
/// Nought when the ineffective rule has ruined the tool, and the tier's own
/// speed otherwise, so the line agrees with what the tool actually does.
#[must_use]
pub fn mining_speed(ruined: bool, tier_speed: f64) -> f64 {
    if ruined { 0.0 } else { tier_speed }
}

/// What the ranged weapon's line says.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ammunition {
    /// A count of usable ammunition across the reachable slots.
    Counted(u64),
    /// The weapon draws from nothing, so the existing enchantment line is
    /// recoloured instead. Whether ammunition is present decides the colour.
    Endless {
        /// Whether any usable ammunition is present.
        present: bool,
    },
}

/// The most stacks this count will walk.
///
/// The inventory is handed over by the host rather than written by a player, but
/// the count is a sum over it and a sum over an unbounded list is unbounded.
pub const MAXIMUM_STACKS: usize = 4096;

/// Counts the ammunition a weapon can reach, off-hand slot first.
///
/// The sum is over every stack the weapon accepts as a held projectile. Counts
/// saturate rather than wrapping, so a store holding more than the count can
/// hold reports the ceiling instead of a small number.
#[must_use]
pub fn usable(
    offhand: Option<&Stack>,
    inventory: &[Stack],
    accepts: &dyn Fn(&ContentId) -> bool,
) -> u64 {
    offhand
        .into_iter()
        .chain(inventory.iter().take(MAXIMUM_STACKS))
        .filter(|stack| !stack.is_empty() && accepts(&stack.entry))
        .fold(0_u64, |total, stack| total.saturating_add(stack.count))
}

/// What the ranged weapon's line reads.
#[must_use]
pub fn ammunition(
    endless: bool,
    offhand: Option<&Stack>,
    inventory: &[Stack],
    accepts: &dyn Fn(&ContentId) -> bool,
) -> Ammunition {
    let counted = usable(offhand, inventory, accepts);
    if endless {
        Ammunition::Endless {
            present: counted > 0,
        }
    } else {
        Ammunition::Counted(counted)
    }
}

/// Whether a ranged weapon gets its line at all.
#[must_use]
pub fn shows_ammunition(settings: &Settings, shape: Shape) -> bool {
    settings.tooltip_enabled && shape == Shape::Ranged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(entry: &str) -> ContentId {
        entry.parse().expect("a valid test identity")
    }
    use crate::matching::Matcher;

    fn accepts_arrows(entry: &ContentId) -> bool {
        entry.namespace().as_str() == "example" && entry.path() == "arrow"
    }

    fn untagged(_tag: &str) -> bool {
        false
    }

    #[test]
    fn the_mining_speed_line_is_drawn_on_a_digging_tool_the_list_does_not_name() {
        let settings = Settings::default();
        assert!(shows_mining_speed(
            &settings,
            "example:pick",
            Shape::Digging,
            &untagged
        ));
        assert!(!shows_mining_speed(
            &settings,
            "example:bow",
            Shape::Ranged,
            &untagged
        ));
    }

    #[test]
    fn an_excluded_tool_and_a_switched_off_tooltip_both_draw_nothing() {
        let excluded = Settings {
            tooltip_excluded: Matcher::parse("example:*"),
            ..Settings::default()
        };
        assert!(!shows_mining_speed(
            &excluded,
            "example:pick",
            Shape::Digging,
            &untagged
        ));

        let off = Settings {
            tooltip_enabled: false,
            ..Settings::default()
        };
        assert!(!shows_mining_speed(
            &off,
            "example:pick",
            Shape::Digging,
            &untagged
        ));
        assert!(!shows_ammunition(&off, Shape::Ranged));
    }

    #[test]
    fn a_ruined_tool_reads_as_mining_at_nothing() {
        assert!((mining_speed(true, 12.0) - 0.0).abs() < f64::EPSILON);
        assert!((mining_speed(false, 12.0) - 12.0).abs() < f64::EPSILON);
    }

    #[test]
    fn the_count_takes_the_off_hand_slot_first_and_then_the_inventory() {
        let offhand = Stack::new(id("example:arrow"), 5);
        let inventory = vec![
            Stack::new(id("example:arrow"), 64),
            Stack::new(id("example:stone"), 64),
            Stack::new(id("example:arrow"), 1),
        ];
        assert_eq!(usable(Some(&offhand), &inventory, &accepts_arrows), 70);
        assert_eq!(usable(None, &inventory, &accepts_arrows), 65);
    }

    #[test]
    fn a_stack_holding_nothing_and_a_stack_the_weapon_refuses_are_both_skipped() {
        let inventory = vec![
            Stack::new(id("example:arrow"), 0),
            Stack::new(id("example:stone"), 64),
        ];
        assert_eq!(usable(None, &inventory, &accepts_arrows), 0);
    }

    #[test]
    fn a_count_at_the_top_of_its_range_saturates_rather_than_wrapping() {
        let inventory = vec![
            Stack::new(id("example:arrow"), u64::MAX),
            Stack::new(id("example:arrow"), 10),
        ];
        assert_eq!(usable(None, &inventory, &accepts_arrows), u64::MAX);
    }

    #[test]
    fn a_weapon_that_draws_from_nothing_reports_presence_rather_than_a_count() {
        let inventory = vec![Stack::new(id("example:arrow"), 3)];
        assert_eq!(
            ammunition(true, None, &inventory, &accepts_arrows),
            Ammunition::Endless { present: true }
        );
        assert_eq!(
            ammunition(true, None, &[], &accepts_arrows),
            Ammunition::Endless { present: false }
        );
        assert_eq!(
            ammunition(false, None, &inventory, &accepts_arrows),
            Ammunition::Counted(3)
        );
    }

    #[test]
    fn an_inventory_longer_than_the_ceiling_is_counted_only_as_far_as_the_ceiling() {
        let inventory = vec![Stack::new(id("example:arrow"), 1); MAXIMUM_STACKS + 10];
        assert_eq!(
            usable(None, &inventory, &accepts_arrows),
            u64::try_from(MAXIMUM_STACKS).expect("the stack ceiling fits in u64")
        );
    }
}
