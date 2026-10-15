//! The tag validator vocabulary, from §7.4.
//!
//! Eleven kinds in four families: five over text, two over lists, two over
//! numbers and two over compounds. Each family has a default kind gated by its
//! own distinct field name, and every kind additionally decodes from an object
//! naming its kind.
//!
//! Every kind checks that the candidate belongs to its family first, so a text
//! validator against a number is false rather than an error. The list family
//! admits the three primitive array kinds alongside the general list.
//!
//! # The two defects fixed here
//!
//! - **An unbounded lower bound is negative infinity.** The original substitutes
//!   the smallest *positive* representable value when a floating-point range
//!   declares only an upper bound, so every negative value and zero fail a range
//!   that meant to admit them.
//! - **A family must be named where context does not supply one.** The original
//!   tries the four families in a fixed order and takes the first that parses.
//!   Each family's default kind is gated by its own distinct field name, so
//!   which orderings actually collide is unknown and untested;
//!   [`Validator::decode`] refuses rather than resolving by luck.
//!
//! # The kind names are read, never invented
//!
//! §14 makes the eleven kind names and their field names compatibility-required:
//! a pack author writes them. [`Vocabulary::declared`] reads them from build-time
//! inputs and [`Vocabulary::placeholder`] carries this package's own, which the
//! tests pin the surrounding **shape** with and which agree with no original.

use std::collections::BTreeMap;

use num_traits::ToPrimitive;
use ravel_sdk::JsonValue;

use crate::data::{Kind, Tag};

/// The four families a validator belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Family {
    /// Validators over a text tag.
    Text,
    /// Validators over any of the four list shapes.
    List,
    /// Validators over a numeric tag.
    Number,
    /// Validators over a compound tag.
    Compound,
}

impl Family {
    /// Every family, in the order this enum declares them.
    pub const ALL: [Self; 4] = [Self::Text, Self::List, Self::Number, Self::Compound];

    /// Returns whether a candidate tag belongs to this family.
    #[must_use]
    pub const fn admits(self, kind: Kind) -> bool {
        match self {
            Self::Text => matches!(kind, Kind::Text),
            Self::List => kind.is_list(),
            Self::Number => kind.is_numeric(),
            Self::Compound => matches!(kind, Kind::Compound),
        }
    }

    /// Returns the family's default kind.
    #[must_use]
    pub const fn default_kind(self) -> KindName {
        match self {
            Self::Text => KindName::TextEquals,
            Self::List => KindName::ListOrdered,
            Self::Number => KindName::NumberEquals,
            Self::Compound => KindName::CompoundPartial,
        }
    }
}

/// The eleven kinds, named for what each does.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum KindName {
    /// Text equal to the expected value.
    TextEquals,
    /// Text containing the expected value.
    TextContains,
    /// Text beginning with the expected value.
    TextPrefix,
    /// Text ending with the expected value.
    TextSuffix,
    /// Text a pattern matches whole.
    TextPattern,
    /// A list matched element for element, in order.
    ListOrdered,
    /// A list every element of which satisfies one validator.
    ListUniversal,
    /// A number equal to the expected value at the candidate's own width.
    NumberEquals,
    /// A number inside a range with optional ends.
    NumberRange,
    /// A compound naming keys that must be present and match.
    CompoundPartial,
    /// A compound that must additionally name no key the validator does not.
    CompoundExact,
}

impl KindName {
    /// Every kind, in the order the vocabulary reads them.
    pub const ALL: [Self; 11] = [
        Self::TextEquals,
        Self::TextContains,
        Self::TextPrefix,
        Self::TextSuffix,
        Self::TextPattern,
        Self::ListOrdered,
        Self::ListUniversal,
        Self::NumberEquals,
        Self::NumberRange,
        Self::CompoundPartial,
        Self::CompoundExact,
    ];

    /// Returns the family this kind belongs to.
    #[must_use]
    pub const fn family(self) -> Family {
        match self {
            Self::TextEquals
            | Self::TextContains
            | Self::TextPrefix
            | Self::TextSuffix
            | Self::TextPattern => Family::Text,
            Self::ListOrdered | Self::ListUniversal => Family::List,
            Self::NumberEquals | Self::NumberRange => Family::Number,
            Self::CompoundPartial | Self::CompoundExact => Family::Compound,
        }
    }

    /// Returns whether this is its family's default kind.
    #[must_use]
    pub fn is_family_default(self) -> bool {
        self.family().default_kind() == self
    }

    const fn index(self) -> usize {
        match self {
            Self::TextEquals => 0,
            Self::TextContains => 1,
            Self::TextPrefix => 2,
            Self::TextSuffix => 3,
            Self::TextPattern => 4,
            Self::ListOrdered => 5,
            Self::ListUniversal => 6,
            Self::NumberEquals => 7,
            Self::NumberRange => 8,
            Self::CompoundPartial => 9,
            Self::CompoundExact => 10,
        }
    }
}

/// Why a vocabulary could not be built.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VocabularyUnavailable {
    /// The build supplied nothing.
    NotSupplied,
    /// The build supplied the wrong number of names.
    WrongCount {
        /// How many names were expected.
        expected: usize,
        /// How many arrived.
        found: usize,
    },
    /// A name is empty.
    Empty,
    /// Two names that have to differ are the same.
    NotDistinct,
}

impl std::fmt::Display for VocabularyUnavailable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotSupplied => {
                write!(formatter, "the build supplied no validator vocabulary")
            }
            Self::WrongCount { expected, found } => {
                write!(
                    formatter,
                    "{expected} names were expected and {found} arrived"
                )
            }
            Self::Empty => write!(formatter, "a validator name is empty"),
            Self::NotDistinct => write!(formatter, "two validator names collide"),
        }
    }
}

impl std::error::Error for VocabularyUnavailable {}

/// The spellings a pack author writes.
///
/// Four groups: the eleven kind names, the field each kind reads its argument
/// from, the two bound names inside a range's argument, and the field that names
/// a kind outright.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vocabulary {
    kinds: [String; 11],
    fields: [String; 11],
    bounds: [String; 2],
    discriminator: String,
}

impl Default for Vocabulary {
    fn default() -> Self {
        Self::placeholder()
    }
}

