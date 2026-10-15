//! The channel and its payloads, from §9.
//!
//! One channel under this package's own namespace, version `1.0`, optional: a
//! client without the package may still connect. Thirteen payload kinds are
//! specified and **twelve** are shipped, under thirteen contracts, because the
//! opaque screen payload travels both ways and needs one contract each way. The
//! kind that is not shipped is the login-finalised payload, whose handler at the
//! reviewed revision does nothing; a registered, sent and inert packet is not
//! carried here, and `NOTES.md` says so rather than reproducing it.
//!
//! Every payload implements [`Packet`], so [`encode`] and [`decode`] own the
//! framing and each body travels behind its own contract id and version. A body
//! is therefore never read as another payload's, and a version bump is refused
//! rather than misread. Nothing here writes an envelope by hand.
//!
//! A ceiling is not a framing error. [`WireError`] is a closed set whose
//! `Truncated` arm means the buffer ended inside a field, so an over-long field
//! is refused by [`admit`] with this crate's own [`PayloadRefused`] instead of
//! being reported as a packet that ended early.
//!
//! Nothing here is authenticated beyond the host's own connection. Any client
//! may send any global command id with any argument bag, so a command name and
//! its arguments are untrusted input and are checked before anything acts on the
//! calling player's behalf. That is a requirement this counterpart adds; the
//! original does not state it.

use std::fmt;

use ravel_sdk::content::{ContentId, ContentIdError};
use ravel_sdk::network::{
    Direction, Packet, PacketContract, PacketId, PacketReader, PacketWriter, WireError, decode,
    encode,
};
use ravel_sdk::{Declared, ModId};

use crate::commands::{Argument, CallRefused, CommandKey, MAX_ARGUMENTS, check_arguments};
use crate::preferences::Preferences;
use crate::values::{ValueMap, ValueRefused};

/// The channel's declared version.
pub const CHANNEL_VERSION: &str = "1.0";

/// The contract version every payload here is declared under.
pub const PAYLOAD_VERSION: u32 = 1;

/// Returns whether a client without this package may still connect.
///
/// The channel is declared optional, so a client that does not have this
/// package installed is not turned away at the handshake.
#[must_use]
pub const fn is_optional() -> bool {
    true
}

/// The thirteen contracts carrying the twelve payload kinds this package ships.
///
/// The opaque screen payload is one kind travelling both ways, so it is two
/// contracts. The kind the specification counts and this does not ship is the
/// login-finalised payload, which is deliberately absent.
pub const PAYLOADS: [(&str, Direction); 13] = [
    ("screen_data_in", Direction::ToServer),
    ("screen_data_out", Direction::ToClient),
    ("style_choice", Direction::ToServer),
    ("manual_request", Direction::ToServer),
    ("list_request", Direction::ToServer),
    ("typed_command", Direction::ToServer),
    ("global_command", Direction::ToServer),
    ("data_request", Direction::ToServer),
    ("preferences", Direction::ToClient),
    ("screen_values", Direction::ToClient),
    ("command_result", Direction::ToClient),
    ("global_client_command", Direction::ToClient),
    ("requested_data", Direction::ToClient),
];

/// The most bytes the opaque screen payload may carry.
///
/// The wire length prefix is a fixed thirty-two-bit count rather than a
/// variable-length one, so a sender announces two thousand million bytes in
/// four. This is what a receiver will actually admit.
pub const MAX_SCREEN_BYTES: usize = 1 << 20;

/// Builds a contract this package owns, at the one version it declares.
///
/// # Errors
///
/// Returns [`ContentIdError`] when the name is not a legal content path, which
/// for one of [`PAYLOADS`] would be a packaging failure.
pub fn contract(name: &str, direction: Direction) -> Result<PacketContract, ContentIdError> {
    let id = ContentId::new(owner().clone(), name)?;
    Ok(PacketContract::new(
        PacketId::new(id),
        direction,
        PAYLOAD_VERSION,
    ))
}

fn owner() -> &'static ModId {
    crate::Entry::declared_id()
}

fn declared(name: &str, direction: Direction) -> PacketContract {
    contract(name, direction).expect("a packaged payload name is a legal content path")
}

/// Returns every contract this package declares, in [`PAYLOADS`] order.
///
/// # Errors
///
/// Returns [`ContentIdError`] for the first name that is not a legal content
/// path.
pub fn contracts() -> Result<Vec<PacketContract>, ContentIdError> {
    PAYLOADS
        .into_iter()
        .map(|(name, direction)| contract(name, direction))
        .collect()
}

