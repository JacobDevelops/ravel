//! The seven per-kind geometries of the specification's §9.
//!
//! Every kind answers the same question — what quads does this face produce —
//! and each answer is a pure function of a probe result, a set of state flags
//! and, for the tiled kind, a world position. Nothing here reaches the game, so
//! all of it is testable without one.
//!
//! # Four of §12's seven defects are corrected here
//!
//! - **Defect 1** is in [`crate::slots::Grid::index`]: the row is multiplied by
//!   the grid's width.
//! - **Defect 2** is in [`tiled`]: a mirrored coordinate is mirrored by the same
//!   extent it is reduced modulo, so a non-square grid is right on all six
//!   faces.
//! - **Defect 3** is in [`rotating_column`]: the axis is the block's own, not
//!   the occluding neighbour's appearance.
//! - **Defect 4** is in the continuation test: it is block identity, as §9.6
//!   already does it, rather than state reference equality.
//!
//! A parity test against the original disagrees on exactly these, which is why
//! each one is named at the assertion that pins it.
//!
//! # The coordinate tables
//!
//! §15 puts the exact edge positions, insets and rotation tables in the pinned
//! artifact, which this package never opened. Every number below is derived
//! from the face basis of [`crate::neighbourhood`] and from the shape the
//! specification describes in words. `NOTES.md` records the whole set as a
//! declared difference rather than a claim of parity.

use crate::conditions::BlockState;
use crate::neighbourhood::{Axis, Face, Neighbourhood, Neighbours, Offset, Probe};
use crate::quadrant::{Quadrant, select};
use crate::slots::{ColumnSlot, Grid, PaneEdge, Slot};

/// One sixteenth of a block, the unit every offset here is a multiple of.
pub const PART: f32 = 1.0 / 16.0;

/// Where the sheet of a thin horizontal slab sits above the block's floor.
pub const SHEET_HEIGHT: f32 = PART;

/// A quarter turn, in the ninety-degree steps a baked quad is rotated in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rotation {
    /// No rotation.
    #[default]
    None,
    /// A quarter turn.
    Quarter,
    /// A half turn.
    Half,
    /// Three quarters of a turn.
    ThreeQuarters,
}

impl Rotation {
    /// Every rotation, in the order this enum declares them.
    pub const ALL: [Self; 4] = [Self::None, Self::Quarter, Self::Half, Self::ThreeQuarters];

    /// Returns the rotation in ninety-degree steps.
    #[must_use]
    pub const fn steps(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Quarter => 1,
            Self::Half => 2,
            Self::ThreeQuarters => 3,
        }
    }

    /// Returns the rotation a column running along `axis` needs on `face`.
    ///
    /// A column's texture runs up the face when the axis projects onto the
    /// face's up vector and across it when it projects onto the right vector.
    /// A face the axis points through has no in-plane projection at all and
    /// takes no rotation.
    #[must_use]
    pub fn for_column(face: Face, axis: Axis) -> Self {
        if face.axis() == axis {
            return Self::None;
        }
        if face.up().axis() == Some(axis) {
            Self::None
        } else {
            Self::Quarter
        }
    }
}

/// The four edges of a quad, in unit face coordinates.
///
/// `u` runs along [`Face::right`] and `v` along [`Face::up`], so a quad's
/// coordinates mean the same thing on every face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edges {
    /// The lower `u` edge.
    pub min_u: f32,
    /// The upper `u` edge.
    pub max_u: f32,
    /// The lower `v` edge.
    pub min_v: f32,
    /// The upper `v` edge.
    pub max_v: f32,
}

impl Edges {
    /// The whole face.
    pub const FULL: Self = Self {
        min_u: 0.0,
        max_u: 1.0,
        min_v: 0.0,
        max_v: 1.0,
    };

    /// Builds edges, refusing anything that is not a usable rectangle.
    ///
    /// A value that is not finite is refused here rather than carried: a NaN
    /// edge compares false against every bound, so a quad holding one would
    /// pass every later test by accident.
    #[must_use]
    pub fn new(min_u: f32, max_u: f32, min_v: f32, max_v: f32) -> Option<Self> {
        let edges = Self {
            min_u,
            max_u,
            min_v,
            max_v,
        };
        edges.is_usable().then_some(edges)
    }

    /// Returns whether every edge is finite, inside the face and ordered.
    #[must_use]
    pub fn is_usable(&self) -> bool {
        let inside = |value: f32| value.is_finite() && (0.0..=1.0).contains(&value);
        inside(self.min_u)
            && inside(self.max_u)
            && inside(self.min_v)
            && inside(self.max_v)
            && self.min_u <= self.max_u
            && self.min_v <= self.max_v
    }
}

/// One quad of one face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    slot: Slot,
    edges: Edges,
    rotation: Rotation,
    inset: f32,
    cullable: bool,
}

impl Quad {
    /// Builds a quad, refusing edges or an inset that are not usable.
    ///
    /// Cullability defaults to "cullable exactly when the inset depth is zero",
    /// which is what makes a flush quad take part in the game's face culling
    /// and an inset one not. A non-finite inset is refused rather than
    /// defaulted, because it would answer that question by accident.
    #[must_use]
    pub fn new(slot: Slot, edges: Edges, rotation: Rotation, inset: f32) -> Option<Self> {
        if !edges.is_usable() || !inset.is_finite() || !(0.0..=1.0).contains(&inset) {
            return None;
        }
        Some(Self {
            slot,
            edges,
            rotation,
            inset,
            cullable: inset == 0.0,
        })
    }

    /// Returns the same quad with its cullability stated rather than derived.
    #[must_use]
    pub const fn cullable(mut self, cullable: bool) -> Self {
        self.cullable = cullable;
        self
    }

    /// Returns the texture slot this quad draws.
    #[must_use]
    pub const fn slot(&self) -> Slot {
        self.slot
    }

    /// Returns the quad's edges.
    #[must_use]
    pub const fn edges(&self) -> Edges {
        self.edges
    }

    /// Returns the quad's rotation.
    #[must_use]
    pub const fn rotation(&self) -> Rotation {
        self.rotation
    }

    /// Returns how far the quad is set back from the face.
    #[must_use]
    pub const fn inset(&self) -> f32 {
        self.inset
    }

