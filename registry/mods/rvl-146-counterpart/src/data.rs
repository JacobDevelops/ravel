//! The codec helpers of §10: the small shapes a dependent composes with.
//!
//! A pair, a map built from pairs, a list wire form, an optional wire form, a
//! compound wire form, and a data-component type pairing a persistent declared
//! form with a network one.
//!
//! **The pair's two field spellings are not invented here.** §14 makes them
//! compatibility-required, because they appear in any data written through the
//! pair, and withholds them. [`PairSpellings::declared`] reads them from
//! build-time inputs and [`PairSpellings::placeholder`] carries this package's
//! own.
//!
//! # The changes declared here
//!
//! - **Encoding a map sorts by key.** The original iterates the map, so the
//!   output order is the map's iteration order, which for the default unordered
//!   map is not stable across runs. Two encodings of one map would differ, and a
//!   parity test over the bytes would pass or fail by luck. [`write_map`] sorts,
//!   so a map has one encoding.
//! - **The list wire form is bounded.** The original reads exactly the declared
//!   size with no upper bound, so on a client-to-server path a declared size of
//!   two billion loops two billion times. [`read_list`] refuses a declared count
//!   past [`MAX_LIST_ELEMENTS`] before it allocates anything.
//! - **Each element carries its own four-byte length.** §10's list is a size then
//!   the elements, with the element's own codec deciding where each one ends.
//!   Ravel publishes no such codec, so an element here is opaque bytes, and
//!   opaque bytes cannot say where they end: the length in front of each one is
//!   this package's framing rather than one it obtained, and a peer writing
//!   §10's shape over a codec of its own does not read here. It is what bounds
//!   an element as well. This is named so that it arrives as a known gap rather
//!   than a surprise; the upgrade path is an element codec, once there is one to
//!   take.
//!
//! What is kept is that a repeated key keeps its last occurrence, because
//! decoding inserts in list order and that is observable behaviour rather than a
//! defect.

use std::collections::HashMap;
use std::fmt;

use ravel_sdk::JsonValue;

/// The most elements a list wire form may declare.
pub const MAX_LIST_ELEMENTS: usize = 65_536;

/// The most bytes one list element may carry.
pub const MAX_ELEMENT_BYTES: usize = 1 << 20;

/// Why the pair spellings could not be built.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairSpellingsUnavailable {
    /// The build supplied nothing.
    NotSupplied,
    /// A spelling is empty.
    Empty,
    /// Both spellings are one word, which would make a pair carry one field.
    NotDistinct,
}

impl fmt::Display for PairSpellingsUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSupplied => formatter.write_str("the build supplied no pair spellings"),
            Self::Empty => formatter.write_str("a pair spelling is empty"),
            Self::NotDistinct => formatter.write_str("both pair spellings are one word"),
        }
    }
}

impl std::error::Error for PairSpellingsUnavailable {}

/// The two field names a pair is written under.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairSpellings {
    first: String,
    second: String,
}

impl Default for PairSpellings {
    fn default() -> Self {
        Self::placeholder()
    }
}

impl PairSpellings {
    /// The spellings the tests pin the document's **shape** with.
    #[must_use]
    pub fn placeholder() -> Self {
        Self {
            first: "left".to_owned(),
            second: "right".to_owned(),
        }
    }

    /// Reads the two spellings from build-time inputs.
    ///
    /// # Errors
    ///
    /// Returns [`PairSpellingsUnavailable::NotSupplied`] when the build supplied
    /// nothing, and what [`PairSpellings::parse`] returns otherwise.
    pub fn declared() -> Result<Self, PairSpellingsUnavailable> {
        let first = option_env!("RVL_146_COUNTERPART_PAIR_FIRST_KEY")
            .ok_or(PairSpellingsUnavailable::NotSupplied)?;
        let second = option_env!("RVL_146_COUNTERPART_PAIR_SECOND_KEY")
            .ok_or(PairSpellingsUnavailable::NotSupplied)?;
        Self::parse(first, second)
    }

    /// Reads two spellings.
    ///
    /// # Errors
    ///
    /// Returns [`PairSpellingsUnavailable::Empty`] for an empty spelling and
    /// [`PairSpellingsUnavailable::NotDistinct`] when both are one word.
    pub fn parse(first: &str, second: &str) -> Result<Self, PairSpellingsUnavailable> {
        if first.is_empty() || second.is_empty() {
            return Err(PairSpellingsUnavailable::Empty);
        }
        if first == second {
            return Err(PairSpellingsUnavailable::NotDistinct);
        }
        Ok(Self {
            first: first.to_owned(),
            second: second.to_owned(),
        })
    }

