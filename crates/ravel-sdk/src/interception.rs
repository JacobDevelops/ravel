//! Ordered answers at published interception points.
//!
//! Declare points with [`InterceptionDeclaration`] and implement [`crate::Mod::intercept`].
//! The first refusal ends a round; each point permits at most one replacement.
//! Only [`Point::WorldIsSaved`] is delivered today; refusal withholds Ravel's
//! write, not the game's save. Other points remain declaration-only.
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
//!     fn interception_declaration(&self) -> Result<Option<InterceptionDeclaration>, ModError> {
//!         let mut declared = InterceptionDeclaration::new(Self::declared_id().clone());
//!         declared.answer_at("block_is_broken");
//!         // Asked before `ledger_mod`, so what this refuses the ledger never sees.
//!         Ok(Some(declared))
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
