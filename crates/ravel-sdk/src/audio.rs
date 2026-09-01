//! Sounds a mod owns, and the side that may play them.
//!
//! A mod could always carry a sound file into its pack as bytes through
//! [`crate::content::ContentDeclaration::resource`]. What it could not do is say
//! what plays them: nothing related the bytes to a call, so a caller had to know
//! a path, and nothing refused a call made where there is no sound hardware.
//!
//! [`AudioDeclaration`] is both halves in one statement. An event for an entry
//! the mod did not declare is refused where it is written, and
//! [`AudioDeclaration::play`] on a server side is refused rather than skipped.
//! The second is the one worth writing a rule for: a call that quietly does
//! nothing on one side is a mod that works in single player — where a client
//! happens to be in the same process — and is silent for everybody else.
//!
//! Declaring an entry is not playing one, so a dedicated server may own the
//! sounds it will never play. A mod is one mod on both sides.
//!
//! # How a request reaches the game
//!
//! [`AudioDeclaration::play`] answers with a [`PlayRequest`] and plays nothing
//! itself. Hold what it answered and hand it over from
//! [`crate::Mod::take_play_requests`], which the host asks after each delivery;
//! the host encodes it and the Java adapter asks the game's sound manager for
//! it. So the sound happens after the call that asked for it, and your mod
//! cannot learn from `play` whether the game did it.
//!
//! **No launched game has played one.** The crossing is built and tested on both
//! sides against a stand-in game; nobody has heard the sound. `docs/guide/limits.md`
//! is where that stays recorded until somebody has.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::audio::AudioDeclaration;
//! use ravel_sdk::{Declared, LifecycleState, Mod, ModError, Side, declare_mod};
//!
//! struct Example;
//!
//! #[declare_mod(id = "example")]
//! impl Mod for Example {
//!     fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
//!         let _ = state;
//!         Ok(())
//!     }
//! }
//!
//! let mut declared = AudioDeclaration::new(Example::declared_id().clone());
//! let chime = declared.entry("door_opens", b"the supplied bytes")?;
//! let event = declared.event("door_opens")?;
//! assert_eq!(event.entry(), chime.name());
//!
//! // An event with nothing to play is refused where it is written.
//! assert!(declared.event("door_closes").is_err());
//!
//! // The client plays it; a server is refused rather than ignored.
//! assert_eq!(declared.play("door_opens", Side::Client)?.entry(), "door_opens");
//! let refusal = declared
//!     .play("door_opens", Side::DedicatedServer)
//!     .expect_err("a server has nothing to play with");
//! assert_eq!(refusal.wrong_side(), Some(Side::DedicatedServer));
//! # Ok::<(), ravel_sdk::audio::AudioError>(())
//! ```

pub use ravel_data::{AudioDeclaration, AudioEntry, AudioError, AudioEvent, PlayRequest};
