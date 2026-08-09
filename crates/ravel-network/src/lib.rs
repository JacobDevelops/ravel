#![forbid(unsafe_code)]

//! Ravel-native networking abstractions independent of `NeoForge` identity.
//!
//! A mod that talks between a client and a server declares a **contract** — a
//! name, a direction, and a version — and implements [`Packet`] for the value
//! that travels under it. [`encode`] and [`decode`] are the whole wire: they put
//! the contract's name and version ahead of the body, so a payload cannot be read
//! as a packet it was not written as, and a mod's own `write`/`read` never see the
//! framing.
//!
//! # What the wire refuses
//!
//! A byte string that arrived from somewhere else is the second thing in Ravel
//! nobody in this workspace authored — the first is the text a mod puts in a
//! refusal — so every reading of it is fallible and nothing is assumed:
//!
//! - a buffer that ends inside a field is [`WireError::Truncated`];
//! - a buffer with bytes left after the body is [`WireError::TrailingBytes`],
//!   because a decoder that ignored them would accept two different byte strings
//!   as the same packet;
//! - a version that is not the contract's is [`WireError::Version`], which is
//!   what stops one side's idea of a packet being read with the other's;
//! - a name that is not the contract's is [`WireError::Identity`];
//! - and text that is not UTF-8 is [`WireError::NotUtf8`].
//!
//! None of these is a variant a mod has to remember to check. They are the return
//! type.
//!
//! # Sides
//!
//! A [`Direction`] says which side may send a packet and which may receive it,
//! and `Side::is_server` is the whole rule: a `ToClient` packet is sent from a
//! server side and received on the client, and a `ToServer` packet is the
//! reverse. Single-player runs a client and an integrated server as two sides, so
//! an integrated server receives what a dedicated server does; that is what
//! `ravel_core::Side` already means and this crate does not redefine it.
//!
//! [`PacketDelivery::decode`] applies that rule before it decodes anything, which
//! is how a client-bound packet reaching a server side is refused rather than
//! read. `ravel_core::ClientOnly` is the other half of the same requirement, for
//! the client-only code a packet handler would otherwise have run.
//!
//! ```
//! use ravel_core::Side;
//! use ravel_network::{Direction, Packet, PacketContract, PacketId, PacketReader, PacketWriter};
//!
//! struct Ping {
//!     nonce: u32,
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
//!     }
//!
//!     fn read(reader: &mut PacketReader<'_>) -> Result<Self, ravel_network::WireError> {
//!         Ok(Self {
//!             nonce: reader.u32()?,
//!         })
//!     }
//! }
//!
//! let encoded = ravel_network::encode(&Ping { nonce: 9 });
//! assert_eq!(ravel_network::decode::<Ping>(&encoded)?.nonce, 9);
//!
//! // A client sends it and a server receives it, never the other way round.
//! assert!(Direction::ToServer.may_send_from(Side::Client));
//! assert!(!Direction::ToServer.may_receive_on(Side::Client));
//! # Ok::<(), ravel_network::WireError>(())
//! ```

use std::fmt;

use ravel_core::{ContentId, ModId, Side};

/// Which way a packet travels.
///
/// A packet goes to the server or to a client and there is no other way for it to
/// go: a "both" direction would be a packet whose side rule is decided at each call
/// rather than by its contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Direction {
    /// Sent by the client and received on a server side.
    ToServer,
    /// Sent by a server side and received on the client.
    ToClient,
}

impl Direction {
    /// Every direction, in the order this enum declares them.
    pub const ALL: [Self; 2] = [Self::ToServer, Self::ToClient];

