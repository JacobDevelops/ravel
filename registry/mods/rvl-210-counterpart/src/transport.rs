//! Section six: the message channel, its field ordering and its codec table.
//!
//! The ordering rule is the behaviour, not the encoding's incidental shape. A
//! message carries no field names, no lengths and no framing, so the receiver's
//! field order is the only thing telling it what it is reading. The order is:
//! walk the concrete message type upward through its ancestors, stopping before
//! the framework base; within each level take that level's own marked fields
//! sorted by name ascending; concatenate the levels most-derived first.
//!
//! Rust has no inheritance, so a message's levels are declared rather than
//! discovered. That is the one structural change here and it is the reason the
//! rule can be held to a test at all: [`Shape`] is a list of levels and
//! [`Shape::wire_order`] is the rule applied to it.

use std::collections::BTreeMap;

use ravel_sdk::network::{PacketReader, PacketWriter, WireError};

use core::fmt;

/// The most levels a message may declare.
///
/// A shape is built from declared data, and a walk with no bound is a walk a
/// malformed declaration can make arbitrarily long.
pub const MAX_LEVELS: usize = 32;

/// The most marked fields one message may carry.
pub const MAX_FIELDS: usize = 256;

/// One level of a message type: its own marked fields, in any order.
///
/// The names are what the rule sorts on, so a level that names the same field
/// twice is a declaration error rather than two fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level {
    fields: BTreeMap<String, String>,
}

impl Level {
    /// A level carrying no marked field.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fields: BTreeMap::new(),
        }
    }

    /// Marks one field for transport, naming the codec its declared type uses.
    ///
    /// A repeated name replaces the earlier mark, because a field is one field.
    #[must_use]
    pub fn mark(mut self, field: &str, codec: &str) -> Self {
        self.fields.insert(field.to_owned(), codec.to_owned());
        self
    }

    /// This level's marked fields, already in ascending name order.
    ///
    /// The sort is not a convenience: the language gives no guaranteed order for
    /// a type's declared fields, and two peers that disagreed about it would
    /// mis-decode every message.
    pub fn ordered(&self) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.fields
            .iter()
            .map(|(field, codec)| (field.as_str(), codec.as_str()))
    }

    /// How many fields this level marks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Whether this level marks nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

impl Default for Level {
    fn default() -> Self {
        Self::new()
    }
}

/// A concrete message type, as its levels from the most derived downward.
///
/// The framework base is not a level: the walk stops before it, so nothing the
/// framework itself marks is on the wire.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Shape {
    levels: Vec<Level>,
}

impl Shape {
    /// A shape with no level yet.
    #[must_use]
    pub const fn new() -> Self {
        Self { levels: Vec::new() }
    }

    /// Adds the next level down, which is an ancestor of every level so far.
    ///
    /// # Errors
    ///
    /// Returns [`ShapeError`] when the shape would pass [`MAX_LEVELS`] or
    /// [`MAX_FIELDS`].
    pub fn below(mut self, level: Level) -> Result<Self, ShapeError> {
        if self.levels.len() == MAX_LEVELS {
            return Err(ShapeError::TooManyLevels { limit: MAX_LEVELS });
        }
        let fields = self.field_count() + level.len();
        if fields > MAX_FIELDS {
            return Err(ShapeError::TooManyFields { limit: MAX_FIELDS });
        }
        self.levels.push(level);
        Ok(self)
    }

    /// How many levels the walk crosses.
    #[must_use]
    pub fn levels(&self) -> usize {
        self.levels.len()
    }

    /// How many fields this message puts on the wire.
    #[must_use]
    pub fn field_count(&self) -> usize {
        self.levels.iter().map(Level::len).sum()
    }

    /// The fields in the order they are written and read.
    ///
    /// Most-derived level first, and within a level ascending by field name.
    pub fn wire_order(&self) -> Vec<(&str, &str)> {
        self.levels.iter().flat_map(Level::ordered).collect()
    }

    /// Writes a message's values in this shape's order.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when a field has no value, when a value is not
    /// of the kind its codec writes, or when the shape names a codec the table
    /// does not hold.
    pub fn write(
        &self,
        table: &Codecs,
        values: &BTreeMap<String, Value>,
        writer: &mut PacketWriter,
    ) -> Result<(), TransportError> {
        for (field, codec) in self.wire_order() {
            let value = values.get(field).ok_or_else(|| TransportError::NoValue {
                field: field.to_owned(),
            })?;
            let codec = table.get(codec).ok_or_else(|| TransportError::NoCodec {
                codec: codec.to_owned(),
            })?;
            codec.write(value, writer)?;
        }
        Ok(())
    }

