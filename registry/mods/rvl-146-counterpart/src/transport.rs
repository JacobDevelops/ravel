//! The argument wire format of §5.1, and the eight argument types it carries.
//!
//! Two peers must derive this format identically, so it is behaviour rather than
//! an implementation choice and this module reproduces its shape: an action name
//! as a length-prefixed string, a boolean saying whether an argument vector
//! follows, and when it does a count followed by one type tag and one encoding
//! per argument.
//!
//! **The numbering is not invented here.** §14 makes the assignment of number to
//! type compatibility-required and withholds it, so [`TagNumbering::declared`]
//! reads it from a build-time input and [`TagNumbering::placeholder`] carries
//! this package's own, which agrees with no original. In the original the
//! numbering is a language enumeration's declaration order, so reordering a
//! declaration silently changes the wire; here it is data a build supplies and a
//! reorder of [`ArgumentKind::ALL`] cannot move a tag.
//!
//! # The changes declared here
//!
//! Every one of them is §13's, and each is about the same thing: this decoder
//! reads bytes an untrusted peer sent.
//!
//! - **The count is a bounded variable-length quantity.** The original writes it
//!   as one signed byte, so a vector longer than a hundred and twenty-seven
//!   wraps negative and the reader allocates an array of that length. Here the
//!   count is a variable-length quantity refused past [`MAX_ARGUMENTS`] on both
//!   sides, so an over-long vector is refused where it is written as well as
//!   where it is read.
//! - **The tag is range-checked.** The original indexes its tag table by the raw
//!   byte, so an out-of-range tag raises an index error and the decoder's own
//!   rejection branch is unreachable. [`TagNumbering::kind`] answers absent and
//!   the packet is refused.
//! - **The decoded size is bounded, not just the element count.** Every text,
//!   stack and compound field carries a bound, and a body past
//!   [`MAX_BODY_BYTES`] is refused before a field is read, so a hostile peer
//!   cannot buy a large decode cheaply.
//! - **The variable-length quantity is zigzag over the signed value.** §5.1 says
//!   an integer and a long travel as a variable-length quantity and does not say
//!   which one, and §14 does not record it, so this is a framing this package
//!   picks rather than one it obtained: seven bits per byte, low group first, the
//!   signed value zigzagged so a small negative costs one byte rather than ten.
//!   A peer that frames its quantities another way disagrees with this on those
//!   two types, and the framing is a constant of this module rather than
//!   something a caller can vary. It is named here so that it arrives as a known
//!   gap rather than a surprise, alongside the numbering §14 does record.
//! - **An absent compound is the absent argument.** The original's compound arm
//!   stores whatever the buffer read yields, including nothing, so a handler
//!   expecting a tag can receive none. There is no such state here: a compound
//!   argument carries a compound.
//!
//! # Declarable only
//!
//! The stack and compound arms carry the game's own stream form, which is the
//! host's and which Ravel publishes no reader for. They are carried here as
//! bounded opaque bytes: this package frames them, counts them and refuses them,
//! and what the bytes mean is the host's.

use std::fmt;

/// The most arguments one action may carry, written and read.
///
/// The original's count is one signed byte, so a hundred and twenty-seven is the
/// most it can carry and every legal packet an original peer writes is inside
/// this bound. Bounding lower would refuse a packet the original happily sends;
/// §13 asks for a count that is bounded rather than one that is narrower.
pub const MAX_ARGUMENTS: usize = 127;

/// The most characters a text argument may carry.
///
/// The original's own bound, kept because it is the writer's contract.
pub const MAX_TEXT_CHARS: usize = 1024;

/// The most bytes one opaque argument may carry.
///
/// A stack and a compound arrive from a peer in the game's own stream form. The
/// upgrade path is a larger constant, or a reader that streams rather than
/// buffers.
pub const MAX_OPAQUE_BYTES: usize = 32 * 1024;

/// The most bytes one encoded action may occupy.
pub const MAX_BODY_BYTES: usize = 64 * 1024;

/// The eight argument types the wire carries.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ArgumentKind {
    /// No value at all.
    Absent,
    /// A thirty-two-bit signed integer, as a variable-length quantity.
    Integer,
    /// A sixty-four-bit signed integer, as a variable-length quantity.
    Long,
    /// A sixteen-bit signed integer, as two fixed bytes.
    Short,
    /// A boolean.
    Boolean,
    /// Text, bounded at [`MAX_TEXT_CHARS`].
    Text,
    /// An item stack, in the game's own stream form.
    Stack,
    /// A compound tag, in the game's own stream form.
    Compound,
}

