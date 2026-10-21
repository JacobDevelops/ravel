//! The pattern card of §9: a five-by-four grid, its evaluation, and the three
//! ways a dependent reads one back.
//!
//! The grid is not three by three. Twenty input positions are stored plus one
//! output position after them, and a three-by-three recipe occupies the first
//! three positions of each of the first three rows, leaving eleven input
//! positions empty. That layout is the contract a dependent reads against, so it
//! is stated as constants here rather than left implicit in an index.

use crate::data::{Record, RecordRefused, Stack, read_tolerant, write_tolerant};
use ravel_sdk::JsonValue;

/// How many positions one row of the grid holds.
pub const WIDTH: usize = 5;
/// How many rows the grid holds.
pub const HEIGHT: usize = 4;
/// How many input positions there are.
pub const INPUTS: usize = WIDTH * HEIGHT;
/// Where the evaluated result is stored.
pub const OUTPUT: usize = INPUTS;
/// How many positions the stored list holds altogether.
pub const POSITIONS: usize = INPUTS + 1;
/// How wide the sub-grid a plain recipe occupies is.
pub const SMALL: usize = 3;
/// How many cards stack together in one slot.
pub const STACK_LIMIT: u8 = 8;

/// The nine input positions inside the sub-grid, in row-major order.
///
/// A plain recipe fills the first three positions of each of the first three
/// rows, which on a five-wide grid is not a contiguous run.
#[must_use]
pub fn small_grid_positions() -> [usize; SMALL * SMALL] {
    let mut positions = [0_usize; SMALL * SMALL];
    let mut index = 0;
    let mut row = 0;
    while row < SMALL {
        let mut column = 0;
        while column < SMALL {
            positions[index] = row * WIDTH + column;
            index += 1;
            column += 1;
        }
        row += 1;
    }
    positions
}

/// One shaped recipe, as the scan of §9 compares against.
///
/// What a recipe is belongs to the host; what the evaluation needs is the shape
/// it demands and the stack it assembles, so that is what this carries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShapedRecipe {
    /// The nine positions, each naming an identity or standing empty.
    pub demands: [Option<ravel_sdk::content::ContentId>; SMALL * SMALL],
    /// What a match assembles.
    pub result: Stack,
}

impl ShapedRecipe {
    /// Answers whether a three-by-three input satisfies this recipe.
    fn accepts(&self, input: &[Option<Stack>; SMALL * SMALL]) -> bool {
        self.demands
            .iter()
            .zip(input)
            .all(|(demand, offered)| match (demand, offered) {
                (None, None) => true,
                (None, Some(stack)) => stack.is_empty(),
                (Some(_), None) => false,
                (Some(identity), Some(stack)) => !stack.is_empty() && &stack.identity == identity,
            })
    }
}

/// A stored crafting pattern and the result it produces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pattern {
    positions: Vec<Option<Stack>>,
}

impl Default for Pattern {
    fn default() -> Self {
        Self::new()
    }
}

impl Pattern {
    /// Builds an empty card: twenty-one positions, none of them filled.
    #[must_use]
    pub fn new() -> Self {
        Self {
            positions: vec![None; POSITIONS],
        }
    }

    /// Returns every stored position, inputs first and the output last.
    #[must_use]
    pub fn positions(&self) -> &[Option<Stack>] {
        &self.positions
    }

    /// Writes one input position, refusing an index outside the inputs.
    ///
    /// The output position is not writable through this. §9 says it is filled by
    /// evaluating the pattern and never by the player, and a setter that took
    /// index twenty would be the route that makes that untrue.
    ///
    /// # Errors
    ///
    /// Returns [`PatternRefused::Position`] for an index at or past
    /// [`OUTPUT`].
    pub fn set_input(&mut self, index: usize, stack: Option<Stack>) -> Result<(), PatternRefused> {
        if index >= INPUTS {
            return Err(PatternRefused::Position { index });
        }
        self.positions[index] = stack;
        Ok(())
    }

    /// Returns the evaluated output position.
    #[must_use]
    pub fn output(&self) -> Option<&Stack> {
        self.positions[OUTPUT].as_ref()
    }

    /// Evaluates the pattern against every registered recipe.
    ///
    /// The first three positions of each of the first three rows are copied into
    /// a three-by-three input in row-major order, the recipes are scanned in the
    /// order they are given, and the first match's assembled result is written
    /// to the output position. No match writes an empty output.
    ///
    /// Known ceiling: the scan is a linear pass over the whole recipe set on
    /// every evaluation, with no index and no cache, which is what the original
    /// does; the upgrade path is an index keyed on the sub-grid's first filled
    /// position rather than a shorter scan.
    pub fn evaluate(&mut self, recipes: &[ShapedRecipe]) -> bool {
        let input = self.small_grid_stacks();
        let matched = recipes.iter().find(|recipe| recipe.accepts(&input));
        self.positions[OUTPUT] = matched.map(|recipe| recipe.result.clone());
        matched.is_some()
    }

