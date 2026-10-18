//! The six payloads of §13, and the validation that decides whether one is
//! acted on.
//!
//! Five travel to the server and one to the client. The framing and the side
//! rule are Ravel's, so what is written here is the field layout of each payload
//! and the check each one is put through, which is the security-relevant part
//! §13 calls out.
//!
//! Three of §13's findings are acted on rather than copied:
//!
//! - the wholesale payload's test is a contents test and not only a kind test,
//!   because a client may otherwise send a match item whose record is arbitrary;
//! - the readout request is bounded by distance as well as by chunk load, so a
//!   player cannot read any loaded position in the world;
//! - the components payload copies one named record rather than the whole
//!   component map of an attacker-chosen stack, and decodes that record before
//!   copying it: knowing a key is present is not knowing that its body is one
//!   this entry writes.
//!
//! The typed map a readout answer travels in is the shared library's and §13
//! says its encoding is that specification's, so the values are written by that
//! library rather than by a second encoding here. What this entry frames around
//! them is the shape — the ordered names and their types — because the provider
//! declares its own values and a client cannot know the shape in advance.

use std::collections::BTreeMap;

use ravel_sdk::Declared;
use ravel_sdk::network::{
    Direction, Packet, PacketContract, PacketId, PacketReader, PacketWriter, WireError,
};

use crate::data::{MAX_COMPONENTS, MAX_NAME, Placement, Record, Stack};
use crate::matching::MatchRule;
use crate::shapes::Pattern;
use crate::storage::Carrier;
use crate::values::{Reading, Readings, shared_from_tag};

/// The version every contract here is declared at.
pub const VERSION: u32 = 1;

/// How many stacks the recipe-browser payload carries.
///
/// Ten slot views, which is what §17's transfer collects: one result and the
/// nine positions of a plain grid.
pub const BROWSER_SLOTS: usize = 10;

/// How many named values a typed payload carries.
pub const MAX_VALUES: usize = 64;

/// How far from a readout a player may be and still be answered, in blocks.
///
/// A bound §13 asks for and the original does not have. The figure is a declared
/// number to raise, not a rule: it is comfortably past any distance a screen is
/// legible from, and the squared form is what the test uses so no square root is
/// taken on a payload a client controls.
pub const READ_RANGE: i64 = 64;

fn contract(name: &str, direction: Direction) -> PacketContract {
    let id = ravel_sdk::content::ContentId::new(crate::Entry::declared_id().clone(), name)
        .expect("a constant this package spells is a legal content path");
    PacketContract::new(PacketId::new(id), direction, VERSION)
}

fn write_stack(writer: &mut PacketWriter, stack: &Stack) {
    writer.text(&stack.identity.to_string());
    writer.u32(u32::from(stack.count));
    writer.u32(stack.damage.cast_unsigned());
    writer.bool(stack.carrier_ready);
    writer.bool(stack.cyclable);
    writer.bool(stack.upgradable);
    writer.u32(stack.infusion);
    let components =
        u32::try_from(stack.components.len()).expect("a packet list shorter than 2^32");
    writer.u32(components);
    for (name, value) in &stack.components {
        writer.text(name);
        writer.text(value);
    }
}

fn read_stack(reader: &mut PacketReader<'_>) -> Result<Stack, WireError> {
    let identity = bounded_text(reader)?;
    let identity = identity
        .parse()
        .map_err(|_| WireError::ContentId { text: identity })?;
    let count = reader.u32()?;
    let count = u8::try_from(count).map_err(|_| WireError::TooLarge {
        needed: usize::try_from(count).unwrap_or(usize::MAX),
        limit: usize::from(u8::MAX),
    })?;
    let damage = reader.u32()?.cast_signed();
    let carrier_ready = reader.bool()?;
    let cyclable = reader.bool()?;
    let upgradable = reader.bool()?;
    let infusion = reader.u32()?;

    let declared = bounded_count(reader, MAX_COMPONENTS)?;
    let mut components = BTreeMap::new();
    for _ in 0..declared {
        let name = bounded_text(reader)?;
        let value = bounded_text(reader)?;
        components.insert(name, value);
    }

    Ok(Stack {
        identity,
        count,
        damage,
        components,
        carrier_ready,
        cyclable,
        upgradable,
        infusion,
    })
}