    /// Returns the first field's name.
    #[must_use]
    pub fn first(&self) -> &str {
        &self.first
    }

    /// Returns the second field's name.
    #[must_use]
    pub fn second(&self) -> &str {
        &self.second
    }
}

/// Why a document did not read as the shape it was asked for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentRefused {
    /// The document is not an object.
    NotAnObject,
    /// The document is not a list.
    NotAList,
    /// A named field is missing.
    Missing(String),
    /// A named field is present and is not text.
    NotText(String),
}

impl fmt::Display for DocumentRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAnObject => formatter.write_str("the document is not an object"),
            Self::NotAList => formatter.write_str("the document is not a list"),
            Self::Missing(key) => write!(formatter, "`{key}` is missing"),
            Self::NotText(key) => write!(formatter, "`{key}` is not text"),
        }
    }
}

impl std::error::Error for DocumentRefused {}

/// Writes a pair as an object with the two declared field names.
#[must_use]
pub fn write_pair(spellings: &PairSpellings, first: &str, second: &str) -> JsonValue {
    let mut document = ravel_sdk::json!({});
    document[spellings.first()] = JsonValue::String(first.to_owned());
    document[spellings.second()] = JsonValue::String(second.to_owned());
    document
}

/// Reads a pair.
///
/// # Errors
///
/// Returns [`DocumentRefused::NotAnObject`], [`DocumentRefused::Missing`] and
/// [`DocumentRefused::NotText`].
pub fn read_pair(
    spellings: &PairSpellings,
    document: &JsonValue,
) -> Result<(String, String), DocumentRefused> {
    if !document.is_object() {
        return Err(DocumentRefused::NotAnObject);
    }
    Ok((
        field(document, spellings.first())?,
        field(document, spellings.second())?,
    ))
}

fn field(document: &JsonValue, key: &str) -> Result<String, DocumentRefused> {
    let value = document
        .get(key)
        .ok_or_else(|| DocumentRefused::Missing(key.to_owned()))?;
    value
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| DocumentRefused::NotText(key.to_owned()))
}

/// Writes a map as a list of pairs, in key order.
///
/// The map is the unordered one §10's decoder defaults to, whose iteration order
/// is not stable across runs, and the sort here is what gives it one encoding.
#[must_use]
pub fn write_map<S: std::hash::BuildHasher>(
    spellings: &PairSpellings,
    entries: &HashMap<String, String, S>,
) -> JsonValue {
    let mut sorted: Vec<(&String, &String)> = entries.iter().collect();
    sorted.sort_by(|left, right| left.0.cmp(right.0));
    JsonValue::Array(
        sorted
            .into_iter()
            .map(|(key, value)| write_pair(spellings, key, value))
            .collect(),
    )
}

/// Reads a map from a list of pairs, inserting in list order.
///
/// A repeated key keeps its last occurrence, which is what inserting in order
/// means and is kept deliberately.
///
/// # Errors
///
/// Returns [`DocumentRefused::NotAList`] for a document that is not a list, and
/// whatever [`read_pair`] returns for an element.
pub fn read_map(
    spellings: &PairSpellings,
    document: &JsonValue,
) -> Result<HashMap<String, String>, DocumentRefused> {
    let elements = document.as_array().ok_or(DocumentRefused::NotAList)?;
    let mut entries = HashMap::new();
    for element in elements {
        let (key, value) = read_pair(spellings, element)?;
        entries.insert(key, value);
    }
    Ok(entries)
}

/// Why a wire form was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WireRefused {
    /// More elements than [`MAX_LIST_ELEMENTS`], refused before anything is
    /// allocated.
    TooManyElements(u32),
    /// An element longer than [`MAX_ELEMENT_BYTES`].
    ElementTooLong(u32),
    /// The buffer ended inside a field.
    Truncated,
    /// The buffer held bytes nothing accounted for.
    TrailingBytes(usize),
    /// A boolean byte was neither zero nor one.
    NotABool(u8),
    /// A compound read yielded nothing where the form demands a value.
    NoCompound,
}

impl fmt::Display for WireRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyElements(count) => {
                write!(formatter, "{count} elements is past {MAX_LIST_ELEMENTS}")
            }
            Self::ElementTooLong(length) => {
                write!(formatter, "{length} bytes is past {MAX_ELEMENT_BYTES}")
            }
            Self::Truncated => formatter.write_str("the buffer ended inside a field"),
            Self::TrailingBytes(unread) => write!(formatter, "{unread} bytes were left unread"),
            Self::NotABool(byte) => write!(formatter, "{byte} is neither zero nor one"),
            Self::NoCompound => formatter.write_str("a compound form read nothing"),
        }
    }
}

