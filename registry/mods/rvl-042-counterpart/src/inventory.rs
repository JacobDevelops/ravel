//! The saved shape of every store a dependent holds.
//!
//! A store is a fixed number of slots holding stacks. Its saved form is a list
//! of entries, one per occupied slot and each carrying its slot index, plus the
//! slot count. On load the count grows the store and never reduces it, and an
//! entry that fails to decode is skipped while the rest still loads. That
//! partial tolerance is the difference between a removed dependent costing one
//! slot and costing the whole machine.
//!
//! The one deliberate departure from the host's own encoding is the count: it is
//! written whole so a slot may hold far more than a normal maximum, which is
//! what a dependent storing bulk material relies on.

use std::fmt;
use std::sync::Arc;

use ravel_sdk::{JsonValue, json};

/// The largest a normal stack is, and the unit a raised per-slot limit is a
/// whole multiple of.
pub const NORMAL_MAXIMUM: u64 = 64;

/// The most slots a saved count may grow a store to.
///
/// The specification states grow-never-reduce and no ceiling, so a saved payload
/// declaring a count of two to the fortieth would ask for that many slots and
/// end the process. A saved form is pack-visible state an operator edits by
/// hand, so the count is untrusted input and a counterpart bounds it. The number
/// is a declared limit rather than a measured one: it is far above any store a
/// dependent builds and far below an allocation that hurts. A dependent that
/// genuinely needs more slots than this raises it here.
pub const MAXIMUM_SLOTS: usize = 65_536;

/// One stack: what it is, how many, and the entry-level data it carries.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Stack {
    /// The entry identifier this stack holds.
    pub entry: String,
    /// How many, which is not held to the normal maximum.
    pub count: u64,
    /// The entry-level data that travels with the stack.
    pub data: JsonValue,
}

impl Stack {
    /// Builds a stack of one entry.
    #[must_use]
    pub fn new(entry: &str, count: u64) -> Self {
        Self {
            entry: entry.to_owned(),
            count,
            data: JsonValue::Null,
        }
    }

    /// Returns whether this stack holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0 || self.entry.is_empty()
    }

    /// Returns whether two stacks may merge: same entry and same data.
    #[must_use]
    pub fn merges_with(&self, other: &Self) -> bool {
        self.entry == other.entry && self.data == other.data
    }

    /// Reads a stack from its saved form.
    fn read(value: &JsonValue) -> Option<Self> {
        let entry = value.get("entry")?.as_str()?;
        let count = value.get("count")?.as_u64()?;
        if entry.is_empty() || count == 0 {
            return None;
        }
        Some(Self {
            entry: entry.to_owned(),
            count,
            data: value.get("data").cloned().unwrap_or(JsonValue::Null),
        })
    }

    /// Writes a stack to its saved form.
    fn write(&self) -> JsonValue {
        json!({ "entry": self.entry, "count": self.count, "data": self.data })
    }
}

/// Who is reaching into the store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Access {
    /// The store's owner, which bypasses the output and predicate rules.
    Owner,
    /// Anything else, which is held to them.
    Outside,
}

/// A predicate consulted on outside access only.
pub type StackPredicate = Arc<dyn Fn(usize, &Stack) -> bool + Send + Sync>;

/// Why a store was refused at construction.
#[derive(Debug, Eq, PartialEq)]
pub enum StoreRefused {
    /// A limit above the normal maximum that is not a whole multiple of it.
    LimitNotAMultiple {
        /// The slot the limit was set on, or `None` for the store-wide default.
        slot: Option<usize>,
        /// The limit that was refused.
        limit: u64,
    },
    /// A limit naming a slot the store does not have.
    NoSuchSlot {
        /// The slot the limit named.
        slot: usize,
    },
}

impl fmt::Display for StoreRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitNotAMultiple { slot, limit } => match slot {
                Some(slot) => write!(
                    formatter,
                    "slot {slot} declares a limit of {limit}, which is above {NORMAL_MAXIMUM} and not a whole multiple of it"
                ),
                None => write!(
                    formatter,
                    "the store declares a default limit of {limit}, which is above {NORMAL_MAXIMUM} and not a whole multiple of it"
                ),
            },
            Self::NoSuchSlot { slot } => {
                write!(formatter, "slot {slot} is outside the store")
            }
        }
    }
}

impl std::error::Error for StoreRefused {}

