//! Block state matching and the three composite predicates, from §7.1 and §7.2.
//!
//! A block state predicate is a map from property name to a matcher. It holds
//! when every named property that the state definition actually declares
//! satisfies its matcher, and a named property the definition does not declare
//! fails the whole predicate rather than being ignored.
//!
//! The two matcher kinds are told apart on decode by shape rather than by a kind
//! field: a bare text value is exact, an object is a range. That is a wire
//! contract and is preserved.
//!
//! §7.2's three composite predicates each combine a registry identity with
//! optional extra tests, and each exposes its stored tag for a caller that wants
//! to inspect rather than test.
//!
//! # The defect fixed here
//!
//! **The item predicate's identity test is inverted.** The original returns
//! false when the stack *is* the declared item, so the predicate can only hold
//! for a stack that is not what it names. [`ItemPredicate`] means the obvious
//! thing and no parity test is written against the original.

use std::collections::BTreeMap;

use ravel_sdk::content::ContentId;
use ravel_sdk::{JsonValue, json};

use crate::data::{DataPredicate, Tag};

/// A source of choices, supplied by the caller so a construction is
/// reproducible.
///
/// The original picks one of a range's two bounds at random when building a
/// representative state. Keeping the randomness means keeping the caller's own
/// source rather than reaching for a process-wide one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Random {
    state: u64,
}

impl Random {
    /// Builds a source from a seed.
    ///
    /// A zero seed would leave the sequence at zero forever, so it is turned
    /// into a non-zero one rather than refused.
    #[must_use]
    pub const fn seeded(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    /// Returns the next value.
    pub fn next_value(&mut self) -> u64 {
        let mut state = self.state;
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        self.state = state;
        state
    }

    /// Returns the next choice between two things.
    pub fn next_bool(&mut self) -> bool {
        self.next_value() & 1 == 1
    }
}

/// One property a block state definition declares.
///
/// A property is its name and the values it admits, in the order that decides
/// how two of them compare. That is the whole of what §7.1 needs: a parser, and
/// an ordering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Property {
    name: String,
    values: Vec<String>,
}

/// The most values one property will admit.
pub const MAX_PROPERTY_VALUES: usize = 256;

/// The most properties one definition will declare.
pub const MAX_PROPERTIES: usize = 64;

impl Property {
    /// Declares a property.
    ///
    /// # Errors
    ///
    /// Returns [`DefinitionError`] for a property with no values, values that
    /// repeat, or more values than [`MAX_PROPERTY_VALUES`].
    pub fn new<I, S>(name: &str, values: I) -> Result<Self, DefinitionError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let values: Vec<String> = values.into_iter().map(Into::into).collect();
        if values.is_empty() {
            return Err(DefinitionError::NoValues(name.to_owned()));
        }
        if values.len() > MAX_PROPERTY_VALUES {
            return Err(DefinitionError::TooManyValues(values.len()));
        }
        let mut seen: Vec<&str> = values.iter().map(String::as_str).collect();
        seen.sort_unstable();
        let count = seen.len();
        seen.dedup();
        if seen.len() != count {
            return Err(DefinitionError::RepeatedValue(name.to_owned()));
        }
        Ok(Self {
            name: name.to_owned(),
            values,
        })
    }

    /// Returns the property's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the values, in the order they compare in.
    #[must_use]
    pub fn values(&self) -> &[String] {
        &self.values
    }

    /// Parses a spelling into this property's own ordering position.
    ///
    /// An unparseable spelling is absent, which is what makes an exact matcher
    /// on one never hold.
    #[must_use]
    pub fn parse(&self, spelling: &str) -> Option<usize> {
        self.values.iter().position(|value| value == spelling)
    }
}

/// Why a state definition was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefinitionError {
    /// A property declares no value at all.
    NoValues(String),
    /// A property declares one value twice.
    RepeatedValue(String),
    /// A property declares more values than [`MAX_PROPERTY_VALUES`].
    TooManyValues(usize),
    /// A definition declares more properties than [`MAX_PROPERTIES`].
    TooManyProperties(usize),
    /// A definition declares one property twice.
    RepeatedProperty(String),
}

impl std::fmt::Display for DefinitionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoValues(name) => write!(formatter, "`{name}` declares no value"),
            Self::RepeatedValue(name) => write!(formatter, "`{name}` declares one value twice"),
            Self::TooManyValues(count) => write!(formatter, "a property declares {count} values"),
            Self::TooManyProperties(count) => {
                write!(formatter, "a definition declares {count} properties")
            }
            Self::RepeatedProperty(name) => {
                write!(formatter, "a definition declares `{name}` twice")
            }
        }
    }
}

