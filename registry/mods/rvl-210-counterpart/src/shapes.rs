//! Section twelve: multiblock shape detection.
//!
//! Given a set of block predicates and a starting position, the detector finds
//! the connected region of matching blocks reachable from that position and
//! validates it against a list of constraints. The result a counterpart owes is
//! not a boolean: a dependent's whole player-facing experience of building a
//! multiblock is the message naming what is wrong, so [`DetectionFailure`] carries the
//! position and the reason.
//!
//! The specification records the traversal order as unread. This detector walks
//! in a stated order and says so in `NOTES.md` rather than claiming the
//! original's. It has no tie to break: anchoring the walk at the start is what
//! "the largest connected region" is read as here, and one anchor reaches one
//! region.

use std::collections::{BTreeSet, VecDeque};

use core::fmt;

/// A position in a level.
pub type Position = [i32; 3];

/// The most blocks one detection will visit.
///
/// A detector is driven by whatever a player built, so an unbounded flood is a
/// player-triggered walk of a whole level.
pub const MAX_VISITED: usize = 32_768;

/// The six directions a region is connected through.
///
/// Faces only: two blocks touching at an edge or a corner are not connected, so
/// a diagonal chain is two regions rather than one.
pub const NEIGHBOURS: [Position; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];

/// One rule a found region is held to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Constraint {
    /// The region holds exactly this many blocks.
    ExactSize(usize),
    /// At least this many.
    MinimumSize(usize),
    /// At most this many.
    MaximumSize(usize),
    /// The region's bounds are a cube.
    Cube,
    /// Exactly this many of one allowed block.
    ExactCount(String, usize),
    /// At least this many of one allowed block.
    MinimumCount(String, usize),
    /// At most this many of one allowed block.
    MaximumCount(String, usize),
    /// The region's interior is empty, which is the hollow variant.
    Hollow,
}

/// Why a detection failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectionFailure {
    /// Where the failure was found.
    pub at: Position,
    /// What is wrong, in the words a dependent shows a player.
    pub reason: ShapeRefused,
}

/// What was wrong with a region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShapeRefused {
    /// The starting position holds nothing the predicates admit.
    NothingAtTheStart,
    /// The region is the wrong size.
    WrongSize {
        /// How many blocks it holds.
        found: usize,
        /// What the constraint asked for.
        wanted: usize,
    },
    /// The region's bounds are not a cube.
    NotACube {
        /// The bounds it does have.
        extent: Position,
    },
    /// The region holds the wrong number of one block.
    WrongCount {
        /// Which block.
        block: String,
        /// How many it holds.
        found: usize,
        /// What the constraint asked for.
        wanted: usize,
    },
    /// A block sits inside a region required to be hollow.
    NotHollow,
    /// The walk reached the published visit bound.
    TooLarge {
        /// The published bound.
        limit: usize,
    },
}

impl fmt::Display for ShapeRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NothingAtTheStart => formatter.write_str("nothing here belongs to the shape"),
            Self::WrongSize { found, wanted } => {
                write!(
                    formatter,
                    "the shape holds {found} blocks and wants {wanted}"
                )
            }
            Self::NotACube { extent } => write!(
                formatter,
                "the shape is {} by {} by {} and wants to be a cube",
                extent[0], extent[1], extent[2]
            ),
            Self::WrongCount {
                block,
                found,
                wanted,
            } => write!(
                formatter,
                "the shape holds {found} {block} and wants {wanted}"
            ),
            Self::NotHollow => formatter.write_str("the shape is not hollow"),
            Self::TooLarge { limit } => {
                write!(formatter, "the shape is larger than the {limit} this reads")
            }
        }
    }
}

impl std::error::Error for ShapeRefused {}

/// A region that satisfied every constraint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Region {
    /// Every position in it, in ascending order.
    pub blocks: Vec<Position>,
    /// The lowest corner of its bounds.
    pub lowest: Position,
    /// The highest corner of its bounds.
    pub highest: Position,
}

impl Region {
    /// How many blocks the region holds.
    #[must_use]
    pub fn size(&self) -> usize {
        self.blocks.len()
    }

