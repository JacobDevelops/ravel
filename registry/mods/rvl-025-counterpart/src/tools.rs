//! The configuration tool of §11: two item registrations that are one tool with
//! two modes.
//!
//! Using the tool switches modes by replacing the item with the other
//! registration and carrying the stored position across, so the mode is a
//! property of which item is held rather than a flag inside it. That is why the
//! second registration has no recipe and why both carry the shared tool tag.
//!
//! §11 records a gap this counterpart cannot close on its own: nothing in this
//! entry ever *sets* the stored position. The setter is published for dependents
//! and every write comes from outside, so [`Tool::select`] exists and no path in
//! this package calls it. `NOTES.md` says so, and the tests here drive it the way
//! a dependent would.

use crate::data::Placement;

/// The item the tool is registered as in its first mode.
pub const PLAIN_FORM: &str = "configuration_tool";

/// The item it is registered as in its second mode.
pub const SELECTING_FORM: &str = "selecting_configuration_tool";

/// The tag both registrations carry.
pub const TOOL_TAG: &str = "rvl_025_counterpart:configuration_tools";

/// Which of the two modes the held tool is in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    /// Does nothing itself: it exists so a crouch-use activates a block.
    Plain,
    /// Stores and applies a selection.
    Selecting,
}

impl Mode {
    /// Returns the registration this mode is held as.
    #[must_use]
    pub const fn form(self) -> &'static str {
        match self {
            Self::Plain => PLAIN_FORM,
            Self::Selecting => SELECTING_FORM,
        }
    }

    /// Returns the mode using the tool switches to.
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Plain => Self::Selecting,
            Self::Selecting => Self::Plain,
        }
    }

    /// Returns whether this mode's registration has a recipe.
    ///
    /// The second does not: it is reached by using the tool rather than by
    /// crafting, which is one of §5's two recipe-less items.
    #[must_use]
    pub const fn has_recipe(self) -> bool {
        matches!(self, Self::Plain)
    }
}

/// What a use of the tool did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Used {
    /// The mode was switched and the stored position carried across.
    Switched {
        /// The mode now in force.
        into: Mode,
        /// What the tool still stores.
        carried: Option<Placement>,
    },
    /// A block from this family was activated, and nothing was placed.
    Activated,
    /// The tool refused because the stored position is in another dimension.
    WrongDimension {
        /// Where the stored position is.
        stored: String,
        /// Where the player is.
        player: String,
    },
    /// The stored position's block was handed the clicked position.
    Applied {
        /// Which position it was handed.
        clicked: (i32, i32, i32),
    },
    /// Nothing happened.
    Nothing,
}

/// The tool, as far as its rules go.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Tool {
    stored: Option<Placement>,
}