impl std::error::Error for DefinitionError {}

/// What a block's states are made of.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Definition {
    properties: Vec<Property>,
}

impl Definition {
    /// Declares a definition over a set of properties.
    ///
    /// # Errors
    ///
    /// Returns [`DefinitionError`] for a repeated property name or more than
    /// [`MAX_PROPERTIES`] of them.
    pub fn new<I>(properties: I) -> Result<Self, DefinitionError>
    where
        I: IntoIterator<Item = Property>,
    {
        let properties: Vec<Property> = properties.into_iter().collect();
        if properties.len() > MAX_PROPERTIES {
            return Err(DefinitionError::TooManyProperties(properties.len()));
        }
        for (index, property) in properties.iter().enumerate() {
            if properties[..index]
                .iter()
                .any(|earlier| earlier.name() == property.name())
            {
                return Err(DefinitionError::RepeatedProperty(
                    property.name().to_owned(),
                ));
            }
        }
        Ok(Self { properties })
    }

    /// Returns the property under a name, when the definition declares it.
    #[must_use]
    pub fn property(&self, name: &str) -> Option<&Property> {
        self.properties
            .iter()
            .find(|property| property.name() == name)
    }

    /// Returns every declared property.
    #[must_use]
    pub fn properties(&self) -> &[Property] {
        &self.properties
    }
}

/// One state: a value per declared property.
pub type State = BTreeMap<String, String>;

/// How a single property's value is tested.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Matcher {
    /// A single value, spelled as text and parsed by the property's own parser.
    Exact(String),
    /// A lower and an upper bound, each optional and each inclusive.
    Range {
        /// The inclusive lower bound, unbounded when absent.
        lower: Option<String>,
        /// The inclusive upper bound, unbounded when absent.
        upper: Option<String>,
    },
}

impl Matcher {
    /// Returns whether a state's current value satisfies this matcher.
    ///
    /// An unparseable spelling never holds, on either side.
    #[must_use]
    pub fn holds(&self, property: &Property, current: &str) -> bool {
        let Some(value) = property.parse(current) else {
            return false;
        };
        match self {
            Self::Exact(expected) => property.parse(expected) == Some(value),
            Self::Range { lower, upper } => {
                if let Some(bound) = lower {
                    match property.parse(bound) {
                        Some(low) if value >= low => {}
                        _ => return false,
                    }
                }
                if let Some(bound) = upper {
                    match property.parse(bound) {
                        Some(high) if value <= high => {}
                        _ => return false,
                    }
                }
                true
            }
        }
    }

    /// Reads a matcher from a document.
    ///
    /// The two kinds are told apart by shape: a bare text value is exact and an
    /// object is a range. That is the wire contract.
    ///
    /// # Errors
    ///
    /// Returns [`MatcherError`] for a document that is neither a text value nor
    /// an object, or a range whose bound is not text.
    pub fn decode(document: &JsonValue) -> Result<Self, MatcherError> {
        if let Some(text) = document.as_str() {
            return Ok(Self::Exact(text.to_owned()));
        }
        let object = document.as_object().ok_or(MatcherError::WrongShape)?;
        let bound = |name: &str| -> Result<Option<String>, MatcherError> {
            match object.get(name) {
                None => Ok(None),
                Some(value) => Ok(Some(
                    value.as_str().ok_or(MatcherError::BoundNotText)?.to_owned(),
                )),
            }
        };
        Ok(Self::Range {
            lower: bound(LOWER_BOUND)?,
            upper: bound(UPPER_BOUND)?,
        })
    }

    /// Writes a matcher back to a document, in the shape it decodes from.
    ///
    /// # Panics
    ///
    /// Panics if a freshly constructed JSON object is not an object.
    #[must_use]
    pub fn encode(&self) -> JsonValue {
        match self {
            Self::Exact(value) => JsonValue::String(value.clone()),
            Self::Range { lower, upper } => {
                let mut document = json!({});
                let object = document
                    .as_object_mut()
                    .expect("the value was just built as an object");
                if let Some(bound) = lower {
                    object.insert(LOWER_BOUND.to_owned(), JsonValue::String(bound.clone()));
                }
                if let Some(bound) = upper {
                    object.insert(UPPER_BOUND.to_owned(), JsonValue::String(bound.clone()));
                }
                document
            }
        }
    }