impl std::error::Error for WireRefused {}

/// Writes a list: a four-byte count, then each element under a four-byte length.
///
/// The per-element length is this package's framing and not §10's, which is
/// declared in the module documentation above.
///
/// # Errors
///
/// Returns [`WireRefused::TooManyElements`] past [`MAX_LIST_ELEMENTS`] and
/// [`WireRefused::ElementTooLong`] past [`MAX_ELEMENT_BYTES`].
pub fn write_list(elements: &[Vec<u8>]) -> Result<Vec<u8>, WireRefused> {
    let count =
        u32::try_from(elements.len()).map_err(|_| WireRefused::TooManyElements(u32::MAX))?;
    if elements.len() > MAX_LIST_ELEMENTS {
        return Err(WireRefused::TooManyElements(count));
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&count.to_be_bytes());
    for element in elements {
        let length =
            u32::try_from(element.len()).map_err(|_| WireRefused::ElementTooLong(u32::MAX))?;
        if element.len() > MAX_ELEMENT_BYTES {
            return Err(WireRefused::ElementTooLong(length));
        }
        bytes.extend_from_slice(&length.to_be_bytes());
        bytes.extend_from_slice(element);
    }
    Ok(bytes)
}

/// Reads a list.
///
/// # Errors
///
/// Returns [`WireRefused::TooManyElements`] for a declared count past
/// [`MAX_LIST_ELEMENTS`], refused before anything is allocated,
/// [`WireRefused::ElementTooLong`], [`WireRefused::Truncated`] and
/// [`WireRefused::TrailingBytes`].
pub fn read_list(bytes: &[u8]) -> Result<Vec<Vec<u8>>, WireRefused> {
    let mut at = 0;
    let count = take_u32(bytes, &mut at)?;
    let count_index = usize::try_from(count).map_err(|_| WireRefused::TooManyElements(count))?;
    if count_index > MAX_LIST_ELEMENTS {
        return Err(WireRefused::TooManyElements(count));
    }
    let mut elements = Vec::with_capacity(count_index);
    for _ in 0..count {
        let length = take_u32(bytes, &mut at)?;
        let length_index =
            usize::try_from(length).map_err(|_| WireRefused::ElementTooLong(length))?;
        if length_index > MAX_ELEMENT_BYTES {
            return Err(WireRefused::ElementTooLong(length));
        }
        let end = at.checked_add(length_index).ok_or(WireRefused::Truncated)?;
        elements.push(bytes.get(at..end).ok_or(WireRefused::Truncated)?.to_vec());
        at = end;
    }
    let unread = bytes.len() - at;
    if unread > 0 {
        return Err(WireRefused::TrailingBytes(unread));
    }
    Ok(elements)
}

fn take_u32(bytes: &[u8], at: &mut usize) -> Result<u32, WireRefused> {
    let end = at.checked_add(4).ok_or(WireRefused::Truncated)?;
    let taken: [u8; 4] = bytes
        .get(*at..end)
        .ok_or(WireRefused::Truncated)?
        .try_into()
        .map_err(|_| WireRefused::Truncated)?;
    *at = end;
    Ok(u32::from_be_bytes(taken))
}

/// Writes an optional value: a boolean, then the value when there is one.
#[must_use]
pub fn write_optional(value: Option<&[u8]>) -> Vec<u8> {
    value.map_or_else(
        || vec![0],
        |present| {
            let mut bytes = Vec::with_capacity(present.len() + 1);
            bytes.push(1);
            bytes.extend_from_slice(present);
            bytes
        },
    )
}

/// Reads an optional value.
///
/// # Errors
///
/// Returns [`WireRefused::Truncated`] for an empty buffer and
/// [`WireRefused::NotABool`] for a flag byte that is neither zero nor one.
pub fn read_optional(bytes: &[u8]) -> Result<Option<Vec<u8>>, WireRefused> {
    match bytes.first().ok_or(WireRefused::Truncated)? {
        0 => {
            if bytes.len() > 1 {
                return Err(WireRefused::TrailingBytes(bytes.len() - 1));
            }
            Ok(None)
        }
        1 => Ok(Some(bytes[1..].to_vec())),
        other => Err(WireRefused::NotABool(*other)),
    }
}

/// Reads a compound form, which demands a value.
///
/// # Errors
///
/// Returns [`WireRefused::NoCompound`] when the host's read yielded nothing.
pub fn read_compound(read: Option<Vec<u8>>) -> Result<Vec<u8>, WireRefused> {
    read.ok_or(WireRefused::NoCompound)
}

