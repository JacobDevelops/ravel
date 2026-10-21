//! Side roles and client-only state.
//!
//! Single-player runs both client and integrated-server roles. Package metadata
//! selects loading sides; [`ClientOnly`] prevents its constructor from running
//! on server sides. It provides no rendering path.
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
//! # Ok::<(), ravel_sdk::sides::ClientOnlyError>(())
//! ```
//!
//! Convert [`ClientOnlyError`] with [`crate::ModError::from_display`]
//! (for example, `.map_err(ModError::from_display)`) when missing state or wrong-side
//! access must refuse the operation. The host attributes the refusal to the hook it called;
//! whether it stops the mod depends on that hook's error contract.

pub use ravel_core::{ClientOnly, ClientOnlyError, EmptySideSet, Side, SideSet};
