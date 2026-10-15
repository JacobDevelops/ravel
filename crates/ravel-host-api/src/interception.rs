use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ravel_core::{ModId, Side, precedence};

/// A published host decision point; live host dispatch is not yet wired.
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

    /// Returns the point `name` publishes, if any point does.
    #[must_use]
    pub fn published(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|point| point.as_str() == name)
    }

    /// Returns whether `side` is a server side, where these decisions belong.
    #[must_use]
    pub const fn happens_on(self, side: Side) -> bool {
        let _ = self;
        side.is_server()
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

/// A mod's answer; exclusive replacement is declared separately with [`Points::replace_at`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Outcome {
    /// The game does what it was going to do.
    Proceed,
    /// The game does not, and no later mod is asked.
    Refuse,
}

struct Answer {
    owner: ModId,
    point: Point,
    replaces: bool,
}

/// Declares interception points and ordering, with ownership checked by the host.
///
/// # Examples
///
/// ```
/// use ravel_core::ModId;
/// use ravel_host_api::InterceptionDeclaration;
///
/// let guard: ModId = "guard_mod".parse()?;
/// let ledger: ModId = "ledger_mod".parse()?;
///
/// let mut declared = InterceptionDeclaration::new(guard.clone());
/// declared.answer_at("block_is_broken");
/// declared.after(&ledger);
///
/// assert_eq!(declared.owner(), &guard);
/// assert_eq!(declared.points(), ["block_is_broken"]);
/// # Ok::<(), ravel_core::ModIdError>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterceptionDeclaration {
    owner: ModId,
    answers: Vec<(String, bool)>,
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

    /// Declares an answer at `point`; unpublished names are refused by [`Points::admit`].
    pub fn answer_at(&mut self, point: &str) -> &mut Self {
        self.answers.push((point.to_owned(), false));
        self
    }

    /// Declares that this mod replaces the outcome at `point`.
    pub fn replace_at(&mut self, point: &str) -> &mut Self {
        self.answers.push((point.to_owned(), true));
        self
    }

    /// Declares that this mod is asked after `earlier`, at every point they share.
    pub fn after(&mut self, earlier: &ModId) -> &mut Self {
        self.after.push(earlier.clone());
        self
    }

    /// Returns the mod this declaration belongs to.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        &self.owner
    }

    /// Returns the point names declared, in declaration order.
    #[must_use]
    pub fn points(&self) -> Vec<&str> {
        self.answers
            .iter()
            .map(|(point, _)| point.as_str())
            .collect()
    }

    /// Returns whether this declaration answers at `point` at all.
    #[must_use]
    pub fn answers_at(&self, point: Point) -> bool {
        self.answers.iter().any(|(name, _)| name == point.as_str())
    }
}

/// Admits and orders declared interceptions without rewriting game code.
///
/// # Examples
///
/// ```
/// use ravel_core::{ModId, Side};
/// use ravel_host_api::{Outcome, Points};
///
/// let guard: ModId = "guard_mod".parse()?;
/// let ledger: ModId = "ledger_mod".parse()?;
///
/// let mut points = Points::default();
/// points.answer_at(&ledger, "block_is_broken");
/// points.answer_at(&guard, "block_is_broken");
/// points.after(&ledger, &guard);
///
/// let round = points.ask("block_is_broken", Side::IntegratedServer, &mut |owner, _| {
///     Ok(if owner == &guard {
///         Outcome::Refuse
///     } else {
///         Outcome::Proceed
///     })
/// })?;
/// assert_eq!(round.asked(), [guard]);
/// assert_eq!(round.outcome(), Outcome::Refuse);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Default)]
pub struct Points {
    answers: Vec<Answer>,
    after: BTreeMap<ModId, BTreeSet<ModId>>,
    /// Retained so admission can report unpublished names and their claimants.
    unpublished: BTreeMap<String, BTreeSet<ModId>>,
}