    fn example(&self, property: &Property, random: &mut Random) -> Option<String> {
        match self {
            Self::Exact(value) => property.parse(value).map(|_| value.clone()),
            Self::Range { lower, upper } => match (lower, upper) {
                (Some(low), Some(high)) => {
                    let picked = if random.next_bool() { high } else { low };
                    property.parse(picked).map(|_| picked.clone())
                }
                (Some(only), None) | (None, Some(only)) => {
                    property.parse(only).map(|_| only.clone())
                }
                (None, None) => None,
            },
        }
    }
}

/// The name a range's lower bound is written under.
pub const LOWER_BOUND: &str = "lower";

/// The name a range's upper bound is written under.
pub const UPPER_BOUND: &str = "upper";

/// Why a matcher did not decode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatcherError {
    /// The document is neither a text value nor an object.
    WrongShape,
    /// A range's bound is not text.
    BoundNotText,
}

impl std::fmt::Display for MatcherError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongShape => write!(
                formatter,
                "a property matcher is neither a value nor a range"
            ),
            Self::BoundNotText => write!(formatter, "a range's bound is not a spelling"),
        }
    }
}

impl std::error::Error for MatcherError {}

/// A test over a block state's properties.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StatePredicate {
    matchers: BTreeMap<String, Matcher>,
}

impl StatePredicate {
    /// The predicate that holds for anything, which is what a block predicate
    /// defaults to.
    #[must_use]
    pub fn anything() -> Self {
        Self::default()
    }

    /// Builds a predicate over named matchers.
    #[must_use]
    pub fn new<I, S>(matchers: I) -> Self
    where
        I: IntoIterator<Item = (S, Matcher)>,
        S: Into<String>,
    {
        Self {
            matchers: matchers
                .into_iter()
                .map(|(name, matcher)| (name.into(), matcher))
                .collect(),
        }
    }

    /// Returns how many properties this names.
    #[must_use]
    pub fn len(&self) -> usize {
        self.matchers.len()
    }

    /// Returns whether this names no property, which makes it hold for
    /// anything.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.matchers.is_empty()
    }

    /// Returns whether a state satisfies this predicate.
    ///
    /// A named property the definition does not declare fails the whole
    /// predicate rather than being ignored.
    #[must_use]
    pub fn holds(&self, definition: &Definition, state: &State) -> bool {
        self.matchers.iter().all(|(name, matcher)| {
            let Some(property) = definition.property(name) else {
                return false;
            };
            let Some(current) = state.get(name) else {
                return false;
            };
            matcher.holds(property, current)
        })
    }

    /// Constructs a representative state from the matchers.
    ///
    /// This runs the predicate backwards, for a caller that needs an example
    /// rather than a test. An exact matcher contributes its value; a range with
    /// both bounds picks one of them at random from the caller's source, and one
    /// with a single bound contributes that bound. A property the definition
    /// does not declare, an unparseable spelling and an unbounded range all
    /// contribute nothing.
    #[must_use]
    pub fn example(&self, definition: &Definition, random: &mut Random) -> State {
        let mut state = State::new();
        for (name, matcher) in &self.matchers {
            let Some(property) = definition.property(name) else {
                continue;
            };
            if let Some(value) = matcher.example(property, random) {
                state.insert(name.clone(), value);
            }
        }
        state
    }
}

/// A test over a block at a position.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockPredicate {
    /// The block this names, which is required.
    pub identity: ContentId,
    /// An optional test over the block entity's fully-saved form.
    pub stored: Option<DataPredicate>,
    /// An optional test over the position.
    pub location: Option<String>,
    /// The state predicate, which defaults to "anything".
    pub state: StatePredicate,
}

/// What a caller knows about a block at a position.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockCandidate {
    /// The block's identity.
    pub identity: ContentId,
    /// The state at the position.
    pub state: State,
    /// The block entity's fully-saved form, when there is one there.
    pub stored: Option<Tag>,
    /// What the location test would answer, when one is asked for.
    pub location: Option<String>,
}

impl BlockPredicate {
    /// Names a block, with no other test.
    #[must_use]
    pub fn new(identity: ContentId) -> Self {
        Self {
            identity,
            stored: None,
            location: None,
            state: StatePredicate::anything(),
        }
    }

    /// Returns the stored-data test, for a caller that wants to inspect rather
    /// than test.
    #[must_use]
    pub fn stored_tag(&self) -> Option<&Tag> {
        self.stored.as_ref().and_then(DataPredicate::expected)
    }

    /// Returns whether a candidate satisfies this predicate.
    ///
    /// A stored-data test requires a block entity to be there: an absent one
    /// fails rather than passing vacuously.
    #[must_use]
    pub fn holds(&self, definition: &Definition, candidate: &BlockCandidate) -> bool {
        if candidate.identity != self.identity {
            return false;
        }
        if !self.state.holds(definition, &candidate.state) {
            return false;
        }
        if let Some(expected) = self.location.as_ref()
            && candidate.location.as_deref() != Some(expected.as_str())
        {
            return false;
        }
        self.stored
            .as_ref()
            .is_none_or(|test| candidate.stored.is_some() && test.holds(candidate.stored.as_ref()))
    }
}