    /// Returns whether the quad takes part in face culling.
    #[must_use]
    pub const fn is_cullable(&self) -> bool {
        self.cullable
    }

    /// Builds a quad from values this crate knows are usable.
    ///
    /// Every call site passes constants. A value that is not usable would be a
    /// packaging fault rather than input, so it produces no quad rather than
    /// a quad nobody checked.
    fn fixed(slot: Slot, edges: Edges, rotation: Rotation, inset: f32) -> Option<Self> {
        Self::new(slot, edges, rotation, inset)
    }
}

/// Which of a thin sheet's four horizontal sides are joined.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SheetFlags {
    /// Joins towards negative and positive `z`, respectively.
    pub north_south: [bool; 2],
    /// Joins towards negative and positive `x`, respectively.
    pub west_east: [bool; 2],
}

impl std::fmt::Debug for SheetFlags {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SheetFlags")
            .field("north", &self.north_south[0])
            .field("south", &self.north_south[1])
            .field("west", &self.west_east[0])
            .field("east", &self.west_east[1])
            .finish()
    }
}

impl SheetFlags {
    /// The names a state spells the four flags by, in [`SheetFlags::get`] order.
    pub const NAMES: [&'static str; 4] = ["north", "south", "west", "east"];

    /// Builds flags from the north/south and west/east axis pairs.
    #[must_use]
    pub const fn new(north_south: [bool; 2], west_east: [bool; 2]) -> Self {
        Self {
            north_south,
            west_east,
        }
    }

    /// Returns the flag for `face`, which is false on a vertical face.
    #[must_use]
    pub const fn get(self, face: Face) -> bool {
        match face {
            Face::North => self.north_south[0],
            Face::South => self.north_south[1],
            Face::West => self.west_east[0],
            Face::East => self.west_east[1],
            Face::Down | Face::Up => false,
        }
    }

    /// Returns the flags with the two depth-axis flags exchanged.
    ///
    /// §9.3 swaps them on the face that points down, which is what keeps a cap
    /// drawn from below the mirror image of the one drawn from above.
    #[must_use]
    pub const fn with_depth_swapped(self) -> Self {
        Self {
            north_south: [self.north_south[1], self.north_south[0]],
            west_east: self.west_east,
        }
    }

    /// Reads the flags a block state carries under [`SheetFlags::NAMES`].
    #[must_use]
    pub fn read(state: &BlockState) -> Self {
        let set = |name: &str| state.property(name) == Some("true");
        Self {
            north_south: [set("north"), set("south")],
            west_east: [set("west"), set("east")],
        }
    }

    /// Returns whether both flags perpendicular to `face` in its own plane are
    /// set, which is the pane connection rule of §9.3.
    ///
    /// A vertical face has no perpendicular pair, and §9.3 gives it no
    /// connection rule at all: a vertical pane face is capped or culled before
    /// any neighbour is sampled. It answers false rather than inventing one.
    #[must_use]
    pub fn runs_across(self, face: Face) -> bool {
        match perpendicular(face) {
            Some((first, second)) => self.get(first) && self.get(second),
            None => false,
        }
    }
}

/// Takes §9.3's eight samples around one pane face.
///
/// This is the pane kinds' whole connection rule and it is not the condition
/// language of §7: a neighbour connects only if it is the same block as `own`
/// *and* both of its two in-plane perpendicular flags are set, which is the
/// neighbouring pane running across this face rather than into it. §8's guard
/// still applies first, so an empty appearance never connects.
///
/// A vertical face is capped or culled by [`connected_pane`] before anything is
/// sampled, so it answers that nothing connects.
#[must_use]
pub fn pane_sample(face: Face, own: &BlockState, neighbourhood: &dyn Neighbourhood) -> Neighbours {
    if face.is_vertical() {
        return Neighbours::NONE;
    }
    let connects = |offset: Offset| {
        let sampled = neighbourhood.appearance(offset);
        !sampled.is_nothing()
            && sampled.same_block(own)
            && SheetFlags::read(&sampled).runs_across(face)
    };
    let mut neighbours = Neighbours::NONE;
    for index in 0..4 {
        neighbours.orthogonal[index] = connects(face.orthogonal(index));
        neighbours.diagonal[index] = connects(face.diagonal(index));
    }
    neighbours
}

/// Reads the pane state one face is drawn from.
///
/// The flags are the block's own and the identity test is §9.3's: a vertical
/// face is culled outright when the position in front of it holds the identical
/// state.
#[must_use]
pub fn pane_sheet(face: Face, own: &BlockState, neighbourhood: &dyn Neighbourhood) -> Sheet {
    Sheet::new(
        SheetFlags::read(own),
        &neighbourhood.appearance(face.normal()) == own,
    )
}

/// Returns the two horizontal faces whose normals lie along `face`'s right
/// vector, or `None` when `face` is vertical.
#[must_use]
pub fn perpendicular(face: Face) -> Option<(Face, Face)> {
    if face.is_vertical() {
        return None;
    }
    let right = face.right();
    let by_normal = |offset: Offset| Face::ALL.into_iter().find(|held| held.normal() == offset);
    match (by_normal(right), by_normal(right.reversed())) {
        (Some(first), Some(second)) => Some((first, second)),
        _ => None,
    }
}

/// §9.1. Quadrant connection on a full cube.
#[must_use]
pub fn connected_block(probe: Probe) -> Vec<Quad> {
    let Some(neighbours) = probe.neighbours() else {
        return Vec::new();
    };
    quadrant_quads(neighbours, Quadrant::Surrounded, 0.0, 0.5)
}

/// §9.2. Quadrant connection on a thin horizontal slab.
///
/// This kind performs no occlusion cull at all, which is why it takes the
/// eight answers rather than a [`Probe`]: a face is never dropped for what sits
/// in front of it.
#[must_use]
pub fn connected_carpet(face: Face, neighbours: Neighbours) -> Vec<Quad> {
    if !face.is_vertical() {
        // The sheet's rim, one sixteenth tall along the bottom of the side.
        return Quad::fixed(
            Slot::Quadrant(Quadrant::Isolated),
            Edges {
                min_u: 0.0,
                max_u: 1.0,
                min_v: 0.0,
                max_v: SHEET_HEIGHT,
            },
            Rotation::None,
            0.0,
        )
        .into_iter()
        .collect();
    }

    // The slab is one sixteenth thick and rests on the block's floor, so its
    // top quad stands fifteen sixteenths back from the block's top face and its
    // bottom quad is flush with the block's bottom. The two insets differ by
    // exactly the thickness the side rim spans, and the flush one is cullable
    // against whatever the slab rests on.
    let inset = if face == Face::Up {
        1.0 - SHEET_HEIGHT
    } else {
        0.0
    };
    // The all-eight case draws from a different slot on the two faces.
    let surrounded = if face == Face::Up {
        Quadrant::Surrounded
    } else {
        Quadrant::Isolated
    };
    quadrant_quads(neighbours, surrounded, inset, 0.5)
}

/// §9.4. A texture grid tiled across world coordinates.
///
/// No neighbour is consulted, so the output depends on where the block sits
/// rather than on what is beside it.
#[must_use]
pub fn tiled(face: Face, position: Offset, grid: &Grid) -> Vec<Quad> {
    let column = tiled_coordinate(position, face.right(), grid.width(), face.mirrors_column());
    let row = tiled_coordinate(position, face.up(), grid.height(), face.mirrors_row());
    Quad::fixed(
        Slot::Cell(grid.index(column, row)),
        Edges::FULL,
        Rotation::None,
        0.0,
    )
    .into_iter()
    .collect()
}

/// Reduces one world coordinate to a cell coordinate.
///
/// §12's defect 2 is that the original mirrors by the grid's width while
/// reducing modulo the grid's height, so the two vertical-axis faces are wrong
/// on a non-square grid even once defect 1 is fixed. Here `extent` is both the
/// modulus and the mirroring extent, so the two cannot disagree.
#[must_use]
pub fn tiled_coordinate(position: Offset, vector: Offset, extent: u32, mirrored: bool) -> u32 {
    let Some(axis) = vector.axis() else {
        return 0;
    };
    if extent == 0 {
        return 0;
    }
    let modulus = i64::from(extent);
    let reduced = i64::from(position.along(axis)).rem_euclid(modulus);
    let reduced = u32::try_from(reduced).unwrap_or(0);
    if mirrored {
        extent.saturating_sub(1).saturating_sub(reduced)
    } else {
        reduced
    }
}

/// The two neighbours a column continues into, and which of them are there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Continuation {
    /// Whether the neighbour along the positive direction of the axis is the
    /// same block.
    pub positive: bool,
    /// Whether the neighbour along the negative direction of the axis is.
    pub negative: bool,
}