/// What a payload carries beyond what its framing can refuse.
///
/// [`WireError`] is a closed set and none of its arms means "a field is longer
/// than this end admits", so the ceilings live here instead of being reported as
/// a packet that ended early.
pub trait Bounded {
    /// Checks this payload against the ceilings its own kind declares.
    ///
    /// # Errors
    ///
    /// Returns [`PayloadRefused`] for a field past a ceiling.
    fn bounds(&self) -> Result<(), PayloadRefused>;
}

/// Decodes a payload and holds it to its own ceilings.
///
/// This is the receiving path for everything in this module: the framing is the
/// SDK's, and the ceilings are this package's.
///
/// # Errors
///
/// Returns [`PayloadRefused::Wire`] for a payload naming another contract,
/// written under another version, ending early or ending with bytes to spare,
/// and [`PayloadRefused::TooLarge`] for a field past a ceiling.
pub fn admit<P: Packet + Bounded>(bytes: &[u8]) -> Result<P, PayloadRefused> {
    let payload = decode::<P>(bytes).map_err(PayloadRefused::Wire)?;
    payload.bounds()?;
    Ok(payload)
}

/// Why an arrived payload was not admitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PayloadRefused {
    /// The bytes are not this payload.
    Wire(WireError),
    /// A field is longer than this end admits.
    TooLarge {
        /// Which field.
        field: &'static str,
        /// How long it was.
        length: usize,
        /// The most it may be.
        ceiling: usize,
    },
}

impl fmt::Display for PayloadRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(error) => error.fmt(formatter),
            Self::TooLarge {
                field,
                length,
                ceiling,
            } => write!(
                formatter,
                "`{field}` carries {length} and {ceiling} is the most this end admits"
            ),
        }
    }
}

impl std::error::Error for PayloadRefused {}

fn ceiling(field: &'static str, length: usize, ceiling: usize) -> Result<(), PayloadRefused> {
    if length > ceiling {
        return Err(PayloadRefused::TooLarge {
            field,
            length,
            ceiling,
        });
    }
    Ok(())
}

/// The opaque block of bytes addressed to whatever screen a player has open.
///
/// It carries a target identifier and raw bytes and nothing interprets them. On
/// arrival it is routed to the player's currently open screen and is **dropped
/// silently** when that screen is not one this package manages, which [`route`]
/// is. The same shape travels both ways, under [`ScreenIn`] and [`ScreenOut`],
/// because a contract names one direction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScreenPayload {
    /// Which screen the bytes are for.
    pub target: String,
    /// The bytes, uninterpreted.
    pub body: Vec<u8>,
}

impl ScreenPayload {
    /// The most characters a target identifier may carry.
    pub const MAX_TARGET: usize = 128;

    fn write(&self, writer: &mut PacketWriter) {
        writer.text(&self.target);
        writer.bytes_field(&self.body);
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            target: reader.text()?.to_owned(),
            body: reader.bytes_field()?.to_vec(),
        })
    }

    fn bounds(&self) -> Result<(), PayloadRefused> {
        ceiling("target", self.target.len(), Self::MAX_TARGET)?;
        ceiling("body", self.body.len(), MAX_SCREEN_BYTES)
    }
}

/// The opaque screen payload on its way to the server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScreenIn(pub ScreenPayload);

impl Packet for ScreenIn {
    fn contract() -> PacketContract {
        declared("screen_data_in", Direction::ToServer)
    }

    fn write(&self, writer: &mut PacketWriter) {
        self.0.write(writer);
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        ScreenPayload::read(reader).map(Self)
    }
}

impl Bounded for ScreenIn {
    fn bounds(&self) -> Result<(), PayloadRefused> {
        self.0.bounds()
    }
}

/// The opaque screen payload on its way to a client.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScreenOut(pub ScreenPayload);

impl Packet for ScreenOut {
    fn contract() -> PacketContract {
        declared("screen_data_out", Direction::ToClient)
    }

    fn write(&self, writer: &mut PacketWriter) {
        self.0.write(writer);
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        ScreenPayload::read(reader).map(Self)
    }
}

impl Bounded for ScreenOut {
    fn bounds(&self) -> Result<(), PayloadRefused> {
        self.0.bounds()
    }
}

/// The value map of §8 on its way to the screen open on a block entity.
///
/// This is the server-to-client screen-data kind, which §9's census counts apart
/// from the opaque payload that travels both ways. It is the packet
/// [`crate::values::write`] produces bytes for: no names and no count, so the
/// two sides agree only because they walk the same map in the same order, and
/// the receiver fills a map it already declared.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScreenValues {
    /// Which screen the values are for.
    pub target: String,
    /// The positional write, in map order.
    pub values: Vec<u8>,
}