/// Reads a text field and refuses one longer than a name may be.
///
/// The length prefix is a client's, so the field is bounded before what it
/// describes is kept rather than after.
fn bounded_text(reader: &mut PacketReader<'_>) -> Result<String, WireError> {
    let text = reader.text()?;
    if text.len() > MAX_NAME {
        return Err(WireError::TooLarge {
            needed: text.len(),
            limit: MAX_NAME,
        });
    }
    Ok(text.to_owned())
}

fn bounded_count(reader: &mut PacketReader<'_>, limit: usize) -> Result<usize, WireError> {
    let declared = usize::try_from(reader.u32()?).expect("packet lengths fit the address space");
    if declared > limit {
        return Err(WireError::TooLarge {
            needed: declared,
            limit,
        });
    }
    Ok(declared)
}

fn write_position(writer: &mut PacketWriter, at: (i32, i32, i32)) {
    writer.u32(at.0.cast_unsigned());
    writer.u32(at.1.cast_unsigned());
    writer.u32(at.2.cast_unsigned());
}

fn read_position(reader: &mut PacketReader<'_>) -> Result<(i32, i32, i32), WireError> {
    Ok((
        reader.u32()?.cast_signed(),
        reader.u32()?.cast_signed(),
        reader.u32()?.cast_signed(),
    ))
}

/// The components of the held item, sent to the server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeldComponents {
    /// The stack whose components are offered.
    pub offered: Stack,
}

impl Packet for HeldComponents {
    fn contract() -> PacketContract {
        contract("held_components", Direction::ToServer)
    }

    fn write(&self, writer: &mut PacketWriter) {
        write_stack(writer, &self.offered);
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            offered: read_stack(reader)?,
        })
    }
}

/// Typed arguments for the held card, sent to the server.
///
/// §13 records that the original validates the held item and then does nothing,
/// because the payload's entire body is commented out at this revision: it is
/// registered, accepted and discarded. That is reproduced deliberately, and
/// [`Applied::Discarded`] is what saying so looks like.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardArguments {
    /// The named values offered.
    pub named: Vec<(String, String)>,
}

impl Packet for CardArguments {
    fn contract() -> PacketContract {
        contract("card_arguments", Direction::ToServer)
    }

    fn write(&self, writer: &mut PacketWriter) {
        writer.u32(u32::try_from(self.named.len()).expect("a packet list shorter than 2^32"));
        for (name, value) in &self.named {
            writer.text(name);
            writer.text(value);
        }
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        let declared = bounded_count(reader, MAX_VALUES)?;
        let mut named = Vec::with_capacity(declared);
        for _ in 0..declared {
            named.push((bounded_text(reader)?, bounded_text(reader)?));
        }
        Ok(Self { named })
    }
}

/// A pattern taken from a recipe browser, sent to the server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserPattern {
    /// The ten slot views the browser collected.
    pub slots: Vec<Option<Stack>>,
}

impl Packet for BrowserPattern {
    fn contract() -> PacketContract {
        contract("browser_pattern", Direction::ToServer)
    }

    fn write(&self, writer: &mut PacketWriter) {
        writer.u32(u32::try_from(self.slots.len()).expect("a packet list shorter than 2^32"));
        for slot in &self.slots {
            writer.bool(slot.is_some());
            if let Some(stack) = slot {
                write_stack(writer, stack);
            }
        }
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        let declared = bounded_count(reader, BROWSER_SLOTS)?;
        let mut slots = Vec::with_capacity(declared);
        for _ in 0..declared {
            slots.push(if reader.bool()? {
                Some(read_stack(reader)?)
            } else {
                None
            });
        }
        Ok(Self { slots })
    }
}

/// A request for a readout's values, sent to the server.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadoutRequest {
    /// The position being asked about.
    pub at: (i32, i32, i32),
}

impl Packet for ReadoutRequest {
    fn contract() -> PacketContract {
        contract("readout_request", Direction::ToServer)
    }

    fn write(&self, writer: &mut PacketWriter) {
        write_position(writer, self.at);
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            at: read_position(reader)?,
        })
    }
}

/// The held item, wholesale, sent to the server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeldItem {
    /// The stack the client wants the held item replaced with.
    pub sent: Stack,
}

impl Packet for HeldItem {
    fn contract() -> PacketContract {
        contract("held_item", Direction::ToServer)
    }

    fn write(&self, writer: &mut PacketWriter) {
        write_stack(writer, &self.sent);
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            sent: read_stack(reader)?,
        })
    }
}

