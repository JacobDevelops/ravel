//! The stored-data model and the comparison a pack author writes against, from
//! §7.3.
//!
//! Stored data is the host's own tree of tags. Ravel publishes no such type, so
//! the tree is modelled here as portable data and the comparison is portable
//! logic over it. That is the whole of what §7.3 is: a set of rules a pack
//! author relies on, which makes them a wire-visible contract rather than an
//! implementation.
//!
//! Two modes exist. The strict one delegates to the host's own comparison, which
//! here is structural equality. The lenient one is the entry's own and is the
//! behaviour worth having:
//!
//! - a null expectation matches anything, a null candidate against a non-null
//!   expectation fails;
//! - two numeric tags compare by decimal value, so a byte and a long holding the
//!   same number match;
//! - two non-numeric tags of different kinds fail;
//! - a compound holds when every key it names is present and matches, and extra
//!   keys in the candidate are ignored;
//! - a list holds when every element matches *some* element of the candidate, in
//!   any order and with repeats allowed, and an empty expected list requires an
//!   empty candidate;
//! - anything else compares by equality.

use std::collections::BTreeMap;

use num_traits::ToPrimitive;

/// One node of a stored-data tree.
///
/// The three array kinds are separate from the general list because §7.4's list
/// family deliberately admits all four.
#[derive(Clone, Debug, PartialEq)]
pub enum Tag {
    /// A signed eight-bit number.
    Byte(i8),
    /// A signed sixteen-bit number.
    Short(i16),
    /// A signed thirty-two-bit number.
    Int(i32),
    /// A signed sixty-four-bit number.
    Long(i64),
    /// A single-precision number.
    Float(f32),
    /// A double-precision number.
    Double(f64),
    /// A text value.
    Text(String),
    /// A general list.
    List(Vec<Self>),
    /// An array of bytes.
    ByteArray(Vec<i8>),
    /// An array of thirty-two-bit numbers.
    IntArray(Vec<i32>),
    /// An array of sixty-four-bit numbers.
    LongArray(Vec<i64>),
    /// A map from name to tag.
    Compound(BTreeMap<String, Self>),
}

impl Tag {
    /// Builds a compound from name and tag pairs.
    #[must_use]
    pub fn compound<I, K>(entries: I) -> Self
    where
        I: IntoIterator<Item = (K, Self)>,
        K: Into<String>,
    {
        Self::Compound(
            entries
                .into_iter()
                .map(|(name, tag)| (name.into(), tag))
                .collect(),
        )
    }

    /// Returns the tag's kind, which is what the family checks read.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        match self {
            Self::Byte(_) => Kind::Byte,
            Self::Short(_) => Kind::Short,
            Self::Int(_) => Kind::Int,
            Self::Long(_) => Kind::Long,
            Self::Float(_) => Kind::Float,
            Self::Double(_) => Kind::Double,
            Self::Text(_) => Kind::Text,
            Self::List(_) => Kind::List,
            Self::ByteArray(_) => Kind::ByteArray,
            Self::IntArray(_) => Kind::IntArray,
            Self::LongArray(_) => Kind::LongArray,
            Self::Compound(_) => Kind::Compound,
        }
    }

    /// Returns this tag's decimal value when it is numeric.
    ///
    /// This is what makes a byte and a long holding the same number match under
    /// the lenient mode.
    #[must_use]
    pub fn decimal(&self) -> Option<f64> {
        match *self {
            Self::Byte(value) => Some(f64::from(value)),
            Self::Short(value) => Some(f64::from(value)),
            Self::Int(value) => Some(f64::from(value)),
            Self::Long(value) => Some(value.to_f64().expect("i64 values fit the f64 range")),
            Self::Float(value) => Some(f64::from(value)),
            Self::Double(value) => Some(value),
            Self::Text(_)
            | Self::List(_)
            | Self::ByteArray(_)
            | Self::IntArray(_)
            | Self::LongArray(_)
            | Self::Compound(_) => None,
        }
    }

    /// Returns whether this tag is numeric.
    #[must_use]
    pub const fn is_numeric(&self) -> bool {
        self.kind().is_numeric()
    }

    /// Returns the elements of any of the four list-shaped kinds.
    ///
    /// The three primitive arrays are admitted alongside the general list,
    /// because §7.4's list family admits all four.
    #[must_use]
    pub fn elements(&self) -> Option<Vec<Self>> {
        match self {
            Self::List(items) => Some(items.clone()),
            Self::ByteArray(items) => Some(items.iter().copied().map(Self::Byte).collect()),
            Self::IntArray(items) => Some(items.iter().copied().map(Self::Int).collect()),
            Self::LongArray(items) => Some(items.iter().copied().map(Self::Long).collect()),
            Self::Byte(_)
            | Self::Short(_)
            | Self::Int(_)
            | Self::Long(_)
            | Self::Float(_)
            | Self::Double(_)
            | Self::Text(_)
            | Self::Compound(_) => None,
        }
    }

    /// Returns whether this tag counts as empty.
    ///
    /// Empty means an empty compound or an empty list of any of the four shapes.
    /// Absence is the other half of the rule and is a missing tag rather than a
    /// value, so it is [`Option::is_none`] at the call site.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Compound(entries) => entries.is_empty(),
            Self::List(items) => items.is_empty(),
            Self::ByteArray(items) => items.is_empty(),
            Self::IntArray(items) => items.is_empty(),
            Self::LongArray(items) => items.is_empty(),
            Self::Byte(_)
            | Self::Short(_)
            | Self::Int(_)
            | Self::Long(_)
            | Self::Float(_)
            | Self::Double(_)
            | Self::Text(_) => false,
        }
    }
}