impl fmt::Debug for Points {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Points")
            .field(
                "answers",
                &self
                    .answers
                    .iter()
                    .map(|answer| (answer.owner.as_str(), answer.point, answer.replaces))
                    .collect::<Vec<_>>(),
            )
            .field("after", &self.after)
            .field("unpublished", &self.unpublished)
            .finish()
    }
}

impl Points {
    /// Declares an answer, validated at admission; the first refusal ends a round.
    pub fn answer_at(&mut self, owner: &ModId, point: &str) {
        self.record(owner, point, false);
    }

    /// Declares exclusive replacement; admission refuses multiple replacements at a point.
    pub fn replace_at(&mut self, owner: &ModId, point: &str) {
        self.record(owner, point, true);
    }

    /// Incorporates a mod's interception points and ordering.
    pub fn declare(&mut self, declaration: &InterceptionDeclaration) {
        for (point, replaces) in &declaration.answers {
            self.record(&declaration.owner, point, *replaces);
        }
        for earlier in &declaration.after {
            self.after(&declaration.owner, earlier);
        }
    }

    fn record(&mut self, owner: &ModId, point: &str, replaces: bool) {
        match Point::published(point) {
            Some(published) => self.answers.push(Answer {
                owner: owner.clone(),
                point: published,
                replaces,
            }),
            None => {
                self.unpublished
                    .entry(point.to_owned())
                    .or_default()
                    .insert(owner.clone());
            }
        }
    }

    /// Declares that `later` is asked after `earlier`, at every point they share.
    pub fn after(&mut self, later: &ModId, earlier: &ModId) {
        self.after
            .entry(earlier.clone())
            .or_default()
            .insert(later.clone());
    }

    /// Checks published point names and exclusive replacement claims.
    ///
    /// # Errors
    ///
    /// Returns [`InterceptionRefused::UnpublishedPoint`] or [`InterceptionRefused::ContestedPoint`].
    pub fn admit(&self) -> Result<(), InterceptionRefused> {
        if let Some((name, owners)) = self.unpublished.first_key_value() {
            return Err(InterceptionRefused::UnpublishedPoint {
                name: name.clone(),
                claimants: owners.iter().cloned().collect(),
            });
        }

        for point in Point::ALL {
            let claimants: Vec<ModId> = self
                .answers
                .iter()
                .filter(|answer| answer.point == point && answer.replaces)
                .map(|answer| answer.owner.clone())
                .collect();
            if claimants.len() > 1 {
                return Err(InterceptionRefused::ContestedPoint { point, claimants });
            }
        }
        Ok(())
    }

    /// Asks mods in declared order, stopping at the first refusal or failure.
    ///
    /// A failure produces [`Outcome::Refuse`] and remains available through [`Round::failure`].
    ///
    /// # Errors
    ///
    /// Returns admission refusals or [`InterceptionRefused::Circular`] for an ordering cycle.
    ///
    /// # Panics
    ///
    /// Panics if the internally built order names a mod that does not answer here.
    pub fn ask(
        &self,
        point: &str,
        side: Side,
        respond: &mut dyn FnMut(&ModId, Occasion) -> Result<Outcome, crate::HostError>,
    ) -> Result<Round, InterceptionRefused> {
        self.admit()?;
        let point =
            Point::published(point).ok_or_else(|| InterceptionRefused::UnpublishedPoint {
                name: point.to_owned(),
                claimants: Vec::new(),
            })?;

        let at_point: Vec<&Answer> = self
            .answers
            .iter()
            .filter(|answer| answer.point == point)
            .collect();
        let order = precedence(
            at_point.iter().map(|answer| answer.owner.clone()),
            &self.after,
        )
        .map_err(|cycle| InterceptionRefused::Circular {
            point,
            // Report cycle participants in declaration order.
            claimants: at_point
                .iter()
                .filter(|answer| cycle.participants().contains(&answer.owner))
                .map(|answer| answer.owner.clone())
                .collect(),
        })?;

        let occasion = Occasion { point, side };
        let mut asked = Vec::with_capacity(order.len());
        let mut outcome = Outcome::Proceed;
        let mut failure = None;
        for owner in order {
            assert!(
                at_point.iter().any(|answer| answer.owner == owner),
                "the order holds only mods that answer at this point"
            );
            asked.push(owner.clone());
            match respond(&owner, occasion) {
                Ok(answered) => {
                    outcome = answered;
                    if outcome == Outcome::Refuse {
                        break;
                    }
                }
                Err(refused) => {
                    outcome = Outcome::Refuse;
                    failure = Some(refused);
                    break;
                }
            }
        }
        Ok(Round {
            point,
            asked,
            outcome,
            failure,
        })
    }
}

