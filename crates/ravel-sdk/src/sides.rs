//! Which side a mod runs on, and the code only one of them may run.
//!
//! A side is a role within a process rather than a process kind. Single-player
//! runs a client and an integrated server together, so a mod in a single-player
//! session is driven twice — once for each side — and that is why
//! [`Side::IntegratedServer`] receives what a dedicated server receives and not
//! what a client does.
//!
//! A mod declares the sides it loads on in its package metadata, and the loader
//! parks it before any of its code runs on a side it did not declare. That covers
//! a mod that is entirely client-side or entirely server-side. It does not cover
//! the ordinary case: a mod that loads everywhere and has client-only *parts*.
//!
//! [`ClientOnly`] is for those parts, and it is the whole of what an author needs
//! to write:
//!
//! ```
//! use ravel_sdk::sides::{ClientOnly, Side};
//!
//! /// Something a mod would only build where there is a screen to draw on.
//! struct Overlay {
//!     lines: Vec<String>,
//! }
//!
//! fn build_overlay() -> Overlay {
//!     Overlay {
//!         lines: vec!["copper lanterns: 3".to_owned()],
//!     }
//! }
//!
//! // The host tells a mod its side when it constructs it. On a server side the
//! // closure is never called, so the overlay is not built rather than being built
//! // and hidden.
//! let server = ClientOnly::built_on(Side::DedicatedServer, build_overlay);
//! assert!(!server.is_present());
//! assert!(server.get(Side::DedicatedServer).is_err());
//!
//! let client = ClientOnly::built_on(Side::Client, build_overlay);
//! assert_eq!(client.get(Side::Client)?.lines.len(), 1);
//! # Ok::<(), ravel_sdk::sides::WrongSide>(())
//! ```
//!
//! A [`WrongSide`] refusal is a mod's to convert, the way a registration refusal
//! is: a mod that treats reaching for client-only code on a server as fatal
//! returns [`crate::ModError::Refused`], and one that has nothing to draw carries
//! on.
//!
//! # What this does not promise
//!
//! Nothing here stops an author writing client-only code outside a
//! [`ClientOnly`]. Ravel does not read a mod's source, so no type could. What
//! holds is that an author has one obvious place to put it, that the place refuses
//! to run it on a server side, and that
//! `tests/conformance/tests/sides_and_packets.rs` proves both by arranging for the
//! code to be reachable on a server and requiring it not to run.

pub use ravel_core::{ClientOnly, EmptySideSet, Side, SideSet, WrongSide};
