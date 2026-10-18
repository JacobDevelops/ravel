//! Ordered answers at published interception points.
//!
//! Hosts collect typed declarations in [`Points`], consume them with [`Points::admit`]
//! at the declaration barrier, and reuse the resulting immutable [`AdmittedPoints`]
//! for rounds. Admission validates every point before any callback runs, rejecting
//! ordering endpoints that answer no point and naming the edge's declaring owners.
//! Each round filters its cached order against owners enabled on its execution side.
//!
//! Declare points with [`InterceptionDeclaration`] and implement [`crate::Mod::intercept`].
//! The first refusal ends a round; each point permits at most one replacement.
//! Ordering may connect owners answering different points, but both must answer somewhere.
//! Only [`Point::WorldIsSaved`] is delivered today; refusal withholds Ravel's
//! write, not the game's save. On a client process it executes on the integrated-server
//! side and excludes client-only owners. Other points remain declaration-only.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::interception::{InterceptionDeclaration, Occasion, Outcome, Point};
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
//!         declared.answer_at(Point::BlockIsBroken);
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
    AdmittedPoints, InterceptionDeclaration, InterceptionRefused, Occasion, Outcome, Point, Points,
    Round,
};
