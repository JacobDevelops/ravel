//! The neighbourhood probe of the specification's §8.
//!
//! For one face, eight neighbours are sampled: the four in-plane orthogonals
//! and the four in-plane diagonals, each expressed relative to the face being
//! drawn rather than to world axes. The probe is a pure function of a face and
//! an accessor, which is what makes it testable without a running game.
//!
//! # The in-plane basis
//!
//! §8 calls the face-to-offset mapping a fixed six-by-four table, arbitrary but
//! load-bearing, and §15 says the numbers themselves are obtainable only from
//! the pinned artifact. Nothing upstream was opened to write this package, so
//! the table here is *derived* rather than reproduced: each face gets the right
//! and up vectors that make `right × up` the face's own outward normal. That is
//! a right-handed basis per face, it is written out in full in
//! [`Face::right`] and [`Face::up`], and `NOTES.md` records it as a declared
//! difference rather than a claim of parity.
//!
//! One property of the derived basis is worth stating because §9.4 depends on
//! it: exactly one of the two vectors points along a negative world axis on
//! exactly three of the six faces, and on the other three neither does. That is
//! the "one of them mirrored for three of the six faces" §9.4 describes, and
//! [`Face::mirrors_column`] and [`Face::mirrors_row`] are where it is read.

use crate::conditions::{BlockState, Condition, Groups};

/// One of the three world axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Axis {
    /// The first horizontal axis.
    X,
    /// The vertical axis.
    Y,
    /// The second horizontal axis.
    Z,
}

impl Axis {
    /// Every axis, in the order this enum declares them.
    pub const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    /// Returns the name a declaration spells this axis by.
    #[must_use]
    pub const fn declared_name(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
        }
    }

    /// Returns the axis a declaration spells by `name`, if any.
    #[must_use]
    pub fn by_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|axis| axis.declared_name() == name)
    }
}

/// A whole-block offset from the position being drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Offset {
    /// The offset along the first horizontal axis.
    pub x: i32,
    /// The offset along the vertical axis.
    pub y: i32,
    /// The offset along the second horizontal axis.
    pub z: i32,
}

impl Offset {
    /// The offset that goes nowhere.
    pub const ZERO: Self = Self::new(0, 0, 0);

    /// Builds an offset.
    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Returns the two offsets added, saturating rather than wrapping.
    #[must_use]
    pub const fn plus(self, other: Self) -> Self {
        Self {
            x: self.x.saturating_add(other.x),
            y: self.y.saturating_add(other.y),
            z: self.z.saturating_add(other.z),
        }
    }

    /// Returns this offset reversed.
    #[must_use]
    pub const fn reversed(self) -> Self {
        Self {
            x: self.x.saturating_neg(),
            y: self.y.saturating_neg(),
            z: self.z.saturating_neg(),
        }
    }

    /// Returns the component along `axis`.
    #[must_use]
    pub const fn along(self, axis: Axis) -> i32 {
        match axis {
            Axis::X => self.x,
            Axis::Y => self.y,
            Axis::Z => self.z,
        }
    }

    /// Returns the axis this offset lies along, if it lies along exactly one.
    #[must_use]
    pub fn axis(self) -> Option<Axis> {
        let mut found = None;
        for axis in Axis::ALL {
            if self.along(axis) != 0 {
                if found.is_some() {
                    return None;
                }
                found = Some(axis);
            }
        }
        found
    }
}

/// One of the six faces of a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Face {
    /// The face whose outward normal points down.
    Down,
    /// The face whose outward normal points up.
    Up,
    /// The face whose outward normal points along negative `z`.
    North,
    /// The face whose outward normal points along positive `z`.
    South,
    /// The face whose outward normal points along negative `x`.
    West,
    /// The face whose outward normal points along positive `x`.
    East,
}

impl Face {
    /// Every face, in the order slot assignment walks them.
    ///
    /// §10 assigns per-face texture indices by walking the six faces in a fixed
    /// order, so the order is part of the behaviour rather than an accident of
    /// this enum's declaration. It is written down here once and read there.
    pub const ALL: [Self; 6] = [
        Self::Down,
        Self::Up,
        Self::North,
        Self::South,
        Self::West,
        Self::East,
    ];

