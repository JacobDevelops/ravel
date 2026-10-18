//! Section twelve's item stores inside items.
//!
//! Two attachments give an item its own item store: a portable container item
//! and a carrying pouch. Neither has a fixed slot count. Both report a slot
//! count taken from the list they currently hold, so an empty container item
//! publishes no slots at all, and both read the whole list out of the item,
//! mutate it, and write it back on every single-slot operation.
//!
//! The pouch's weight arithmetic is one of the four values section 13.3 blocks:
//! it sits behind the access widening of section 13.1 and no Ravel hook exposes
//! it. [`Weight`] is the shape a hook would supply, and every pouch rule that
//! needs it answers absent until one does.

use core::fmt;

use rvl_210_counterpart::quantity::{Amount, QuantityKind};
use rvl_210_counterpart::storage::Attempt;

use crate::kinds::{Ingredient, Stack, Stackable};

/// The per-slot limit both stores impose.
///
/// Section twelve: the pouch's is 64 regardless of the item's own maximum, and
/// the container item's is 64 as well.
pub const PER_SLOT: Amount = 64;

/// The most entries a list-valued item may hold.
///
/// The list is read out of an item, so it is bounded before it is walked.
pub const MAX_ENTRIES: usize = 256;

/// What a list-valued item's store refuses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HoldingsRefused {
    /// The list holds more entries than [`MAX_ENTRIES`].
    TooManyEntries {
        /// How many it holds.
        held: usize,
        /// The published ceiling.
        limit: usize,
    },
}

impl fmt::Display for HoldingsRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyEntries { held, limit } => {
                write!(formatter, "{held} entries is past the {limit} admitted")
            }
        }
    }
}

impl std::error::Error for HoldingsRefused {}

/// The list an item holds, which is the whole of what either store is.
///
/// Every single-slot operation reads the list out, mutates it and writes it
/// back, so the list is the store rather than a cache of it. Both the container
/// item and the pouch run their operations against it, and the two differ only
/// in the ceiling and the validity rule they apply on top.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Listing {
    entries: Vec<Stack>,
}

impl Listing {
    /// An item holding nothing, which publishes no slots at all.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// The list an item currently holds.
    ///
    /// # Errors
    ///
    /// Returns [`HoldingsRefused::TooManyEntries`] past [`MAX_ENTRIES`].
    pub fn of(entries: impl IntoIterator<Item = Stack>) -> Result<Self, HoldingsRefused> {
        let entries: Vec<Stack> = entries.into_iter().collect();
        if entries.len() > MAX_ENTRIES {
            return Err(HoldingsRefused::TooManyEntries {
                held: entries.len(),
                limit: MAX_ENTRIES,
            });
        }
        Ok(Self { entries })
    }

    /// The slot count, taken from the current contents.
    ///
    /// Section twelve: a store's slot count changes as the store is used, so a
    /// dependent that caches the count sees it go stale.
    #[must_use]
    pub fn slots(&self) -> usize {
        self.entries.len()
    }

    /// Whether the item holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// What one slot holds.
    #[must_use]
    pub fn at(&self, slot: usize) -> Option<&Stack> {
        self.entries.get(slot)
    }

    /// Everything held, in list order.
    pub fn entries(&self) -> impl Iterator<Item = &Stack> + '_ {
        self.entries.iter()
    }

    /// Puts a quantity into one slot, answering how much was accepted.
    ///
    /// Section twelve's first rule: insertion refuses a partial merge into an
    /// occupied slot when the items are not the same item with the same data,
    /// returning the whole input rather than a remainder. The slot one past the
    /// end appends, which is how a store with no fixed slot count grows.
    ///
    /// `ceiling` is the per-slot limit in force, which the pouch narrows further
    /// by weight.
    pub fn insert(
        &mut self,
        slot: usize,
        value: &Stack,
        amount: Amount,
        ceiling: Amount,
        attempt: Attempt,
        kind: &Stackable,
    ) -> Amount {
        if amount == 0 || value.is_empty() {
            return 0;
        }
        if let Some(held) = self.entries.get(slot).cloned() {
            if !kind.matches(&held, value, kind.ignoring_quantity_conditions()) {
                // The whole input comes back, not a remainder.
                return 0;
            }
            let headroom = kind.maximum().saturating_sub(held.count);
            let accepted = ceiling.saturating_sub(held.count).min(amount).min(headroom);
            if accepted == 0 {
                return 0;
            }
            if attempt.changes_anything() {
                let Ok(merged) = kind.with_quantity(&held, held.count + accepted) else {
                    return 0;
                };
                self.entries[slot] = merged;
            }
            accepted
        } else {
            if slot != self.entries.len() || self.entries.len() >= MAX_ENTRIES {
                return 0;
            }
            let accepted = ceiling.min(amount).min(kind.maximum());
            if accepted == 0 {
                return 0;
            }
            if attempt.changes_anything() {
                let Ok(placed) = kind.with_quantity(value, accepted) else {
                    return 0;
                };
                self.entries.push(placed);
            }
            accepted
        }
    }

    /// Takes a quantity out of one slot, answering what came out.
    ///
    /// Section twelve's second rule: the existing entry is split in place and
    /// the remainder written back only when not simulating. A simulated
    /// extraction here reads and answers without touching the list at all,
    /// which is the same answer by a route that has nothing to discard.
    pub fn extract(
        &mut self,
        slot: usize,
        amount: Amount,
        attempt: Attempt,
        kind: &Stackable,
    ) -> Option<Stack> {
        let held = self.entries.get(slot).cloned()?;
        let taken = held.count.min(amount);
        if taken == 0 {
            return None;
        }
        let out = kind.with_quantity(&held, taken).ok()?;
        if attempt.changes_anything() {
            let left = held.count - taken;
            if left == 0 {
                self.entries.remove(slot);
            } else {
                // A refused remainder answers nothing extracted rather than
                // leaving the whole entry in place beside a stack this method
                // said came out of it.
                let remainder = kind.with_quantity(&held, left).ok()?;
                self.entries[slot] = remainder;
            }
        }
        Some(out)
    }
}

