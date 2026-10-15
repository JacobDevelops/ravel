//! The named values a screen stays current against, from §8.
//!
//! A block entity exposes an ordered map of named values and a screen
//! subscribes to it. The wire carries no names and no count, so the two sides
//! agree only because they walk the same map in the same order: the encoding is
//! positional, and that makes the order a wire format rather than a
//! presentation detail. The map here is insertion-ordered for exactly that
//! reason, and it never iterates a hash.
//!
//! The dirty rule is deliberately coarse. One difference makes the whole set
//! dirty, every value is re-copied, and dirtiness is reported once; there is no
//! per-value tracking to disagree with the positional write.

use std::fmt;

/// A value a block entity publishes to the screen open on it.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A signed 32-bit number.
    Integer(i32),
    /// A signed 16-bit number.
    Short(i16),
    /// A truth value.
    Boolean(bool),
    /// A single-precision number.
    Float(f32),
    /// Text.
    Text(String),
    /// One of a fixed set, carried as an index into it.
    Enumeration {
        /// Which of the set this is.
        index: u8,
        /// How many the set holds.
        count: u8,
    },
}

impl Value {
    /// The most bytes a text value may carry.
    pub const MAX_TEXT: usize = 32_768;

    /// Returns whether two values are the same value.
    ///
    /// A float is compared by its bit pattern rather than by `PartialEq`. A map
    /// holding a NaN would otherwise never equal its own remembered copy, and
    /// the subscription below would report dirty on every check for the life of
    /// the screen instead of once per change.
    #[must_use]
    pub fn is(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Float(left), Self::Float(right)) => left.to_bits() == right.to_bits(),
            _ => self == other,
        }
    }

    /// Returns the tag the wire writes for this value's type.
    const fn tag(&self) -> u8 {
        match self {
            Self::Integer(_) => 0,
            Self::Short(_) => 1,
            Self::Boolean(_) => 2,
            Self::Float(_) => 3,
            Self::Text(_) => 4,
            Self::Enumeration { .. } => 5,
        }
    }
}

/// The ordered map of named values a block entity publishes.
///
/// Order is insertion order and nothing re-sorts it, because the reader walks
/// positionally. Changing the declared set is therefore a wire-format change,
/// which [`ValueMap::shape`] makes checkable rather than assumed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValueMap {
    entries: Vec<(String, Value)>,
}

impl ValueMap {
    /// Builds an empty map.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Declares a named value, or replaces the one already under that name.
    ///
    /// A replacement keeps the position the name already held, because moving a
    /// name would silently renumber every value after it.
    pub fn declare(&mut self, name: &str, value: Value) {
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|(existing, _)| existing == name)
        {
            existing.1 = value;
            return;
        }
        self.entries.push((name.to_owned(), value));
    }

    /// Returns the value under `name`.
    #[must_use]
    pub fn value(&self, name: &str) -> Option<&Value> {
        self.entries
            .iter()
            .find(|(existing, _)| existing == name)
            .map(|(_, value)| value)
    }

    /// Returns every value, in map order.
    #[must_use]
    pub fn values(&self) -> Vec<&Value> {
        self.entries.iter().map(|(_, value)| value).collect()
    }

    /// Returns every name, in map order.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.entries.iter().map(|(name, _)| name.as_str()).collect()
    }

    /// Returns how many values the map holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the map holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the map's shape: the ordered names and their types.
    ///
    /// Two sides agree on the wire when their shapes agree. Comparing shapes is
    /// what turns "the declared value set changed" from a silent disagreement
    /// into a checkable one.
    #[must_use]
    pub fn shape(&self) -> Vec<(String, u8)> {
        self.entries
            .iter()
            .map(|(name, value)| (name.clone(), value.tag()))
            .collect()
    }
}

/// A screen's subscription to a block entity's value map.
#[derive(Clone, Debug, Default)]
pub struct Subscription {
    remembered: Option<Vec<Value>>,
}

impl Subscription {
    /// Builds a subscription that remembers nothing yet.
    #[must_use]
    pub const fn new() -> Self {
        Self { remembered: None }
    }

    /// Compares the map against what is remembered and re-copies if it differs.
    ///
    /// The first difference makes the whole set dirty and the answer is one
    /// verdict for the map, not one per value.
    pub fn check(&mut self, map: &ValueMap) -> bool {
        let current: Vec<Value> = map.values().into_iter().cloned().collect();
        let dirty = self.remembered.as_ref().is_none_or(|remembered| {
            remembered.len() != current.len()
                || !remembered
                    .iter()
                    .zip(&current)
                    .all(|(held, now)| held.is(now))
        });
        if dirty {
            self.remembered = Some(current);
        }
        dirty
    }

