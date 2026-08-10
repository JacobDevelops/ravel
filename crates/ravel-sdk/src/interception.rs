//! Answering at a point in the game rather than being told what happened there.
//!
//! [`crate::events`] is what the game did. This is what the game is about to do:
//! Ravel publishes a closed set of [`Point`]s, a mod declares that it answers at
//! one, and the round asks every mod in the declared order while the outcome
//! still stands. The first [`Outcome::Refuse`] ends the round, so a mod that
//! refused is not overruled by a later one it never knew about.
//!
//! A mod that needs the game's own outcome replaced rather than composed with
//! declares that instead, through [`Points::replace_at`]. Refusing and
//! proceeding compose, because either answer is still the game's own; replacing
//! does not, so exactly one mod may do it at a point and [`Points::admit`]
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
//! **Nothing calls out at any point yet.** No host in this workspace asks a
//! round, exactly as none carries a packet. What ships is the declaration, the
//! order, and the two refusals, all decidable without a game.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::interception::{Outcome, Points};
//! use ravel_sdk::ModId;
//!
//! let guard: ModId = "guard_mod".parse()?;
//! let ledger: ModId = "ledger_mod".parse()?;
//!
//! let mut points = Points::default();
//! points.answer_at(&ledger, "block_is_broken", |_| Outcome::Proceed);
//! points.answer_at(&guard, "block_is_broken", |occasion| {
//!     let _ = occasion.point();
//!     Outcome::Refuse
//! });
//! // The guard answers first, so what it refuses the ledger never sees.
//! points.after(&ledger, &guard);
//!
//! let round = points.ask("block_is_broken").expect("a published point is asked");
//! assert_eq!(round.asked(), [guard.clone()]);
//! assert_eq!(round.outcome(), Outcome::Refuse);
//!
//! // Two mods replacing one outcome is refused before any round runs.
//! let mut contested = Points::default();
//! contested.replace_at(&guard, "block_is_broken", |_| Outcome::Proceed);
//! contested.replace_at(&ledger, "block_is_broken", |_| Outcome::Proceed);
//! let refusal = contested.admit().expect_err("two replacements are a conflict");
//! assert_eq!(refusal.point(), "block_is_broken");
//! assert_eq!(refusal.claimants(), [guard, ledger]);
//! # Ok::<(), ravel_sdk::ModIdError>(())
//! ```

pub use ravel_host_api::{InterceptionRefused, Occasion, Outcome, Point, Points, Round};