impl ScreenValues {
    /// Builds the push a dirty subscription sends.
    ///
    /// # Errors
    ///
    /// Returns whatever [`crate::values::write`] refuses.
    pub fn of(target: &str, map: &ValueMap) -> Result<Self, ValueRefused> {
        Ok(Self {
            target: target.to_owned(),
            values: crate::values::write(map)?,
        })
    }

    /// Fills a map of the same shape from what arrived.
    ///
    /// # Errors
    ///
    /// Returns whatever [`crate::values::read`] refuses, which for a map of a
    /// different shape is the disagreement §8 makes checkable.
    pub fn apply(&self, map: &mut ValueMap) -> Result<(), ValueRefused> {
        crate::values::read(map, &self.values)
    }
}

impl Packet for ScreenValues {
    fn contract() -> PacketContract {
        declared("screen_values", Direction::ToClient)
    }

    fn write(&self, writer: &mut PacketWriter) {
        writer.text(&self.target);
        writer.bytes_field(&self.values);
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            target: reader.text()?.to_owned(),
            values: reader.bytes_field()?.to_vec(),
        })
    }
}

impl Bounded for ScreenValues {
    fn bounds(&self) -> Result<(), PayloadRefused> {
        ceiling("target", self.target.len(), ScreenPayload::MAX_TARGET)?;
        ceiling("values", self.values.len(), MAX_SCREEN_BYTES)
    }
}

/// Routes an arrived screen payload, dropping it when the screen is not ours.
///
/// Answers the bytes when the payload reached a screen this package manages and
/// nothing at all otherwise. A payload for somebody else's screen is dropped and
/// no refusal is reported, which is what the original does and what a
/// dependent's screen relies on.
#[must_use]
pub fn route<'payload>(
    payload: &'payload ScreenPayload,
    open: Option<&str>,
    managed: &[&str],
) -> Option<&'payload [u8]> {
    let open = open?;
    if open != payload.target || !managed.contains(&open) {
        return None;
    }
    Some(&payload.body)
}

/// A client telling the server which widget style that player prefers.
///
/// This is the only client-to-server path into the preference record of §5:
/// there is none for the offsets and none for the anchor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleChoice {
    /// The style the player chose.
    pub style: String,
}

impl StyleChoice {
    /// Applies the choice, refusing a style this build does not publish.
    ///
    /// Answers whether anything changed. A name a client invented changes
    /// nothing, which is the refusal §5's style setter already states.
    pub fn apply(&self, preferences: &mut Preferences) -> bool {
        preferences.set_style(&self.style)
    }
}

impl Packet for StyleChoice {
    fn contract() -> PacketContract {
        declared("style_choice", Direction::ToServer)
    }

    fn write(&self, writer: &mut PacketWriter) {
        writer.text(&self.style);
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            style: reader.text()?.to_owned(),
        })
    }
}

impl Bounded for StyleChoice {
    fn bounds(&self) -> Result<(), PayloadRefused> {
        ceiling("style", self.style.len(), ScreenPayload::MAX_TARGET)
    }
}

/// The preference record on its way to the player it belongs to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreferencesPush {
    /// The record, in its own wire form.
    pub record: Vec<u8>,
}

impl PreferencesPush {
    /// Builds the push from a record.
    #[must_use]
    pub fn of(preferences: &Preferences) -> Self {
        Self {
            record: preferences.to_wire(),
        }
    }

    /// Reads the record back.
    ///
    /// # Errors
    ///
    /// Returns whatever [`Preferences::from_wire`] refuses.
    pub fn record(&self) -> Result<Preferences, crate::preferences::PreferencesRefused> {
        Preferences::from_wire(&self.record)
    }
}

impl Packet for PreferencesPush {
    fn contract() -> PacketContract {
        declared("preferences", Direction::ToClient)
    }

    fn write(&self, writer: &mut PacketWriter) {
        writer.bytes_field(&self.record);
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            record: reader.bytes_field()?.to_vec(),
        })
    }
}

impl Bounded for PreferencesPush {
    fn bounds(&self) -> Result<(), PayloadRefused> {
        ceiling("record", self.record.len(), MAX_SCREEN_BYTES)
    }
}