/// One pass over the mods answering at one point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Round {
    point: Point,
    asked: Vec<ModId>,
    outcome: Outcome,
    failure: Option<crate::HostError>,
}

impl Round {
    /// Returns the point the round ran at.
    #[must_use]
    pub const fn point(&self) -> Point {
        self.point
    }

    /// Returns only the mods actually asked, in dispatch order.
    #[must_use]
    pub fn asked(&self) -> &[ModId] {
        &self.asked
    }

    /// Returns what the round decided.
    #[must_use]
    pub const fn outcome(&self) -> Outcome {
        self.outcome
    }

    /// Returns the failure that ended the round with [`Outcome::Refuse`], if any.
    #[must_use]
    pub const fn failure(&self) -> Option<&crate::HostError> {
        self.failure.as_ref()
    }
}

/// Why a declared set of interceptions cannot be asked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterceptionRefused {
    /// A mod declared an answer at a name no point publishes.
    UnpublishedPoint {
        /// The name that was written.
        name: String,
        /// The mods that wrote it, in name order.
        claimants: Vec<ModId>,
    },
    /// Two mods each replace the outcome at one point.
    ContestedPoint {
        /// The contested point.
        point: Point,
        /// Both claimants, in declaration order.
        claimants: Vec<ModId>,
    },
    /// The mods at one point each wait for another.
    Circular {
        /// The point whose order was refused.
        point: Point,
        /// The mods that each wait for another, in declaration order.
        claimants: Vec<ModId>,
    },
}

impl InterceptionRefused {
    /// Returns the point the refusal is about, by the name a mod wrote.
    #[must_use]
    pub fn point(&self) -> &str {
        match self {
            Self::UnpublishedPoint { name, .. } => name,
            Self::ContestedPoint { point, .. } | Self::Circular { point, .. } => point.as_str(),
        }
    }

    /// Returns the mods the refusal names, in the order it names them.
    #[must_use]
    pub fn claimants(&self) -> &[ModId] {
        match self {
            Self::UnpublishedPoint { claimants, .. }
            | Self::ContestedPoint { claimants, .. }
            | Self::Circular { claimants, .. } => claimants,
        }
    }
}

impl fmt::Display for InterceptionRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnpublishedPoint { name, .. } => write!(
                formatter,
                "`{name}` is not a point Ravel publishes, so nothing would ever ask there"
            ),
            Self::ContestedPoint { point, claimants } => {
                let names: Vec<&str> = claimants.iter().map(ModId::as_str).collect();
                write!(
                    formatter,
                    "`{}` and `{}` each replace the outcome at `{point}`, and two replacements are \
                     two different games rather than an order",
                    names.first().copied().unwrap_or_default(),
                    names.get(1).copied().unwrap_or_default()
                )
            }
            Self::Circular { point, claimants } => {
                let names: Vec<&str> = claimants.iter().map(ModId::as_str).collect();
                write!(
                    formatter,
                    "at `{point}`, `{}` each wait for another, and a cycle has no order",
                    names.join("`, `")
                )
            }
        }
    }
}

