//! Section ten's storage interfaces, the move helper and the difference tracker.
//!
//! The move is the part of section ten with a rule a dependent can be broken by,
//! and it is stated there in one sentence: simulate first, let the simulation
//! decide the real amount, and when the real insertion accepts less than the
//! simulation promised put the shortfall back into the source and return a
//! reduced result. The dedicated inconsistency is raised only when that recovery
//! also fails, so a partial move completes rather than raising.

use core::fmt;

use crate::quantity::{Amount, Conditions, Held, QuantityKind};

/// Whether a call changes anything or only reports what it would do.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Attempt {
    /// Report what would happen and change nothing.
    Simulated,
    /// Do it.
    Real,
}

impl Attempt {
    /// Whether this attempt changes the store.
    #[must_use]
    pub const fn changes_anything(self) -> bool {
        matches!(self, Self::Real)
    }
}

/// A place a quantity can be put and taken from.
pub trait Store {
    /// The kind this store holds.
    type Kind: QuantityKind;

    /// The kind, so a caller can match against what is held.
    fn kind(&self) -> &Self::Kind;

    /// Puts a quantity in and answers how much was accepted.
    ///
    /// A simulated insertion answers the same number a real one would and
    /// changes nothing.
    fn insert(
        &mut self,
        value: &<Self::Kind as QuantityKind>::Instance,
        amount: Amount,
        attempt: Attempt,
    ) -> Amount;

    /// Takes a quantity out and answers how much was taken.
    fn extract(
        &mut self,
        value: &<Self::Kind as QuantityKind>::Instance,
        amount: Amount,
        attempt: Attempt,
    ) -> Amount;
}

/// A store whose contents sit in numbered slots.
///
/// The one guarantee the slotted form adds is in the result: a caller is told
/// which slot a result came from, which the slotless form cannot answer.
pub trait SlottedStore: Store {
    /// How many slots there are.
    fn slots(&self) -> usize;

    /// Puts a quantity in and answers how much went where.
    fn insert_slotted(
        &mut self,
        value: &<Self::Kind as QuantityKind>::Instance,
        amount: Amount,
        attempt: Attempt,
    ) -> Slotted;
}

/// How much moved and which slot it moved through.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Slotted {
    /// How much was accepted or taken.
    pub amount: Amount,
    /// The slot it went through, absent when nothing moved.
    pub slot: Option<usize>,
}

impl Slotted {
    /// Nothing moved and no slot is named.
    pub const NOTHING: Self = Self {
        amount: 0,
        slot: None,
    };

    /// A result naming its slot.
    #[must_use]
    pub const fn at(slot: usize, amount: Amount) -> Self {
        if amount == 0 {
            return Self::NOTHING;
        }
        Self {
            amount,
            slot: Some(slot),
        }
    }
}

/// The dedicated error a move raises when its recovery also fails.
///
/// It carries the shortfall the source would not take back, because a caller
/// that sees this has lost exactly that much and the number is the only thing
/// that lets it say so.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Inconsistent {
    /// How much the simulation promised the destination would accept.
    pub promised: Amount,
    /// How much the destination really accepted.
    pub accepted: Amount,
    /// How much the source would not take back.
    pub stranded: Amount,
}

impl fmt::Display for Inconsistent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a move promised {} and placed {}, and {} could not be put back",
            self.promised, self.accepted, self.stranded
        )
    }
}

impl std::error::Error for Inconsistent {}

/// Moves a quantity from one store to another.
///
/// The order is the specification's: simulate the extraction to learn what is
/// there, simulate the insertion to learn what would be taken, extract that much
/// for real and insert it for real. A real insertion that accepts less than the
/// simulation promised is not by itself a failure; the shortfall goes back into
/// the source and the reduced amount is returned.
///
/// # Errors
///
/// Returns [`Inconsistent`] only when the destination accepted less than it
/// promised **and** the source would not take the shortfall back.
pub fn move_quantity<K, From, To>(
    from: &mut From,
    to: &mut To,
    value: &K::Instance,
    amount: Amount,
) -> Result<Amount, Inconsistent>
where
    K: QuantityKind,
    From: Store<Kind = K>,
    To: Store<Kind = K>,
{
    let available = from.extract(value, amount, Attempt::Simulated);
    if available == 0 {
        return Ok(0);
    }
    let promised = to.insert(value, available, Attempt::Simulated);
    if promised == 0 {
        return Ok(0);
    }
    let taken = from.extract(value, promised, Attempt::Real);
    if taken == 0 {
        return Ok(0);
    }
    let accepted = to.insert(value, taken, Attempt::Real);
    if accepted >= taken {
        return Ok(taken);
    }
    let shortfall = taken - accepted;
    let returned = from.insert(value, shortfall, Attempt::Real);
    if returned == shortfall {
        return Ok(accepted);
    }
    Err(Inconsistent {
        promised,
        accepted,
        stranded: shortfall - returned,
    })
}