/// One argument in the form it travels in: a tag and the two fields it selects
/// between.
///
/// Reading is total on purpose. [`WireError`] has no arm for an unrecognised
/// tag, so the framing reads whatever arrived and [`TypedCommand::bag`] is where
/// a tag this package does not publish is refused, alongside the bounds the same
/// untrusted bag is held to.
#[derive(Clone, Debug, Eq, PartialEq)]
struct WireArgument {
    tag: u32,
    number: u32,
    text: String,
}

impl WireArgument {
    const INTEGER: u32 = 0;
    const TEXT: u32 = 1;
    const BOOLEAN: u32 = 2;

    fn of(argument: &Argument) -> Self {
        match argument {
            Argument::Integer(number) => Self {
                tag: Self::INTEGER,
                number: number.cast_unsigned(),
                text: String::new(),
            },
            Argument::Text(text) => Self {
                tag: Self::TEXT,
                number: 0,
                text: text.clone(),
            },
            Argument::Boolean(truth) => Self {
                tag: Self::BOOLEAN,
                number: u32::from(*truth),
                text: String::new(),
            },
        }
    }

    fn argument(&self) -> Result<Argument, CallRefused> {
        match self.tag {
            Self::INTEGER => Ok(Argument::Integer(self.number.cast_signed())),
            Self::TEXT => Ok(Argument::Text(self.text.clone())),
            Self::BOOLEAN => Ok(Argument::Boolean(self.number != 0)),
            tag => Err(CallRefused::UnknownArgument { tag }),
        }
    }

    fn write(&self, writer: &mut PacketWriter) {
        writer.u32(self.tag);
        writer.u32(self.number);
        writer.text(&self.text);
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            tag: reader.u32()?,
            number: reader.u32()?,
            text: reader.text()?.to_owned(),
        })
    }
}

/// A client calling a command, with the typed argument bag it carries.
///
/// This and [`ListRequest`] are the transport for §7. Both carry a command name
/// and a bag; a list's element codec is looked up by the command name at decode
/// time, so a client that has not declared the element type cannot read a list
/// it asked for.
///
/// The bag is typed on the wire, so all three of §7's argument types cross it.
/// The arguments are held in their wire form and are only turned into
/// [`Argument`] by [`TypedCommand::bag`], which is the one place the bag off the
/// network is checked before a handler acts on the calling player's behalf.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedCommand {
    /// The namespace of the package that owns the command.
    pub namespace: String,
    /// The command's own name.
    pub name: String,
    arguments: Vec<WireArgument>,
}

impl TypedCommand {
    /// Builds a call carrying a typed bag.
    #[must_use]
    pub fn new(namespace: &str, name: &str, arguments: &[Argument]) -> Self {
        Self {
            namespace: namespace.to_owned(),
            name: name.to_owned(),
            arguments: arguments.iter().map(WireArgument::of).collect(),
        }
    }

    /// Returns the key this call names, or nothing when it is not a legal one.
    #[must_use]
    pub fn key(&self) -> Option<CommandKey> {
        CommandKey::new(&self.namespace, &self.name)
    }

    /// Returns how many arguments the call carries.
    #[must_use]
    pub fn count(&self) -> usize {
        self.arguments.len()
    }

    /// Returns the arguments as a checked bag.
    ///
    /// # Errors
    ///
    /// Returns [`CallRefused::UnknownArgument`] for a tag this package does not
    /// publish, and whatever [`check_arguments`] refuses. The bag arrived from a
    /// client, so it is read and bounded before a handler sees it.
    pub fn bag(&self) -> Result<Vec<Argument>, CallRefused> {
        let bag = self
            .arguments
            .iter()
            .map(WireArgument::argument)
            .collect::<Result<Vec<Argument>, CallRefused>>()?;
        check_arguments(&bag)?;
        Ok(bag)
    }

    fn write_body(&self, writer: &mut PacketWriter) {
        writer.u32(u32::try_from(self.arguments.len()).unwrap_or(u32::MAX));
        for argument in &self.arguments {
            argument.write(writer);
        }
    }

    fn read_body(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        let namespace = reader.text()?.to_owned();
        let name = reader.text()?.to_owned();
        let announced = reader.u32()?;
        // Nothing is reserved for the announced count: a client that announces
        // four thousand million arguments must not make this end allocate for
        // them before a single one has arrived.
        let mut arguments = Vec::new();
        for _ in 0..announced {
            arguments.push(WireArgument::read(reader)?);
        }
        Ok(Self {
            namespace,
            name,
            arguments,
        })
    }

    fn bounded(&self) -> Result<(), PayloadRefused> {
        ceiling("arguments", self.arguments.len(), MAX_ARGUMENTS)
    }
}