impl std::error::Error for InterceptionRefused {}

#[cfg(test)]
mod tests {
    use ravel_core::{ModId, Side};

    use super::{InterceptionRefused, Occasion, Outcome, Point, Points, Round};

    fn id(name: &str) -> ModId {
        name.parse().expect("a legal mod id")
    }

    /// Checks dispatch order independently by comparing returned positions.
    fn asked_before(round: &Round, earlier: &ModId, later: &ModId) -> bool {
        match (
            round.asked().iter().position(|id| id == earlier),
            round.asked().iter().position(|id| id == later),
        ) {
            (Some(first), Some(second)) => first < second,
            _ => false,
        }
    }

    #[test]
    fn every_published_point_names_itself_once_and_is_found_by_that_name() {
        for point in Point::ALL {
            assert_eq!(Point::published(point.as_str()), Some(point));
            assert_eq!(point.to_string(), point.as_str());
        }
        assert_eq!(Point::published("a_point_nobody_publishes"), None);
    }

    #[test]
    fn a_point_is_reached_on_server_sides_and_not_on_the_client() {
        for point in Point::ALL {
            assert!(point.happens_on(Side::IntegratedServer));
            assert!(point.happens_on(Side::DedicatedServer));
            assert!(!point.happens_on(Side::Client));
        }
    }

    #[test]
    fn mods_at_one_point_are_asked_in_declared_order() {
        let early = id("sample");
        let late = id("probe");

        let mut points = Points::default();
        points.answer_at(&early, "block_is_broken");
        points.answer_at(&late, "block_is_broken");
        points.after(&late, &early);

        let round = points
            .ask("block_is_broken", Side::IntegratedServer, &mut |_, _| {
                Ok(Outcome::Proceed)
            })
            .expect("a published point is asked");

        assert_eq!(round.asked(), [early.clone(), late.clone()]);
        assert!(asked_before(&round, &early, &late));
        assert_eq!(round.outcome(), Outcome::Proceed);
        assert_eq!(round.point(), Point::BlockIsBroken);
    }

    #[test]
    fn the_first_refusal_ends_the_round() {
        let early = id("sample");
        let late = id("probe");

        let mut points = Points::default();
        points.answer_at(&early, "block_is_broken");
        points.answer_at(&late, "block_is_broken");
        points.after(&late, &early);

        let refusing = early.clone();
        let round = points
            .ask(
                "block_is_broken",
                Side::IntegratedServer,
                &mut |owner, _| {
                    Ok(if owner == &refusing {
                        Outcome::Refuse
                    } else {
                        Outcome::Proceed
                    })
                },
            )
            .expect("a published point is asked");

        assert_eq!(round.asked(), [early]);
        assert_eq!(round.outcome(), Outcome::Refuse);
    }

    #[test]
    fn a_mod_declared_at_another_point_is_not_asked_at_this_one() {
        let here = id("sample");
        let elsewhere = id("probe");

        let mut points = Points::default();
        points.answer_at(&here, "block_is_broken");
        points.answer_at(&elsewhere, "item_is_used");

        let refusing = elsewhere.clone();
        let round = points
            .ask(
                "block_is_broken",
                Side::IntegratedServer,
                &mut |owner, _| {
                    Ok(if owner == &refusing {
                        Outcome::Refuse
                    } else {
                        Outcome::Proceed
                    })
                },
            )
            .expect("a published point is asked");
        assert_eq!(round.asked(), [here]);
        assert_eq!(round.outcome(), Outcome::Proceed);
    }

    #[test]
    fn the_occasion_a_mod_is_handed_names_the_point_it_is_answering_at() {
        let owner = id("sample");
        let mut points = Points::default();
        points.answer_at(&owner, "world_is_saved");

        let round = points
            .ask(
                "world_is_saved",
                Side::DedicatedServer,
                &mut |_, occasion: Occasion| {
                    assert_eq!(occasion.point(), Point::WorldIsSaved);
                    assert_eq!(occasion.side(), Side::DedicatedServer);
                    Ok(Outcome::Proceed)
                },
            )
            .expect("a published point is asked");
        assert_eq!(round.asked(), [owner]);
    }