    /// Returns the direction's canonical lowercase name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ToServer => "to-server",
            Self::ToClient => "to-client",
        }
    }

    /// Returns whether `side` may send a packet travelling this way.
    #[must_use]
    pub const fn may_send_from(self, side: Side) -> bool {
        match self {
            Self::ToServer => !side.is_server(),
            Self::ToClient => side.is_server(),
        }
    }

    /// Returns whether `side` may receive a packet travelling this way.
    ///
    /// The exact complement of [`Direction::may_send_from`], because one side of
    /// a link sends what the other receives. Writing it as one function that both
    /// callers negated would read worse at every call site and say the same
    /// thing.
    #[must_use]
    pub const fn may_receive_on(self, side: Side) -> bool {
        match self {
            Self::ToServer => side.is_server(),
            Self::ToClient => !side.is_server(),
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The name one packet contract is known by.
///
/// A packet name is namespaced by the mod that owns the contract, which is what
/// makes it unique across an install without a registration barrier: two mods
/// cannot spell the same [`PacketId`] the way two mods can both want
/// `minecraft:stone`. The owner is also who a delivery routes to, so the name and
/// the recipient cannot disagree.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PacketId(ContentId);

impl PacketId {
    /// Names a contract.
    #[must_use]
    pub const fn new(id: ContentId) -> Self {
        Self(id)
    }

    /// Returns the mod that owns the contract, and receives what is sent under it.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        self.0.namespace()
    }

    /// Returns the underlying content identifier.
    #[must_use]
    pub const fn id(&self) -> &ContentId {
        &self.0
    }
}

impl fmt::Display for PacketId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// What one packet is, apart from its payload.
///
/// The version is the contract's rather than the payload's. A mod that changes
/// what it sends raises this, and the other end refuses the old spelling instead
/// of reading new fields out of an old buffer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PacketContract {
    id: PacketId,
    direction: Direction,
    version: u32,
}

impl PacketContract {
    /// Declares a contract.
    #[must_use]
    pub const fn new(id: PacketId, direction: Direction, version: u32) -> Self {
        Self {
            id,
            direction,
            version,
        }
    }

    /// Returns the contract's name.
    #[must_use]
    pub const fn id(&self) -> &PacketId {
        &self.id
    }

    /// Returns which way packets under this contract travel.
    #[must_use]
    pub const fn direction(&self) -> Direction {
        self.direction
    }

    /// Returns the contract's version.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }
}

/// A value that travels under a [`PacketContract`].
///
/// [`Packet::write`] cannot fail, because a value that exists can always be
/// written; [`Packet::read`] can, because bytes that arrived can be anything.
/// Neither sees the contract's name or version — [`encode`] and [`decode`] own
/// the framing, so a mod cannot write one framing and read another.
pub trait Packet: Sized {
    /// Returns the contract this type travels under.
    fn contract() -> PacketContract;

    /// Writes the body.
    fn write(&self, writer: &mut PacketWriter);

    /// Reads the body.
    ///
    /// # Errors
    ///
    /// Returns [`WireError`] when the bytes do not spell this packet.
    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError>;
}

/// Appends fields to a packet body.
///
/// Every integer is big-endian and every variable-length field carries a `u32`
/// length ahead of it, so the format is the same on every platform Ravel builds
/// for and a reader never has to guess where a field ends.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PacketWriter {
    bytes: Vec<u8>,
}

impl PacketWriter {
    /// Appends a 32-bit unsigned integer.
    pub fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    /// Appends a boolean as one byte.
    pub fn bool(&mut self, value: bool) {
        self.bytes.push(u8::from(value));
    }

    /// Appends a length-prefixed UTF-8 string.
    ///
    /// # Panics
    ///
    /// Panics when the text is longer than [`u32::MAX`] bytes, which is a value
    /// no packet body can carry and no caller in this workspace produces.
    pub fn text(&mut self, value: &str) {
        self.bytes_field(value.as_bytes());
    }

    /// Appends a length-prefixed byte string.
    ///
    /// # Panics
    ///
    /// Panics for the reason [`PacketWriter::text`] does.
    pub fn bytes_field(&mut self, value: &[u8]) {
        let length = u32::try_from(value.len()).expect("a packet field shorter than 4 GiB");
        self.u32(length);
        self.bytes.extend_from_slice(value);
    }

    /// Returns the bytes written so far.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

/// Reads fields from a packet body, refusing everything the bytes do not say.
#[derive(Clone, Debug)]
pub struct PacketReader<'bytes> {
    bytes: &'bytes [u8],
    read: usize,
}

