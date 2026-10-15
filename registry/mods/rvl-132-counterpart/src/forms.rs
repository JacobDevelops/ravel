//! Section seven's serialised forms, and the tag model they are written in.
//!
//! These forms are storable by a dependent and by a saved world, so they are a
//! compatibility surface rather than an implementation choice, and section seven
//! specifies them exactly.
//!
//! Widths are part of the specification: the charge decoder accepts a 32-bit
//! integer as well as a 64-bit one and widens it, every other decoder refuses a
//! tag of the wrong shape, and the item decoder additionally refuses anything
//! that is not a compound. A form that erased widths could not express any of
//! that, so [`Tag`] carries them and [`Tag::to_json`] maps them onto the SDK's
//! value type without losing them.
//!
//! Section 17 leaves the count-escape field's spelling unresolved, and section
//! seven states that the spelling is a compatibility requirement. It is
//! therefore read from a build-time input rather than invented:
//! [`EscapeField::declared`] is what a save-compatible build supplies and
//! [`EscapeField::placeholder`] is what the tests pin the surrounding format
//! with.

use core::fmt;
use std::collections::BTreeMap;

use ravel_sdk::{JsonValue, json};
use rvl_210_counterpart::quantity::{Amount, KindError};

use crate::data::{self, Components, Value};
use crate::kinds::{Charge, Flags, Portion, Stack, Truth};

/// The build-time input the count-escape field's spelling is read from.
pub const ESCAPE_INPUT: &str = "RVL_132_COUNTERPART_COUNT_ESCAPE_FIELD";

/// The count the game's own item encoding caps at.
///
/// Section seven: above this the entry writes the item with its count clamped
/// here and carries the true count in the escape field.
pub const COUNT_CAP: Amount = 99;

/// The most entries one compound or list tag may carry.
pub const MAX_TAG_ENTRIES: usize = 512;

/// The most levels a tag tree may nest.
///
/// Reading a tag is recursive and this is what stops a hostile form from running
/// the read away.
pub const MAX_TAG_DEPTH: u32 = 64;

/// What a stored form refuses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TagError {
    /// The stored form is not the shape this decoder reads.
    Shape {
        /// What was expected.
        expected: &'static str,
    },
    /// A named field is missing.
    Missing {
        /// The field's name.
        field: String,
    },
    /// A number is outside the range its field carries.
    Range {
        /// The field's name.
        field: String,
    },
    /// The form nests past [`MAX_TAG_DEPTH`].
    TooDeep {
        /// The published ceiling.
        limit: u32,
    },
    /// A compound or list carries more than [`MAX_TAG_ENTRIES`].
    TooMany {
        /// The published ceiling.
        limit: usize,
    },
    /// A clamped count arrived without the escape field that carries the true
    /// one.
    ///
    /// Defect eleven: a reader unaware of the field accepts the clamped count
    /// with no error, which is silent data loss rather than a decode failure.
    /// This package's own writer always supplies the field at or above the cap,
    /// so its own reader requires it.
    NoEscape {
        /// The field the true count is carried in.
        field: String,
    },
    /// No build supplied the compatibility-required escape field spelling.
    NoEscapeDeclared {
        /// The build-time input that supplies it.
        input: &'static str,
    },
    /// The data map the form carries was refused.
    Data(data::Refused),
}

impl TagError {
    /// The same refusal as the vocabulary's own.
    ///
    /// [`rvl_210_counterpart::quantity::QuantityKind::read`] answers a
    /// [`KindError`], so this is how a refusal reaches a dependent that only
    /// names the vocabulary.
    #[must_use]
    pub fn into_kind_error(self) -> KindError {
        KindError::new(self.to_string())
    }
}

impl fmt::Display for TagError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape { expected } => write!(formatter, "the stored form is not {expected}"),
            Self::Missing { field } => write!(formatter, "the stored form names no {field}"),
            Self::Range { field } => write!(formatter, "{field} is outside the range it carries"),
            Self::TooDeep { limit } => {
                write!(formatter, "the stored form nests past the {limit} admitted")
            }
            Self::TooMany { limit } => {
                write!(formatter, "a tag carries more than the {limit} admitted")
            }
            Self::NoEscape { field } => write!(
                formatter,
                "a clamped count arrived without {field}, so the true count is lost"
            ),
            Self::NoEscapeDeclared { input } => write!(
                formatter,
                "no build supplied {input}, so the escape field has no compatible spelling"
            ),
            Self::Data(refused) => refused.fmt(formatter),
        }
    }
}

impl From<data::Refused> for TagError {
    fn from(refused: data::Refused) -> Self {
        Self::Data(refused)
    }
}

/// One tag, at the width the stored form carries it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Tag {
    /// A signed byte.
    Byte(i8),
    /// A signed 32-bit integer.
    Int(i32),
    /// A signed 64-bit integer.
    Long(i64),
    /// Text.
    Text(String),
    /// A named compound.
    Compound(BTreeMap<String, Self>),
    /// An ordered list.
    List(Vec<Self>),
}

impl Tag {
    /// An empty compound.
    #[must_use]
    pub fn compound() -> Self {
        Self::Compound(BTreeMap::new())
    }

    /// How deeply this tag nests, counting itself as one level.
    #[must_use]
    pub fn depth(&self) -> u32 {
        match self {
            Self::Byte(_) | Self::Int(_) | Self::Long(_) | Self::Text(_) => 1,
            Self::Compound(fields) => {
                1 + fields.values().map(Self::depth).max().unwrap_or_default()
            }
            Self::List(values) => 1 + values.iter().map(Self::depth).max().unwrap_or_default(),
        }
    }

    /// This tag as one of the SDK's values, keeping its width.
    #[must_use]
    pub fn to_json(&self) -> JsonValue {
        match self {
            Self::Byte(value) => json!({ "byte": value }),
            Self::Int(value) => json!({ "int": value }),
            Self::Long(value) => json!({ "long": value }),
            Self::Text(value) => json!({ "text": value }),
            // A compound is written as its pairs in name order rather than as
            // an object, so the form carries its order and a repeated name is
            // visible to the reader rather than folded away by the writer.
            Self::Compound(fields) => {
                let written: Vec<JsonValue> = fields
                    .iter()
                    .map(|(name, tag)| json!([name, tag.to_json()]))
                    .collect();
                json!({ "compound": written })
            }
            Self::List(values) => {
                let written: Vec<JsonValue> = values.iter().map(Self::to_json).collect();
                json!({ "list": written })
            }
        }
    }