impl Vocabulary {
    /// The spellings the tests pin the decoding shape against.
    ///
    /// These are this package's own words. They agree with no original and a
    /// build that means to read an original's documents supplies its own.
    #[must_use]
    pub fn placeholder() -> Self {
        Self {
            kinds: [
                "equals".to_owned(),
                "contains".to_owned(),
                "starts_with".to_owned(),
                "ends_with".to_owned(),
                "pattern".to_owned(),
                "ordered".to_owned(),
                "universal".to_owned(),
                "number_equals".to_owned(),
                "in_range".to_owned(),
                "partial".to_owned(),
                "exact_keys".to_owned(),
            ],
            fields: [
                "text".to_owned(),
                "substring".to_owned(),
                "beginning".to_owned(),
                "ending".to_owned(),
                "expression".to_owned(),
                "elements".to_owned(),
                "element".to_owned(),
                "value".to_owned(),
                "range".to_owned(),
                "keys".to_owned(),
                "every_key".to_owned(),
            ],
            bounds: ["lower".to_owned(), "upper".to_owned()],
            discriminator: "kind".to_owned(),
        }
    }

    /// Reads the spellings from build-time inputs.
    ///
    /// # Errors
    ///
    /// Returns [`VocabularyUnavailable`] when the build supplied nothing, or
    /// supplied a group of the wrong size, an empty name, or names that collide
    /// where they must not.
    pub fn declared() -> Result<Self, VocabularyUnavailable> {
        let kinds = option_env!("RVL_060_COUNTERPART_VALIDATOR_KINDS")
            .ok_or(VocabularyUnavailable::NotSupplied)?;
        let fields = option_env!("RVL_060_COUNTERPART_VALIDATOR_FIELDS")
            .ok_or(VocabularyUnavailable::NotSupplied)?;
        let bounds = option_env!("RVL_060_COUNTERPART_VALIDATOR_BOUNDS")
            .ok_or(VocabularyUnavailable::NotSupplied)?;
        let discriminator = option_env!("RVL_060_COUNTERPART_VALIDATOR_DISCRIMINATOR")
            .ok_or(VocabularyUnavailable::NotSupplied)?;
        Self::parse(kinds, fields, bounds, discriminator)
    }

    /// Reads the spellings from four comma-separated groups.
    ///
    /// # Errors
    ///
    /// Returns [`VocabularyUnavailable`] for a group of the wrong size, an empty
    /// name, kind names that are not distinct, family gates that are not
    /// distinct, or a discriminator that collides with a family gate.
    pub fn parse(
        kinds: &str,
        fields: &str,
        bounds: &str,
        discriminator: &str,
    ) -> Result<Self, VocabularyUnavailable> {
        let kinds = exactly::<11>(kinds)?;
        let fields = exactly::<11>(fields)?;
        let bounds = exactly::<2>(bounds)?;
        let discriminator = discriminator.trim();
        if discriminator.is_empty() {
            return Err(VocabularyUnavailable::Empty);
        }
        let vocabulary = Self {
            kinds,
            fields,
            bounds,
            discriminator: discriminator.to_owned(),
        };
        vocabulary.check()?;
        Ok(vocabulary)
    }

    fn check(&self) -> Result<(), VocabularyUnavailable> {
        let mut seen: Vec<&str> = self.kinds.iter().map(String::as_str).collect();
        seen.sort_unstable();
        seen.dedup();
        if seen.len() != self.kinds.len() {
            return Err(VocabularyUnavailable::NotDistinct);
        }
        let mut gates: Vec<&str> = Family::ALL
            .into_iter()
            .map(|family| self.field(family.default_kind()))
            .collect();
        gates.push(&self.discriminator);
        gates.sort_unstable();
        let count = gates.len();
        gates.dedup();
        if gates.len() != count {
            return Err(VocabularyUnavailable::NotDistinct);
        }
        if self.bounds[0] == self.bounds[1] {
            return Err(VocabularyUnavailable::NotDistinct);
        }
        Ok(())
    }

    /// Returns the name a kind is written as.
    #[must_use]
    pub fn kind(&self, kind: KindName) -> &str {
        &self.kinds[kind.index()]
    }

    /// Returns the field a kind reads its argument from.
    #[must_use]
    pub fn field(&self, kind: KindName) -> &str {
        &self.fields[kind.index()]
    }

    /// Returns the field that names a kind outright.
    #[must_use]
    pub fn discriminator(&self) -> &str {
        &self.discriminator
    }

    /// Returns the lower and upper bound names inside a range's argument.
    #[must_use]
    pub fn bounds(&self) -> (&str, &str) {
        (&self.bounds[0], &self.bounds[1])
    }

    /// Returns the kind a name spells.
    #[must_use]
    pub fn named(&self, name: &str) -> Option<KindName> {
        KindName::ALL
            .into_iter()
            .find(|kind| self.kind(*kind) == name)
    }
}

fn exactly<const N: usize>(text: &str) -> Result<[String; N], VocabularyUnavailable> {
    let parts: Vec<&str> = text.split(',').map(str::trim).collect();
    if parts.len() != N {
        return Err(VocabularyUnavailable::WrongCount {
            expected: N,
            found: parts.len(),
        });
    }
    if parts.iter().any(|part| part.is_empty()) {
        return Err(VocabularyUnavailable::Empty);
    }
    let owned: Vec<String> = parts.into_iter().map(str::to_owned).collect();
    owned
        .try_into()
        .map_err(|_| VocabularyUnavailable::WrongCount {
            expected: N,
            found: N,
        })
}

/// One validator.
#[derive(Clone, Debug, PartialEq)]
pub enum Validator {
    /// Text equal to the expected value.
    TextEquals(String),
    /// Text containing the expected value.
    TextContains(String),
    /// Text beginning with the expected value.
    TextPrefix(String),
    /// Text ending with the expected value.
    TextSuffix(String),
    /// Text the pattern matches whole.
    TextPattern(Pattern),
    /// A list matched element for element, in order and at the same length.
    ListOrdered(Vec<Self>),
    /// A list every element of which satisfies one validator.
    ListUniversal(Box<Self>),
    /// A number equal to the expected value at the candidate's own width.
    NumberEquals(f64),
    /// A number inside a range with optional ends.
    NumberRange {
        /// The inclusive lower bound, unbounded when absent.
        lower: Option<f64>,
        /// The inclusive upper bound, unbounded when absent.
        upper: Option<f64>,
    },
    /// A compound whose named keys must be present and match, extra keys
    /// allowed.
    CompoundPartial(BTreeMap<String, Self>),
    /// A compound whose named keys must be present and match, and which must
    /// name no key the validator does not.
    CompoundExact(BTreeMap<String, Self>),
}

/// How deep a validator document will nest.
///
/// A validator arrives from a pack author's document, so its depth is not the
/// reader's to trust. The upgrade path is a larger constant.
pub const MAX_DEPTH: usize = 32;

/// The most elements or keys one validator will hold.
pub const MAX_WIDTH: usize = 1024;