impl Tool {
    /// Builds a tool storing nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self { stored: None }
    }

    /// Returns the stored position.
    #[must_use]
    pub const fn stored(&self) -> Option<&Placement> {
        self.stored.as_ref()
    }

    /// Stores a position.
    ///
    /// Published for dependents. Nothing in this entry calls it, which is §11's
    /// recorded gap rather than an oversight here.
    pub fn select(&mut self, at: Placement) {
        self.stored = Some(at);
    }

    /// Clears the stored position by removing the record rather than storing a
    /// sentinel, which is what §11 states.
    pub fn clear(&mut self) {
        self.stored = None;
    }

    /// Switches the mode, carrying the stored position across.
    #[must_use]
    pub fn switch(&self, from: Mode) -> Used {
        Used::Switched {
            into: from.other(),
            carried: self.stored.clone(),
        }
    }

    /// Runs a use of the tool on a block.
    ///
    /// The order is §11's. A crouch-use on a block from this family activates
    /// that block in both modes and takes precedence over selection. In the
    /// second mode only, a stored position in a different dimension refuses and
    /// says so, and otherwise a block at the stored position that accepts
    /// selections is handed the clicked position.
    #[must_use]
    pub fn use_on(
        &self,
        mode: Mode,
        crouching: bool,
        on_family_block: bool,
        player_dimension: &str,
        clicked: (i32, i32, i32),
        stored_accepts: bool,
    ) -> Used {
        if crouching && on_family_block {
            return Used::Activated;
        }
        if mode == Mode::Plain {
            return Used::Nothing;
        }
        let Some(stored) = self.stored.as_ref() else {
            return Used::Nothing;
        };
        if stored.dimension != player_dimension {
            return Used::WrongDimension {
                stored: stored.dimension.clone(),
                player: player_dimension.to_owned(),
            };
        }
        if !stored_accepts {
            return Used::Nothing;
        }
        Used::Applied { clicked }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(dimension: &str) -> Placement {
        Placement::new(dimension, (4, 5, 6)).expect("a legal dimension")
    }

    #[test]
    fn the_two_modes_are_two_registrations_and_only_the_first_has_a_recipe() {
        assert_eq!(Mode::Plain.form(), PLAIN_FORM);
        assert_eq!(Mode::Selecting.form(), SELECTING_FORM);
        assert_ne!(Mode::Plain.form(), Mode::Selecting.form());
        assert!(Mode::Plain.has_recipe());
        assert!(!Mode::Selecting.has_recipe());
    }

    #[test]
    fn switching_carries_the_stored_position_across_the_replacement() {
        let mut tool = Tool::new();
        tool.select(placement("counterpart:deep"));

        assert_eq!(
            tool.switch(Mode::Plain),
            Used::Switched {
                into: Mode::Selecting,
                carried: Some(placement("counterpart:deep")),
            }
        );
        assert_eq!(
            tool.switch(Mode::Selecting),
            Used::Switched {
                into: Mode::Plain,
                carried: Some(placement("counterpart:deep")),
            }
        );
    }

    #[test]
    fn clearing_removes_the_record_rather_than_storing_a_sentinel() {
        let mut tool = Tool::new();
        tool.select(placement("counterpart:deep"));
        assert!(tool.stored().is_some());

        tool.clear();
        assert_eq!(tool.stored(), None);
    }

    #[test]
    fn a_crouch_use_on_a_family_block_activates_it_in_both_modes() {
        let mut tool = Tool::new();
        tool.select(placement("counterpart:deep"));

        for mode in [Mode::Plain, Mode::Selecting] {
            assert_eq!(
                tool.use_on(mode, true, true, "counterpart:deep", (0, 0, 0), true),
                Used::Activated,
                "activation takes precedence over selection"
            );
        }
    }

    #[test]
    fn the_first_mode_does_nothing_of_its_own() {
        let mut tool = Tool::new();
        tool.select(placement("counterpart:deep"));
        assert_eq!(
            tool.use_on(
                Mode::Plain,
                false,
                false,
                "counterpart:deep",
                (1, 2, 3),
                true
            ),
            Used::Nothing
        );
    }

    #[test]
    fn a_stored_position_in_another_dimension_refuses_and_says_both_names() {
        let mut tool = Tool::new();
        tool.select(placement("counterpart:deep"));

        assert_eq!(
            tool.use_on(
                Mode::Selecting,
                false,
                false,
                "counterpart:high",
                (1, 2, 3),
                true
            ),
            Used::WrongDimension {
                stored: "counterpart:deep".to_owned(),
                player: "counterpart:high".to_owned(),
            }
        );
    }

    #[test]
    fn a_stored_block_that_accepts_selections_is_handed_the_clicked_position() {
        let mut tool = Tool::new();
        tool.select(placement("counterpart:deep"));

        assert_eq!(
            tool.use_on(
                Mode::Selecting,
                false,
                false,
                "counterpart:deep",
                (7, 8, 9),
                true
            ),
            Used::Applied { clicked: (7, 8, 9) }
        );
        assert_eq!(
            tool.use_on(
                Mode::Selecting,
                false,
                false,
                "counterpart:deep",
                (7, 8, 9),
                false
            ),
            Used::Nothing
        );
    }

    #[test]
    fn a_tool_that_stores_nothing_does_nothing_in_the_second_mode() {
        let tool = Tool::new();
        assert_eq!(
            tool.use_on(
                Mode::Selecting,
                false,
                false,
                "counterpart:deep",
                (1, 2, 3),
                true
            ),
            Used::Nothing
        );
    }

    #[test]
    fn nothing_in_this_package_sets_the_stored_position() {
        // §11's recorded gap, held to an assertion rather than left in prose: a
        // tool this package built and drove is still storing nothing, and only a
        // dependent's call to `select` changes that.
        let tool = Tool::new();
        assert_eq!(
            tool.use_on(
                Mode::Selecting,
                false,
                true,
                "counterpart:deep",
                (0, 0, 0),
                true,
            ),
            Used::Nothing
        );
        assert_eq!(
            tool.switch(Mode::Selecting),
            Used::Switched {
                into: Mode::Plain,
                carried: None,
            }
        );
        assert_eq!(tool.stored(), None);
    }
}