    /// Reads a tag back.
    ///
    /// # Errors
    ///
    /// Returns [`TagError::Shape`] for anything that is not one of the six
    /// widths, [`TagError::Range`] for a number outside its width,
    /// [`TagError::TooDeep`] past [`MAX_TAG_DEPTH`] and [`TagError::TooMany`]
    /// past [`MAX_TAG_ENTRIES`].
    pub fn from_json(stored: &JsonValue) -> Result<Self, TagError> {
        Self::read(stored, 0)
    }

    fn read(stored: &JsonValue, depth: u32) -> Result<Self, TagError> {
        if depth >= MAX_TAG_DEPTH {
            return Err(TagError::TooDeep {
                limit: MAX_TAG_DEPTH,
            });
        }
        let object = stored.as_object().ok_or(TagError::Shape {
            expected: "a tag with a width",
        })?;
        if object.len() != 1 {
            return Err(TagError::Shape {
                expected: "a tag naming exactly one width",
            });
        }
        let (width, body) = object.iter().next().ok_or(TagError::Shape {
            expected: "a tag naming exactly one width",
        })?;
        match width.as_str() {
            "byte" => Ok(Self::Byte(narrow_byte(body)?)),
            "int" => Ok(Self::Int(narrow_int(body)?)),
            "long" => Ok(Self::Long(body.as_i64().ok_or_else(|| {
                TagError::Range {
                    field: "long".to_owned(),
                }
            })?)),
            "text" => Ok(Self::Text(
                body.as_str()
                    .ok_or(TagError::Shape { expected: "text" })?
                    .to_owned(),
            )),
            "compound" => {
                let pairs = body.as_array().ok_or(TagError::Shape {
                    expected: "a compound's pairs",
                })?;
                if pairs.len() > MAX_TAG_ENTRIES {
                    return Err(TagError::TooMany {
                        limit: MAX_TAG_ENTRIES,
                    });
                }
                let mut read = BTreeMap::new();
                for pair in pairs {
                    let pair =
                        pair.as_array()
                            .filter(|pair| pair.len() == 2)
                            .ok_or(TagError::Shape {
                                expected: "a name and a tag",
                            })?;
                    let name = pair[0].as_str().ok_or(TagError::Shape {
                        expected: "a name and a tag",
                    })?;
                    let tag = Self::read(&pair[1], depth + 1)?;
                    if read.insert(name.to_owned(), tag).is_some() {
                        return Err(TagError::Shape {
                            expected: "a compound naming each field once",
                        });
                    }
                }
                Ok(Self::Compound(read))
            }
            "list" => Self::read_list(body, depth),
            _ => Err(TagError::Shape {
                expected: "one of the six widths",
            }),
        }
    }

    fn read_list(body: &JsonValue, depth: u32) -> Result<Self, TagError> {
        let values = body
            .as_array()
            .ok_or(TagError::Shape { expected: "a list" })?;
        if values.len() > MAX_TAG_ENTRIES {
            return Err(TagError::TooMany {
                limit: MAX_TAG_ENTRIES,
            });
        }
        let mut read = Vec::with_capacity(values.len());
        for value in values {
            read.push(Self::read(value, depth + 1)?);
        }
        Ok(Self::List(read))
    }

    fn fields(&self) -> Result<&BTreeMap<String, Self>, TagError> {
        match self {
            Self::Compound(fields) => Ok(fields),
            Self::Byte(_) | Self::Int(_) | Self::Long(_) | Self::Text(_) | Self::List(_) => {
                Err(TagError::Shape {
                    expected: "a compound",
                })
            }
        }
    }
}

fn narrow_byte(body: &JsonValue) -> Result<i8, TagError> {
    let raw = body.as_i64().ok_or_else(|| TagError::Range {
        field: "byte".to_owned(),
    })?;
    i8::try_from(raw).map_err(|_| TagError::Range {
        field: "byte".to_owned(),
    })
}

fn narrow_int(body: &JsonValue) -> Result<i32, TagError> {
    let raw = body.as_i64().ok_or_else(|| TagError::Range {
        field: "int".to_owned(),
    })?;
    i32::try_from(raw).map_err(|_| TagError::Range {
        field: "int".to_owned(),
    })
}

/// The count-escape field's spelling.
///
/// Section seven makes the spelling a compatibility requirement and section 17
/// records that it is obtainable only from the pinned artifact. Nothing here
/// guesses it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscapeField(String);

impl EscapeField {
    /// The spelling this build supplied.
    ///
    /// # Errors
    ///
    /// Returns [`TagError::NoEscapeDeclared`] when the build supplied nothing.
    /// A package that has to agree with the original cannot fall back to a
    /// placeholder and still claim to agree, so this is an error rather than a
    /// default.
    pub fn declared() -> Result<Self, TagError> {
        Self::supplied(option_env!("RVL_132_COUNTERPART_COUNT_ESCAPE_FIELD"))
    }

    /// The spelling a supplied value names, refusing an empty one.
    ///
    /// # Errors
    ///
    /// Returns [`TagError::NoEscapeDeclared`] for nothing supplied and for a
    /// blank or whitespace-only spelling.
    pub fn supplied(value: Option<&str>) -> Result<Self, TagError> {
        match value.map(str::trim) {
            Some(spelling) if !spelling.is_empty() => Ok(Self(spelling.to_owned())),
            _ => Err(TagError::NoEscapeDeclared {
                input: ESCAPE_INPUT,
            }),
        }
    }

    /// This package's own spelling, which pins the format and not the identity.
    #[must_use]
    pub fn placeholder() -> Self {
        Self("rvl_132_counterpart_true_count".to_owned())
    }

    /// The spelling.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl Default for EscapeField {
    fn default() -> Self {
        Self::placeholder()
    }
}

fn data_tag(data: &Components) -> Tag {
    Tag::Compound(
        data.iter()
            .map(|(name, value)| (name.clone(), value_tag(value)))
            .collect(),
    )
}

/// A component value is written under a name for its shape, so that a record
/// and a sequence are told apart from the values they hold and every shape reads
/// back as the one that was written.
fn value_tag(value: &Value) -> Tag {
    let (shape, body) = match value {
        Value::Truth(truth) => ("truth", Tag::Byte(i8::from(*truth))),
        Value::Whole(whole) => ("whole", Tag::Long(*whole)),
        // By bits, so that a decimal that is not a number survives the round
        // trip as the same value the order and the digest read.
        Value::Decimal(decimal) => ("decimal", Tag::Long(decimal.to_bits().cast_signed())),
        Value::Text(text) => ("text", Tag::Text(text.clone())),
        Value::Sequence(values) => (
            "sequence",
            Tag::List(values.iter().map(value_tag).collect()),
        ),
        Value::Record(fields) => (
            "record",
            Tag::Compound(
                fields
                    .iter()
                    .map(|(name, value)| (name.clone(), value_tag(value)))
                    .collect(),
            ),
        ),
    };
    Tag::Compound(BTreeMap::from([(shape.to_owned(), body)]))
}