impl<'bytes> PacketReader<'bytes> {
    /// Starts reading `bytes`.
    #[must_use]
    pub const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, read: 0 }
    }

    /// Returns how many bytes have not been read.
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.bytes.len() - self.read
    }

    /// Reads a 32-bit unsigned integer.
    ///
    /// # Errors
    ///
    /// Returns [`WireError::Truncated`] when fewer than four bytes remain.
    pub fn u32(&mut self) -> Result<u32, WireError> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Reads a boolean.
    ///
    /// # Errors
    ///
    /// Returns [`WireError::Truncated`] when no byte remains, and
    /// [`WireError::NotABool`] for a byte that is neither zero nor one. A third
    /// value is not a `true` with rubbish in it; it is a buffer that was not
    /// written by this writer.
    pub fn bool(&mut self) -> Result<bool, WireError> {
        match self.take(1)?[0] {
            0 => Ok(false),
            1 => Ok(true),
            byte => Err(WireError::NotABool { byte }),
        }
    }

    /// Reads a length-prefixed UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns [`WireError::Truncated`] when the buffer ends inside the field and
    /// [`WireError::NotUtf8`] when the bytes are not UTF-8.
    pub fn text(&mut self) -> Result<&'bytes str, WireError> {
        let bytes = self.bytes_field()?;
        std::str::from_utf8(bytes).map_err(|_| WireError::NotUtf8)
    }

    /// Reads a length-prefixed byte string.
    ///
    /// # Errors
    ///
    /// Returns [`WireError::Truncated`] when the buffer ends inside the field.
    pub fn bytes_field(&mut self) -> Result<&'bytes [u8], WireError> {
        let length = self.u32()? as usize;
        self.take(length)
    }

    /// Refuses any byte the body did not account for.
    ///
    /// # Errors
    ///
    /// Returns [`WireError::TrailingBytes`] when bytes remain. A decoder that
    /// ignored them would read two different byte strings as one packet, and the
    /// difference is what an attacker would put there.
    pub const fn finish(self) -> Result<(), WireError> {
        let unread = self.remaining();
        if unread == 0 {
            Ok(())
        } else {
            Err(WireError::TrailingBytes { unread })
        }
    }

    /// Takes `count` bytes, or refuses because the buffer ends first.
    fn take(&mut self, count: usize) -> Result<&'bytes [u8], WireError> {
        let available = self.remaining();
        if count > available {
            return Err(WireError::Truncated {
                needed: count,
                available,
            });
        }
        let taken = &self.bytes[self.read..self.read + count];
        self.read += count;
        Ok(taken)
    }
}

/// Why a byte string is not the packet it was read as.
///
/// Every variant carries what was expected and what was there, because a report
/// that only said "malformed" would leave a player with nothing and a mod author
/// with less.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WireError {
    /// The buffer ended inside a field.
    Truncated {
        /// The bytes the field needed.
        needed: usize,
        /// The bytes that were left.
        available: usize,
    },
    /// The body ended and bytes remained.
    TrailingBytes {
        /// How many bytes nothing accounted for.
        unread: usize,
    },
    /// A length-prefixed text field was not UTF-8.
    NotUtf8,
    /// A boolean byte was neither zero nor one.
    NotABool {
        /// The byte that was there.
        byte: u8,
    },
    /// The payload names a contract other than the one it was read as.
    Identity {
        /// The contract the reader expected.
        expected: String,
        /// The contract the payload named.
        found: String,
    },
    /// The payload was written under another version of the contract.
    Version {
        /// The version the reader expected.
        expected: u32,
        /// The version the payload named.
        found: u32,
    },
}

impl fmt::Display for WireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { needed, available } => write!(
                formatter,
                "the packet ends early: a field needs {needed} bytes and {available} are left"
            ),
            Self::TrailingBytes { unread } => write!(
                formatter,
                "the packet body ended with {unread} bytes nothing accounts for"
            ),
            Self::NotUtf8 => formatter.write_str("a packet text field is not UTF-8"),
            Self::NotABool { byte } => {
                write!(formatter, "`{byte}` is not a boolean; a boolean is 0 or 1")
            }
            Self::Identity { expected, found } => write!(
                formatter,
                "the payload names `{found}` and was read as `{expected}`"
            ),
            Self::Version { expected, found } => write!(
                formatter,
                "the payload is version {found} and this end speaks version {expected}"
            ),
        }
    }
}

impl std::error::Error for WireError {}

