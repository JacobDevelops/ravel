//! The recipe search of §6: four pieces of state and the grace period they make.
//!
//! A processing block drives this over one recipe type. It holds a flag saying
//! the search is stuck, a flag saying the inputs changed since the last search, a
//! remembered last successful recipe and a current one.
//!
//! The net effect is a one-shot grace period. The first failed search after an
//! input change does not park the block and the second does, which is what keeps
//! a machine from parking on a transient empty inventory. That is reproduced
//! exactly, including the one part that reads like an oversight and is not: the
//! fast path, where the remembered recipe still matches, returns **without
//! touching the changed flag**.
//!
//! # The change declared here
//!
//! **A level that is not there is not a searched-and-empty recipe list.** The
//! original's scan yields no match when its level supplier yields nothing, and
//! the no-match path then sets the stuck flag as if every recipe had been
//! examined. A block whose level is momentarily absent parks. [`Search::run`]
//! answers [`Outcome::CouldNotSearch`] and leaves all four pieces of state
//! exactly as they were.
//!
//! # A disclosed ceiling that is the host's rather than this package's
//!
//! The scan is linear in the number of recipes of the type and runs on every
//! cache miss; nothing is indexed by input. [`Catalogue`] is the seam, so a
//! dependent that keeps an input-keyed index implements it over that index and
//! changes only how long a miss takes. None of the flag behaviour here depends on
//! which it is.

use ravel_sdk::content::ContentId;

/// The recipes of one type that a level holds.
///
/// A dependent implements this over whatever it can reach. The scan is the
/// implementation's; the flags are this module's.
pub trait Catalogue {
    /// Returns whether a remembered recipe still matches the current inputs.
    fn still_matches(&self, recipe: &ContentId) -> bool;

    /// Returns the first recipe of the declared type that matches, and absent
    /// when none does.
    fn first_match(&self) -> Option<ContentId>;
}

/// What one search did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// The remembered recipe still matched and became the current one. The fast
    /// path.
    Remembered,
    /// A scan ran and matched.
    Matched,
    /// A scan ran and found nothing.
    Missed,
    /// There was no level to search, so nothing was concluded.
    CouldNotSearch,
}

/// The state a processing block drives.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Search {
    stuck: bool,
    changed: bool,
    remembered: Option<ContentId>,
    current: Option<ContentId>,
}

impl Search {
    /// A block that has searched for nothing yet.
    #[must_use]
    pub const fn idle() -> Self {
        Self {
            stuck: false,
            changed: false,
            remembered: None,
            current: None,
        }
    }

    /// Returns whether the search is parked.
    #[must_use]
    pub const fn is_stuck(&self) -> bool {
        self.stuck
    }

    /// Returns whether the inputs changed since the last conclusive search.
    #[must_use]
    pub const fn inputs_changed(&self) -> bool {
        self.changed
    }

    /// Returns the recipe being worked, if any.
    #[must_use]
    pub const fn current(&self) -> Option<&ContentId> {
        self.current.as_ref()
    }

    /// Returns the last recipe that matched, if any.
    #[must_use]
    pub const fn remembered(&self) -> Option<&ContentId> {
        self.remembered.as_ref()
    }

    /// Records that the inputs changed: the block is no longer parked and the
    /// next failed search is the one it is forgiven.
    pub const fn changed(&mut self) {
        self.stuck = false;
        self.changed = true;
    }

    /// Returns whether the block should tick.
    #[must_use]
    pub const fn should_tick(&self) -> bool {
        self.current.is_some() || !self.stuck
    }

    /// Runs one search against the level's catalogue, or against no level.
    pub fn run(&mut self, level: Option<&dyn Catalogue>) -> Outcome {
        let Some(catalogue) = level else {
            return Outcome::CouldNotSearch;
        };
        if let Some(remembered) = self.remembered.clone()
            && catalogue.still_matches(&remembered)
        {
            self.current = Some(remembered);
            self.stuck = false;
            return Outcome::Remembered;
        }
        self.remembered = None;
        self.stuck = false;
        if let Some(found) = catalogue.first_match() {
            self.changed = false;
            self.remembered = Some(found.clone());
            self.current = Some(found);
            self.stuck = false;
            Outcome::Matched
        } else {
            if self.changed {
                self.changed = false;
            } else {
                self.stuck = true;
                self.current = None;
            }
            Outcome::Missed
        }
    }

