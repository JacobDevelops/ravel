use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ravel_core::{ModId, Side, precedence};

/// A published host decision point; world-save dispatch runs on the executing server side.
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
}

/// Admits and orders declared interceptions without rewriting game code.
///
/// # Examples
///
/// ```
/// use ravel_core::{ModId, Side};
/// use ravel_host_api::{Outcome, Point, Points};
///
/// let guard: ModId = "guard_mod".parse()?;
/// let ledger: ModId = "ledger_mod".parse()?;
///
/// let mut points = Points::default();
/// points.answer_at(&ledger, Point::BlockIsBroken);
/// points.answer_at(&guard, Point::BlockIsBroken);
/// points.after(&ledger, &guard);
///
/// let points = points.admit()?;
/// let round = points.ask(Point::BlockIsBroken, Side::IntegratedServer, |_| true, &mut |owner, _| {
///     Ok(if owner == &guard {
///         Outcome::Refuse
///     } else {
///         Outcome::Proceed
///     })
/// });
/// assert_eq!(round.asked(), [guard]);
/// assert_eq!(round.outcome(), Outcome::Refuse);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Default)]
pub struct Points {
    answers: Vec<Answer>,
    after: BTreeMap<ModId, BTreeSet<ModId>>,
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
            .finish()
    }
}

impl Points {
    /// Declares an answer, validated at admission; the first refusal ends a round.
    pub fn answer_at(&mut self, owner: &ModId, point: Point) {
        self.record(owner, point, false);
    }

    /// Declares exclusive replacement; admission refuses multiple replacements at a point.
    pub fn replace_at(&mut self, owner: &ModId, point: Point) {
        self.record(owner, point, true);
    }

    /// Incorporates a mod's interception points and ordering.
    pub fn declare(&mut self, declaration: &InterceptionDeclaration) {
        for (point, replaces) in &declaration.answers {
            self.record(&declaration.owner, *point, *replaces);
        }
        for earlier in &declaration.after {
            self.after(&declaration.owner, earlier);
        }
    }

    fn record(&mut self, owner: &ModId, point: Point, replaces: bool) {
        if let Some(answer) = self
            .answers
            .iter_mut()
            .find(|answer| answer.owner == *owner && answer.point == point)
        {
            answer.replaces |= replaces;
            return;
        }
        self.answers.push(Answer {
            owner: owner.clone(),
            point,
            replaces,
        });
    }

    /// Declares that `later` is asked after `earlier`, at every point they share.
    pub fn after(&mut self, later: &ModId, earlier: &ModId) {
        self.after
            .entry(earlier.clone())
            .or_default()
            .insert(later.clone());
    }

    /// Checks exclusive replacement claims and ordering at every point.
    ///
    /// # Errors
    ///
    /// Returns contested replacements or circular ordering.
    pub fn admit(self) -> Result<AdmittedPoints, InterceptionRefused> {
        let mut ordered = BTreeMap::new();
        for point in Point::ALL {
            ordered.insert(point, self.order(point)?);
        }
        Ok(AdmittedPoints { ordered })
    }

    fn order(&self, point: Point) -> Result<Vec<ModId>, InterceptionRefused> {
        let claimants: Vec<ModId> = self
            .answers
            .iter()
            .filter(|answer| answer.point == point && answer.replaces)
            .map(|answer| answer.owner.clone())
            .collect();
        if claimants.len() > 1 {
            return Err(InterceptionRefused::ContestedPoint { point, claimants });
        }
        let owners = self.answers.iter().filter(|answer| answer.point == point);
        precedence(owners.map(|answer| answer.owner.clone()), &self.after).map_err(|cycle| {
            InterceptionRefused::Circular {
                point,
                claimants: self
                    .answers
                    .iter()
                    .filter(|answer| {
                        answer.point == point && cycle.participants().contains(&answer.owner)
                    })
                    .map(|answer| answer.owner.clone())
                    .collect(),
            }
        })
    }
}

/// Immutable interception order admitted once at the declaration barrier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedPoints {
    ordered: BTreeMap<Point, Vec<ModId>>,
}