    /// Returns whether every filled input position lies inside the sub-grid.
    ///
    /// This is the intended test §9 says the original disables. The original
    /// demands that *every* stored position be inside the sub-grid, and since
    /// the stored list is always twenty-one long, the fourth position is outside
    /// by construction and the predicate is false for every card ever written.
    /// The predicate here reads the twenty input positions and asks the question
    /// of the filled ones only; the output position is not an input and is not
    /// consulted. A parity test against the original disagrees here on purpose.
    #[must_use]
    pub fn fits_small_grid(&self) -> bool {
        let inside = small_grid_positions();
        self.positions
            .iter()
            .take(INPUTS)
            .enumerate()
            .all(|(index, slot)| {
                slot.as_ref()
                    .is_none_or(|stack| stack.is_empty() || inside.contains(&index))
            })
    }

    /// Returns all twenty input positions as ingredients.
    ///
    /// The first of §9's three readbacks: what a dependent that wants the whole
    /// pattern asks for.
    #[must_use]
    pub fn all_inputs(&self) -> Vec<Option<&Stack>> {
        self.positions
            .iter()
            .take(INPUTS)
            .map(Option::as_ref)
            .collect()
    }

    /// Returns the nine sub-grid positions as ingredients.
    ///
    /// The second readback: what a dependent that wants a plain-shaped recipe
    /// asks for. It is not the same answer as [`Pattern::all_inputs`], and §9 is
    /// explicit that the three are not equivalent.
    #[must_use]
    pub fn small_grid_ingredients(&self) -> Vec<Option<&Stack>> {
        small_grid_positions()
            .into_iter()
            .map(|index| self.positions[index].as_ref())
            .collect()
    }

    /// Returns the nine sub-grid positions as raw stacks.
    ///
    /// The third readback, and the one evaluation itself uses.
    #[must_use]
    pub fn small_grid_stacks(&self) -> [Option<Stack>; SMALL * SMALL] {
        let positions = small_grid_positions();
        std::array::from_fn(|index| self.positions[positions[index]].clone())
    }

    /// Writes the card as its persisted document.
    #[must_use]
    pub fn to_saved(&self) -> JsonValue {
        write_tolerant(&self.positions)
    }

    /// Reads a card back, keeping its empty positions.
    ///
    /// # Errors
    ///
    /// Returns [`PatternRefused::Record`] for a document that is not a list of
    /// stacks, and [`PatternRefused::Length`] for a list that is not exactly
    /// [`POSITIONS`] long, because a shorter one would silently renumber every
    /// position after the gap and a longer one would carry a position no reader
    /// has an index for.
    pub fn from_saved(document: &JsonValue) -> Result<Self, PatternRefused> {
        let positions = read_tolerant(Record::PatternContents, document, POSITIONS)
            .map_err(PatternRefused::Record)?;
        if positions.len() != POSITIONS {
            return Err(PatternRefused::Length {
                found: positions.len(),
            });
        }
        Ok(Self { positions })
    }
}

/// The hotbar slot a card's own screen holds still while it is open.
///
/// §9's menu quirk: a player cannot take the card out from under its own screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MenuLock {
    /// The hotbar index the open card came from.
    pub held_at: usize,
}

impl MenuLock {
    /// Answers whether the player may take the item in `index` out.
    #[must_use]
    pub const fn may_remove(&self, index: usize) -> bool {
        index != self.held_at
    }
}

/// Why a card refused something.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatternRefused {
    /// A write named a position that is not an input.
    Position {
        /// The index that was named.
        index: usize,
    },
    /// A persisted card is not exactly [`POSITIONS`] long.
    Length {
        /// How many the document carried.
        found: usize,
    },
    /// The persisted form is not one a record may carry.
    Record(RecordRefused),
}

impl std::fmt::Display for PatternRefused {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Position { index } => {
                write!(formatter, "position {index} is not one a player writes")
            }
            Self::Length { found } => write!(
                formatter,
                "a stored card carries {found} positions and every card carries {POSITIONS}"
            ),
            Self::Record(refused) => write!(formatter, "{refused}"),
        }
    }
}