    /// Returns what the subscription last copied.
    #[must_use]
    pub fn remembered(&self) -> Option<&[Value]> {
        self.remembered.as_deref()
    }
}

/// Writes every value in map order, with no names and no count.
///
/// # Errors
///
/// Returns [`ValueRefused::TextTooLong`] for a text value longer than
/// [`read`] will admit. The bound belongs on both sides: a sender that wrote
/// one anyway would produce a buffer its own peer refuses, and the refusal
/// would surface at the far end rather than at the mistake.
pub fn write(map: &ValueMap) -> Result<Vec<u8>, ValueRefused> {
    let mut bytes = Vec::new();
    for value in map.values() {
        match value {
            Value::Integer(number) => bytes.extend_from_slice(&number.to_be_bytes()),
            Value::Short(number) => bytes.extend_from_slice(&number.to_be_bytes()),
            Value::Boolean(truth) => bytes.push(u8::from(*truth)),
            Value::Float(number) => bytes.extend_from_slice(&number.to_be_bytes()),
            Value::Text(text) => {
                let body = text.as_bytes();
                if body.len() > Value::MAX_TEXT {
                    return Err(ValueRefused::TextTooLong { length: body.len() });
                }
                bytes
                    .extend_from_slice(&crate::arithmetic::bounded_count(body.len()).to_be_bytes());
                bytes.extend_from_slice(body);
            }
            Value::Enumeration { index, .. } => bytes.push(*index),
        }
    }
    Ok(bytes)
}

/// Reads a positional write back into a map that already has the right shape.
///
/// The reader walks the map in the same order the writer did and assigns
/// positionally, which is why it is handed the map it is filling rather than
/// building one: nothing on the wire says what the values are called.
///
/// # Errors
///
/// Returns [`ValueRefused`] for a buffer that ends inside a value, carries text
/// that is not UTF-8 or longer than [`Value::MAX_TEXT`], or carries an
/// enumeration index outside the set the map declares. That last one is the
/// divergence §8 asks for: the original indexes its value array with no bounds
/// check and raises on a mismatch.
pub fn read(map: &mut ValueMap, bytes: &[u8]) -> Result<(), ValueRefused> {
    let mut read = 0_usize;
    let mut filled = Vec::with_capacity(map.entries.len());
    for (_, declared) in &map.entries {
        let value = match declared {
            Value::Integer(_) => {
                let taken = slice_at(bytes, &mut read, 4)?;
                Value::Integer(i32::from_be_bytes([taken[0], taken[1], taken[2], taken[3]]))
            }
            Value::Short(_) => {
                let taken = slice_at(bytes, &mut read, 2)?;
                Value::Short(i16::from_be_bytes([taken[0], taken[1]]))
            }
            Value::Boolean(_) => match slice_at(bytes, &mut read, 1)?[0] {
                0 => Value::Boolean(false),
                1 => Value::Boolean(true),
                byte => return Err(ValueRefused::NotABool { byte }),
            },
            Value::Float(_) => {
                let taken = slice_at(bytes, &mut read, 4)?;
                Value::Float(f32::from_be_bytes([taken[0], taken[1], taken[2], taken[3]]))
            }
            Value::Text(_) => {
                let header = slice_at(bytes, &mut read, 4)?;
                let announced = i32::from_be_bytes([header[0], header[1], header[2], header[3]]);
                let length = usize::try_from(announced).map_err(|_| ValueRefused::Truncated)?;
                if length > Value::MAX_TEXT {
                    return Err(ValueRefused::TextTooLong { length });
                }
                let body = slice_at(bytes, &mut read, length)?;
                Value::Text(
                    std::str::from_utf8(body)
                        .map_err(|_| ValueRefused::NotText)?
                        .to_owned(),
                )
            }
            Value::Enumeration { count, .. } => {
                let index = slice_at(bytes, &mut read, 1)?[0];
                if index >= *count {
                    return Err(ValueRefused::Index {
                        index,
                        count: *count,
                    });
                }
                Value::Enumeration {
                    index,
                    count: *count,
                }
            }
        };
        filled.push(value);
    }

    if read != bytes.len() {
        return Err(ValueRefused::Trailing);
    }
    for (entry, value) in map.entries.iter_mut().zip(filled) {
        entry.1 = value;
    }
    Ok(())
}

fn slice_at<'bytes>(
    bytes: &'bytes [u8],
    read: &mut usize,
    count: usize,
) -> Result<&'bytes [u8], ValueRefused> {
    let end = read.checked_add(count).ok_or(ValueRefused::Truncated)?;
    if end > bytes.len() {
        return Err(ValueRefused::Truncated);
    }
    let taken = &bytes[*read..end];
    *read = end;
    Ok(taken)
}