    /// Reads a message's values back in this shape's order.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the bytes run out, when they do not spell
    /// the declared type, or when the shape names a codec the table lacks.
    pub fn read(
        &self,
        table: &Codecs,
        reader: &mut PacketReader<'_>,
    ) -> Result<BTreeMap<String, Value>, TransportError> {
        let mut values = BTreeMap::new();
        for (field, codec) in self.wire_order() {
            let codec = table.get(codec).ok_or_else(|| TransportError::NoCodec {
                codec: codec.to_owned(),
            })?;
            values.insert(field.to_owned(), codec.read(reader)?);
        }
        Ok(values)
    }
}

/// What a shape refuses at declaration time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShapeError {
    /// The walk would cross more levels than a message may declare.
    TooManyLevels {
        /// The published limit.
        limit: usize,
    },
    /// The message would put more fields on the wire than one may carry.
    TooManyFields {
        /// The published limit.
        limit: usize,
    },
}

impl fmt::Display for ShapeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyLevels { limit } => {
                write!(formatter, "a message may declare at most {limit} levels")
            }
            Self::TooManyFields { limit } => {
                write!(formatter, "a message may carry at most {limit} fields")
            }
        }
    }
}

/// One value on the wire, in the four kinds the portable table writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    /// A truth value.
    Truth(bool),
    /// A whole number.
    Number(u32),
    /// Text.
    Text(String),
    /// Opaque bytes.
    Bytes(Vec<u8>),
}

impl Value {
    /// Which codec kind this value belongs to.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Truth(_) => TRUTH,
            Self::Number(_) => NUMBER,
            Self::Text(_) => TEXT,
            Self::Bytes(_) => BYTES,
        }
    }
}

/// The codec name for a truth value.
pub const TRUTH: &str = "truth";
/// The codec name for a whole number.
pub const NUMBER: &str = "number";
/// The codec name for text.
pub const TEXT: &str = "text";
/// The codec name for opaque bytes.
pub const BYTES: &str = "bytes";

/// The four codecs the table holds without being loaded.
pub const PORTABLE_CODECS: [&str; 4] = [TRUTH, NUMBER, TEXT, BYTES];

/// One entry in the type-to-encoder table.
#[derive(Clone, Copy, Debug)]
pub struct Codec {
    kind: &'static str,
}

impl Codec {
    fn write(self, value: &Value, writer: &mut PacketWriter) -> Result<(), TransportError> {
        if value.kind() != self.kind {
            return Err(TransportError::WrongKind {
                codec: self.kind,
                value: value.kind(),
            });
        }
        match value {
            Value::Truth(truth) => writer.bool(*truth),
            Value::Number(number) => writer.u32(*number),
            Value::Text(text) => writer.text(text),
            Value::Bytes(bytes) => writer.bytes_field(bytes),
        }
        Ok(())
    }

    fn read(self, reader: &mut PacketReader<'_>) -> Result<Value, TransportError> {
        let value = match self.kind {
            TRUTH => Value::Truth(reader.bool()?),
            NUMBER => Value::Number(reader.u32()?),
            TEXT => Value::Text(reader.text()?.to_owned()),
            BYTES => Value::Bytes(reader.bytes_field()?.to_vec()),
            other => {
                return Err(TransportError::NoCodec {
                    codec: other.to_owned(),
                });
            }
        };
        Ok(value)
    }
}

/// The type-to-encoder table.
///
/// The portable entries exist as soon as the table does, which is what the
/// specification's static initialiser gives the original. This platform's
/// additions arrive at common setup through [`Codecs::load_platform`], and a
/// dependent adds its own with [`Codecs::add`]. A message sent before the
/// platform additions land has no entry to read, and that is the ordering the
/// specification calls load-bearing.
#[derive(Clone, Debug)]
pub struct Codecs {
    entries: BTreeMap<String, Codec>,
    platform_loaded: bool,
}

impl Default for Codecs {
    fn default() -> Self {
        Self::new()
    }
}

impl Codecs {
    /// The table holding its portable entries and nothing else.
    #[must_use]
    pub fn new() -> Self {
        let mut entries = BTreeMap::new();
        for kind in PORTABLE_CODECS {
            entries.insert(kind.to_owned(), Codec { kind });
        }
        Self {
            entries,
            platform_loaded: false,
        }
    }