    /// Returns the position in [`Self::ALL`] used by per-face slot assignment.
    #[must_use]
    pub const fn position(self) -> usize {
        match self {
            Self::Down => 0,
            Self::Up => 1,
            Self::North => 2,
            Self::South => 3,
            Self::West => 4,
            Self::East => 5,
        }
    }

    /// Returns the name a declaration spells this face by.
    #[must_use]
    pub const fn declared_name(self) -> &'static str {
        match self {
            Self::Down => "down",
            Self::Up => "up",
            Self::North => "north",
            Self::South => "south",
            Self::West => "west",
            Self::East => "east",
        }
    }

    /// Returns the face a declaration spells by `name`, if any.
    #[must_use]
    pub fn by_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|face| face.declared_name() == name)
    }

    /// Returns the offset of the position directly in front of this face.
    #[must_use]
    pub const fn normal(self) -> Offset {
        match self {
            Self::Down => Offset::new(0, -1, 0),
            Self::Up => Offset::new(0, 1, 0),
            Self::North => Offset::new(0, 0, -1),
            Self::South => Offset::new(0, 0, 1),
            Self::West => Offset::new(-1, 0, 0),
            Self::East => Offset::new(1, 0, 0),
        }
    }

    /// Returns the axis this face's normal lies along.
    #[must_use]
    pub const fn axis(self) -> Axis {
        match self {
            Self::Down | Self::Up => Axis::Y,
            Self::North | Self::South => Axis::Z,
            Self::West | Self::East => Axis::X,
        }
    }

    /// Returns whether this face's normal points along the vertical axis.
    ///
    /// The specification calls these the vertical faces and the other four the
    /// horizontal ones, naming each by the direction it points rather than by
    /// the plane it lies in. §9.2, §9.3 and §9.6 all branch on this.
    #[must_use]
    pub const fn is_vertical(self) -> bool {
        matches!(self, Self::Down | Self::Up)
    }

    /// Returns the face on the other side of the block.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Down => Self::Up,
            Self::Up => Self::Down,
            Self::North => Self::South,
            Self::South => Self::North,
            Self::West => Self::East,
            Self::East => Self::West,
        }
    }

    /// Returns the in-plane vector that face coordinates run rightwards along.
    #[must_use]
    pub const fn right(self) -> Offset {
        match self {
            Self::Down | Self::Up | Self::South => Offset::new(1, 0, 0),
            Self::North => Offset::new(-1, 0, 0),
            Self::West => Offset::new(0, 0, 1),
            Self::East => Offset::new(0, 0, -1),
        }
    }

    /// Returns the in-plane vector that face coordinates run upwards along.
    #[must_use]
    pub const fn up(self) -> Offset {
        match self {
            Self::Down => Offset::new(0, 0, 1),
            Self::Up => Offset::new(0, 0, -1),
            Self::North | Self::South | Self::West | Self::East => Offset::new(0, 1, 0),
        }
    }

    /// Returns whether the column coordinate of §9.4 is mirrored on this face.
    #[must_use]
    pub fn mirrors_column(self) -> bool {
        let right = self.right();
        right.x + right.y + right.z < 0
    }

    /// Returns whether the row coordinate of §9.4 is mirrored on this face.
    #[must_use]
    pub fn mirrors_row(self) -> bool {
        let up = self.up();
        up.x + up.y + up.z < 0
    }

    /// Returns the offset of the in-plane orthogonal at `index`.
    ///
    /// Index zero is rightwards and the four run anticlockwise, so index one is
    /// upwards, two leftwards and three downwards. Any other index folds into
    /// that cycle.
    #[must_use]
    pub const fn orthogonal(self, index: usize) -> Offset {
        match index % 4 {
            0 => self.right(),
            1 => self.up(),
            2 => self.right().reversed(),
            _ => self.up().reversed(),
        }
    }

    /// Returns the offset of the in-plane diagonal between orthogonal `index`
    /// and the one after it.
    #[must_use]
    pub const fn diagonal(self, index: usize) -> Offset {
        self.orthogonal(index).plus(self.orthogonal(index + 1))
    }
}

