//! Sound declarations and client-side playback requests.
//!
//! [`AudioDeclaration::play`] refuses server-side calls and returns a
//! [`PlayRequest`]; queue it for [`crate::Mod::take_play_requests`] to deliver.
//! Launched-game playback remains unverified; see `docs/guide/limits.md`.
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
//! let chime = declared.entry("door_opens", b"the supplied bytes".to_vec())?;
//! let event = declared.event("door_opens")?;
//! assert_eq!(event.entry(), chime.name());
//!
//! // An event with nothing to play is refused where it is written.
//! assert!(declared.event("door_closes").is_err());
//!
//! // The client plays it; a server is refused rather than ignored.
//! assert_eq!(declared.play(&event, Side::Client)?.entry(), "door_opens");
//! let refusal = declared
//!     .play(&event, Side::DedicatedServer)
//!     .expect_err("a server has nothing to play with");
//! assert_eq!(refusal.wrong_side(), Some(Side::DedicatedServer));
//! # Ok::<(), ravel_sdk::audio::AudioError>(())
//! ```

pub use ravel_data::{AudioDeclaration, AudioEntry, AudioError, AudioEvent, PlayRequest};