/// The concrete kinds a tag may be.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Kind {
    /// [`Tag::Byte`].
    Byte,
    /// [`Tag::Short`].
    Short,
    /// [`Tag::Int`].
    Int,
    /// [`Tag::Long`].
    Long,
    /// [`Tag::Float`].
    Float,
    /// [`Tag::Double`].
    Double,
    /// [`Tag::Text`].
    Text,
    /// [`Tag::List`].
    List,
    /// [`Tag::ByteArray`].
    ByteArray,
    /// [`Tag::IntArray`].
    IntArray,
    /// [`Tag::LongArray`].
    LongArray,
    /// [`Tag::Compound`].
    Compound,
}

impl Kind {
    /// Returns whether this kind holds a number.
    #[must_use]
    pub const fn is_numeric(self) -> bool {
        matches!(
            self,
            Self::Byte | Self::Short | Self::Int | Self::Long | Self::Float | Self::Double
        )
    }

    /// Returns whether this kind is one of the four list shapes.
    #[must_use]
    pub const fn is_list(self) -> bool {
        matches!(
            self,
            Self::List | Self::ByteArray | Self::IntArray | Self::LongArray
        )
    }
}

/// Which of the two comparisons a stored-data test uses.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum Mode {
    /// The entry's own rules, which are what a pack author writes against.
    #[default]
    Lenient,
    /// The host's own comparison, which here is structural equality.
    Strict,
}

/// A test over a candidate's stored data.
#[derive(Clone, Debug, PartialEq)]
pub struct DataPredicate {
    expected: Option<Tag>,
    mode: Mode,
}

/// How deep a comparison will walk.
///
/// Stored data arrives from a world or from a document a pack author wrote, so
/// its depth is not the caller's to trust. Past this the comparison answers
/// false rather than recursing, which is a refusal a hostile document cannot
/// turn into an exhausted stack.
pub const MAX_DEPTH: usize = 64;

/// How wide a comparison will walk at one level.
///
/// The lenient list rule tests every expected element against every candidate
/// element, so its cost is the product of the two widths, and both arrive from a
/// world or a pack author's document. Depth alone does not bound that. Past this
/// the comparison answers false rather than walking, on the same grounds
/// [`MAX_DEPTH`] does. The upgrade path is a larger constant, or an index over
/// the candidate's elements built once per level.
pub const MAX_WIDTH: usize = 1024;

impl DataPredicate {
    /// A test that matches anything, which is what a null expectation is.
    #[must_use]
    pub const fn anything() -> Self {
        Self {
            expected: None,
            mode: Mode::Lenient,
        }
    }