impl ArgumentKind {
    /// Every kind. This is a listing order and never a wire order: what a kind
    /// travels as is [`TagNumbering`]'s answer and nothing else.
    pub const ALL: [Self; 8] = [
        Self::Absent,
        Self::Integer,
        Self::Long,
        Self::Short,
        Self::Boolean,
        Self::Text,
        Self::Stack,
        Self::Compound,
    ];

    /// Returns the kind's name, which is what a build-time numbering spells.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Integer => "integer",
            Self::Long => "long",
            Self::Short => "short",
            Self::Boolean => "boolean",
            Self::Text => "text",
            Self::Stack => "stack",
            Self::Compound => "compound",
        }
    }

    /// Returns the kind a name spells, or absent for a name naming none.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == name)
    }

    const fn ordinal(self) -> usize {
        match self {
            Self::Absent => 0,
            Self::Integer => 1,
            Self::Long => 2,
            Self::Short => 3,
            Self::Boolean => 4,
            Self::Text => 5,
            Self::Stack => 6,
            Self::Compound => 7,
        }
    }
}

impl fmt::Display for ArgumentKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One argument, in the eight shapes the wire carries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Argument {
    /// No value at all.
    Absent,
    /// A thirty-two-bit signed integer.
    Integer(i32),
    /// A sixty-four-bit signed integer.
    Long(i64),
    /// A sixteen-bit signed integer.
    Short(i16),
    /// A boolean.
    Boolean(bool),
    /// Text.
    Text(String),
    /// An item stack, as the host's own bytes.
    Stack(Vec<u8>),
    /// A compound tag, as the host's own bytes.
    Compound(Vec<u8>),
}

impl Argument {
    /// Returns which of the eight kinds this is.
    #[must_use]
    pub const fn kind(&self) -> ArgumentKind {
        match self {
            Self::Absent => ArgumentKind::Absent,
            Self::Integer(_) => ArgumentKind::Integer,
            Self::Long(_) => ArgumentKind::Long,
            Self::Short(_) => ArgumentKind::Short,
            Self::Boolean(_) => ArgumentKind::Boolean,
            Self::Text(_) => ArgumentKind::Text,
            Self::Stack(_) => ArgumentKind::Stack,
            Self::Compound(_) => ArgumentKind::Compound,
        }
    }
}

/// Why a build-time numbering was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NumberingUnavailable {
    /// The build supplied nothing.
    NotSupplied,
    /// An entry is not a `name=number` pair, or the number is not a byte.
    Malformed(String),
    /// An entry names no argument kind.
    UnknownKind(String),
    /// A kind is numbered twice.
    DuplicateKind(ArgumentKind),
    /// Two kinds carry one number, which would make a peer unable to tell them
    /// apart.
    DuplicateNumber(u8),
    /// A kind is not numbered at all.
    Missing(ArgumentKind),
}

impl fmt::Display for NumberingUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSupplied => {
                formatter.write_str("the build supplied no argument tag numbering")
            }
            Self::Malformed(entry) => write!(formatter, "`{entry}` is not a kind and a byte"),
            Self::UnknownKind(name) => write!(formatter, "`{name}` names no argument kind"),
            Self::DuplicateKind(kind) => write!(formatter, "`{kind}` is numbered twice"),
            Self::DuplicateNumber(tag) => write!(formatter, "two kinds carry the tag {tag}"),
            Self::Missing(kind) => write!(formatter, "`{kind}` carries no tag"),
        }
    }
}

impl std::error::Error for NumberingUnavailable {}

/// Which one-byte tag each argument kind travels as.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TagNumbering {
    tags: [u8; 8],
}

impl Default for TagNumbering {
    fn default() -> Self {
        Self::placeholder()
    }
}

impl TagNumbering {
    /// The numbering the tests pin the format's **shape** with.
    ///
    /// This package's own. A build that means to talk to an original supplies
    /// the original's; a build that does not can only talk to itself, which is
    /// what a placeholder means here.
    #[must_use]
    pub const fn placeholder() -> Self {
        Self {
            tags: [0, 1, 2, 3, 4, 5, 6, 7],
        }
    }