/// What a pouch's weight arithmetic reads.
///
/// Section 13.3 blocks this: the two widened fields on the pouch and the one on
/// its contents are read through the access widening of section 13.1, which
/// Ravel does not do.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Weight {
    /// How much weight the pouch already carries.
    pub used: Amount,
    /// How much it carries in total.
    pub capacity: Amount,
    /// What one of the item being inserted weighs.
    pub each: Amount,
}

impl Weight {
    /// How many of the item the remaining weight admits.
    #[must_use]
    pub const fn admits(self) -> Amount {
        if self.each == 0 {
            return 0;
        }
        let remaining = self.capacity.saturating_sub(self.used);
        remaining.div_euclid(self.each)
    }
}

/// The carrying pouch's store.
///
/// It differs from the container in three ways: its per-slot limit is 64
/// regardless of the item's own maximum, an item is valid for it only when the
/// remaining weight admits at least one, and its insertion is capped by that
/// weight rather than by a stack size.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Pouch {
    weight: Option<Weight>,
}

impl Pouch {
    /// A pouch whose weight arithmetic no hook supplies.
    #[must_use]
    pub const fn blocked() -> Self {
        Self { weight: None }
    }

    /// A pouch whose weight arithmetic a hook supplied.
    #[must_use]
    pub const fn weighing(weight: Weight) -> Self {
        Self {
            weight: Some(weight),
        }
    }

    /// The weight arithmetic in force, absent where nothing supplies it.
    #[must_use]
    pub const fn weight(self) -> Option<Weight> {
        self.weight
    }

    /// Whether an item is valid for the pouch.
    ///
    /// Answers nothing where the weight arithmetic is blocked, because the rule
    /// is entirely about weight and there is no weight-free reading of it.
    #[must_use]
    pub const fn admits(self) -> Option<bool> {
        match self.weight {
            Some(weight) => Some(weight.admits() >= 1),
            None => None,
        }
    }

    /// The per-slot limit in force, which is the lesser of 64 and what the
    /// remaining weight admits.
    ///
    /// Answers nothing where the weight arithmetic is blocked.
    #[must_use]
    pub const fn ceiling(self) -> Option<Amount> {
        match self.weight {
            Some(weight) => {
                let admitted = weight.admits();
                Some(if admitted < PER_SLOT {
                    admitted
                } else {
                    PER_SLOT
                })
            }
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Components, Value};

    fn kind() -> Stackable {
        Stackable::default()
    }

    #[test]
    fn an_empty_container_item_publishes_no_slots_at_all() {
        let listing = Listing::empty();
        assert_eq!(listing.slots(), 0);
        assert!(listing.is_empty());
        assert_eq!(listing.at(0), None);
    }

    #[test]
    fn the_slot_count_is_taken_from_the_current_contents_and_goes_stale() {
        let mut listing = Listing::empty();
        let cached = listing.slots();
        assert_eq!(cached, 0);
        listing.insert(
            0,
            &Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                1,
            ),
            10,
            PER_SLOT,
            Attempt::Real,
            &kind(),
        );
        assert_eq!(listing.slots(), 1);
        assert_ne!(listing.slots(), cached);
        listing.extract(0, 10, Attempt::Real, &kind());
        assert_eq!(listing.slots(), 0);
    }

