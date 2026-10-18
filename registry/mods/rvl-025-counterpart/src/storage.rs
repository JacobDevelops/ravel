//! The handheld carrier of §10: six slots, one selection, and the item swap
//! that follows every change to either.
//!
//! Two of §10's findings are acted on here rather than reproduced. The
//! replacement is written to the hand the interaction came from, where the
//! original writes it to the currently selected hotbar index and misplaces the
//! carrier whenever the two differ; the original contains a commented-out
//! hand-based write, so the divergence was noticed there and not resolved. And
//! the replacement path tests that the selected slot's contents declare carrier
//! support instead of casting to the declaration, which is the latent failure
//! §10 names and the one §13's wholesale payload can otherwise reach.

use crate::data::{Record, RecordRefused, Stack, read_tolerant, write_tolerant};
use ravel_sdk::JsonValue;

/// How many sub-items the carrier holds.
pub const SLOTS: usize = 6;

/// How many slots a cycle looks at before it gives up.
///
/// Six slots and one attempt more, which is what makes a carrier holding exactly
/// one occupied slot select that slot rather than stopping where it started.
pub const CYCLE_ATTEMPTS: usize = SLOTS + 1;

/// Which hand an interaction came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Hand {
    /// The hand a player swings.
    Main,
    /// The other one.
    Off,
}

/// Which direction a cycle step walks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    /// Towards the next slot.
    Forward,
    /// Towards the previous one.
    Backward,
}

/// What using the carrier opened.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Opened {
    /// The carrier's own screen.
    Carrier,
    /// The selected sub-item's screen.
    Selected {
        /// Which slot the sub-item is in.
        slot: usize,
    },
}

/// What the carrier became, and where it was written.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Replacement {
    /// The item form the carrier now presents.
    pub presents: String,
    /// The hand the replacement was written into.
    pub into: Hand,
}

/// The plain form the carrier presents with nothing selected.
pub const PLAIN_FORM: &str = "rvl_025_counterpart:carrier";

/// The component a sub-item nominates its display form under.
///
/// §10's swap replaces the carrier with the item form the selected sub-item
/// nominates. What an item form is belongs to the host; the nomination is the
/// sub-item's, so it travels with the stack.
pub const FORM_COMPONENT: &str = "carrier_form";

/// A carrier's contents and which of them is presented.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Carrier {
    slots: Vec<Option<Stack>>,
    selected: usize,
}

impl Default for Carrier {
    fn default() -> Self {
        Self::new()
    }
}