    /// Reads the numbering from a build-time input.
    ///
    /// The input is a comma-separated list of `kind=number` pairs naming all
    /// eight kinds, as in `absent=0,integer=1,...`.
    ///
    /// # Errors
    ///
    /// Returns [`NumberingUnavailable::NotSupplied`] when the build supplied
    /// nothing, and what [`TagNumbering::parse`] returns otherwise.
    pub fn declared() -> Result<Self, NumberingUnavailable> {
        option_env!("RVL_146_COUNTERPART_ARGUMENT_TAGS")
            .ok_or(NumberingUnavailable::NotSupplied)
            .and_then(Self::parse)
    }

    /// Reads a numbering from its written form.
    ///
    /// # Errors
    ///
    /// Returns [`NumberingUnavailable::Malformed`] for an entry that is not a
    /// kind and a byte, [`NumberingUnavailable::UnknownKind`] for a name naming
    /// no kind, [`NumberingUnavailable::DuplicateKind`] for a kind numbered
    /// twice, [`NumberingUnavailable::DuplicateNumber`] for one number carrying
    /// two kinds, and [`NumberingUnavailable::Missing`] for a kind the input
    /// leaves out. All eight are required, because a numbering that defaults an
    /// omission is a numbering two peers can disagree about silently.
    pub fn parse(written: &str) -> Result<Self, NumberingUnavailable> {
        let mut tags = [None; 8];
        for entry in written.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            let (name, number) = entry
                .split_once('=')
                .ok_or_else(|| NumberingUnavailable::Malformed(entry.to_owned()))?;
            let kind = ArgumentKind::named(name.trim())
                .ok_or_else(|| NumberingUnavailable::UnknownKind(name.trim().to_owned()))?;
            let tag: u8 = number
                .trim()
                .parse()
                .map_err(|_| NumberingUnavailable::Malformed(entry.to_owned()))?;
            if tags[kind.ordinal()].is_some() {
                return Err(NumberingUnavailable::DuplicateKind(kind));
            }
            if ArgumentKind::ALL
                .into_iter()
                .any(|other| tags[other.ordinal()] == Some(tag))
            {
                return Err(NumberingUnavailable::DuplicateNumber(tag));
            }
            tags[kind.ordinal()] = Some(tag);
        }
        let mut settled = [0u8; 8];
        for kind in ArgumentKind::ALL {
            settled[kind.ordinal()] =
                tags[kind.ordinal()].ok_or(NumberingUnavailable::Missing(kind))?;
        }
        Ok(Self { tags: settled })
    }

    /// Returns the tag a kind travels as.
    #[must_use]
    pub const fn number(&self, kind: ArgumentKind) -> u8 {
        self.tags[kind.ordinal()]
    }

    /// Returns the kind a tag names, or absent for a tag naming none.
    ///
    /// This is the range check the original does not have: a tag outside the
    /// numbering is answered rather than used as an index.
    #[must_use]
    pub fn kind(&self, tag: u8) -> Option<ArgumentKind> {
        ArgumentKind::ALL
            .into_iter()
            .find(|kind| self.tags[kind.ordinal()] == tag)
    }
}

/// Why an action could not be written.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WriteRefused {
    /// The action name is empty, which nothing could look up.
    EmptyName,
    /// More arguments than [`MAX_ARGUMENTS`].
    TooManyArguments(usize),
    /// A text argument longer than [`MAX_TEXT_CHARS`].
    TextTooLong(usize),
    /// An opaque argument longer than [`MAX_OPAQUE_BYTES`].
    OpaqueTooLong(usize),
    /// The whole action would be longer than [`MAX_BODY_BYTES`].
    BodyTooLong(usize),
}

impl fmt::Display for WriteRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => formatter.write_str("an action with no name names nothing"),
            Self::TooManyArguments(count) => {
                write!(formatter, "{count} arguments is past {MAX_ARGUMENTS}")
            }
            Self::TextTooLong(length) => {
                write!(formatter, "{length} characters is past {MAX_TEXT_CHARS}")
            }
            Self::OpaqueTooLong(length) => {
                write!(formatter, "{length} bytes is past {MAX_OPAQUE_BYTES}")
            }
            Self::BodyTooLong(length) => {
                write!(formatter, "{length} bytes is past {MAX_BODY_BYTES}")
            }
        }
    }
}

