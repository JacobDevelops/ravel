//! Section eight: the menu and store semantics that are behaviour rather than
//! presentation.
//!
//! Everything about drawing is section eighteen's exclusion. What survives is
//! the slot-range vocabulary that makes shift-click transfer behave, the store
//! with its dirty-marking callback, the indexed wrapper that answers "which
//! slots hold something matching this" without scanning, and the change counter
//! a dependent polls instead of comparing contents.

use std::collections::BTreeMap;

use ravel_sdk::content::ContentId;

/// One stack in a store: what it is and how many.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Stack {
    /// What it is; always a valid namespaced identity.
    pub prototype: ContentId,
    /// How many. Zero alone denotes an empty slot.
    pub count: u32,
}

impl Default for Stack {
    fn default() -> Self {
        Self::of(
            "minecraft:air"
                .parse()
                .expect("a valid static item identity"),
            0,
        )
    }
}

impl Stack {
    /// A stack of something.
    #[must_use]
    pub fn of(prototype: ContentId, count: u32) -> Self {
        Self { prototype, count }
    }

    /// Whether this slot holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
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
    index: BTreeMap<ContentId, Vec<usize>>,
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
        let now = (!stack.is_empty()).then(|| stack.prototype.clone());
        let Some(held) = self.slots.get_mut(slot) else {
            return false;
        };
        let was = core::mem::replace(held, stack);
        self.changes = self.changes.saturating_add(1);
        self.dirty = true;
        let before = (!was.is_empty()).then_some(was.prototype);
        if before != now
            && let Some(before) = &before
        {
            self.unindex(slot, before);
        }
        if let Some(now) = now {
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
    pub fn slots_holding(&self, prototype: &ContentId) -> &[usize] {
        self.index.get(prototype).map_or(&[], Vec::as_slice)
    }

    /// How much of a prototype the store holds in total.
    #[must_use]
    pub fn count_of(&self, prototype: &ContentId) -> u64 {
        self.slots_holding(prototype)
            .iter()
            .filter_map(|slot| self.slots.get(*slot))
            .fold(0u64, |total, held| {
                total.saturating_add(u64::from(held.count))
            })
    }

    fn unindex(&mut self, slot: usize, prototype: &ContentId) {
        if let Some(slots) = self.index.get_mut(prototype) {
            slots.retain(|held| *held != slot);
            if slots.is_empty() {
                self.index.remove(prototype);
            }
        }
    }
}

/// Puts a stack into the first slot that admits it.
///
/// Answers the slot it went into, or nothing when no slot admits it. A slot
/// already holding another prototype does not admit it, and neither does one
/// that cannot take the whole stack: saturating there would destroy the overflow
/// and still answer the slot, so a caller could not tell that part of its stack
/// was never accepted. The store's own change counter is what a dependent
/// watches afterwards.
#[must_use]
pub fn insert_first(store: &mut Store, stack: &Stack) -> Option<usize> {
    if stack.is_empty() {
        return None;
    }
    let admits = |held: &Stack| {
        if held.is_empty() {
            return true;
        }
        held.prototype == stack.prototype && held.count.checked_add(stack.count).is_some()
    };
    let slot = (0..store.len()).find(|slot| store.slot(*slot).is_some_and(admits))?;
    let held = store.slot(slot)?;
    let count = if held.is_empty() {
        stack.count
    } else {
        held.count.checked_add(stack.count)?
    };
    store.put(slot, Stack::of(stack.prototype.clone(), count));
    Some(slot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_insertion_stacks_onto_a_matching_slot_and_takes_an_empty_one_otherwise() {
        let mut store = Store::new(2);
        assert_eq!(
            insert_first(
                &mut store,
                &Stack::of(
                    "minecraft:one".parse().expect("a valid content identity"),
                    2
                )
            ),
            Some(0)
        );
        assert_eq!(
            insert_first(
                &mut store,
                &Stack::of(
                    "minecraft:one".parse().expect("a valid content identity"),
                    3
                )
            ),
            Some(0)
        );
        assert_eq!(
            store.count_of(&"minecraft:one".parse().expect("a valid item identity")),
            5
        );
        assert_eq!(
            insert_first(
                &mut store,
                &Stack::of(
                    "minecraft:two".parse().expect("a valid content identity"),
                    1
                )
            ),
            Some(1)
        );
        assert_eq!(
            insert_first(
                &mut store,
                &Stack::of(
                    "minecraft:three".parse().expect("a valid content identity"),
                    1
                )
            ),
            None,
            "no slot admits a third prototype"
        );
    }

    #[test]
    fn an_empty_stack_is_inserted_nowhere_and_changes_nothing() {
        let mut store = Store::new(2);
        assert_eq!(insert_first(&mut store, &Stack::default()), None);
        assert_eq!(store.changes(), 0);
    }

    #[test]
    fn an_insertion_that_would_overflow_a_slot_is_refused_rather_than_losing_the_overflow() {
        let mut store = Store::new(1);
        assert_eq!(
            insert_first(
                &mut store,
                &Stack::of(
                    "minecraft:one".parse().expect("a valid content identity"),
                    u32::MAX
                )
            ),
            Some(0)
        );
        let changes = store.changes();
        assert_eq!(
            insert_first(
                &mut store,
                &Stack::of(
                    "minecraft:one".parse().expect("a valid content identity"),
                    9
                )
            ),
            None,
            "answering the slot here would report nine accepted that never were"
        );
        assert_eq!(
            store.count_of(&"minecraft:one".parse().expect("a valid item identity")),
            u64::from(u32::MAX)
        );
        assert_eq!(
            store.changes(),
            changes,
            "a refused insertion changed nothing"
        );
    }

    #[test]
    fn a_second_slot_takes_what_the_first_cannot() {
        let mut store = Store::new(2);
        assert_eq!(
            insert_first(
                &mut store,
                &Stack::of(
                    "minecraft:one".parse().expect("a valid content identity"),
                    u32::MAX
                )
            ),
            Some(0)
        );
        assert_eq!(
            insert_first(
                &mut store,
                &Stack::of(
                    "minecraft:one".parse().expect("a valid content identity"),
                    9
                )
            ),
            Some(1),
            "the first slot does not admit it, so the empty one does"
        );
    }

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
        store.put(
            0,
            Stack::of(
                "minecraft:one".parse().expect("a valid content identity"),
                3,
            ),
        );
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
        assert!(!store.put(
            9,
            Stack::of(
                "minecraft:one".parse().expect("a valid content identity"),
                1
            )
        ));
        assert_eq!(store.changes(), 0);
        assert!(!store.dirty());
    }

    #[test]
    fn the_index_answers_which_slots_hold_a_prototype_and_follows_every_write() {
        let mut store = Store::new(6);
        store.put(
            4,
            Stack::of(
                "minecraft:one".parse().expect("a valid content identity"),
                1,
            ),
        );
        store.put(
            1,
            Stack::of(
                "minecraft:one".parse().expect("a valid content identity"),
                2,
            ),
        );
        store.put(
            2,
            Stack::of(
                "minecraft:two".parse().expect("a valid content identity"),
                3,
            ),
        );
        assert_eq!(
            store.slots_holding(&"minecraft:one".parse().expect("a valid item identity")),
            &[1, 4]
        );
        assert_eq!(
            store.slots_holding(&"minecraft:two".parse().expect("a valid item identity")),
            &[2]
        );
        assert_eq!(
            store.count_of(&"minecraft:one".parse().expect("a valid item identity")),
            3
        );
        store.clear(1);
        assert_eq!(
            store.slots_holding(&"minecraft:one".parse().expect("a valid item identity")),
            &[4]
        );
        assert_eq!(
            store.count_of(&"minecraft:one".parse().expect("a valid item identity")),
            1
        );
        store.put(
            4,
            Stack::of(
                "minecraft:two".parse().expect("a valid content identity"),
                1,
            ),
        );
        assert_eq!(
            store
                .slots_holding(&"minecraft:one".parse().expect("a valid item identity"))
                .len(),
            0,
            "an emptied prototype leaves no entry behind"
        );
        assert_eq!(
            store.slots_holding(&"minecraft:two".parse().expect("a valid item identity")),
            &[2, 4]
        );
    }

    #[test]
    fn a_prototype_nothing_holds_answers_an_empty_run_rather_than_scanning() {
        let store = Store::new(6);
        assert_eq!(
            store
                .slots_holding(&"minecraft:never".parse().expect("a valid item identity"))
                .len(),
            0
        );
        assert_eq!(
            store.count_of(&"minecraft:never".parse().expect("a valid item identity")),
            0
        );
    }

    #[test]
    fn a_total_that_would_overflow_saturates() {
        let mut store = Store::new(2);
        store.put(
            0,
            Stack::of(
                "minecraft:one".parse().expect("a valid content identity"),
                u32::MAX,
            ),
        );
        store.put(
            1,
            Stack::of(
                "minecraft:one".parse().expect("a valid content identity"),
                u32::MAX,
            ),
        );
        assert_eq!(
            store.count_of(&"minecraft:one".parse().expect("a valid item identity")),
            u64::from(u32::MAX) * 2
        );
    }

    #[test]
    fn zero_count_slots_remove_the_previous_index_without_indexing_the_new_identity() {
        let mut store = Store::new(1);
        let before = "minecraft:stone"
            .parse::<ContentId>()
            .expect("a valid item");
        let after = "minecraft:air".parse::<ContentId>().expect("a valid item");
        assert!(store.put(0, Stack::of(before.clone(), 3)));
        assert!(store.put(0, Stack::of(after.clone(), 0)));
        let empty: &[usize] = &[];
        assert_eq!(store.slots_holding(&before), empty);
        assert_eq!(store.slots_holding(&after), empty);
        assert_eq!(store.count_of(&before), 0);
        assert_eq!(store.count_of(&after), 0);
        assert_eq!(insert_first(&mut store, &Stack::of(after, 1)), Some(0));
    }

    #[test]
    fn an_empty_stack_is_empty_however_it_was_written() {
        assert!(Stack::default().is_empty());
        assert!(
            Stack::of(
                "minecraft:one".parse().expect("a valid content identity"),
                0
            )
            .is_empty()
        );
        assert!("".parse::<ContentId>().is_err());
        assert!(
            !Stack::of(
                "minecraft:one".parse().expect("a valid content identity"),
                1
            )
            .is_empty()
        );
    }
}
