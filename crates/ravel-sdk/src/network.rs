//! What a mod declares it sends between a client and a server.
//!
//! Declares, and today no more than declares: see "There is no send today" below
//! before you plan around this module.
//!
//! A mod declares a [`PacketContract`] — a name, a [`Direction`], and a version —
//! and implements [`Packet`] for the value that travels under it. That is the
//! whole of the author-facing surface: [`encode`] and [`decode`] own the framing,
//! so a mod writes and reads its own fields and never the envelope, and cannot
//! write one framing and read another.
//!
//! A packet name is namespaced by the mod that owns the contract, so it is unique
//! across an install without a registration barrier — two mods cannot spell the
//! same [`PacketId`] — and the owner is who the loader routes a delivery to. A mod
//! is never handed bytes for a contract it does not own.
//!
//! # Round trip, and the ways a payload is refused
//!
//! ```
//! use ravel_sdk::network::{
//!     Direction, Packet, PacketContract, PacketId, PacketReader, PacketWriter, WireError,
//! };
//!
//! #[derive(Debug, Eq, PartialEq)]
//! struct Ping {
//!     nonce: u32,
//!     note: String,
//! }
//!
//! impl Packet for Ping {
//!     fn contract() -> PacketContract {
//!         PacketContract::new(
//!             PacketId::new("example_mod:ping".parse().expect("a legal content id")),
//!             Direction::ToServer,
//!             1,
//!         )
//!     }
//!
//!     fn write(&self, writer: &mut PacketWriter) {
//!         writer.u32(self.nonce);
//!         writer.text(&self.note);
//!     }
//!
//!     fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
//!         Ok(Self {
//!             nonce: reader.u32()?,
//!             note: reader.text()?.to_owned(),
//!         })
//!     }
//! }
//!
//! let sent = Ping {
//!     nonce: 7,
//!     note: "hello".to_owned(),
//! };
//! let bytes = ravel_sdk::network::encode(&sent);
//! assert_eq!(ravel_sdk::network::decode::<Ping>(&bytes)?, sent);
//!
//! // A buffer that ends inside a field is refused, not read as a shorter packet.
//! assert!(ravel_sdk::network::decode::<Ping>(&bytes[..bytes.len() - 1]).is_err());
//!
//! // So is one with anything left over, because otherwise two different byte
//! // strings would decode to one packet.
//! let mut trailing = bytes.clone();
//! trailing.push(0);
//! assert!(matches!(
//!     ravel_sdk::network::decode::<Ping>(&trailing),
//!     Err(WireError::TrailingBytes { .. })
//! ));
//! # Ok::<(), WireError>(())
//! ```
//!
//! A version mismatch is the third: a mod that changes what it sends raises the
//! contract's version, and the other end returns [`WireError::Version`] instead of
//! reading new fields out of an old buffer.
//!
//! # Every way a payload is refused
//!
//! [`WireError`] is a closed set, and a mod that matches on it writes these arms
//! and no others:
//!
//! - [`WireError::Truncated`] — the buffer ended inside a field, carrying the
//!   bytes the field `needed` and the bytes `available`;
//! - [`WireError::TrailingBytes`] — the body ended with `unread` bytes nothing
//!   accounted for;
//! - [`WireError::NotUtf8`] — a text field's bytes are not UTF-8, and carries
//!   nothing;
//! - [`WireError::NotABool`] — a boolean `byte` was neither zero nor one;
//! - [`WireError::Identity`] — the payload names one contract and was read as
//!   another, carrying `expected` and `found` as strings;
//! - [`WireError::Version`] — the payload was written under another version of
//!   the contract, carrying `expected` and `found` as `u32`.
//!
//! Every one of them is [`std::fmt::Display`], so a mod that only reports the
//! refusal writes `error.to_string()` and never names an arm.
//!
//! # A contract is a triple, not a name
//!
//! [`PacketId`], like [`crate::content::RegistryKey`], [`crate::tags::TagId`] and
//! [`crate::services::ServiceId`], is one canonical string and implements
//! [`std::fmt::Display`]. [`PacketContract`] deliberately does not: it is a name
//! *and* a [`Direction`] *and* a version, and any single spelling of the three
//! would be a wire format nothing reads and nothing parses back. Read the parts
//! instead — [`PacketContract::id`], [`PacketContract::direction`] and
//! [`PacketContract::version`] — and print the id when a message wants a name.
//!
//! ```
//! use ravel_sdk::network::{Direction, PacketContract, PacketId};
//!
//! let contract = PacketContract::new(
//!     PacketId::new("example_mod:ping".parse().expect("a legal content id")),
//!     Direction::ToServer,
//!     1,
//! );
//!
//! assert_eq!(contract.id().to_string(), "example_mod:ping");
//! assert_eq!(contract.direction().to_string(), "to-server");
//! assert_eq!(contract.version(), 1);
//! assert_eq!(contract.id().owner().as_str(), "example_mod");
//! ```
//!
//! # Sides
//!
//! A [`Direction`] decides which side may send a contract and which may receive
//! it, and [`PacketDelivery::decode`] applies that rule before it looks at the
//! payload. A client-bound packet arriving on a server side is therefore refused
//! with none of the mod's reading code having run — which is the same requirement
//! [`crate::sides::ClientOnly`] holds for the rest of a mod's client-only work.
//!
//! # There is no send today
//!
//! **Nothing in Ravel carries a mod's packet to the other end.** This module is
//! the contract, the framing and the side rule, and all three are real: a value
//! round-trips, a payload is refused every way it can be wrong, and
//! [`encode_from`] refuses to produce bytes for a side that may not send them.
//! What does not exist is the leg after that. No wire adapter is written, no
//! launch path moves a byte between a client and a server on a mod's behalf, and
//! [`Mod::receive_packet`](crate::Mod::receive_packet) — the receiving half — has
//! no production caller either.
//!
//! So a mod can declare a contract, implement [`Packet`], encode a value and
//! prove the round trip in its own tests, and that is the whole of what it can do
//! now. `ROADMAP.md` places the wire adapters after the abstractions this module
//! is; until they land, a declared packet is a specification of what a mod will
//! send rather than something it sends. `docs/guide/limits.md` says the same in
//! an author's words.

pub use ravel_network::{
    DeliveryRefused, Direction, Packet, PacketContract, PacketDelivery, PacketId, PacketReader,
    PacketWriter, WireError, decode, encode, encode_from,
};