impl Carrier {
    /// Builds an empty carrier with the first slot selected.
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: vec![None; SLOTS],
            selected: 0,
        }
    }

    /// Returns the six slots in order.
    #[must_use]
    pub fn slots(&self) -> &[Option<Stack>] {
        &self.slots
    }

    /// Returns which slot is selected.
    #[must_use]
    pub const fn selected(&self) -> usize {
        self.selected
    }

    /// Returns the selected slot's contents.
    #[must_use]
    pub fn selection(&self) -> Option<&Stack> {
        self.slots.get(self.selected).and_then(Option::as_ref)
    }

    /// Answers whether the carrier would take this stack.
    ///
    /// A stack that does not declare carrier support is refused, and that
    /// refusal is what the swap of §10 otherwise relies on for its own safety.
    #[must_use]
    pub fn accepts(stack: &Stack) -> bool {
        !stack.is_empty() && stack.carrier_ready
    }

    /// Puts a stack into a slot, refusing an unsupported item or a bad index.
    ///
    /// Each slot holds exactly one item, so the count is normalised to one on
    /// the way in rather than a larger stack being held and mis-reported.
    ///
    /// # Errors
    ///
    /// Returns [`CarrierRefused::Slot`] for an index outside the six and
    /// [`CarrierRefused::Unsupported`] for a stack that does not declare
    /// carrier support.
    pub fn insert(&mut self, slot: usize, stack: Stack) -> Result<(), CarrierRefused> {
        if slot >= SLOTS {
            return Err(CarrierRefused::Slot { index: slot });
        }
        if !Self::accepts(&stack) {
            return Err(CarrierRefused::Unsupported);
        }
        let mut held = stack;
        held.count = 1;
        self.slots[slot] = Some(held);
        Ok(())
    }

    /// Takes from a slot, or reports what is there without taking it.
    ///
    /// A simulated extraction answers with the slot's contents and leaves them.
    /// A real one clears the slot outright and ignores the amount asked for,
    /// which is §10's rule and is why a slot never holds a partial stack.
    #[must_use]
    pub fn extract(&mut self, slot: usize, simulate: bool) -> Option<Stack> {
        let held = self.slots.get(slot)?.clone()?;
        if !simulate {
            self.slots[slot] = None;
        }
        Some(held)
    }

    /// Walks the slots and selects the first non-empty one it reaches.
    ///
    /// The walk is modulo six and gives up after [`CYCLE_ATTEMPTS`]. A carrier
    /// with one occupied slot selects that slot; a carrier with none leaves the
    /// selection where it was. The answer says whether the selection moved,
    /// which is one of the two things that triggers the swap below.
    pub fn cycle(&mut self, step: Step) -> bool {
        let mut candidate = self.selected;
        for _ in 0..CYCLE_ATTEMPTS {
            candidate = match step {
                Step::Forward => (candidate + 1) % SLOTS,
                Step::Backward => (candidate + SLOTS - 1) % SLOTS,
            };
            if self.slots[candidate].is_some() {
                let moved = candidate != self.selected;
                self.selected = candidate;
                return moved;
            }
        }
        false
    }

    /// Answers what using the carrier opens.
    ///
    /// Crouching always opens the carrier's own screen. Otherwise the selected
    /// sub-item's screen opens, or the carrier's own when nothing is selected.
    #[must_use]
    pub fn use_item(&self, crouching: bool) -> Opened {
        if crouching {
            return Opened::Carrier;
        }
        match self.selection() {
            Some(_) => Opened::Selected {
                slot: self.selected,
            },
            None => Opened::Carrier,
        }
    }

    /// Produces the replacement the carrier becomes, written into `hand`.
    ///
    /// The carrier is replaced whenever the selection changes or the selected
    /// slot's contents change: an occupied selection presents the form that
    /// sub-item nominates and an empty one presents the plain carrier. The data
    /// record is carried across by the caller, because the record is the
    /// carrier's and not the replacement's.
    ///
    /// # Errors
    ///
    /// Returns [`CarrierRefused::Unsupported`] when the selected slot holds an
    /// item that does not declare carrier support. Insertion cannot produce
    /// that state; a record written straight into the item can, and §13's
    /// wholesale payload is the route that does. The original casts here without
    /// testing and fails on the next selection change.
    pub fn replacement(&self, hand: Hand) -> Result<Replacement, CarrierRefused> {
        let presents = match self.selection() {
            None => PLAIN_FORM.to_owned(),
            Some(stack) if !Self::accepts(stack) => return Err(CarrierRefused::Unsupported),
            Some(stack) => stack
                .components
                .get(FORM_COMPONENT)
                .cloned()
                .unwrap_or_else(|| stack.identity.to_string()),
        };
        Ok(Replacement {
            presents,
            into: hand,
        })
    }

    /// Writes the carrier as its persisted document.
    #[must_use]
    pub fn to_saved(&self) -> JsonValue {
        ravel_sdk::json!({
            "slots": write_tolerant(&self.slots),
            "selected": self.selected,
        })
    }

    /// Reads a carrier back.
    ///
    /// The selection must name one of the six slots, because this record arrives
    /// from a saved world and from the network. A stored
    /// item that does not declare carrier support is kept, not dropped: it is
    /// exactly the state [`Carrier::replacement`] refuses, and dropping it here
    /// would hide the refusal instead of surfacing it.
    ///
    /// # Errors
    ///
    /// Returns [`CarrierRefused::Record`] for a document that is not the
    /// carrier's shape or carries a malformed selection, and [`CarrierRefused::Slot`]
    /// for a slot list that is not exactly [`SLOTS`] long or an out-of-range selection.
    pub fn from_saved(document: &JsonValue) -> Result<Self, CarrierRefused> {
        let object = document
            .as_object()
            .ok_or(CarrierRefused::Record(RecordRefused::Shape {
                record: Some(Record::CarrierContents),
            }))?;
        let slots = read_tolerant(
            Record::CarrierContents,
            object.get("slots").unwrap_or(&JsonValue::Null),
            SLOTS,
        )
        .map_err(CarrierRefused::Record)?;
        if slots.len() != SLOTS {
            return Err(CarrierRefused::Slot { index: slots.len() });
        }
        let selected = crate::data::saved_field(document, "selected", 0, |value| {
            value.as_u64().and_then(|value| usize::try_from(value).ok())
        })
        .map_err(CarrierRefused::Record)?;
        if selected >= SLOTS {
            return Err(CarrierRefused::Slot { index: selected });
        }
        Ok(Self { slots, selected })
    }
}

