#![forbid(unsafe_code)]

//! Declares an ordered block-breaking interception and conditionally refuses it.
//!
//! `tests/conformance/tests/integration/sdk_gaps.rs` drives the host's ordering and round.

use ravel_sdk::interception::{InterceptionDeclaration, Occasion, Outcome, Point};
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
/// use ravel_test_mod_interception::InterceptionMod;
///
/// // Guarding, it refuses; the host ends the round there.
/// let mut guarding = InterceptionMod::guarding();
/// assert_eq!(guarding.answer(), Outcome::Refuse);
///
/// // Not guarding, it has nothing to say and the round carries on.
/// let mut open = InterceptionMod::new();
/// assert_eq!(open.answer(), Outcome::Proceed);
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InterceptionMod {
    entered: Vec<LifecycleState>,
    guarding: bool,
    asked: Vec<Point>,
}

impl InterceptionMod {
    /// Builds the mod instance the loader drives.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds it with the condition it refuses under already holding.
    #[must_use]
    pub fn guarding() -> Self {
        Self {
            guarding: true,
            ..Self::default()
        }
    }

    /// Returns the states this mod was taken through, in order.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }

    /// Returns the points this mod has been asked at, in order.
    ///
    /// A mod that was never asked and a mod that answered `Proceed` are
    /// indistinguishable from the outside, and they are different things.
    #[must_use]
    pub fn asked(&self) -> &[Point] {
        &self.asked
    }

    /// Returns the mod this one is declared to be asked before.
    ///
    /// # Panics
    ///
    /// Panics if `follower_mod` stops being a legal mod id.
    #[must_use]
    pub fn follower() -> ModId {
        "follower_mod".parse().expect("a legal mod id")
    }

    /// Answers as the host would ask it to, without a host.
    ///
    /// The condition is this mod's own: what a mod at a point decides is the
    /// mod's business, and Ravel owns only the order and what a refusal means.
    pub fn answer(&mut self) -> Outcome {
        if self.guarding {
            Outcome::Refuse
        } else {
            Outcome::Proceed
        }
    }
}

#[declare_mod(id = "interception")]
impl Mod for InterceptionMod {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        self.entered.push(state);
        Ok(())
    }

    fn interception_declaration(&self) -> Result<Option<InterceptionDeclaration>, ModError> {
        let mut declared = InterceptionDeclaration::new(Self::declared_id().clone());
        declared.answer_at(POINT);
        Ok(Some(declared))
    }

    fn intercept(&mut self, occasion: Occasion) -> Result<Outcome, ModError> {
        self.asked.push(occasion.point());
        Ok(self.answer())
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::interception::{Occasion, Outcome, Point};
    use ravel_sdk::{Declared, Mod, Side};

    use super::{InterceptionMod, MOD_ID, POINT};

    #[test]
    fn the_declared_identity_is_the_one_this_mod_is_packaged_under() {
        assert_eq!(InterceptionMod::declared_id().as_str(), MOD_ID);
    }

    /// The declaration is the mod's whole half of the contract.
    #[test]
    fn this_mod_declares_the_point_it_answers_at_and_owns_the_declaration() {
        let declared = InterceptionMod::new()
            .interception_declaration()
            .expect("valid interception declaration")
            .expect("this mod answers somewhere");

        assert_eq!(declared.owner(), InterceptionMod::declared_id());
        assert_eq!(declared.points(), [POINT]);
        assert!(declared.answers_at(POINT));
        assert!(
            !declared.answers_at(Point::WorldIsSaved),
            "a point this mod never declared"
        );
    }

    /// Being asked is observable, and the answer is this mod's own condition.
    #[test]
    fn the_mod_records_being_asked_and_refuses_only_while_guarding() {
        let occasion = Occasion::at(POINT, Side::IntegratedServer);

        let mut open = InterceptionMod::new();
        assert_eq!(open.intercept(occasion), Ok(Outcome::Proceed));
        assert_eq!(open.asked(), [POINT]);

        let mut guarding = InterceptionMod::guarding();
        assert_eq!(guarding.intercept(occasion), Ok(Outcome::Refuse));
        assert_eq!(guarding.asked(), [POINT]);
    }

    #[test]
    fn the_point_this_mod_answers_at_is_reached_on_server_sides() {
        assert!(POINT.happens_on(Side::IntegratedServer));
        assert!(POINT.happens_on(Side::DedicatedServer));
        assert!(!POINT.happens_on(Side::Client));
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(InterceptionMod::new());
