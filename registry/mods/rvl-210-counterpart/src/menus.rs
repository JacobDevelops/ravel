//! Section eight: the menu and store semantics that are behaviour rather than
//! presentation.
//!
//! Everything about drawing is section eighteen's exclusion. What survives is
//! the slot-range vocabulary that makes shift-click transfer behave, the store
//! with its dirty-marking callback, the indexed wrapper that answers "which
//! slots hold something matching this" without scanning, and the change counter
//! a dependent polls instead of comparing contents.

use std::collections::BTreeMap;

/// One stack in a store: what it is and how many.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct Stack {
    /// What it is. Empty means the slot holds nothing.
    pub prototype: String,
    /// How many.
    pub count: u32,
}

impl Stack {
    /// A stack of something.
    #[must_use]
    pub fn of(prototype: &str, count: u32) -> Self {
        Self {
            prototype: prototype.to_owned(),
            count,
        }
    }

    /// Whether this slot holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.prototype.is_empty() || self.count == 0
    }
}

/// What a run of slots is for.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Role {
    /// The player's own inventory.
    Player,
    /// The machine's working slots.
    Machine,
    /// Slots a result comes out of and nothing goes into.
    OutputOnly,
}

/// A run of slots with one role.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Range {
    /// The first slot.
    pub from: usize,
    /// One past the last slot.
    pub to: usize,
    /// What the run is for.
    pub role: Role,
}

impl Range {
    /// Declares a run. A backwards run is empty rather than refused.
    #[must_use]
    pub const fn new(from: usize, to: usize, role: Role) -> Self {
        Self {
            from,
            to: if to < from { from } else { to },
            role,
        }
    }

    /// Whether a slot is in this run.
    #[must_use]
    pub const fn holds(&self, slot: usize) -> bool {
        slot >= self.from && slot < self.to
    }

    /// How many slots the run covers.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.to - self.from
    }

    /// Whether the run covers nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.to == self.from
    }
}

/// A menu's declared slot ranges.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Ranges {
    ranges: Vec<Range>,
}

impl Ranges {
    /// A menu with no run declared.
    #[must_use]
    pub const fn new() -> Self {
        Self { ranges: Vec::new() }
    }

    /// Declares one run.
    #[must_use]
    pub fn with(mut self, range: Range) -> Self {
        self.ranges.push(range);
        self
    }

    /// The role of a slot, if any run claims it.
    #[must_use]
    pub fn role(&self, slot: usize) -> Option<Role> {
        self.ranges
            .iter()
            .find(|range| range.holds(slot))
            .map(|range| range.role)
    }

    /// Where a shift-click from a slot sends its contents.
    ///
    /// This is the whole reason the vocabulary exists: a player's slot sends to
    /// the machine, a machine's and an output slot both send back to the player,
    /// and a slot no run claims sends nowhere. Nothing is ever sent into an
    /// output-only run.
    #[must_use]
    pub fn transfer_target(&self, slot: usize) -> Option<Role> {
        match self.role(slot)? {
            Role::Player => self
                .ranges
                .iter()
                .any(|range| range.role == Role::Machine && !range.is_empty())
                .then_some(Role::Machine),
            Role::Machine | Role::OutputOnly => self
                .ranges
                .iter()
                .any(|range| range.role == Role::Player && !range.is_empty())
                .then_some(Role::Player),
        }
    }

    /// Whether a slot admits something put into it.
    #[must_use]
    pub fn admits_insertion(&self, slot: usize) -> bool {
        !matches!(self.role(slot), Some(Role::OutputOnly) | None)
    }

    /// Every run, in declaration order.
    #[must_use]
    pub fn ranges(&self) -> &[Range] {
        &self.ranges
    }
}

/// A store of stacks with a dirty-marking callback and a change counter.
///
/// The counter increments on every mutation, which is what a dependent polls
/// instead of comparing contents. It saturates rather than wrapping: a wrapped
/// counter can return to a value a dependent already saw, which is exactly the
/// case polling it is meant to catch.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Store {
    slots: Vec<Stack>,
    changes: u64,
    dirty: bool,
    index: BTreeMap<String, Vec<usize>>,
}