/// Why the carrier refused something.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CarrierRefused {
    /// A slot index is outside the six.
    Slot {
        /// The index that was named.
        index: usize,
    },
    /// A stack does not declare itself installable in the carrier.
    Unsupported,
    /// The persisted form is not one a record may carry.
    Record(RecordRefused),
}

impl std::fmt::Display for CarrierRefused {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Slot { index } => write!(formatter, "the carrier has no slot {index}"),
            Self::Unsupported => {
                formatter.write_str("the item does not declare itself installable in a carrier")
            }
            Self::Record(refused) => write!(formatter, "{refused}"),
        }
    }
}

impl std::error::Error for CarrierRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Record(refused) => Some(refused),
            Self::Slot { .. } | Self::Unsupported => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wrapped_record_refusal_remains_the_error_source() {
        use std::error::Error;

        let record = RecordRefused::Shape { record: None };
        let refused = CarrierRefused::Record(record.clone());
        assert_eq!(
            refused
                .source()
                .and_then(|source| source.downcast_ref::<RecordRefused>()),
            Some(&record)
        );
        assert!(CarrierRefused::Slot { index: 0 }.source().is_none());
        assert!(CarrierRefused::Unsupported.source().is_none());
    }

    fn ready(identity: &str) -> Stack {
        let mut stack = Stack::of(identity).expect("a legal identity");
        stack.carrier_ready = true;
        stack
    }

    #[test]
    fn the_carrier_refuses_an_item_that_does_not_declare_carrier_support() {
        let mut carrier = Carrier::new();
        let plain = Stack::of("counterpart:plain").expect("a legal identity");

        assert_eq!(carrier.insert(0, plain), Err(CarrierRefused::Unsupported));
        assert_eq!(
            carrier.insert(SLOTS, ready("counterpart:device")),
            Err(CarrierRefused::Slot { index: SLOTS })
        );
        assert!(carrier.slots().iter().all(Option::is_none));
    }

    #[test]
    fn a_slot_holds_exactly_one_item_however_large_the_stack_offered_was() {
        let mut carrier = Carrier::new();
        let mut many = ready("counterpart:device");
        many.count = 64;
        carrier.insert(0, many).expect("the carrier takes it");

        assert_eq!(carrier.slots()[0].as_ref().expect("it is there").count, 1);
    }

    #[test]
    fn a_simulated_extraction_leaves_the_slot_and_a_real_one_clears_it() {
        let mut carrier = Carrier::new();
        carrier
            .insert(2, ready("counterpart:device"))
            .expect("the carrier takes it");

        assert!(carrier.extract(2, true).is_some());
        assert!(carrier.slots()[2].is_some());

        assert!(carrier.extract(2, false).is_some());
        assert!(carrier.slots()[2].is_none());
        assert!(carrier.extract(2, false).is_none());
    }

    #[test]
    fn cycling_stops_at_the_first_occupied_slot_in_the_direction_asked_for() {
        let mut carrier = Carrier::new();
        carrier
            .insert(3, ready("counterpart:device"))
            .expect("the carrier takes it");

        assert!(carrier.cycle(Step::Forward));
        assert_eq!(carrier.selected(), 3);

        let mut backward = Carrier::new();
        backward
            .insert(4, ready("counterpart:device"))
            .expect("the carrier takes it");
        assert!(backward.cycle(Step::Backward));
        assert_eq!(backward.selected(), 4);
    }

    #[test]
    fn one_occupied_slot_is_selected_even_when_it_is_the_one_already_selected() {
        let mut carrier = Carrier::new();
        carrier
            .insert(0, ready("counterpart:device"))
            .expect("the carrier takes it");

        assert!(
            !carrier.cycle(Step::Forward),
            "the walk comes back to it, so the selection did not move"
        );
        assert_eq!(carrier.selected(), 0);
        assert!(carrier.selection().is_some());
    }

    #[test]
    fn an_empty_carrier_leaves_its_selection_where_it_was() {
        let mut carrier = Carrier::new();
        assert!(!carrier.cycle(Step::Forward));
        assert!(!carrier.cycle(Step::Backward));
        assert_eq!(carrier.selected(), 0);
    }

    #[test]
    fn crouching_always_opens_the_carrier_and_otherwise_the_selection_decides() {
        let mut carrier = Carrier::new();
        assert_eq!(carrier.use_item(true), Opened::Carrier);
        assert_eq!(carrier.use_item(false), Opened::Carrier);

        carrier
            .insert(0, ready("counterpart:device"))
            .expect("the carrier takes it");
        assert_eq!(carrier.use_item(false), Opened::Selected { slot: 0 });
        assert_eq!(carrier.use_item(true), Opened::Carrier);
    }

    #[test]
    fn the_replacement_is_written_into_the_hand_the_interaction_came_from() {
        let mut carrier = Carrier::new();
        let mut device = ready("counterpart:device");
        device
            .components
            .insert(FORM_COMPONENT.to_owned(), "counterpart:filled".to_owned());
        carrier.insert(0, device).expect("the carrier takes it");

        assert_eq!(
            carrier
                .replacement(Hand::Off)
                .expect("the slot is supported"),
            Replacement {
                presents: "counterpart:filled".to_owned(),
                into: Hand::Off,
            }
        );
    }

    #[test]
    fn an_empty_selection_presents_the_plain_carrier() {
        let carrier = Carrier::new();
        assert_eq!(
            carrier
                .replacement(Hand::Main)
                .expect("nothing selected is supported")
                .presents,
            PLAIN_FORM
        );
    }

    #[test]
    fn a_record_that_smuggled_in_an_unsupported_item_refuses_at_the_swap() {
        let smuggled = ravel_sdk::json!({
            "slots": [
                Stack::of("counterpart:plain").expect("a legal identity").to_saved(),
                null, null, null, null, null
            ],
            "selected": 0,
        });
        let carrier = Carrier::from_saved(&smuggled).expect("the record itself is well formed");

        assert_eq!(
            carrier.replacement(Hand::Main),
            Err(CarrierRefused::Unsupported),
            "the swap tests the slot rather than casting to the declaration"
        );
    }

    #[test]
    fn a_carrier_round_trips_and_a_hostile_selection_is_refused() {
        let mut carrier = Carrier::new();
        carrier
            .insert(4, ready("counterpart:device"))
            .expect("the carrier takes it");
        carrier.cycle(Step::Forward);

        let read = Carrier::from_saved(&carrier.to_saved()).expect("what was written reads back");
        assert_eq!(read, carrier);

        for selected in [
            ravel_sdk::json!(u64::MAX),
            ravel_sdk::json!(SLOTS),
            ravel_sdk::json!(-1),
            ravel_sdk::json!("0"),
            ravel_sdk::json!(null),
        ] {
            let hostile = ravel_sdk::json!({
                "slots": [null, null, null, null, null, null],
                "selected": selected,
            });
            assert!(Carrier::from_saved(&hostile).is_err());
        }
    }

    #[test]
    fn a_slot_list_that_is_not_six_long_is_refused() {
        let short = ravel_sdk::json!({ "slots": [null, null], "selected": 0 });
        assert_eq!(
            Carrier::from_saved(&short),
            Err(CarrierRefused::Slot { index: 2 })
        );
    }
}
