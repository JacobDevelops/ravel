//! The wire contract this package offers a dependent, and the outbox it queues
//! into.
//!
//! §10 of the specification is the largest gap in it. Eighteen compiled entries
//! are attributed to networking and the entry's own description names packets,
//! and no channel identifier, protocol version, payload identifier, direction or
//! wire format is recoverable from any document in the artifact. A counterpart
//! cannot derive the original's wire identity, so it cannot be wire-compatible
//! with an unmodified dependent, and this module does not pretend otherwise: the
//! identity below is this package's own.
//!
//! What is real here is the whole contract Ravel publishes for one. A display's
//! readout is written and read under a named, versioned, client-bound contract;
//! a payload that is short, long or written under another version is refused;
//! and a client is refused when it tries to send one, because only a server end
//! may send a client-bound packet. Ravel carries no packet between two ends at
//! the reviewed revision, which its own network module states, so what a
//! dependent gets today is the contract and the framing rather than delivery.

use ravel_sdk::content::{ContentId, ContentIdError};
use ravel_sdk::network::{
    DeliveryRefused, Direction, Packet, PacketContract, PacketId, PacketReader, PacketWriter,
    WireError, encode_from,
};
use ravel_sdk::{Declared, Side};

use crate::presentation::Readout;

/// The name this package's one contract is spelled as.
pub const CONTRACT: &str = "rvl_180_counterpart:readout";

/// The path the contract is named under, within this package's own namespace.
pub const CONTRACT_PATH: &str = "readout";

/// The version this package writes and reads its readouts under.
pub const CONTRACT_VERSION: u32 = 1;

/// How many encoded readouts the outbox holds before it stops holding more.
///
/// A readout is produced per tick by whatever is showing one, and nothing in
/// Ravel drains a mod's queue for it today, so the queue is what would grow
/// without end. Reaching the bound drops the newest rather than the oldest,
/// because a display that has stopped being drained is one whose backlog is
/// already stale.
pub const MAX_PENDING: usize = 256;

/// Returns the one contract this package declares.
///
/// [`Packet::contract`] cannot fail, so this is where the spelling is checked:
/// a test holds it to being legal, which is what keeps the `expect` below from
/// being the only thing standing between a mistyped constant and a panic inside
/// an encode.
///
/// # Errors
///
/// Returns [`ContentIdError`] when [`CONTRACT_PATH`] is not a legal content
/// path, which for a constant would be a packaging failure.
pub fn contract() -> Result<PacketContract, ContentIdError> {
    Ok(PacketContract::new(
        PacketId::new(ContentId::new(
            crate::Entry::declared_id().clone(),
            CONTRACT_PATH,
        )?),
        Direction::ToClient,
        CONTRACT_VERSION,
    ))
}

impl Packet for Readout {
    fn contract() -> PacketContract {
        contract().expect("a packaged contract path is a legal content path")
    }

    fn write(&self, writer: &mut PacketWriter) {
        let contained = self.contained();
        writer.bool(contained.is_some());
        writer.u32(self.capacity());
        writer.u32(self.stored());
        // Written only for the display that names what it holds, so one readout
        // has one encoding and a name appended to an energy payload is trailing
        // bytes rather than a field nothing reads.
        if let Some(name) = contained {
            writer.text(name);
        }
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        let names_contents = reader.bool()?;
        let capacity = reader.u32()?;
        let stored = reader.u32()?;
        if names_contents {
            Ok(Self::fluid(capacity, stored, reader.text()?))
        } else {
            Ok(Self::energy(capacity, stored))
        }
    }
}

/// The readouts this package has encoded and not yet handed over.
#[derive(Clone, Debug, Default)]
pub struct Outbox {
    pending: Vec<Vec<u8>>,
    dropped: usize,
}

impl Outbox {
    /// Builds an empty outbox.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Encodes `readout` for sending from `side` and queues the bytes.
    ///
    /// Returns whether the bytes were queued. `false` means the bound was
    /// reached and the readout was dropped, which is counted rather than
    /// silent.
    ///
    /// # Errors
    ///
    /// Returns [`DeliveryRefused`] when `side` may not send a client-bound
    /// packet, with none of the encoding having happened.
    pub fn queue(&mut self, readout: &Readout, side: Side) -> Result<bool, DeliveryRefused> {
        let bytes = encode_from(readout, side)?;
        if self.pending.len() >= MAX_PENDING {
            self.dropped += 1;
            return Ok(false);
        }
        self.pending.push(bytes);
        Ok(true)
    }