impl std::error::Error for WriteRefused {}

/// Why an action could not be read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadRefused {
    /// The buffer is longer than [`MAX_BODY_BYTES`], refused before a field is
    /// read.
    BodyTooLong(usize),
    /// The buffer ended inside a field.
    Truncated,
    /// A variable-length quantity does not end, or does not fit its width.
    Overlong,
    /// The declared count is past [`MAX_ARGUMENTS`].
    TooManyArguments(u32),
    /// A text field's bytes are not UTF-8.
    NotUtf8,
    /// A text field is longer than [`MAX_TEXT_CHARS`].
    TextTooLong(usize),
    /// An opaque field is longer than [`MAX_OPAQUE_BYTES`].
    OpaqueTooLong(u32),
    /// A boolean byte was neither zero nor one.
    NotABool(u8),
    /// A tag lies outside the numbering.
    UnknownTag(u8),
    /// The action name is empty, which the writer refuses at the other end.
    EmptyName,
    /// The action ended with bytes nothing accounted for.
    TrailingBytes(usize),
}

impl fmt::Display for ReadRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BodyTooLong(length) => {
                write!(formatter, "{length} bytes is past {MAX_BODY_BYTES}")
            }
            Self::Truncated => formatter.write_str("the buffer ended inside a field"),
            Self::Overlong => formatter.write_str("a length does not end or does not fit"),
            Self::TooManyArguments(count) => {
                write!(formatter, "{count} arguments is past {MAX_ARGUMENTS}")
            }
            Self::NotUtf8 => formatter.write_str("a text field is not UTF-8"),
            Self::TextTooLong(length) => {
                write!(formatter, "{length} characters is past {MAX_TEXT_CHARS}")
            }
            Self::OpaqueTooLong(length) => {
                write!(formatter, "{length} bytes is past {MAX_OPAQUE_BYTES}")
            }
            Self::NotABool(byte) => write!(formatter, "{byte} is neither zero nor one"),
            Self::UnknownTag(tag) => write!(formatter, "{tag} names no argument kind"),
            Self::EmptyName => formatter.write_str("an action with no name names nothing"),
            Self::TrailingBytes(unread) => write!(formatter, "{unread} bytes were left unread"),
        }
    }
}

impl std::error::Error for ReadRefused {}

/// A named action and the arguments it arrived with.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Action {
    /// The action's name, which a screen or a menu looks up.
    pub name: String,
    /// The arguments, absent when the sender wrote none at all. An empty vector
    /// and no vector are different states on the wire and stay different here.
    pub arguments: Option<Vec<Argument>>,
}

impl Action {
    /// An action carrying no argument vector at all.
    #[must_use]
    pub fn bare(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            arguments: None,
        }
    }

    /// An action carrying a vector.
    #[must_use]
    pub fn carrying(name: &str, arguments: Vec<Argument>) -> Self {
        Self {
            name: name.to_owned(),
            arguments: Some(arguments),
        }
    }

    /// Returns how many arguments arrived, and zero when none did.
    #[must_use]
    pub fn count(&self) -> usize {
        self.arguments.as_ref().map_or(0, Vec::len)
    }

    /// Returns the argument at `index`, or absent when there is none.
    ///
    /// This is the whole of the reading surface. The original offers a strict
    /// read that throws, a lenient one, and a blind cast on both; the cast is
    /// what lets a wrong-typed vector reach a handler, so there is no cast here
    /// and no throwing read either. A caller that wants a particular type
    /// matches on [`Argument`], and a caller that declared its shape had it
    /// checked before dispatch.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Argument> {
        self.arguments.as_ref().and_then(|args| args.get(index))
    }
}

fn write_varint(into: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = u8::try_from(value & 0x7f).expect("seven bits are a byte");
        value >>= 7;
        if value == 0 {
            into.push(byte);
            return;
        }
        into.push(byte | 0x80);
    }
}

const fn zigzag(value: i64) -> u64 {
    (value.cast_unsigned() << 1) ^ (value >> 63).cast_unsigned()
}

const fn unzigzag(value: u64) -> i64 {
    (value >> 1).cast_signed() ^ -(value & 1).cast_signed()
}