/// A readout's values, sent to the client that asked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadoutAnswer {
    /// Which position was asked about.
    pub at: (i32, i32, i32),
    values: Vec<(String, Reading)>,
    shape: Vec<(String, u8)>,
    body: Vec<u8>,
}

impl ReadoutAnswer {
    /// Builds an answer, refusing oversized names, sets, or text before sending.
    ///
    /// Duplicate names keep their first position and last value; exposed amounts
    /// use the shared map's energy clamp, exactly as they do after decoding.
    pub fn new(at: (i32, i32, i32), values: &[(String, Reading)]) -> Result<Self, WireError> {
        if values.len() > MAX_VALUES {
            return Err(WireError::TooLarge {
                needed: values.len(),
                limit: MAX_VALUES,
            });
        }
        let mut readings = Readings::new();
        for (name, reading) in values {
            if name.len() > MAX_NAME {
                return Err(WireError::TooLarge {
                    needed: name.len(),
                    limit: MAX_NAME,
                });
            }
            readings.declare(name, reading.clone());
        }
        let map = readings.to_shared();
        let body =
            rvl_120_counterpart::values::write(&map).map_err(|error| WireError::InvalidValue {
                message: error.to_string(),
            })?;
        let readings = Readings::from_shared(&map);
        let values = readings
            .names()
            .into_iter()
            .filter_map(|name| {
                readings
                    .value(name)
                    .map(|reading| (name.to_owned(), reading.clone()))
            })
            .collect();
        Ok(Self {
            at,
            values,
            shape: map.shape(),
            body,
        })
    }

    /// Returns the reported values without permitting unvalidated mutation.
    #[must_use]
    pub fn values(&self) -> &[(String, Reading)] {
        &self.values
    }
}

impl Packet for ReadoutAnswer {
    fn contract() -> PacketContract {
        contract("readout_answer", Direction::ToClient)
    }

    fn write(&self, writer: &mut PacketWriter) {
        write_position(writer, self.at);
        writer.u32(u32::try_from(self.shape.len()).expect("a packet list shorter than 2^32"));
        for (name, tag) in &self.shape {
            writer.text(name);
            writer.u32(u32::from(*tag));
        }
        writer.bytes_field(&self.body);
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        let at = read_position(reader)?;
        let declared = bounded_count(reader, MAX_VALUES)?;

        // The shape is read first and the body is read against it, because the
        // shared library's own form is positional: it says what the values are,
        // and nothing in it says what they are called.
        let mut map = rvl_120_counterpart::values::ValueMap::new();
        for _ in 0..declared {
            let name = bounded_text(reader)?;
            let tag = reader.u32()?;
            let template = u8::try_from(tag)
                .ok()
                .and_then(shared_from_tag)
                .ok_or_else(|| WireError::InvalidValue {
                    message: format!("unknown readout value tag {tag}"),
                })?;
            map.declare(&name, template);
        }
        let body = reader.bytes_field()?;
        rvl_120_counterpart::values::read(&mut map, body).map_err(|error| {
            WireError::InvalidValue {
                message: error.to_string(),
            }
        })?;

        let readings = Readings::from_shared(&map);
        let values = readings
            .names()
            .into_iter()
            .filter_map(|name| {
                readings
                    .value(name)
                    .map(|reading| (name.to_owned(), reading.clone()))
            })
            .collect();
        Ok(Self {
            at,
            values,
            shape: map.shape(),
            body: body.to_vec(),
        })
    }
}

/// Every contract this entry declares, in the order §13's table lists them.
#[must_use]
pub fn contracts() -> Vec<PacketContract> {
    vec![
        HeldComponents::contract(),
        CardArguments::contract(),
        BrowserPattern::contract(),
        ReadoutRequest::contract(),
        HeldItem::contract(),
        ReadoutAnswer::contract(),
    ]
}

/// Which of this entry's items a payload may act on.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Held {
    /// The pattern card of §9.
    PatternCard,
    /// The match item of §8.
    MatchItem,
    /// The handheld carrier of §10.
    Carrier,
    /// Anything else, including nothing.
    #[default]
    Other,
}

/// What a payload's validation decided.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Applied {
    /// The payload was acted on.
    Accepted,
    /// The payload was accepted and deliberately does nothing.
    Discarded,
    /// The payload was refused, with the reason.
    Refused(Refusal),
}