/// A test over an entity.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityPredicate {
    /// The entity kind this names, which is required.
    pub kind: ContentId,
    /// An optional test over the position.
    pub location: Option<String>,
    /// The status effects that must be present.
    pub effects: Vec<String>,
    /// An optional test over the entity's stored data.
    pub stored: Option<DataPredicate>,
    /// The flags that must be set.
    pub flags: Vec<String>,
    /// An optional test applied to the entity's current target.
    pub target: Option<Box<Self>>,
}

/// What a caller knows about an entity.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityCandidate {
    /// The entity's kind.
    pub kind: ContentId,
    /// Where it is, when the caller has a location to offer.
    pub location: Option<String>,
    /// Its status effects.
    pub effects: Vec<String>,
    /// Its stored data.
    pub stored: Option<Tag>,
    /// Its flags.
    pub flags: Vec<String>,
    /// What it is targeting, absent for a non-targeting entity.
    pub target: Option<Box<Self>>,
}

impl EntityCandidate {
    /// Names an entity kind with no optional observations.
    #[must_use]
    pub fn new(kind: ContentId) -> Self {
        Self {
            kind,
            location: None,
            effects: Vec::new(),
            stored: None,
            flags: Vec::new(),
            target: None,
        }
    }
}

/// How deep a target chain will be followed.
///
/// An entity that targets an entity that targets it is a cycle the walk would
/// not leave on its own.
pub const MAX_TARGET_DEPTH: usize = 8;

impl EntityPredicate {
    /// Names an entity kind, with no other test.
    #[must_use]
    pub fn new(kind: ContentId) -> Self {
        Self {
            kind,
            location: None,
            effects: Vec::new(),
            stored: None,
            flags: Vec::new(),
            target: None,
        }
    }

    /// Returns the stored-data test, for a caller that wants to inspect rather
    /// than test.
    #[must_use]
    pub fn stored_tag(&self) -> Option<&Tag> {
        self.stored.as_ref().and_then(DataPredicate::expected)
    }

    /// Returns whether a candidate satisfies this predicate.
    ///
    /// A predicate that names a target is not satisfied by an entity that has
    /// none. §7.2 marks the original's reading inferred — it passes the absent
    /// target to the same predicate and says nothing about what that means — and
    /// the outcome is the same either way here, because an [`EntityPredicate`]
    /// requires a kind and an absent target has none.
    #[must_use]
    pub fn holds(&self, candidate: &EntityCandidate) -> bool {
        self.holds_at(candidate, 0)
    }

    fn holds_at(&self, candidate: &EntityCandidate, depth: usize) -> bool {
        if depth >= MAX_TARGET_DEPTH {
            return false;
        }
        if candidate.kind != self.kind {
            return false;
        }
        if let Some(expected) = self.location.as_ref()
            && candidate.location.as_deref() != Some(expected.as_str())
        {
            return false;
        }
        if !self
            .effects
            .iter()
            .all(|effect| candidate.effects.contains(effect))
        {
            return false;
        }
        if !self.flags.iter().all(|flag| candidate.flags.contains(flag)) {
            return false;
        }
        if let Some(test) = self.stored.as_ref()
            && !test.holds(candidate.stored.as_ref())
        {
            return false;
        }
        self.target.as_ref().is_none_or(|test| {
            candidate
                .target
                .as_ref()
                .is_some_and(|target| test.holds_at(target, depth + 1))
        })
    }
}

/// A bound on a stack's durability or count.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct Bound {
    /// The inclusive lower end, unbounded when absent.
    pub lower: Option<i32>,
    /// The inclusive upper end, unbounded when absent.
    pub upper: Option<i32>,
}

impl Bound {
    /// The bound that admits anything.
    pub const ANY: Self = Self {
        lower: None,
        upper: None,
    };

    /// Returns whether this bound constrains nothing.
    #[must_use]
    pub const fn is_any(self) -> bool {
        self.lower.is_none() && self.upper.is_none()
    }

    /// Returns whether a value is inside the bound.
    #[must_use]
    pub fn admits(self, value: i32) -> bool {
        self.lower.is_none_or(|low| value >= low) && self.upper.is_none_or(|high| value <= high)
    }
}