fn value_from(tag: &Tag, depth: u32) -> Result<Value, TagError> {
    if depth >= data::MAX_VALUE_DEPTH {
        return Err(TagError::TooDeep {
            limit: data::MAX_VALUE_DEPTH,
        });
    }
    let fields = tag.fields()?;
    if fields.len() != 1 {
        return Err(TagError::Shape {
            expected: "a value naming exactly one shape",
        });
    }
    let (shape, body) = fields.iter().next().ok_or(TagError::Shape {
        expected: "a value naming exactly one shape",
    })?;
    Ok(match (shape.as_str(), body) {
        ("truth", Tag::Byte(byte)) => Value::Truth(*byte != 0),
        ("whole", Tag::Long(whole)) => Value::Whole(*whole),
        ("decimal", Tag::Long(bits)) => Value::Decimal(f64::from_bits(bits.cast_unsigned())),
        ("text", Tag::Text(text)) => Value::Text(text.clone()),
        ("sequence", Tag::List(values)) => Value::Sequence(
            values
                .iter()
                .map(|value| value_from(value, depth + 1))
                .collect::<Result<Vec<Value>, TagError>>()?,
        ),
        ("record", Tag::Compound(inner)) => {
            let mut read = BTreeMap::new();
            for (name, value) in inner {
                read.insert(name.clone(), value_from(value, depth + 1)?);
            }
            Value::Record(read)
        }
        (
            _,
            Tag::Byte(_)
            | Tag::Int(_)
            | Tag::Long(_)
            | Tag::Text(_)
            | Tag::Compound(_)
            | Tag::List(_),
        ) => {
            return Err(TagError::Shape {
                expected: "a value whose shape and body agree",
            });
        }
    })
}

fn data_from(tag: &Tag) -> Result<Components, TagError> {
    let fields = tag.fields()?;
    if fields.len() > data::MAX_COMPONENTS {
        return Err(TagError::TooMany {
            limit: data::MAX_COMPONENTS,
        });
    }
    let mut entries = Vec::with_capacity(fields.len());
    for (name, value) in fields {
        entries.push((name.clone(), value_from(value, 0)?));
    }
    Ok(data::components(entries)?)
}

/// Writes one stackable item, with the escape field this build supplies.
///
/// Section seven: the game's own optional item encoding, plus the escape field.
/// Above [`COUNT_CAP`] the written count is clamped and the true one goes in the
/// escape field, so a reader unaware of the field sees the cap rather than a
/// decode failure. This writer supplies the field at the cap as well as above
/// it, which is what makes [`stack_from_with`] able to require it.
///
/// # Errors
///
/// Returns [`TagError::Range`] for a count past the stackable kind's maximum.
/// `Stack` carries public fields and no invariant, so such an instance can
/// exist; writing it as the maximum would read back as a different number with
/// no error, which is the silent loss defect eleven is about.
pub fn stack_tag_with(value: &Stack, escape: &EscapeField) -> Result<Tag, TagError> {
    let mut fields = BTreeMap::new();
    if value.is_empty() {
        return Ok(Tag::Compound(fields));
    }
    let true_count = i32::try_from(value.count).map_err(|_| TagError::Range {
        field: "count".to_owned(),
    })?;
    fields.insert("id".to_owned(), Tag::Text(value.identity.clone()));
    let clamped = value.count.min(COUNT_CAP);
    let written = i32::try_from(clamped).map_err(|_| TagError::Range {
        field: "count".to_owned(),
    })?;
    fields.insert("count".to_owned(), Tag::Int(written));
    if value.count >= COUNT_CAP {
        fields.insert(escape.name().to_owned(), Tag::Int(true_count));
    }
    if !value.data.is_empty() {
        fields.insert("components".to_owned(), data_tag(&value.data));
    }
    Ok(Tag::Compound(fields))
}

/// Writes one stackable item under the placeholder escape spelling.
///
/// # Errors
///
/// Returns whatever [`stack_tag_with`] refuses.
pub fn stack_tag(value: &Stack) -> Result<Tag, TagError> {
    stack_tag_with(value, &EscapeField::placeholder())
}

/// Reads one stackable item back.
///
/// # Errors
///
/// Returns [`TagError::Shape`] for anything that is not a compound, which the
/// item decoder refuses even when the value it parsed came back fine, and
/// [`TagError::NoEscape`] for a clamped count with no escape field.
pub fn stack_from_with(tag: &Tag, escape: &EscapeField) -> Result<Stack, TagError> {
    let fields = tag.fields()?;
    let Some(identity) = fields.get("id") else {
        return Ok(Stack::empty());
    };
    let Tag::Text(identity) = identity else {
        return Err(TagError::Shape { expected: "text" });
    };
    let Some(Tag::Int(written)) = fields.get("count") else {
        return Err(TagError::Missing {
            field: "count".to_owned(),
        });
    };
    let written = Amount::try_from(*written).map_err(|_| TagError::Range {
        field: "count".to_owned(),
    })?;
    let count = match fields.get(escape.name()) {
        Some(Tag::Int(true_count)) => {
            Amount::try_from(*true_count).map_err(|_| TagError::Range {
                field: escape.name().to_owned(),
            })?
        }
        Some(_) => {
            return Err(TagError::Shape {
                expected: "an integer true count",
            });
        }
        None if written >= COUNT_CAP => {
            return Err(TagError::NoEscape {
                field: escape.name().to_owned(),
            });
        }
        None => written,
    };
    let data = match fields.get("components") {
        Some(components) => data_from(components)?,
        None => Components::new(),
    };
    Ok(Stack {
        identity: identity.clone(),
        count,
        data,
    })
}

/// Reads one stackable item back under the placeholder escape spelling.
///
/// # Errors
///
/// Returns whatever [`stack_from_with`] refuses.
pub fn stack_from(tag: &Tag) -> Result<Stack, TagError> {
    stack_from_with(tag, &EscapeField::placeholder())
}

/// Writes one fluid portion.
///
/// Section seven: the game's own optional fluid encoding, or an empty compound
/// when the portion is empty.
/// # Errors
///
/// Returns [`TagError::Range`] for a volume past the fluid kind's maximum,
/// which the field cannot carry and must not be written as the maximum.
pub fn portion_tag(value: &Portion) -> Result<Tag, TagError> {
    let mut fields = BTreeMap::new();
    if value.is_empty() {
        return Ok(Tag::Compound(fields));
    }
    let volume = i32::try_from(value.volume).map_err(|_| TagError::Range {
        field: "amount".to_owned(),
    })?;
    fields.insert("fluid".to_owned(), Tag::Text(value.fluid.clone()));
    fields.insert("amount".to_owned(), Tag::Int(volume));
    if !value.data.is_empty() {
        fields.insert("components".to_owned(), data_tag(&value.data));
    }
    Ok(Tag::Compound(fields))
}