/// Encodes `packet` under its own contract.
///
/// The contract's name and version go ahead of the body, which is what makes
/// [`decode`] able to refuse a payload rather than misread one.
pub fn encode<P: Packet>(packet: &P) -> Vec<u8> {
    let contract = P::contract();
    let mut writer = PacketWriter::default();
    writer.text(&contract.id().to_string());
    writer.u32(contract.version());
    packet.write(&mut writer);
    writer.finish()
}

/// Encodes `packet` for sending from `side`.
///
/// # Errors
///
/// Returns [`DeliveryRefused::WrongSide`] when `side` may not send a packet
/// travelling this contract's way — a client sending a client-bound packet, or a
/// server sending a server-bound one.
pub fn encode_from<P: Packet>(packet: &P, side: Side) -> Result<Vec<u8>, DeliveryRefused> {
    let contract = P::contract();
    if !contract.direction().may_send_from(side) {
        return Err(DeliveryRefused::WrongSide {
            id: contract.id().clone(),
            direction: contract.direction(),
            side,
        });
    }
    Ok(encode(packet))
}

/// Decodes a payload as `P`, refusing anything that is not exactly one `P`.
///
/// # Errors
///
/// Returns [`WireError`] for a payload naming another contract, written under
/// another version, ending early, or ending with bytes left over.
pub fn decode<P: Packet>(bytes: &[u8]) -> Result<P, WireError> {
    let contract = P::contract();
    let mut reader = PacketReader::new(bytes);

    let named = reader.text()?;
    let expected = contract.id().to_string();
    if named != expected {
        return Err(WireError::Identity {
            expected,
            found: named.to_owned(),
        });
    }
    let version = reader.u32()?;
    if version != contract.version() {
        return Err(WireError::Version {
            expected: contract.version(),
            found: version,
        });
    }

    let packet = P::read(&mut reader)?;
    reader.finish()?;
    Ok(packet)
}

/// One packet as it reaches the side that is meant to read it.
///
/// The shape mirrors `ravel_host_api::CommandInvocation`, and for the same
/// reason: what reaches a mod is a thing that was addressed to it, the side it
/// arrived on, and bytes nobody in this workspace wrote.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PacketDelivery {
    /// The contract the payload claims to be written under.
    ///
    /// Claims, not is. [`PacketDelivery::decode`] checks it against the payload's
    /// own framing, so a delivery that names one contract and carries another is
    /// refused rather than trusted.
    pub id: PacketId,
    /// Which way the packet travelled.
    pub direction: Direction,
    /// The side the packet arrived on.
    pub side: Side,
    /// The encoded packet, exactly as the sender produced it.
    pub payload: Vec<u8>,
}

impl PacketDelivery {
    /// Records a delivery.
    #[must_use]
    pub const fn new(id: PacketId, direction: Direction, side: Side, payload: Vec<u8>) -> Self {
        Self {
            id,
            direction,
            side,
            payload,
        }
    }

    /// Builds the delivery `packet` would arrive as on `side`.
    ///
    /// This is the sending half of a link, kept here rather than in a transport
    /// so a test and a running game encode the same bytes.
    ///
    /// # Errors
    ///
    /// Returns [`DeliveryRefused::WrongSide`] when `side` may not receive a
    /// packet travelling this contract's way.
    pub fn of<P: Packet>(packet: &P, side: Side) -> Result<Self, DeliveryRefused> {
        let contract = P::contract();
        if !contract.direction().may_receive_on(side) {
            return Err(DeliveryRefused::WrongSide {
                id: contract.id().clone(),
                direction: contract.direction(),
                side,
            });
        }
        Ok(Self::new(
            contract.id().clone(),
            contract.direction(),
            side,
            encode(packet),
        ))
    }

    /// Reads this delivery as `P`.
    ///
    /// The side rule is applied before the payload is looked at, which is the
    /// order that matters: a client-bound packet on a server side is refused
    /// without any of the mod's own reading code running.
    ///
    /// # Errors
    ///
    /// Returns [`DeliveryRefused::WrongSide`] when this side may not receive the
    /// contract, [`DeliveryRefused::WrongContract`] when the delivery is
    /// addressed to another contract, [`DeliveryRefused::WrongDirection`] when it
    /// claims a direction the contract does not have, and
    /// [`DeliveryRefused::Malformed`] when the payload is not one `P`.
    pub fn decode<P: Packet>(&self) -> Result<P, DeliveryRefused> {
        let contract = P::contract();
        if !contract.direction().may_receive_on(self.side) {
            return Err(DeliveryRefused::WrongSide {
                id: contract.id().clone(),
                direction: contract.direction(),
                side: self.side,
            });
        }
        if &self.id != contract.id() {
            return Err(DeliveryRefused::WrongContract {
                expected: contract.id().clone(),
                found: self.id.clone(),
            });
        }
        if self.direction != contract.direction() {
            return Err(DeliveryRefused::WrongDirection {
                id: contract.id().clone(),
                expected: contract.direction(),
                found: self.direction,
            });
        }
        decode(&self.payload).map_err(|source| DeliveryRefused::Malformed {
            id: contract.id().clone(),
            source,
        })
    }
}