    #[test]
    fn two_replacements_at_one_point_are_refused_at_the_barrier() {
        let first = id("sample");
        let second = id("probe");

        let mut points = Points::default();
        points.replace_at(&first, "block_is_broken");
        points.replace_at(&second, "block_is_broken");

        let refusal = points
            .admit()
            .expect_err("two replacements at one point are a conflict");

        assert_eq!(refusal.point(), "block_is_broken");
        assert_eq!(refusal.claimants(), [first.clone(), second.clone()]);
        assert!(refusal.to_string().contains(first.as_str()));
        assert!(refusal.to_string().contains(second.as_str()));

        assert!(
            points
                .ask("block_is_broken", Side::IntegratedServer, &mut |_, _| Ok(
                    Outcome::Proceed
                ))
                .is_err()
        );
    }

    #[test]
    fn a_replacement_beside_plain_answers_is_admitted() {
        let replacing = id("sample");
        let answering = id("probe");

        let mut points = Points::default();
        points.replace_at(&replacing, "block_is_broken");
        points.answer_at(&answering, "block_is_broken");

        points
            .admit()
            .expect("one replacement beside answers is admitted");
    }

    #[test]
    fn one_replacement_at_each_of_two_points_is_not_a_conflict() {
        let first = id("sample");
        let second = id("probe");

        let mut points = Points::default();
        points.replace_at(&first, "block_is_broken");
        points.replace_at(&second, "item_is_used");

        points
            .admit()
            .expect("a conflict is per point rather than per set");
    }

    #[test]
    fn an_answer_at_a_name_no_point_publishes_is_refused_at_the_barrier() {
        let owner = id("sample");
        let mut points = Points::default();
        points.answer_at(&owner, "a_point_nobody_publishes");

        let refusal = points
            .admit()
            .expect_err("a name nothing publishes names nothing");

        assert_eq!(refusal.point(), "a_point_nobody_publishes");
        assert_eq!(refusal.claimants(), [owner]);
        assert!(matches!(
            refusal,
            InterceptionRefused::UnpublishedPoint { .. }
        ));
    }

    #[test]
    fn asking_a_name_no_point_publishes_is_refused_rather_than_answered_empty() {
        let points = Points::default();
        let refusal = points
            .ask(
                "a_point_nobody_publishes",
                Side::IntegratedServer,
                &mut |_, _| Ok(Outcome::Proceed),
            )
            .expect_err("an unpublished point is not an empty round");
        assert_eq!(refusal.point(), "a_point_nobody_publishes");
    }

    #[test]
    fn a_cycle_between_mods_at_one_point_is_refused_rather_than_ordered() {
        let first = id("sample");
        let second = id("probe");

        let mut points = Points::default();
        points.answer_at(&first, "block_is_broken");
        points.answer_at(&second, "block_is_broken");
        points.after(&first, &second);
        points.after(&second, &first);

        let refusal = points
            .ask("block_is_broken", Side::IntegratedServer, &mut |_, _| {
                Ok(Outcome::Proceed)
            })
            .expect_err("a cycle has no order and is not given one");

        assert_eq!(refusal.point(), "block_is_broken");
        assert_eq!(refusal.claimants(), [first, second]);
    }

    #[test]
    fn the_debug_form_names_what_was_declared_without_naming_a_closure() {
        let owner = id("sample");
        let mut points = Points::default();
        points.replace_at(&owner, "block_is_broken");

        let rendered = format!("{points:?}");
        assert!(rendered.contains("sample"));
        assert!(rendered.contains("BlockIsBroken"));
    }
}
