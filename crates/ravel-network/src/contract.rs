use std::fmt;

use ravel_core::{ContentId, ModId, Side};

use crate::{PacketReader, PacketWriter, WireError};

/// Which side receives a packet.
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

/// A packet contract's namespaced identity and routing owner.
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

/// A packet's identity, direction, and wire version.
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

/// A packet body; [`crate::encode`] and [`crate::decode`] handle its contract framing.
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