impl Continuation {
    /// Builds a continuation answer.
    #[must_use]
    pub const fn new(positive: bool, negative: bool) -> Self {
        Self { positive, negative }
    }

    /// Returns the column slot the pair selects.
    #[must_use]
    pub const fn slot(self) -> ColumnSlot {
        match (self.positive, self.negative) {
            (true, true) => ColumnSlot::Continuation,
            (true, false) => ColumnSlot::PositiveEnd,
            (false, true) => ColumnSlot::NegativeEnd,
            (false, false) => ColumnSlot::Standalone,
        }
    }
}

/// §9.5. Axis-aware continuation, rotated per face.
///
/// `axis` is the block's *own* axis property. §12's defect 3 is that the
/// original reads it from the occluding neighbour's appearance, so the column's
/// rendering is decided by what sits in front of each face. §12's defect 4 is
/// that the original tests continuation by state reference equality; the caller
/// here answers [`Continuation`] by block identity, as §9.6 already does.
#[must_use]
pub fn rotating_column(face: Face, axis: Option<Axis>, continuation: Continuation) -> Vec<Quad> {
    let Some(axis) = axis else {
        return column_quad(ColumnSlot::Standalone, Rotation::None);
    };
    if axis == face.axis() {
        return column_quad(ColumnSlot::Cap, Rotation::None);
    }
    column_quad(continuation.slot(), Rotation::for_column(face, axis))
}

/// §9.6. As §9.5 with block identity, no rotation, and an occlusion cull.
#[must_use]
pub fn upright_column(
    face: Face,
    probe: Probe,
    axis: Option<Axis>,
    continuation: Continuation,
) -> Vec<Quad> {
    if probe.neighbours().is_none() {
        return Vec::new();
    }
    if face.is_vertical() {
        // The cap is returned without consulting an axis property at all.
        return column_quad(ColumnSlot::Cap, Rotation::None);
    }
    let Some(axis) = axis else {
        return column_quad(ColumnSlot::Standalone, Rotation::None);
    };
    if axis == face.axis() {
        return column_quad(ColumnSlot::Cap, Rotation::None);
    }
    column_quad(continuation.slot(), Rotation::None)
}

/// What a pane face needs to know beyond its eight neighbours.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Sheet {
    /// The block's own four directional flags.
    pub flags: SheetFlags,
    /// Whether the position in front of the face holds the identical state.
    pub front_identical: bool,
}

impl Sheet {
    /// Builds the pane state one face is drawn from.
    #[must_use]
    pub const fn new(flags: SheetFlags, front_identical: bool) -> Self {
        Self {
            flags,
            front_identical,
        }
    }
}

/// §9.3. Quadrant connection on a pane, driven by state flags.
#[must_use]
pub fn connected_pane(face: Face, sheet: Sheet, neighbours: Neighbours) -> Vec<Quad> {
    pane_quads(face, sheet, neighbours, None)
}

/// §9.7. The pane geometry of §9.3 with the slot selection of §9.5.
///
/// The quads and the in-plane flag branching come from the pane kind; the slot
/// index comes from the column continuation. A vertical face returns the same
/// cap as §9.3, so the continuation does not reach it.
#[must_use]
pub fn pane_column(
    face: Face,
    sheet: Sheet,
    neighbours: Neighbours,
    continuation: Continuation,
) -> Vec<Quad> {
    pane_quads(face, sheet, neighbours, Some(continuation.slot()))
}