impl Validator {
    /// Returns the kind this validator is.
    #[must_use]
    pub const fn kind(&self) -> KindName {
        match self {
            Self::TextEquals(_) => KindName::TextEquals,
            Self::TextContains(_) => KindName::TextContains,
            Self::TextPrefix(_) => KindName::TextPrefix,
            Self::TextSuffix(_) => KindName::TextSuffix,
            Self::TextPattern(_) => KindName::TextPattern,
            Self::ListOrdered(_) => KindName::ListOrdered,
            Self::ListUniversal(_) => KindName::ListUniversal,
            Self::NumberEquals(_) => KindName::NumberEquals,
            Self::NumberRange { .. } => KindName::NumberRange,
            Self::CompoundPartial(_) => KindName::CompoundPartial,
            Self::CompoundExact(_) => KindName::CompoundExact,
        }
    }

    /// Returns the family this validator belongs to.
    #[must_use]
    pub const fn family(&self) -> Family {
        self.kind().family()
    }

    /// Returns whether a candidate satisfies this validator.
    ///
    /// A candidate outside the validator's family is false rather than an error.
    #[must_use]
    pub fn holds(&self, candidate: &Tag) -> bool {
        if !self.family().admits(candidate.kind()) {
            return false;
        }
        match self {
            Self::TextEquals(expected) => text(candidate) == Some(expected.as_str()),
            Self::TextContains(expected) => {
                text(candidate).is_some_and(|value| value.contains(expected.as_str()))
            }
            Self::TextPrefix(expected) => {
                text(candidate).is_some_and(|value| value.starts_with(expected.as_str()))
            }
            Self::TextSuffix(expected) => {
                text(candidate).is_some_and(|value| value.ends_with(expected.as_str()))
            }
            Self::TextPattern(pattern) => {
                text(candidate).is_some_and(|value| pattern.matches_whole(value))
            }
            Self::ListOrdered(expected) => candidate.elements().is_some_and(|elements| {
                if elements.is_empty() || elements.len() != expected.len() {
                    return false;
                }
                expected
                    .iter()
                    .zip(elements.iter())
                    .all(|(validator, element)| validator.holds(element))
            }),
            Self::ListUniversal(expected) => candidate.elements().is_some_and(|elements| {
                !elements.is_empty() && elements.iter().all(|element| expected.holds(element))
            }),
            Self::NumberEquals(expected) => equal_at_width(*expected, candidate),
            Self::NumberRange { lower, upper } => candidate.decimal().is_some_and(|value| {
                let low = lower.unwrap_or(f64::NEG_INFINITY);
                let high = upper.unwrap_or(f64::INFINITY);
                value >= low && value <= high
            }),
            Self::CompoundPartial(keys) => compound(candidate).is_some_and(|entries| {
                keys.iter().all(|(name, validator)| {
                    entries.get(name).is_some_and(|tag| validator.holds(tag))
                })
            }),
            Self::CompoundExact(keys) => compound(candidate).is_some_and(|entries| {
                entries.len() == keys.len()
                    && keys.iter().all(|(name, validator)| {
                        entries.get(name).is_some_and(|tag| validator.holds(tag))
                    })
            }),
        }
    }

    /// Reads a validator from a document.
    ///
    /// `context` is the family the surrounding document already decided. When it
    /// is absent, the document must name a kind: the original's first-parse
    /// order is not reproduced.
    ///
    /// # Errors
    ///
    /// Returns [`ValidatorError`] for a document that is not an object, names no
    /// family and no kind, names an unknown kind, is missing the field its kind
    /// reads, holds a field of the wrong shape, nests past [`MAX_DEPTH`], or is
    /// wider than [`MAX_WIDTH`].
    pub fn decode(
        document: &JsonValue,
        vocabulary: &Vocabulary,
        context: Option<Family>,
    ) -> Result<Self, ValidatorError> {
        decode_at(document, vocabulary, context, 0)
    }
}

fn text(tag: &Tag) -> Option<&str> {
    match tag {
        Tag::Text(value) => Some(value.as_str()),
        Tag::Byte(_)
        | Tag::Short(_)
        | Tag::Int(_)
        | Tag::Long(_)
        | Tag::Float(_)
        | Tag::Double(_)
        | Tag::List(_)
        | Tag::ByteArray(_)
        | Tag::IntArray(_)
        | Tag::LongArray(_)
        | Tag::Compound(_) => None,
    }
}

fn compound(tag: &Tag) -> Option<&BTreeMap<String, Tag>> {
    match tag {
        Tag::Compound(entries) => Some(entries),
        Tag::Byte(_)
        | Tag::Short(_)
        | Tag::Int(_)
        | Tag::Long(_)
        | Tag::Float(_)
        | Tag::Double(_)
        | Tag::Text(_)
        | Tag::List(_)
        | Tag::ByteArray(_)
        | Tag::IntArray(_)
        | Tag::LongArray(_) => None,
    }
}

fn equal_at_width(expected: f64, candidate: &Tag) -> bool {
    // Integer-width matching truncates, with the original NaN-zero and saturation rules.
    let integer = || {
        expected.trunc().to_i64().unwrap_or_else(|| {
            if expected.is_nan() {
                0
            } else if expected.is_sign_negative() {
                i64::MIN
            } else {
                i64::MAX
            }
        })
    };
    match *candidate {
        Tag::Byte(value) => i8::try_from(integer()).is_ok_and(|narrow| narrow == value),
        Tag::Short(value) => i16::try_from(integer()).is_ok_and(|narrow| narrow == value),
        Tag::Int(value) => i32::try_from(integer()).is_ok_and(|narrow| narrow == value),
        Tag::Long(value) => integer() == value && expected.fract() == 0.0,
        Tag::Float(value) => {
            expected
                .to_f32()
                .expect("float narrowing includes infinities and NaN")
                .partial_cmp(&value)
                == Some(std::cmp::Ordering::Equal)
        }
        Tag::Double(value) => expected.partial_cmp(&value) == Some(std::cmp::Ordering::Equal),
        Tag::Text(_)
        | Tag::List(_)
        | Tag::ByteArray(_)
        | Tag::IntArray(_)
        | Tag::LongArray(_)
        | Tag::Compound(_) => false,
    }
}