/// What the eight samples around one face answered.
///
/// `orthogonal[i]` is the neighbour at [`Face::orthogonal`] of `i` and
/// `diagonal[i]` is the one at [`Face::diagonal`] of `i`, so quadrant `i` of a
/// face is bounded by `orthogonal[i]`, `orthogonal[i + 1]` and `diagonal[i]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Neighbours {
    /// Whether each in-plane orthogonal connects.
    pub orthogonal: [bool; 4],
    /// Whether each in-plane diagonal connects.
    pub diagonal: [bool; 4],
}

impl Neighbours {
    /// The answer in which nothing connects.
    pub const NONE: Self = Self {
        orthogonal: [false; 4],
        diagonal: [false; 4],
    };

    /// The answer in which everything connects.
    pub const ALL: Self = Self {
        orthogonal: [true; 4],
        diagonal: [true; 4],
    };

    /// Returns whether all eight neighbours connect.
    #[must_use]
    pub fn all_connected(&self) -> bool {
        self.orthogonal.iter().chain(&self.diagonal).all(|&it| it)
    }

    /// Returns the three answers quadrant `index` is decided by.
    ///
    /// The first is the orthogonal at `index`, the second the one after it, and
    /// the third their shared diagonal.
    #[must_use]
    pub fn quadrant(&self, index: usize) -> (bool, bool, bool) {
        let index = index % 4;
        (
            self.orthogonal[index],
            self.orthogonal[(index + 1) % 4],
            self.diagonal[index],
        )
    }
}

/// What a probe needs to know about the positions around the one being drawn.
///
/// Both halves are *appearances* rather than raw states: §8 is explicit that a
/// neighbour presenting itself as another block for rendering purposes connects
/// as that other block, and that the condition is applied to the block's own
/// appearance as seen from the sampled position. Ravel publishes no live block
/// state access from the render thread, so the real implementation of this is
/// host-blocked; the probe itself is a pure function of whatever answers it.
pub trait Neighbourhood {
    /// Returns the appearance of the position at `offset`.
    fn appearance(&self, offset: Offset) -> BlockState;

    /// Returns the appearance the block being drawn presents when it is looked
    /// at from `offset`.
    ///
    /// The default answers with the block's own appearance at the centre, which
    /// is what a block with no appearance override presents in every direction.
    fn appearance_from(&self, offset: Offset) -> BlockState {
        let _ = offset;
        self.appearance(Offset::ZERO)
    }
}

/// The answer a probe gives for one face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Probe {
    /// The face is culled and produces no quad at all.
    Culled,
    /// The face is drawn, and these are its eight answers.
    Drawn(Neighbours),
}

impl Probe {
    /// Returns the neighbours, or `None` when the face was culled.
    #[must_use]
    pub const fn neighbours(self) -> Option<Neighbours> {
        match self {
            Self::Culled => None,
            Self::Drawn(neighbours) => Some(neighbours),
        }
    }
}

/// Probes the eight neighbours of one face.
///
/// The occlusion test runs first: if the position directly in front of the face
/// presents an appearance of the same block and is not empty, the face is
/// culled and nothing else is asked. Then each of the eight samples is taken,
/// an empty appearance never connects, and everything else is put to
/// `condition`.
pub fn probe(
    face: Face,
    condition: &Condition,
    groups: &Groups,
    neighbourhood: &dyn Neighbourhood,
) -> Probe {
    if occluded(face, neighbourhood) {
        return Probe::Culled;
    }
    Probe::Drawn(sample(face, condition, groups, neighbourhood))
}

/// Answers whether the position in front of `face` occludes it.
#[must_use]
pub fn occluded(face: Face, neighbourhood: &dyn Neighbourhood) -> bool {
    let own = neighbourhood.appearance(Offset::ZERO);
    let front = neighbourhood.appearance(face.normal());
    !front.is_nothing() && front.same_block(&own)
}

/// Takes the eight samples without the occlusion test.
///
/// §9.2 needs this: the carpet kind performs no occlusion cull at all, so a
/// face is never dropped for what sits in front of it.
#[must_use]
pub fn sample(
    face: Face,
    condition: &Condition,
    groups: &Groups,
    neighbourhood: &dyn Neighbourhood,
) -> Neighbours {
    let mut neighbours = Neighbours::NONE;
    for index in 0..4 {
        neighbours.orthogonal[index] =
            connects(face.orthogonal(index), condition, groups, neighbourhood);
        neighbours.diagonal[index] =
            connects(face.diagonal(index), condition, groups, neighbourhood);
    }
    neighbours
}