/// What a load did.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LoadReport {
    /// How many entries were placed.
    pub placed: usize,
    /// How many entries were skipped because they did not decode or were out of
    /// range.
    pub skipped: usize,
    /// How many slots the store grew by.
    pub grown: usize,
}

/// A fixed number of slots holding stacks.
#[derive(Clone, Default)]
pub struct Store {
    slots: Vec<Option<Stack>>,
    default_limit: u64,
    limits: Vec<Option<u64>>,
    outputs: Vec<bool>,
    insert: Option<StackPredicate>,
    extract: Option<StackPredicate>,
}

impl fmt::Debug for Store {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Store")
            .field("slots", &self.slots)
            .field("default_limit", &self.default_limit)
            .field("limits", &self.limits)
            .field("outputs", &self.outputs)
            .field("insert", &self.insert.is_some())
            .field("extract", &self.extract.is_some())
            .finish()
    }
}

impl Store {
    /// Builds a store of a number of slots with one default limit.
    ///
    /// # Errors
    ///
    /// Returns [`StoreRefused::LimitNotAMultiple`] when the default limit is
    /// above the normal maximum without being a whole multiple of it.
    pub fn new(slots: usize, default_limit: u64) -> Result<Self, StoreRefused> {
        check_limit(None, default_limit)?;
        Ok(Self {
            slots: vec![None; slots],
            default_limit,
            limits: vec![None; slots],
            outputs: vec![false; slots],
            insert: None,
            extract: None,
        })
    }

    /// Sets one slot's limit, overriding the store-wide default.
    ///
    /// # Errors
    ///
    /// Returns [`StoreRefused::NoSuchSlot`] for a slot outside the store, and
    /// [`StoreRefused::LimitNotAMultiple`] for a raised limit that is not a
    /// whole multiple of the normal maximum. That refusal is behaviour.
    pub fn set_limit(&mut self, slot: usize, limit: u64) -> Result<(), StoreRefused> {
        if slot >= self.slots.len() {
            return Err(StoreRefused::NoSuchSlot { slot });
        }
        check_limit(Some(slot), limit)?;
        self.limits[slot] = Some(limit);
        Ok(())
    }

    /// Marks a slot as an output slot.
    ///
    /// # Errors
    ///
    /// Returns [`StoreRefused::NoSuchSlot`] for a slot outside the store.
    pub fn mark_output(&mut self, slot: usize) -> Result<(), StoreRefused> {
        if slot >= self.slots.len() {
            return Err(StoreRefused::NoSuchSlot { slot });
        }
        self.outputs[slot] = true;
        Ok(())
    }

    /// Supplies the predicate consulted before an outside insertion.
    pub fn on_insert(&mut self, predicate: StackPredicate) {
        self.insert = Some(predicate);
    }

    /// Supplies the predicate consulted before an outside extraction.
    pub fn on_extract(&mut self, predicate: StackPredicate) {
        self.extract = Some(predicate);
    }

    /// Returns how many slots the store has.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Returns whether the store has no slot at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Returns what a slot holds.
    #[must_use]
    pub fn slot(&self, slot: usize) -> Option<&Stack> {
        self.slots.get(slot).and_then(Option::as_ref)
    }

    /// Puts a stack into a slot without consulting anything, as the owner does.
    ///
    /// # Errors
    ///
    /// Returns [`StoreRefused::NoSuchSlot`] if `slot` is outside the store.
    pub fn place(&mut self, slot: usize, stack: Option<Stack>) -> Result<(), StoreRefused> {
        if slot >= self.slots.len() {
            return Err(StoreRefused::NoSuchSlot { slot });
        }
        self.slots[slot] = stack.filter(|held| !held.is_empty());
        Ok(())
    }

    /// Returns how many of one entry a slot may hold.
    #[must_use]
    pub fn limit(&self, slot: usize) -> u64 {
        self.limits
            .get(slot)
            .copied()
            .flatten()
            .unwrap_or(self.default_limit)
    }

    /// Returns whether any slot is marked as an output slot.
    #[must_use]
    pub fn has_outputs(&self) -> bool {
        self.outputs.iter().any(|marked| *marked)
    }

    /// Returns whether a stack may be put into a slot.
    ///
    /// The owner bypasses both the output rule and the predicate. With no slot
    /// marked as an output, the output rule does not apply at all.
    #[must_use]
    pub fn may_insert(&self, slot: usize, stack: &Stack, access: Access) -> bool {
        if slot >= self.slots.len() {
            return false;
        }
        if access == Access::Owner {
            return true;
        }
        if self.has_outputs() && self.outputs[slot] {
            return false;
        }
        self.insert
            .as_ref()
            .is_none_or(|predicate| predicate(slot, stack))
    }