fn decode_at(
    document: &JsonValue,
    vocabulary: &Vocabulary,
    context: Option<Family>,
    depth: usize,
) -> Result<Validator, ValidatorError> {
    if depth >= MAX_DEPTH {
        return Err(ValidatorError::TooDeep(MAX_DEPTH));
    }
    let object = document.as_object().ok_or(ValidatorError::NotAnObject)?;
    let kind = if let Some(named) = object.get(vocabulary.discriminator()) {
        let name = named.as_str().ok_or(ValidatorError::NotAnObject)?;
        vocabulary
            .named(name)
            .ok_or_else(|| ValidatorError::UnknownKind(name.to_owned()))?
    } else {
        let family = context.ok_or(ValidatorError::FamilyNotNamed)?;
        let default = family.default_kind();
        if !object.contains_key(vocabulary.field(default)) {
            return Err(ValidatorError::FieldMissing(
                vocabulary.field(default).to_owned(),
            ));
        }
        default
    };
    let field = vocabulary.field(kind);
    let argument = object
        .get(field)
        .ok_or_else(|| ValidatorError::FieldMissing(field.to_owned()))?;
    build(kind, argument, vocabulary, depth)
}

fn build(
    kind: KindName,
    argument: &JsonValue,
    vocabulary: &Vocabulary,
    depth: usize,
) -> Result<Validator, ValidatorError> {
    let wrong = || ValidatorError::WrongShape(vocabulary.field(kind).to_owned());
    match kind {
        KindName::TextEquals => Ok(Validator::TextEquals(
            argument.as_str().ok_or_else(wrong)?.to_owned(),
        )),
        KindName::TextContains => Ok(Validator::TextContains(
            argument.as_str().ok_or_else(wrong)?.to_owned(),
        )),
        KindName::TextPrefix => Ok(Validator::TextPrefix(
            argument.as_str().ok_or_else(wrong)?.to_owned(),
        )),
        KindName::TextSuffix => Ok(Validator::TextSuffix(
            argument.as_str().ok_or_else(wrong)?.to_owned(),
        )),
        KindName::TextPattern => {
            let source = argument.as_str().ok_or_else(wrong)?;
            Ok(Validator::TextPattern(
                Pattern::parse(source).map_err(ValidatorError::Pattern)?,
            ))
        }
        KindName::ListOrdered => {
            let items = argument.as_array().ok_or_else(wrong)?;
            if items.len() > MAX_WIDTH {
                return Err(ValidatorError::TooWide(items.len()));
            }
            let mut elements = Vec::with_capacity(items.len());
            for item in items {
                elements.push(decode_at(item, vocabulary, None, depth + 1)?);
            }
            Ok(Validator::ListOrdered(elements))
        }
        KindName::ListUniversal => Ok(Validator::ListUniversal(Box::new(decode_at(
            argument,
            vocabulary,
            None,
            depth + 1,
        )?))),
        KindName::NumberEquals => Ok(Validator::NumberEquals(
            argument.as_f64().ok_or_else(wrong)?,
        )),
        KindName::NumberRange => {
            let object = argument.as_object().ok_or_else(wrong)?;
            let (low, high) = vocabulary.bounds();
            let lower = match object.get(low) {
                None => None,
                Some(value) => Some(value.as_f64().ok_or_else(wrong)?),
            };
            let upper = match object.get(high) {
                None => None,
                Some(value) => Some(value.as_f64().ok_or_else(wrong)?),
            };
            Ok(Validator::NumberRange { lower, upper })
        }
        KindName::CompoundPartial | KindName::CompoundExact => {
            let object = argument.as_object().ok_or_else(wrong)?;
            if object.len() > MAX_WIDTH {
                return Err(ValidatorError::TooWide(object.len()));
            }
            let mut keys = BTreeMap::new();
            for (name, value) in object {
                keys.insert(name.clone(), decode_at(value, vocabulary, None, depth + 1)?);
            }
            if kind == KindName::CompoundPartial {
                Ok(Validator::CompoundPartial(keys))
            } else {
                Ok(Validator::CompoundExact(keys))
            }
        }
    }
}

/// Why a validator did not decode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidatorError {
    /// The document is not an object.
    NotAnObject,
    /// No family was supplied by context and the document names no kind.
    ///
    /// This is the refusal that replaces the original's first-parse order.
    FamilyNotNamed,
    /// The document names a kind the vocabulary does not publish.
    UnknownKind(String),
    /// The field the kind reads its argument from is absent.
    FieldMissing(String),
    /// The field the kind reads its argument from holds the wrong shape.
    WrongShape(String),
    /// The document nests past [`MAX_DEPTH`].
    TooDeep(usize),
    /// The document holds more than [`MAX_WIDTH`] elements or keys.
    TooWide(usize),
    /// A pattern is outside the supported subset.
    Pattern(PatternError),
}

impl std::fmt::Display for ValidatorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAnObject => write!(formatter, "a validator is not an object"),
            Self::FamilyNotNamed => write!(
                formatter,
                "a validator names no kind and nothing supplied its family"
            ),
            Self::UnknownKind(name) => write!(formatter, "`{name}` names no validator kind"),
            Self::FieldMissing(field) => write!(formatter, "a validator has no `{field}`"),
            Self::WrongShape(field) => write!(formatter, "`{field}` holds the wrong shape"),
            Self::TooDeep(ceiling) => write!(formatter, "a validator nests past {ceiling}"),
            Self::TooWide(width) => write!(formatter, "a validator holds {width} entries"),
            Self::Pattern(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for ValidatorError {}

/// Why a pattern was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatternError {
    /// The pattern uses a construct outside the supported subset.
    Unsupported(char),
    /// A repetition has no atom before it.
    DanglingRepeat,
    /// A character class does not close.
    UnclosedClass,
    /// An escape has nothing after it.
    DanglingEscape,
    /// The pattern holds more atoms than [`MAX_PATTERN_ATOMS`].
    TooLong(usize),
}

impl std::fmt::Display for PatternError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(found) => {
                write!(
                    formatter,
                    "`{found}` is outside the supported pattern subset"
                )
            }
            Self::DanglingRepeat => write!(formatter, "a repetition has nothing to repeat"),
            Self::UnclosedClass => write!(formatter, "a character class does not close"),
            Self::DanglingEscape => write!(formatter, "an escape has nothing after it"),
            Self::TooLong(atoms) => write!(formatter, "a pattern holds {atoms} atoms"),
        }
    }
}

impl std::error::Error for PatternError {}

/// The most atoms a pattern may hold.
pub const MAX_PATTERN_ATOMS: usize = 256;