impl Packet for TypedCommand {
    fn contract() -> PacketContract {
        declared("typed_command", Direction::ToServer)
    }

    fn write(&self, writer: &mut PacketWriter) {
        writer.text(&self.namespace);
        writer.text(&self.name);
        self.write_body(writer);
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        Self::read_body(reader)
    }
}

impl Bounded for TypedCommand {
    fn bounds(&self) -> Result<(), PayloadRefused> {
        self.bounded()
    }
}

/// A client asking for a list command's answer.
///
/// The same body as [`TypedCommand`] under its own contract, which is what keeps
/// the two apart on the wire rather than by hoping a caller decoded the right
/// one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListRequest(pub TypedCommand);

impl Packet for ListRequest {
    fn contract() -> PacketContract {
        declared("list_request", Direction::ToServer)
    }

    fn write(&self, writer: &mut PacketWriter) {
        self.0.write(writer);
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        TypedCommand::read(reader).map(Self)
    }
}

impl Bounded for ListRequest {
    fn bounds(&self) -> Result<(), PayloadRefused> {
        self.0.bounded()
    }
}

/// What a command answered, on its way back to the client that called it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandResult {
    /// The command that answered.
    pub name: String,
    /// The answer, uninterpreted.
    pub body: Vec<u8>,
}

impl Packet for CommandResult {
    fn contract() -> PacketContract {
        declared("command_result", Direction::ToClient)
    }

    fn write(&self, writer: &mut PacketWriter) {
        writer.text(&self.name);
        writer.bytes_field(&self.body);
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            name: reader.text()?.to_owned(),
            body: reader.bytes_field()?.to_vec(),
        })
    }
}

impl Bounded for CommandResult {
    fn bounds(&self) -> Result<(), PayloadRefused> {
        ceiling("name", self.name.len(), CommandKey::MAX_PART)?;
        ceiling("body", self.body.len(), MAX_SCREEN_BYTES)
    }
}

