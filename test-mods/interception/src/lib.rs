#![forbid(unsafe_code)]

//! Behaviour interception test mod.
//!
//! The mod for [`ravel_sdk::interception`]. It answers at a published point,
//! refuses there under a condition it owns, and is ordered against another mod —
//! which is what `ROADMAP.md`'s M6-W2 row asks an original mod to exercise.
//!
//! It patches nothing. A point is a place the host calls out from, so what this
//! mod declares is an answer the host would ask for, and this crate carries
//! `#![forbid(unsafe_code)]` like every other.

use ravel_sdk::interception::{InterceptionRefused, Outcome, Point, Points, Round};
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, ModId, declare_mod};

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "interception";

/// The point this mod answers at.
pub const POINT: Point = Point::BlockIsBroken;

/// A mod that answers when a block is about to be broken.
///
/// # Examples
///
/// ```
/// use ravel_sdk::interception::Outcome;
/// use ravel_sdk::Declared;
/// use ravel_test_mod_interception::InterceptionMod;
///
/// // Guarding, this mod refuses and the round ends at it.
/// let round = InterceptionMod::round(true).expect("a published point is asked");
/// assert_eq!(round.outcome(), Outcome::Refuse);
/// assert_eq!(round.asked(), [InterceptionMod::declared_id().clone()]);
///
/// // Not guarding, the mod after it is asked too.
/// let round = InterceptionMod::round(false).expect("a published point is asked");
/// assert_eq!(round.outcome(), Outcome::Proceed);
/// assert_eq!(round.asked().len(), 2);
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InterceptionMod {
    entered: Vec<LifecycleState>,
}

impl InterceptionMod {
    /// Builds the mod instance the loader drives.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the states this mod was taken through, in order.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }

    /// Returns the mod this one is declared to run before.
    ///
    /// # Panics
    ///
    /// Panics if `follower_mod` stops being a legal mod id.
    #[must_use]
    pub fn follower() -> ModId {
        "follower_mod".parse().expect("a legal mod id")
    }

    /// Declares this mod and one that follows it, and asks the point.
    ///
    /// `guarding` is this mod's own condition. It refuses when it holds, which
    /// is what a mod at a point is for: the answer is the mod's, and Ravel owns
    /// only the order and what a refusal means.
    ///
    /// # Errors
    ///
    /// Returns [`InterceptionRefused`] when the set cannot be asked, which this
    /// set can: one point, two mods, one edge, no replacement.
    pub fn round(guarding: bool) -> Result<Round, InterceptionRefused> {
        let follower = Self::follower();
        let mut points = Points::default();
        points.answer_at(Self::declared_id(), POINT.as_str(), move |_| {
            if guarding {
                Outcome::Refuse
            } else {
                Outcome::Proceed
            }
        });
        points.answer_at(&follower, POINT.as_str(), |_| Outcome::Proceed);
        points.after(&follower, Self::declared_id());
        points.ask(POINT.as_str())
    }
}

#[declare_mod(id = "interception")]
impl Mod for InterceptionMod {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        self.entered.push(state);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::interception::{Outcome, Points};
    use ravel_sdk::{Declared, Side};

    use super::{InterceptionMod, MOD_ID, POINT};

    #[test]
    fn the_declared_identity_is_the_one_this_mod_is_packaged_under() {
        assert_eq!(InterceptionMod::declared_id().as_str(), MOD_ID);
    }

    #[test]
    fn this_mod_is_asked_before_the_mod_declared_to_follow_it() {
        let round = InterceptionMod::round(false).expect("a published point is asked");
        let mine = round
            .asked()
            .iter()
            .position(|id| id == InterceptionMod::declared_id())
            .expect("this mod answers here");
        let theirs = round
            .asked()
            .iter()
            .position(|id| id == &InterceptionMod::follower())
            .expect("the follower answers here");

        assert!(mine < theirs);
        assert_eq!(round.outcome(), Outcome::Proceed);
    }

    /// The mod after the refusal is absent from the round rather than present
    /// with a discarded answer, which is the difference between ending a round
    /// and reconciling one.
    #[test]
    fn a_refusal_from_this_mod_ends_the_round_before_the_follower_is_asked() {
        let round = InterceptionMod::round(true).expect("a published point is asked");

        assert_eq!(round.outcome(), Outcome::Refuse);
        assert_eq!(round.asked(), [InterceptionMod::declared_id().clone()]);
        assert!(!round.asked().contains(&InterceptionMod::follower()));
        assert_eq!(round.point(), POINT);
    }

    #[test]
    fn a_second_mod_replacing_the_outcome_this_mod_replaces_is_refused() {
        let follower = InterceptionMod::follower();
        let mut points = Points::default();
        points.replace_at(InterceptionMod::declared_id(), POINT.as_str(), |_| {
            Outcome::Proceed
        });
        points.replace_at(&follower, POINT.as_str(), |_| Outcome::Proceed);

        let refusal = points
            .admit()
            .expect_err("two replacements at one point are a conflict");

        assert_eq!(refusal.point(), POINT.as_str());
        assert_eq!(
            refusal.claimants(),
            [InterceptionMod::declared_id().clone(), follower]
        );
    }

    #[test]
    fn the_point_this_mod_answers_at_is_reached_on_server_sides() {
        assert!(POINT.happens_on(Side::IntegratedServer));
        assert!(POINT.happens_on(Side::DedicatedServer));
        assert!(!POINT.happens_on(Side::Client));
    }
}