/// The most matching steps one candidate may cost.
///
/// A backtracking matcher is exponential in the worst case and a pattern arrives
/// from a document. The budget makes the cost a refusal rather than a hang; the
/// upgrade path is a matcher that does not backtrack.
pub const MAX_PATTERN_STEPS: u32 = 200_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Repeat {
    Once,
    ZeroOrMore,
    OneOrMore,
    ZeroOrOne,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Atom {
    Any,
    Literal(char),
    Class {
        negated: bool,
        ranges: Vec<(char, char)>,
    },
}

impl Atom {
    fn admits(&self, value: char) -> bool {
        match self {
            Self::Any => true,
            Self::Literal(expected) => *expected == value,
            Self::Class { negated, ranges } => {
                let inside = ranges
                    .iter()
                    .any(|(low, high)| value >= *low && value <= *high);
                inside != *negated
            }
        }
    }
}

/// A pattern that must match a whole value.
///
/// **Declared change.** The original delegates to the platform's own regular
/// expression engine. This package supports a stated subset — literals, `.`, a
/// `[...]` character class with ranges and a leading `^` negation, the
/// repetitions `*`, `+` and `?`, and `\` escaping one of this subset's own
/// metacharacters — and refuses anything else at decode rather than accepting a
/// pattern it would match differently. `\d`, `\w` and `\s` name classes this
/// subset does not have, so each is refused rather than read as its letter. Matching is anchored at both ends either way, which is
/// what the original's whole-value rule means.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pattern {
    source: String,
    pieces: Vec<(Atom, Repeat)>,
}

impl Pattern {
    /// Reads a pattern.
    ///
    /// # Errors
    ///
    /// Returns [`PatternError`] for a construct outside the supported subset, a
    /// repetition with nothing before it, an unclosed class, a dangling escape,
    /// or a pattern past [`MAX_PATTERN_ATOMS`].
    pub fn parse(source: &str) -> Result<Self, PatternError> {
        let characters: Vec<char> = source.chars().collect();
        let mut pieces: Vec<(Atom, Repeat)> = Vec::new();
        let mut index = 0;
        while index < characters.len() {
            let atom = match characters[index] {
                '.' => {
                    index += 1;
                    Atom::Any
                }
                '\\' => {
                    index += 1;
                    let escaped = *characters.get(index).ok_or(PatternError::DanglingEscape)?;
                    index += 1;
                    Atom::Literal(escaped_literal(escaped)?)
                }
                '[' => {
                    index += 1;
                    let (class, next) = parse_class(&characters, index)?;
                    index = next;
                    class
                }
                '*' | '+' | '?' => return Err(PatternError::DanglingRepeat),
                unsupported @ ('(' | ')' | '|' | '{' | '}' | '^' | '$') => {
                    return Err(PatternError::Unsupported(unsupported));
                }
                literal => {
                    index += 1;
                    Atom::Literal(literal)
                }
            };
            let repeat = match characters.get(index) {
                Some('*') => {
                    index += 1;
                    Repeat::ZeroOrMore
                }
                Some('+') => {
                    index += 1;
                    Repeat::OneOrMore
                }
                Some('?') => {
                    index += 1;
                    Repeat::ZeroOrOne
                }
                _ => Repeat::Once,
            };
            if pieces.len() == MAX_PATTERN_ATOMS {
                return Err(PatternError::TooLong(MAX_PATTERN_ATOMS + 1));
            }
            pieces.push((atom, repeat));
        }
        Ok(Self {
            source: source.to_owned(),
            pieces,
        })
    }

    /// Returns the spelling this pattern was read from.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns whether the pattern matches the whole of a value.
    ///
    /// A candidate that costs more than [`MAX_PATTERN_STEPS`] is false rather
    /// than a hang.
    #[must_use]
    pub fn matches_whole(&self, candidate: &str) -> bool {
        let characters: Vec<char> = candidate.chars().collect();
        let mut budget = MAX_PATTERN_STEPS;
        walk(&self.pieces, &characters, &mut budget)
    }
}

/// The characters an escape may name, which are this subset's own metacharacters.
///
/// An escape outside this set is refused rather than read as the character
/// itself: a pattern author writing `\d` for a digit means a class this subset
/// does not have, and reading it as the letter `d` would match something else
/// silently.
const ESCAPABLE: [char; 9] = ['.', '[', ']', '*', '+', '?', '\\', '-', '^'];

fn escaped_literal(escaped: char) -> Result<char, PatternError> {
    if ESCAPABLE.contains(&escaped) {
        Ok(escaped)
    } else {
        Err(PatternError::Unsupported(escaped))
    }
}

fn parse_class(characters: &[char], mut index: usize) -> Result<(Atom, usize), PatternError> {
    let negated = characters.get(index) == Some(&'^');
    if negated {
        index += 1;
    }
    let mut ranges = Vec::new();
    loop {
        let current = *characters.get(index).ok_or(PatternError::UnclosedClass)?;
        if current == ']' {
            index += 1;
            break;
        }
        let low = if current == '\\' {
            index += 1;
            escaped_literal(*characters.get(index).ok_or(PatternError::DanglingEscape)?)?
        } else {
            current
        };
        index += 1;
        if characters.get(index) == Some(&'-')
            && let Some(&high) = characters.get(index + 1)
            && high != ']'
        {
            ranges.push((low, high));
            index += 2;
        } else {
            ranges.push((low, low));
        }
        if ranges.len() > MAX_PATTERN_ATOMS {
            return Err(PatternError::TooLong(ranges.len()));
        }
    }
    Ok((Atom::Class { negated, ranges }, index))
}

