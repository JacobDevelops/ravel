use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ravel_core::{ModId, Side, precedence};

/// A place the host calls out from and a mod may answer at.
///
/// The set is closed, and closed for the same reason [`crate::GameEvent`]'s is:
/// a point is a place a version adapter has resolved a hook for, so a name no
/// adapter published names nothing. A mod never spells a hook — `SPEC.md` §3.7's
/// version boundary and `docs/architecture/dependency-rules.md` both forbid it —
/// and this is the mod-facing form the hook becomes.
///
/// **Nothing calls out at any of these yet.** No host in this workspace asks a
/// point, exactly as no host carries a packet, and the module documentation says
/// so at greater length. What is shipped is the declaration, the order and the
/// two refusals, all of which are decidable without a game.
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

    /// Returns whether `side` is a side this point is reached on.
    ///
    /// All three are server-side decisions: what a block does when it breaks is
    /// the server's answer even in single player, where the integrated server
    /// gives it. A client that answered separately would answer differently, and
    /// the two answers would be two games.
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

/// What the host is about to do, handed to every mod asked at a point.
///
/// It carries the point and the side today and no more, for the same reason
/// [`crate::GameEvent`] carries no payload: the projection from a hook's
/// parameters to a value a mod can read lands with the wiring that connects a
/// hook to the loader. A mod that matches on the point is writing against what
/// exists; one that expects a block position is writing against what does not.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Occasion {
    point: Point,
    side: Side,
}

impl Occasion {
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

/// What a mod answers at a point.
///
/// Two answers rather than three, and the third one a caller reaches for —
/// "replace this outcome with mine" — is a property of the *declaration* instead
/// of the answer. That is [`Points::replace_at`], and it is separated because
/// the two compose differently: any number of mods may proceed or refuse, and
/// two mods replacing one outcome is a conflict no order resolves.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Outcome {
    /// The game does what it was going to do.
    Proceed,
    /// The game does not, and no later mod is asked.
    Refuse,
}

/// One mod's answer at one point.
struct Answer {
    owner: ModId,
    point: Point,
    replaces: bool,
    respond: Box<dyn Fn(&Occasion) -> Outcome>,
}

/// Every mod's declared answers, and the order they are asked in.
///
/// A mod declares that it wants to *answer* at a point rather than be told what
/// happened at one. Declaring is the whole of what a mod does here: the order
/// among mods, the conflict refusal, and the round itself belong to Ravel,
/// because each of them is a question about the set and no member of a set can
/// answer one.
///
/// This is not a way to rewrite the game's code. A point is a place the host
/// already calls out from, so an interception is a call the host makes into a
/// mod and never a patch a mod applies to the host. Nothing here asks for an
/// exception to `#![forbid(unsafe_code)]` and no crate that carries it has one.
///
/// # Examples
///
/// ```
/// use ravel_core::ModId;
/// use ravel_host_api::{Outcome, Points};
///
/// let guard: ModId = "guard_mod".parse()?;
/// let ledger: ModId = "ledger_mod".parse()?;
///
/// let mut points = Points::default();
/// points.answer_at(&ledger, "block_is_broken", |_| Outcome::Proceed);
/// points.answer_at(&guard, "block_is_broken", |_| Outcome::Refuse);
/// // The guard answers first, so what it refuses the ledger never sees.
/// points.after(&ledger, &guard);
///
/// let round = points.ask("block_is_broken").expect("a published point is asked");
/// assert_eq!(round.asked(), [guard]);
/// assert_eq!(round.outcome(), Outcome::Refuse);
/// # Ok::<(), ravel_core::ModIdError>(())
/// ```
#[derive(Default)]
pub struct Points {
    answers: Vec<Answer>,
    after: BTreeMap<ModId, BTreeSet<ModId>>,
    /// Names no point publishes, kept so the barrier can name them and the mods
    /// that wrote them.
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
    /// Declares that `owner` answers at `point`.
    ///
    /// An answer composes with every other answer at the point: the round takes
    /// them in order and the first [`Outcome::Refuse`] ends it, so a mod that
    /// proceeds has said nothing and a mod that refuses has said the only thing
    /// there is to say.
    ///
    /// The point name is not checked here, because a mod that builds one at run
    /// time wants the answer at the barrier rather than at each call site; that
    /// is [`Points::admit`], which every round runs first.
    pub fn answer_at(
        &mut self,
        owner: &ModId,
        point: &str,
        answer: impl Fn(&Occasion) -> Outcome + 'static,
    ) {
        self.declare(owner, point, false, answer);
    }