fn pane_quads(
    face: Face,
    sheet: Sheet,
    neighbours: Neighbours,
    column: Option<ColumnSlot>,
) -> Vec<Quad> {
    if face.is_vertical() {
        if sheet.front_identical {
            return Vec::new();
        }
        return pane_cap(face, sheet.flags);
    }

    let recolour = |quads: Vec<Quad>| match column {
        None => quads,
        Some(slot) => quads
            .into_iter()
            .map(|quad| Quad {
                slot: Slot::Column(slot),
                ..quad
            })
            .collect(),
    };

    // All eight connecting returns one full-face centre quad before any
    // in-plane flag is consulted.
    if neighbours.all_connected() {
        return recolour(
            Quad::fixed(
                Slot::Quadrant(Quadrant::Surrounded),
                Edges::FULL,
                Rotation::None,
                PANE_INSET,
            )
            .into_iter()
            .collect(),
        );
    }

    let Some((first, second)) = perpendicular(face) else {
        return Vec::new();
    };
    let (first_set, second_set) = (sheet.flags.get(first), sheet.flags.get(second));
    let own = sheet.flags.get(face);

    let quads = match (first_set, second_set) {
        (true, true) => {
            let inner = if own { 0.5 - PART } else { 0.5 };
            quadrant_quads(neighbours, Quadrant::Surrounded, PANE_INSET, inner)
        }
        (true, false) => side_quad(true),
        (false, true) => side_quad(false),
        (false, false) => {
            if !sheet.front_identical && !own {
                Quad::fixed(
                    Slot::Quadrant(Quadrant::Isolated),
                    Edges {
                        min_u: 0.5 - PART,
                        max_u: 0.5 + PART,
                        min_v: 0.0,
                        max_v: 1.0,
                    },
                    Rotation::None,
                    PANE_INSET,
                )
                .into_iter()
                .collect()
            } else {
                Vec::new()
            }
        }
    };
    recolour(quads)
}

/// How far a pane's sheet sits back from the face it is drawn against.
pub const PANE_INSET: f32 = 7.0 * PART;

fn side_quad(towards_first: bool) -> Vec<Quad> {
    let (min_u, max_u, slot) = if towards_first {
        (0.5 - PART, 1.0, Quadrant::FirstEdge)
    } else {
        (0.0, 0.5 + PART, Quadrant::SecondEdge)
    };
    Quad::fixed(
        Slot::Quadrant(slot),
        Edges {
            min_u,
            max_u,
            min_v: 0.0,
            max_v: 1.0,
        },
        Rotation::None,
        PANE_INSET,
    )
    .into_iter()
    .collect()
}

fn pane_cap(face: Face, flags: SheetFlags) -> Vec<Quad> {
    let flags = if face == Face::Down {
        flags.with_depth_swapped()
    } else {
        flags
    };
    let mut quads = Vec::with_capacity(5);
    quads.extend(Quad::fixed(
        Slot::Edge(PaneEdge::Rim),
        Edges {
            min_u: 0.5 - PART,
            max_u: 0.5 + PART,
            min_v: 0.5 - PART,
            max_v: 0.5 + PART,
        },
        Rotation::None,
        0.0,
    ));
    for arm in [Face::North, Face::South, Face::West, Face::East] {
        if !flags.get(arm) {
            continue;
        }
        let Some(edges) = arm_edges(face, arm) else {
            continue;
        };
        quads.extend(Quad::fixed(
            Slot::Edge(PaneEdge::Flank),
            edges,
            Rotation::None,
            0.0,
        ));
    }
    quads
}

/// Returns the strip from the cap's centre out to the edge in `arm`'s
/// direction, in `face`'s own coordinates.
fn arm_edges(face: Face, arm: Face) -> Option<Edges> {
    let normal = arm.normal();
    let right = face.right();
    let up = face.up();
    let along_right = dot(normal, right);
    let along_up = dot(normal, up);
    match (along_right, along_up) {
        (1, 0) => Edges::new(0.5 + PART, 1.0, 0.5 - PART, 0.5 + PART),
        (-1, 0) => Edges::new(0.0, 0.5 - PART, 0.5 - PART, 0.5 + PART),
        (0, 1) => Edges::new(0.5 - PART, 0.5 + PART, 0.5 + PART, 1.0),
        (0, -1) => Edges::new(0.5 - PART, 0.5 + PART, 0.0, 0.5 - PART),
        _ => None,
    }
}

const fn dot(first: Offset, second: Offset) -> i32 {
    first.x * second.x + first.y * second.y + first.z * second.z
}

fn column_quad(slot: ColumnSlot, rotation: Rotation) -> Vec<Quad> {
    Quad::fixed(Slot::Column(slot), Edges::FULL, rotation, 0.0)
        .into_iter()
        .collect()
}

/// The shared body of §9.1, §9.2 and the both-flags branch of §9.3.
///
/// All eight connecting returns one full-face quad in `surrounded`; anything
/// else returns one quad per quadrant, each selected by §6 from its own two
/// orthogonals and their shared diagonal.
fn quadrant_quads(
    neighbours: Neighbours,
    surrounded: Quadrant,
    inset: f32,
    inner: f32,
) -> Vec<Quad> {
    if neighbours.all_connected() {
        return Quad::fixed(
            Slot::Quadrant(surrounded),
            Edges::FULL,
            Rotation::None,
            inset,
        )
        .into_iter()
        .collect();
    }
    let mut quads = Vec::with_capacity(4);
    for index in 0..4 {
        let (first, second, diagonal) = neighbours.quadrant(index);
        let slot = select(first, second, diagonal);
        let Some(edges) = quadrant_edges(index, inner) else {
            continue;
        };
        quads.extend(Quad::fixed(
            Slot::Quadrant(slot),
            edges,
            Rotation::None,
            inset,
        ));
    }
    quads
}