/// Reads one fluid portion back.
///
/// # Errors
///
/// Returns [`TagError::Shape`] for a tag of the wrong shape and
/// [`TagError::Missing`] for a named fluid with no volume.
pub fn portion_from(tag: &Tag) -> Result<Portion, TagError> {
    let fields = tag.fields()?;
    let Some(fluid) = fields.get("fluid") else {
        return Ok(Portion::empty());
    };
    let Tag::Text(fluid) = fluid else {
        return Err(TagError::Shape { expected: "text" });
    };
    let Some(Tag::Int(volume)) = fields.get("amount") else {
        return Err(TagError::Missing {
            field: "amount".to_owned(),
        });
    };
    let volume = Amount::try_from(*volume).map_err(|_| TagError::Range {
        field: "amount".to_owned(),
    })?;
    let data = match fields.get("components") {
        Some(components) => data_from(components)?,
        None => Components::new(),
    };
    Ok(Portion {
        fluid: fluid.clone(),
        volume,
        data,
    })
}

/// Writes one scalar charge, as a signed 64-bit integer.
///
/// # Errors
///
/// Returns [`TagError::Range`] for a charge past the scalar kind's maximum.
pub fn charge_tag(value: Charge) -> Result<Tag, TagError> {
    let amount = i64::try_from(value.amount).map_err(|_| TagError::Range {
        field: "charge".to_owned(),
    })?;
    Ok(Tag::Long(amount))
}

/// Reads one scalar charge back.
///
/// Section seven's one tolerance: a 32-bit integer is accepted as well as a
/// 64-bit one and widened, for backwards compatibility with an older form.
///
/// # Errors
///
/// Returns [`TagError::Shape`] for any other tag and [`TagError::Range`] for a
/// negative charge, which the kind does not carry.
pub fn charge_from(tag: &Tag) -> Result<Charge, TagError> {
    let raw = match tag {
        Tag::Long(long) => *long,
        Tag::Int(int) => i64::from(*int),
        Tag::Byte(_) | Tag::Text(_) | Tag::Compound(_) | Tag::List(_) => {
            return Err(TagError::Shape {
                expected: "a 64-bit or 32-bit integer",
            });
        }
    };
    let amount = Amount::try_from(raw).map_err(|_| TagError::Range {
        field: "charge".to_owned(),
    })?;
    Ok(Charge::of(amount))
}

/// Writes a flag-set match condition, as a signed 32-bit integer.
#[must_use]
pub fn flags_tag(value: Flags) -> Tag {
    Tag::Int(value.bits().cast_signed())
}

/// Reads a flag-set match condition back.
///
/// # Errors
///
/// Returns [`TagError::Shape`] for anything that is not a 32-bit integer. Every
/// decoder but the charge one refuses a tag of the wrong shape.
pub fn flags_from(tag: &Tag) -> Result<Flags, TagError> {
    match tag {
        Tag::Int(bits) => Ok(Flags::from_bits(bits.cast_unsigned())),
        Tag::Byte(_) | Tag::Long(_) | Tag::Text(_) | Tag::Compound(_) | Tag::List(_) => {
            Err(TagError::Shape {
                expected: "a 32-bit integer",
            })
        }
    }
}

/// Writes a single-truth match condition, as a byte.
#[must_use]
pub fn truth_tag(value: Truth) -> Tag {
    Tag::Byte(value.byte())
}

/// Reads a single-truth match condition back.
///
/// # Errors
///
/// Returns [`TagError::Shape`] for anything that is not a byte and
/// [`TagError::Range`] for a byte that is neither one nor zero.
pub fn truth_from(tag: &Tag) -> Result<Truth, TagError> {
    match tag {
        Tag::Byte(1) => Ok(Truth::SET),
        Tag::Byte(0) => Ok(Truth::NONE),
        Tag::Byte(_) => Err(TagError::Range {
            field: "condition".to_owned(),
        }),
        Tag::Int(_) | Tag::Long(_) | Tag::Text(_) | Tag::Compound(_) | Tag::List(_) => {
            Err(TagError::Shape { expected: "a byte" })
        }
    }
}

/// The most alternatives one candidate set may hold.
pub const MAX_ALTERNATIVES: usize = 256;

/// One candidate value: an instance paired with the flags it is compared under.
///
/// Section five names this as one of the vocabulary's types. RVL-210 owns the
/// vocabulary and this is the serialiser for it that section two installs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate<I> {
    /// The instance.
    pub value: I,
    /// The flags it is to be compared under.
    pub under: Flags,
}

/// A per-slot candidate set: the alternatives one recipe input admits.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CandidateSet<I> {
    alternatives: Vec<Candidate<I>>,
}

impl<I> CandidateSet<I> {
    /// An empty set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            alternatives: Vec::new(),
        }
    }

    /// Adds one alternative.
    ///
    /// # Errors
    ///
    /// Returns [`TagError::TooMany`] past [`MAX_ALTERNATIVES`].
    pub fn add(&mut self, candidate: Candidate<I>) -> Result<(), TagError> {
        if self.alternatives.len() >= MAX_ALTERNATIVES {
            return Err(TagError::TooMany {
                limit: MAX_ALTERNATIVES,
            });
        }
        self.alternatives.push(candidate);
        Ok(())
    }

    /// How many alternatives the set holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.alternatives.len()
    }

    /// Whether the set admits nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.alternatives.is_empty()
    }

    /// The alternatives, in the order they were added.
    pub fn alternatives(&self) -> impl Iterator<Item = &Candidate<I>> + '_ {
        self.alternatives.iter()
    }
}

/// One of the two alternative-set serialisers section two installs.
///
/// The table files these rather than bare names, so a caller that resolved a
/// name has something it can run. A table that only remembered which names were
/// installed would answer a lookup and do nothing.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Serialiser {
    /// One instance paired with the flags it is compared under.
    Candidate,
    /// The alternatives one recipe input admits.
    Set,
}

impl Serialiser {
    /// Both, in the order section two installs them.
    pub const ALL: [Self; 2] = [Self::Candidate, Self::Set];