    /// How far the bounds reach on each axis, counting both ends.
    #[must_use]
    pub fn extent(&self) -> Position {
        [
            self.highest[0] - self.lowest[0] + 1,
            self.highest[1] - self.lowest[1] + 1,
            self.highest[2] - self.lowest[2] + 1,
        ]
    }

    /// Whether a position is inside the bounds but not on their surface.
    #[must_use]
    pub fn is_interior(&self, at: Position) -> bool {
        (0..3).all(|axis| at[axis] > self.lowest[axis] && at[axis] < self.highest[axis])
    }
}

/// Finds the connected region reachable from a position and validates it.
///
/// Section twelve says "the largest connected region", and the reading here is
/// that the largest one a player means is the one they are standing at: the walk
/// is anchored at `start`, so it never sees a second region and never has to
/// choose between two. A detector that searched for candidate regions would need
/// a search space section twelve does not name.
///
/// `block_at` answers what is at a position, or nothing where the level holds
/// no block this detector may read. `admits` decides which blocks belong to the
/// shape, which is the predicate set.
///
/// # Errors
///
/// Returns [`DetectionFailure`] naming the position and the reason, which is what a
/// dependent shows a player.
pub fn detect(
    start: Position,
    block_at: &dyn Fn(Position) -> Option<String>,
    admits: &dyn Fn(&str) -> bool,
    constraints: &[Constraint],
) -> Result<Region, DetectionFailure> {
    if !block_at(start).is_some_and(|block| admits(&block)) {
        return Err(DetectionFailure {
            at: start,
            reason: ShapeRefused::NothingAtTheStart,
        });
    }
    let mut found: BTreeSet<Position> = BTreeSet::new();
    let mut queue: VecDeque<Position> = VecDeque::new();
    queue.push_back(start);
    found.insert(start);
    while let Some(at) = queue.pop_front() {
        for step in NEIGHBOURS {
            let Some(next) = offset(at, step) else {
                continue;
            };
            if found.contains(&next) {
                continue;
            }
            if block_at(next).is_some_and(|block| admits(&block)) {
                if found.len() >= MAX_VISITED {
                    return Err(DetectionFailure {
                        at: next,
                        reason: ShapeRefused::TooLarge { limit: MAX_VISITED },
                    });
                }
                found.insert(next);
                queue.push_back(next);
            }
        }
    }
    let blocks: Vec<Position> = found.into_iter().collect();
    let region = bounds(blocks);
    validate(&region, block_at, constraints)?;
    Ok(region)
}

fn offset(at: Position, step: Position) -> Option<Position> {
    Some([
        at[0].checked_add(step[0])?,
        at[1].checked_add(step[1])?,
        at[2].checked_add(step[2])?,
    ])
}

fn bounds(blocks: Vec<Position>) -> Region {
    let mut lowest = blocks.first().copied().unwrap_or([0, 0, 0]);
    let mut highest = lowest;
    for at in &blocks {
        for axis in 0..3 {
            lowest[axis] = lowest[axis].min(at[axis]);
            highest[axis] = highest[axis].max(at[axis]);
        }
    }
    Region {
        blocks,
        lowest,
        highest,
    }
}

