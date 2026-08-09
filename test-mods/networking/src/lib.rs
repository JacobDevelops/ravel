#![forbid(unsafe_code)]

//! Side-aware networking abstraction test mod.
//!
//! The server half of M4-W3's pair. This mod owns one server-bound contract,
//! `networking:ping`, and it is what a client talks to; `ravel-test-mod-client-only`
//! owns the client-bound contract and is what talks back. The contracts are split
//! that way because a [`PacketId`] is namespaced by the mod that owns it and the
//! loader routes a delivery to that mod, so each end owns what it receives and
//! nothing has to be told who a packet is for.
//!
//! Every type here comes from `ravel-sdk`. `Cargo.toml` beside this file lists that
//! crate and nothing else, which is a mod author's whole manifest.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::network::{Direction, Packet, PacketDelivery};
//! use ravel_sdk::sides::Side;
//! use ravel_test_mod_networking::{NetworkingMod, Ping};
//!
//! // A client sends it; the encoding refuses a server trying to.
//! let ping = Ping::new(41);
//! assert!(ravel_sdk::network::encode_from(&ping, Side::DedicatedServer).is_err());
//! let bytes = ravel_sdk::network::encode_from(&ping, Side::Client)?;
//!
//! // And it round-trips into the same value on the other end.
//! let delivery = PacketDelivery::new(
//!     Ping::contract().id().clone(),
//!     Direction::ToServer,
//!     Side::DedicatedServer,
//!     bytes,
//! );
//! assert_eq!(delivery.decode::<Ping>()?.nonce(), 41);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use ravel_sdk::network::{
    Direction, Packet, PacketContract, PacketDelivery, PacketId, PacketReader, PacketWriter,
    WireError,
};
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod};

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "networking";

/// The version both ends of `networking:ping` speak.
///
/// A constant so the mod and a test cannot disagree about it, and so raising it is
/// one edit. Raising it is what makes an old client's payload refused rather than
/// misread.
pub const PING_VERSION: u32 = 1;

/// A client asking a server to answer.
///
/// The payload is what a real packet's is: values the sender chose, with nothing
/// derived and nothing implicit. `nonce` is the client's own number and `note` is
/// text a player could have typed, which is why reading it can fail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ping {
    nonce: u32,
    note: String,
    urgent: bool,
}

impl Ping {
    /// Builds a ping with no note.
    #[must_use]
    pub fn new(nonce: u32) -> Self {
        Self {
            nonce,
            note: String::new(),
            urgent: false,
        }
    }

    /// Builds a ping carrying `note`.
    #[must_use]
    pub fn noted(nonce: u32, note: &str, urgent: bool) -> Self {
        Self {
            nonce,
            note: note.to_owned(),
            urgent,
        }
    }

    /// Returns the number the sender chose.
    #[must_use]
    pub const fn nonce(&self) -> u32 {
        self.nonce
    }

    /// Returns the note the sender attached, which may be empty.
    #[must_use]
    pub fn note(&self) -> &str {
        &self.note
    }

    /// Returns whether the sender marked it urgent.
    #[must_use]
    pub const fn is_urgent(&self) -> bool {
        self.urgent
    }
}

impl Packet for Ping {
    fn contract() -> PacketContract {
        PacketContract::new(ping_id(), Direction::ToServer, PING_VERSION)
    }

    fn write(&self, writer: &mut PacketWriter) {
        writer.u32(self.nonce);
        writer.text(&self.note);
        writer.bool(self.urgent);
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            nonce: reader.u32()?,
            note: reader.text()?.to_owned(),
            urgent: reader.bool()?,
        })
    }
}

/// Returns the name of the contract this mod receives under.
///
/// # Panics
///
/// Panics if [`MOD_ID`] stops being a legal mod id, which would mean this mod can
/// no longer be packaged under the identity its manifest declares.
#[must_use]
pub fn ping_id() -> PacketId {
    PacketId::new(
        format!("{MOD_ID}:ping")
            .parse()
            .expect("a mod id and a literal spell a legal content id"),
    )
}

/// A mod that answers pings and refuses anything else addressed to it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NetworkingMod {
    answered: Vec<Ping>,
}

impl NetworkingMod {
    /// Builds the mod instance the loader drives.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            answered: Vec::new(),
        }
    }

    /// Returns the pings this mod read, in arrival order.
    #[must_use]
    pub fn answered(&self) -> &[Ping] {
        &self.answered
    }
}

#[declare_mod(id = "networking")]
impl Mod for NetworkingMod {
    /// Accepts every step. Nothing here needs setting up in a phase.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }

    /// Reads one ping, or refuses the bytes and says why.
    ///
    /// The refusal carries the wire error's own words, because a payload that did
    /// not decode is the one thing here nobody in this workspace wrote and a
    /// report that only said "bad packet" would leave nothing to act on.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when the delivery is not one [`Ping`] — a
    /// truncated body, bytes left over, another contract, another version, or a
    /// side that is not an end of this link.
    fn receive_packet(&mut self, delivery: &PacketDelivery) -> Result<(), ModError> {
        let ping = delivery
            .decode::<Ping>()
            .map_err(|refused| ModError::Refused {
                mod_id: Self::declared_id().clone(),
                state: LifecycleState::Running,
                detail: refused.to_string(),
            })?;
        self.answered.push(ping);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::network::{
        DeliveryRefused, Direction, Packet, PacketDelivery, PacketId, WireError, decode, encode,
        encode_from,
    };
    use ravel_sdk::sides::Side;
    use ravel_sdk::{Declared, LifecycleState, Mod, ModError, ModId};

    use super::{MOD_ID, NetworkingMod, PING_VERSION, Ping, ping_id};

    fn arriving(payload: Vec<u8>) -> PacketDelivery {
        PacketDelivery::new(
            ping_id(),
            Direction::ToServer,
            Side::DedicatedServer,
            payload,
        )
    }

    #[test]
    fn the_mod_is_packaged_under_the_identity_it_reports() {
        let expected: ModId = MOD_ID.parse().expect("a legal mod id");
        assert_eq!(NetworkingMod::declared_id(), &expected);
        assert_eq!(NetworkingMod::new().id(), &expected);
        assert_eq!(NetworkingMod::new(), NetworkingMod::default());
        assert!(NetworkingMod::new().answered().is_empty());
    }

    #[test]
    fn the_contract_is_owned_by_this_mod_and_travels_to_the_server() {
        let contract = Ping::contract();
        assert_eq!(contract.id(), &ping_id());
        assert_eq!(contract.id().owner(), NetworkingMod::declared_id());
        assert_eq!(contract.direction(), Direction::ToServer);
        assert_eq!(contract.version(), PING_VERSION);
    }

    #[test]
    fn every_lifecycle_step_is_accepted() {
        let mut host = NetworkingMod::new();
        for state in LifecycleState::ALL {
            host.enter(state).expect("nothing is refused");
        }
    }

    #[test]
    fn a_ping_reaches_this_mod_as_the_value_the_client_sent() {
        let sent = Ping::noted(41, "a note a player typed", true);
        let mut host = NetworkingMod::new();

        host.receive_packet(&arriving(encode(&sent)))
            .expect("its own contract");
        assert_eq!(host.answered(), std::slice::from_ref(&sent));
        assert_eq!(host.answered()[0].nonce(), 41);
        assert_eq!(host.answered()[0].note(), "a note a player typed");
        assert!(host.answered()[0].is_urgent());

        // And the value survives the codec on its own, without a delivery.
        assert_eq!(decode::<Ping>(&encode(&sent)).expect("its own bytes"), sent);
    }

    #[test]
    fn a_ping_with_an_empty_note_round_trips_too() {
        // The edge a length prefix gets wrong: an empty string is a field that is
        // there and holds nothing, not a field that is absent.
        let sent = Ping::new(0);
        assert_eq!(sent.note(), "");
        assert_eq!(decode::<Ping>(&encode(&sent)).expect("its own bytes"), sent);
    }

    #[test]
    fn only_a_client_may_send_this_contract() {
        assert!(encode_from(&Ping::new(1), Side::Client).is_ok());
        for side in Side::ALL.into_iter().filter(|side| side.is_server()) {
            let error =
                encode_from(&Ping::new(1), side).expect_err("a server does not send a ping");
            assert!(
                matches!(error, DeliveryRefused::WrongSide { .. }),
                "{error}"
            );
        }
    }

    #[test]
    fn a_truncated_payload_is_refused_and_the_mod_reads_nothing() {
        let full = encode(&Ping::noted(7, "note", false));
        let mut host = NetworkingMod::new();

        let error = host
            .receive_packet(&arriving(full[..full.len() - 1].to_vec()))
            .expect_err("the body ends early");
        assert_eq!(error.mod_id(), NetworkingMod::declared_id());
        assert_eq!(error.state(), LifecycleState::Running);
        assert!(host.answered().is_empty());

        // And the whole payload is accepted, so the refusal is about the cut.
        host.receive_packet(&arriving(full))
            .expect("the whole payload");
        assert_eq!(host.answered().len(), 1);
    }

    #[test]
    fn a_payload_with_bytes_left_over_is_refused() {
        let mut trailing = encode(&Ping::new(1));
        trailing.push(0);

        let error = NetworkingMod::new()
            .receive_packet(&arriving(trailing))
            .expect_err("a byte nothing accounts for");
        assert!(
            matches!(&error, ModError::Refused { detail, .. } if detail.contains("accounts for")),
            "{error}"
        );
    }

    #[test]
    fn a_payload_written_under_another_version_is_refused_rather_than_misread() {
        let mut other = encode(&Ping::new(1));
        let offset = 4 + ping_id().to_string().len();
        other[offset..offset + 4].copy_from_slice(&(PING_VERSION + 1).to_be_bytes());

        assert_eq!(
            arriving(other).decode::<Ping>(),
            Err(DeliveryRefused::Malformed {
                id: ping_id(),
                source: WireError::Version {
                    expected: PING_VERSION,
                    found: PING_VERSION + 1,
                },
            })
        );
    }

    #[test]
    fn a_delivery_addressed_to_another_contract_is_refused() {
        let mislabelled = PacketDelivery::new(
            PacketId::new("networking:other".parse().expect("a legal content id")),
            Direction::ToServer,
            Side::DedicatedServer,
            encode(&Ping::new(1)),
        );

        assert!(
            matches!(
                mislabelled.decode::<Ping>(),
                Err(DeliveryRefused::WrongContract { .. })
            ),
            "a delivery for another name was read as a ping"
        );
    }

    #[test]
    fn a_ping_arriving_on_the_client_is_refused_before_any_of_it_is_read() {
        let on_client = PacketDelivery::new(
            ping_id(),
            Direction::ToServer,
            Side::Client,
            encode(&Ping::new(1)),
        );
        let mut host = NetworkingMod::new();

        assert!(matches!(
            host.receive_packet(&on_client),
            Err(ModError::Refused { .. })
        ));
        assert!(host.answered().is_empty());
    }
}