    /// Restores the two remembered identifiers a world held.
    ///
    /// The flags are not saved and are not restored: a loaded block has not
    /// searched yet, so it is neither stuck nor forgiven.
    pub fn restore(&mut self, current: Option<ContentId>, remembered: Option<ContentId>) {
        self.current = current;
        self.remembered = remembered;
        self.stuck = false;
        self.changed = false;
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::content::ContentId;

    use super::{Catalogue, Outcome, Search};

    fn recipe(path: &str) -> ContentId {
        ContentId::new(
            "rvl_146_counterpart"
                .parse()
                .expect("the declared identity is a legal mod id"),
            path,
        )
        .expect("a legal content path")
    }

    /// A catalogue a test drives: what matches, and whether the remembered one
    /// still does.
    struct Fixed {
        matches: Option<ContentId>,
        remembered_still_matches: bool,
    }

    impl Catalogue for Fixed {
        fn still_matches(&self, recipe: &ContentId) -> bool {
            let _ = recipe;
            self.remembered_still_matches
        }

        fn first_match(&self) -> Option<ContentId> {
            self.matches.clone()
        }
    }

    fn holding(path: &str) -> Fixed {
        Fixed {
            matches: Some(recipe(path)),
            remembered_still_matches: true,
        }
    }

    fn empty() -> Fixed {
        Fixed {
            matches: None,
            remembered_still_matches: false,
        }
    }

    #[test]
    fn a_scan_that_matches_remembers_the_recipe_and_clears_both_flags() {
        let mut search = Search::idle();
        search.changed();
        assert_eq!(search.run(Some(&holding("smelting"))), Outcome::Matched);
        assert_eq!(search.current(), Some(&recipe("smelting")));
        assert_eq!(search.remembered(), Some(&recipe("smelting")));
        assert!(!search.is_stuck());
        assert!(!search.inputs_changed());
    }

    #[test]
    fn the_fast_path_leaves_the_changed_flag_exactly_where_it_was() {
        let mut search = Search::idle();
        assert_eq!(search.run(Some(&holding("smelting"))), Outcome::Matched);
        search.changed();
        assert!(search.inputs_changed());
        assert_eq!(search.run(Some(&holding("smelting"))), Outcome::Remembered);
        assert!(
            search.inputs_changed(),
            "the fast path returns without touching the changed flag"
        );
        assert!(!search.is_stuck());
    }

    #[test]
    fn the_first_failed_search_after_an_input_change_does_not_park_the_block() {
        let mut search = Search::idle();
        assert_eq!(search.run(Some(&holding("smelting"))), Outcome::Matched);
        search.changed();
        assert_eq!(search.run(Some(&empty())), Outcome::Missed);
        assert!(!search.is_stuck(), "the first miss is forgiven");
        assert!(!search.inputs_changed());
        assert_eq!(
            search.current(),
            Some(&recipe("smelting")),
            "the forgiven miss leaves the current recipe alone"
        );
        assert!(search.should_tick());
    }

    #[test]
    fn the_second_failed_search_parks_the_block_and_clears_the_current_recipe() {
        let mut search = Search::idle();
        assert_eq!(search.run(Some(&holding("smelting"))), Outcome::Matched);
        search.changed();
        assert_eq!(search.run(Some(&empty())), Outcome::Missed);
        assert_eq!(search.run(Some(&empty())), Outcome::Missed);
        assert!(search.is_stuck());
        assert_eq!(search.current(), None);
        assert_eq!(search.remembered(), None);
        assert!(!search.should_tick());
    }

    #[test]
    fn an_input_change_unparks_the_block_without_searching() {
        let mut search = Search::idle();
        assert_eq!(search.run(Some(&empty())), Outcome::Missed);
        assert!(search.is_stuck());
        search.changed();
        assert!(!search.is_stuck());
        assert!(search.should_tick());
    }

    #[test]
    fn no_level_is_not_a_searched_and_empty_recipe_list() {
        let mut search = Search::idle();
        assert_eq!(search.run(Some(&holding("smelting"))), Outcome::Matched);
        search.changed();
        let before = search.clone();
        assert_eq!(search.run(None), Outcome::CouldNotSearch);
        assert_eq!(search, before, "nothing was concluded, so nothing moved");
        assert!(!search.is_stuck());
    }

    #[test]
    fn a_remembered_recipe_that_no_longer_matches_falls_through_to_a_scan() {
        let mut search = Search::idle();
        assert_eq!(search.run(Some(&holding("smelting"))), Outcome::Matched);
        let stale = Fixed {
            matches: Some(recipe("blasting")),
            remembered_still_matches: false,
        };
        assert_eq!(search.run(Some(&stale)), Outcome::Matched);
        assert_eq!(search.current(), Some(&recipe("blasting")));
        assert_eq!(search.remembered(), Some(&recipe("blasting")));
    }

    #[test]
    fn a_block_that_has_never_searched_ticks() {
        let search = Search::idle();
        assert!(search.should_tick());
        assert_eq!(search.current(), None);
    }

    #[test]
    fn a_restored_block_holds_its_recipes_and_neither_flag() {
        let mut search = Search::idle();
        search.restore(Some(recipe("smelting")), Some(recipe("blasting")));
        assert_eq!(search.current(), Some(&recipe("smelting")));
        assert_eq!(search.remembered(), Some(&recipe("blasting")));
        assert!(!search.is_stuck());
        assert!(!search.inputs_changed());
    }
}