/// A test over an item stack.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemPredicate {
    /// The item this names, which is required.
    pub identity: ContentId,
    /// An optional test over the stack's stored data.
    pub stored: Option<DataPredicate>,
    /// A bound on the remaining durability.
    pub durability: Bound,
    /// A bound on the count.
    pub count: Bound,
}

/// What a caller knows about an item stack.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemCandidate {
    /// The stack's item identity.
    pub identity: ContentId,
    /// The stack's stored data.
    pub stored: Option<Tag>,
    /// The remaining durability, absent for an item that does not take damage.
    pub durability: Option<i32>,
    /// How many are in the stack.
    pub count: i32,
}

impl ItemPredicate {
    /// Names an item, with no other test.
    #[must_use]
    pub fn new(identity: ContentId) -> Self {
        Self {
            identity,
            stored: None,
            durability: Bound::ANY,
            count: Bound::ANY,
        }
    }

    /// Returns the stored-data test, for a caller that wants to inspect rather
    /// than test.
    #[must_use]
    pub fn stored_tag(&self) -> Option<&Tag> {
        self.stored.as_ref().and_then(DataPredicate::expected)
    }

    /// Returns whether a candidate satisfies this predicate.
    ///
    /// The identity test means what it says, which the original's does not.
    ///
    /// Two further rules are the original's and are kept deliberately rather
    /// than by accident: a durability bound that is anything other than "any"
    /// fails outright for an item that does not take damage, and the value
    /// compared is the *remaining* durability rather than the damage taken.
    #[must_use]
    pub fn holds(&self, candidate: &ItemCandidate) -> bool {
        if candidate.identity != self.identity {
            return false;
        }
        if !self.count.admits(candidate.count) {
            return false;
        }
        if !self.durability.is_any() {
            let Some(remaining) = candidate.durability else {
                return false;
            };
            if !self.durability.admits(remaining) {
                return false;
            }
        }
        self.stored
            .as_ref()
            .is_none_or(|test| test.holds(candidate.stored.as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::json;

    use super::{
        BlockCandidate, BlockPredicate, Bound, Definition, EntityCandidate, EntityPredicate,
        ItemCandidate, ItemPredicate, MAX_PROPERTIES, Matcher, MatcherError, Property, Random,
        State, StatePredicate,
    };
    use crate::data::{DataPredicate, Mode, Tag};

    fn age() -> Property {
        Property::new("age", ["0", "1", "2", "3"]).expect("four distinct values")
    }

    fn facing() -> Property {
        Property::new("facing", ["north", "east", "south", "west"]).expect("four distinct values")
    }

    fn definition() -> Definition {
        Definition::new([age(), facing()]).expect("two distinct properties")
    }

    fn state(pairs: &[(&str, &str)]) -> State {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[test]
    fn a_predicate_holds_when_every_named_property_satisfies_its_matcher() {
        let predicate = StatePredicate::new([
            ("age", Matcher::Exact("2".to_owned())),
            ("facing", Matcher::Exact("south".to_owned())),
        ]);
        assert!(predicate.holds(&definition(), &state(&[("age", "2"), ("facing", "south")])));
        assert!(!predicate.holds(&definition(), &state(&[("age", "1"), ("facing", "south")])));
        assert_eq!(predicate.len(), 2);
    }

    #[test]
    fn a_named_property_the_definition_does_not_declare_fails_the_whole_predicate() {
        let predicate = StatePredicate::new([("absent", Matcher::Exact("1".to_owned()))]);
        assert!(
            !predicate.holds(&definition(), &state(&[("age", "1")])),
            "the rule is a failure rather than an ignored name"
        );
    }

    #[test]
    fn an_empty_predicate_holds_for_anything() {
        assert!(StatePredicate::anything().holds(&definition(), &State::new()));
        assert_eq!(StatePredicate::anything().len(), 0);
    }

    #[test]
    fn an_exact_matcher_holds_only_for_the_parsed_value() {
        let matcher = Matcher::Exact("east".to_owned());
        assert!(matcher.holds(&facing(), "east"));
        assert!(!matcher.holds(&facing(), "west"));
    }

    #[test]
    fn an_unparseable_spelling_never_holds_on_either_side() {
        assert!(!Matcher::Exact("up".to_owned()).holds(&facing(), "north"));
        assert!(!Matcher::Exact("north".to_owned()).holds(&facing(), "up"));
        let range = Matcher::Range {
            lower: Some("nope".to_owned()),
            upper: None,
        };
        assert!(!range.holds(&facing(), "north"));
    }

    #[test]
    fn both_range_bounds_are_inclusive_and_an_absent_bound_is_unbounded() {
        let both = Matcher::Range {
            lower: Some("1".to_owned()),
            upper: Some("2".to_owned()),
        };
        assert!(both.holds(&age(), "1"));
        assert!(both.holds(&age(), "2"));
        assert!(!both.holds(&age(), "0"));
        assert!(!both.holds(&age(), "3"));
        let upper_only = Matcher::Range {
            lower: None,
            upper: Some("1".to_owned()),
        };
        assert!(upper_only.holds(&age(), "0"));
        assert!(!upper_only.holds(&age(), "2"));
    }

    #[test]
    fn the_two_matcher_kinds_are_told_apart_by_shape_rather_than_a_kind_field() {
        assert_eq!(
            Matcher::decode(&json!("north")),
            Ok(Matcher::Exact("north".to_owned()))
        );
        assert_eq!(
            Matcher::decode(&json!({ "lower": "1", "upper": "3" })),
            Ok(Matcher::Range {
                lower: Some("1".to_owned()),
                upper: Some("3".to_owned()),
            })
        );
        assert_eq!(
            Matcher::decode(&json!({})),
            Ok(Matcher::Range {
                lower: None,
                upper: None,
            }),
            "an object with neither bound is still a range"
        );
    }

    #[test]
    fn a_matcher_round_trips_through_the_shape_it_decodes_from() {
        for matcher in [
            Matcher::Exact("north".to_owned()),
            Matcher::Range {
                lower: Some("1".to_owned()),
                upper: None,
            },
        ] {
            assert_eq!(Matcher::decode(&matcher.encode()), Ok(matcher));
        }
    }

    #[test]
    fn a_matcher_is_refused_two_ways() {
        assert_eq!(Matcher::decode(&json!(4)), Err(MatcherError::WrongShape));
        assert_eq!(
            Matcher::decode(&json!({ "lower": 4 })),
            Err(MatcherError::BoundNotText)
        );
    }

    #[test]
    fn running_the_predicate_backwards_builds_a_representative_state() {
        let predicate = StatePredicate::new([
            ("age", Matcher::Exact("2".to_owned())),
            ("facing", Matcher::Exact("west".to_owned())),
        ]);
        let mut random = Random::seeded(1);
        let built = predicate.example(&definition(), &mut random);
        assert_eq!(built, state(&[("age", "2"), ("facing", "west")]));
        assert!(predicate.holds(&definition(), &built));
    }

    #[test]
    fn a_range_with_both_bounds_picks_one_of_them_and_the_source_makes_it_reproducible() {
        let predicate = StatePredicate::new([(
            "age",
            Matcher::Range {
                lower: Some("0".to_owned()),
                upper: Some("3".to_owned()),
            },
        )]);
        let picked = |seed: u64| {
            let mut random = Random::seeded(seed);
            predicate
                .example(&definition(), &mut random)
                .get("age")
                .cloned()
                .expect("a bound is contributed")
        };
        assert_eq!(picked(7), picked(7), "one seed gives one answer");
        let choices: std::collections::BTreeSet<String> = (0..64).map(picked).collect();
        assert_eq!(
            choices,
            ["0".to_owned(), "3".to_owned()].into_iter().collect(),
            "only the two bounds are ever picked"
        );
    }

    #[test]
    fn a_range_with_one_bound_contributes_it_and_an_unbounded_one_contributes_nothing() {
        let mut random = Random::seeded(3);
        let single = StatePredicate::new([(
            "age",
            Matcher::Range {
                lower: Some("1".to_owned()),
                upper: None,
            },
        )]);
        assert_eq!(
            single.example(&definition(), &mut random).get("age"),
            Some(&"1".to_owned())
        );
        let open = StatePredicate::new([(
            "age",
            Matcher::Range {
                lower: None,
                upper: None,
            },
        )]);
        assert_eq!(open.example(&definition(), &mut random).len(), 0);
    }

    #[test]
    fn a_seeded_source_of_zero_still_produces_a_sequence() {
        let mut random = Random::seeded(0);
        let first = random.next_value();
        assert_ne!(first, 0);
        assert_ne!(random.next_value(), first);
    }

    #[test]
    fn a_definition_is_refused_five_ways() {
        assert!(Property::new("p", Vec::<String>::new()).is_err());
        assert!(Property::new("p", ["a", "a"]).is_err());
        let many: Vec<String> = (0..=super::MAX_PROPERTY_VALUES)
            .map(|index| index.to_string())
            .collect();
        assert!(Property::new("p", many).is_err());
        assert!(Definition::new([age(), age()]).is_err());
        let properties: Vec<Property> = (0..=MAX_PROPERTIES)
            .map(|index| Property::new(&format!("p{index}"), ["a"]).expect("one value"))
            .collect();
        assert!(Definition::new(properties).is_err());
    }

    #[test]
    fn a_block_predicate_tests_the_block_the_state_the_location_and_the_stored_data() {
        let mut predicate =
            BlockPredicate::new("minecraft:lantern".parse().expect("a content identity"));
        predicate.state = StatePredicate::new([("facing", Matcher::Exact("north".to_owned()))]);
        predicate.location = Some("cave".to_owned());
        predicate.stored = Some(DataPredicate::new(
            Tag::compound([("lit", Tag::Byte(1))]),
            Mode::Lenient,
        ));
        let candidate = BlockCandidate {
            identity: "minecraft:lantern".parse().expect("a content identity"),
            state: state(&[("facing", "north")]),
            stored: Some(Tag::compound([("lit", Tag::Int(1)), ("age", Tag::Int(0))])),
            location: Some("cave".to_owned()),
        };
        assert!(predicate.holds(&definition(), &candidate));

        let mut elsewhere = candidate.clone();
        elsewhere.location = Some("surface".to_owned());
        assert!(!predicate.holds(&definition(), &elsewhere));

        let mut other = candidate.clone();
        other.identity = "minecraft:torch".parse().expect("a content identity");
        assert!(!predicate.holds(&definition(), &other));

        let mut turned = candidate;
        turned.state = state(&[("facing", "south")]);
        assert!(!predicate.holds(&definition(), &turned));
    }

    #[test]
    fn a_block_predicate_with_a_stored_test_needs_a_block_entity_to_be_there() {
        let mut predicate =
            BlockPredicate::new("minecraft:lantern".parse().expect("a content identity"));
        predicate.stored = Some(DataPredicate::anything());
        let bare = BlockCandidate {
            identity: "minecraft:lantern".parse().expect("a content identity"),
            state: State::new(),
            stored: None,
            location: None,
        };
        assert!(
            !predicate.holds(&definition(), &bare),
            "a test that matches anything still needs something to be there"
        );
        assert!(predicate.stored_tag().is_none());
    }

    #[test]
    fn a_block_predicate_defaults_its_state_test_to_anything() {
        let predicate =
            BlockPredicate::new("minecraft:lantern".parse().expect("a content identity"));
        let candidate = BlockCandidate {
            identity: "minecraft:lantern".parse().expect("a content identity"),
            state: state(&[("facing", "west")]),
            stored: None,
            location: None,
        };
        assert!(predicate.holds(&definition(), &candidate));
    }

    #[test]
    fn an_entity_predicate_tests_its_kind_and_its_five_optional_tests() {
        let mut predicate =
            EntityPredicate::new("example:wanderer".parse().expect("a content identity"));
        predicate.effects = vec!["haste".to_owned()];
        predicate.flags = vec!["airborne".to_owned()];
        predicate.location = Some("cave".to_owned());
        predicate.stored = Some(DataPredicate::new(Tag::Int(3), Mode::Lenient));
        let candidate = EntityCandidate {
            kind: "example:wanderer".parse().expect("a content identity"),
            location: Some("cave".to_owned()),
            effects: vec!["haste".to_owned(), "speed".to_owned()],
            stored: Some(Tag::Long(3)),
            flags: vec!["airborne".to_owned()],
            target: None,
        };
        assert!(predicate.holds(&candidate));

        let mut without = candidate.clone();
        without.effects = vec!["speed".to_owned()];
        assert!(!predicate.holds(&without));

        let mut grounded = candidate;
        grounded.flags = Vec::new();
        assert!(!predicate.holds(&grounded));
    }

    #[test]
    fn a_predicate_naming_a_target_is_not_satisfied_by_an_entity_that_has_none() {
        let mut predicate =
            EntityPredicate::new("example:wanderer".parse().expect("a content identity"));
        predicate.target = Some(Box::new(EntityPredicate::new(
            "example:traveller".parse().expect("a content identity"),
        )));
        let alone = EntityCandidate::new("example:wanderer".parse().expect("a content identity"));
        assert!(!predicate.holds(&alone));
        let hunting = EntityCandidate {
            target: Some(Box::new(EntityCandidate::new(
                "example:traveller".parse().expect("a content identity"),
            ))),
            ..EntityCandidate::new("example:wanderer".parse().expect("a content identity"))
        };
        assert!(predicate.holds(&hunting));
    }

    #[test]
    fn a_target_chain_is_bounded_rather_than_followed_forever() {
        let mut predicate =
            EntityPredicate::new("example:wanderer".parse().expect("a content identity"));
        let mut inner =
            EntityPredicate::new("example:wanderer".parse().expect("a content identity"));
        for _ in 0..super::MAX_TARGET_DEPTH {
            let mut outer =
                EntityPredicate::new("example:wanderer".parse().expect("a content identity"));
            outer.target = Some(Box::new(inner));
            inner = outer;
        }
        predicate.target = Some(Box::new(inner));
        let mut candidate =
            EntityCandidate::new("example:wanderer".parse().expect("a content identity"));
        for _ in 0..(super::MAX_TARGET_DEPTH + 4) {
            candidate = EntityCandidate {
                target: Some(Box::new(candidate)),
                ..EntityCandidate::new("example:wanderer".parse().expect("a content identity"))
            };
        }
        assert!(!predicate.holds(&candidate));
    }

    #[test]
    fn the_item_predicate_holds_for_the_stack_it_names() {
        let predicate =
            ItemPredicate::new("minecraft:lantern".parse().expect("a content identity"));
        let named = ItemCandidate {
            identity: "minecraft:lantern".parse().expect("a content identity"),
            count: 1,
            stored: None,
            durability: None,
        };
        assert!(
            predicate.holds(&named),
            "the original returns false here, which is the inverted identity test"
        );
        let other = ItemCandidate {
            identity: "minecraft:torch".parse().expect("a content identity"),
            count: 1,
            stored: None,
            durability: None,
        };
        assert!(!predicate.holds(&other));
    }

    #[test]
    fn a_durability_bound_other_than_any_fails_outright_for_an_item_that_takes_no_damage() {
        let mut predicate =
            ItemPredicate::new("minecraft:lantern".parse().expect("a content identity"));
        predicate.durability = Bound {
            lower: Some(0),
            upper: None,
        };
        let indestructible = ItemCandidate {
            identity: "minecraft:lantern".parse().expect("a content identity"),
            durability: None,
            count: 1,
            stored: None,
        };
        assert!(
            !predicate.holds(&indestructible),
            "kept deliberately: the original does this and it is defensible"
        );
    }

    #[test]
    fn the_durability_compared_is_the_remaining_amount_rather_than_the_damage_taken() {
        let mut predicate =
            ItemPredicate::new("minecraft:pick".parse().expect("a content identity"));
        predicate.durability = Bound {
            lower: Some(100),
            upper: None,
        };
        let fresh = ItemCandidate {
            identity: "minecraft:pick".parse().expect("a content identity"),
            durability: Some(250),
            count: 1,
            stored: None,
        };
        let worn = ItemCandidate {
            identity: "minecraft:pick".parse().expect("a content identity"),
            durability: Some(5),
            count: 1,
            stored: None,
        };
        assert!(predicate.holds(&fresh));
        assert!(!predicate.holds(&worn));
    }

    #[test]
    fn a_count_bound_is_inclusive_at_both_ends() {
        let mut predicate =
            ItemPredicate::new("minecraft:lantern".parse().expect("a content identity"));
        predicate.count = Bound {
            lower: Some(2),
            upper: Some(4),
        };
        let with = |count| ItemCandidate {
            identity: "minecraft:lantern".parse().expect("a content identity"),
            count,
            stored: None,
            durability: None,
        };
        assert!(predicate.holds(&with(2)));
        assert!(predicate.holds(&with(4)));
        assert!(!predicate.holds(&with(1)));
        assert!(!predicate.holds(&with(5)));
        assert!(Bound::ANY.is_any());
        assert!(Bound::ANY.admits(i32::MIN));
    }

    #[test]
    fn each_composite_predicate_exposes_its_stored_tag_for_inspection() {
        let expected = Tag::compound([("lit", Tag::Byte(1))]);
        let mut block =
            BlockPredicate::new("minecraft:lantern".parse().expect("a content identity"));
        block.stored = Some(DataPredicate::new(expected.clone(), Mode::Lenient));
        assert_eq!(block.stored_tag(), Some(&expected));
        let mut entity =
            EntityPredicate::new("example:wanderer".parse().expect("a content identity"));
        entity.stored = Some(DataPredicate::new(expected.clone(), Mode::Strict));
        assert_eq!(entity.stored_tag(), Some(&expected));
        let mut item = ItemPredicate::new("minecraft:lantern".parse().expect("a content identity"));
        item.stored = Some(DataPredicate::new(expected.clone(), Mode::Lenient));
        assert_eq!(item.stored_tag(), Some(&expected));
        assert!(
            ItemPredicate::new("minecraft:lantern".parse().expect("a content identity"))
                .stored_tag()
                .is_none()
        );
    }
}