    #[test]
    fn a_list_past_the_entry_ceiling_is_refused_before_it_is_walked() {
        let entries = (0..=MAX_ENTRIES).map(|index| {
            Stack::of(
                format!("minecraft:k{index}")
                    .parse()
                    .expect("a valid content identity"),
                1,
            )
        });
        assert_eq!(
            Listing::of(entries),
            Err(HoldingsRefused::TooManyEntries {
                held: MAX_ENTRIES + 1,
                limit: MAX_ENTRIES
            })
        );
        let entries = (0..MAX_ENTRIES).map(|index| {
            Stack::of(
                format!("minecraft:k{index}")
                    .parse()
                    .expect("a valid content identity"),
                1,
            )
        });
        assert_eq!(
            Listing::of(entries).map(|held| held.slots()),
            Ok(MAX_ENTRIES)
        );
    }

    #[test]
    fn an_insertion_past_the_entry_ceiling_appends_nothing() {
        let entries = (0..MAX_ENTRIES).map(|index| {
            Stack::of(
                format!("minecraft:k{index}")
                    .parse()
                    .expect("a valid content identity"),
                1,
            )
        });
        let mut listing = Listing::of(entries).expect("at the ceiling");
        assert_eq!(
            listing.insert(
                MAX_ENTRIES,
                &Stack::of("minecraft:one_too_many".parse().expect("a valid item"), 1),
                1,
                PER_SLOT,
                Attempt::Real,
                &kind()
            ),
            0
        );
        assert_eq!(listing.slots(), MAX_ENTRIES);
    }

    #[test]
    fn an_insertion_into_the_slot_past_the_end_appends() {
        let mut listing = Listing::empty();
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        assert_eq!(
            listing.insert(0, &stone, 10, PER_SLOT, Attempt::Real, &kind()),
            10
        );
        assert_eq!(listing.at(0).map(|held| held.count), Some(10));
        assert_eq!(
            listing.insert(5, &stone, 10, PER_SLOT, Attempt::Real, &kind()),
            0
        );
        assert_eq!(listing.slots(), 1);
    }

