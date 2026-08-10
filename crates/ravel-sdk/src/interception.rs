//! Answering at a point in the game rather than being told what happened there.
//!
//! [`crate::events`] is what the game did. This is what the game is about to do:
//! Ravel publishes a closed set of [`Point`]s, a mod declares that it answers at
//! one, and the round asks every mod in the declared order while the outcome
//! still stands. The first [`Outcome::Refuse`] ends the round, so a mod that
//! refused is not overruled by a later one it never knew about.
//!
//! # Writing one
//!
//! Two methods on [`crate::Mod`], and both have defaults, so a mod that
//! intercepts nothing writes neither:
//!
//! - `interception_declaration` returns an [`InterceptionDeclaration`] naming
//!   the points you answer at and the mods you are asked after.
//! - `intercept` is what the host calls when it reaches one, and what you return
//!   is [`Outcome::Proceed`] or [`Outcome::Refuse`].
//!
//! Declaring a point and not implementing `intercept` is refused loudly rather
//! than treated as proceeding, for the same reason declaring settings and not
//! reading them is: a declaration nothing answers is a mod whose author believes
//! it decides something.
//!
//! A mod that needs the game's own outcome replaced rather than composed with
//! declares that instead, through [`InterceptionDeclaration::replace_at`].
//! Refusing and proceeding compose, because either answer is still the game's
//! own; replacing does not, so exactly one mod may do it at a point and the host
//! refuses a set where two do — at the barrier, naming both, rather than as a
//! race decided by load order.
//!
//! # What this is not
//!
//! It is not a way to rewrite the game's code. A point is a place the host
//! already calls out from, so an interception is a call the host makes into a
//! mod, never a patch a mod applies. Every maintained crate carries
//! `#![forbid(unsafe_code)]` and this module asks for no exception.
//!
//! A mod also never names a hook. A hook is a version's resolved target and
//! `docs/architecture/dependency-rules.md` keeps it out of this surface; a
//! [`Point`] is what one becomes by the time it reaches a mod, which is the same
//! relationship [`crate::events::GameEvent`] has and the reason both sets are
//! closed.
//!
//! # What is carried, and what is not
//!
//! **One point is reached in a running game: [`Point::WorldIsSaved`].** Its hook
//! is pinned and the host asks a round there, so a mod that refuses withholds
//! Ravel's own world write for that pass — which is exactly what that seam does,
//! and not more: it does not stop Minecraft saving the world. What was withheld
//! is offered again at the next save rather than discarded.
//!
//! [`Point::BlockIsBroken`] and [`Point::ItemIsUsed`] are declared and not yet
//! reached. No hook resolves them in the pinned version data, so a mod that
//! declares one is writing against a specification: the declaration, the order
//! and the refusals are all real and checked today, and the call is not.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::interception::{InterceptionDeclaration, Occasion, Outcome};
//! use ravel_sdk::prelude::*;
//!
//! #[derive(Default)]
//! struct Guard {
//!     armed: bool,
//! }
//!
//! #[declare_mod(id = "guard_mod")]
//! impl Mod for Guard {
//!     fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
//!         let _ = state;
//!         Ok(())
//!     }
//!
//!     fn interception_declaration(&self) -> Option<InterceptionDeclaration> {
//!         let mut declared = InterceptionDeclaration::new(Self::declared_id().clone());
//!         declared.answer_at("block_is_broken");
//!         // Asked before `ledger_mod`, so what this refuses the ledger never sees.
//!         Some(declared)
//!     }
//!
//!     fn intercept(&mut self, occasion: Occasion) -> Result<Outcome, ModError> {
//!         let _ = occasion.point();
//!         Ok(if self.armed {
//!             Outcome::Refuse
//!         } else {
//!             Outcome::Proceed
//!         })
//!     }
//! }
//!
//! let mut guard = Guard { armed: true };
//! let occasion = Occasion::at(
//!     ravel_sdk::interception::Point::BlockIsBroken,
//!     Side::IntegratedServer,
//! );
//! assert_eq!(guard.intercept(occasion), Ok(Outcome::Refuse));
//! ```

pub use ravel_host_api::{
    InterceptionDeclaration, InterceptionRefused, Occasion, Outcome, Point, Points, Round,
};