    /// Adds this platform's entries, which is what common setup does.
    ///
    /// Answers whether anything was added, so a second call is visible rather
    /// than silent.
    pub fn load_platform(&mut self, additions: &[(&str, &'static str)]) -> bool {
        if self.platform_loaded {
            return false;
        }
        for (name, kind) in additions {
            self.entries.insert((*name).to_owned(), Codec { kind });
        }
        self.platform_loaded = true;
        true
    }

    /// Whether this platform's additions have been loaded yet.
    #[must_use]
    pub const fn platform_loaded(&self) -> bool {
        self.platform_loaded
    }

    /// Adds one entry for a dependent's own declared field type.
    pub fn add(&mut self, name: &str, kind: &'static str) {
        self.entries.insert(name.to_owned(), Codec { kind });
    }

    /// The entry for one declared field type.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Codec> {
        self.entries.get(name).copied()
    }

    /// How many entries the table holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the table is empty, which it never is.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// What the transport refuses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportError {
    /// The shape marks a field the caller supplied no value for.
    NoValue {
        /// The field.
        field: String,
    },
    /// The shape names a codec the table does not hold.
    NoCodec {
        /// The codec name.
        codec: String,
    },
    /// The value is not of the kind the codec writes.
    WrongKind {
        /// What the codec writes.
        codec: &'static str,
        /// What it was handed.
        value: &'static str,
    },
    /// The bytes do not spell what the shape says they do.
    Wire(WireError),
}

impl From<WireError> for TransportError {
    fn from(error: WireError) -> Self {
        Self::Wire(error)
    }
}

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoValue { field } => write!(formatter, "no value for the marked field {field}"),
            Self::NoCodec { codec } => write!(formatter, "the table holds no codec named {codec}"),
            Self::WrongKind { codec, value } => {
                write!(formatter, "a {codec} codec was handed a {value}")
            }
            Self::Wire(error) => write!(formatter, "the bytes do not spell the message: {error}"),
        }
    }
}

/// Who a message goes to.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// One player.
    Player {
        /// Which one.
        player: String,
    },
    /// Every player.
    Everyone,
    /// Every player within a radius of a point in a level.
    Nearby {
        /// The level the point is in.
        level: String,
        /// The point.
        at: [f64; 3],
        /// How far the message carries.
        radius: f64,
    },
}

impl Target {
    /// The radius the cosmetic effect of section nine broadcasts within.
    pub const COSMETIC_RADIUS: f64 = 50.0;

    /// Whether a player at a point in a level is a recipient.
    ///
    /// A non-finite or negative radius reaches nobody rather than everybody: the
    /// radius comes from a caller and a comparison against a non-finite number
    /// is false in every direction, so the case is decided here rather than left
    /// to the arithmetic.
    #[must_use]
    pub fn reaches(&self, player: &str, level: &str, at: [f64; 3]) -> bool {
        match self {
            Self::Player { player: one } => one == player,
            Self::Everyone => true,
            Self::Nearby {
                level: theirs,
                at: centre,
                radius,
            } => {
                if theirs != level || !radius.is_finite() || *radius < 0.0 {
                    return false;
                }
                let squared: f64 = (0..3)
                    .map(|axis| {
                        let gap = at[axis] - centre[axis];
                        gap * gap
                    })
                    .sum();
                squared <= radius * radius
            }
        }
    }
}

/// The thirteen message kinds this entry registers of its own.
///
/// The order is the specification's own list. The player-position base is not
/// one of them and is [`POSITION_BASE`].
pub const MESSAGE_KINDS: [&str; 13] = [
    "resource_reload_request",
    "cosmetic_effect",
    "screen_button_click",
    "screen_value_update",
    "stored_data_request",
    "stored_data_reply",
    "advancement_unlocked_request",
    "advancement_unlocked_reply",
    "advancement_reward_claim",
    "diagnostic_ping",
    "diagnostic_reply",
    "diagnostic_payload",
    "diagnostic_timing",
];

/// The abstract player-position message, published for dependents to extend.
///
/// It is not one of [`MESSAGE_KINDS`]: this entry sends nothing under it.
pub const POSITION_BASE: &str = "player_position";