/// Why a payload was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// The held item is not one this payload acts on.
    WrongHeldItem,
    /// The sent item is not one this payload may install.
    WrongSentItem,
    /// The record inside the sent item is not one this entry writes.
    BadRecord,
    /// The record named is not the one this payload copies.
    WrongRecord,
    /// The open menu is not the one this payload writes into.
    WrongMenu,
    /// The position's chunk is not loaded.
    ChunkAbsent,
    /// The position is further from the player than a readout may be read.
    OutOfRange,
}

/// Applies a components payload to the held item.
///
/// The held item must be the pattern card, and exactly one record is copied
/// across rather than the whole component map: §13's third finding is that the
/// original copies an attacker-chosen stack's every component onto a held item.
#[must_use]
pub fn apply_components(held: Held, payload: &HeldComponents, record: Record) -> Applied {
    if held != Held::PatternCard {
        return Applied::Refused(Refusal::WrongHeldItem);
    }
    if record != Record::PatternContents {
        return Applied::Refused(Refusal::WrongRecord);
    }
    let Some(written) = payload.offered.components.get(record.as_str()) else {
        return Applied::Refused(Refusal::BadRecord);
    };
    if decodes(record, written) {
        Applied::Accepted
    } else {
        Applied::Refused(Refusal::BadRecord)
    }
}

/// Answers whether `written` is a body this entry would write under `record`.
///
/// Knowing that a key is present is not knowing that its body is one this entry
/// writes, which is the difference between a payload that names a record and one
/// that carries it. Each of §4's five records is decoded by the rules that record
/// already has, so a client cannot hand any of them an arbitrary body under the
/// right name.
#[must_use]
pub fn decodes(record: Record, written: &str) -> bool {
    let Ok(document) = written.parse::<ravel_sdk::JsonValue>() else {
        return false;
    };
    match record {
        Record::PatternContents => Pattern::from_saved(&document).is_ok(),
        Record::MatchRule => MatchRule::from_saved(&document).is_ok(),
        Record::InfusionProgress => document
            .as_u64()
            .is_some_and(|steps| u32::try_from(steps).is_ok()),
        Record::CarrierContents => Carrier::from_saved(&document)
            .is_ok_and(|carrier| carrier.slots().iter().flatten().all(Carrier::accepts)),
        Record::SelectedPosition => Placement::from_saved(&document).is_ok(),
    }
}

/// Applies the typed-arguments payload, which validates and then does nothing.
#[must_use]
pub fn apply_card_arguments(held: Held, _payload: &CardArguments) -> Applied {
    if held == Held::PatternCard {
        Applied::Discarded
    } else {
        Applied::Refused(Refusal::WrongHeldItem)
    }
}

/// Applies a recipe-browser payload to the open menu.
///
/// Both conditions §13 states have to hold: the held item is the pattern card
/// and the open menu is that card's menu.
#[must_use]
pub fn apply_browser_pattern(
    held: Held,
    card_menu_open: bool,
    _payload: &BrowserPattern,
) -> Applied {
    if held != Held::PatternCard {
        return Applied::Refused(Refusal::WrongHeldItem);
    }
    if !card_menu_open {
        return Applied::Refused(Refusal::WrongMenu);
    }
    Applied::Accepted
}

/// Answers a readout request.
///
/// The chunk-load check is the original's. The distance check is not: §13's
/// second finding is that the original checks chunk load but neither distance
/// nor line of sight, so any player may read any loaded position. The squared
/// distance is compared, so nothing here takes a root of a client's numbers, and
/// the arithmetic is done at 64 bits so a hostile pair of coordinates cannot
/// wrap into range.
#[must_use]
pub fn answer_readout(
    request: &ReadoutRequest,
    player_at: (i32, i32, i32),
    chunk_loaded: bool,
) -> Applied {
    if !chunk_loaded {
        return Applied::Refused(Refusal::ChunkAbsent);
    }
    let span = |a: i32, b: i32| i64::from(a) - i64::from(b);
    let (x, y, z) = (
        span(request.at.0, player_at.0),
        span(request.at.1, player_at.1),
        span(request.at.2, player_at.2),
    );
    let squared = x
        .saturating_mul(x)
        .saturating_add(y.saturating_mul(y))
        .saturating_add(z.saturating_mul(z));
    if squared > READ_RANGE * READ_RANGE {
        return Applied::Refused(Refusal::OutOfRange);
    }
    Applied::Accepted
}