/// A data-component type: a persistent declared form and a network form.
///
/// **Declarable only.** Ravel publishes no data-component registration and no
/// codec for either form, so what this holds is the two forms' names and the
/// pairing between them, which is the part §4 registers under an identifier and
/// the part a host would need. Nothing here encodes a component.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentForm {
    persistent: String,
    network: String,
}

impl ComponentForm {
    /// Pairs the two forms.
    #[must_use]
    pub fn pairing(persistent: &str, network: &str) -> Self {
        Self {
            persistent: persistent.to_owned(),
            network: network.to_owned(),
        }
    }

    /// The one form used for both, which is the ordinary case.
    #[must_use]
    pub fn same(both: &str) -> Self {
        Self::pairing(both, both)
    }

    /// Returns the form a world is written with.
    #[must_use]
    pub fn persistent(&self) -> &str {
        &self.persistent
    }

    /// Returns the form a peer is written with.
    #[must_use]
    pub fn network(&self) -> &str {
        &self.network
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use ravel_sdk::{JsonValue, json};

    use super::{
        ComponentForm, DocumentRefused, MAX_ELEMENT_BYTES, MAX_LIST_ELEMENTS, PairSpellings,
        PairSpellingsUnavailable, WireRefused, read_compound, read_list, read_map, read_optional,
        read_pair, write_list, write_map, write_optional, write_pair,
    };

    fn spellings() -> PairSpellings {
        PairSpellings::placeholder()
    }

    #[test]
    fn a_pair_is_written_and_read_under_the_declared_spellings() {
        let document = write_pair(&spellings(), "one", "two");
        assert_eq!(document[spellings().first()], json!("one"));
        assert_eq!(
            read_pair(&spellings(), &document),
            Ok(("one".to_owned(), "two".to_owned()))
        );
    }

    #[test]
    fn supplied_spellings_change_the_document_a_pack_author_writes() {
        let theirs = PairSpellings::parse("k", "v").expect("two distinct words");
        assert_ne!(
            write_pair(&spellings(), "one", "two"),
            write_pair(&theirs, "one", "two")
        );
        assert_eq!(
            read_pair(&theirs, &write_pair(&theirs, "one", "two")),
            Ok(("one".to_owned(), "two".to_owned()))
        );
        assert_eq!(
            read_pair(&spellings(), &write_pair(&theirs, "one", "two")),
            Err(DocumentRefused::Missing(spellings().first().to_owned()))
        );
    }

    #[test]
    fn three_ways_a_supplied_pair_spelling_is_refused_and_the_declared_one_is_absent() {
        assert_eq!(
            PairSpellings::parse("", "b"),
            Err(PairSpellingsUnavailable::Empty)
        );
        assert_eq!(
            PairSpellings::parse("a", ""),
            Err(PairSpellingsUnavailable::Empty)
        );
        assert_eq!(
            PairSpellings::parse("a", "a"),
            Err(PairSpellingsUnavailable::NotDistinct)
        );
        assert_eq!(
            PairSpellings::declared(),
            Err(PairSpellingsUnavailable::NotSupplied)
        );
    }

    #[test]
    fn a_document_that_is_not_the_shape_it_was_asked_for_is_refused() {
        assert_eq!(
            read_pair(&spellings(), &json!([])),
            Err(DocumentRefused::NotAnObject)
        );
        let mut wrong = json!({});
        wrong[spellings().first()] = json!(7);
        wrong[spellings().second()] = json!("two");
        assert_eq!(
            read_pair(&spellings(), &wrong),
            Err(DocumentRefused::NotText(spellings().first().to_owned()))
        );
        assert_eq!(
            read_map(&spellings(), &json!({})),
            Err(DocumentRefused::NotAList)
        );
    }

    #[test]
    fn one_map_has_one_encoding_whatever_order_it_was_built_in() {
        // An unordered map, so the order the pairs come out in is the code's
        // rather than the type's: a map whose own iteration order differs run to
        // run still encodes one way.
        let keys = ["m", "a", "z", "b", "q"];
        let mut first: HashMap<String, String> = HashMap::new();
        for key in keys {
            first.insert(key.to_owned(), key.to_uppercase());
        }
        let mut second: HashMap<String, String> = HashMap::new();
        for key in keys.iter().rev() {
            second.insert((*key).to_owned(), key.to_uppercase());
        }
        let encoded = write_map(&spellings(), &first);
        assert_eq!(encoded, write_map(&spellings(), &second));
        assert_eq!(
            encoded
                .as_array()
                .expect("a list of pairs")
                .iter()
                .map(|pair| pair[spellings().first()]
                    .as_str()
                    .expect("a written key")
                    .to_owned())
                .collect::<Vec<_>>(),
            ["a", "b", "m", "q", "z"],
            "sorted on encode, which is what gives it the one encoding"
        );
        assert_eq!(read_map(&spellings(), &encoded), Ok(first));
    }

    #[test]
    fn a_repeated_key_keeps_its_last_occurrence() {
        let document = JsonValue::Array(vec![
            write_pair(&spellings(), "a", "first"),
            write_pair(&spellings(), "a", "last"),
        ]);
        let read = read_map(&spellings(), &document).expect("two well-formed pairs");
        assert_eq!(read.get("a").map(String::as_str), Some("last"));
        assert_eq!(read.len(), 1);
    }

    #[test]
    fn a_list_round_trips_and_an_empty_one_is_a_list() {
        let elements = vec![vec![1, 2], Vec::new(), vec![3]];
        let bytes = write_list(&elements).expect("three small elements");
        assert_eq!(read_list(&bytes), Ok(elements));
        assert_eq!(
            read_list(&write_list(&[]).expect("no elements")),
            Ok(vec![])
        );
    }

    #[test]
    fn a_declared_element_count_past_the_bound_is_refused_before_anything_is_allocated() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2_000_000_000u32.to_be_bytes());
        assert_eq!(
            read_list(&bytes),
            Err(WireRefused::TooManyElements(2_000_000_000))
        );
    }

    #[test]
    fn a_declared_element_length_past_the_bound_is_refused_and_so_is_a_truncated_list() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.extend_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(
            read_list(&bytes),
            Err(WireRefused::ElementTooLong(u32::MAX))
        );

        let mut short = Vec::new();
        short.extend_from_slice(&1u32.to_be_bytes());
        short.extend_from_slice(&4u32.to_be_bytes());
        short.push(1);
        assert_eq!(read_list(&short), Err(WireRefused::Truncated));
        assert_eq!(read_list(&[]), Err(WireRefused::Truncated));
    }

    #[test]
    fn a_list_with_bytes_left_over_is_refused() {
        let mut bytes = write_list(&[vec![1]]).expect("one element");
        bytes.push(0);
        assert_eq!(read_list(&bytes), Err(WireRefused::TrailingBytes(1)));
    }

    #[test]
    fn the_writer_refuses_a_list_and_an_element_past_the_bounds_too() {
        let many = vec![Vec::new(); MAX_LIST_ELEMENTS + 1];
        assert!(matches!(
            write_list(&many),
            Err(WireRefused::TooManyElements(_))
        ));
        let long = vec![vec![0u8; MAX_ELEMENT_BYTES + 1]];
        assert!(matches!(
            write_list(&long),
            Err(WireRefused::ElementTooLong(_))
        ));
    }

    #[test]
    fn an_optional_carries_a_boolean_and_then_the_value() {
        assert_eq!(read_optional(&write_optional(None)), Ok(None));
        assert_eq!(
            read_optional(&write_optional(Some(&[7, 8]))),
            Ok(Some(vec![7, 8]))
        );
        assert_eq!(read_optional(&[]), Err(WireRefused::Truncated));
        assert_eq!(read_optional(&[2]), Err(WireRefused::NotABool(2)));
        assert_eq!(
            read_optional(&[0, 9]),
            Err(WireRefused::TrailingBytes(1)),
            "an absent value carries nothing after its flag"
        );
    }

    #[test]
    fn a_compound_form_tells_an_empty_compound_from_no_compound() {
        // §5.1's compound arm stores whatever the host's read yields, absent
        // included, so a handler expecting a tag can receive none. The
        // distinction that has to survive is between a compound that carries
        // nothing and no compound at all: the first is a value and the second is
        // a refusal.
        assert_eq!(read_compound(Some(vec![1])), Ok(vec![1]));
        assert_eq!(
            read_compound(Some(Vec::new())),
            Ok(Vec::new()),
            "an empty compound is a compound"
        );
        assert_eq!(read_compound(None), Err(WireRefused::NoCompound));
    }

    #[test]
    fn a_component_type_pairs_a_persistent_form_with_a_network_one() {
        let paired = ComponentForm::pairing("stored", "sent");
        assert_eq!(paired.persistent(), "stored");
        assert_eq!(paired.network(), "sent");
        let same = ComponentForm::same("both");
        assert_eq!(same.persistent(), same.network());
    }
}
