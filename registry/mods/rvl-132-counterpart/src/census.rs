//! What the pinned entry counts, carried as data.
//!
//! Sections one, thirteen and sixteen state counts, and every one of them is a
//! fact about the original that a reader of this package checks against. They
//! are here rather than in prose so that a count and the arithmetic behind it
//! fail a test rather than drift, which is what happened to the numbers section
//! sixteen was asked to recount.
//!
//! Section 13.3's blocked set is here for the same reason. Four values and one
//! placement are blocked, none is stubbed anywhere in this package, and
//! `NOTES.md` says the same thing in prose.

/// How many portable source files the pinned artifact is built from.
pub const SHIPPED_SOURCES: usize = 52;

/// How many of those are the in-game test harness's own classes.
pub const HARNESS_SOURCES: usize = 6;

/// How many unit-test files sit beside them, unshipped.
pub const UNIT_TESTS: usize = 35;

/// How many one-file companion mods the harness drives, unshipped.
pub const COMPANION_MODS: usize = 6;

/// How many translation documents ship.
pub const TRANSLATIONS: usize = 30;

/// How many structure templates ship, which the harness uses and which is empty.
pub const TEMPLATES: usize = 1;

/// How many logo images ship, which are branding and were not opened.
pub const LOGOS: usize = 2;

/// How many resource documents ship in total.
///
/// The three counts above are the whole of it; the total follows their sum.
pub const SHIPPED_RESOURCES: usize = TRANSLATIONS + TEMPLATES + LOGOS;

/// How many access-widening documents ship.
pub const WIDENING_DOCUMENTS: usize = 1;

/// The access-widening document's entries, as section 13.1 groups them.
///
/// Three on the brewing tables and their entry type, one on a tag class's
/// backing array, six across the smelting stations, two on the brewing station,
/// and two on the carrying pouch plus one on its contents.
pub const WIDENING_GROUPS: [(&str, usize); 5] = [
    ("brewing tables and their entry type", 3),
    ("a tag class's backing array", 1),
    ("the smelting stations", 6),
    ("the brewing station", 2),
    ("the carrying pouch and its contents", 3),
];

/// How many entries the access-widening document holds.
pub const WIDENING_ENTRIES: usize = 15;

/// The four values section 13.3 blocks.
///
/// Each sits behind the access widening of section 13.1, which Ravel does not
/// do. Each is an absence in this package rather than a stub: the rule that
/// reads the value is implemented and tested, and the reading answers nothing
/// until a Ravel hook supplies it.
pub const BLOCKED_VALUES: [&str; 4] = [
    "the raw burn timer behind the smelting heat reading",
    "the brewing station's fuel count",
    "the brewing station's brewable test",
    "the carrying pouch's weight arithmetic",
];

/// The one placement section 13.3 blocks outright.
///
/// One shipped source file is declared in a package under the game's own root
/// namespace. Section 13.2 records that the placement obtains nothing: it is not
/// the package of the types it reads, so it gets no package-private access, and
/// it reaches the brewing state through the widened fields like any other
/// caller. It is namespace occupation and nothing more, it carries no behaviour,
/// and blocking it costs nothing.
pub const BLOCKED_PLACEMENT: &str = "a shipped class declared in the host's own root namespace";

/// The four mods in the pinned pack that declare this entry as a required
/// dependency.
///
/// All four are on both sides with a lower-bounded version range, no mod
/// declares it optionally, and no mod outside those four names it. All four are
/// outside this wave, so this package names their roles rather than them.
pub const DEPENDENT_ROLES: [&str; 4] = [
    "a machine-network mod",
    "a crafting-automation mod",
    "a terminal mod",
    "a transfer mod",
];

/// What the four dependents observe from this entry, in order of how much would
/// break without it.
pub const OBSERVED: [&str; 5] = [
    "the three ingredient kinds and their registered names",
    "the matching, ordering and hashing",
    "the serialised forms",
    "the store-adapter table",
    "the attachments",
];

/// What section sixteen confirms this entry consumes from RVL-210, which section
/// 17 of that entry already named.
pub const CONFIRMED_OF_RVL_210: [&str; 5] = [
    "the capability-attachment registry",
    "the helper facade",
    "the mod base and its settings declaration",
    "the mutable single-value box",
    "the ingredient-keyed map and the filtering iterator",
];

/// What section sixteen adds: consumed at the pinned revision and not named by
/// RVL-210's own section 17.
pub const ADDED_BY_THE_RECOUNT: [&str; 4] = [
    "the compatibility loader",
    "the client and common proxy split",
    "the cached recipe lookup",
    "the startup analytics ping and the remote version check",
];

/// What section sixteen removes: named by RVL-210's section 17 and not consumed
/// at the pinned revision.
pub const REMOVED_BY_THE_RECOUNT: [&str; 5] = [
    "the plain list among the quantity collections",
    "the linked list among the quantity collections",
    "the single-use fluid tank",
    "the parse error type",
    "the game-test harness's reach into RVL-210, which is none",
];

/// What section sixteen confirms this entry consumes nothing of.
pub const NOT_CONSUMED: [&str; 7] = [
    "the command transport",
    "the screen channel",
    "the query grammar",
    "the shape detector",
    "the saved-data mechanism",
    "registry dumping",
    "the manual engine",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_widening_groups_add_up_to_the_fifteen_entries() {
        let counted: usize = WIDENING_GROUPS.iter().map(|(_, count)| *count).sum();
        assert_eq!(counted, WIDENING_ENTRIES);
    }

    #[test]
    fn four_values_and_one_placement_are_blocked_and_named() {
        assert_ne!(BLOCKED_PLACEMENT, "");
        for blocked in BLOCKED_VALUES {
            assert_ne!(blocked, "");
            assert!(!BLOCKED_PLACEMENT.contains(blocked));
        }
    }

    #[test]
    fn the_recount_names_a_dependent_set_in_both_directions() {
        for added in ADDED_BY_THE_RECOUNT {
            assert!(!REMOVED_BY_THE_RECOUNT.contains(&added));
            assert!(!NOT_CONSUMED.contains(&added));
            assert!(!CONFIRMED_OF_RVL_210.contains(&added));
        }
        for removed in REMOVED_BY_THE_RECOUNT {
            assert!(!CONFIRMED_OF_RVL_210.contains(&removed));
            assert!(!NOT_CONSUMED.contains(&removed));
        }
    }

    #[test]
    fn four_dependents_observe_five_things_in_the_stated_order() {
        for role in DEPENDENT_ROLES {
            assert_ne!(role, "");
        }
        for (position, observed) in OBSERVED.iter().enumerate() {
            assert!(!OBSERVED[..position].contains(observed));
        }
    }
}