    /// The name it is installed under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Set => "candidate_set",
        }
    }

    /// Writes a stackable-item candidate set through this serialiser.
    ///
    /// A single candidate is written as a set of one, so both serialisers answer
    /// the same shape of question and a caller that resolved either name can run
    /// it.
    ///
    /// # Errors
    ///
    /// Returns whatever the stackable writer refuses.
    pub fn write(self, set: &CandidateSet<Stack>) -> Result<Tag, TagError> {
        match self {
            Self::Candidate => set.alternatives().next().map_or_else(
                || Ok(Tag::compound()),
                |candidate| candidate_tag(candidate, stack_tag),
            ),
            Self::Set => candidate_set_tag(set, stack_tag),
        }
    }

    /// Reads a stackable-item candidate set back through this serialiser.
    ///
    /// # Errors
    ///
    /// Returns whatever the underlying reader refuses.
    pub fn read(self, tag: &Tag) -> Result<CandidateSet<Stack>, TagError> {
        match self {
            Self::Candidate => {
                let mut set = CandidateSet::new();
                if tag == &Tag::compound() {
                    return Ok(set);
                }
                set.add(candidate_from(tag, stack_from)?)?;
                Ok(set)
            }
            Self::Set => candidate_set_from(tag, stack_from),
        }
    }
}

/// The two names section two installs into the shared serialiser table.
pub const SERIALISERS: [&str; 2] = ["candidate", "candidate_set"];

/// The shared alternative-set serialiser table.
///
/// Section two: at registry construction, at a deliberately late priority, this
/// package installs its two serialisers and contributes nothing else to the
/// table.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Serialisers {
    installed: Vec<(String, Option<Serialiser>)>,
}

impl Serialisers {
    /// An empty table.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            installed: Vec::new(),
        }
    }

    /// Installs a dependent's serialiser under a name, answering whether it was
    /// new.
    ///
    /// A dependent's serialiser is its own to run; this table only holds the
    /// name so that a second contribution under it collides.
    pub fn install(&mut self, name: &str) -> bool {
        self.file(name, None)
    }

    fn file(&mut self, name: &str, serialiser: Option<Serialiser>) -> bool {
        if self.installed.iter().any(|(held, _)| held == name) {
            return false;
        }
        self.installed.push((name.to_owned(), serialiser));
        true
    }

    /// Installs this package's two.
    ///
    /// # Errors
    ///
    /// Returns [`TagError::Shape`] naming the table when one of the two is
    /// already installed, which would mean the late-priority contribution ran
    /// twice.
    pub fn install_own(&mut self) -> Result<(), TagError> {
        for serialiser in Serialiser::ALL {
            if !self.file(serialiser.name(), Some(serialiser)) {
                return Err(TagError::Shape {
                    expected: "a table this package has not already contributed to",
                });
            }
        }
        Ok(())
    }

    /// The serialiser filed under one name, for a name this package installed.
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<Serialiser> {
        self.installed
            .iter()
            .find(|(held, _)| held == name)
            .and_then(|(_, serialiser)| *serialiser)
    }

    /// Every installed name, in installation order.
    pub fn installed(&self) -> impl Iterator<Item = &str> + '_ {
        self.installed.iter().map(|(name, _)| name.as_str())
    }

    /// How many serialisers are installed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.installed.len()
    }

    /// Whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.installed.is_empty()
    }
}

/// Writes one candidate value, given a writer for its instance.
///
/// # Errors
///
/// Returns whatever the instance writer refuses.
pub fn candidate_tag<I>(
    candidate: &Candidate<I>,
    write: impl Fn(&I) -> Result<Tag, TagError>,
) -> Result<Tag, TagError> {
    Ok(Tag::Compound(BTreeMap::from([
        ("value".to_owned(), write(&candidate.value)?),
        ("under".to_owned(), flags_tag(candidate.under)),
    ])))
}

/// Reads one candidate value back.
///
/// # Errors
///
/// Returns [`TagError::Missing`] for either field absent and whatever the
/// instance reader refuses.
pub fn candidate_from<I>(
    tag: &Tag,
    read: impl Fn(&Tag) -> Result<I, TagError>,
) -> Result<Candidate<I>, TagError> {
    let fields = tag.fields()?;
    let value = fields.get("value").ok_or_else(|| TagError::Missing {
        field: "value".to_owned(),
    })?;
    let under = fields.get("under").ok_or_else(|| TagError::Missing {
        field: "under".to_owned(),
    })?;
    Ok(Candidate {
        value: read(value)?,
        under: flags_from(under)?,
    })
}

/// Writes one candidate set.
///
/// # Errors
///
/// Returns whatever the instance writer refuses.
pub fn candidate_set_tag<I>(
    set: &CandidateSet<I>,
    write: impl Fn(&I) -> Result<Tag, TagError> + Copy,
) -> Result<Tag, TagError> {
    let written: Result<Vec<Tag>, TagError> = set
        .alternatives()
        .map(|candidate| candidate_tag(candidate, write))
        .collect();
    Ok(Tag::List(written?))
}