    /// A test against an expected tag.
    #[must_use]
    pub const fn new(expected: Tag, mode: Mode) -> Self {
        Self {
            expected: Some(expected),
            mode,
        }
    }

    /// Returns the mode this test compares under.
    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// Returns the tag this test expects, for a caller that wants to inspect
    /// rather than test.
    #[must_use]
    pub fn expected(&self) -> Option<&Tag> {
        self.expected.as_ref()
    }

    /// Returns whether a candidate satisfies this test.
    #[must_use]
    pub fn holds(&self, candidate: Option<&Tag>) -> bool {
        let Some(expected) = self.expected.as_ref() else {
            return true;
        };
        let Some(candidate) = candidate else {
            return false;
        };
        match self.mode {
            Mode::Strict => expected == candidate,
            Mode::Lenient => matches(expected, candidate, 0),
        }
    }

    /// Returns whether this test's expectation is empty, and whether a candidate
    /// satisfies the empty rule.
    ///
    /// An empty expectation matches only an empty candidate, where empty means
    /// absent, an empty compound, or an empty list.
    #[must_use]
    pub fn empty_holds(&self, candidate: Option<&Tag>) -> bool {
        let expected_empty = self.expected.as_ref().is_none_or(Tag::is_empty);
        if !expected_empty {
            return false;
        }
        candidate.is_none_or(Tag::is_empty)
    }
}

/// Compares an expectation against a candidate under the lenient rules.
///
/// This is the whole of §7.3 and is public because a dependent testing stored
/// data reaches for the rules rather than for a predicate.
#[must_use]
pub fn lenient(expected: &Tag, candidate: &Tag) -> bool {
    matches(expected, candidate, 0)
}

fn matches(expected: &Tag, candidate: &Tag, depth: usize) -> bool {
    if depth >= MAX_DEPTH {
        return false;
    }
    if expected.is_numeric() && candidate.is_numeric() {
        return match (expected.decimal(), candidate.decimal()) {
            (Some(left), Some(right)) => {
                left.partial_cmp(&right) == Some(std::cmp::Ordering::Equal)
            }
            _ => false,
        };
    }
    if expected.kind() != candidate.kind() {
        return false;
    }
    match (expected, candidate) {
        (Tag::Compound(wanted), Tag::Compound(present)) => {
            if wanted.len() > MAX_WIDTH || present.len() > MAX_WIDTH {
                return false;
            }
            wanted.iter().all(|(name, tag)| {
                present
                    .get(name)
                    .is_some_and(|found| matches(tag, found, depth + 1))
            })
        }
        // A list of lists is the common shape and is compared where it lies; the
        // three primitive arrays are the only case that has to build elements.
        (Tag::List(wanted), Tag::List(present)) => elements_match(wanted, present, depth),
        _ => {
            if let (Some(wanted), Some(present)) = (expected.elements(), candidate.elements()) {
                return elements_match(&wanted, &present, depth);
            }
            expected == candidate
        }
    }
}

fn elements_match(wanted: &[Tag], present: &[Tag], depth: usize) -> bool {
    if wanted.is_empty() {
        return present.is_empty();
    }
    if wanted.len() > MAX_WIDTH || present.len() > MAX_WIDTH {
        return false;
    }
    wanted.iter().all(|element| {
        present
            .iter()
            .any(|found| matches(element, found, depth + 1))
    })
}

#[cfg(test)]
mod tests {
    use super::{DataPredicate, MAX_DEPTH, MAX_WIDTH, Mode, Tag, lenient};

    #[test]
    fn numeric_matching_keeps_ieee_equality_and_wide_integer_rounding() {
        assert!(!lenient(&Tag::Double(f64::NAN), &Tag::Double(f64::NAN)));
        assert!(lenient(&Tag::Double(-0.0), &Tag::Double(0.0)));
        assert!(lenient(
            &Tag::Double(f64::INFINITY),
            &Tag::Double(f64::INFINITY)
        ));
        assert!(lenient(
            &Tag::Long(i64::MAX),
            &Tag::Double(9_223_372_036_854_775_808.0)
        ));
        assert!(lenient(
            &Tag::Long(i64::MIN),
            &Tag::Double(-9_223_372_036_854_775_808.0)
        ));
    }