impl Store {
    /// A store of a fixed number of empty slots.
    #[must_use]
    pub fn new(slots: usize) -> Self {
        Self {
            slots: vec![Stack::default(); slots],
            changes: 0,
            dirty: false,
            index: BTreeMap::new(),
        }
    }

    /// How many slots there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether the store has no slots at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// What is in a slot.
    #[must_use]
    pub fn slot(&self, slot: usize) -> Option<&Stack> {
        self.slots.get(slot)
    }

    /// How many times this store has changed.
    #[must_use]
    pub const fn changes(&self) -> u64 {
        self.changes
    }

    /// Whether the owner has been told to save.
    #[must_use]
    pub const fn dirty(&self) -> bool {
        self.dirty
    }

    /// Clears the dirty mark, which the owner does once it has saved.
    pub const fn cleared(&mut self) {
        self.dirty = false;
    }

    /// Puts a stack in a slot, notifying the owner.
    ///
    /// Answers whether the slot exists. A write past the end changes nothing and
    /// does not count as a change.
    pub fn put(&mut self, slot: usize, stack: Stack) -> bool {
        if slot >= self.slots.len() {
            return false;
        }
        let now = if stack.is_empty() {
            String::new()
        } else {
            stack.prototype.clone()
        };
        let Some(held) = self.slots.get_mut(slot) else {
            return false;
        };
        let was = core::mem::replace(held, stack);
        self.changes = self.changes.saturating_add(1);
        self.dirty = true;
        let before = if was.is_empty() {
            String::new()
        } else {
            was.prototype
        };
        if before != now {
            self.unindex(slot, &before);
        }
        if !now.is_empty() {
            let slots = self.index.entry(now).or_default();
            if !slots.contains(&slot) {
                slots.push(slot);
                slots.sort_unstable();
            }
        }
        true
    }

    /// Empties a slot, notifying the owner.
    pub fn clear(&mut self, slot: usize) -> bool {
        self.put(slot, Stack::default())
    }

    /// Which slots hold something matching a prototype.
    ///
    /// Answered from the index maintained on write, so the cost does not grow
    /// with the size of the store. The slots come back in ascending order.
    #[must_use]
    pub fn slots_holding(&self, prototype: &str) -> &[usize] {
        self.index.get(prototype).map_or(&[], Vec::as_slice)
    }

    /// How much of a prototype the store holds in total.
    #[must_use]
    pub fn count_of(&self, prototype: &str) -> u64 {
        self.slots_holding(prototype)
            .iter()
            .filter_map(|slot| self.slots.get(*slot))
            .fold(0u64, |total, held| {
                total.saturating_add(u64::from(held.count))
            })
    }