fn connects(
    offset: Offset,
    condition: &Condition,
    groups: &Groups,
    neighbourhood: &dyn Neighbourhood,
) -> bool {
    let sampled = neighbourhood.appearance(offset);
    if sampled.is_nothing() {
        return false;
    }
    let own = neighbourhood.appearance_from(offset);
    condition.connects(&own, &sampled, groups)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::BTreeMap;

    use super::{Axis, Face, Neighbourhood, Neighbours, Offset, Probe, probe, sample};
    use crate::conditions::{BlockState, Condition, Groups};

    /// A neighbourhood written out position by position.
    #[derive(Default)]
    pub(crate) struct Written {
        centre: BlockState,
        around: BTreeMap<Offset, BlockState>,
        presented: BTreeMap<Offset, BlockState>,
    }

    impl Written {
        pub(crate) fn of(centre: BlockState) -> Self {
            Self {
                centre,
                ..Self::default()
            }
        }

        pub(crate) fn at(mut self, offset: Offset, state: BlockState) -> Self {
            self.around.insert(offset, state);
            self
        }

        pub(crate) fn all_around(mut self, face: Face, state: &BlockState) -> Self {
            for index in 0..4 {
                self.around.insert(face.orthogonal(index), state.clone());
                self.around.insert(face.diagonal(index), state.clone());
            }
            self
        }

        fn presenting(mut self, offset: Offset, state: BlockState) -> Self {
            self.presented.insert(offset, state);
            self
        }
    }

    impl Neighbourhood for Written {
        fn appearance(&self, offset: Offset) -> BlockState {
            if offset == Offset::ZERO {
                return self.centre.clone();
            }
            self.around
                .get(&offset)
                .cloned()
                .unwrap_or_else(BlockState::nothing)
        }

        fn appearance_from(&self, offset: Offset) -> BlockState {
            self.presented
                .get(&offset)
                .cloned()
                .unwrap_or_else(|| self.centre.clone())
        }
    }

    #[test]
    fn every_face_carries_a_right_handed_basis_whose_cross_product_is_its_normal() {
        for face in Face::ALL {
            let right = face.right();
            let up = face.up();
            let cross = Offset::new(
                right.y * up.z - right.z * up.y,
                right.z * up.x - right.x * up.z,
                right.x * up.y - right.y * up.x,
            );
            assert_eq!(cross, face.normal(), "{face:?}");
            assert!(right.axis().is_some(), "{face:?} right is one axis");
            assert!(up.axis().is_some(), "{face:?} up is one axis");
            assert_ne!(right.axis(), up.axis(), "{face:?} basis is not degenerate");
            assert_ne!(
                right.axis(),
                Some(face.axis()),
                "{face:?} right is in plane"
            );
            assert_ne!(up.axis(), Some(face.axis()), "{face:?} up is in plane");
        }
    }

    #[test]
    fn exactly_three_of_the_six_faces_mirror_exactly_one_coordinate() {
        let mirrored: Vec<Face> = Face::ALL
            .into_iter()
            .filter(|face| face.mirrors_column() || face.mirrors_row())
            .collect();
        assert_eq!(mirrored.len(), 3, "{mirrored:?}");
        for face in Face::ALL {
            assert!(
                !(face.mirrors_column() && face.mirrors_row()),
                "{face:?} mirrors both"
            );
        }
    }

    #[test]
    fn the_four_orthogonals_are_distinct_and_the_diagonals_are_their_sums() {
        for face in Face::ALL {
            let mut seen: Vec<Offset> = (0..4).map(|i| face.orthogonal(i)).collect();
            let count = seen.len();
            seen.sort_unstable();
            seen.dedup();
            assert_eq!(seen.len(), count, "{face:?}");
            for index in 0..4 {
                assert_eq!(
                    face.diagonal(index),
                    face.orthogonal(index).plus(face.orthogonal(index + 1)),
                    "{face:?} diagonal {index}"
                );
                assert_eq!(
                    face.orthogonal(index).plus(face.orthogonal(index + 2)),
                    Offset::ZERO,
                    "{face:?} orthogonal {index} is opposite the one two along"
                );
            }
        }
    }

    #[test]
    fn no_sampled_offset_leaves_the_faces_own_plane() {
        for face in Face::ALL {
            for index in 0..4 {
                assert_eq!(face.orthogonal(index).along(face.axis()), 0, "{face:?}");
                assert_eq!(face.diagonal(index).along(face.axis()), 0, "{face:?}");
            }
        }
    }

    #[test]
    fn a_face_whose_front_presents_the_same_block_is_culled_before_anything_is_sampled() {
        let neighbourhood = Written::of(BlockState::new("base:panel"))
            .at(Face::North.normal(), BlockState::new("base:panel"))
            .at(Face::North.orthogonal(0), BlockState::new("base:panel"));
        assert_eq!(
            probe(
                Face::North,
                &Condition::SameBlock,
                &Groups::new(),
                &neighbourhood
            ),
            Probe::Culled
        );

        // The same neighbourhood on a face nothing sits in front of is drawn.
        assert!(matches!(
            probe(
                Face::South,
                &Condition::SameBlock,
                &Groups::new(),
                &neighbourhood
            ),
            Probe::Drawn(_)
        ));
    }

    #[test]
    fn an_empty_position_in_front_does_not_cull_and_an_empty_sample_never_connects() {
        let neighbourhood = Written::of(BlockState::new("base:panel"));
        let Probe::Drawn(neighbours) = probe(
            Face::North,
            // A condition that would connect anything, so only the emptiness
            // guard can be what refuses the samples.
            &Condition::Inverted(Box::new(Condition::Never)),
            &Groups::new(),
            &neighbourhood,
        ) else {
            panic!("an empty position in front does not occlude");
        };
        assert_eq!(neighbours, Neighbours::NONE);
    }

    #[test]
    fn all_eight_samples_are_taken_and_each_lands_in_its_own_slot() {
        let face = Face::Up;
        let mut neighbourhood = Written::of(BlockState::new("base:panel"));
        neighbourhood = neighbourhood.at(face.orthogonal(1), BlockState::new("base:panel"));
        neighbourhood = neighbourhood.at(face.diagonal(2), BlockState::new("base:panel"));

        let neighbours = sample(face, &Condition::SameBlock, &Groups::new(), &neighbourhood);
        assert_eq!(neighbours.orthogonal, [false, true, false, false]);
        assert_eq!(neighbours.diagonal, [false, false, true, false]);
        assert!(!neighbours.all_connected());

        let (first, second, diagonal) = neighbours.quadrant(1);
        assert_eq!((first, second, diagonal), (true, false, false));
    }

    #[test]
    fn a_neighbour_that_presents_another_block_connects_as_that_other_block() {
        let face = Face::Up;
        let offset = face.orthogonal(0);
        let neighbourhood =
            Written::of(BlockState::new("base:panel")).at(offset, BlockState::new("base:beam"));
        let condition = Condition::SameBlock;

        let raw = sample(face, &condition, &Groups::new(), &neighbourhood);
        assert!(!raw.orthogonal[0], "a different block does not connect");

        // The block being drawn presents itself as the neighbour's block when
        // looked at from there, so the same-block test now holds.
        let presenting = Written::of(BlockState::new("base:panel"))
            .at(offset, BlockState::new("base:beam"))
            .presenting(offset, BlockState::new("base:beam"));
        let overridden = sample(face, &condition, &Groups::new(), &presenting);
        assert!(
            overridden.orthogonal[0],
            "the condition reads the appearance, not the raw state"
        );
    }

    #[test]
    fn the_six_faces_declare_six_names_and_three_axes() {
        let mut names: Vec<&str> = Face::ALL.iter().map(|f| f.declared_name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 6);
        for (position, face) in Face::ALL.into_iter().enumerate() {
            assert_eq!(face.position(), position);
            assert_eq!(Face::by_name(face.declared_name()), Some(face));
            assert_eq!(face.opposite().opposite(), face);
            assert_eq!(face.opposite().axis(), face.axis());
            assert_eq!(face.is_vertical(), face.axis() == Axis::Y);
        }
        assert_eq!(Face::by_name("nothing_spells_this"), None);
        for axis in Axis::ALL {
            assert_eq!(Axis::by_name(axis.declared_name()), Some(axis));
        }
        assert_eq!(Axis::by_name("w"), None);
    }
}