    #[test]
    fn a_null_expectation_matches_anything_including_a_null_candidate() {
        let predicate = DataPredicate::anything();
        assert!(predicate.holds(None));
        assert!(predicate.holds(Some(&Tag::Int(1))));
        assert!(predicate.expected().is_none());
    }

    #[test]
    fn a_null_candidate_against_a_non_null_expectation_fails() {
        let predicate = DataPredicate::new(Tag::Int(1), Mode::Lenient);
        assert!(!predicate.holds(None));
    }

    #[test]
    fn two_numeric_tags_compare_by_decimal_value_across_widths() {
        assert!(lenient(&Tag::Byte(7), &Tag::Long(7)));
        assert!(lenient(&Tag::Long(7), &Tag::Byte(7)));
        assert!(lenient(&Tag::Double(7.0), &Tag::Int(7)));
        assert!(!lenient(&Tag::Byte(7), &Tag::Long(8)));
    }

    #[test]
    fn a_number_and_a_text_do_not_match_even_when_the_text_spells_the_number() {
        assert!(!lenient(&Tag::Int(7), &Tag::Text("7".to_owned())));
        assert!(!lenient(&Tag::Text("7".to_owned()), &Tag::Int(7)));
    }

    #[test]
    fn two_non_numeric_tags_of_different_kinds_fail() {
        assert!(!lenient(
            &Tag::Text("a".to_owned()),
            &Tag::List(vec![Tag::Text("a".to_owned())])
        ));
        assert!(!lenient(&Tag::Compound([].into()), &Tag::List(vec![])));
    }

    #[test]
    fn a_compound_names_keys_and_ignores_the_rest() {
        let expected = Tag::compound([("a", Tag::Int(1))]);
        let candidate = Tag::compound([("a", Tag::Byte(1)), ("b", Tag::Text("x".to_owned()))]);
        assert!(lenient(&expected, &candidate));
        let missing = Tag::compound([("b", Tag::Int(1))]);
        assert!(!lenient(&expected, &missing));
    }

    #[test]
    fn a_nested_compound_is_compared_the_same_way_at_every_level() {
        let expected = Tag::compound([("outer", Tag::compound([("inner", Tag::Short(3))]))]);
        let candidate = Tag::compound([(
            "outer",
            Tag::compound([("inner", Tag::Long(3)), ("extra", Tag::Int(9))]),
        )]);
        assert!(lenient(&expected, &candidate));
    }

    #[test]
    fn a_list_holds_when_every_element_matches_some_element_in_any_order() {
        let expected = Tag::List(vec![Tag::Int(2), Tag::Int(1)]);
        let candidate = Tag::List(vec![Tag::Int(1), Tag::Int(3), Tag::Int(2)]);
        assert!(lenient(&expected, &candidate));
    }

    #[test]
    fn a_list_admits_repeats_because_each_element_is_matched_on_its_own() {
        let expected = Tag::List(vec![Tag::Int(1), Tag::Int(1)]);
        let candidate = Tag::List(vec![Tag::Int(1)]);
        assert!(
            lenient(&expected, &candidate),
            "the rule is per-element, so one candidate satisfies both"
        );
    }

    #[test]
    fn an_element_nothing_matches_fails_the_list() {
        let expected = Tag::List(vec![Tag::Int(4)]);
        let candidate = Tag::List(vec![Tag::Int(1), Tag::Int(2)]);
        assert!(!lenient(&expected, &candidate));
    }

    #[test]
    fn an_empty_expected_list_requires_an_empty_candidate() {
        assert!(lenient(&Tag::List(vec![]), &Tag::List(vec![])));
        assert!(!lenient(&Tag::List(vec![]), &Tag::List(vec![Tag::Int(1)])));
    }

