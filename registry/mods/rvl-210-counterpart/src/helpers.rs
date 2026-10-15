//! Section seventeen's helpers: block-entity lookup, item-store manipulation,
//! fluid handling and the side questions.
//!
//! A dependent names the one it needs. Nothing routes between them, because
//! nothing here shares state: only the side questions carry a side at all.

use ravel_sdk::Side;

use crate::menus::{Stack, Store};
use crate::storage::{Attempt, Tank};

/// The side questions a dependent asks.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Sides {
    side: Side,
}

impl Sides {
    /// The side questions for one side.
    #[must_use]
    pub const fn on(side: Side) -> Self {
        Self { side }
    }

    /// The side this answers for.
    #[must_use]
    pub const fn side(self) -> Side {
        self.side
    }

    /// Whether this is a client.
    #[must_use]
    pub const fn is_client(self) -> bool {
        matches!(self.side, Side::Client)
    }

    /// Whether this is either server.
    #[must_use]
    pub const fn is_server(self) -> bool {
        self.side.is_server()
    }

    /// Whether this is a server inside a single-player client process.
    #[must_use]
    pub const fn is_integrated(self) -> bool {
        matches!(self.side, Side::IntegratedServer)
    }
}

/// Looks a block entity up at a position.
///
/// `lookup` is what the level answers, and a level with nothing there answers
/// nothing rather than building one. Which block entities exist is the level's
/// answer rather than this package's, so no side is asked for here.
pub fn block_entity_at<T>(at: [i32; 3], lookup: impl Fn([i32; 3]) -> Option<T>) -> Option<T> {
    lookup(at)
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
    let held = store.slot(slot)?.clone();
    let count = if held.is_empty() {
        stack.count
    } else {
        held.count.checked_add(stack.count)?
    };
    store.put(slot, Stack::of(&stack.prototype, count));
    Some(slot)
}

/// How much of a prototype a store holds.
#[must_use]
pub fn count_of(store: &Store, prototype: &str) -> u64 {
    store.count_of(prototype)
}

/// Fills a tank, matching contents by name.
pub fn fill(tank: &mut Tank<String>, fluid: &str, amount: u64, attempt: Attempt) -> u64 {
    tank.fill(&fluid.to_owned(), amount, attempt, &|left, right| {
        left == right
    })
}

/// Drains a tank.
pub fn drain(tank: &mut Tank<String>, amount: u64, attempt: Attempt) -> u64 {
    tank.drain(amount, attempt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_side_questions_answer_each_of_the_three_sides() {
        assert!(Sides::on(Side::Client).is_client());
        assert!(!Sides::on(Side::Client).is_server());
        assert_eq!(Sides::on(Side::Client).side(), Side::Client);
        assert!(Sides::on(Side::IntegratedServer).is_integrated());
        assert!(Sides::on(Side::IntegratedServer).is_server());
        assert!(Sides::on(Side::DedicatedServer).is_server());
        assert!(!Sides::on(Side::DedicatedServer).is_integrated());
    }

    #[test]
    fn a_block_entity_lookup_answers_what_the_level_has_and_nothing_otherwise() {
        let level = |at: [i32; 3]| (at == [0, 0, 0]).then(|| "press".to_owned());
        assert_eq!(block_entity_at([0, 0, 0], level), Some("press".to_owned()));
        assert_eq!(block_entity_at([1, 0, 0], level), None);
    }

    #[test]
    fn an_insertion_stacks_onto_a_matching_slot_and_takes_an_empty_one_otherwise() {
        let mut store = Store::new(2);
        assert_eq!(insert_first(&mut store, &Stack::of("one", 2)), Some(0));
        assert_eq!(insert_first(&mut store, &Stack::of("one", 3)), Some(0));
        assert_eq!(count_of(&store, "one"), 5);
        assert_eq!(insert_first(&mut store, &Stack::of("two", 1)), Some(1));
        assert_eq!(
            insert_first(&mut store, &Stack::of("three", 1)),
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
            insert_first(&mut store, &Stack::of("one", u32::MAX)),
            Some(0)
        );
        let changes = store.changes();
        assert_eq!(
            insert_first(&mut store, &Stack::of("one", 9)),
            None,
            "answering the slot here would report nine accepted that never were"
        );
        assert_eq!(count_of(&store, "one"), u64::from(u32::MAX));
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
            insert_first(&mut store, &Stack::of("one", u32::MAX)),
            Some(0)
        );
        assert_eq!(
            insert_first(&mut store, &Stack::of("one", 9)),
            Some(1),
            "the first slot does not admit it, so the empty one does"
        );
    }

    #[test]
    fn a_tank_fills_and_drains_and_refuses_a_second_fluid() {
        let mut tank = Tank::new(10);
        assert_eq!(fill(&mut tank, "water", 6, Attempt::Real), 6);
        assert_eq!(fill(&mut tank, "lava", 4, Attempt::Real), 0);
        assert_eq!(fill(&mut tank, "water", 99, Attempt::Simulated), 4);
        assert_eq!(tank.amount(), 6, "a simulation changed nothing");
        assert_eq!(drain(&mut tank, 99, Attempt::Real), 6);
    }
}