/// Reads a buffer field by field, refusing rather than saturating.
struct Reader<'bytes> {
    bytes: &'bytes [u8],
    at: usize,
}

impl<'bytes> Reader<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    fn byte(&mut self) -> Result<u8, ReadRefused> {
        let byte = *self.bytes.get(self.at).ok_or(ReadRefused::Truncated)?;
        self.at += 1;
        Ok(byte)
    }

    fn varint(&mut self) -> Result<u64, ReadRefused> {
        let mut value: u64 = 0;
        for shift in 0..10 {
            let byte = self.byte()?;
            let carried = u64::from(byte & 0x7f);
            value |= carried
                .checked_shl(shift * 7)
                .filter(|shifted| (shifted >> (shift * 7)) == carried)
                .ok_or(ReadRefused::Overlong)?;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(ReadRefused::Overlong)
    }

    fn slice(&mut self, length: usize) -> Result<&'bytes [u8], ReadRefused> {
        let end = self.at.checked_add(length).ok_or(ReadRefused::Overlong)?;
        let taken = self.bytes.get(self.at..end).ok_or(ReadRefused::Truncated)?;
        self.at = end;
        Ok(taken)
    }

    fn text(&mut self) -> Result<String, ReadRefused> {
        let length = usize::try_from(self.varint()?).map_err(|_| ReadRefused::Overlong)?;
        if length > MAX_TEXT_CHARS * 4 {
            return Err(ReadRefused::TextTooLong(length));
        }
        let taken = self.slice(length)?;
        let text = std::str::from_utf8(taken).map_err(|_| ReadRefused::NotUtf8)?;
        let characters = text.chars().count();
        if characters > MAX_TEXT_CHARS {
            return Err(ReadRefused::TextTooLong(characters));
        }
        Ok(text.to_owned())
    }

    fn opaque(&mut self) -> Result<Vec<u8>, ReadRefused> {
        let length = u32::try_from(self.varint()?).map_err(|_| ReadRefused::Overlong)?;
        let length_index = usize::try_from(length).map_err(|_| ReadRefused::Overlong)?;
        if length_index > MAX_OPAQUE_BYTES {
            return Err(ReadRefused::OpaqueTooLong(length));
        }
        Ok(self.slice(length_index)?.to_vec())
    }

    fn boolean(&mut self) -> Result<bool, ReadRefused> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(ReadRefused::NotABool(other)),
        }
    }

    const fn unread(&self) -> usize {
        self.bytes.len() - self.at
    }
}

/// Writes an action in the wire shape of §5.1.
///
/// # Errors
///
/// Returns [`WriteRefused`]: an empty name, a vector past [`MAX_ARGUMENTS`], a
/// text past [`MAX_TEXT_CHARS`], an opaque field past [`MAX_OPAQUE_BYTES`], or a
/// body past [`MAX_BODY_BYTES`]. Every bound is applied here as well as at the
/// reader, which is the half the original has nowhere.
pub fn write(action: &Action, numbering: &TagNumbering) -> Result<Vec<u8>, WriteRefused> {
    if action.name.is_empty() {
        return Err(WriteRefused::EmptyName);
    }
    let mut bytes = Vec::new();
    let name = action.name.as_bytes();
    let name_length =
        u64::try_from(name.len()).map_err(|_| WriteRefused::BodyTooLong(name.len()))?;
    write_varint(&mut bytes, name_length);
    bytes.extend_from_slice(name);
    match action.arguments.as_ref() {
        None => bytes.push(0),
        Some(arguments) => {
            if arguments.len() > MAX_ARGUMENTS {
                return Err(WriteRefused::TooManyArguments(arguments.len()));
            }
            bytes.push(1);
            let count = u64::try_from(arguments.len())
                .map_err(|_| WriteRefused::TooManyArguments(arguments.len()))?;
            write_varint(&mut bytes, count);
            for argument in arguments {
                bytes.push(numbering.number(argument.kind()));
                match argument {
                    Argument::Absent => {}
                    Argument::Integer(value) => {
                        write_varint(&mut bytes, zigzag(i64::from(*value)));
                    }
                    Argument::Long(value) => write_varint(&mut bytes, zigzag(*value)),
                    Argument::Short(value) => bytes.extend_from_slice(&value.to_be_bytes()),
                    Argument::Boolean(value) => bytes.push(u8::from(*value)),
                    Argument::Text(value) => {
                        let characters = value.chars().count();
                        if characters > MAX_TEXT_CHARS {
                            return Err(WriteRefused::TextTooLong(characters));
                        }
                        let length = u64::try_from(value.len())
                            .map_err(|_| WriteRefused::TextTooLong(characters))?;
                        write_varint(&mut bytes, length);
                        bytes.extend_from_slice(value.as_bytes());
                    }
                    Argument::Stack(value) | Argument::Compound(value) => {
                        if value.len() > MAX_OPAQUE_BYTES {
                            return Err(WriteRefused::OpaqueTooLong(value.len()));
                        }
                        let length = u64::try_from(value.len())
                            .map_err(|_| WriteRefused::OpaqueTooLong(value.len()))?;
                        write_varint(&mut bytes, length);
                        bytes.extend_from_slice(value);
                    }
                }
            }
        }
    }
    if bytes.len() > MAX_BODY_BYTES {
        return Err(WriteRefused::BodyTooLong(bytes.len()));
    }
    Ok(bytes)
}