    /// Returns how many encoded readouts are waiting.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Returns how many readouts the bound has dropped.
    #[must_use]
    pub const fn dropped(&self) -> usize {
        self.dropped
    }

    /// Hands over everything waiting and empties the queue.
    pub fn take(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.pending)
    }
}

#[cfg(test)]
mod tests {
    use super::{CONTRACT, CONTRACT_VERSION, MAX_PENDING, Outbox, contract};
    use crate::presentation::Readout;
    use ravel_sdk::Side;
    use ravel_sdk::network::{Direction, Packet, PacketDelivery, WireError, decode, encode};

    #[test]
    fn the_contract_is_named_by_this_package_and_bound_to_one_direction_and_version() {
        let contract = Readout::contract();
        assert_eq!(contract.id().to_string(), CONTRACT);
        assert_eq!(contract.direction(), Direction::ToClient);
        assert_eq!(contract.version(), CONTRACT_VERSION);
        assert_eq!(contract.id().owner().as_str(), "rvl_180_counterpart");
    }

    #[test]
    fn the_contract_path_this_package_holds_as_a_constant_is_a_legal_one() {
        let checked = contract().expect("the constant spells a legal content path");
        assert_eq!(checked.id().to_string(), CONTRACT);
    }

    #[test]
    fn both_displays_round_trip_under_the_contract() {
        for readout in [
            Readout::energy(64_000, 12_500),
            Readout::fluid(16_000, 16_000, "water"),
            Readout::energy(0, 0),
        ] {
            let bytes = encode(&readout);
            assert_eq!(
                decode::<Readout>(&bytes).expect("what was written reads back"),
                readout
            );
        }
    }

    #[test]
    fn a_payload_that_is_short_or_long_is_refused_rather_than_read() {
        let bytes = encode(&Readout::fluid(500, 250, "lava"));
        assert!(decode::<Readout>(&bytes[..bytes.len() - 1]).is_err());

        let mut trailing = bytes;
        trailing.push(0);
        assert!(matches!(
            decode::<Readout>(&trailing),
            Err(WireError::TrailingBytes { .. })
        ));
    }

    #[test]
    fn a_name_appended_to_an_energy_payload_is_refused_rather_than_ignored() {
        let mut bytes = encode(&Readout::energy(64, 32));
        bytes.extend_from_slice(&[0, 0, 0, 5, b'w', b'a', b't', b'e', b'r']);
        assert!(
            decode::<Readout>(&bytes).is_err(),
            "one readout has to have one encoding"
        );
    }

    #[test]
    fn only_a_server_end_may_send_a_client_bound_readout() {
        let readout = Readout::energy(10, 5);
        let mut outbox = Outbox::new();
        assert!(
            outbox.queue(&readout, Side::Client).is_err(),
            "a client sending a client-bound packet is refused before anything is encoded"
        );
        assert_eq!(outbox.pending(), 0);

        for side in [Side::IntegratedServer, Side::DedicatedServer] {
            assert!(
                outbox
                    .queue(&readout, side)
                    .expect("a server end may send one")
            );
        }
        assert_eq!(outbox.pending(), 2);
    }

    #[test]
    fn a_client_bound_readout_is_refused_when_it_arrives_on_a_server() {
        let readout = Readout::fluid(100, 40, "water");
        let delivery = PacketDelivery::of(&readout, Side::Client)
            .expect("a client receives a client-bound packet");
        assert_eq!(
            delivery.decode::<Readout>().expect("the client reads it"),
            readout
        );

        assert!(
            PacketDelivery::of(&readout, Side::DedicatedServer).is_err(),
            "a server has no reason to receive a client-bound readout"
        );
    }

    #[test]
    fn the_queue_is_bounded_and_taking_it_empties_it() {
        let readout = Readout::energy(1_000, 1);
        let mut outbox = Outbox::new();
        for index in 0..MAX_PENDING * 4 {
            let queued = outbox
                .queue(&readout, Side::DedicatedServer)
                .expect("a server end may send one");
            assert_eq!(queued, index < MAX_PENDING);
        }
        assert_eq!(outbox.pending(), MAX_PENDING);
        assert_eq!(outbox.dropped(), MAX_PENDING * 3);

        let taken = outbox.take();
        assert_eq!(taken.len(), MAX_PENDING);
        assert_eq!(outbox.pending(), 0);
        assert_eq!(outbox.take().len(), 0);
    }
}