impl std::error::Error for PatternRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Record(refused) => Some(refused),
            Self::Position { .. } | Self::Length { .. } => None,
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
        let refused = PatternRefused::Record(record.clone());
        assert_eq!(
            refused
                .source()
                .and_then(|source| source.downcast_ref::<RecordRefused>()),
            Some(&record)
        );
        assert!(PatternRefused::Position { index: 0 }.source().is_none());
        assert!(PatternRefused::Length { found: 0 }.source().is_none());
    }

    fn stack(identity: &str) -> Stack {
        Stack::of(identity).expect("a legal identity")
    }

    fn plank_recipe() -> ShapedRecipe {
        ShapedRecipe {
            demands: [
                Some("counterpart:beam".parse().expect("a legal identity")),
                Some("counterpart:beam".parse().expect("a legal identity")),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ],
            result: stack("counterpart:plate"),
        }
    }

    fn filled_card() -> Pattern {
        let mut card = Pattern::new();
        card.set_input(0, Some(stack("counterpart:beam")))
            .expect("position zero is an input");
        card.set_input(1, Some(stack("counterpart:beam")))
            .expect("position one is an input");
        card
    }

    #[test]
    fn the_grid_is_five_by_four_with_the_output_after_the_twenty_inputs() {
        assert_eq!(Pattern::new().positions().len(), POSITIONS);
    }

    #[test]
    fn the_sub_grid_is_the_first_three_of_each_of_the_first_three_rows() {
        assert_eq!(small_grid_positions(), [0, 1, 2, 5, 6, 7, 10, 11, 12]);
    }

    #[test]
    fn a_player_cannot_write_the_output_position() {
        let mut card = Pattern::new();
        assert_eq!(
            card.set_input(OUTPUT, Some(stack("counterpart:plate"))),
            Err(PatternRefused::Position { index: OUTPUT })
        );
        assert!(card.output().is_none());
    }

    #[test]
    fn evaluation_writes_the_first_matching_recipe_and_nothing_when_none_match() {
        let mut card = filled_card();
        let other = ShapedRecipe {
            result: stack("counterpart:second_plate"),
            ..plank_recipe()
        };
        let recipes = [plank_recipe(), other];

        assert!(card.evaluate(&recipes));
        assert_eq!(
            card.output()
                .expect("a match writes the output")
                .identity
                .to_string(),
            "counterpart:plate",
            "the scan stops at the first match in the order it was given"
        );

        let mut empty = Pattern::new();
        assert!(!empty.evaluate(&recipes));
        assert!(empty.output().is_none());
    }

    #[test]
    fn evaluating_a_card_that_no_longer_matches_clears_the_output_it_had() {
        let mut card = filled_card();
        assert!(card.evaluate(&[plank_recipe()]));
        card.set_input(1, None).expect("position one is an input");

        assert!(!card.evaluate(&[plank_recipe()]));
        assert!(card.output().is_none());
    }

    #[test]
    fn the_sub_grid_predicate_answers_for_a_card_the_original_always_refuses() {
        let card = filled_card();
        assert!(
            card.fits_small_grid(),
            "the corrected test asks the question of the filled positions only"
        );

        let mut outside = filled_card();
        outside
            .set_input(3, Some(stack("counterpart:beam")))
            .expect("position three is an input");
        assert!(!outside.fits_small_grid());
    }

    #[test]
    fn an_evaluated_output_does_not_make_a_card_stop_fitting_the_sub_grid() {
        let mut card = filled_card();
        card.evaluate(&[plank_recipe()]);
        assert!(card.output().is_some());
        assert!(card.fits_small_grid());
    }

    #[test]
    fn the_three_readbacks_answer_differently_about_the_same_card() {
        let mut card = filled_card();
        card.set_input(3, Some(stack("counterpart:beam")))
            .expect("position three is an input");

        assert_eq!(card.all_inputs().len(), INPUTS);
        assert_eq!(card.small_grid_ingredients().len(), SMALL * SMALL);
        assert!(
            card.all_inputs()[3].is_some(),
            "the whole-pattern readback sees position three"
        );
        assert!(
            card.small_grid_ingredients().iter().flatten().count() == 2,
            "the sub-grid readbacks do not, which is why the three are not equivalent"
        );
    }

    #[test]
    fn a_card_round_trips_and_a_short_or_long_document_is_refused() {
        let mut card = filled_card();
        card.evaluate(&[plank_recipe()]);
        let read = Pattern::from_saved(&card.to_saved()).expect("what was written reads back");
        assert_eq!(read, card);

        let short = JsonValue::Array(vec![JsonValue::Null; POSITIONS - 1]);
        assert_eq!(
            Pattern::from_saved(&short),
            Err(PatternRefused::Length {
                found: POSITIONS - 1
            })
        );

        let long = JsonValue::Array(vec![JsonValue::Null; POSITIONS + 1]);
        assert!(matches!(
            Pattern::from_saved(&long),
            Err(PatternRefused::Record(RecordRefused::TooMany { .. }))
        ));
    }

    #[test]
    fn the_slot_a_card_came_from_is_held_still_while_its_screen_is_open() {
        let lock = MenuLock { held_at: 4 };
        assert!(!lock.may_remove(4));
        assert!(lock.may_remove(3));
    }
}
