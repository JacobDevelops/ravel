use ravel_core::Side;

use crate::DeliveryRefused;
use crate::contract::{Direction, Packet, PacketId};
use crate::wire::{decode, encode};

/// Encodes `packet` for sending from `side`.
///
/// # Errors
///
/// Returns [`DeliveryRefused::WrongSide`] when `side` may not send this contract.
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

/// An addressed payload and the side it arrived on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PacketDelivery {
    /// The claimed contract, checked against the payload by [`Self::decode`].
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

    /// Reads this delivery as `P`, checking the side before decoding the payload.
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