fn validate(
    region: &Region,
    block_at: &dyn Fn(Position) -> Option<String>,
    constraints: &[Constraint],
) -> Result<(), DetectionFailure> {
    let at = region.lowest;
    let count_of = |wanted: &str| {
        region
            .blocks
            .iter()
            .filter(|position| block_at(**position).as_deref() == Some(wanted))
            .count()
    };
    for constraint in constraints {
        match constraint {
            Constraint::ExactSize(wanted) if region.size() != *wanted => {
                return Err(wrong_size(region, *wanted));
            }
            Constraint::MinimumSize(wanted) if region.size() < *wanted => {
                return Err(wrong_size(region, *wanted));
            }
            Constraint::MaximumSize(wanted) if region.size() > *wanted => {
                return Err(wrong_size(region, *wanted));
            }
            Constraint::Cube => {
                let extent = region.extent();
                if extent[0] != extent[1] || extent[1] != extent[2] {
                    return Err(DetectionFailure {
                        at,
                        reason: ShapeRefused::NotACube { extent },
                    });
                }
            }
            Constraint::ExactCount(block, wanted) if count_of(block) != *wanted => {
                return Err(wrong_count(at, block, count_of(block), *wanted));
            }
            Constraint::MinimumCount(block, wanted) if count_of(block) < *wanted => {
                return Err(wrong_count(at, block, count_of(block), *wanted));
            }
            Constraint::MaximumCount(block, wanted) if count_of(block) > *wanted => {
                return Err(wrong_count(at, block, count_of(block), *wanted));
            }
            Constraint::Hollow => {
                if let Some(inside) = region
                    .blocks
                    .iter()
                    .find(|position| region.is_interior(**position))
                {
                    return Err(DetectionFailure {
                        at: *inside,
                        reason: ShapeRefused::NotHollow,
                    });
                }
            }
            Constraint::ExactSize(_)
            | Constraint::MinimumSize(_)
            | Constraint::MaximumSize(_)
            | Constraint::ExactCount(..)
            | Constraint::MinimumCount(..)
            | Constraint::MaximumCount(..) => {}
        }
    }
    Ok(())
}

fn wrong_size(region: &Region, wanted: usize) -> DetectionFailure {
    DetectionFailure {
        at: region.lowest,
        reason: ShapeRefused::WrongSize {
            found: region.size(),
            wanted,
        },
    }
}