/// Reads an action written by [`write`].
///
/// # Errors
///
/// Returns [`ReadRefused`]. Every bound the writer applies is applied again
/// here, the empty name among them, because the bytes arrive from a peer and the
/// peer's writer is not this one.
pub fn read(bytes: &[u8], numbering: &TagNumbering) -> Result<Action, ReadRefused> {
    if bytes.len() > MAX_BODY_BYTES {
        return Err(ReadRefused::BodyTooLong(bytes.len()));
    }
    let mut reader = Reader::new(bytes);
    let name = reader.text()?;
    if name.is_empty() {
        return Err(ReadRefused::EmptyName);
    }
    let arguments = if reader.boolean()? {
        let count = u32::try_from(reader.varint()?).map_err(|_| ReadRefused::Overlong)?;
        let count_index = usize::try_from(count).map_err(|_| ReadRefused::Overlong)?;
        if count_index > MAX_ARGUMENTS {
            return Err(ReadRefused::TooManyArguments(count));
        }
        let mut arguments = Vec::with_capacity(count_index);
        for _ in 0..count {
            let tag = reader.byte()?;
            let kind = numbering.kind(tag).ok_or(ReadRefused::UnknownTag(tag))?;
            arguments.push(match kind {
                ArgumentKind::Absent => Argument::Absent,
                ArgumentKind::Integer => {
                    let value = unzigzag(reader.varint()?);
                    Argument::Integer(i32::try_from(value).map_err(|_| ReadRefused::Overlong)?)
                }
                ArgumentKind::Long => Argument::Long(unzigzag(reader.varint()?)),
                ArgumentKind::Short => {
                    let taken = reader.slice(2)?;
                    Argument::Short(i16::from_be_bytes([taken[0], taken[1]]))
                }
                ArgumentKind::Boolean => Argument::Boolean(reader.boolean()?),
                ArgumentKind::Text => Argument::Text(reader.text()?),
                ArgumentKind::Stack => Argument::Stack(reader.opaque()?),
                ArgumentKind::Compound => Argument::Compound(reader.opaque()?),
            });
        }
        Some(arguments)
    } else {
        None
    };
    let unread = reader.unread();
    if unread > 0 {
        return Err(ReadRefused::TrailingBytes(unread));
    }
    Ok(Action { name, arguments })
}

#[cfg(test)]
mod tests {
    use super::{
        Action, Argument, ArgumentKind, MAX_ARGUMENTS, MAX_BODY_BYTES, MAX_OPAQUE_BYTES,
        MAX_TEXT_CHARS, NumberingUnavailable, ReadRefused, TagNumbering, WriteRefused, read, write,
    };

    fn numbering() -> TagNumbering {
        TagNumbering::placeholder()
    }

    #[test]
    fn every_kind_round_trips_under_the_placeholder_numbering() {
        let sent = Action::carrying(
            "commit",
            vec![
                Argument::Absent,
                Argument::Integer(-70_000),
                Argument::Long(i64::MIN),
                Argument::Short(-1),
                Argument::Boolean(true),
                Argument::Text("a name".to_owned()),
                Argument::Stack(vec![1, 2, 3]),
                Argument::Compound(vec![9]),
            ],
        );
        let bytes = write(&sent, &numbering()).expect("every field is inside its bound");
        assert_eq!(read(&bytes, &numbering()), Ok(sent));
    }