/// Why a positional write was not read back.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueRefused {
    /// The buffer ended inside a value.
    Truncated,
    /// The buffer carried bytes after the last declared value.
    Trailing,
    /// A boolean byte was neither zero nor one.
    NotABool {
        /// What the buffer carried.
        byte: u8,
    },
    /// A text value's bytes are not UTF-8.
    NotText,
    /// A text value is longer than the map admits.
    TextTooLong {
        /// How long the buffer said it was.
        length: usize,
    },
    /// An enumeration index is outside the set the map declares.
    Index {
        /// What the buffer carried.
        index: u8,
        /// How many the set holds.
        count: u8,
    },
}

impl fmt::Display for ValueRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => formatter.write_str("the write ended inside a value"),
            Self::Trailing => formatter.write_str("the write carried more than the map declares"),
            Self::NotABool { byte } => write!(formatter, "{byte} is neither zero nor one"),
            Self::NotText => formatter.write_str("a text value's bytes are not UTF-8"),
            Self::TextTooLong { length } => write!(
                formatter,
                "a text value announces {length} bytes and {} is the most one may carry",
                Value::MAX_TEXT
            ),
            Self::Index { index, count } => write!(
                formatter,
                "{index} is outside the {count} values the set declares"
            ),
        }
    }
}

impl std::error::Error for ValueRefused {}