    fn unindex(&mut self, slot: usize, prototype: &str) {
        if prototype.is_empty() {
            return;
        }
        if let Some(slots) = self.index.get_mut(prototype) {
            slots.retain(|held| *held != slot);
            if slots.is_empty() {
                self.index.remove(prototype);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ranges() -> Ranges {
        Ranges::new()
            .with(Range::new(0, 3, Role::Machine))
            .with(Range::new(3, 4, Role::OutputOnly))
            .with(Range::new(4, 40, Role::Player))
    }

    #[test]
    fn a_slot_belongs_to_the_run_that_claims_it_and_to_no_other() {
        let ranges = ranges();
        assert_eq!(ranges.role(0), Some(Role::Machine));
        assert_eq!(ranges.role(3), Some(Role::OutputOnly));
        assert_eq!(ranges.role(39), Some(Role::Player));
        assert_eq!(ranges.role(40), None, "one past the last run");
    }

    #[test]
    fn shift_click_sends_a_player_slot_to_the_machine_and_everything_else_back() {
        let ranges = ranges();
        assert_eq!(ranges.transfer_target(4), Some(Role::Machine));
        assert_eq!(ranges.transfer_target(0), Some(Role::Player));
        assert_eq!(
            ranges.transfer_target(3),
            Some(Role::Player),
            "an output slot sends back rather than nowhere"
        );
        assert_eq!(ranges.transfer_target(99), None);
    }

    #[test]
    fn nothing_is_ever_put_into_an_output_only_run() {
        let ranges = ranges();
        assert!(ranges.admits_insertion(0));
        assert!(ranges.admits_insertion(4));
        assert!(!ranges.admits_insertion(3));
        assert!(!ranges.admits_insertion(99));
        assert!(
            !ranges
                .ranges()
                .iter()
                .any(|range| range.role == Role::OutputOnly
                    && (range.from..range.to).any(|slot| ranges.admits_insertion(slot)))
        );
    }

    #[test]
    fn a_backwards_run_covers_nothing_rather_than_wrapping() {
        let range = Range::new(9, 2, Role::Player);
        assert!(range.is_empty());
        assert_eq!(range.len(), 0);
        assert!(!range.holds(5));
    }

    #[test]
    fn a_menu_with_no_machine_run_sends_a_player_slot_nowhere() {
        let ranges = Ranges::new().with(Range::new(0, 4, Role::Player));
        assert_eq!(ranges.transfer_target(0), None);
    }

    #[test]
    fn the_change_counter_increments_on_every_mutation_and_the_owner_is_told() {
        let mut store = Store::new(4);
        assert_eq!(store.changes(), 0);
        assert!(!store.dirty());
        store.put(0, Stack::of("one", 3));
        assert_eq!(store.changes(), 1);
        assert!(store.dirty());
        store.cleared();
        assert!(!store.dirty());
        store.clear(0);
        assert_eq!(store.changes(), 2);
        assert!(store.dirty(), "clearing is a mutation like any other");
    }

    #[test]
    fn a_write_past_the_end_changes_nothing_and_does_not_count_as_a_change() {
        let mut store = Store::new(2);
        assert!(!store.put(9, Stack::of("one", 1)));
        assert_eq!(store.changes(), 0);
        assert!(!store.dirty());
    }

    #[test]
    fn the_index_answers_which_slots_hold_a_prototype_and_follows_every_write() {
        let mut store = Store::new(6);
        store.put(4, Stack::of("one", 1));
        store.put(1, Stack::of("one", 2));
        store.put(2, Stack::of("two", 3));
        assert_eq!(store.slots_holding("one"), &[1, 4]);
        assert_eq!(store.slots_holding("two"), &[2]);
        assert_eq!(store.count_of("one"), 3);
        store.clear(1);
        assert_eq!(store.slots_holding("one"), &[4]);
        assert_eq!(store.count_of("one"), 1);
        store.put(4, Stack::of("two", 1));
        assert_eq!(
            store.slots_holding("one").len(),
            0,
            "an emptied prototype leaves no entry behind"
        );
        assert_eq!(store.slots_holding("two"), &[2, 4]);
    }

    #[test]
    fn a_prototype_nothing_holds_answers_an_empty_run_rather_than_scanning() {
        let store = Store::new(6);
        assert_eq!(store.slots_holding("never").len(), 0);
        assert_eq!(store.count_of("never"), 0);
    }

    #[test]
    fn a_total_that_would_overflow_saturates() {
        let mut store = Store::new(2);
        store.put(0, Stack::of("one", u32::MAX));
        store.put(1, Stack::of("one", u32::MAX));
        assert_eq!(store.count_of("one"), u64::from(u32::MAX) * 2);
    }

    #[test]
    fn an_empty_stack_is_empty_however_it_was_written() {
        assert!(Stack::default().is_empty());
        assert!(Stack::of("one", 0).is_empty());
        assert!(Stack::of("", 5).is_empty());
        assert!(!Stack::of("one", 1).is_empty());
    }
}
