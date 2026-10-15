//! An entry declared reusable with a number of uses.
//!
//! A reusable entry always declares a crafting remainder, so crafting returns it
//! rather than consuming it. Whether a use is charged at all is a random draw,
//! and the draw is taken from a caller-supplied source rather than a global one
//! so the rule can be tested.

use crate::inventory::Stack;

/// What a player is shown about how much is left.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsesLeft {
    /// Fewer than one declared use means the entry is never spent.
    Unlimited,
    /// Exactly one use remains, which reads differently to a player.
    Last,
    /// This many uses remain.
    Remaining(u32),
}

/// The declared reusability of one entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Reusable {
    uses: u32,
}

impl Reusable {
    /// Declares an entry reusable a number of times; fewer than one is unlimited.
    #[must_use]
    pub const fn new(uses: u32) -> Self {
        Self { uses }
    }

    /// Returns whether this entry is never spent.
    #[must_use]
    pub const fn unlimited(&self) -> bool {
        self.uses < 1
    }

    /// Returns the durability, one less than the declared uses.
    ///
    /// The offset is load-bearing: a fresh entry sits one below its use count,
    /// so the figure shown to a player and the number of crafts actually
    /// available agree.
    #[must_use]
    pub const fn durability(&self) -> Option<u32> {
        if self.unlimited() {
            None
        } else {
            Some(self.uses - 1)
        }
    }

    /// Returns whether the entry may be repaired, which a reusable one may not.
    #[must_use]
    pub const fn repairable(&self) -> bool {
        false
    }

    /// Returns what a player is shown for a given damage.
    ///
    /// §10 specifies the display for reachable damage only, and damage above the
    /// maximum is unreachable: an entry is spent and gone at the maximum, so
    /// nothing holds one to answer for. Damage beyond it saturates and reads as
    /// the last use rather than inventing a spent case the specification does
    /// not describe.
    #[must_use]
    pub const fn uses_left(&self, damage: u32) -> UsesLeft {
        match self.durability() {
            None => UsesLeft::Unlimited,
            Some(maximum) => {
                let left = maximum.saturating_sub(damage) + 1;
                if left == 1 {
                    UsesLeft::Last
                } else {
                    UsesLeft::Remaining(left)
                }
            }
        }
    }

    /// Returns the probability that one use is actually charged.
    ///
    /// The use is skipped with probability one minus the reciprocal of one plus
    /// the durability-preserving enchantment level, so it is charged with that
    /// reciprocal.
    #[must_use]
    pub fn charge_probability(&self, preserving_level: u32) -> f64 {
        1.0 / f64::from(preserving_level + 1)
    }

    /// Returns the single copy crafting gives back, or nothing if it was spent.
    ///
    /// `draw` supplies a value in `[0, 1)`. Unlimited, the copy comes back
    /// unchanged. Otherwise the use is charged when the draw falls below
    /// [`Self::charge_probability`], damage rises by one, and exceeding the
    /// maximum consumes the entry.
    pub fn remainder(
        &self,
        stack: &Stack,
        damage: u32,
        preserving_level: u32,
        draw: &mut dyn FnMut() -> f64,
    ) -> Option<(Stack, u32)> {
        let mut copy = stack.clone();
        copy.count = 1;
        let Some(maximum) = self.durability() else {
            return Some((copy, damage));
        };
        if draw() >= self.charge_probability(preserving_level) {
            return Some((copy, damage));
        }
        let charged = damage + 1;
        if charged > maximum {
            return None;
        }
        Some((copy, charged))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draws(values: Vec<f64>) -> impl FnMut() -> f64 {
        let mut values = values.into_iter();
        move || values.next().unwrap_or(1.0)
    }

    #[test]
    fn fewer_than_one_declared_use_is_unlimited_and_has_no_durability() {
        let entry = Reusable::new(0);
        assert!(entry.unlimited());
        assert_eq!(entry.durability(), None);
        assert_eq!(entry.uses_left(0), UsesLeft::Unlimited);
    }

    #[test]
    fn durability_is_one_below_the_declared_uses_and_the_shown_figure_adds_it_back() {
        let entry = Reusable::new(4);
        assert_eq!(entry.durability(), Some(3));
        assert_eq!(entry.uses_left(0), UsesLeft::Remaining(4));
        assert_eq!(entry.uses_left(2), UsesLeft::Remaining(2));
        assert_eq!(entry.uses_left(3), UsesLeft::Last);
    }

    #[test]
    fn a_reusable_entry_is_never_repairable() {
        assert!(!Reusable::new(4).repairable());
    }

    #[test]
    fn an_unlimited_entry_comes_back_unchanged_whatever_the_draw() {
        let entry = Reusable::new(0);
        let stack = Stack::new("tool", 1);
        let mut draw = draws(vec![0.0]);
        assert_eq!(
            entry.remainder(&stack, 7, 0, &mut draw),
            Some((Stack::new("tool", 1), 7))
        );
    }

    #[test]
    fn the_charge_probability_is_the_reciprocal_of_one_plus_the_level() {
        let entry = Reusable::new(4);
        assert!((entry.charge_probability(0) - 1.0).abs() < f64::EPSILON);
        assert!((entry.charge_probability(1) - 0.5).abs() < f64::EPSILON);
        assert!((entry.charge_probability(3) - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn a_draw_at_or_above_the_probability_skips_the_charge() {
        let entry = Reusable::new(4);
        let stack = Stack::new("tool", 1);
        let mut skipped = draws(vec![0.5]);
        assert_eq!(
            entry.remainder(&stack, 0, 1, &mut skipped),
            Some((Stack::new("tool", 1), 0)),
            "a draw equal to the probability is not below it"
        );
        let mut charged = draws(vec![0.49]);
        assert_eq!(
            entry.remainder(&stack, 0, 1, &mut charged),
            Some((Stack::new("tool", 1), 1))
        );
    }

    #[test]
    fn the_remainder_is_a_single_copy_however_many_were_held() {
        let entry = Reusable::new(4);
        let mut stack = Stack::new("tool", 9);
        stack.data = ravel_sdk::json!({ "charge": 2 });
        let mut draw = draws(vec![1.0]);
        let (copy, _) = entry
            .remainder(&stack, 0, 0, &mut draw)
            .expect("an unspent entry comes back");
        assert_eq!(copy.count, 1);
        assert_eq!(copy.data, stack.data);
    }

    #[test]
    fn exceeding_the_maximum_consumes_the_entry() {
        let entry = Reusable::new(2);
        let stack = Stack::new("tool", 1);
        let mut draw = draws(vec![0.0]);
        assert_eq!(
            entry.remainder(&stack, 0, 0, &mut draw),
            Some((stack.clone(), 1))
        );
        let mut spend = draws(vec![0.0]);
        assert_eq!(entry.remainder(&stack, 1, 0, &mut spend), None);
    }
}