    #[test]
    fn a_merge_into_a_matching_slot_is_capped_by_the_per_slot_limit() {
        let mut listing = Listing::of([Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            60,
        )])
        .expect("one entry");
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        assert_eq!(
            listing.insert(0, &stone, 20, PER_SLOT, Attempt::Real, &kind()),
            4
        );
        assert_eq!(listing.at(0).map(|held| held.count), Some(PER_SLOT));
        assert_eq!(
            listing.insert(0, &stone, 20, PER_SLOT, Attempt::Real, &kind()),
            0
        );
    }

    #[test]
    fn the_whole_input_comes_back_when_the_occupied_slot_holds_another_item() {
        let mut listing = Listing::of([Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        )])
        .expect("one entry");
        assert_eq!(
            listing.insert(
                0,
                &Stack::of("minecraft:dirt".parse().expect("a valid item"), 1),
                10,
                PER_SLOT,
                Attempt::Real,
                &kind()
            ),
            0
        );
        assert_eq!(listing.at(0).map(|held| held.count), Some(1));
    }

    #[test]
    fn the_whole_input_comes_back_when_the_data_differs() {
        let sharp = Stack::of(
            "minecraft:axe".parse().expect("a valid content identity"),
            1,
        )
        .carrying(Components::from([("wear".to_owned(), Value::Whole(0))]));
        let blunt = Stack::of(
            "minecraft:axe".parse().expect("a valid content identity"),
            1,
        )
        .carrying(Components::from([("wear".to_owned(), Value::Whole(90))]));
        let mut listing = Listing::of([sharp]).expect("one entry");
        assert_eq!(
            listing.insert(0, &blunt, 1, PER_SLOT, Attempt::Real, &kind()),
            0
        );
    }

    #[test]
    fn a_simulated_insertion_answers_the_same_number_and_writes_nothing_back() {
        let mut listing = Listing::empty();
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        assert_eq!(
            listing.insert(0, &stone, 10, PER_SLOT, Attempt::Simulated, &kind()),
            10
        );
        assert_eq!(listing.slots(), 0);
    }

    #[test]
    fn an_extraction_splits_the_entry_and_writes_the_remainder_back() {
        let mut listing = Listing::of([Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            30,
        )])
        .expect("one entry");
        let out = listing
            .extract(0, 12, Attempt::Real, &kind())
            .expect("something to take");
        assert_eq!(out.count, 12);
        assert_eq!(listing.at(0).map(|held| held.count), Some(18));
    }

    #[test]
    fn an_extraction_that_empties_the_slot_removes_it() {
        let mut listing = Listing::of([Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            30,
        )])
        .expect("one entry");
        assert_eq!(
            listing
                .extract(0, 30, Attempt::Real, &kind())
                .map(|held| held.count),
            Some(30)
        );
        assert_eq!(listing.slots(), 0);
    }

    #[test]
    fn a_simulated_extraction_leaves_the_list_exactly_as_it_was() {
        let mut listing = Listing::of([Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            30,
        )])
        .expect("one entry");
        let before = listing.clone();
        assert_eq!(
            listing
                .extract(0, 12, Attempt::Simulated, &kind())
                .map(|held| held.count),
            Some(12)
        );
        assert_eq!(listing, before);
    }

    #[test]
    fn an_extraction_takes_no_more_than_the_slot_holds() {
        let mut listing = Listing::of([Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            5,
        )])
        .expect("one entry");
        assert_eq!(
            listing
                .extract(0, 50, Attempt::Real, &kind())
                .map(|held| held.count),
            Some(5)
        );
        assert_eq!(listing.slots(), 0);
    }

    #[test]
    fn an_extraction_from_a_slot_that_is_not_there_takes_nothing() {
        let mut listing = Listing::empty();
        assert_eq!(listing.extract(0, 5, Attempt::Real, &kind()), None);
        let mut listing = Listing::of([Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            5,
        )])
        .expect("one entry");
        assert_eq!(listing.extract(0, 0, Attempt::Real, &kind()), None);
    }

    #[test]
    fn the_whole_list_reads_back_in_list_order() {
        let listing = Listing::of([
            Stack::of(
                "minecraft:stone".parse().expect("a valid content identity"),
                5,
            ),
            Stack::of(
                "minecraft:dirt".parse().expect("a valid content identity"),
                7,
            ),
        ])
        .expect("two entries");
        let held: Vec<(String, Amount)> = listing
            .entries()
            .map(|entry| (entry.identity.to_string(), entry.count))
            .collect();
        assert_eq!(
            held,
            vec![
                ("minecraft:stone".to_owned(), 5),
                ("minecraft:dirt".to_owned(), 7)
            ]
        );
        assert_eq!(Listing::empty().entries().count(), 0);
    }

    #[test]
    fn a_pouch_whose_weight_no_hook_supplies_answers_nothing_rather_than_guessing() {
        let pouch = Pouch::blocked();
        assert_eq!(pouch.weight(), None);
        assert_eq!(pouch.admits(), None);
        assert_eq!(pouch.ceiling(), None);
    }

    #[test]
    fn a_pouch_admits_an_item_only_when_the_remaining_weight_takes_at_least_one() {
        let room = Pouch::weighing(Weight {
            used: 0,
            capacity: 64,
            each: 8,
        });
        assert_eq!(room.admits(), Some(true));
        let full = Pouch::weighing(Weight {
            used: 64,
            capacity: 64,
            each: 8,
        });
        assert_eq!(full.admits(), Some(false));
        let nearly = Pouch::weighing(Weight {
            used: 60,
            capacity: 64,
            each: 8,
        });
        assert_eq!(nearly.admits(), Some(false));
    }

    #[test]
    fn a_pouchs_insertion_is_capped_by_weight_rather_than_by_a_stack_size() {
        let pouch = Pouch::weighing(Weight {
            used: 0,
            capacity: 40,
            each: 8,
        });
        assert_eq!(pouch.ceiling(), Some(5));
        let mut listing = Listing::empty();
        let stone = Stack::of(
            "minecraft:stone".parse().expect("a valid content identity"),
            1,
        );
        let ceiling = pouch.ceiling().expect("a supplied weight");
        assert_eq!(
            listing.insert(0, &stone, 64, ceiling, Attempt::Real, &kind()),
            5
        );
    }

    #[test]
    fn a_pouch_with_room_to_spare_is_still_capped_at_sixty_four() {
        let pouch = Pouch::weighing(Weight {
            used: 0,
            capacity: 10_000,
            each: 1,
        });
        assert_eq!(pouch.ceiling(), Some(PER_SLOT));
    }

    #[test]
    fn a_weightless_item_is_admitted_by_nothing_rather_than_by_everything() {
        let pouch = Pouch::weighing(Weight {
            used: 0,
            capacity: 64,
            each: 0,
        });
        assert_eq!(pouch.admits(), Some(false));
        assert_eq!(pouch.ceiling(), Some(0));
    }

    #[test]
    fn refusals_say_what_was_wrong() {
        assert!(
            HoldingsRefused::TooManyEntries { held: 9, limit: 8 }
                .to_string()
                .contains("past the 8")
        );
    }
}