fn walk(pieces: &[(Atom, Repeat)], candidate: &[char], budget: &mut u32) -> bool {
    if *budget == 0 {
        return false;
    }
    *budget -= 1;
    let Some(((atom, repeat), rest)) = pieces.split_first() else {
        return candidate.is_empty();
    };
    match repeat {
        Repeat::Once => match candidate.split_first() {
            Some((first, tail)) if atom.admits(*first) => walk(rest, tail, budget),
            _ => false,
        },
        Repeat::ZeroOrOne => {
            if let Some((first, tail)) = candidate.split_first()
                && atom.admits(*first)
                && walk(rest, tail, budget)
            {
                return true;
            }
            walk(rest, candidate, budget)
        }
        Repeat::ZeroOrMore | Repeat::OneOrMore => {
            let mut taken = 0;
            while taken < candidate.len() && atom.admits(candidate[taken]) {
                taken += 1;
            }
            let least = usize::from(*repeat == Repeat::OneOrMore);
            let mut count = taken;
            loop {
                if count < least {
                    return false;
                }
                if walk(rest, &candidate[count..], budget) {
                    return true;
                }
                if count == least {
                    return false;
                }
                count -= 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ravel_sdk::json;

    use super::{
        Family, KindName, Pattern, PatternError, Validator, ValidatorError, Vocabulary,
        VocabularyUnavailable,
    };
    use crate::data::Tag;

    #[test]
    fn numeric_width_conversion_keeps_truncation_saturation_and_ieee_equality() {
        assert!(super::equal_at_width(f64::NAN, &Tag::Byte(0)));
        assert!(super::equal_at_width(1.9, &Tag::Int(1)));
        assert!(!super::equal_at_width(1.9, &Tag::Long(1)));
        assert!(!super::equal_at_width(f64::NAN, &Tag::Float(f32::NAN)));
        assert!(super::equal_at_width(-0.0, &Tag::Float(0.0)));
        assert!(super::equal_at_width(f64::MAX, &Tag::Float(f32::INFINITY)));
        assert!(super::equal_at_width(
            9_223_372_036_854_775_808.0,
            &Tag::Long(i64::MAX)
        ));
        assert!(super::equal_at_width(
            -9_223_372_036_854_775_808.0,
            &Tag::Long(i64::MIN)
        ));
    }

    fn vocabulary() -> Vocabulary {
        Vocabulary::placeholder()
    }

    #[test]
    fn eleven_kinds_sit_in_four_families_with_the_counts_the_specification_states() {
        assert_eq!(KindName::ALL.len(), 11);
        let count = |family: Family| {
            KindName::ALL
                .into_iter()
                .filter(|kind| kind.family() == family)
                .count()
        };
        assert_eq!(count(Family::Text), 5);
        assert_eq!(count(Family::List), 2);
        assert_eq!(count(Family::Number), 2);
        assert_eq!(count(Family::Compound), 2);
    }

    #[test]
    fn each_family_has_exactly_one_default_kind() {
        for family in Family::ALL {
            let defaults = KindName::ALL
                .into_iter()
                .filter(|kind| kind.family() == family && kind.is_family_default())
                .count();
            assert_eq!(defaults, 1, "{family:?} has one default");
        }
        assert_eq!(Family::Text.default_kind(), KindName::TextEquals);
        assert_eq!(Family::List.default_kind(), KindName::ListOrdered);
        assert_eq!(Family::Number.default_kind(), KindName::NumberEquals);
        assert_eq!(Family::Compound.default_kind(), KindName::CompoundPartial);
    }

    #[test]
    fn the_four_family_gates_and_the_discriminator_are_all_distinct() {
        let vocabulary = vocabulary();
        let mut gates: Vec<&str> = Family::ALL
            .into_iter()
            .map(|family| vocabulary.field(family.default_kind()))
            .collect();
        gates.push(vocabulary.discriminator());
        let count = gates.len();
        gates.sort_unstable();
        gates.dedup();
        assert_eq!(gates.len(), count);
    }

    #[test]
    fn the_placeholder_vocabulary_names_eleven_distinct_kinds() {
        let vocabulary = vocabulary();
        let mut names: Vec<&str> = KindName::ALL
            .into_iter()
            .map(|kind| vocabulary.kind(kind))
            .collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 11);
        for kind in KindName::ALL {
            assert_eq!(vocabulary.named(vocabulary.kind(kind)), Some(kind));
        }
        assert_eq!(vocabulary.named("nothing_spells_this"), None);
    }

    #[test]
    fn the_declared_vocabulary_is_absent_rather_than_defaulted() {
        assert_eq!(
            Vocabulary::declared(),
            Err(VocabularyUnavailable::NotSupplied),
            "no build input is supplied in this workspace, and a placeholder is not an answer"
        );
    }

    #[test]
    fn a_supplied_vocabulary_is_refused_five_ways() {
        assert!(matches!(
            Vocabulary::parse("a,b", "a,b", "l,u", "k"),
            Err(VocabularyUnavailable::WrongCount { expected: 11, .. })
        ));
        let eleven = "a,b,c,d,e,f,g,h,i,j,k";
        let fields = "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f11";
        assert_eq!(
            Vocabulary::parse(eleven, fields, "l,u", "  "),
            Err(VocabularyUnavailable::Empty)
        );
        assert_eq!(
            Vocabulary::parse("a,a,c,d,e,f,g,h,i,j,k", fields, "l,u", "kind"),
            Err(VocabularyUnavailable::NotDistinct)
        );
        assert_eq!(
            Vocabulary::parse(eleven, "f1,f2,f3,f4,f5,f1,f7,f8,f9,f10,f11", "l,u", "kind"),
            Err(VocabularyUnavailable::NotDistinct),
            "two family gates cannot be the same field"
        );
        assert_eq!(
            Vocabulary::parse(eleven, fields, "same,same", "kind"),
            Err(VocabularyUnavailable::NotDistinct)
        );
        assert_eq!(
            Vocabulary::parse(eleven, fields, "l,u", "f1"),
            Err(VocabularyUnavailable::NotDistinct),
            "the discriminator cannot be a family gate"
        );
        assert!(Vocabulary::parse(eleven, fields, "l,u", "kind").is_ok());
    }

    #[test]
    fn every_kind_checks_the_candidates_family_before_anything_else() {
        let number = Tag::Int(1);
        assert!(!Validator::TextEquals("1".to_owned()).holds(&number));
        assert!(!Validator::ListUniversal(Box::new(Validator::NumberEquals(1.0))).holds(&number));
        assert!(!Validator::CompoundPartial(BTreeMap::new()).holds(&number));
        assert!(!Validator::NumberEquals(1.0).holds(&Tag::Text("1".to_owned())));
    }

    #[test]
    fn the_five_text_kinds_do_what_they_are_named_for() {
        let candidate = Tag::Text("lantern".to_owned());
        assert!(Validator::TextEquals("lantern".to_owned()).holds(&candidate));
        assert!(!Validator::TextEquals("lanterns".to_owned()).holds(&candidate));
        assert!(Validator::TextContains("nte".to_owned()).holds(&candidate));
        assert!(Validator::TextPrefix("lan".to_owned()).holds(&candidate));
        assert!(Validator::TextSuffix("ern".to_owned()).holds(&candidate));
        assert!(!Validator::TextPrefix("ern".to_owned()).holds(&candidate));
    }

    #[test]
    fn a_pattern_must_match_the_whole_value_rather_than_a_part_of_it() {
        let pattern = Pattern::parse("lan.*").expect("a supported pattern");
        assert!(pattern.matches_whole("lantern"));
        assert!(!pattern.matches_whole("a lantern"));
        let digits = Pattern::parse("[0-9]+").expect("a supported pattern");
        assert!(digits.matches_whole("12345"));
        assert!(!digits.matches_whole("12a45"));
        assert!(!digits.matches_whole(""));
    }

    #[test]
    fn the_supported_pattern_subset_is_stated_and_the_rest_is_refused() {
        assert!(Pattern::parse("a?b*c+.").is_ok());
        assert!(Pattern::parse("[^a-c]").is_ok());
        assert!(Pattern::parse(r"\.").is_ok());
        assert_eq!(Pattern::parse("(a)"), Err(PatternError::Unsupported('(')));
        assert_eq!(Pattern::parse("a|b"), Err(PatternError::Unsupported('|')));
        assert_eq!(Pattern::parse("*a"), Err(PatternError::DanglingRepeat));
        assert_eq!(Pattern::parse("[a"), Err(PatternError::UnclosedClass));
        assert_eq!(Pattern::parse(r"a\"), Err(PatternError::DanglingEscape));
    }

    #[test]
    fn an_escaped_metacharacter_is_a_literal() {
        let pattern = Pattern::parse(r"a\.c").expect("a supported pattern");
        assert!(pattern.matches_whole("a.c"));
        assert!(!pattern.matches_whole("abc"));
        let hyphen = Pattern::parse(r"[a\-c]").expect("an escaped hyphen is a literal");
        assert!(hyphen.matches_whole("-"));
        assert!(!hyphen.matches_whole("b"));
    }

    #[test]
    fn an_escape_naming_a_class_this_subset_does_not_have_is_refused() {
        for (spelling, named) in [
            (r"\d+", 'd'),
            (r"\w", 'w'),
            (r"\s", 's'),
            (r"\D", 'D'),
            (r"\b", 'b'),
            (r"[\d]", 'd'),
        ] {
            assert_eq!(
                Pattern::parse(spelling),
                Err(PatternError::Unsupported(named)),
                "`{spelling}` would match the letter rather than the class"
            );
        }
    }

    #[test]
    fn a_negated_class_admits_what_it_does_not_name() {
        let pattern = Pattern::parse("[^0-9]+").expect("a supported pattern");
        assert!(pattern.matches_whole("abc"));
        assert!(!pattern.matches_whole("ab1"));
    }

    #[test]
    fn a_hostile_pattern_is_refused_by_the_step_budget_rather_than_running_forever() {
        let pattern = Pattern::parse(&format!("{}b", "a*".repeat(24)))
            .expect("the pattern itself is inside the subset");
        assert!(
            !pattern.matches_whole(&"a".repeat(40)),
            "the budget answers false rather than exhausting anything"
        );
    }

    #[test]
    fn a_pattern_longer_than_the_ceiling_is_refused() {
        let long = "a".repeat(super::MAX_PATTERN_ATOMS + 1);
        assert!(matches!(
            Pattern::parse(&long),
            Err(PatternError::TooLong(_))
        ));
    }

    #[test]
    fn both_list_kinds_reject_an_empty_candidate_outright() {
        let ordered = Validator::ListOrdered(Vec::new());
        let universal = Validator::ListUniversal(Box::new(Validator::NumberEquals(1.0)));
        for empty in [Tag::List(vec![]), Tag::IntArray(vec![])] {
            assert!(
                !ordered.holds(&empty),
                "an author testing for an empty list would have written this and expected it to hold"
            );
            assert!(!universal.holds(&empty));
        }
    }

    #[test]
    fn the_ordered_list_kind_matches_position_for_position_at_the_same_length() {
        let validator = Validator::ListOrdered(vec![
            Validator::NumberEquals(1.0),
            Validator::NumberEquals(2.0),
        ]);
        assert!(validator.holds(&Tag::List(vec![Tag::Int(1), Tag::Int(2)])));
        assert!(!validator.holds(&Tag::List(vec![Tag::Int(2), Tag::Int(1)])));
        assert!(!validator.holds(&Tag::List(vec![Tag::Int(1)])));
        assert!(!validator.holds(&Tag::List(vec![Tag::Int(1), Tag::Int(2), Tag::Int(3)])));
    }

    #[test]
    fn the_universal_list_kind_holds_every_element_to_one_validator() {
        let validator = Validator::ListUniversal(Box::new(Validator::NumberRange {
            lower: Some(0.0),
            upper: Some(10.0),
        }));
        assert!(validator.holds(&Tag::IntArray(vec![0, 5, 10])));
        assert!(!validator.holds(&Tag::IntArray(vec![0, 11])));
    }

    #[test]
    fn the_list_family_admits_the_three_primitive_arrays_as_well_as_the_general_list() {
        let validator = Validator::ListUniversal(Box::new(Validator::NumberEquals(1.0)));
        assert!(validator.holds(&Tag::ByteArray(vec![1])));
        assert!(validator.holds(&Tag::IntArray(vec![1])));
        assert!(validator.holds(&Tag::LongArray(vec![1])));
        assert!(validator.holds(&Tag::List(vec![Tag::Short(1)])));
    }

    #[test]
    fn a_number_is_compared_at_the_candidates_own_width() {
        assert!(Validator::NumberEquals(7.0).holds(&Tag::Byte(7)));
        assert!(Validator::NumberEquals(7.0).holds(&Tag::Long(7)));
        assert!(Validator::NumberEquals(7.5).holds(&Tag::Double(7.5)));
        assert!(!Validator::NumberEquals(7.5).holds(&Tag::Long(7)));
        assert!(
            !Validator::NumberEquals(300.0).holds(&Tag::Byte(44)),
            "a value that does not fit the candidate's width does not match by truncation"
        );
    }

    #[test]
    fn an_absent_lower_bound_is_negative_infinity_rather_than_the_smallest_positive_value() {
        let validator = Validator::NumberRange {
            lower: None,
            upper: Some(10.0),
        };
        assert!(
            validator.holds(&Tag::Double(-5.0)),
            "this is the fixed defect"
        );
        assert!(validator.holds(&Tag::Double(0.0)));
        assert!(validator.holds(&Tag::Float(-0.5)));
        assert!(validator.holds(&Tag::Int(-100)));
        assert!(!validator.holds(&Tag::Double(10.5)));
    }

    #[test]
    fn both_bounds_are_inclusive_and_an_absent_one_is_unbounded() {
        let validator = Validator::NumberRange {
            lower: Some(1.0),
            upper: Some(3.0),
        };
        assert!(validator.holds(&Tag::Int(1)));
        assert!(validator.holds(&Tag::Int(3)));
        assert!(!validator.holds(&Tag::Int(0)));
        assert!(!validator.holds(&Tag::Int(4)));
        let open = Validator::NumberRange {
            lower: None,
            upper: None,
        };
        assert!(open.holds(&Tag::Long(i64::MIN)));
        assert!(open.holds(&Tag::Long(i64::MAX)));
    }

    #[test]
    fn the_partial_compound_kind_allows_extra_keys_and_the_exact_one_does_not() {
        let mut keys = BTreeMap::new();
        keys.insert("a".to_owned(), Validator::NumberEquals(1.0));
        let candidate = Tag::compound([("a", Tag::Int(1)), ("b", Tag::Int(2))]);
        assert!(Validator::CompoundPartial(keys.clone()).holds(&candidate));
        assert!(!Validator::CompoundExact(keys.clone()).holds(&candidate));
        let tight = Tag::compound([("a", Tag::Int(1))]);
        assert!(Validator::CompoundExact(keys).holds(&tight));
    }

    #[test]
    fn a_missing_key_fails_both_compound_kinds() {
        let mut keys = BTreeMap::new();
        keys.insert("a".to_owned(), Validator::NumberEquals(1.0));
        let candidate = Tag::compound([("b", Tag::Int(1))]);
        assert!(!Validator::CompoundPartial(keys.clone()).holds(&candidate));
        assert!(!Validator::CompoundExact(keys).holds(&candidate));
    }

    #[test]
    fn a_family_from_context_decodes_that_familys_default_kind() {
        let vocabulary = vocabulary();
        let document = json!({ "text": "lantern" });
        let decoded = Validator::decode(&document, &vocabulary, Some(Family::Text))
            .expect("the text gate is present");
        assert_eq!(decoded, Validator::TextEquals("lantern".to_owned()));
        assert_eq!(decoded.kind(), KindName::TextEquals);
    }

    #[test]
    fn a_validator_with_no_family_named_is_refused_rather_than_resolved_by_parse_order() {
        let vocabulary = vocabulary();
        let document = json!({ "text": "lantern" });
        assert_eq!(
            Validator::decode(&document, &vocabulary, None),
            Err(ValidatorError::FamilyNotNamed),
            "this is the fixed defect: the original takes the first family that parses"
        );
    }

    #[test]
    fn naming_a_kind_outright_decodes_without_any_context() {
        let vocabulary = vocabulary();
        let document = json!({ "kind": "starts_with", "beginning": "lan" });
        assert_eq!(
            Validator::decode(&document, &vocabulary, None),
            Ok(Validator::TextPrefix("lan".to_owned()))
        );
    }

    #[test]
    fn a_named_kind_wins_over_the_context_family() {
        let vocabulary = vocabulary();
        let document = json!({ "kind": "number_equals", "value": 4 });
        assert_eq!(
            Validator::decode(&document, &vocabulary, Some(Family::Text)),
            Ok(Validator::NumberEquals(4.0))
        );
    }

    #[test]
    fn every_one_of_the_eleven_kinds_decodes_from_a_document_naming_it() {
        let vocabulary = vocabulary();
        let documents = [
            json!({ "kind": "equals", "text": "a" }),
            json!({ "kind": "contains", "substring": "a" }),
            json!({ "kind": "starts_with", "beginning": "a" }),
            json!({ "kind": "ends_with", "ending": "a" }),
            json!({ "kind": "pattern", "expression": "a+" }),
            json!({ "kind": "ordered", "elements": [] }),
            json!({ "kind": "universal", "element": { "kind": "equals", "text": "a" } }),
            json!({ "kind": "number_equals", "value": 1 }),
            json!({ "kind": "in_range", "range": { "lower": 1 } }),
            json!({ "kind": "partial", "keys": {} }),
            json!({ "kind": "exact_keys", "every_key": {} }),
        ];
        assert_eq!(documents.len(), KindName::ALL.len());
        for (document, expected) in documents.iter().zip(KindName::ALL) {
            let decoded = Validator::decode(document, &vocabulary, None)
                .unwrap_or_else(|error| panic!("{expected:?} decodes: {error}"));
            assert_eq!(decoded.kind(), expected);
        }
    }

    #[test]
    fn a_range_reads_each_bound_by_its_own_name_and_leaves_an_absent_one_unbounded() {
        let vocabulary = vocabulary();
        let document = json!({ "kind": "in_range", "range": { "upper": 9 } });
        assert_eq!(
            Validator::decode(&document, &vocabulary, None),
            Ok(Validator::NumberRange {
                lower: None,
                upper: Some(9.0),
            })
        );
    }

    #[test]
    fn a_document_is_refused_six_ways() {
        let vocabulary = vocabulary();
        assert_eq!(
            Validator::decode(&json!(4), &vocabulary, None),
            Err(ValidatorError::NotAnObject)
        );
        assert_eq!(
            Validator::decode(&json!({ "kind": "nothing" }), &vocabulary, None),
            Err(ValidatorError::UnknownKind("nothing".to_owned()))
        );
        assert_eq!(
            Validator::decode(&json!({ "kind": "equals" }), &vocabulary, None),
            Err(ValidatorError::FieldMissing("text".to_owned()))
        );
        assert_eq!(
            Validator::decode(&json!({ "kind": "equals", "text": 4 }), &vocabulary, None),
            Err(ValidatorError::WrongShape("text".to_owned()))
        );
        assert_eq!(
            Validator::decode(
                &json!({ "substring": "a" }),
                &vocabulary,
                Some(Family::Text)
            ),
            Err(ValidatorError::FieldMissing("text".to_owned())),
            "the context supplies the family, not the kind"
        );
        assert!(matches!(
            Validator::decode(
                &json!({ "kind": "pattern", "expression": "(a)" }),
                &vocabulary,
                None
            ),
            Err(ValidatorError::Pattern(_))
        ));
    }

    #[test]
    fn a_document_that_nests_past_the_ceiling_is_refused_before_the_stack_is() {
        let vocabulary = vocabulary();
        let mut document = json!({ "kind": "equals", "text": "a" });
        for _ in 0..(super::MAX_DEPTH + 2) {
            document = json!({ "kind": "universal", "element": document });
        }
        assert_eq!(
            Validator::decode(&document, &vocabulary, None),
            Err(ValidatorError::TooDeep(super::MAX_DEPTH))
        );
    }

    #[test]
    fn a_document_wider_than_the_ceiling_is_refused_before_it_allocates() {
        let vocabulary = vocabulary();
        let elements: Vec<_> = (0..=super::MAX_WIDTH)
            .map(|_| json!({ "kind": "equals", "text": "a" }))
            .collect();
        let document = json!({ "kind": "ordered", "elements": elements });
        assert_eq!(
            Validator::decode(&document, &vocabulary, None),
            Err(ValidatorError::TooWide(super::MAX_WIDTH + 1))
        );
    }

    #[test]
    fn a_nested_element_must_name_its_own_kind_because_no_context_flows_into_it() {
        let vocabulary = vocabulary();
        let document = json!({ "kind": "ordered", "elements": [{ "text": "a" }] });
        assert_eq!(
            Validator::decode(&document, &vocabulary, None),
            Err(ValidatorError::FamilyNotNamed)
        );
    }
}