/// Clamps an enumeration index into a declared set.
///
/// Where a caller would rather carry on than refuse, this is the other half of
/// the divergence: the index is brought inside the set instead of indexing past
/// its end. An empty set has no value to clamp to and answers nothing.
#[must_use]
pub const fn clamped_index(index: u8, count: u8) -> Option<u8> {
    if count == 0 {
        return None;
    }
    if index < count {
        Some(index)
    } else {
        Some(count - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ValueMap {
        let mut map = ValueMap::new();
        map.declare("count", Value::Integer(3));
        map.declare("level", Value::Short(-2));
        map.declare("running", Value::Boolean(true));
        map.declare("share", Value::Float(0.5));
        map.declare("label", Value::Text("here".to_owned()));
        map.declare("mode", Value::Enumeration { index: 1, count: 3 });
        map
    }

    #[test]
    fn the_map_keeps_declaration_order_and_never_sorts_it() {
        let map = sample();
        assert_eq!(
            map.names(),
            ["count", "level", "running", "share", "label", "mode"]
        );
        assert_eq!(map.len(), 6);
        assert!(!map.is_empty());
        assert!(ValueMap::new().is_empty());
    }

    #[test]
    fn redeclaring_a_name_keeps_the_position_it_already_held() {
        let mut map = sample();
        map.declare("count", Value::Integer(9));
        assert_eq!(map.names()[0], "count");
        assert_eq!(map.value("count"), Some(&Value::Integer(9)));
        assert_eq!(map.len(), 6);
    }

    #[test]
    fn the_first_difference_makes_the_whole_set_dirty_and_reports_once() {
        let mut map = sample();
        let mut subscription = Subscription::new();
        assert!(subscription.check(&map), "the first check always copies");
        assert!(!subscription.check(&map), "an unchanged map is not dirty");

        map.declare("count", Value::Integer(4));
        assert!(subscription.check(&map));
        assert!(
            !subscription.check(&map),
            "the copy is taken when the difference is found, so one change reports once"
        );
        assert_eq!(subscription.remembered().map(<[Value]>::len), Some(6));
    }

    #[test]
    fn a_change_to_a_later_value_is_still_one_verdict_for_the_whole_map() {
        let mut map = sample();
        let mut subscription = Subscription::new();
        assert!(subscription.check(&map));
        map.declare("label", Value::Text("elsewhere".to_owned()));
        assert!(subscription.check(&map));
        assert_eq!(
            subscription.remembered().map(<[Value]>::to_vec),
            Some(map.values().into_iter().cloned().collect::<Vec<_>>())
        );
    }

    #[test]
    fn every_value_round_trips_positionally_with_no_names_on_the_wire() {
        let map = sample();
        let bytes = write(&map).expect("every text value is inside the bound");
        let mut receiving = sample();
        receiving.declare("count", Value::Integer(0));
        receiving.declare("label", Value::Text(String::new()));

        read(&mut receiving, &bytes).expect("the same shape reads the same bytes");
        assert_eq!(receiving, map);
        assert_eq!(receiving.shape(), map.shape());
    }

    #[test]
    fn the_wire_carries_no_count_so_two_shapes_are_told_apart_only_by_the_shape() {
        let map = sample();
        let bytes = write(&map).expect("every text value is inside the bound");
        let mut shorter = ValueMap::new();
        shorter.declare("count", Value::Integer(0));
        assert_eq!(read(&mut shorter, &bytes), Err(ValueRefused::Trailing));
    }

    #[test]
    fn a_write_that_ends_inside_a_value_is_refused_rather_than_read_short() {
        let map = sample();
        let bytes = write(&map).expect("every text value is inside the bound");
        let mut receiving = sample();
        assert_eq!(
            read(&mut receiving, &bytes[..bytes.len() - 1]),
            Err(ValueRefused::Truncated)
        );
        assert_eq!(receiving, map, "a refused read leaves the map as it was");
    }

    #[test]
    fn an_enumeration_index_outside_the_declared_set_is_refused_rather_than_indexed() {
        let mut map = ValueMap::new();
        map.declare("mode", Value::Enumeration { index: 0, count: 3 });
        assert_eq!(
            read(&mut map, &[7]),
            Err(ValueRefused::Index { index: 7, count: 3 })
        );
        assert_eq!(
            read(&mut map, &[3]),
            Err(ValueRefused::Index { index: 3, count: 3 }),
            "the count itself is one past the end"
        );
        read(&mut map, &[2]).expect("the last of three is inside the set");
    }

    #[test]
    fn clamping_brings_an_index_inside_the_set_and_answers_nothing_for_an_empty_one() {
        assert_eq!(clamped_index(0, 3), Some(0));
        assert_eq!(clamped_index(2, 3), Some(2));
        assert_eq!(clamped_index(3, 3), Some(2));
        assert_eq!(clamped_index(255, 3), Some(2));
        assert_eq!(clamped_index(0, 0), None);
    }

    #[test]
    fn a_boolean_that_is_neither_zero_nor_one_is_refused() {
        let mut map = ValueMap::new();
        map.declare("running", Value::Boolean(false));
        assert_eq!(
            read(&mut map, &[2]),
            Err(ValueRefused::NotABool { byte: 2 })
        );
    }

    #[test]
    fn a_text_value_announcing_more_than_it_carries_is_refused_before_it_allocates() {
        let mut map = ValueMap::new();
        map.declare("label", Value::Text(String::new()));

        let mut lying = i32::MAX.to_be_bytes().to_vec();
        lying.extend_from_slice(b"four");
        assert_eq!(
            read(&mut map, &lying),
            Err(ValueRefused::TextTooLong {
                length: 2_147_483_647
            })
        );

        let mut negative = (-1_i32).to_be_bytes().to_vec();
        negative.extend_from_slice(b"four");
        assert_eq!(read(&mut map, &negative), Err(ValueRefused::Truncated));

        let mut not_text = 2_i32.to_be_bytes().to_vec();
        not_text.extend_from_slice(&[0xFF, 0xFE]);
        assert_eq!(read(&mut map, &not_text), Err(ValueRefused::NotText));
    }

    #[test]
    fn a_value_that_is_never_equal_to_itself_still_reports_dirty_once() {
        let mut map = ValueMap::new();
        map.declare("share", Value::Float(f32::NAN));
        let mut subscription = Subscription::new();
        assert!(subscription.check(&map), "the first check always copies");
        assert!(
            !subscription.check(&map),
            "a value that never equals itself would otherwise re-send the whole set every tick"
        );

        map.declare("share", Value::Float(0.5));
        assert!(subscription.check(&map), "a real change is still a change");
        assert!(!subscription.check(&map));
    }

    #[test]
    fn two_floats_that_differ_only_in_their_bit_pattern_are_not_the_same_value() {
        assert!(Value::Float(0.0).is(&Value::Float(0.0)));
        assert!(!Value::Float(0.0).is(&Value::Float(-0.0)));
        assert!(Value::Integer(1).is(&Value::Integer(1)));
        assert!(!Value::Integer(1).is(&Value::Short(1)));
    }

    #[test]
    fn a_text_value_longer_than_a_reader_admits_is_refused_at_the_write() {
        let mut map = ValueMap::new();
        map.declare("label", Value::Text("t".repeat(Value::MAX_TEXT + 1)));
        assert_eq!(
            write(&map),
            Err(ValueRefused::TextTooLong {
                length: Value::MAX_TEXT + 1
            }),
            "the sender is refused rather than its peer"
        );

        map.declare("label", Value::Text("t".repeat(Value::MAX_TEXT)));
        assert!(write(&map).is_ok(), "the ceiling itself is admitted");
    }

    #[test]
    fn the_shape_changes_when_the_declared_set_does_which_is_a_wire_format_change() {
        let map = sample();
        let mut widened = sample();
        widened.declare("extra", Value::Integer(0));
        assert_ne!(map.shape(), widened.shape());

        let mut retyped = sample();
        retyped.entries[0].1 = Value::Float(3.0);
        assert_ne!(map.shape(), retyped.shape());
    }
}