impl AdmittedPoints {
    /// Asks mods in declared order, stopping at the first refusal or failure.
    ///
    /// A failure produces [`Outcome::Refuse`]; [`Round::failed_by`] identifies the
    /// last owner actually asked. Errors are normalized to that owner without changing
    /// their kind or phase. The caller must attribute the error to the callback's
    /// actual lifecycle phase before returning it.
    ///
    pub fn ask(
        &self,
        point: Point,
        side: Side,
        eligible: impl Fn(&ModId) -> bool,
        respond: &mut dyn FnMut(&ModId, Occasion) -> Result<Outcome, crate::HostError>,
    ) -> Round {
        let order = &self.ordered[&point];

        let occasion = Occasion { point, side };
        let mut asked = Vec::with_capacity(order.len());
        let mut outcome = Outcome::Proceed;
        let mut failure = None;
        let mut refused_by = None;
        for owner in order {
            if !eligible(owner) {
                continue;
            }
            asked.push(owner.clone());
            match respond(owner, occasion) {
                Ok(answered) => {
                    outcome = answered;
                    if outcome == Outcome::Refuse {
                        refused_by = Some(owner.clone());
                        break;
                    }
                }
                Err(refused) => {
                    outcome = Outcome::Refuse;
                    failure = Some(refused.with_owner(owner.clone()));
                    break;
                }
            }
        }
        Round {
            point,
            asked,
            outcome,
            refused_by,
            failure,
        }
    }
}

/// One pass over the mods answering at one point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Round {
    point: Point,
    asked: Vec<ModId>,
    outcome: Outcome,
    refused_by: Option<ModId>,
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

    /// Returns the mod that answered [`Outcome::Refuse`], excluding callback failures.
    #[must_use]
    pub const fn refused_by(&self) -> Option<&ModId> {
        self.refused_by.as_ref()
    }

    /// Returns the failure that ended the round with [`Outcome::Refuse`], if any.
    #[must_use]
    pub fn failure(&self) -> Option<&crate::HostError> {
        self.failure.as_ref()
    }

    /// Returns the actual callback owner whose failure ended the round, if any.
    #[must_use]
    pub fn failed_by(&self) -> Option<&ModId> {
        self.failure.as_ref().and_then(|_| self.asked.last())
    }
}

/// Why a declared set of interceptions cannot be asked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterceptionRefused {
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
    /// Returns the published point the refusal is about.
    #[must_use]
    pub const fn point(&self) -> Point {
        match self {
            Self::ContestedPoint { point, .. } | Self::Circular { point, .. } => *point,
        }
    }

    /// Returns the mods the refusal names, in the order it names them.
    #[must_use]
    pub fn claimants(&self) -> &[ModId] {
        match self {
            Self::ContestedPoint { claimants, .. } | Self::Circular { claimants, .. } => claimants,
        }
    }
}