    #[test]
    fn the_three_primitive_arrays_are_lists_here_too() {
        assert!(lenient(
            &Tag::ByteArray(vec![2]),
            &Tag::ByteArray(vec![1, 2])
        ));
        assert!(lenient(&Tag::IntArray(vec![9]), &Tag::IntArray(vec![9])));
        assert!(!lenient(&Tag::LongArray(vec![]), &Tag::LongArray(vec![1])));
        assert!(
            !lenient(&Tag::IntArray(vec![1]), &Tag::List(vec![Tag::Int(1)])),
            "the kinds still have to agree"
        );
    }

    #[test]
    fn anything_else_compares_by_equality() {
        assert!(lenient(
            &Tag::Text("a".to_owned()),
            &Tag::Text("a".to_owned())
        ));
        assert!(!lenient(
            &Tag::Text("a".to_owned()),
            &Tag::Text("b".to_owned())
        ));
    }

    #[test]
    fn the_strict_mode_is_structural_equality_and_admits_no_widening() {
        let strict = DataPredicate::new(Tag::Int(7), Mode::Strict);
        assert!(strict.holds(Some(&Tag::Int(7))));
        assert!(!strict.holds(Some(&Tag::Long(7))));
        let lenient_predicate = DataPredicate::new(Tag::Int(7), Mode::Lenient);
        assert!(lenient_predicate.holds(Some(&Tag::Long(7))));
        assert_eq!(strict.mode(), Mode::Strict);
    }

    #[test]
    fn a_strict_compound_refuses_the_extra_keys_a_lenient_one_ignores() {
        let expected = Tag::compound([("a", Tag::Int(1))]);
        let candidate = Tag::compound([("a", Tag::Int(1)), ("b", Tag::Int(2))]);
        assert!(!DataPredicate::new(expected.clone(), Mode::Strict).holds(Some(&candidate)));
        assert!(DataPredicate::new(expected, Mode::Lenient).holds(Some(&candidate)));
    }

    #[test]
    fn empty_means_absent_an_empty_compound_or_an_empty_list() {
        let predicate = DataPredicate::new(Tag::Compound([].into()), Mode::Lenient);
        assert!(predicate.empty_holds(None));
        assert!(predicate.empty_holds(Some(&Tag::Compound([].into()))));
        assert!(predicate.empty_holds(Some(&Tag::List(vec![]))));
        assert!(!predicate.empty_holds(Some(&Tag::Int(0))));
        let full = DataPredicate::new(Tag::Int(1), Mode::Lenient);
        assert!(!full.empty_holds(None));
    }

    #[test]
    fn a_tree_deeper_than_the_ceiling_answers_false_rather_than_walking_it() {
        let mut deep = Tag::Int(1);
        for _ in 0..(MAX_DEPTH + 4) {
            deep = Tag::compound([("down", deep)]);
        }
        assert!(
            !lenient(&deep, &deep),
            "the ceiling refuses before the stack does"
        );
        let shallow = Tag::compound([("down", Tag::Int(1))]);
        assert!(lenient(&shallow, &shallow));
    }

    #[test]
    fn a_tree_wider_than_the_ceiling_answers_false_rather_than_walking_it() {
        let element = |value: usize| Tag::Int(i32::try_from(value).expect("a small index"));
        let wide = Tag::List((0..=MAX_WIDTH).map(element).collect());
        assert!(
            !lenient(&wide, &wide),
            "depth alone does not bound a rule whose cost is the product of two widths"
        );
        let at_ceiling = Tag::List((0..MAX_WIDTH).map(element).collect());
        assert!(
            lenient(&at_ceiling, &at_ceiling),
            "the ceiling itself walks"
        );

        let wide_compound =
            Tag::compound((0..=MAX_WIDTH).map(|value| (format!("k{value}"), Tag::Int(1))));
        assert!(!lenient(&wide_compound, &wide_compound));
    }

    #[test]
    fn a_kind_reports_its_own_family() {
        assert!(Tag::Byte(0).kind().is_numeric());
        assert!(Tag::Double(0.0).kind().is_numeric());
        assert!(!Tag::Text(String::new()).kind().is_numeric());
        assert!(Tag::IntArray(vec![]).kind().is_list());
        assert!(Tag::List(vec![]).kind().is_list());
        assert!(!Tag::Compound([].into()).kind().is_list());
    }
}