/// Applies a wholesale replacement of the held item.
///
/// Both the held item and the sent item must be either the match item or the
/// carrier, which is the original's own guard. The contents test is the addition
/// §13's first finding asks for: a client may send a match item whose record is
/// arbitrary, so the record is decoded and held to its own rules before the
/// replacement happens. That is also what closes the route to §10's latent
/// failure, because a carrier record naming an unsupported item is refused here
/// rather than at the next selection change.
#[must_use]
pub fn apply_held_item(held: Held, sent_kind: Held, payload: &HeldItem) -> Applied {
    let permitted = |kind: Held| matches!(kind, Held::MatchItem | Held::Carrier);
    if !permitted(held) {
        return Applied::Refused(Refusal::WrongHeldItem);
    }
    if !permitted(sent_kind) || sent_kind != held {
        return Applied::Refused(Refusal::WrongSentItem);
    }
    let record = match sent_kind {
        Held::MatchItem => Record::MatchRule,
        Held::Carrier => Record::CarrierContents,
        Held::PatternCard | Held::Other => return Applied::Refused(Refusal::WrongSentItem),
    };
    let Some(written) = payload.sent.components.get(record.as_str()) else {
        return Applied::Refused(Refusal::BadRecord);
    };
    if decodes(record, written) {
        Applied::Accepted
    } else {
        Applied::Refused(Refusal::BadRecord)
    }
}