impl fmt::Display for InterceptionRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            (None, _) | (Some(_), None) => false,
        }
    }

    #[test]
    fn every_published_point_displays_its_stable_name() {
        for point in Point::ALL {
            assert_eq!(point.to_string(), point.as_str());
        }
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
        points.answer_at(&early, Point::BlockIsBroken);
        points.answer_at(&late, Point::BlockIsBroken);
        points.after(&late, &early);

        let round = points.admit().expect("declarations admitted").ask(
            Point::BlockIsBroken,
            Side::IntegratedServer,
            |_| true,
            &mut |_, _| Ok(Outcome::Proceed),
        );

        assert_eq!(round.asked(), [early.clone(), late.clone()]);
        assert!(asked_before(&round, &early, &late));
        assert_eq!(round.outcome(), Outcome::Proceed);
        assert_eq!(round.point(), Point::BlockIsBroken);
        assert_eq!(round.refused_by(), None);
    }

    #[test]
    fn the_first_refusal_ends_the_round() {
        let early = id("sample");
        let late = id("probe");

        let mut points = Points::default();
        points.answer_at(&early, Point::BlockIsBroken);
        points.answer_at(&late, Point::BlockIsBroken);
        points.after(&late, &early);

        let refusing = early.clone();
        let round = points.admit().expect("declarations admitted").ask(
            Point::BlockIsBroken,
            Side::IntegratedServer,
            |_| true,
            &mut |owner, _| {
                Ok(if owner == &refusing {
                    Outcome::Refuse
                } else {
                    Outcome::Proceed
                })
            },
        );

        assert_eq!(round.refused_by(), Some(&early));
        assert_eq!(round.asked(), [early]);
        assert_eq!(round.outcome(), Outcome::Refuse);
        assert_eq!(round.failed_by(), None);
        assert_eq!(round.failure(), None);
    }

    #[test]
    fn a_mod_declared_at_another_point_is_not_asked_at_this_one() {
        let here = id("sample");
        let elsewhere = id("probe");

        let mut points = Points::default();
        points.answer_at(&here, Point::BlockIsBroken);
        points.answer_at(&elsewhere, Point::ItemIsUsed);

        let refusing = elsewhere.clone();
        let round = points.admit().expect("declarations admitted").ask(
            Point::BlockIsBroken,
            Side::IntegratedServer,
            |_| true,
            &mut |owner, _| {
                Ok(if owner == &refusing {
                    Outcome::Refuse
                } else {
                    Outcome::Proceed
                })
            },
        );
        assert_eq!(round.asked(), [here]);
        assert_eq!(round.outcome(), Outcome::Proceed);
    }

    #[test]
    fn the_occasion_a_mod_is_handed_names_the_point_it_is_answering_at() {
        let owner = id("sample");
        let mut points = Points::default();
        points.answer_at(&owner, Point::WorldIsSaved);

        let round = points.admit().expect("declarations admitted").ask(
            Point::WorldIsSaved,
            Side::DedicatedServer,
            |_| true,
            &mut |_, occasion: Occasion| {
                assert_eq!(occasion.point(), Point::WorldIsSaved);
                assert_eq!(occasion.side(), Side::DedicatedServer);
                Ok(Outcome::Proceed)
            },
        );
        assert_eq!(round.asked(), [owner]);
    }

    #[test]
    fn two_replacements_at_one_point_are_refused_at_the_barrier() {
        let first = id("sample");
        let second = id("probe");

        let mut points = Points::default();
        points.replace_at(&first, Point::BlockIsBroken);
        points.replace_at(&second, Point::BlockIsBroken);

        let refusal = points
            .admit()
            .expect_err("two replacements at one point are a conflict");

        assert_eq!(refusal.point(), Point::BlockIsBroken);
        assert_eq!(refusal.claimants(), [first.clone(), second.clone()]);
        assert!(refusal.to_string().contains(first.as_str()));
        assert!(refusal.to_string().contains(second.as_str()));
    }

    #[test]
    fn a_replacement_beside_plain_answers_is_admitted() {
        let replacing = id("sample");
        let answering = id("probe");

        let mut points = Points::default();
        points.replace_at(&replacing, Point::BlockIsBroken);
        points.answer_at(&answering, Point::BlockIsBroken);

        points
            .admit()
            .expect("one replacement beside answers is admitted");
    }

    #[test]
    fn one_replacement_at_each_of_two_points_is_not_a_conflict() {
        let first = id("sample");
        let second = id("probe");

        let mut points = Points::default();
        points.replace_at(&first, Point::BlockIsBroken);
        points.replace_at(&second, Point::ItemIsUsed);

        points
            .admit()
            .expect("a conflict is per point rather than per set");
    }

    #[test]
    fn an_unanswered_published_point_proceeds_without_a_refusing_owner() {
        let points = Points::default();
        let round = points.admit().expect("declarations admitted").ask(
            Point::WorldIsSaved,
            Side::IntegratedServer,
            |_| true,
            &mut |_, _| panic!("no mod answers here"),
        );
        assert_eq!(round.point(), Point::WorldIsSaved);
        assert_eq!(round.asked(), []);
        assert_eq!(round.outcome(), Outcome::Proceed);
        assert_eq!(round.refused_by(), None);
    }

    #[test]
    fn a_cycle_between_mods_at_one_point_is_refused_rather_than_ordered() {
        let first = id("sample");
        let second = id("probe");

        let mut points = Points::default();
        points.answer_at(&first, Point::BlockIsBroken);
        points.answer_at(&second, Point::BlockIsBroken);
        points.after(&first, &second);
        points.after(&second, &first);

        let refusal = points
            .admit()
            .expect_err("a cycle is rejected before any round");

        assert_eq!(refusal.point(), Point::BlockIsBroken);
        assert_eq!(refusal.claimants(), [first, second]);
    }

    #[test]
    fn the_debug_form_names_what_was_declared_without_naming_a_closure() {
        let owner = id("sample");
        let mut points = Points::default();
        points.replace_at(&owner, Point::BlockIsBroken);

        let rendered = format!("{points:?}");
        assert!(rendered.contains("sample"));
        assert!(rendered.contains("BlockIsBroken"));
    }

    #[test]
    fn admitted_orders_are_reused_and_ineligible_owners_are_not_asked() {
        let owner = id("sample");
        let mut declared = Points::default();
        declared.answer_at(&owner, Point::WorldIsSaved);
        let admitted = declared.admit().expect("valid declarations");
        let skipped = admitted.ask(
            Point::WorldIsSaved,
            Side::IntegratedServer,
            |_| false,
            &mut |_, _| panic!("ineligible owner must not run"),
        );
        assert_eq!(skipped.asked(), []);
        assert_eq!(skipped.outcome(), Outcome::Proceed);
        for _ in 0..2 {
            let round = admitted.ask(
                Point::WorldIsSaved,
                Side::IntegratedServer,
                |_| true,
                &mut |_, _| Ok(Outcome::Proceed),
            );
            assert_eq!(round.asked(), std::slice::from_ref(&owner));
        }
    }

    #[test]
    fn duplicate_claims_merge_without_self_contention_or_duplicate_callbacks() {
        let owner = id("sample");
        let mut declaration = super::InterceptionDeclaration::new(owner.clone());
        declaration.answer_at(Point::WorldIsSaved);
        declaration.replace_at(Point::WorldIsSaved);
        declaration.replace_at(Point::WorldIsSaved);
        let mut points = Points::default();
        points.declare(&declaration);
        points.declare(&declaration);
        points.answer_at(&owner, Point::WorldIsSaved);
        let admitted = points.admit().expect("one distinct replacing owner");
        let mut calls = 0;
        let round = admitted.ask(
            Point::WorldIsSaved,
            Side::IntegratedServer,
            |_| true,
            &mut |_, _| {
                calls += 1;
                Ok(Outcome::Proceed)
            },
        );
        assert_eq!(calls, 1);
        assert_eq!(round.asked(), std::slice::from_ref(&owner));
        let mut points = Points::default();
        points.declare(&declaration);
        points.answer_at(&owner, Point::WorldIsSaved);
        points.replace_at(&id("probe"), Point::WorldIsSaved);
        assert!(matches!(
            points.admit(),
            Err(InterceptionRefused::ContestedPoint { .. })
        ));
    }

    #[test]
    fn callback_failure_is_reattributed_to_the_actual_callback_owner() {
        let owner = id("sample");
        let mut points = Points::default();
        points.answer_at(&owner, Point::WorldIsSaved);
        let failure = crate::HostError::OperationRefused {
            mod_id: id("unrelated"),
            state: ravel_core::LifecycleState::Running,
            detail: "attributed callback failure".to_owned(),
        };
        let round = points.admit().expect("declarations admitted").ask(
            Point::WorldIsSaved,
            Side::DedicatedServer,
            |_| true,
            &mut |_, _| Err(failure.clone()),
        );
        assert_eq!(round.failed_by(), Some(&owner));
        assert_eq!(round.failure(), Some(&failure.with_owner(owner)));
        assert_eq!(round.outcome(), Outcome::Refuse);
        assert_eq!(round.refused_by(), None);
    }
}