/// Writes a payload under its own contract, framing and all.
#[must_use]
pub fn sent<P: Packet>(payload: &P) -> Vec<u8> {
    encode(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::values::Value;

    /// §9's census, written from the specification rather than from
    /// [`PAYLOADS`]: one kind both ways, six client to server, six server to
    /// client. Nothing here is derived from the table it is checking, which is
    /// what lets it notice a kind the table does not carry.
    const BOTH_WAYS: [&str; 1] = ["screen_data"];
    const TO_SERVER: [&str; 6] = [
        "style_choice",
        "manual_request",
        "list_request",
        "typed_command",
        "global_command",
        "data_request",
    ];
    const TO_CLIENT: [&str; 6] = [
        "preferences",
        "screen_values",
        "command_result",
        "global_client_command",
        "requested_data",
        "login_finalised",
    ];

    /// The one kind of the census this package deliberately does not ship.
    const NOT_SHIPPED: &str = "login_finalised";

    #[test]
    fn the_channel_is_optional_and_declares_its_own_version() {
        assert!(
            is_optional(),
            "a client without this package may still connect"
        );
        assert_eq!(CHANNEL_VERSION, "1.0");
    }

    #[test]
    fn the_census_of_thirteen_kinds_is_shipped_as_twelve_kinds_and_thirteen_contracts() {
        assert_eq!(BOTH_WAYS.len() + TO_SERVER.len() + TO_CLIENT.len(), 13);

        let shipped: Vec<&str> = PAYLOADS.iter().map(|(name, _)| *name).collect();
        assert!(
            !shipped.contains(&NOT_SHIPPED),
            "the login-finalised payload is registered, sent and inert, and is not carried"
        );

        // Every kind the census counts is shipped, and the both-ways kind is
        // shipped as one contract each way.
        for kind in BOTH_WAYS {
            assert!(shipped.contains(&format!("{kind}_in").as_str()));
            assert!(shipped.contains(&format!("{kind}_out").as_str()));
        }
        for kind in TO_SERVER.into_iter().chain(TO_CLIENT) {
            if kind == NOT_SHIPPED {
                continue;
            }
            assert!(shipped.contains(&kind), "{kind} is a kind §9 counts");
        }

        assert_eq!(
            PAYLOADS.len(),
            BOTH_WAYS.len() * 2 + TO_SERVER.len() + TO_CLIENT.len() - 1
        );
        let to_server = PAYLOADS
            .iter()
            .filter(|(_, direction)| *direction == Direction::ToServer)
            .count();
        assert_eq!(to_server, TO_SERVER.len() + BOTH_WAYS.len());
        assert_eq!(
            PAYLOADS.len() - to_server,
            TO_CLIENT.len() - 1 + BOTH_WAYS.len()
        );
    }

    #[test]
    fn every_declared_name_is_a_legal_content_path_in_this_package_namespace() {
        let contracts = contracts().expect("every packaged name is legal");
        assert_eq!(contracts.len(), PAYLOADS.len());
        for (contract, (name, direction)) in contracts.iter().zip(PAYLOADS) {
            assert_eq!(contract.id().id().path(), name);
            assert_eq!(contract.direction(), direction);
            assert_eq!(contract.version(), PAYLOAD_VERSION);
            assert_eq!(contract.id().owner().as_str(), crate::MOD_ID);
        }
    }

    #[test]
    fn every_payload_name_is_declared_once() {
        let mut names: Vec<&str> = PAYLOADS.iter().map(|(name, _)| *name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), PAYLOADS.len());
    }

    #[test]
    fn every_shipped_payload_type_travels_under_a_contract_this_package_declares() {
        let declared: Vec<(String, Direction)> = PAYLOADS
            .iter()
            .map(|(name, direction)| ((*name).to_owned(), *direction))
            .collect();
        for contract in [
            ScreenIn::contract(),
            ScreenOut::contract(),
            ScreenValues::contract(),
            StyleChoice::contract(),
            PreferencesPush::contract(),
            TypedCommand::contract(),
            ListRequest::contract(),
            CommandResult::contract(),
        ] {
            assert!(
                declared.contains(&(contract.id().id().path().to_owned(), contract.direction())),
                "{} is not one of the declared contracts",
                contract.id()
            );
            assert_eq!(contract.version(), PAYLOAD_VERSION);
        }
    }

    #[test]
    fn the_opaque_payload_travels_both_ways_under_two_contracts() {
        assert_eq!(ScreenIn::contract().direction(), Direction::ToServer);
        assert_eq!(ScreenOut::contract().direction(), Direction::ToClient);
        assert_ne!(
            ScreenIn::contract().id().to_string(),
            ScreenOut::contract().id().to_string()
        );
    }

    #[test]
    fn every_payload_carries_its_contract_identity_so_one_is_never_read_as_another() {
        let choice = StyleChoice {
            style: "raised".to_owned(),
        };
        let bytes = sent(&choice);
        assert_eq!(admit::<StyleChoice>(&bytes), Ok(choice));

        // A one-field body would otherwise be interchangeable with any other.
        assert!(matches!(
            admit::<CommandResult>(&bytes),
            Err(PayloadRefused::Wire(WireError::Identity { .. }))
        ));
        assert!(matches!(
            admit::<ScreenIn>(&sent(&ScreenOut(ScreenPayload {
                target: "dependent_screen".to_owned(),
                body: vec![1],
            }))),
            Err(PayloadRefused::Wire(WireError::Identity { .. }))
        ));
    }

    #[test]
    fn a_payload_written_under_another_version_of_a_contract_is_refused() {
        let sent_bytes = sent(&StyleChoice {
            style: "raised".to_owned(),
        });
        let mut older = PacketWriter::default();
        older.text(&StyleChoice::contract().id().to_string());
        older.u32(PAYLOAD_VERSION + 1);
        older.text("raised");
        let older = older.finish();

        assert_ne!(sent_bytes, older);
        assert_eq!(
            admit::<StyleChoice>(&older),
            Err(PayloadRefused::Wire(WireError::Version {
                expected: PAYLOAD_VERSION,
                found: PAYLOAD_VERSION + 1
            }))
        );
    }

    #[test]
    fn the_opaque_payload_round_trips_and_is_read_whole() {
        let sent_payload = ScreenIn(ScreenPayload {
            target: "dependent_screen".to_owned(),
            body: vec![1, 2, 3],
        });
        assert_eq!(admit::<ScreenIn>(&sent(&sent_payload)), Ok(sent_payload));
    }

    #[test]
    fn an_over_large_opaque_field_is_refused_as_a_ceiling_and_not_as_a_short_packet() {
        let huge = ScreenIn(ScreenPayload {
            target: "dependent_screen".to_owned(),
            body: vec![0; MAX_SCREEN_BYTES + 1],
        });
        assert_eq!(
            admit::<ScreenIn>(&sent(&huge)),
            Err(PayloadRefused::TooLarge {
                field: "body",
                length: MAX_SCREEN_BYTES + 1,
                ceiling: MAX_SCREEN_BYTES
            }),
            "a field past a ceiling is not a packet that ended early"
        );

        let long_target = ScreenIn(ScreenPayload {
            target: "t".repeat(ScreenPayload::MAX_TARGET + 1),
            body: Vec::new(),
        });
        assert_eq!(
            admit::<ScreenIn>(&sent(&long_target)),
            Err(PayloadRefused::TooLarge {
                field: "target",
                length: ScreenPayload::MAX_TARGET + 1,
                ceiling: ScreenPayload::MAX_TARGET
            })
        );
    }

    #[test]
    fn a_buffer_that_ends_inside_a_field_is_refused_rather_than_read_short() {
        let bytes = sent(&ScreenIn(ScreenPayload {
            target: "dependent_screen".to_owned(),
            body: vec![9],
        }));
        assert!(matches!(
            admit::<ScreenIn>(&bytes[..bytes.len() - 1]),
            Err(PayloadRefused::Wire(WireError::Truncated { .. }))
        ));

        let mut trailing = bytes;
        trailing.push(0);
        assert!(matches!(
            admit::<ScreenIn>(&trailing),
            Err(PayloadRefused::Wire(WireError::TrailingBytes { .. }))
        ));
    }

    #[test]
    fn a_payload_for_a_screen_this_package_does_not_manage_is_dropped_silently() {
        let payload = ScreenPayload {
            target: "dependent_screen".to_owned(),
            body: vec![7],
        };
        assert_eq!(
            route(&payload, Some("dependent_screen"), &["dependent_screen"]),
            Some([7].as_slice())
        );
        assert_eq!(route(&payload, Some("dependent_screen"), &[]), None);
        assert_eq!(
            route(&payload, Some("somebody_else"), &["somebody_else"]),
            None
        );
        assert_eq!(route(&payload, None, &["dependent_screen"]), None);
    }

    fn sample_map() -> ValueMap {
        let mut map = ValueMap::new();
        map.declare("count", Value::Integer(3));
        map.declare("label", Value::Text("here".to_owned()));
        map.declare("mode", Value::Enumeration { index: 1, count: 3 });
        map
    }

    #[test]
    fn the_screen_value_push_carries_a_positional_write_to_a_map_of_the_same_shape() {
        let map = sample_map();
        let push =
            ScreenValues::of("dependent_screen", &map).expect("every value is inside its bound");
        let arrived =
            admit::<ScreenValues>(&sent(&push)).expect("the push round-trips under its contract");

        let mut receiving = sample_map();
        receiving.declare("count", Value::Integer(0));
        receiving.declare("label", Value::Text(String::new()));
        arrived
            .apply(&mut receiving)
            .expect("the same shape reads the same bytes");
        assert_eq!(receiving, map);
        assert_eq!(arrived.target, "dependent_screen");
    }

    #[test]
    fn a_screen_value_push_read_into_a_map_of_another_shape_is_refused() {
        let push = ScreenValues::of("dependent_screen", &sample_map())
            .expect("every value is inside its bound");
        let mut shorter = ValueMap::new();
        shorter.declare("count", Value::Integer(0));
        assert_eq!(push.apply(&mut shorter), Err(ValueRefused::Trailing));
    }

    #[test]
    fn a_style_choice_round_trips_and_only_a_published_style_changes_anything() {
        let chosen = StyleChoice {
            style: "raised".to_owned(),
        };
        assert_eq!(admit::<StyleChoice>(&sent(&chosen)), Ok(chosen.clone()));

        let mut preferences = Preferences::new();
        assert!(chosen.apply(&mut preferences));
        assert_eq!(preferences.style(), "raised");

        let invented = StyleChoice {
            style: "whatever_a_client_typed".to_owned(),
        };
        assert!(!invented.apply(&mut preferences));
        assert_eq!(preferences.style(), "raised");
    }

    #[test]
    fn the_style_choice_is_the_only_client_to_server_path_into_the_record() {
        let client_to_server: Vec<&str> = PAYLOADS
            .iter()
            .filter(|(_, direction)| *direction == Direction::ToServer)
            .map(|(name, _)| *name)
            .collect();
        assert!(client_to_server.contains(&"style_choice"));
        assert!(
            !client_to_server.iter().any(|name| name.contains("offset")),
            "there is no client-to-server path for the offsets"
        );
    }

    #[test]
    fn the_preference_push_carries_the_record_and_reads_it_back() {
        let mut preferences = Preferences::new();
        preferences.set_offsets(3, 4);
        let push = PreferencesPush::of(&preferences);
        let travelled = admit::<PreferencesPush>(&sent(&push)).expect("the payload round-trips");

        let record = travelled.record().expect("the record reads back");
        assert_eq!(record.horizontal(), 3);
        assert_eq!(record.vertical(), 4);
        assert!(!record.is_dirty());
    }

    #[test]
    fn a_typed_command_carries_all_three_argument_types_across_the_wire() {
        let bag = [
            Argument::Integer(-7),
            Argument::Text("first".to_owned()),
            Argument::Boolean(true),
        ];
        let call = TypedCommand::new("dependent", "act", &bag);
        let arrived = admit::<TypedCommand>(&sent(&call)).expect("the payload round-trips");

        assert_eq!(arrived, call);
        assert_eq!(
            arrived.bag().expect("a bounded bag"),
            bag,
            "an integer and a boolean arrive as themselves rather than as text"
        );
        assert!(arrived.key().is_some());
        assert_eq!(arrived.count(), 3);
    }

    #[test]
    fn a_key_a_client_invented_is_refused_before_anything_is_looked_up() {
        let invented = TypedCommand::new("dependent/../elsewhere", "act", &[]);
        assert_eq!(invented.key(), None);
    }

    #[test]
    fn an_argument_tag_this_package_does_not_publish_is_refused_at_the_bag() {
        let mut writer = PacketWriter::default();
        writer.text(&TypedCommand::contract().id().to_string());
        writer.u32(PAYLOAD_VERSION);
        writer.text("dependent");
        writer.text("act");
        writer.u32(1);
        writer.u32(9);
        writer.u32(0);
        writer.text("");
        let bytes = writer.finish();

        let arrived = admit::<TypedCommand>(&bytes).expect("the framing is well formed");
        assert_eq!(
            arrived.bag(),
            Err(CallRefused::UnknownArgument { tag: 9 }),
            "a tag off the network is refused rather than guessed at"
        );
    }

    #[test]
    fn an_argument_count_larger_than_a_call_may_carry_is_refused_as_a_ceiling() {
        let bag = vec![Argument::Boolean(true); MAX_ARGUMENTS + 1];
        let call = TypedCommand::new("dependent", "act", &bag);
        assert_eq!(
            admit::<TypedCommand>(&sent(&call)),
            Err(PayloadRefused::TooLarge {
                field: "arguments",
                length: MAX_ARGUMENTS + 1,
                ceiling: MAX_ARGUMENTS
            })
        );
        assert_eq!(
            call.bag(),
            Err(CallRefused::TooManyArguments {
                count: MAX_ARGUMENTS + 1
            })
        );
    }

    #[test]
    fn an_announced_count_a_buffer_does_not_carry_is_refused_before_it_is_believed() {
        let mut writer = PacketWriter::default();
        writer.text(&TypedCommand::contract().id().to_string());
        writer.u32(PAYLOAD_VERSION);
        writer.text("dependent");
        writer.text("act");
        writer.u32(u32::MAX);
        let bytes = writer.finish();

        assert!(matches!(
            admit::<TypedCommand>(&bytes),
            Err(PayloadRefused::Wire(WireError::Truncated { .. }))
        ));
    }

    #[test]
    fn a_command_result_round_trips_and_bounds_its_answer() {
        let answered = CommandResult {
            name: "act".to_owned(),
            body: vec![1, 2],
        };
        assert_eq!(admit::<CommandResult>(&sent(&answered)), Ok(answered));

        let huge = CommandResult {
            name: "act".to_owned(),
            body: vec![0; MAX_SCREEN_BYTES + 1],
        };
        assert_eq!(
            admit::<CommandResult>(&sent(&huge)),
            Err(PayloadRefused::TooLarge {
                field: "body",
                length: MAX_SCREEN_BYTES + 1,
                ceiling: MAX_SCREEN_BYTES
            })
        );
    }

    #[test]
    fn the_list_request_and_the_typed_command_are_two_contracts_of_the_same_transport() {
        let call = TypedCommand::new("dependent", "listing", &[Argument::Integer(1)]);
        let listing = ListRequest(call.clone());

        assert_eq!(
            TypedCommand::contract().direction(),
            ListRequest::contract().direction()
        );
        assert_ne!(
            TypedCommand::contract().id().to_string(),
            ListRequest::contract().id().to_string()
        );

        assert_eq!(admit::<ListRequest>(&sent(&listing)), Ok(listing));
        assert!(
            matches!(
                admit::<TypedCommand>(&sent(&ListRequest(call))),
                Err(PayloadRefused::Wire(WireError::Identity { .. }))
            ),
            "the two are told apart by their framing rather than by their body"
        );
    }
}