/// What changed between two snapshots of a collection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Difference<I> {
    /// What the second snapshot holds and the first did not, or holds more of.
    pub added: Vec<Held<I>>,
    /// What the first snapshot held and the second does not, or holds less of.
    pub removed: Vec<Held<I>>,
}

impl<I> Default for Difference<I> {
    fn default() -> Self {
        Self {
            added: Vec::new(),
            removed: Vec::new(),
        }
    }
}

impl<I> Difference<I> {
    /// Whether the two snapshots agree.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }
}

/// Compares two snapshots and yields what was added and what was removed.
///
/// Both snapshots are read as collapsed under `under`, so an instance held in
/// two entries before and one after is not reported as a change. This is what
/// lets a store's contents be sent to a client incrementally.
pub fn difference<K>(
    kind: &K,
    before: &[Held<K::Instance>],
    after: &[Held<K::Instance>],
    under: Conditions,
) -> Difference<K::Instance>
where
    K: QuantityKind,
    K::Instance: Clone,
{
    let fold = |entries: &[Held<K::Instance>]| {
        let mut folded: Vec<Held<K::Instance>> = Vec::new();
        for entry in entries {
            if let Some(slot) = folded
                .iter_mut()
                .find(|held| kind.matches(&held.value, &entry.value, under))
            {
                slot.amount = slot.amount.saturating_add(entry.amount);
            } else {
                folded.push(entry.clone());
            }
        }
        folded
    };
    let before = fold(before);
    let after = fold(after);
    let mut difference = Difference::default();
    for entry in &after {
        let held = before
            .iter()
            .find(|held| kind.matches(&held.value, &entry.value, under))
            .map_or(0, |held| held.amount);
        if entry.amount > held {
            difference
                .added
                .push(Held::new(entry.value.clone(), entry.amount - held));
        }
    }
    for entry in &before {
        let held = after
            .iter()
            .find(|held| kind.matches(&held.value, &entry.value, under))
            .map_or(0, |held| held.amount);
        if entry.amount > held {
            difference
                .removed
                .push(Held::new(entry.value.clone(), entry.amount - held));
        }
    }
    difference
}

/// Section seventeen's single-use tank: one instance and a capacity.
///
/// It holds one instance at a time. Putting a different instance in while
/// something is held is refused rather than replacing what is there, because a
/// tank that silently swapped its contents would lose the first fill.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tank<I> {
    capacity: Amount,
    held: Option<Held<I>>,
}

impl<I> Tank<I> {
    /// An empty tank of one capacity.
    #[must_use]
    pub const fn new(capacity: Amount) -> Self {
        Self {
            capacity,
            held: None,
        }
    }

    /// How much this tank can hold.
    #[must_use]
    pub const fn capacity(&self) -> Amount {
        self.capacity
    }

    /// How much is held.
    #[must_use]
    pub fn amount(&self) -> Amount {
        self.held.as_ref().map_or(0, |held| held.amount)
    }

    /// What is held, if anything.
    #[must_use]
    pub fn held(&self) -> Option<&I> {
        self.held.as_ref().map(|held| &held.value)
    }

    /// How much more would fit.
    #[must_use]
    pub fn room(&self) -> Amount {
        self.capacity.saturating_sub(self.amount())
    }
}

impl<I: Clone> Tank<I> {
    /// Puts a quantity in and answers how much was accepted.
    ///
    /// Accepts nothing at all when a different instance is held.
    pub fn fill(
        &mut self,
        value: &I,
        amount: Amount,
        attempt: Attempt,
        alike: &dyn Fn(&I, &I) -> bool,
    ) -> Amount {
        if let Some(held) = &self.held
            && !alike(&held.value, value)
        {
            return 0;
        }
        let accepted = amount.min(self.room());
        if accepted > 0 && attempt.changes_anything() {
            match &mut self.held {
                Some(held) => held.amount = held.amount.saturating_add(accepted),
                None => self.held = Some(Held::new(value.clone(), accepted)),
            }
        }
        accepted
    }