/// The shape the player-position base contributes to anything extending it.
///
/// Its three fields sort ascending within their own level, so a dependent's own
/// level is written first and these three follow in this order whatever the
/// dependent called its own fields.
#[must_use]
pub fn position_level() -> Level {
    Level::new()
        .mark("x", NUMBER)
        .mark("y", NUMBER)
        .mark("z", NUMBER)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape() -> Shape {
        Shape::new()
            .below(Level::new().mark("zeta", NUMBER).mark("alpha", TRUTH))
            .and_then(|shape| shape.below(Level::new().mark("beta", TEXT).mark("gamma", NUMBER)))
            .expect("two levels and four fields are inside both limits")
    }

    fn values() -> BTreeMap<String, Value> {
        BTreeMap::from([
            ("alpha".to_owned(), Value::Truth(true)),
            ("zeta".to_owned(), Value::Number(9)),
            ("beta".to_owned(), Value::Text("written".to_owned())),
            ("gamma".to_owned(), Value::Number(4)),
        ])
    }

    #[test]
    fn the_wire_order_is_most_derived_first_and_ascending_by_name_within_a_level() {
        let shape = shape();
        let order: Vec<&str> = shape
            .wire_order()
            .into_iter()
            .map(|(field, _)| field)
            .collect();
        assert_eq!(order, vec!["alpha", "zeta", "beta", "gamma"]);
    }

    #[test]
    fn the_order_is_not_declaration_order() {
        let declared = ["zeta", "alpha", "beta", "gamma"];
        let shape = shape();
        let order: Vec<&str> = shape
            .wire_order()
            .into_iter()
            .map(|(field, _)| field)
            .collect();
        assert_ne!(order, declared.to_vec());
    }

    #[test]
    fn a_message_round_trips_through_the_order_it_was_written_in() {
        let table = Codecs::new();
        let shape = shape();
        let mut writer = PacketWriter::default();
        shape
            .write(&table, &values(), &mut writer)
            .expect("every field has a value and a codec");
        let bytes = writer.finish();
        let mut reader = PacketReader::new(&bytes);
        let read = shape.read(&table, &mut reader).expect("the same order");
        assert_eq!(read, values());
        assert!(reader.finish().is_ok(), "nothing was left unread");
    }

    #[test]
    fn two_peers_that_disagree_about_the_order_disagree_about_the_bytes() {
        let table = Codecs::new();
        let mine = shape();
        let theirs = Shape::new()
            .below(Level::new().mark("beta", TEXT).mark("gamma", NUMBER))
            .and_then(|shape| shape.below(Level::new().mark("zeta", NUMBER).mark("alpha", TRUTH)))
            .expect("the same four fields with the levels the other way up");
        let mut writer = PacketWriter::default();
        mine.write(&table, &values(), &mut writer)
            .expect("the fields are all there");
        let bytes = writer.finish();
        let mut reader = PacketReader::new(&bytes);
        let read = theirs.read(&table, &mut reader);
        assert!(
            read.is_err() || read.ok() != Some(values()),
            "a peer reading in another order must not read the same message"
        );
    }

    #[test]
    fn the_bytes_carry_no_field_name_and_no_framing() {
        let table = Codecs::new();
        let mut writer = PacketWriter::default();
        shape()
            .write(&table, &values(), &mut writer)
            .expect("the fields are all there");
        let bytes = writer.finish();
        assert!(
            !bytes.windows(5).any(|window| window == b"alpha"),
            "a field name reached the wire"
        );
        // One truth, one number, the text with its length, and one number.
        assert_eq!(bytes.len(), 1 + 4 + 4 + "written".len() + 4);
    }

    #[test]
    fn a_field_with_no_value_and_a_codec_the_table_lacks_are_both_refused() {
        let table = Codecs::new();
        let mut writer = PacketWriter::default();
        let mut short = values();
        short.remove("beta");
        assert_eq!(
            shape().write(&table, &short, &mut writer),
            Err(TransportError::NoValue {
                field: "beta".to_owned()
            })
        );
        let unknown = Shape::new()
            .below(Level::new().mark("one", "nothing_declares_this"))
            .expect("one level");
        let supplied = BTreeMap::from([("one".to_owned(), Value::Number(1))]);
        assert!(matches!(
            unknown.write(&table, &supplied, &mut writer),
            Err(TransportError::NoCodec { .. })
        ));
    }

    #[test]
    fn a_value_of_the_wrong_kind_is_refused_rather_than_written_as_another() {
        let table = Codecs::new();
        let mut writer = PacketWriter::default();
        let shape = Shape::new()
            .below(Level::new().mark("one", NUMBER))
            .expect("one level");
        let values = BTreeMap::from([("one".to_owned(), Value::Text("nine".to_owned()))]);
        assert_eq!(
            shape.write(&table, &values, &mut writer),
            Err(TransportError::WrongKind {
                codec: NUMBER,
                value: TEXT,
            })
        );
    }

    #[test]
    fn truncated_bytes_are_refused_rather_than_read_as_a_shorter_message() {
        let table = Codecs::new();
        let mut writer = PacketWriter::default();
        shape()
            .write(&table, &values(), &mut writer)
            .expect("the fields are all there");
        let bytes = writer.finish();
        for cut in 0..bytes.len() {
            let mut reader = PacketReader::new(&bytes[..cut]);
            assert!(
                shape().read(&table, &mut reader).is_err(),
                "a message cut at {cut} bytes was read as whole"
            );
        }
    }

    #[test]
    fn the_portable_entries_exist_before_the_platform_ones_are_loaded() {
        let mut table = Codecs::new();
        assert_eq!(table.len(), PORTABLE_CODECS.len());
        assert!(!table.platform_loaded());
        for name in PORTABLE_CODECS {
            assert!(table.get(name).is_some());
        }
        assert!(table.get("position").is_none());
        assert!(table.load_platform(&[("position", NUMBER)]));
        assert!(table.get("position").is_some());
        assert!(
            !table.load_platform(&[("position", NUMBER)]),
            "common setup runs once and a second load says so"
        );
    }

    #[test]
    fn a_dependent_adds_its_own_declared_field_type() {
        let mut table = Codecs::new();
        table.add("mine", TEXT);
        assert_eq!(table.len(), PORTABLE_CODECS.len() + 1);
        assert!(table.get("mine").is_some());
    }

    #[test]
    fn a_shape_refuses_more_levels_and_more_fields_than_it_publishes() {
        let mut shape = Shape::new();
        for _ in 0..MAX_LEVELS {
            shape = shape.below(Level::new()).expect("inside the limit");
        }
        assert_eq!(
            shape.clone().below(Level::new()),
            Err(ShapeError::TooManyLevels { limit: MAX_LEVELS })
        );
        let mut wide = Level::new();
        for field in 0..=MAX_FIELDS {
            wide = wide.mark(&format!("field{field:04}"), NUMBER);
        }
        assert_eq!(
            Shape::new().below(wide),
            Err(ShapeError::TooManyFields { limit: MAX_FIELDS })
        );
    }

    #[test]
    fn a_level_that_names_one_field_twice_carries_it_once() {
        let level = Level::new().mark("one", NUMBER).mark("one", TEXT);
        assert_eq!(level.len(), 1);
        assert_eq!(level.ordered().next(), Some(("one", TEXT)));
    }

    #[test]
    fn this_entry_registers_thirteen_message_kinds_and_the_base_is_not_one() {
        assert_eq!(MESSAGE_KINDS.len(), 13);
        let mut sorted = MESSAGE_KINDS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 13, "every kind is named once");
        assert!(!MESSAGE_KINDS.contains(&POSITION_BASE));
    }

    #[test]
    fn the_position_base_contributes_its_three_fields_after_a_dependent_s_own() {
        let shape = Shape::new()
            .below(Level::new().mark("aim", NUMBER))
            .and_then(|shape| shape.below(position_level()))
            .expect("two levels");
        let order: Vec<&str> = shape
            .wire_order()
            .into_iter()
            .map(|(field, _)| field)
            .collect();
        assert_eq!(order, vec!["aim", "x", "y", "z"]);
    }

    #[test]
    fn a_message_to_one_player_reaches_that_player_only() {
        let target = Target::Player {
            player: "one".to_owned(),
        };
        assert!(target.reaches("one", "overworld", [0.0, 0.0, 0.0]));
        assert!(!target.reaches("two", "overworld", [0.0, 0.0, 0.0]));
    }

    #[test]
    fn a_message_to_everyone_reaches_every_player_in_every_level() {
        let target = Target::Everyone;
        assert!(target.reaches("one", "overworld", [0.0, 0.0, 0.0]));
        assert!(target.reaches("two", "elsewhere", [9e9, 0.0, 0.0]));
    }

    #[test]
    fn the_cosmetic_effect_carries_fifty_blocks_and_stops_at_its_edge() {
        let target = Target::Nearby {
            level: "overworld".to_owned(),
            at: [0.0, 0.0, 0.0],
            radius: Target::COSMETIC_RADIUS,
        };
        assert!((Target::COSMETIC_RADIUS - 50.0).abs() < f64::EPSILON);
        assert!(target.reaches("one", "overworld", [50.0, 0.0, 0.0]));
        assert!(!target.reaches("one", "overworld", [50.1, 0.0, 0.0]));
        assert!(!target.reaches("one", "elsewhere", [0.0, 0.0, 0.0]));
        assert!(target.reaches("one", "overworld", [28.0, 28.0, 28.0]));
    }

    #[test]
    fn a_radius_that_is_not_a_number_reaches_nobody() {
        for radius in [f64::NAN, f64::INFINITY, -1.0] {
            let target = Target::Nearby {
                level: "overworld".to_owned(),
                at: [0.0, 0.0, 0.0],
                radius,
            };
            assert!(!target.reaches("one", "overworld", [0.0, 0.0, 0.0]));
        }
    }
}