/// Returns the region quadrant `index` covers, with its inner edges at `inner`.
///
/// Quadrant zero is the corner the face's right and up vectors both point into
/// and the four run anticlockwise from there, which is the same cycle
/// [`Face::orthogonal`] uses.
#[must_use]
pub fn quadrant_edges(index: usize, inner: f32) -> Option<Edges> {
    let far = 1.0 - inner;
    match index % 4 {
        0 => Edges::new(inner, 1.0, inner, 1.0),
        1 => Edges::new(0.0, far, inner, 1.0),
        2 => Edges::new(0.0, far, 0.0, far),
        _ => Edges::new(inner, 1.0, 0.0, far),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        Continuation, Edges, PANE_INSET, PART, Quad, Rotation, SHEET_HEIGHT, Sheet, SheetFlags,
        arm_edges, connected_block, connected_carpet, connected_pane, pane_column, pane_sample,
        pane_sheet, perpendicular, rotating_column, tiled, tiled_coordinate, upright_column,
    };
    use crate::conditions::BlockState;
    use crate::neighbourhood::{Axis, Face, Neighbourhood, Neighbours, Offset, Probe};
    use crate::quadrant::Quadrant;
    use crate::slots::{ColumnSlot, Grid, PaneEdge, Slot, TextureId};

    fn drawn(neighbours: Neighbours) -> Probe {
        Probe::Drawn(neighbours)
    }

    /// A neighbourhood written out position by position.
    #[derive(Default)]
    struct Written {
        centre: BlockState,
        around: BTreeMap<Offset, BlockState>,
    }

    impl Written {
        fn of(centre: BlockState) -> Self {
            Self {
                centre,
                around: BTreeMap::new(),
            }
        }

        fn at(mut self, offset: Offset, state: BlockState) -> Self {
            self.around.insert(offset, state);
            self
        }

        fn all_around(mut self, face: Face, state: &BlockState) -> Self {
            for index in 0..4 {
                self.around.insert(face.orthogonal(index), state.clone());
                self.around.insert(face.diagonal(index), state.clone());
            }
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
    }

    fn pane(north: &str, south: &str, west: &str, east: &str) -> BlockState {
        BlockState::new("base:pane")
            .with("north", north)
            .with("south", south)
            .with("west", west)
            .with("east", east)
    }

    /// A pane whose named flags are set and whose other three are clear.
    fn flagged(set: [Face; 2]) -> BlockState {
        let value = |face: Face| if set.contains(&face) { "true" } else { "false" };
        pane(
            value(Face::North),
            value(Face::South),
            value(Face::West),
            value(Face::East),
        )
    }

    #[test]
    fn a_pane_neighbour_that_runs_into_the_face_does_not_connect() {
        for face in Face::ALL {
            let Some((first, second)) = perpendicular(face) else {
                continue;
            };
            let own = flagged([first, second]);

            // The neighbour's sheet runs into the face rather than across it:
            // its two perpendicular flags are clear. §9.3 says it never
            // connects, however plainly it is the same block.
            let into = flagged([face, opposite(face)]);
            assert!(!SheetFlags::read(&into).runs_across(face), "{face:?}");
            let neighbours = pane_sample(
                face,
                &own,
                &Written::of(own.clone()).all_around(face, &into),
            );
            assert_eq!(
                neighbours,
                Neighbours::NONE,
                "{face:?}: a neighbour running into the face connects"
            );
            assert_ne!(
                connected_pane(
                    face,
                    pane_sheet(face, &own, &Written::of(own.clone())),
                    neighbours
                )
                .len(),
                0,
                "{face:?}: and the face is still drawn, from its own two flags"
            );

            // The same block running across the face is the case that does.
            let across = flagged([first, second]);
            let connected = pane_sample(
                face,
                &own,
                &Written::of(own.clone()).all_around(face, &across),
            );
            assert!(connected.all_connected(), "{face:?}");

            // A different block running across it does not.
            let other = BlockState::new("base:other")
                .with("north", "true")
                .with("south", "true")
                .with("west", "true")
                .with("east", "true");
            assert_eq!(
                pane_sample(
                    face,
                    &own,
                    &Written::of(own.clone()).all_around(face, &other)
                ),
                Neighbours::NONE,
                "{face:?}: the rule is same block *and* both flags"
            );
        }
    }

    #[test]
    fn a_pane_samples_nothing_on_a_vertical_face_and_asks_whether_it_is_capped() {
        let own = flagged([Face::West, Face::East]);
        for face in [Face::Up, Face::Down] {
            assert!(
                !SheetFlags::read(&own).runs_across(face),
                "a vertical face has no perpendicular pair to run across"
            );
            let written = Written::of(own.clone()).all_around(face, &own);
            assert_eq!(pane_sample(face, &own, &written), Neighbours::NONE);

            // §9.3 culls a vertical face when the block in front holds the
            // identical state, and caps it otherwise.
            let identical = Written::of(own.clone()).at(face.normal(), own.clone());
            assert!(pane_sheet(face, &own, &identical).front_identical);
            assert_eq!(
                connected_pane(face, pane_sheet(face, &own, &identical), Neighbours::NONE).len(),
                0
            );

            let differing =
                Written::of(own.clone()).at(face.normal(), flagged([Face::North, Face::South]));
            assert!(!pane_sheet(face, &own, &differing).front_identical);
            assert_ne!(
                connected_pane(face, pane_sheet(face, &own, &differing), Neighbours::NONE).len(),
                0
            );
        }
    }

    fn opposite(face: Face) -> Face {
        Face::ALL
            .into_iter()
            .find(|held| held.normal() == face.normal().reversed())
            .expect("every face has an opposite")
    }

    fn slots(quads: &[Quad]) -> Vec<Slot> {
        quads.iter().map(Quad::slot).collect()
    }

    #[test]
    fn a_culled_face_produces_no_quad_at_all() {
        assert_eq!(connected_block(Probe::Culled).len(), 0);
        assert_eq!(
            upright_column(
                Face::North,
                Probe::Culled,
                Some(Axis::Y),
                Continuation::default()
            )
            .len(),
            0
        );
    }

    #[test]
    fn a_fully_surrounded_face_is_one_quad_and_anything_else_is_four() {
        let all = connected_block(drawn(Neighbours::ALL));
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].slot(), Slot::Quadrant(Quadrant::Surrounded));
        assert_eq!(all[0].edges(), Edges::FULL);
        assert!(all[0].is_cullable(), "a flush quad takes part in culling");

        let none = connected_block(drawn(Neighbours::NONE));
        assert_eq!(none.len(), 4);
        assert_eq!(slots(&none), vec![Slot::Quadrant(Quadrant::Isolated); 4]);

        // One orthogonal connecting changes exactly the two quadrants it bounds.
        let mut one = Neighbours::NONE;
        one.orthogonal[1] = true;
        let quads = connected_block(drawn(one));
        assert_eq!(
            slots(&quads),
            vec![
                Slot::Quadrant(Quadrant::SecondEdge),
                Slot::Quadrant(Quadrant::FirstEdge),
                Slot::Quadrant(Quadrant::Isolated),
                Slot::Quadrant(Quadrant::Isolated),
            ]
        );
    }

    #[test]
    fn the_four_quadrants_tile_the_face_without_overlapping() {
        let quads = connected_block(drawn(Neighbours::NONE));
        let area: f32 = quads
            .iter()
            .map(|quad| {
                let edges = quad.edges();
                (edges.max_u - edges.min_u) * (edges.max_v - edges.min_v)
            })
            .sum();
        assert!((area - 1.0).abs() < 1e-6, "four quarters cover the face");
        for quad in &quads {
            assert!(quad.edges().is_usable());
        }
    }

    #[test]
    fn a_carpets_side_is_a_thin_rim_and_its_two_flat_faces_are_inset_differently() {
        for face in [Face::North, Face::South, Face::West, Face::East] {
            let quads = connected_carpet(face, Neighbours::NONE);
            assert_eq!(quads.len(), 1, "{face:?}");
            assert!((quads[0].edges().max_v - PART).abs() < 1e-6, "{face:?}");
        }

        let top = connected_carpet(Face::Up, Neighbours::ALL);
        let bottom = connected_carpet(Face::Down, Neighbours::ALL);
        assert_eq!(top.len(), 1);
        assert_eq!(bottom.len(), 1);
        // The two insets are the two faces of one slab the thickness of the
        // side rim, so they sum with that thickness to the whole block. That
        // pins the shape rather than the two numbers: any pair that leaves the
        // slab thicker, thinner or off the floor fails it.
        assert!(
            (top[0].inset() + SHEET_HEIGHT + bottom[0].inset() - 1.0).abs() < 1e-6,
            "§9.2's two insets bound a slab one sixteenth thick resting on the floor"
        );
        assert_ne!(
            top[0].inset(),
            bottom[0].inset(),
            "§9.2 requires the two insets to differ"
        );
        assert_ne!(
            top[0].slot(),
            bottom[0].slot(),
            "and the all-eight case to draw from a different slot on each"
        );
        assert!(
            !top[0].is_cullable(),
            "the raised top quad does not take part in face culling"
        );
        assert!(
            bottom[0].is_cullable(),
            "and the flush bottom quad does, against whatever the slab rests on"
        );
    }

    #[test]
    fn a_carpet_draws_a_face_the_connected_block_kind_drops() {
        // §9.2 performs no occlusion cull at all. The comparison is what makes
        // that an assertion about behaviour rather than about a signature: on
        // the same occluded face, the kind of §9.1 produces nothing and this
        // one still draws.
        for face in Face::ALL {
            assert_eq!(connected_block(Probe::Culled).len(), 0, "{face:?}");
            assert_ne!(
                connected_carpet(face, Neighbours::NONE).len(),
                0,
                "{face:?}"
            );
        }
    }

    #[test]
    fn a_tiled_grid_addresses_a_declared_cell_from_every_world_position() {
        let cells: Vec<TextureId> = (0..6)
            .map(|n| TextureId::parse(&format!("base:c{n}")).expect("legal"))
            .collect();
        let grid = Grid::new(3, 2, cells).expect("a three by two grid");

        for x in -20..20 {
            for y in -20..20 {
                for z in -20..20 {
                    for face in Face::ALL {
                        let quads = tiled(face, Offset::new(x, y, z), &grid);
                        assert_eq!(quads.len(), 1);
                        let Slot::Cell(cell) = quads[0].slot() else {
                            panic!("a tiled face draws a cell");
                        };
                        assert!(cell < 6, "{face:?} at {x},{y},{z} reached cell {cell}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_mirrored_coordinate_is_mirrored_by_the_extent_it_was_reduced_by() {
        // Defect 2 of §12. The mirrored answer is the reduced one counted back
        // from the same extent, so it stays inside the grid for any extent.
        for extent in 1u32..8 {
            for value in -20i32..20 {
                let plain = tiled_coordinate(
                    Offset::new(value, 0, 0),
                    Offset::new(1, 0, 0),
                    extent,
                    false,
                );
                let mirrored = tiled_coordinate(
                    Offset::new(value, 0, 0),
                    Offset::new(-1, 0, 0),
                    extent,
                    true,
                );
                assert!(plain < extent);
                assert!(mirrored < extent);
                assert_eq!(
                    mirrored,
                    extent - 1 - plain,
                    "extent {extent} value {value}"
                );
            }
        }
    }

    #[test]
    fn a_non_square_grid_is_addressed_the_same_way_on_a_mirrored_face() {
        let cells: Vec<TextureId> = (0..8)
            .map(|n| TextureId::parse(&format!("base:c{n}")).expect("legal"))
            .collect();
        // Four across, two down: the case defects 1 and 2 both break.
        let grid = Grid::new(4, 2, cells).expect("a four by two grid");
        let mirrored: Vec<Face> = Face::ALL
            .into_iter()
            .filter(|face| face.mirrors_column() || face.mirrors_row())
            .collect();
        assert_eq!(mirrored.len(), 3);
        for face in mirrored {
            for step in 0..8 {
                let quads = tiled(face, Offset::new(step, step, step), &grid);
                let Slot::Cell(cell) = quads[0].slot() else {
                    panic!("a tiled face draws a cell");
                };
                assert!(cell < 8, "{face:?} step {step} reached cell {cell}");
            }
        }
    }

    #[test]
    fn a_column_reads_its_own_axis_and_never_the_face_in_front_of_it() {
        // Defect 3 of §12. The axis argument is the block's own, so there is no
        // argument through which a neighbour could supply one.
        assert_eq!(
            slots(&rotating_column(
                Face::North,
                None,
                Continuation::new(true, true)
            )),
            vec![Slot::Column(ColumnSlot::Standalone)],
            "no axis is standalone whatever the neighbours answer"
        );
        assert_eq!(
            slots(&rotating_column(
                Face::Up,
                Some(Axis::Y),
                Continuation::default()
            )),
            vec![Slot::Column(ColumnSlot::Cap)],
            "the face the axis points through is the cap"
        );
    }

    #[test]
    fn the_continuation_pair_selects_all_four_column_slots() {
        let expected = [
            ((true, true), ColumnSlot::Continuation),
            ((true, false), ColumnSlot::PositiveEnd),
            ((false, true), ColumnSlot::NegativeEnd),
            ((false, false), ColumnSlot::Standalone),
        ];
        for ((positive, negative), slot) in expected {
            let quads = rotating_column(
                Face::North,
                Some(Axis::Y),
                Continuation::new(positive, negative),
            );
            assert_eq!(slots(&quads), vec![Slot::Column(slot)]);
        }
    }

    #[test]
    fn a_rotating_column_turns_with_its_axis_and_an_upright_one_never_does() {
        let along_up = rotating_column(Face::North, Some(Axis::Y), Continuation::default());
        assert_eq!(along_up[0].rotation(), Rotation::None);
        let across = rotating_column(Face::North, Some(Axis::X), Continuation::default());
        assert_eq!(
            across[0].rotation(),
            Rotation::Quarter,
            "an axis that runs across the face turns the quad a quarter"
        );

        for face in Face::ALL {
            for axis in Axis::ALL {
                let quads = upright_column(
                    face,
                    Probe::Drawn(Neighbours::NONE),
                    Some(axis),
                    Continuation::default(),
                );
                for quad in quads {
                    assert_eq!(quad.rotation(), Rotation::None, "{face:?} {axis:?}");
                }
            }
        }
    }

    #[test]
    fn an_upright_columns_vertical_faces_are_capped_without_consulting_an_axis() {
        for face in [Face::Up, Face::Down] {
            for axis in [None, Some(Axis::X), Some(Axis::Y), Some(Axis::Z)] {
                let quads = upright_column(
                    face,
                    Probe::Drawn(Neighbours::NONE),
                    axis,
                    Continuation::new(true, true),
                );
                assert_eq!(
                    slots(&quads),
                    vec![Slot::Column(ColumnSlot::Cap)],
                    "{face:?} {axis:?}"
                );
            }
        }
    }

    #[test]
    fn the_two_faces_in_a_faces_own_plane_are_the_ones_its_right_vector_names() {
        for face in Face::ALL {
            match perpendicular(face) {
                None => assert!(face.is_vertical(), "{face:?}"),
                Some((first, second)) => {
                    assert_eq!(first.normal(), face.right(), "{face:?}");
                    assert_eq!(second, first.opposite(), "{face:?}");
                    assert!(!first.is_vertical() && !second.is_vertical(), "{face:?}");
                }
            }
        }
    }

    #[test]
    fn a_panes_vertical_face_is_culled_by_an_identical_front_and_capped_otherwise() {
        let flags = SheetFlags::new([true, false], [false, true]);
        assert_eq!(
            connected_pane(Face::Up, Sheet::new(flags, true), Neighbours::NONE).len(),
            0,
            "an identical state in front culls the face outright"
        );

        let cap = connected_pane(Face::Up, Sheet::new(flags, false), Neighbours::NONE);
        assert_eq!(cap.len(), 3, "one centre and one arm per set flag");
        assert_eq!(cap[0].slot(), Slot::Edge(PaneEdge::Rim));
        assert!(
            cap[1..]
                .iter()
                .all(|q| q.slot() == Slot::Edge(PaneEdge::Flank)),
            "the arms draw the flank texture"
        );
    }

    #[test]
    fn the_two_depth_flags_are_exchanged_on_the_face_that_points_down() {
        let flags = SheetFlags::new([true, false], [false, false]);
        let above = connected_pane(Face::Up, Sheet::new(flags, false), Neighbours::NONE);
        let below = connected_pane(Face::Down, Sheet::new(flags, false), Neighbours::NONE);
        assert_eq!(above.len(), 2);
        assert_eq!(below.len(), 2);
        assert_eq!(
            above[1].edges(),
            below[1].edges(),
            "the swap compensates the mirrored basis, so the arm the north flag \
             draws points the same way in the world from either side"
        );
        assert_ne!(
            arm_edges(Face::Up, Face::North),
            arm_edges(Face::Down, Face::North),
            "and without the swap it would not: the two faces' up vectors differ"
        );

        let symmetric = SheetFlags::new([true, true], [false, false]);
        assert_eq!(symmetric.with_depth_swapped(), symmetric);
        assert!(flags.with_depth_swapped().get(Face::South));
        assert!(!flags.with_depth_swapped().get(Face::North));
        assert_eq!(
            flags.with_depth_swapped().with_depth_swapped(),
            flags,
            "the swap is its own inverse"
        );
    }

    #[test]
    fn a_panes_horizontal_face_branches_on_its_two_in_plane_flags() {
        let face = Face::North;
        let (first, second) = perpendicular(face).expect("a horizontal face has a pair");

        let both = SheetFlags::new(
            [false, false],
            [first == Face::West || second == Face::West, true],
        );
        let quads = connected_pane(face, Sheet::new(both, false), Neighbours::NONE);
        assert_eq!(quads.len(), 4, "both flags set gives four quadrant quads");

        let one = SheetFlags::new([false, false], [first == Face::West, first == Face::East]);
        let quads = connected_pane(face, Sheet::new(one, false), Neighbours::NONE);
        assert_eq!(quads.len(), 1, "exactly one flag gives one side quad");
        assert_eq!(quads[0].slot(), Slot::Quadrant(Quadrant::FirstEdge));

        let neither = SheetFlags::default();
        let quads = connected_pane(face, Sheet::new(neither, false), Neighbours::NONE);
        assert_eq!(quads.len(), 1, "neither flag gives one narrow strip");
        assert!((quads[0].edges().max_u - quads[0].edges().min_u - 2.0 * PART).abs() < 1e-6);

        let own_set = SheetFlags::new([true, false], [false, false]);
        assert_eq!(
            connected_pane(face, Sheet::new(own_set, false), Neighbours::NONE).len(),
            0,
            "the strip is not drawn when the face's own flag is set"
        );
        assert_eq!(
            connected_pane(face, Sheet::new(neither, true), Neighbours::NONE).len(),
            0,
            "nor when the block in front is identical"
        );
    }

    #[test]
    fn all_eight_connecting_returns_one_centre_quad_before_any_flag_is_read() {
        let face = Face::North;
        for flags in [
            SheetFlags::default(),
            SheetFlags::new([true, true], [true, true]),
        ] {
            let quads = connected_pane(face, Sheet::new(flags, false), Neighbours::ALL);
            assert_eq!(quads.len(), 1, "{flags:?}");
            assert_eq!(quads[0].slot(), Slot::Quadrant(Quadrant::Surrounded));
            assert_eq!(quads[0].edges(), Edges::FULL);
            assert_eq!(quads[0].inset(), PANE_INSET);
        }
    }

    #[test]
    fn the_both_flags_branch_shifts_its_inner_edge_by_a_sixteenth() {
        let face = Face::North;
        let (first, second) = perpendicular(face).expect("a pair");
        let mut flags = SheetFlags::default();
        for held in [first, second] {
            match held {
                Face::West => flags.west_east[0] = true,
                Face::East => flags.west_east[1] = true,
                Face::North => flags.north_south[0] = true,
                Face::South => flags.north_south[1] = true,
                Face::Up | Face::Down => {}
            }
        }
        let clear = connected_pane(face, Sheet::new(flags, false), Neighbours::NONE);

        let mut set = flags;
        set.north_south[0] = true;
        let shifted = connected_pane(face, Sheet::new(set, false), Neighbours::NONE);

        assert_eq!(clear.len(), 4);
        assert_eq!(shifted.len(), 4);
        let inner_clear = clear[0].edges().min_u;
        let inner_shifted = shifted[0].edges().min_u;
        assert!((inner_clear - inner_shifted - PART).abs() < 1e-6);
    }

    #[test]
    fn a_pane_column_keeps_the_pane_shapes_and_takes_the_column_slot() {
        let face = Face::North;
        let flags = SheetFlags::new([false, false], [true, true]);
        let sheet = Sheet::new(flags, false);
        let pane = connected_pane(face, sheet, Neighbours::NONE);
        let column = pane_column(
            face,
            sheet,
            Neighbours::NONE,
            Continuation::new(true, false),
        );

        assert_eq!(pane.len(), column.len(), "the shapes are the pane's");
        for (from_pane, from_column) in pane.iter().zip(&column) {
            assert_eq!(from_pane.edges(), from_column.edges());
            assert_eq!(from_pane.inset(), from_column.inset());
        }
        assert!(
            column
                .iter()
                .all(|quad| quad.slot() == Slot::Column(ColumnSlot::PositiveEnd)),
            "and the slot is the column's"
        );

        let cap = pane_column(
            Face::Up,
            sheet,
            Neighbours::NONE,
            Continuation::new(true, true),
        );
        assert_eq!(
            cap,
            connected_pane(Face::Up, sheet, Neighbours::NONE),
            "a vertical face returns the same cap as the pane kind"
        );
    }

    #[test]
    fn a_quad_whose_geometry_is_not_a_number_is_refused_rather_than_carried() {
        let slot = Slot::Quadrant(Quadrant::Isolated);
        assert_eq!(Edges::new(f32::NAN, 1.0, 0.0, 1.0), None);
        assert_eq!(Edges::new(0.0, f32::INFINITY, 0.0, 1.0), None);
        assert_eq!(Edges::new(0.6, 0.4, 0.0, 1.0), None, "unordered edges");
        assert_eq!(Edges::new(-0.1, 1.0, 0.0, 1.0), None, "outside the face");

        assert_eq!(
            Quad::new(slot, Edges::FULL, Rotation::None, f32::NAN),
            None,
            "a NaN inset would answer the cullability question by accident"
        );
        assert_eq!(
            Quad::new(slot, Edges::FULL, Rotation::None, f32::INFINITY),
            None
        );

        let flush = Quad::new(slot, Edges::FULL, Rotation::None, 0.0).expect("usable");
        let inset = Quad::new(slot, Edges::FULL, Rotation::None, PART).expect("usable");
        assert!(flush.is_cullable());
        assert!(!inset.is_cullable());
        assert!(inset.cullable(true).is_cullable(), "and it can be stated");
    }

    #[test]
    fn every_quad_every_kind_produces_is_a_usable_rectangle() {
        let cells: Vec<TextureId> = (0..6)
            .map(|n| TextureId::parse(&format!("base:c{n}")).expect("legal"))
            .collect();
        let grid = Grid::new(3, 2, cells).expect("a grid");
        let mut produced = 0usize;

        for face in Face::ALL {
            for neighbours in [Neighbours::NONE, Neighbours::ALL] {
                let mut batches = vec![
                    connected_block(Probe::Drawn(neighbours)),
                    connected_carpet(face, neighbours),
                    tiled(face, Offset::new(3, -7, 11), &grid),
                ];
                for axis in [None, Some(Axis::X), Some(Axis::Y), Some(Axis::Z)] {
                    batches.push(rotating_column(face, axis, Continuation::new(true, false)));
                    batches.push(upright_column(
                        face,
                        Probe::Drawn(neighbours),
                        axis,
                        Continuation::new(false, true),
                    ));
                }
                for north in [false, true] {
                    for east in [false, true] {
                        let flags = SheetFlags::new([north, false], [false, east]);
                        let sheet = Sheet::new(flags, false);
                        batches.push(connected_pane(face, sheet, neighbours));
                        batches.push(pane_column(
                            face,
                            sheet,
                            neighbours,
                            Continuation::new(north, east),
                        ));
                    }
                }
                for batch in batches {
                    for quad in batch {
                        assert!(quad.edges().is_usable(), "{face:?} {quad:?}");
                        assert!(quad.inset().is_finite());
                        assert_eq!(quad.is_cullable(), quad.inset() == 0.0);
                        produced += 1;
                    }
                }
            }
        }
        assert!(produced > 200, "the sweep produced {produced} quads");
    }
}