    /// Takes a quantity out and answers how much was taken.
    pub fn drain(&mut self, amount: Amount, attempt: Attempt) -> Amount {
        let taken = amount.min(self.amount());
        if taken > 0 && attempt.changes_anything() {
            let emptied = self.amount() == taken;
            if let Some(held) = &mut self.held {
                held.amount -= taken;
            }
            if emptied {
                self.held = None;
            }
        }
        taken
    }
}

/// Section seventeen's mutable single-value box.
///
/// It exists so a value can be handed to something that keeps it and changed
/// afterwards, which is what the original uses it for.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueBox<T> {
    value: T,
}

impl<T> ValueBox<T> {
    /// Builds a box around one value.
    pub const fn new(value: T) -> Self {
        Self { value }
    }

    /// Reads what is held.
    pub const fn get(&self) -> &T {
        &self.value
    }

    /// Replaces what is held and hands back what was there.
    pub fn replace(&mut self, value: T) -> T {
        core::mem::replace(&mut self.value, value)
    }

    /// Changes what is held in place.
    pub fn change(&mut self, change: impl FnOnce(&mut T)) {
        change(&mut self.value);
    }
}

/// Fills a tank, matching contents by name.
pub fn fill(tank: &mut Tank<String>, fluid: &str, amount: u64, attempt: Attempt) -> u64 {
    tank.fill(&fluid.to_owned(), amount, attempt, &|left, right| {
        left == right
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quantity::KindError;
    use ravel_sdk::{JsonValue, json};

    #[test]
    fn a_tank_fills_and_drains_and_refuses_a_second_fluid() {
        let mut tank = Tank::new(10);
        assert_eq!(fill(&mut tank, "water", 6, Attempt::Real), 6);
        assert_eq!(fill(&mut tank, "lava", 4, Attempt::Real), 0);
        assert_eq!(fill(&mut tank, "water", 99, Attempt::Simulated), 4);
        assert_eq!(tank.amount(), 6, "a simulation changed nothing");
        assert_eq!(tank.drain(99, Attempt::Real), 6);
    }

    #[derive(Clone, Copy, Debug)]
    struct Ones;

    impl QuantityKind for Ones {
        type Instance = u32;

        fn conditions(&self) -> Conditions {
            Conditions::NONE.with(0)
        }

        fn matches(&self, left: &u32, right: &u32, under: Conditions) -> bool {
            !under.holds(0) || left == right
        }

        fn digest(&self, value: &u32, under: Conditions) -> u64 {
            if under.holds(0) { u64::from(*value) } else { 0 }
        }

        fn write(&self, value: &u32) -> JsonValue {
            json!(value)
        }

        fn read(&self, stored: &JsonValue) -> Result<u32, KindError> {
            stored
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| KindError::new("not a number this kind holds"))
        }
    }

    /// A store with a fixed room, which is enough to drive every branch of the
    /// move: `takes_back` is what makes a recovery fail.
    #[derive(Debug)]
    struct Room {
        held: Amount,
        available: Amount,
        /// What a real insertion accepts, when it is less than it promised.
        real_cap: Option<Amount>,
        /// What a real extraction yields, when a concurrent drain has taken
        /// some of what the simulation saw.
        real_take: Option<Amount>,
        takes_back: bool,
    }

    impl Room {
        const fn holding(held: Amount) -> Self {
            Self {
                held,
                available: 0,
                real_cap: None,
                real_take: None,
                takes_back: true,
            }
        }

        const fn empty(room: Amount) -> Self {
            Self {
                held: 0,
                available: room,
                real_cap: None,
                real_take: None,
                takes_back: true,
            }
        }
    }

    impl Store for Room {
        type Kind = Ones;

        fn kind(&self) -> &Ones {
            &Ones
        }

        fn insert(&mut self, _value: &u32, amount: Amount, attempt: Attempt) -> Amount {
            let mut accepted = amount.min(self.available);
            if attempt.changes_anything() {
                if !self.takes_back {
                    return 0;
                }
                if let Some(cap) = self.real_cap {
                    accepted = accepted.min(cap);
                }
                self.held += accepted;
                self.available -= accepted;
            }
            accepted
        }

        fn extract(&mut self, _value: &u32, amount: Amount, attempt: Attempt) -> Amount {
            let mut taken = amount.min(self.held);
            if attempt.changes_anything() {
                if let Some(cap) = self.real_take {
                    taken = taken.min(cap);
                }
                self.held -= taken;
                self.available += taken;
            }
            taken
        }
    }

    #[test]
    fn a_move_is_bounded_by_the_source_the_destination_and_the_request() {
        let mut from = Room::holding(10);
        let mut to = Room::empty(4);
        assert_eq!(move_quantity(&mut from, &mut to, &1, 100), Ok(4));
        assert_eq!(from.held, 6);
        assert_eq!(to.held, 4);
    }

    #[test]
    fn a_move_from_an_empty_source_or_into_a_full_destination_moves_nothing() {
        let mut empty = Room::holding(0);
        let mut room = Room::empty(4);
        assert_eq!(move_quantity(&mut empty, &mut room, &1, 5), Ok(0));
        let mut from = Room::holding(5);
        let mut full = Room::empty(0);
        assert_eq!(move_quantity(&mut from, &mut full, &1, 5), Ok(0));
        assert_eq!(from.held, 5, "nothing left the source");
    }

    #[test]
    fn a_destination_that_accepts_less_than_it_promised_completes_at_the_reduced_amount() {
        let mut from = Room::holding(10);
        let mut to = Room::empty(8);
        to.real_cap = Some(3);
        assert_eq!(
            move_quantity(&mut from, &mut to, &1, 8),
            Ok(3),
            "the shortfall goes back and the reduced amount is what moved"
        );
        assert_eq!(to.held, 3);
        assert_eq!(from.held, 7, "the shortfall is back in the source");
    }

    #[test]
    fn the_inconsistency_is_raised_only_when_the_recovery_also_fails() {
        let mut from = Room::holding(10);
        from.takes_back = false;
        let mut to = Room::empty(8);
        to.real_cap = Some(3);
        assert_eq!(
            move_quantity(&mut from, &mut to, &1, 8),
            Err(Inconsistent {
                promised: 8,
                accepted: 3,
                stranded: 5,
            })
        );
    }

    #[test]
    fn the_inconsistency_reports_what_the_simulation_promised_not_what_was_taken() {
        // A source drained between the simulation and the real extraction yields
        // less than the destination promised to take, so the two numbers differ
        // and the error has to name the promise.
        let mut from = Room::holding(10);
        from.real_take = Some(5);
        from.takes_back = false;
        let mut to = Room::empty(8);
        to.real_cap = Some(3);
        assert_eq!(
            move_quantity(&mut from, &mut to, &1, 8),
            Err(Inconsistent {
                promised: 8,
                accepted: 3,
                stranded: 2,
            }),
            "eight was promised, five was taken, three landed and two stranded"
        );
    }

    #[test]
    fn a_simulated_call_changes_nothing_and_answers_what_a_real_one_would() {
        let mut store = Room::empty(6);
        assert_eq!(store.insert(&1, 4, Attempt::Simulated), 4);
        assert_eq!(store.held, 0);
        assert_eq!(store.insert(&1, 4, Attempt::Real), 4);
        assert_eq!(store.held, 4);
        assert!(!Attempt::Simulated.changes_anything());
    }

    #[test]
    fn a_slotted_result_names_its_slot_and_nothing_moved_names_none() {
        assert_eq!(Slotted::at(3, 5).slot, Some(3));
        assert_eq!(Slotted::at(3, 0), Slotted::NOTHING);
    }

    /// A store in numbered slots, which is what the slotted form is for. This
    /// package publishes the vocabulary and a dependent implements it over its
    /// own store; this is the fixture that holds the added guarantee to a test.
    #[derive(Debug)]
    struct Shelf {
        slots: Vec<Amount>,
        each: Amount,
    }

    impl Shelf {
        fn of(slots: &[Amount], each: Amount) -> Self {
            Self {
                slots: slots.to_vec(),
                each,
            }
        }

        fn first_with_room(&self) -> Option<usize> {
            self.slots.iter().position(|held| *held < self.each)
        }
    }

    impl Store for Shelf {
        type Kind = Ones;

        fn kind(&self) -> &Ones {
            &Ones
        }

        fn insert(&mut self, value: &u32, amount: Amount, attempt: Attempt) -> Amount {
            self.insert_slotted(value, amount, attempt).amount
        }

        fn extract(&mut self, _value: &u32, amount: Amount, attempt: Attempt) -> Amount {
            let mut left = amount;
            let mut taken = 0;
            for held in &mut self.slots {
                let from_here = left.min(*held);
                if attempt.changes_anything() {
                    *held -= from_here;
                }
                taken += from_here;
                left -= from_here;
            }
            taken
        }
    }

    impl SlottedStore for Shelf {
        fn slots(&self) -> usize {
            self.slots.len()
        }

        fn insert_slotted(&mut self, _value: &u32, amount: Amount, attempt: Attempt) -> Slotted {
            let Some(slot) = self.first_with_room() else {
                return Slotted::NOTHING;
            };
            let Some(held) = self.slots.get_mut(slot) else {
                return Slotted::NOTHING;
            };
            let accepted = amount.min(self.each - *held);
            if attempt.changes_anything() {
                *held += accepted;
            }
            Slotted::at(slot, accepted)
        }
    }

    #[test]
    fn the_slotted_form_answers_which_slot_a_result_came_from_and_the_slotless_one_cannot() {
        let mut shelf = Shelf::of(&[4, 0, 0], 4);
        let landed = shelf.insert_slotted(&1, 3, Attempt::Simulated);
        assert_eq!(
            landed,
            Slotted::at(1, 3),
            "the first slot is full, so the second is where it would go"
        );
        assert_eq!(shelf.slots, vec![4, 0, 0], "a simulation changed nothing");

        let landed = shelf.insert_slotted(&1, 3, Attempt::Real);
        assert_eq!(landed.slot, Some(1));
        assert_eq!(landed.amount, 3);
        assert_eq!(shelf.slots, vec![4, 3, 0]);
        assert_eq!(shelf.slots(), 3);

        assert_eq!(
            shelf.insert(&1, 3, Attempt::Simulated),
            1,
            "the slotless form answers the amount and names no slot"
        );

        let mut full = Shelf::of(&[4, 4], 4);
        assert_eq!(
            full.insert_slotted(&1, 1, Attempt::Real),
            Slotted::NOTHING,
            "nothing moved, so no slot is named"
        );
    }

    #[test]
    fn a_difference_reports_only_what_changed_and_folds_repeated_entries_first() {
        let before = [Held::new(1u32, 2), Held::new(1, 3), Held::new(2, 4)];
        let after = [Held::new(1u32, 5), Held::new(3, 1)];
        let changed = difference(&Ones, &before, &after, Conditions::NONE.with(0));
        assert_eq!(changed.added, vec![Held::new(3, 1)]);
        assert_eq!(changed.removed, vec![Held::new(2, 4)]);
        assert!(
            difference(&Ones, &before, &before, Conditions::NONE.with(0)).is_empty(),
            "a snapshot against itself is no change"
        );
    }

    #[test]
    fn a_difference_reports_a_partial_change_as_the_amount_and_not_the_whole_entry() {
        let before = [Held::new(1u32, 9)];
        let after = [Held::new(1u32, 4)];
        let changed = difference(&Ones, &before, &after, Conditions::NONE.with(0));
        assert_eq!(changed.removed, vec![Held::new(1, 5)]);
        assert_eq!(changed.added.len(), 0);
    }

    #[test]
    fn a_tank_refuses_a_second_instance_and_is_bounded_by_its_capacity() {
        let alike = |left: &u32, right: &u32| left == right;
        let mut tank = Tank::new(10);
        assert_eq!(tank.fill(&1, 4, Attempt::Real, &alike), 4);
        assert_eq!(tank.fill(&2, 4, Attempt::Real, &alike), 0);
        assert_eq!(tank.fill(&1, 99, Attempt::Real, &alike), 6);
        assert_eq!(tank.amount(), 10);
        assert_eq!(tank.room(), 0);
        assert_eq!(tank.drain(99, Attempt::Real), 10);
        assert_eq!(tank.held(), None, "an emptied tank holds no instance");
        assert_eq!(tank.fill(&2, 1, Attempt::Real, &alike), 1);
    }

    #[test]
    fn a_simulated_fill_and_drain_leave_the_tank_alone() {
        let alike = |left: &u32, right: &u32| left == right;
        let mut tank = Tank::new(10);
        assert_eq!(tank.fill(&1, 4, Attempt::Simulated, &alike), 4);
        assert_eq!(tank.amount(), 0);
        assert_eq!(tank.fill(&1, 4, Attempt::Real, &alike), 4);
        assert_eq!(tank.drain(2, Attempt::Simulated), 2);
        assert_eq!(tank.amount(), 4);
    }

    #[test]
    fn a_box_hands_back_what_it_held_when_it_is_replaced() {
        let mut value = ValueBox::new(3u32);
        assert_eq!(value.replace(7), 3);
        assert_eq!(*value.get(), 7);
        value.change(|value| *value += 1);
        assert_eq!(*value.get(), 8);
    }
}