    #[test]
    fn an_absent_vector_and_an_empty_one_are_different_on_the_wire() {
        let bare = write(&Action::bare("open"), &numbering()).expect("a bare action");
        let empty = write(&Action::carrying("open", Vec::new()), &numbering()).expect("an empty");
        assert_ne!(bare, empty);
        assert_eq!(read(&bare, &numbering()).expect("a bare action").count(), 0);
        assert!(
            read(&bare, &numbering())
                .expect("a bare action")
                .arguments
                .is_none()
        );
        assert_eq!(
            read(&empty, &numbering())
                .expect("an empty vector")
                .arguments,
            Some(Vec::new())
        );
    }

    #[test]
    fn a_tag_outside_the_numbering_is_refused_rather_than_indexed() {
        let mut bytes = write(
            &Action::carrying("go", vec![Argument::Absent]),
            &numbering(),
        )
        .expect("one arg");
        let tag = bytes.last_mut().expect("the tag is the last byte");
        *tag = 200;
        assert_eq!(
            read(&bytes, &numbering()),
            Err(ReadRefused::UnknownTag(200))
        );
    }

    #[test]
    fn a_declared_count_past_the_bound_is_refused_before_anything_is_allocated() {
        // The name, an argument vector present, and a declared count of two
        // billion with no argument after it.
        let mut bytes = vec![2, b'g', b'o', 1];
        bytes.extend_from_slice(&[0x80, 0x94, 0xeb, 0xdc, 0x03]);
        assert!(matches!(
            read(&bytes, &numbering()),
            Err(ReadRefused::TooManyArguments(_))
        ));
    }

    #[test]
    fn a_vector_past_the_bound_is_refused_at_the_writer_too() {
        let arguments = vec![Argument::Absent; MAX_ARGUMENTS + 1];
        assert_eq!(
            write(&Action::carrying("go", arguments), &numbering()),
            Err(WriteRefused::TooManyArguments(MAX_ARGUMENTS + 1))
        );
    }

    #[test]
    fn a_body_past_the_bound_is_refused_before_a_field_is_read() {
        let bytes = vec![0u8; MAX_BODY_BYTES + 1];
        assert_eq!(
            read(&bytes, &numbering()),
            Err(ReadRefused::BodyTooLong(MAX_BODY_BYTES + 1))
        );
    }

    #[test]
    fn text_is_bounded_at_both_ends() {
        let long = "x".repeat(MAX_TEXT_CHARS + 1);
        assert_eq!(
            write(
                &Action::carrying("go", vec![Argument::Text(long)]),
                &numbering()
            ),
            Err(WriteRefused::TextTooLong(MAX_TEXT_CHARS + 1))
        );
        let mut bytes = vec![2, b'g', b'o', 1, 1, 5];
        bytes.push(0xff);
        bytes.push(0xff);
        bytes.push(0x07);
        assert!(matches!(
            read(&bytes, &numbering()),
            Err(ReadRefused::TextTooLong(_))
        ));
    }

    #[test]
    fn an_opaque_field_is_bounded_at_both_ends() {
        let long = vec![0u8; MAX_OPAQUE_BYTES + 1];
        assert_eq!(
            write(
                &Action::carrying("go", vec![Argument::Compound(long)]),
                &numbering()
            ),
            Err(WriteRefused::OpaqueTooLong(MAX_OPAQUE_BYTES + 1))
        );
        let mut bytes = vec![2, b'g', b'o', 1, 1, 7];
        bytes.extend_from_slice(&[0xff, 0xff, 0xff, 0x7f]);
        assert!(matches!(
            read(&bytes, &numbering()),
            Err(ReadRefused::OpaqueTooLong(_))
        ));
    }

    #[test]
    fn a_truncated_buffer_is_refused_rather_than_read_short() {
        let bytes = write(
            &Action::carrying("go", vec![Argument::Text("hello".to_owned())]),
            &numbering(),
        )
        .expect("one text argument");
        assert_eq!(
            read(&bytes[..bytes.len() - 1], &numbering()),
            Err(ReadRefused::Truncated)
        );
    }

