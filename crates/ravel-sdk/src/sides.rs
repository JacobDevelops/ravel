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
//! to write.
//!
//! # What is a client-only part
//!
//! State a mod keeps only because it is on the client. A packet contract declared
//! [`crate::network::Direction::ToClient`] has the client as its only receiving
//! end, and [`crate::events::GameEvent::ClientTick`] happens on no server side, so
//! what such a packet last carried and what such an event last counted are values
//! a server side has no reason to hold.
//!
//! Drawing is not one of them. No mod rendering path is provided:
//! every screen Ravel owns is declared in data and drawn in Java,
//! and nothing extends that to a mod's own. A mod's client-only state is
//! what it *knows* on the client, not what it puts on the display.
//!
//! ```
//! use ravel_sdk::sides::{ClientOnly, Side};
//!
//! /// What a `to-client` packet last told this mod. Only the client receives one,
//! /// so only the client has anything to remember.
//! #[derive(Default)]
//! struct LatestReport {
//!     lines: Vec<String>,
//! }
//!
//! // The host tells a mod its side when it constructs it. On a server side the
//! // closure is never called, so the value is not built rather than being built
//! // and hidden.
//! let server = ClientOnly::built_on(Side::DedicatedServer, LatestReport::default);
//! assert!(!server.is_present());
//! assert!(server.get(Side::DedicatedServer).is_err());
//!
//! let mut client = ClientOnly::built_on(Side::Client, LatestReport::default);
//! client.get_mut(Side::Client)?.lines.push("copper lanterns: 3".to_owned());
//! assert_eq!(client.get(Side::Client)?.lines.len(), 1);
//! # Ok::<(), ravel_sdk::sides::WrongSide>(())
//! ```
//!
//! A [`WrongSide`] refusal is a mod's to convert, the way a registration refusal
//! is: a mod that treats reaching for client-only state on a server as fatal
//! returns [`crate::ModError::Refused`], and one that has nothing to report
//! carries on.
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