/// Why a packet was not read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeliveryRefused {
    /// The side may not send or receive a packet travelling this way.
    WrongSide {
        /// The contract that was refused.
        id: PacketId,
        /// The way it travels.
        direction: Direction,
        /// The side that asked.
        side: Side,
    },
    /// The delivery is addressed to another contract.
    WrongContract {
        /// The contract it was read as.
        expected: PacketId,
        /// The contract it names.
        found: PacketId,
    },
    /// The delivery claims a direction its contract does not have.
    WrongDirection {
        /// The contract.
        id: PacketId,
        /// The way the contract travels.
        expected: Direction,
        /// The way the delivery claims to have travelled.
        found: Direction,
    },
    /// The payload is not one packet of this contract.
    Malformed {
        /// The contract it was read as.
        id: PacketId,
        /// Why the bytes were refused.
        source: WireError,
    },
}

impl fmt::Display for DeliveryRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSide {
                id,
                direction,
                side,
            } => write!(
                formatter,
                "`{id}` travels {direction} and the `{}` side is not an end of that",
                side.name()
            ),
            Self::WrongContract { expected, found } => write!(
                formatter,
                "the delivery names `{found}` and was read as `{expected}`"
            ),
            Self::WrongDirection {
                id,
                expected,
                found,
            } => write!(
                formatter,
                "`{id}` travels {expected} and the delivery claims {found}"
            ),
            Self::Malformed { id, source } => write!(formatter, "`{id}` was not read: {source}"),
        }
    }
}

