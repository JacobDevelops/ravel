//! What a mod sends between a client and a server.
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
//! # Sides
//!
//! A [`Direction`] decides which side may send a contract and which may receive
//! it, and [`PacketDelivery::decode`] applies that rule before it looks at the
//! payload. A client-bound packet arriving on a server side is therefore refused
//! with none of the mod's reading code having run — which is the same requirement
//! [`crate::sides::ClientOnly`] holds for the rest of a mod's client-only work.

pub use ravel_network::{
    DeliveryRefused, Direction, Packet, PacketContract, PacketDelivery, PacketId, PacketReader,
    PacketWriter, WireError, decode, encode, encode_from,
};