    #[test]
    fn trailing_bytes_are_refused_so_two_buffers_never_read_as_one_action() {
        let mut bytes = write(&Action::bare("go"), &numbering()).expect("a bare action");
        bytes.push(0);
        assert_eq!(
            read(&bytes, &numbering()),
            Err(ReadRefused::TrailingBytes(1))
        );
    }

    #[test]
    fn a_boolean_byte_that_is_neither_zero_nor_one_is_refused() {
        let mut bytes = write(&Action::bare("go"), &numbering()).expect("a bare action");
        *bytes.last_mut().expect("the vector flag") = 2;
        assert_eq!(read(&bytes, &numbering()), Err(ReadRefused::NotABool(2)));
    }

    #[test]
    fn an_action_with_no_name_is_refused_at_the_reader_as_well_as_the_writer() {
        assert_eq!(
            write(&Action::bare(""), &numbering()),
            Err(WriteRefused::EmptyName)
        );
        // The bytes a peer that has no such writer sends: a zero-length name and
        // no vector. Decoding it in full and then answering `Unknown` would
        // apply this bound at one end only.
        assert_eq!(read(&[0, 0], &numbering()), Err(ReadRefused::EmptyName));
        assert_eq!(
            read(&[0, 1, 0], &numbering()),
            Err(ReadRefused::EmptyName),
            "an unnamed action is refused before its arguments are decoded"
        );
    }

    #[test]
    fn an_overlong_length_is_refused_rather_than_wrapping() {
        let bytes = vec![0xff; 12];
        assert_eq!(read(&bytes, &numbering()), Err(ReadRefused::Overlong));
    }

    #[test]
    fn the_numbering_a_build_supplies_moves_every_tag() {
        let supplied = TagNumbering::parse(
            "absent=9,integer=8,long=7,short=6,boolean=5,text=4,stack=3,compound=2",
        )
        .expect("eight distinct kinds and eight distinct numbers");
        assert_eq!(supplied.number(ArgumentKind::Absent), 9);
        assert_eq!(supplied.kind(9), Some(ArgumentKind::Absent));
        assert_eq!(supplied.kind(0), None);
        let action = Action::carrying("go", vec![Argument::Integer(1)]);
        let ours = write(&action, &numbering()).expect("a small action");
        let theirs = write(&action, &supplied).expect("a small action");
        assert_ne!(ours, theirs, "a numbering that changed nothing is not one");
        assert_eq!(read(&theirs, &supplied), Ok(action));
    }

    #[test]
    fn six_ways_a_supplied_numbering_is_refused() {
        assert_eq!(
            TagNumbering::parse("absent"),
            Err(NumberingUnavailable::Malformed("absent".to_owned()))
        );
        assert_eq!(
            TagNumbering::parse("absent=300"),
            Err(NumberingUnavailable::Malformed("absent=300".to_owned()))
        );
        assert_eq!(
            TagNumbering::parse("colour=1"),
            Err(NumberingUnavailable::UnknownKind("colour".to_owned()))
        );
        assert_eq!(
            TagNumbering::parse("absent=0,absent=1"),
            Err(NumberingUnavailable::DuplicateKind(ArgumentKind::Absent))
        );
        assert_eq!(
            TagNumbering::parse("absent=0,integer=0"),
            Err(NumberingUnavailable::DuplicateNumber(0))
        );
        assert_eq!(
            TagNumbering::parse("absent=0"),
            Err(NumberingUnavailable::Missing(ArgumentKind::Integer))
        );
    }

    #[test]
    fn the_declared_numbering_is_absent_rather_than_defaulted() {
        // No build here supplies one, so a package that shipped a default would
        // silently talk to itself while claiming to talk to an original.
        assert_eq!(
            TagNumbering::declared(),
            Err(NumberingUnavailable::NotSupplied)
        );
    }

    #[test]
    fn a_kind_names_itself_and_a_name_names_its_kind() {
        for kind in ArgumentKind::ALL {
            assert_eq!(ArgumentKind::named(kind.as_str()), Some(kind));
        }
        assert_eq!(ArgumentKind::named("absent "), None);
    }

    #[test]
    fn reading_an_argument_answers_absent_rather_than_throwing_or_casting() {
        let action = Action::carrying("go", vec![Argument::Boolean(false)]);
        assert_eq!(action.get(0), Some(&Argument::Boolean(false)));
        assert_eq!(action.get(1), None);
        assert_eq!(Action::bare("go").get(0), None);
    }
}