impl std::error::Error for DeliveryRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Malformed { source, .. } => Some(source),
            Self::WrongSide { .. } | Self::WrongContract { .. } | Self::WrongDirection { .. } => {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::Side;

    use super::{
        DeliveryRefused, Direction, Packet, PacketContract, PacketDelivery, PacketId, PacketReader,
        PacketWriter, WireError, decode, encode, encode_from,
    };

    fn packet_id(text: &str) -> PacketId {
        PacketId::new(text.parse().expect("a legal content id"))
    }

    /// A packet with one field of every kind the writer offers.
    #[derive(Clone, Debug, Eq, PartialEq)]
    struct Every {
        count: u32,
        flag: bool,
        label: String,
        blob: Vec<u8>,
    }

    impl Packet for Every {
        fn contract() -> PacketContract {
            PacketContract::new(packet_id("example_mod:every"), Direction::ToServer, 3)
        }

        fn write(&self, writer: &mut PacketWriter) {
            writer.u32(self.count);
            writer.bool(self.flag);
            writer.text(&self.label);
            writer.bytes_field(&self.blob);
        }

        fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
            Ok(Self {
                count: reader.u32()?,
                flag: reader.bool()?,
                label: reader.text()?.to_owned(),
                blob: reader.bytes_field()?.to_vec(),
            })
        }
    }

    /// The same body under a client-bound contract, for the side cases.
    #[derive(Clone, Debug, Eq, PartialEq)]
    struct Inbound {
        count: u32,
    }

    impl Packet for Inbound {
        fn contract() -> PacketContract {
            PacketContract::new(packet_id("example_mod:inbound"), Direction::ToClient, 1)
        }

        fn write(&self, writer: &mut PacketWriter) {
            writer.u32(self.count);
        }

        fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
            Ok(Self {
                count: reader.u32()?,
            })
        }
    }

    fn every() -> Every {
        Every {
            count: 0x0102_0304,
            flag: true,
            label: "copper lantern".to_owned(),
            blob: vec![0, 255, 7],
        }
    }

    #[test]
    fn every_field_kind_survives_a_round_trip() {
        let original = every();
        let decoded: Every = decode(&encode(&original)).expect("its own bytes");
        assert_eq!(decoded, original);
    }

    #[test]
    fn the_empty_values_of_every_field_kind_survive_too() {
        // The edges a length prefix gets wrong: an empty string and an empty byte
        // string are not the same thing as an absent field.
        let original = Every {
            count: 0,
            flag: false,
            label: String::new(),
            blob: Vec::new(),
        };
        assert_eq!(
            decode::<Every>(&encode(&original)).expect("its own bytes"),
            original
        );
    }

    #[test]
    fn encoding_the_same_packet_twice_produces_the_same_bytes() {
        assert_eq!(encode(&every()), encode(&every()));
    }

    #[test]
    fn a_truncated_payload_is_refused_at_every_length_it_could_be_cut_to() {
        let full = encode(&every());
        for cut in 0..full.len() {
            let error = decode::<Every>(&full[..cut]).expect_err("the payload ends early");
            assert!(
                matches!(
                    error,
                    WireError::Truncated { .. } | WireError::Identity { .. } | WireError::NotUtf8
                ),
                "cut at {cut}: {error}"
            );
        }
        assert!(decode::<Every>(&full).is_ok());
    }

    #[test]
    fn a_payload_with_bytes_left_over_is_refused_rather_than_read() {
        let mut trailing = encode(&every());
        trailing.push(0);

        assert_eq!(
            decode::<Every>(&trailing),
            Err(WireError::TrailingBytes { unread: 1 })
        );
    }

    #[test]
    fn a_payload_written_under_another_version_is_refused() {
        // The version sits after the length-prefixed name, so this rewrites the
        // four bytes at that offset rather than assuming where they are.
        let mut other = encode(&every());
        let name = Every::contract().id().to_string();
        let offset = 4 + name.len();
        other[offset..offset + 4].copy_from_slice(&99_u32.to_be_bytes());

        assert_eq!(
            decode::<Every>(&other),
            Err(WireError::Version {
                expected: 3,
                found: 99,
            })
        );
    }

    #[test]
    fn a_payload_written_under_another_contract_is_refused() {
        let error = decode::<Inbound>(&encode(&every())).expect_err("another contract");
        assert_eq!(
            error,
            WireError::Identity {
                expected: "example_mod:inbound".to_owned(),
                found: "example_mod:every".to_owned(),
            }
        );
        assert!(error.to_string().contains("example_mod:every"), "{error}");
    }

    #[test]
    fn a_boolean_byte_that_is_neither_zero_nor_one_is_refused() {
        let mut wrong = encode(&every());
        let offset = 4 + Every::contract().id().to_string().len() + 4 + 4;
        wrong[offset] = 2;

        assert_eq!(
            decode::<Every>(&wrong),
            Err(WireError::NotABool { byte: 2 })
        );
    }

    #[test]
    fn a_text_field_that_is_not_utf8_is_refused() {
        let mut reader = PacketReader::new(&[0, 0, 0, 1, 0xFF]);
        assert_eq!(reader.text(), Err(WireError::NotUtf8));
    }

    #[test]
    fn a_length_prefix_longer_than_the_buffer_is_truncation_and_not_a_panic() {
        let mut reader = PacketReader::new(&[0xFF, 0xFF, 0xFF, 0xFF, 1, 2, 3]);
        assert_eq!(
            reader.bytes_field(),
            Err(WireError::Truncated {
                needed: 0xFFFF_FFFF,
                available: 3,
            })
        );
    }

    #[test]
    fn each_direction_has_exactly_one_sending_side_group_and_one_receiving_one() {
        for side in Side::ALL {
            for direction in Direction::ALL {
                assert_ne!(
                    direction.may_send_from(side),
                    direction.may_receive_on(side),
                    "{direction} on {}",
                    side.name()
                );
            }
        }

        assert!(Direction::ToServer.may_send_from(Side::Client));
        assert!(Direction::ToServer.may_receive_on(Side::IntegratedServer));
        assert!(Direction::ToServer.may_receive_on(Side::DedicatedServer));
        assert!(Direction::ToClient.may_receive_on(Side::Client));
        assert!(!Direction::ToClient.may_receive_on(Side::DedicatedServer));
        assert_eq!(Direction::ToClient.to_string(), "to-client");
    }

    #[test]
    fn a_side_that_may_not_send_a_contract_encodes_nothing() {
        let error =
            encode_from(&every(), Side::DedicatedServer).expect_err("a server sends it not");
        assert_eq!(
            error,
            DeliveryRefused::WrongSide {
                id: packet_id("example_mod:every"),
                direction: Direction::ToServer,
                side: Side::DedicatedServer,
            }
        );
        assert!(error.to_string().contains("dedicated-server"), "{error}");
        assert!(encode_from(&every(), Side::Client).is_ok());
    }

    #[test]
    fn a_client_bound_delivery_is_refused_on_every_server_side() {
        for side in Side::ALL {
            let built = PacketDelivery::of(&Inbound { count: 5 }, side);
            if side.is_server() {
                assert!(
                    matches!(built, Err(DeliveryRefused::WrongSide { .. })),
                    "{}",
                    side.name()
                );
                continue;
            }
            let delivery = built.expect("the client receives it");
            assert_eq!(
                delivery.decode::<Inbound>().expect("its own contract"),
                Inbound { count: 5 }
            );

            // The same bytes, handed to a server side, are refused before the
            // packet's own reading code runs.
            let forced = PacketDelivery::new(
                delivery.id.clone(),
                delivery.direction,
                Side::DedicatedServer,
                delivery.payload.clone(),
            );
            assert!(matches!(
                forced.decode::<Inbound>(),
                Err(DeliveryRefused::WrongSide { .. })
            ));
        }
    }

    #[test]
    fn a_delivery_addressed_to_another_contract_is_refused() {
        let delivery = PacketDelivery::of(&Inbound { count: 1 }, Side::Client)
            .expect("the client receives it");
        let mislabelled = PacketDelivery::new(
            packet_id("example_mod:other"),
            Direction::ToClient,
            Side::Client,
            delivery.payload.clone(),
        );

        assert_eq!(
            mislabelled.decode::<Inbound>(),
            Err(DeliveryRefused::WrongContract {
                expected: packet_id("example_mod:inbound"),
                found: packet_id("example_mod:other"),
            })
        );
    }

    #[test]
    fn a_delivery_claiming_the_wrong_direction_is_refused() {
        let delivery = PacketDelivery::of(&Inbound { count: 1 }, Side::Client)
            .expect("the client receives it");
        let lying = PacketDelivery::new(
            delivery.id.clone(),
            Direction::ToServer,
            Side::Client,
            delivery.payload.clone(),
        );

        assert_eq!(
            lying.decode::<Inbound>(),
            Err(DeliveryRefused::WrongDirection {
                id: packet_id("example_mod:inbound"),
                expected: Direction::ToClient,
                found: Direction::ToServer,
            })
        );
    }

    #[test]
    fn a_malformed_payload_reaches_the_caller_with_the_wire_error_inside_it() {
        let delivery = PacketDelivery::new(
            packet_id("example_mod:inbound"),
            Direction::ToClient,
            Side::Client,
            Vec::new(),
        );

        let error = delivery
            .decode::<Inbound>()
            .expect_err("an empty payload is no packet");
        assert!(
            matches!(
                &error,
                DeliveryRefused::Malformed { source, .. } if matches!(source, WireError::Truncated { .. })
            ),
            "{error}"
        );
        assert!(std::error::Error::source(&error).is_some());
    }

    #[test]
    fn a_contract_names_its_owner_and_that_is_who_a_delivery_routes_to() {
        let contract = Every::contract();
        assert_eq!(contract.id().owner().as_str(), "example_mod");
        assert_eq!(contract.id().id().path(), "every");
        assert_eq!(contract.version(), 3);
        assert_eq!(contract.direction(), Direction::ToServer);
        assert_eq!(contract.id().to_string(), "example_mod:every");
    }

    #[test]
    fn a_reader_reports_what_it_has_not_read_yet() {
        let mut reader = PacketReader::new(&[0, 0, 0, 1, 9]);
        assert_eq!(reader.remaining(), 5);
        assert_eq!(reader.u32(), Ok(1));
        assert_eq!(reader.remaining(), 1);
        assert_eq!(reader.finish(), Err(WireError::TrailingBytes { unread: 1 }));
    }
}