/// Builds the answer a readout sends back to the one client that asked.
pub fn readout_answer(
    at: (i32, i32, i32),
    readings: &Readings,
) -> Result<ReadoutAnswer, WireError> {
    ReadoutAnswer::new(
        at,
        &readings
            .names()
            .into_iter()
            .filter_map(|name| {
                readings
                    .value(name)
                    .map(|reading| (name.to_owned(), reading.clone()))
            })
            .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravel_sdk::network::{decode, encode};

    fn stack(identity: &str) -> Stack {
        Stack::of(identity).expect("a legal identity")
    }

    #[test]
    fn a_wire_stack_refuses_a_count_larger_than_a_byte() {
        for count in [u32::from(u8::MAX) + 1, u32::MAX] {
            let mut writer = PacketWriter::default();
            writer.text("counterpart:beam");
            writer.u32(count);
            let bytes = writer.finish();
            assert_eq!(
                read_stack(&mut PacketReader::new(&bytes)),
                Err(WireError::TooLarge {
                    needed: usize::try_from(count).expect("a u32 fits the host"),
                    limit: usize::from(u8::MAX),
                })
            );
        }
    }

    #[test]
    fn a_wire_stack_refuses_an_unqualified_or_traversing_identity() {
        for bad in ["beam", "counterpart:../beam"] {
            let mut writer = PacketWriter::default();
            writer.text(bad);
            let bytes = writer.finish();
            assert!(matches!(
                read_stack(&mut PacketReader::new(&bytes)),
                Err(WireError::ContentId { .. })
            ));
        }
    }

    fn carrier_record(supported: bool) -> String {
        let mut carrier = Carrier::new();
        if supported {
            let mut device = stack("counterpart:device");
            device.carrier_ready = true;
            carrier.insert(0, device).expect("the carrier takes it");
            return carrier.to_saved().to_string();
        }
        ravel_sdk::json!({
            "slots": [stack("counterpart:plain").to_saved(), null, null, null, null, null],
            "selected": 0,
        })
        .to_string()
    }

    #[test]
    fn six_contracts_are_declared_and_five_of_them_travel_to_the_server() {
        let contracts = contracts();
        assert_eq!(contracts.len(), 6);
        assert_eq!(
            contracts
                .iter()
                .filter(|contract| contract.direction() == Direction::ToServer)
                .count(),
            5
        );
        assert_eq!(
            contracts
                .iter()
                .filter(|contract| contract.direction() == Direction::ToClient)
                .count(),
            1
        );
        for contract in &contracts {
            assert_eq!(contract.version(), VERSION);
            assert_eq!(contract.id().owner().as_str(), crate::MOD_ID);
        }
    }

    #[test]
    fn every_contract_is_named_once() {
        let mut names: Vec<String> = contracts()
            .iter()
            .map(|contract| contract.id().to_string())
            .collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 6);
    }

    #[test]
    fn every_payload_round_trips_through_its_own_framing() {
        let mut offered = stack("counterpart:card");
        offered
            .components
            .insert("pattern".to_owned(), "written".to_owned());

        let components = HeldComponents { offered };
        assert_eq!(
            decode::<HeldComponents>(&encode(&components)).expect("it reads back"),
            components
        );

        let arguments = CardArguments {
            named: vec![("first".to_owned(), "value".to_owned())],
        };
        assert_eq!(
            decode::<CardArguments>(&encode(&arguments)).expect("it reads back"),
            arguments
        );

        let browser = BrowserPattern {
            slots: vec![Some(stack("counterpart:beam")), None],
        };
        assert_eq!(
            decode::<BrowserPattern>(&encode(&browser)).expect("it reads back"),
            browser
        );

        let request = ReadoutRequest { at: (-3, 70, 12) };
        assert_eq!(
            decode::<ReadoutRequest>(&encode(&request)).expect("it reads back"),
            request
        );

        let held = HeldItem {
            sent: stack("counterpart:carrier"),
        };
        assert_eq!(
            decode::<HeldItem>(&encode(&held)).expect("it reads back"),
            held
        );

        let answer = ReadoutAnswer::new(
            (1, 2, 3),
            &[
                ("stored".to_owned(), Reading::Amount(4_096)),
                ("label".to_owned(), Reading::Text("first".to_owned())),
                ("running".to_owned(), Reading::Flag(true)),
            ],
        )
        .expect("bounded values");
        assert_eq!(
            decode::<ReadoutAnswer>(&encode(&answer)).expect("it reads back"),
            answer
        );
    }

    #[test]
    fn an_answer_travels_in_the_shared_librarys_map_and_takes_its_energy_clamp() {
        let answer = ReadoutAnswer::new(
            (0, 0, 0),
            &[
                ("stored".to_owned(), Reading::Amount(1)),
                ("label".to_owned(), Reading::Text("first".to_owned())),
                ("stored".to_owned(), Reading::Amount(i64::MAX)),
            ],
        )
        .expect("bounded values");
        let read = decode::<ReadoutAnswer>(&encode(&answer)).expect("it reads back");
        assert_eq!(
            read, answer,
            "sender and receiver expose the same canonical answer"
        );
        assert_eq!(
            read.values(),
            [
                (
                    "stored".to_owned(),
                    Reading::Amount(i64::from(rvl_120_counterpart::clamp_energy(i64::MAX)))
                ),
                ("label".to_owned(), Reading::Text("first".to_owned())),
            ],
            "the map has no 64-bit member, so an amount crosses through the clamp the \
             shared library publishes for exactly that"
        );
    }

    #[test]
    fn an_answer_refuses_oversized_text_at_the_sender() {
        let limit = rvl_120_counterpart::values::Value::MAX_TEXT;
        let bounded = ReadoutAnswer::new(
            (0, 0, 0),
            &[("label".to_owned(), Reading::Text("x".repeat(limit)))],
        )
        .expect("text at the ceiling is valid");
        assert_eq!(
            decode::<ReadoutAnswer>(&encode(&bounded)).expect("the peer admits the ceiling"),
            bounded
        );
        assert!(matches!(
            ReadoutAnswer::new(
                (0, 0, 0),
                &[("label".to_owned(), Reading::Text("x".repeat(limit + 1)))],
            ),
            Err(WireError::InvalidValue { message }) if message.contains("text")
        ));
    }

    #[test]
    fn readout_value_failures_preserve_the_domain_reason() {
        let mut writer = PacketWriter::default();
        let contract = ReadoutAnswer::contract();
        writer.text(&contract.id().to_string());
        writer.u32(contract.version());
        write_position(&mut writer, (0, 0, 0));
        writer.u32(1);
        writer.text("running");
        writer.u32(2);
        writer.bytes_field(&[2]);
        assert!(matches!(
            decode::<ReadoutAnswer>(&writer.finish()),
            Err(WireError::InvalidValue { message }) if message == "2 is neither zero nor one"
        ));
    }

    #[test]
    fn an_answer_bounds_its_shape_before_sending() {
        let values = vec![("x".to_owned(), Reading::Flag(false)); MAX_VALUES + 1];
        assert_eq!(
            ReadoutAnswer::new((0, 0, 0), &values),
            Err(WireError::TooLarge {
                needed: MAX_VALUES + 1,
                limit: MAX_VALUES
            })
        );
        assert_eq!(
            ReadoutAnswer::new(
                (0, 0, 0),
                &[("x".repeat(MAX_NAME + 1), Reading::Flag(false))],
            ),
            Err(WireError::TooLarge {
                needed: MAX_NAME + 1,
                limit: MAX_NAME
            })
        );
    }

    #[test]
    fn an_answer_claiming_a_shape_no_readout_produces_is_refused() {
        let mut writer = PacketWriter::default();
        let contract = ReadoutAnswer::contract();
        writer.text(&contract.id().to_string());
        writer.u32(contract.version());
        write_position(&mut writer, (0, 0, 0));
        writer.u32(1);
        writer.text("stored");
        // A tag the shared library has and this entry never writes.
        writer.u32(u32::from(crate::values::shared_tag(
            &rvl_120_counterpart::values::Value::Float(0.0),
        )));
        writer.bytes_field(&[0, 0, 0, 0]);

        assert!(matches!(
            decode::<ReadoutAnswer>(&writer.finish()),
            Err(WireError::InvalidValue { message }) if message.contains("tag")
        ));
    }

    #[test]
    fn a_payload_declaring_more_entries_than_it_may_is_refused_before_allocating() {
        let mut bytes = Vec::new();
        let crowded = CardArguments {
            named: (0..=MAX_VALUES)
                .map(|index| (format!("name{index}"), "value".to_owned()))
                .collect(),
        };
        bytes.extend_from_slice(&encode(&crowded));
        assert_eq!(
            decode::<CardArguments>(&bytes),
            Err(WireError::TooLarge {
                needed: MAX_VALUES + 1,
                limit: MAX_VALUES
            })
        );

        let overfull = BrowserPattern {
            slots: vec![None; BROWSER_SLOTS + 1],
        };
        assert_eq!(
            decode::<BrowserPattern>(&encode(&overfull)),
            Err(WireError::TooLarge {
                needed: BROWSER_SLOTS + 1,
                limit: BROWSER_SLOTS
            })
        );
    }

    fn pattern_record() -> String {
        Pattern::new().to_saved().to_string()
    }

    #[test]
    fn a_components_payload_reaches_only_the_pattern_card_and_only_one_record() {
        let mut offered = stack("counterpart:card");
        offered.components.insert(
            Record::PatternContents.as_str().to_owned(),
            pattern_record(),
        );
        offered
            .components
            .insert("something_else".to_owned(), "smuggled".to_owned());
        let payload = HeldComponents { offered };

        assert_eq!(
            apply_components(Held::PatternCard, &payload, Record::PatternContents),
            Applied::Accepted
        );
        assert_eq!(
            apply_components(Held::Carrier, &payload, Record::PatternContents),
            Applied::Refused(Refusal::WrongHeldItem)
        );
        assert_eq!(
            apply_components(Held::PatternCard, &payload, Record::CarrierContents),
            Applied::Refused(Refusal::WrongRecord),
            "one named record is copied rather than the whole component map"
        );
    }

    #[test]
    fn a_components_payload_carrying_an_arbitrary_body_under_the_right_key_is_refused() {
        let mut offered = stack("counterpart:card");
        offered.components.insert(
            Record::PatternContents.as_str().to_owned(),
            "not a document".to_owned(),
        );
        assert_eq!(
            apply_components(
                Held::PatternCard,
                &HeldComponents { offered },
                Record::PatternContents
            ),
            Applied::Refused(Refusal::BadRecord),
            "knowing a key is present is not knowing its body is one this entry writes"
        );
    }

    #[test]
    fn each_of_the_five_records_is_decoded_by_its_own_rules() {
        assert!(decodes(Record::PatternContents, &pattern_record()));
        assert!(!decodes(Record::PatternContents, "17"));
        assert!(decodes(Record::InfusionProgress, "17"));
        assert!(!decodes(Record::InfusionProgress, "-1"));
        assert!(decodes(Record::CarrierContents, &carrier_record(true)));
        assert!(!decodes(Record::CarrierContents, &carrier_record(false)));
        assert!(decodes(
            Record::SelectedPosition,
            &Placement::new("counterpart:deep", (1, 2, 3))
                .expect("a legal dimension")
                .to_saved()
                .to_string()
        ));
        assert!(!decodes(Record::SelectedPosition, "{}"));
        assert!(!decodes(Record::MatchRule, "not a document"));
    }

    #[test]
    fn the_typed_arguments_payload_is_accepted_and_discarded() {
        let payload = CardArguments { named: Vec::new() };
        assert_eq!(
            apply_card_arguments(Held::PatternCard, &payload),
            Applied::Discarded
        );
        assert_eq!(
            apply_card_arguments(Held::MatchItem, &payload),
            Applied::Refused(Refusal::WrongHeldItem)
        );
    }

    #[test]
    fn the_browser_payload_needs_both_the_card_and_that_cards_own_menu() {
        let payload = BrowserPattern { slots: Vec::new() };
        assert_eq!(
            apply_browser_pattern(Held::PatternCard, true, &payload),
            Applied::Accepted
        );
        assert_eq!(
            apply_browser_pattern(Held::PatternCard, false, &payload),
            Applied::Refused(Refusal::WrongMenu)
        );
        assert_eq!(
            apply_browser_pattern(Held::Carrier, true, &payload),
            Applied::Refused(Refusal::WrongHeldItem)
        );
    }

    #[test]
    fn a_readout_request_is_bounded_by_distance_as_well_as_by_chunk_load() {
        let near = ReadoutRequest { at: (0, 0, 0) };
        assert_eq!(answer_readout(&near, (0, 0, 4), true), Applied::Accepted);
        assert_eq!(
            answer_readout(&near, (0, 0, 4), false),
            Applied::Refused(Refusal::ChunkAbsent)
        );

        let far = ReadoutRequest {
            at: (0, 0, i32::from(u8::MAX) * 8),
        };
        assert_eq!(
            answer_readout(&far, (0, 0, 0), true),
            Applied::Refused(Refusal::OutOfRange),
            "chunk load alone would let any player read any loaded position"
        );
    }

    #[test]
    fn a_hostile_pair_of_coordinates_cannot_wrap_back_into_range() {
        let extreme = ReadoutRequest {
            at: (i32::MIN, i32::MAX, i32::MIN),
        };
        assert_eq!(
            answer_readout(&extreme, (i32::MAX, i32::MIN, i32::MAX), true),
            Applied::Refused(Refusal::OutOfRange)
        );
    }

    #[test]
    fn the_wholesale_payload_validates_the_record_and_not_only_the_item_kind() {
        let mut sent = stack("counterpart:carrier");
        sent.components.insert(
            Record::CarrierContents.as_str().to_owned(),
            carrier_record(true),
        );
        let sound = HeldItem { sent };
        assert_eq!(
            apply_held_item(Held::Carrier, Held::Carrier, &sound),
            Applied::Accepted
        );

        let mut smuggled = stack("counterpart:carrier");
        smuggled.components.insert(
            Record::CarrierContents.as_str().to_owned(),
            carrier_record(false),
        );
        let hostile = HeldItem { sent: smuggled };
        assert_eq!(
            apply_held_item(Held::Carrier, Held::Carrier, &hostile),
            Applied::Refused(Refusal::BadRecord),
            "a class test alone would install a carrier whose next selection fails"
        );
    }

    #[test]
    fn the_wholesale_payload_refuses_an_item_kind_outside_the_two_permitted() {
        let mut sent = stack("counterpart:carrier");
        sent.components.insert(
            Record::CarrierContents.as_str().to_owned(),
            carrier_record(true),
        );
        let payload = HeldItem { sent };

        assert_eq!(
            apply_held_item(Held::PatternCard, Held::Carrier, &payload),
            Applied::Refused(Refusal::WrongHeldItem)
        );
        assert_eq!(
            apply_held_item(Held::Carrier, Held::Other, &payload),
            Applied::Refused(Refusal::WrongSentItem)
        );
        assert_eq!(
            apply_held_item(Held::Carrier, Held::MatchItem, &payload),
            Applied::Refused(Refusal::WrongSentItem),
            "the two permitted kinds are not interchangeable with each other"
        );
    }

    #[test]
    fn a_match_record_that_is_not_a_rule_is_refused_rather_than_installed() {
        let mut sent = stack("counterpart:match_item");
        sent.components.insert(
            Record::MatchRule.as_str().to_owned(),
            "not a document".to_owned(),
        );
        assert_eq!(
            apply_held_item(Held::MatchItem, Held::MatchItem, &HeldItem { sent }),
            Applied::Refused(Refusal::BadRecord)
        );
    }

    #[test]
    fn an_answer_carries_the_readings_in_the_order_they_were_collected() {
        let mut readings = Readings::new();
        readings.declare("stored", Reading::Amount(10));
        readings.declare("maximum", Reading::Amount(20));

        let answer = readout_answer((1, 2, 3), &readings).expect("bounded readings");
        assert_eq!(answer.at, (1, 2, 3));
        assert_eq!(
            answer.values,
            [
                ("stored".to_owned(), Reading::Amount(10)),
                ("maximum".to_owned(), Reading::Amount(20)),
            ]
        );
    }
}