    /// Declares that `owner` replaces the outcome at `point`.
    ///
    /// A replacement is what a mod writes when the game's own outcome is not one
    /// it wants composed with — the mod is deciding, not voting. Exactly one mod
    /// may do that at a point, and [`Points::admit`] refuses a set where two do.
    pub fn replace_at(
        &mut self,
        owner: &ModId,
        point: &str,
        answer: impl Fn(&Occasion) -> Outcome + 'static,
    ) {
        self.declare(owner, point, true, answer);
    }

    /// Records one declaration, published or not.
    ///
    /// An unpublished name is kept rather than dropped, so [`Points::admit`] can
    /// name it. Dropping it here is what would make a typo silent, which is the
    /// failure this capability exists to refuse.
    fn declare(
        &mut self,
        owner: &ModId,
        point: &str,
        replaces: bool,
        answer: impl Fn(&Occasion) -> Outcome + 'static,
    ) {
        match Point::published(point) {
            Some(published) => self.answers.push(Answer {
                owner: owner.clone(),
                point: published,
                replaces,
                respond: Box::new(answer),
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

    /// Answers whether the declared set can be asked at all.
    ///
    /// Two refusals, both about the set rather than about any one mod: a point
    /// nothing published, and two mods replacing one outcome. Both are decided
    /// before a round runs, because a set that cannot be asked coherently should
    /// not be asked once and then found out.
    ///
    /// # Errors
    ///
    /// Returns [`InterceptionRefused::UnpublishedPoint`] naming the name and the
    /// mods that wrote it, and [`InterceptionRefused::ContestedPoint`] naming the
    /// point and both claimants.
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

    /// Asks every mod that answers at `point`, in the declared order.
    ///
    /// Each mod is asked while the outcome still stands and the first
    /// [`Outcome::Refuse`] ends the round, so a mod that refused is not
    /// overruled by a later one it never knew about. A point whose answers were
    /// collected and then reconciled would have no meaning for the mod that
    /// refused, which is why they are not.
    ///
    /// # Errors
    ///
    /// Returns whatever [`Points::admit`] refuses, which every round runs first,
    /// and [`InterceptionRefused::Circular`] when the mods at the point each wait
    /// for another.
    ///
    /// # Panics
    ///
    /// Panics if the order names a mod that does not answer at the point, which
    /// cannot happen: the order is built from exactly those mods.
    pub fn ask(&self, point: &str) -> Result<Round, InterceptionRefused> {
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
            // In the order the mods declared, because a mod author reading the
            // refusal is looking for the edge they wrote and wrote it here.
            claimants: at_point
                .iter()
                .filter(|answer| cycle.participants().contains(&answer.owner))
                .map(|answer| answer.owner.clone())
                .collect(),
        })?;

        let occasion = Occasion {
            point,
            // A point is reached on a server side, and the integrated server is
            // the one every side of the game has.
            side: Side::IntegratedServer,
        };
        let mut asked = Vec::with_capacity(order.len());
        let mut outcome = Outcome::Proceed;
        for owner in order {
            let answer = at_point
                .iter()
                .find(|answer| answer.owner == owner)
                .expect("the order holds only mods that answer at this point");
            asked.push(owner);
            outcome = (answer.respond)(&occasion);
            if outcome == Outcome::Refuse {
                break;
            }
        }
        Ok(Round {
            point,
            asked,
            outcome,
        })
    }
}

/// One pass over the mods answering at one point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Round {
    point: Point,
    asked: Vec<ModId>,
    outcome: Outcome,
}

impl Round {
    /// Returns the point the round ran at.
    #[must_use]
    pub const fn point(&self) -> Point {
        self.point
    }

    /// Returns the mods that were asked, in the order they were asked.
    ///
    /// A mod after the one that refused is absent rather than present with a
    /// discarded answer, because it was not asked.
    #[must_use]
    pub fn asked(&self) -> &[ModId] {
        &self.asked
    }

    /// Returns what the round decided.
    #[must_use]
    pub const fn outcome(&self) -> Outcome {
        self.outcome
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

    /// Checks a round against what was declared by a different route than the
    /// round took: the order is read back positionally rather than trusting the
    /// sequence that produced it.
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
        points.answer_at(&early, "block_is_broken", |_| Outcome::Proceed);
        points.answer_at(&late, "block_is_broken", |_| Outcome::Proceed);
        points.after(&late, &early);

        let round = points
            .ask("block_is_broken")
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
        points.answer_at(&early, "block_is_broken", |_| Outcome::Refuse);
        points.answer_at(&late, "block_is_broken", |_| Outcome::Proceed);
        points.after(&late, &early);

        let round = points
            .ask("block_is_broken")
            .expect("a published point is asked");

        assert_eq!(round.asked(), [early]);
        assert_eq!(round.outcome(), Outcome::Refuse);
    }

