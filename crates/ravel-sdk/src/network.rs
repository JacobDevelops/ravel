//! Packet contracts, framing, and side validation; no network transport is provided.
//!
//! Implement [`Packet`] for a value and use [`encode`] and [`decode`] for framing.
//!
//! # Round trip and malformed payloads
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
//! [`PacketDelivery::decode`] validates the receiving side before parsing.
//! [`encode_from`] validates the sending side. No production path transports
//! these bytes or calls [`crate::Mod::receive_packet`]; see `docs/guide/limits.md`.

pub use ravel_network::{
    DeliveryRefused, Direction, Packet, PacketContract, PacketDelivery, PacketId, PacketReader,
    PacketWriter, WireError, decode, encode, encode_from,
};