    /// Returns whether a slot may be taken from.
    #[must_use]
    pub fn may_extract(&self, slot: usize, access: Access) -> bool {
        let Some(held) = self.slots.get(slot) else {
            return false;
        };
        if access == Access::Owner {
            return true;
        }
        if self.has_outputs() && !self.outputs[slot] {
            return false;
        }
        let Some(stack) = held else {
            return false;
        };
        self.extract
            .as_ref()
            .is_none_or(|predicate| predicate(slot, stack))
    }

    /// Writes the store's saved form: one entry per occupied slot, and the count.
    #[must_use]
    pub fn save(&self) -> JsonValue {
        let entries: Vec<JsonValue> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, held)| {
                held.as_ref()
                    .map(|stack| json!({ "slot": index, "stack": stack.write() }))
            })
            .collect();
        json!({ "entries": entries, "count": self.slots.len() })
    }

    /// Reads a saved form back, growing to the recorded count and never below.
    ///
    /// An entry that does not decode, or whose index is outside the store, is
    /// skipped and counted rather than failing the load. A count above
    /// [`MAXIMUM_SLOTS`] is refused the same way: the store keeps the size it
    /// has and the refusal is counted as a skip rather than allocated.
    pub fn load(&mut self, payload: &JsonValue) -> LoadReport {
        let mut report = LoadReport::default();
        if let Some(count) = payload.get("count").and_then(JsonValue::as_u64) {
            match usize::try_from(count) {
                Ok(wanted) if wanted <= MAXIMUM_SLOTS => {
                    if wanted > self.slots.len() {
                        report.grown = wanted - self.slots.len();
                        self.slots.resize(wanted, None);
                        self.limits.resize(wanted, None);
                        self.outputs.resize(wanted, false);
                    }
                }
                _ => report.skipped += 1,
            }
        }
        let Some(entries) = payload.get("entries").and_then(JsonValue::as_array) else {
            report.skipped += 1;
            return report;
        };
        for entry in entries {
            let placed = entry
                .get("slot")
                .and_then(JsonValue::as_u64)
                .and_then(|slot| usize::try_from(slot).ok())
                .filter(|slot| *slot < self.slots.len())
                .zip(entry.get("stack").and_then(Stack::read));
            match placed {
                Some((slot, stack)) => {
                    self.slots[slot] = Some(stack);
                    report.placed += 1;
                }
                None => report.skipped += 1,
            }
        }
        report
    }
}

/// One direction's view of a store, consulting a direction-aware predicate.
///
/// The original's wrapped view answers "may this go here" as false when no
/// insert predicate is set, where the unwrapped store answers true. That looks
/// unintended, so both default to permitting here and the divergence is recorded.
#[derive(Clone)]
pub struct Facing<'store> {
    store: &'store Store,
    allows: Option<StackPredicate>,
}

impl fmt::Debug for Facing<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Facing")
            .field("allows", &self.allows.is_some())
            .finish()
    }
}

impl<'store> Facing<'store> {
    /// Builds a view over a store for one direction.
    #[must_use]
    pub const fn new(store: &'store Store, allows: Option<StackPredicate>) -> Self {
        Self { store, allows }
    }

    /// Returns whether a stack may be put into a slot from this direction.
    #[must_use]
    pub fn may_insert(&self, slot: usize, stack: &Stack) -> bool {
        self.allows
            .as_ref()
            .is_none_or(|predicate| predicate(slot, stack))
            && self.store.may_insert(slot, stack, Access::Outside)
    }

    /// Returns whether a slot may be taken from in this direction.
    ///
    /// The direction's own predicate is consulted first, over what the slot
    /// holds, and an empty slot is taken from by nobody.
    #[must_use]
    pub fn may_extract(&self, slot: usize) -> bool {
        let Some(stack) = self.store.slot(slot) else {
            return false;
        };
        self.allows
            .as_ref()
            .is_none_or(|predicate| predicate(slot, stack))
            && self.store.may_extract(slot, Access::Outside)
    }
}

