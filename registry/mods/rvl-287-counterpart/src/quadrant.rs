//! The quadrant selection rule of the specification's §6.
//!
//! Three booleans — whether the first neighbour connects, whether the second
//! connects, and whether the diagonal they share connects — reduce to one of
//! five texture slots. Every connection-based kind in [`crate::geometry`] goes
//! through this one function, and the specification calls it the single most
//! parity-testable rule in the entry, so it is a free function over plain
//! booleans with no state behind it.

use core::fmt;

/// The five texture slots a connection-based declaration names.
///
/// The declaration spells these by name and the runtime addresses them by the
/// number [`Quadrant::slot_number`] gives. Slot zero is never one of these: it
/// is the particle texture throughout, which is why the five start at one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Quadrant {
    /// Neither neighbour connects.
    Isolated,
    /// The first neighbour connects and the second does not.
    FirstEdge,
    /// The second neighbour connects and the first does not.
    SecondEdge,
    /// Both neighbours connect and the diagonal between them does not.
    InnerCorner,
    /// Both neighbours and their shared diagonal connect.
    Surrounded,
}

impl Quadrant {
    /// Every slot, in the order this enum declares them.
    pub const ALL: [Self; 5] = [
        Self::Isolated,
        Self::FirstEdge,
        Self::SecondEdge,
        Self::InnerCorner,
        Self::Surrounded,
    ];

    /// Returns the slot number a texture map addresses this slot by.
    ///
    /// One-based, because slot zero is the particle texture.
    #[must_use]
    pub const fn slot_number(self) -> u32 {
        match self {
            Self::Isolated => 1,
            Self::FirstEdge => 2,
            Self::SecondEdge => 3,
            Self::InnerCorner => 4,
            Self::Surrounded => 5,
        }
    }

    /// Returns the name a declaration spells this slot by.
    #[must_use]
    pub const fn declared_name(self) -> &'static str {
        match self {
            Self::Isolated => "alone",
            Self::FirstEdge => "first_side",
            Self::SecondEdge => "second_side",
            Self::InnerCorner => "notch",
            Self::Surrounded => "enclosed",
        }
    }

    /// Returns the slot a declaration spells by `name`, if any.
    #[must_use]
    pub fn by_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|slot| slot.declared_name() == name)
    }
}

impl fmt::Display for Quadrant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.declared_name())
    }
}

/// Reduces the three connection answers for one quadrant to its texture slot.
///
/// The diagonal is consulted only when both neighbours connect, which is what
/// makes the eight input combinations cover five outcomes rather than eight.
#[must_use]
pub const fn select(first: bool, second: bool, diagonal: bool) -> Quadrant {
    match (first, second, diagonal) {
        (true, true, true) => Quadrant::Surrounded,
        (true, true, false) => Quadrant::InnerCorner,
        (true, false, _) => Quadrant::FirstEdge,
        (false, true, _) => Quadrant::SecondEdge,
        (false, false, _) => Quadrant::Isolated,
    }
}

#[cfg(test)]
mod tests {
    use super::{Quadrant, select};

    /// The specification's table, all eight rows, written out rather than
    /// generated: a table generated from the implementation agrees with any
    /// implementation.
    const TABLE: [(bool, bool, bool, Quadrant); 8] = [
        (true, true, true, Quadrant::Surrounded),
        (true, true, false, Quadrant::InnerCorner),
        (true, false, true, Quadrant::FirstEdge),
        (true, false, false, Quadrant::FirstEdge),
        (false, true, true, Quadrant::SecondEdge),
        (false, true, false, Quadrant::SecondEdge),
        (false, false, true, Quadrant::Isolated),
        (false, false, false, Quadrant::Isolated),
    ];

    #[test]
    fn every_one_of_the_eight_input_combinations_selects_the_specified_slot() {
        for (first, second, diagonal, expected) in TABLE {
            assert_eq!(
                select(first, second, diagonal),
                expected,
                "first={first} second={second} diagonal={diagonal}"
            );
        }
    }

    #[test]
    fn the_diagonal_is_read_only_when_both_neighbours_connect() {
        for first in [false, true] {
            for second in [false, true] {
                let with = select(first, second, true);
                let without = select(first, second, false);
                if first && second {
                    assert_ne!(with, without, "both connected, so the diagonal decides");
                } else {
                    assert_eq!(with, without, "the diagonal is not consulted here");
                }
            }
        }
    }

    #[test]
    fn the_five_slots_have_five_distinct_numbers_and_five_distinct_names() {
        let mut numbers: Vec<u32> = Quadrant::ALL.iter().map(|s| s.slot_number()).collect();
        numbers.sort_unstable();
        numbers.dedup();
        assert_eq!(numbers.len(), 5);
        assert!(
            !numbers.contains(&0),
            "slot zero is the particle texture and is never a quadrant slot"
        );

        let mut names: Vec<&str> = Quadrant::ALL.iter().map(|s| s.declared_name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 5);
    }

    #[test]
    fn a_declared_name_round_trips_and_an_unknown_one_is_not_a_slot() {
        for slot in Quadrant::ALL {
            assert_eq!(Quadrant::by_name(slot.declared_name()), Some(slot));
        }
        assert_eq!(Quadrant::by_name("nothing_spells_this"), None);
    }
}
