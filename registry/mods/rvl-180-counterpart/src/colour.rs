//! The sixteen dye colours the shared grouping is cut along.
//!
//! §4 of the specification observes sixteen sub-tags in each aggregate, one per
//! dye colour, and nothing else about them. The set is the game's, not the
//! original's: sixteen dyes exist in the base game and a conventional grouping
//! over dyed blocks has exactly one leaf for each. That is why the list can be
//! written here at all without reading anything.

use std::fmt;

/// One of the base game's sixteen dyes.
///
/// The order is the game's own dye order, which is the order the leaves are
/// counted in and the order the aggregate names them in.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Dye {
    /// The first dye.
    White,
    /// The second dye.
    Orange,
    /// The third dye.
    Magenta,
    /// The fourth dye.
    LightBlue,
    /// The fifth dye.
    Yellow,
    /// The sixth dye.
    Lime,
    /// The seventh dye.
    Pink,
    /// The eighth dye.
    Gray,
    /// The ninth dye.
    LightGray,
    /// The tenth dye.
    Cyan,
    /// The eleventh dye.
    Purple,
    /// The twelfth dye.
    Blue,
    /// The thirteenth dye.
    Brown,
    /// The fourteenth dye.
    Green,
    /// The fifteenth dye.
    Red,
    /// The sixteenth dye.
    Black,
}

impl Dye {
    /// Every dye, in the base game's order.
    pub const ALL: [Self; 16] = [
        Self::White,
        Self::Orange,
        Self::Magenta,
        Self::LightBlue,
        Self::Yellow,
        Self::Lime,
        Self::Pink,
        Self::Gray,
        Self::LightGray,
        Self::Cyan,
        Self::Purple,
        Self::Blue,
        Self::Brown,
        Self::Green,
        Self::Red,
        Self::Black,
    ];

    /// Returns the dye's canonical lowercase name.
    ///
    /// This is the spelling a content path carries, so it holds to the charset
    /// a content path admits: lowercase letters and an underscore.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::White => "white",
            Self::Orange => "orange",
            Self::Magenta => "magenta",
            Self::LightBlue => "light_blue",
            Self::Yellow => "yellow",
            Self::Lime => "lime",
            Self::Pink => "pink",
            Self::Gray => "gray",
            Self::LightGray => "light_gray",
            Self::Cyan => "cyan",
            Self::Purple => "purple",
            Self::Blue => "blue",
            Self::Brown => "brown",
            Self::Green => "green",
            Self::Red => "red",
            Self::Black => "black",
        }
    }
}

impl fmt::Display for Dye {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::Dye;
    use std::collections::BTreeSet;

    #[test]
    fn there_are_sixteen_dyes_and_each_is_named_once() {
        let names: BTreeSet<&str> = Dye::ALL.iter().map(|dye| dye.as_str()).collect();
        assert_eq!(names.len(), 16);
    }

    #[test]
    fn every_dye_name_is_legal_in_a_content_path() {
        for dye in Dye::ALL {
            assert!(
                dye.as_str()
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character == '_'),
                "{dye} would be refused as a content path"
            );
        }
    }
}
