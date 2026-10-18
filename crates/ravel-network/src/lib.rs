#![forbid(unsafe_code)]

//! Versioned packet contracts, binary framing, and side-checked delivery.
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

use ravel_core::Side;

mod contract;
mod delivery;
mod wire;

pub use contract::{Direction, Packet, PacketContract, PacketId};
pub use delivery::{PacketDelivery, encode_from};
pub use wire::{PacketReader, PacketWriter, decode, encode};

/// Why a byte string cannot be decoded as the requested packet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WireError {
    /// The buffer ended inside a field.
    Truncated {
        /// The bytes the field needed.
        needed: usize,
        /// The bytes that were left.
        available: usize,
    },
    /// A field exceeds its contract's size ceiling.
    TooLarge {
        /// The size the payload requires.
        needed: usize,
        /// The largest size the contract accepts.
        limit: usize,
    },
    /// A field's bytes are structurally readable but its value is not accepted.
    InvalidValue {
        /// The domain-specific reason the value was refused.
        message: String,
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
    /// A text field was not a legal namespaced content identifier.
    ContentId {
        /// The text the field carried.
        text: String,
    },
}

impl fmt::Display for WireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { needed, available } => write!(
                formatter,
                "the packet ends early: a field needs {needed} bytes and {available} are left"
            ),
            Self::TooLarge { needed, limit } => write!(
                formatter,
                "a packet field's size is {needed} and its limit is {limit}"
            ),
            Self::InvalidValue { message } => formatter.write_str(message),
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
            Self::ContentId { text } => {
                write!(
                    formatter,
                    "`{text}` is not a legal namespaced content identifier"
                )
            }
        }
    }
}

impl std::error::Error for WireError {}

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
                side.as_str()
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
        let encoded = encode(&original);
        assert_eq!(encoded, encode(&original));
        let decoded: Every = decode(&encoded).expect("its own bytes");
        assert_eq!(decoded, original);
    }

    #[test]
    fn the_empty_values_of_every_field_kind_survive_too() {
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
                    side.as_str()
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
                    side.as_str()
                );
                continue;
            }
            let delivery = built.expect("the client receives it");
            assert_eq!(
                delivery.decode::<Inbound>().expect("its own contract"),
                Inbound { count: 5 }
            );

            // Reject the wrong side before running the packet's decoder.
            let forced = PacketDelivery::new(
                delivery.id.clone(),
                delivery.direction,
                Side::DedicatedServer,
                delivery.payload,
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
            delivery.payload,
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
            delivery.payload,
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