/// Reads one candidate set back.
///
/// # Errors
///
/// Returns [`TagError::Shape`] for anything that is not a list and
/// [`TagError::TooMany`] past [`MAX_ALTERNATIVES`].
pub fn candidate_set_from<I>(
    tag: &Tag,
    read: impl Fn(&Tag) -> Result<I, TagError> + Copy,
) -> Result<CandidateSet<I>, TagError> {
    let Tag::List(values) = tag else {
        return Err(TagError::Shape {
            expected: "a list of candidates",
        });
    };
    let mut set = CandidateSet::new();
    for value in values {
        set.add(candidate_from(value, read)?)?;
    }
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinds::{ConditionSet, Ingredient, Scalar, Stackable};
    use rvl_210_counterpart::quantity::QuantityKind;

    fn round_trip(tag: &Tag) -> Tag {
        Tag::from_json(&tag.to_json()).expect("a tag this module wrote")
    }

    #[test]
    fn every_width_round_trips_through_the_value_form() {
        let tags = [
            Tag::Byte(-1),
            Tag::Int(70_000),
            Tag::Long(i64::MAX),
            Tag::Text("a".to_owned()),
            Tag::List(vec![Tag::Byte(1), Tag::Int(2)]),
            Tag::Compound(BTreeMap::from([("a".to_owned(), Tag::Long(3))])),
        ];
        for tag in tags {
            assert_eq!(round_trip(&tag), tag);
        }
    }

    #[test]
    fn a_width_the_form_does_not_name_is_refused() {
        assert_eq!(
            Tag::from_json(&json!({ "float": 1 })),
            Err(TagError::Shape {
                expected: "one of the six widths"
            })
        );
        assert!(Tag::from_json(&json!(5)).is_err());
        assert!(Tag::from_json(&json!({ "int": 1, "long": 2 })).is_err());
    }

    #[test]
    fn a_number_outside_its_width_is_refused_rather_than_wrapped() {
        assert_eq!(
            Tag::from_json(&json!({ "byte": 200 })),
            Err(TagError::Range {
                field: "byte".to_owned()
            })
        );
        assert_eq!(
            Tag::from_json(&json!({ "int": 3_000_000_000i64 })),
            Err(TagError::Range {
                field: "int".to_owned()
            })
        );
    }

    #[test]
    fn a_tag_nesting_past_the_ceiling_is_refused_before_the_read_runs_away() {
        let mut written = json!({ "int": 1 });
        for _ in 0..=MAX_TAG_DEPTH {
            written = json!({ "list": [written] });
        }
        assert_eq!(
            Tag::from_json(&written),
            Err(TagError::TooDeep {
                limit: MAX_TAG_DEPTH
            })
        );
    }

    #[test]
    fn a_tag_at_the_depth_ceiling_is_admitted() {
        let mut written = json!({ "int": 1 });
        for _ in 1..MAX_TAG_DEPTH {
            written = json!({ "list": [written] });
        }
        assert_eq!(
            Tag::from_json(&written).map(|tag| tag.depth()),
            Ok(MAX_TAG_DEPTH)
        );
    }

    #[test]
    fn a_tag_carrying_more_entries_than_admitted_is_refused_before_it_allocates() {
        let values: Vec<JsonValue> = (0..=MAX_TAG_ENTRIES).map(|_| json!({ "int": 0 })).collect();
        assert_eq!(
            Tag::from_json(&json!({ "list": values })),
            Err(TagError::TooMany {
                limit: MAX_TAG_ENTRIES
            })
        );
    }

    #[test]
    fn a_stack_under_the_cap_round_trips_without_the_escape_field() {
        let stack = Stack::of("minecraft:stone", 12);
        let tag = stack_tag(&stack).expect("a stack inside the kind's range");
        let fields = tag.fields().expect("a compound");
        assert!(!fields.contains_key(EscapeField::placeholder().name()));
        assert_eq!(stack_from(&tag), Ok(stack));
    }

    #[test]
    fn a_stack_above_the_cap_writes_a_clamped_count_and_the_true_one() {
        let stack = Stack::of("minecraft:stone", 5_000);
        let tag = stack_tag(&stack).expect("a stack inside the kind's range");
        let fields = tag.fields().expect("a compound");
        assert_eq!(fields.get("count"), Some(&Tag::Int(99)));
        assert_eq!(
            fields.get(EscapeField::placeholder().name()),
            Some(&Tag::Int(5_000))
        );
        assert_eq!(stack_from(&tag), Ok(stack));
    }

    #[test]
    fn a_stack_of_exactly_the_cap_still_carries_the_escape_field() {
        let stack = Stack::of("minecraft:stone", COUNT_CAP);
        let tag = stack_tag(&stack).expect("a stack inside the kind's range");
        let fields = tag.fields().expect("a compound");
        assert_eq!(fields.get("count"), Some(&Tag::Int(99)));
        assert_eq!(
            fields.get(EscapeField::placeholder().name()),
            Some(&Tag::Int(99))
        );
        assert_eq!(stack_from(&tag), Ok(stack));
    }

    #[test]
    fn a_clamped_count_with_no_escape_field_is_refused_rather_than_read_as_the_cap() {
        // Defect eleven: a reader unaware of the field accepts 99 silently,
        // which is data loss rather than a decode failure.
        let escape = EscapeField::placeholder();
        let tag = Tag::Compound(BTreeMap::from([
            ("id".to_owned(), Tag::Text("minecraft:stone".to_owned())),
            ("count".to_owned(), Tag::Int(99)),
        ]));
        assert_eq!(
            stack_from(&tag),
            Err(TagError::NoEscape {
                field: escape.name().to_owned()
            })
        );
    }

    #[test]
    fn an_escape_field_of_another_spelling_is_not_read() {
        let written = stack_tag_with(
            &Stack::of("minecraft:stone", 500),
            &EscapeField::supplied(Some("other_count")).expect("a spelling"),
        )
        .expect("a stack inside the kind's range");
        assert!(stack_from(&written).is_err());
        assert_eq!(
            stack_from_with(
                &written,
                &EscapeField::supplied(Some("other_count")).expect("a spelling")
            )
            .map(|stack| stack.count),
            Ok(500)
        );
    }

    #[test]
    fn the_escape_spelling_is_refused_when_no_build_supplies_one() {
        assert_eq!(
            EscapeField::supplied(None),
            Err(TagError::NoEscapeDeclared {
                input: ESCAPE_INPUT
            })
        );
        assert_eq!(
            EscapeField::supplied(Some("   ")),
            Err(TagError::NoEscapeDeclared {
                input: ESCAPE_INPUT
            })
        );
        assert_eq!(
            EscapeField::declared(),
            EscapeField::supplied(option_env!("RVL_132_COUNTERPART_COUNT_ESCAPE_FIELD"))
        );
    }

    #[test]
    fn the_placeholder_spelling_is_this_packages_own_and_not_a_field_name() {
        let placeholder = EscapeField::placeholder();
        assert!(placeholder.name().starts_with("rvl_132_counterpart"));
        assert_ne!(placeholder.name(), "count");
        assert_ne!(placeholder.name(), "id");
    }

    #[test]
    fn a_quantity_past_the_kinds_maximum_is_refused_rather_than_written_as_it() {
        // Defect eleven is silent data loss on the reading side; writing a count
        // the field cannot carry as the field's maximum is the same loss on the
        // writing side. `Stack` has public fields and no invariant, so the
        // instance exists.
        assert_eq!(
            stack_tag(&Stack::of("minecraft:stone", 5_000_000_000)),
            Err(TagError::Range {
                field: "count".to_owned()
            })
        );
        assert_eq!(
            portion_tag(&Portion::of("minecraft:water", 5_000_000_000)),
            Err(TagError::Range {
                field: "amount".to_owned()
            })
        );
        assert_eq!(
            charge_tag(Charge::of(u64::MAX)),
            Err(TagError::Range {
                field: "charge".to_owned()
            })
        );
    }

    #[test]
    fn the_empty_stack_writes_an_empty_compound_and_reads_back_empty() {
        let tag = stack_tag(&Stack::empty()).expect("the empty stack");
        assert_eq!(tag, Tag::compound());
        assert_eq!(stack_from(&tag), Ok(Stack::empty()));
    }

    #[test]
    fn the_item_decoder_refuses_anything_that_is_not_a_compound() {
        assert_eq!(
            stack_from(&Tag::Int(4)),
            Err(TagError::Shape {
                expected: "a compound"
            })
        );
        assert_eq!(
            stack_from(&Tag::Text("minecraft:stone".to_owned())),
            Err(TagError::Shape {
                expected: "a compound"
            })
        );
    }

    #[test]
    fn a_stacks_data_round_trips_through_the_form() {
        let data = Components::from([
            ("name".to_owned(), Value::Text("axe".to_owned())),
            ("wear".to_owned(), Value::Whole(7)),
            ("sharp".to_owned(), Value::Truth(true)),
            (
                "trim".to_owned(),
                Value::Sequence(vec![Value::Whole(1), Value::Whole(2)]),
            ),
        ]);
        let stack = Stack::of("minecraft:axe", 1).carrying(data);
        assert_eq!(
            stack_from(&stack_tag(&stack).expect("a stack inside the kind's range")),
            Ok(stack)
        );
    }

    #[test]
    fn a_named_item_with_no_count_is_refused() {
        let tag = Tag::Compound(BTreeMap::from([(
            "id".to_owned(),
            Tag::Text("minecraft:stone".to_owned()),
        )]));
        assert_eq!(
            stack_from(&tag),
            Err(TagError::Missing {
                field: "count".to_owned()
            })
        );
    }

    #[test]
    fn the_empty_portion_writes_an_empty_compound_and_reads_back_empty() {
        let tag = portion_tag(&Portion::empty()).expect("the empty portion");
        assert_eq!(tag, Tag::compound());
        assert_eq!(portion_from(&tag), Ok(Portion::empty()));
    }

    #[test]
    fn a_portion_round_trips_with_and_without_data() {
        let plain = Portion::of("minecraft:water", 1_000);
        assert_eq!(
            portion_from(&portion_tag(&plain).expect("a portion inside the kind's range")),
            Ok(plain)
        );
        let detailed = Portion::of("minecraft:water", 1_000)
            .carrying(Components::from([("tint".to_owned(), Value::Whole(4))]));
        assert_eq!(
            portion_from(&portion_tag(&detailed).expect("a portion inside the kind's range")),
            Ok(detailed)
        );
    }

    #[test]
    fn the_fluid_decoder_refuses_a_tag_of_the_wrong_shape() {
        assert!(portion_from(&Tag::Long(1)).is_err());
        assert_eq!(
            portion_from(&Tag::Compound(BTreeMap::from([(
                "fluid".to_owned(),
                Tag::Int(1)
            )]))),
            Err(TagError::Shape { expected: "text" })
        );
    }

    #[test]
    fn a_charge_round_trips_and_the_wide_range_survives() {
        let charge = Charge::of(1 + (1 << 40));
        assert_eq!(
            charge_from(&charge_tag(charge).expect("a charge inside the kind's range")),
            Ok(charge)
        );
    }

    #[test]
    fn the_charge_decoder_widens_the_older_thirty_two_bit_form() {
        assert_eq!(charge_from(&Tag::Int(4_000)), Ok(Charge::of(4_000)));
        assert_eq!(charge_from(&Tag::Long(4_000)), Ok(Charge::of(4_000)));
    }

    #[test]
    fn the_charge_decoder_refuses_every_other_shape_and_a_negative_value() {
        assert!(charge_from(&Tag::Text("4".to_owned())).is_err());
        assert!(charge_from(&Tag::Byte(4)).is_err());
        assert_eq!(
            charge_from(&Tag::Long(-1)),
            Err(TagError::Range {
                field: "charge".to_owned()
            })
        );
    }

    #[test]
    fn a_flag_condition_round_trips_as_a_thirty_two_bit_integer() {
        let flags = Flags::slot(Stack::IDENTITY).combine(Flags::slot(Stack::DATA));
        let tag = flags_tag(flags);
        assert!(matches!(tag, Tag::Int(_)));
        assert_eq!(flags_from(&tag), Ok(flags));
        assert_eq!(flags_from(&flags_tag(Flags::NONE)), Ok(Flags::NONE));
    }

    #[test]
    fn the_top_flag_bit_survives_the_signed_round_trip() {
        let flags = Flags::from_bits(u32::MAX);
        assert_eq!(flags_tag(flags), Tag::Int(-1));
        assert_eq!(flags_from(&Tag::Int(-1)), Ok(flags));
    }

    #[test]
    fn a_condition_decoder_refuses_a_tag_of_the_wrong_shape() {
        assert!(flags_from(&Tag::Long(1)).is_err());
        assert!(truth_from(&Tag::Int(1)).is_err());
    }

    #[test]
    fn a_truth_condition_round_trips_as_a_byte_of_one_or_zero() {
        assert_eq!(truth_tag(Truth::SET), Tag::Byte(1));
        assert_eq!(truth_tag(Truth::NONE), Tag::Byte(0));
        assert_eq!(truth_from(&Tag::Byte(1)), Ok(Truth::SET));
        assert_eq!(truth_from(&Tag::Byte(0)), Ok(Truth::NONE));
        assert_eq!(
            truth_from(&Tag::Byte(2)),
            Err(TagError::Range {
                field: "condition".to_owned()
            })
        );
    }

    #[test]
    fn the_kinds_write_and_read_through_the_vocabularys_own_methods() {
        let items = Stackable::default();
        let stack = Stack::of("minecraft:stone", 200);
        assert_eq!(items.read(&items.write(&stack)), Ok(stack));
        assert_eq!(
            Scalar.read(&Scalar.write(&Charge::of(9))),
            Ok(Charge::of(9))
        );
        assert!(Scalar.read(&json!("nine")).is_err());
        assert_eq!(items.name(), crate::kinds::STACKABLE);
    }

    #[test]
    fn a_candidate_round_trips_with_its_flags() {
        let candidate = Candidate {
            value: Stack::of("minecraft:stone", 3),
            under: Flags::slot(Stack::IDENTITY),
        };
        let tag = candidate_tag(&candidate, stack_tag).expect("a candidate in range");
        assert_eq!(candidate_from(&tag, stack_from), Ok(candidate));
    }

    #[test]
    fn a_candidate_missing_either_field_is_refused() {
        let only_value = Tag::Compound(BTreeMap::from([(
            "value".to_owned(),
            stack_tag(&Stack::empty()).expect("the empty stack"),
        )]));
        assert_eq!(
            candidate_from(&only_value, stack_from),
            Err(TagError::Missing {
                field: "under".to_owned()
            })
        );
    }

    #[test]
    fn a_candidate_set_round_trips_in_order() {
        let mut set = CandidateSet::new();
        for count in 1..4 {
            set.add(Candidate {
                value: Stack::of("minecraft:stone", count),
                under: Flags::slot(Stack::COUNT),
            })
            .expect("inside the ceiling");
        }
        let tag = candidate_set_tag(&set, stack_tag).expect("a set in range");
        let read = candidate_set_from(&tag, stack_from).expect("a set this module wrote");
        assert_eq!(read.len(), 3);
        assert_eq!(read, set);
    }

    #[test]
    fn a_candidate_set_has_an_enforced_ceiling_at_both_ends() {
        let mut set = CandidateSet::new();
        for _ in 0..MAX_ALTERNATIVES {
            set.add(Candidate {
                value: Charge::of(1),
                under: Flags::NONE,
            })
            .expect("inside the ceiling");
        }
        assert_eq!(
            set.add(Candidate {
                value: Charge::of(1),
                under: Flags::NONE
            }),
            Err(TagError::TooMany {
                limit: MAX_ALTERNATIVES
            })
        );
        let written = Tag::List(
            (0..=MAX_ALTERNATIVES)
                .map(|_| {
                    candidate_tag(
                        &Candidate {
                            value: Charge::of(1),
                            under: Flags::NONE,
                        },
                        |charge| charge_tag(*charge),
                    )
                    .expect("a charge in range")
                })
                .collect(),
        );
        assert_eq!(
            candidate_set_from(&written, charge_from),
            Err(TagError::TooMany {
                limit: MAX_ALTERNATIVES
            })
        );
    }

    #[test]
    fn an_empty_candidate_set_reports_itself_empty() {
        let set: CandidateSet<Charge> = CandidateSet::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn a_candidate_set_that_is_not_a_list_is_refused() {
        assert!(candidate_set_from(&Tag::compound(), charge_from).is_err());
    }

    #[test]
    fn the_serialiser_table_takes_exactly_this_packages_two_contributions() {
        let mut table = Serialisers::new();
        assert!(table.is_empty());
        table.install_own().expect("a fresh table");
        assert_eq!(table.len(), 2);
        assert_eq!(table.installed().collect::<Vec<&str>>(), SERIALISERS);
        assert!(table.install_own().is_err());
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn a_resolved_serialiser_name_hands_back_something_that_runs() {
        let mut table = Serialisers::new();
        table.install_own().expect("a fresh table");
        let mut set = CandidateSet::new();
        for count in 1..3 {
            set.add(Candidate {
                value: Stack::of("minecraft:stone", count),
                under: Flags::slot(Stack::COUNT),
            })
            .expect("inside the ceiling");
        }
        let serialiser = table.resolve("candidate_set").expect("an installed name");
        assert_eq!(serialiser, Serialiser::Set);
        assert_eq!(
            serialiser.read(&serialiser.write(&set).expect("a set in range")),
            Ok(set.clone())
        );

        let one = table.resolve("candidate").expect("an installed name");
        let read = one
            .read(&one.write(&set).expect("a set in range"))
            .expect("a candidate this table wrote");
        assert_eq!(read.len(), 1);
        assert_eq!(
            read.alternatives().next().map(|held| held.value.count),
            Some(1)
        );
    }

    #[test]
    fn the_single_candidate_serialiser_round_trips_an_empty_set() {
        let serialiser = Serialiser::Candidate;
        let empty: CandidateSet<Stack> = CandidateSet::new();
        assert_eq!(serialiser.write(&empty), Ok(Tag::compound()));
        assert_eq!(serialiser.read(&Tag::compound()), Ok(empty));
    }

    #[test]
    fn a_name_the_table_does_not_hold_resolves_to_nothing() {
        let mut table = Serialisers::new();
        table.install_own().expect("a fresh table");
        assert_eq!(table.resolve("other:set"), None);
        table.install("other:set");
        assert_eq!(table.resolve("other:set"), None);
        assert_eq!(table.len(), 3);
    }

    #[test]
    fn a_dependents_serialiser_does_not_collide_with_this_packages_two() {
        let mut table = Serialisers::new();
        table.install_own().expect("a fresh table");
        assert!(table.install("other:set"));
        assert!(!table.install("other:set"));
        assert_eq!(table.len(), 3);
    }

    #[test]
    fn a_data_map_past_the_component_ceiling_is_refused_at_the_read() {
        let fields: BTreeMap<String, Tag> = (0..=data::MAX_COMPONENTS)
            .map(|index| (format!("k{index}"), Tag::Long(0)))
            .collect();
        assert_eq!(
            data_from(&Tag::Compound(fields)),
            Err(TagError::TooMany {
                limit: data::MAX_COMPONENTS
            })
        );
    }

    #[test]
    fn a_component_value_nesting_past_its_ceiling_is_refused_at_the_read() {
        let mut value = Value::Whole(0);
        for _ in 0..=data::MAX_VALUE_DEPTH {
            value = Value::Sequence(vec![value]);
        }
        let written = Tag::Compound(BTreeMap::from([("a".to_owned(), value_tag(&value))]));
        assert_eq!(
            data_from(&written),
            Err(TagError::TooDeep {
                limit: data::MAX_VALUE_DEPTH
            })
        );
    }

    #[test]
    fn a_component_value_at_its_depth_ceiling_reads_back_whole() {
        let mut value = Value::Whole(0);
        for _ in 1..data::MAX_VALUE_DEPTH {
            value = Value::Sequence(vec![value]);
        }
        assert_eq!(value.depth(), data::MAX_VALUE_DEPTH);
        let written = Tag::Compound(BTreeMap::from([("a".to_owned(), value_tag(&value))]));
        assert_eq!(
            data_from(&written).map(|read| read.get("a").cloned()),
            Ok(Some(value))
        );
    }

    #[test]
    fn a_value_whose_shape_and_body_disagree_is_refused() {
        let written = Tag::Compound(BTreeMap::from([(
            "a".to_owned(),
            Tag::Compound(BTreeMap::from([("truth".to_owned(), Tag::Long(1))])),
        )]));
        assert_eq!(
            data_from(&written),
            Err(TagError::Shape {
                expected: "a value whose shape and body agree"
            })
        );
    }

    #[test]
    fn a_decimal_component_survives_the_round_trip_by_its_bits() {
        let data = Components::from([
            ("rate".to_owned(), Value::Decimal(0.5)),
            ("absent".to_owned(), Value::Decimal(f64::NAN)),
        ]);
        let stack = Stack::of("minecraft:axe", 1).carrying(data.clone());
        let read = stack_from(&stack_tag(&stack).expect("a stack inside the kind's range"))
            .expect("a stack this module wrote");
        assert_eq!(read.data, data);
    }

    #[test]
    fn refusals_say_what_was_wrong() {
        assert!(
            TagError::NoEscape {
                field: "f".to_owned()
            }
            .to_string()
            .contains("true count is lost")
        );
        assert!(
            TagError::NoEscapeDeclared {
                input: ESCAPE_INPUT
            }
            .to_string()
            .contains(ESCAPE_INPUT)
        );
        assert!(
            TagError::Data(data::Refused::TooDeep { limit: 1 })
                .to_string()
                .contains("nests past")
        );
        assert!(
            TagError::Missing {
                field: "f".to_owned()
            }
            .to_string()
            .contains("names no f")
        );
    }
}