fn check_limit(slot: Option<usize>, limit: u64) -> Result<(), StoreRefused> {
    if limit > NORMAL_MAXIMUM && !limit.is_multiple_of(NORMAL_MAXIMUM) {
        return Err(StoreRefused::LimitNotAMultiple { slot, limit });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(slots: usize) -> Store {
        Store::new(slots, NORMAL_MAXIMUM).expect("the normal maximum is a legal default")
    }

    #[test]
    fn a_raised_limit_that_is_not_a_whole_multiple_is_refused_at_construction() {
        assert_eq!(
            Store::new(1, 100).map(|_| ()),
            Err(StoreRefused::LimitNotAMultiple {
                slot: None,
                limit: 100
            })
        );
        let mut held = store(2);
        assert_eq!(
            held.set_limit(0, 65),
            Err(StoreRefused::LimitNotAMultiple {
                slot: Some(0),
                limit: 65
            })
        );
        assert!(held.set_limit(0, NORMAL_MAXIMUM * 8).is_ok());
        assert_eq!(held.limit(0), 512);
        assert_eq!(held.limit(1), NORMAL_MAXIMUM);
    }

    #[test]
    fn a_limit_below_the_normal_maximum_is_admitted_whatever_it_is() {
        let mut held = store(1);
        assert!(held.set_limit(0, 7).is_ok());
        assert_eq!(held.limit(0), 7);
    }

    #[test]
    fn a_count_above_the_normal_maximum_round_trips() {
        let mut held = store(1);
        held.place(0, Some(Stack::new("bulk", 4096)))
            .expect("slot zero is in the store");
        let saved = held.save();

        let mut read = store(1);
        let report = read.load(&saved);
        assert_eq!(report.placed, 1);
        assert_eq!(read.slot(0).map(|stack| stack.count), Some(4096));
    }

    #[test]
    fn the_saved_form_carries_one_entry_per_occupied_slot_with_its_index() {
        let mut held = store(4);
        held.place(2, Some(Stack::new("one", 3))).expect("in range");
        let saved = held.save();
        assert_eq!(saved["count"], 4);
        assert_eq!(saved["entries"].as_array().map(Vec::len), Some(1));
        assert_eq!(saved["entries"][0]["slot"], 2);
    }

    #[test]
    fn a_load_grows_to_the_recorded_count_and_never_below_the_current_size() {
        let mut small = store(2);
        small
            .place(0, Some(Stack::new("one", 1)))
            .expect("in range");
        let saved = small.save();

        let mut large = store(9);
        let report = large.load(&saved);
        assert_eq!(large.len(), 9, "a smaller recorded count does not reduce");
        assert_eq!(report.grown, 0);

        let mut grown = store(1);
        let report = grown.load(&json!({ "entries": [], "count": 6 }));
        assert_eq!(grown.len(), 6);
        assert_eq!(report.grown, 5);
    }

    #[test]
    fn an_entry_that_does_not_decode_is_skipped_and_the_rest_still_loads() {
        let mut held = store(3);
        let report = held.load(&json!({
            "count": 3,
            "entries": [
                { "slot": 0, "stack": { "entry": "one", "count": 2 } },
                { "slot": 1, "stack": { "entry": "", "count": 2 } },
                { "slot": 9, "stack": { "entry": "far", "count": 1 } },
                { "slot": 2, "stack": { "entry": "two", "count": 5 } }
            ]
        }));
        assert_eq!(report.placed, 2);
        assert_eq!(report.skipped, 2);
        assert_eq!(held.slot(0).map(|stack| stack.count), Some(2));
        assert_eq!(held.slot(2).map(|stack| stack.count), Some(5));
        assert!(held.slot(1).is_none());
    }

    #[test]
    fn with_no_slot_marked_as_output_neither_restriction_applies() {
        let mut held = store(2);
        held.place(0, Some(Stack::new("one", 1))).expect("in range");
        assert!(held.may_insert(0, &Stack::new("one", 1), Access::Outside));
        assert!(held.may_extract(0, Access::Outside));
    }

    #[test]
    fn an_output_slot_refuses_outside_insertion_and_a_plain_slot_refuses_outside_extraction() {
        let mut held = store(2);
        held.mark_output(1).expect("in range");
        held.place(0, Some(Stack::new("one", 1))).expect("in range");
        held.place(1, Some(Stack::new("two", 1))).expect("in range");

        assert!(!held.may_insert(1, &Stack::new("two", 1), Access::Outside));
        assert!(held.may_insert(0, &Stack::new("one", 1), Access::Outside));
        assert!(!held.may_extract(0, Access::Outside));
        assert!(held.may_extract(1, Access::Outside));
    }

    #[test]
    fn the_owner_bypasses_the_output_rule_and_both_predicates() {
        let mut held = store(1);
        held.mark_output(0).expect("in range");
        held.place(0, Some(Stack::new("one", 1))).expect("in range");
        held.on_insert(Arc::new(|_, _| false));
        held.on_extract(Arc::new(|_, _| false));

        assert!(held.may_insert(0, &Stack::new("one", 1), Access::Owner));
        assert!(held.may_extract(0, Access::Owner));
        assert!(!held.may_insert(0, &Stack::new("one", 1), Access::Outside));
        assert!(!held.may_extract(0, Access::Outside));
    }

    #[test]
    fn a_direction_view_with_no_predicate_permits_where_the_original_refused() {
        let held = store(1);
        let facing = Facing::new(&held, None);
        assert!(facing.may_insert(0, &Stack::new("one", 1)));

        let refusing = Facing::new(&held, Some(Arc::new(|_, _| false)));
        assert!(!refusing.may_insert(0, &Stack::new("one", 1)));
    }

    #[test]
    fn a_direction_consults_its_own_predicate_before_taking_from_a_slot() {
        let mut held = store(2);
        held.place(0, Some(Stack::new("one", 1))).expect("in range");
        held.place(1, Some(Stack::new("two", 1))).expect("in range");

        let permitting = Facing::new(&held, Some(Arc::new(|slot, _| slot == 0)));
        assert!(permitting.may_extract(0));
        assert!(
            !permitting.may_extract(1),
            "a direction wired to refuse extraction refuses it"
        );

        let plain = Facing::new(&held, None);
        assert!(plain.may_extract(0), "no predicate permits both directions");
        assert!(plain.may_extract(1));
        assert!(!plain.may_extract(2), "there is no such slot to take from");
    }

    #[test]
    fn a_saved_count_above_the_declared_maximum_is_skipped_rather_than_allocated() {
        let mut held = store(2);
        let report = held.load(&json!({ "entries": [], "count": 1_099_511_627_776_u64 }));
        assert_eq!(held.len(), 2, "a hostile count grows the store by nothing");
        assert_eq!(report.grown, 0);
        assert_eq!(report.skipped, 1);

        let report = held.load(&json!({ "entries": [], "count": u64::MAX }));
        assert_eq!(held.len(), 2);
        assert_eq!(report.skipped, 1);

        let mut room = store(0);
        let report = room.load(&json!({ "entries": [], "count": MAXIMUM_SLOTS }));
        assert_eq!(
            room.len(),
            MAXIMUM_SLOTS,
            "the declared maximum still grows"
        );
        assert_eq!(report.skipped, 0);
    }

    #[test]
    fn a_payload_carrying_no_entry_list_is_counted_as_a_skip() {
        let mut held = store(1);
        let report = held.load(&json!({ "count": 1 }));
        assert_eq!(report.skipped, 1);

        let report = held.load(&json!({ "count": 1, "entries": "not a list" }));
        assert_eq!(report.skipped, 1);

        let mut empty = store(1);
        let report = empty.load(&json!({ "count": 1, "entries": [] }));
        assert_eq!(
            report.skipped, 0,
            "an inventory that is legitimately empty is not a skipped entry"
        );
    }

    #[test]
    fn a_deep_copy_carries_limits_outputs_predicates_and_stacks() {
        let mut held = store(2);
        held.set_limit(0, NORMAL_MAXIMUM * 2).expect("a multiple");
        held.mark_output(1).expect("in range");
        held.on_insert(Arc::new(|slot, _| slot == 0));
        held.place(0, Some(Stack::new("one", 3))).expect("in range");

        let copy = held.clone();
        assert_eq!(copy.limit(0), 128);
        assert!(copy.has_outputs());
        assert_eq!(copy.slot(0), Some(&Stack::new("one", 3)));
        assert!(!copy.may_insert(1, &Stack::new("one", 1), Access::Outside));
    }

    #[test]
    fn two_stacks_merge_only_on_the_same_entry_and_the_same_data() {
        let mut one = Stack::new("one", 1);
        let two = Stack::new("one", 1);
        assert!(one.merges_with(&two));
        one.data = json!({ "charge": 1 });
        assert!(!one.merges_with(&two));
        assert!(!Stack::new("one", 1).merges_with(&Stack::new("two", 1)));
    }
}
