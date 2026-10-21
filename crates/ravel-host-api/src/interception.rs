use std::fmt;

use ravel_core::{ModId, Side};

/// A published host decision point; every point executes on the world side,
/// including the integrated-server side of a client process.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Point {
    /// A block is about to be broken.
    BlockIsBroken,
    /// An item is about to be used.
    ItemIsUsed,
    /// A world is about to be saved.
    WorldIsSaved,
}

impl Point {
    /// Every published point, in the order this enum declares them.
    pub const ALL: [Self; 3] = [Self::BlockIsBroken, Self::ItemIsUsed, Self::WorldIsSaved];

    /// Returns the point's published name, which is what a mod writes.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BlockIsBroken => "block_is_broken",
            Self::ItemIsUsed => "item_is_used",
            Self::WorldIsSaved => "world_is_saved",
        }
    }

    /// Returns whether `side` is a server side, where these decisions belong.
    #[must_use]
    pub const fn happens_on(self, side: Side) -> bool {
        match self {
            Self::BlockIsBroken | Self::ItemIsUsed | Self::WorldIsSaved => side.is_server(),
        }
    }
}

impl fmt::Display for Point {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A host decision's point and side; hook payloads are not yet projected.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Occasion {
    point: Point,
    side: Side,
}

impl Occasion {
    /// Builds an occasion for host dispatch or direct mod tests.
    #[must_use]
    pub const fn at(point: Point, side: Side) -> Self {
        Self { point, side }
    }

    /// Returns the point the host is calling out from.
    #[must_use]
    pub const fn point(self) -> Point {
        self.point
    }

    /// Returns the side the round is running on.
    #[must_use]
    pub const fn side(self) -> Side {
        self.side
    }
}

/// A mod's answer; exclusive replacement is declared separately in its declaration.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Outcome {
    /// The game does what it was going to do.
    Proceed,
    /// The game does not, and no later mod is asked.
    Refuse,
}

/// Declares interception points and ordering, with ownership checked by the host.
///
/// Unpublished points cannot enter a typed declaration.
/// ```compile_fail
/// use ravel_host_api::InterceptionDeclaration;
/// let mut declared = InterceptionDeclaration::new("sample".parse().unwrap());
/// declared.answer_at("a_point_nobody_publishes");
/// ```
///
/// # Examples
///
/// ```
/// use ravel_core::ModId;
/// use ravel_host_api::{InterceptionDeclaration, Point};
///
/// let guard: ModId = "guard_mod".parse()?;
/// let ledger: ModId = "ledger_mod".parse()?;
///
/// let mut declared = InterceptionDeclaration::new(guard.clone());
/// declared.answer_at(Point::BlockIsBroken);
/// declared.after(&ledger);
///
/// assert_eq!(declared.owner(), &guard);
/// assert_eq!(declared.points(), [Point::BlockIsBroken]);
/// # Ok::<(), ravel_core::ModIdError>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterceptionDeclaration {
    owner: ModId,
    answers: Vec<(Point, bool)>,
    after: Vec<ModId>,
}

impl InterceptionDeclaration {
    /// Begins a declaration for `owner`.
    #[must_use]
    pub fn new(owner: ModId) -> Self {
        Self {
            owner,
            answers: Vec::new(),
            after: Vec::new(),
        }
    }

    /// Declares an answer at a published `point`.
    pub fn answer_at(&mut self, point: Point) -> &mut Self {
        self.answers.push((point, false));
        self
    }

    /// Declares that this mod replaces the outcome at `point`.
    pub fn replace_at(&mut self, point: Point) -> &mut Self {
        self.answers.push((point, true));
        self
    }

    /// Declares that this mod is asked after `earlier`, at every point they share.
    /// Both owners must answer at least one point, though not necessarily the same point.
    pub fn after(&mut self, earlier: &ModId) -> &mut Self {
        self.after.push(earlier.clone());
        self
    }

    /// Returns the mod this declaration belongs to.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        &self.owner
    }

    /// Returns the points declared, in declaration order.
    #[must_use]
    pub fn points(&self) -> Vec<Point> {
        self.answers.iter().map(|(point, _)| *point).collect()
    }

    /// Returns whether this declaration answers at `point` at all.
    #[must_use]
    pub fn answers_at(&self, point: Point) -> bool {
        self.answers.iter().any(|(declared, _)| *declared == point)
    }

    /// Returns the declared points and whether each replaces the outcome.
    pub fn answers(&self) -> impl Iterator<Item = (Point, bool)> + '_ {
        self.answers.iter().copied()
    }

    /// Returns the owners this declaration follows.
    #[must_use]
    pub fn predecessors(&self) -> &[ModId] {
        &self.after
    }
}