fn wrong_count(at: Position, block: &str, found: usize, wanted: usize) -> DetectionFailure {
    DetectionFailure {
        at,
        reason: ShapeRefused::WrongCount {
            block: block.to_owned(),
            found,
            wanted,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(side: i32) -> impl Fn(Position) -> Option<String> {
        move |at: Position| {
            (0..3)
                .all(|axis| at[axis] >= 0 && at[axis] < side)
                .then(|| "wall".to_owned())
        }
    }

    fn everything(_block: &str) -> bool {
        true
    }

    #[test]
    fn a_start_holding_nothing_the_predicates_admit_names_that_position() {
        let level = |_at: Position| None;
        assert_eq!(
            detect([4, 4, 4], &level, &everything, &[]),
            Err(DetectionFailure {
                at: [4, 4, 4],
                reason: ShapeRefused::NothingAtTheStart,
            })
        );
    }

    #[test]
    fn a_predicate_that_admits_nothing_refuses_at_the_start() {
        let level = cube(3);
        let refused = detect([0, 0, 0], &level, &|_block| false, &[]);
        assert!(matches!(
            refused,
            Err(DetectionFailure {
                reason: ShapeRefused::NothingAtTheStart,
                ..
            })
        ));
    }

    #[test]
    fn the_region_found_is_every_connected_block_and_its_bounds() {
        let level = cube(3);
        let region = detect([1, 1, 1], &level, &everything, &[]).expect("a region");
        assert_eq!(region.size(), 27);
        assert_eq!(region.lowest, [0, 0, 0]);
        assert_eq!(region.highest, [2, 2, 2]);
        assert_eq!(region.extent(), [3, 3, 3]);
    }

    #[test]
    fn two_blocks_touching_only_at_a_corner_are_two_regions() {
        let level = |at: Position| (at == [0, 0, 0] || at == [1, 1, 1]).then(|| "wall".to_owned());
        let region = detect([0, 0, 0], &level, &everything, &[]).expect("a region");
        assert_eq!(region.size(), 1, "a diagonal is not a connection");
    }

    #[test]
    fn every_size_constraint_reports_what_it_found_and_what_it_wanted() {
        let level = cube(3);
        for (constraint, wanted) in [
            (Constraint::ExactSize(8), 8),
            (Constraint::MinimumSize(64), 64),
            (Constraint::MaximumSize(8), 8),
        ] {
            assert_eq!(
                detect([0, 0, 0], &level, &everything, &[constraint]),
                Err(DetectionFailure {
                    at: [0, 0, 0],
                    reason: ShapeRefused::WrongSize { found: 27, wanted },
                })
            );
        }
        assert!(
            detect(
                [0, 0, 0],
                &level,
                &everything,
                &[
                    Constraint::ExactSize(27),
                    Constraint::MinimumSize(1),
                    Constraint::MaximumSize(27)
                ]
            )
            .is_ok()
        );
    }

    #[test]
    fn the_cube_constraint_reports_the_extent_it_found() {
        let level = |at: Position| {
            (at[1] == 0 && at[2] == 0 && (0..4).contains(&at[0])).then(|| "wall".to_owned())
        };
        assert_eq!(
            detect([0, 0, 0], &level, &everything, &[Constraint::Cube]),
            Err(DetectionFailure {
                at: [0, 0, 0],
                reason: ShapeRefused::NotACube { extent: [4, 1, 1] },
            })
        );
        assert!(detect([0, 0, 0], &cube(3), &everything, &[Constraint::Cube]).is_ok());
    }

    #[test]
    fn every_count_constraint_names_the_block_it_counted() {
        let level = |at: Position| {
            if !(0..3).all(|axis| at[axis] >= 0 && at[axis] < 3) {
                return None;
            }
            Some(if at == [1, 1, 1] { "core" } else { "wall" }.to_owned())
        };
        assert_eq!(
            detect(
                [0, 0, 0],
                &level,
                &everything,
                &[Constraint::ExactCount("core".to_owned(), 2)]
            ),
            Err(DetectionFailure {
                at: [0, 0, 0],
                reason: ShapeRefused::WrongCount {
                    block: "core".to_owned(),
                    found: 1,
                    wanted: 2,
                },
            })
        );
        assert!(
            detect(
                [0, 0, 0],
                &level,
                &everything,
                &[
                    Constraint::ExactCount("core".to_owned(), 1),
                    Constraint::MinimumCount("wall".to_owned(), 26),
                    Constraint::MaximumCount("core".to_owned(), 1),
                ]
            )
            .is_ok()
        );
        assert!(matches!(
            detect(
                [0, 0, 0],
                &level,
                &everything,
                &[Constraint::MinimumCount("core".to_owned(), 9)]
            ),
            Err(DetectionFailure {
                reason: ShapeRefused::WrongCount { .. },
                ..
            })
        ));
    }

    #[test]
    fn the_hollow_variant_names_the_block_that_is_inside() {
        let level = cube(3);
        assert_eq!(
            detect([0, 0, 0], &level, &everything, &[Constraint::Hollow]),
            Err(DetectionFailure {
                at: [1, 1, 1],
                reason: ShapeRefused::NotHollow,
            })
        );
        let shell = |at: Position| {
            let inside = (0..3).all(|axis| at[axis] >= 0 && at[axis] < 3);
            (inside && at != [1, 1, 1]).then(|| "wall".to_owned())
        };
        assert!(detect([0, 0, 0], &shell, &everything, &[Constraint::Hollow]).is_ok());
    }

    #[test]
    fn a_failure_reads_as_the_message_a_dependent_shows() {
        assert_eq!(
            ShapeRefused::NothingAtTheStart.to_string(),
            "nothing here belongs to the shape"
        );
        assert_eq!(
            ShapeRefused::WrongSize {
                found: 3,
                wanted: 9
            }
            .to_string(),
            "the shape holds 3 blocks and wants 9"
        );
        assert!(
            ShapeRefused::NotACube { extent: [4, 1, 1] }
                .to_string()
                .contains('4')
        );
        assert!(
            ShapeRefused::WrongCount {
                block: "core".to_owned(),
                found: 1,
                wanted: 2
            }
            .to_string()
            .contains("core")
        );
        assert_eq!(
            ShapeRefused::NotHollow.to_string(),
            "the shape is not hollow"
        );
        assert!(
            ShapeRefused::TooLarge { limit: 9 }
                .to_string()
                .contains('9')
        );
    }

    #[test]
    fn a_walk_at_the_edge_of_the_coordinate_space_does_not_overflow() {
        let level = |at: Position| (at == [i32::MAX, 0, 0]).then(|| "wall".to_owned());
        let region = detect([i32::MAX, 0, 0], &level, &everything, &[]).expect("a region");
        assert_eq!(region.size(), 1);
    }
}