    #[test]
    fn a_mod_declared_at_another_point_is_not_asked_at_this_one() {
        let here = id("sample");
        let elsewhere = id("probe");

        let mut points = Points::default();
        points.answer_at(&here, "block_is_broken", |_| Outcome::Proceed);
        points.answer_at(&elsewhere, "item_is_used", |_| Outcome::Refuse);

        let round = points
            .ask("block_is_broken")
            .expect("a published point is asked");
        assert_eq!(round.asked(), [here]);
        assert_eq!(round.outcome(), Outcome::Proceed);
    }

    #[test]
    fn the_occasion_a_mod_is_handed_names_the_point_it_is_answering_at() {
        let owner = id("sample");
        let mut points = Points::default();
        points.answer_at(&owner, "world_is_saved", |occasion: &Occasion| {
            assert_eq!(occasion.point(), Point::WorldIsSaved);
            assert!(occasion.side().is_server());
            Outcome::Proceed
        });

        let round = points
            .ask("world_is_saved")
            .expect("a published point is asked");
        assert_eq!(round.asked(), [owner]);
    }

    #[test]
    fn two_replacements_at_one_point_are_refused_at_the_barrier() {
        let first = id("sample");
        let second = id("probe");

        let mut points = Points::default();
        points.replace_at(&first, "block_is_broken", |_| Outcome::Proceed);
        points.replace_at(&second, "block_is_broken", |_| Outcome::Proceed);

        let refusal = points
            .admit()
            .expect_err("two replacements at one point are a conflict");

        assert_eq!(refusal.point(), "block_is_broken");
        assert_eq!(refusal.claimants(), [first.clone(), second.clone()]);
        assert!(refusal.to_string().contains(first.as_str()));
        assert!(refusal.to_string().contains(second.as_str()));

        // The round is refused too, rather than running on a set the barrier
        // already said cannot be asked.
        assert!(points.ask("block_is_broken").is_err());
    }

    #[test]
    fn a_replacement_beside_plain_answers_is_admitted() {
        let replacing = id("sample");
        let answering = id("probe");

        let mut points = Points::default();
        points.replace_at(&replacing, "block_is_broken", |_| Outcome::Proceed);
        points.answer_at(&answering, "block_is_broken", |_| Outcome::Refuse);

        points
            .admit()
            .expect("one replacement beside answers is admitted");
    }

    #[test]
    fn one_replacement_at_each_of_two_points_is_not_a_conflict() {
        let first = id("sample");
        let second = id("probe");

        let mut points = Points::default();
        points.replace_at(&first, "block_is_broken", |_| Outcome::Proceed);
        points.replace_at(&second, "item_is_used", |_| Outcome::Proceed);

        points
            .admit()
            .expect("a conflict is per point rather than per set");
    }

    #[test]
    fn an_answer_at_a_name_no_point_publishes_is_refused_at_the_barrier() {
        let owner = id("sample");
        let mut points = Points::default();
        points.answer_at(&owner, "a_point_nobody_publishes", |_| Outcome::Proceed);

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
            .ask("a_point_nobody_publishes")
            .expect_err("an unpublished point is not an empty round");
        assert_eq!(refusal.point(), "a_point_nobody_publishes");
    }

    #[test]
    fn a_cycle_between_mods_at_one_point_is_refused_rather_than_ordered() {
        let first = id("sample");
        let second = id("probe");

        let mut points = Points::default();
        points.answer_at(&first, "block_is_broken", |_| Outcome::Proceed);
        points.answer_at(&second, "block_is_broken", |_| Outcome::Proceed);
        points.after(&first, &second);
        points.after(&second, &first);

        let refusal = points
            .ask("block_is_broken")
            .expect_err("a cycle has no order and is not given one");

        assert_eq!(refusal.point(), "block_is_broken");
        assert_eq!(refusal.claimants(), [first, second]);
    }

    #[test]
    fn the_debug_form_names_what_was_declared_without_naming_a_closure() {
        let owner = id("sample");
        let mut points = Points::default();
        points.replace_at(&owner, "block_is_broken", |_| Outcome::Proceed);

        let rendered = format!("{points:?}");
        assert!(rendered.contains("sample"));
        assert!(rendered.contains("BlockIsBroken"));
    }
}
