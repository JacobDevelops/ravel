//! Section 6.4's shared data comparison, and the values it compares.
//!
//! Both of the two kinds that carry data compare it through one routine, and a
//! server setting changes the answer, so the routine is a behaviour surface
//! rather than a helper. Section 6.4 states it in four steps and all four are
//! here.
//!
//! Step four is where the specification records defect seven: the original
//! falls back to an identity hash for a value type with no natural order, so two
//! structurally equal values compare unequal after a copy and items that should
//! match do not. [`Value`] has a total order over every shape it admits, so the
//! fallback has nothing to fall back to and the defect cannot be expressed.

use core::cmp::Ordering;
use core::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};

/// The most component kinds one instance's data may carry.
///
/// A decoded map is untrusted input and this is the ceiling it is held to before
/// anything is allocated for it. The number is a disclosed ceiling rather than a
/// measured one: the original states no limit, and a map larger than this is far
/// past anything a game object carries.
pub const MAX_COMPONENTS: usize = 256;

/// The most nested levels one component value may hold.
///
/// Every walk over a value is recursive, so every walk checks this.
pub const MAX_VALUE_DEPTH: u32 = 16;

/// The most component-kind names the ignore list of section three may name.
pub const MAX_IGNORED: usize = 128;

/// What a data map refuses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refused {
    /// The map names more component kinds than [`MAX_COMPONENTS`].
    TooManyComponents {
        /// How many it named.
        named: usize,
        /// The published ceiling.
        limit: usize,
    },
    /// A value nests deeper than [`MAX_VALUE_DEPTH`].
    TooDeep {
        /// The published ceiling.
        limit: u32,
    },
    /// The ignore list names more kinds than [`MAX_IGNORED`].
    TooManyIgnored {
        /// How many it named.
        named: usize,
        /// The published ceiling.
        limit: usize,
    },
}

impl core::fmt::Display for Refused {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooManyComponents { named, limit } => {
                write!(
                    formatter,
                    "{named} component kinds is past the {limit} admitted"
                )
            }
            Self::TooDeep { limit } => {
                write!(
                    formatter,
                    "a component value nests past the {limit} admitted"
                )
            }
            Self::TooManyIgnored { named, limit } => {
                write!(
                    formatter,
                    "{named} ignored kinds is past the {limit} admitted"
                )
            }
        }
    }
}

/// One component value.
///
/// The shapes are the ones a typed data component carries. What matters about
/// this type is not the shapes but that it has a total order over all of them,
/// including a decimal that is not a number: section 6.4's step four has no
/// unordered case to fall through to.
#[derive(Clone, Debug)]
pub enum Value {
    /// A truth value.
    Truth(bool),
    /// A whole number.
    Whole(i64),
    /// A decimal, ordered by [`f64::total_cmp`] so that every value including
    /// one that is not a number sits somewhere in the order.
    Decimal(f64),
    /// Text.
    Text(String),
    /// An ordered sequence.
    Sequence(Vec<Value>),
    /// A named record.
    Record(BTreeMap<String, Value>),
}

impl Value {
    const fn rank(&self) -> u8 {
        match self {
            Self::Truth(_) => 0,
            Self::Whole(_) => 1,
            Self::Decimal(_) => 2,
            Self::Text(_) => 3,
            Self::Sequence(_) => 4,
            Self::Record(_) => 5,
        }
    }

    /// How deeply this value nests, counting itself as one level.
    #[must_use]
    pub fn depth(&self) -> u32 {
        match self {
            Self::Truth(_) | Self::Whole(_) | Self::Decimal(_) | Self::Text(_) => 1,
            Self::Sequence(values) => 1 + values.iter().map(Self::depth).max().unwrap_or_default(),
            Self::Record(fields) => 1 + fields.values().map(Self::depth).max().unwrap_or_default(),
        }
    }

    /// Whether this value nests no deeper than [`MAX_VALUE_DEPTH`].
    ///
    /// # Errors
    ///
    /// Returns [`Refused::TooDeep`] when it does. Depth is measured before any
    /// comparison or digest walks the value, so no walk can run away.
    pub fn bounded(&self) -> Result<(), Refused> {
        if self.depth() > MAX_VALUE_DEPTH {
            return Err(Refused::TooDeep {
                limit: MAX_VALUE_DEPTH,
            });
        }
        Ok(())
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Value {}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Truth(left), Self::Truth(right)) => left.cmp(right),
            (Self::Whole(left), Self::Whole(right)) => left.cmp(right),
            (Self::Decimal(left), Self::Decimal(right)) => left.total_cmp(right),
            (Self::Text(left), Self::Text(right)) => left.cmp(right),
            (Self::Sequence(left), Self::Sequence(right)) => left.cmp(right),
            (Self::Record(left), Self::Record(right)) => left.cmp(right),
            _ => self.rank().cmp(&other.rank()),
        }
    }
}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.rank().hash(state);
        match self {
            Self::Truth(value) => value.hash(state),
            Self::Whole(value) => value.hash(state),
            // By bits, so that two decimals the order tells apart digest apart
            // and two it calls equal digest alike.
            Self::Decimal(value) => value.to_bits().hash(state),
            Self::Text(value) => value.hash(state),
            Self::Sequence(values) => values.hash(state),
            Self::Record(fields) => fields.hash(state),
        }
    }
}

/// The typed data one instance carries, keyed by component-kind name.
pub type Components = BTreeMap<String, Value>;

/// Builds a data map, held to the published ceilings.
///
/// # Errors
///
/// Returns [`Refused::TooManyComponents`] past [`MAX_COMPONENTS`] and
/// [`Refused::TooDeep`] for a value nesting past [`MAX_VALUE_DEPTH`]. Both are
/// checked before the map is built, because the caller is usually a decoder.
pub fn components(
    entries: impl IntoIterator<Item = (String, Value)>,
) -> Result<Components, Refused> {
    let mut map = Components::new();
    for (name, value) in entries {
        if map.len() >= MAX_COMPONENTS && !map.contains_key(&name) {
            return Err(Refused::TooManyComponents {
                named: map.len() + 1,
                limit: MAX_COMPONENTS,
            });
        }
        value.bounded()?;
        map.insert(name, value);
    }
    Ok(map)
}

/// Section 6.4's comparison, holding the ignore list in force.
///
/// The list is resolved when the comparator is built and never afterwards,
/// which is what the original does. What makes that harmless is section three's
/// rule that the whole comparator is rebuilt on every settings load, so
/// [`Comparator::new`] is the only way to get one and there is no way to change
/// a built one's list.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Comparator {
    ignored: BTreeSet<String>,
}

impl Comparator {
    /// Builds a comparator ignoring the named component kinds.
    ///
    /// # Errors
    ///
    /// Returns [`Refused::TooManyIgnored`] past [`MAX_IGNORED`]. The list comes
    /// from a resolved setting, which is not a trusted input.
    pub fn new(ignored: impl IntoIterator<Item = String>) -> Result<Self, Refused> {
        let ignored: BTreeSet<String> = ignored.into_iter().collect();
        if ignored.len() > MAX_IGNORED {
            return Err(Refused::TooManyIgnored {
                named: ignored.len(),
                limit: MAX_IGNORED,
            });
        }
        Ok(Self { ignored })
    }

    /// A comparator ignoring nothing.
    #[must_use]
    pub fn ignoring_nothing() -> Self {
        Self::default()
    }

    /// The component kinds this comparator ignores.
    pub fn ignored(&self) -> impl Iterator<Item = &str> + '_ {
        self.ignored.iter().map(String::as_str)
    }

    /// Whether one side actually holds a kind this comparator ignores.
    ///
    /// Step two of section 6.4: a side is filtered only when filtering it would
    /// change it.
    #[must_use]
    pub fn filters(&self, side: &Components) -> bool {
        side.keys().any(|name| self.ignored.contains(name))
    }

    fn surviving<'data>(&self, side: &'data Components) -> Vec<(&'data str, &'data Value)> {
        if self.filters(side) {
            side.iter()
                .filter(|(name, _)| !self.ignored.contains(name.as_str()))
                .map(|(name, value)| (name.as_str(), value))
                .collect()
        } else {
            side.iter()
                .map(|(name, value)| (name.as_str(), value))
                .collect()
        }
    }

    /// Compares two data maps by section 6.4's four steps.
    #[must_use]
    pub fn compare(&self, left: &Components, right: &Components) -> Ordering {
        // 1. Equal maps compare equal immediately.
        if left == right {
            return Ordering::Equal;
        }
        // 2. Each side is filtered only when it holds an ignored kind.
        let left = self.surviving(left);
        let right = self.surviving(right);
        // 3. Differing key sets are compared as sorted name lists, which puts
        //    the shorter set first on a common prefix.
        let left_names: Vec<&str> = left.iter().map(|(name, _)| *name).collect();
        let right_names: Vec<&str> = right.iter().map(|(name, _)| *name).collect();
        if left_names != right_names {
            return left_names.cmp(&right_names);
        }
        // 4. Otherwise the shared values, structurally. This is where the
        //    original falls back to an identity hash.
        for ((_, left_value), (_, right_value)) in left.iter().zip(right.iter()) {
            let verdict = left_value.cmp(right_value);
            if verdict != Ordering::Equal {
                return verdict;
            }
        }
        Ordering::Equal
    }

    /// Whether two data maps compare equal.
    #[must_use]
    pub fn equal(&self, left: &Components, right: &Components) -> bool {
        self.compare(left, right) == Ordering::Equal
    }

    /// A digest agreeing with [`Comparator::compare`].
    ///
    /// Two maps this comparator calls equal digest alike, because both digest
    /// their surviving entries and nothing else.
    #[must_use]
    pub fn digest(&self, side: &Components) -> u64 {
        let mut hasher = DefaultHasher::new();
        for (name, value) in self.surviving(side) {
            name.hash(&mut hasher);
            value.hash(&mut hasher);
        }
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> Value {
        Value::Text(value.to_owned())
    }

    fn map(entries: &[(&str, Value)]) -> Components {
        entries
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect()
    }

    #[test]
    fn identical_maps_compare_equal_without_reaching_the_value_step() {
        let comparator = Comparator::ignoring_nothing();
        let one = map(&[("depth", Value::Whole(3))]);
        assert_eq!(comparator.compare(&one, &one.clone()), Ordering::Equal);
    }

    #[test]
    fn a_copy_of_a_structural_value_compares_equal() {
        // Defect seven: the original falls back to an identity hash here and
        // answers unequal for two distinct objects holding the same thing.
        let comparator = Comparator::ignoring_nothing();
        let left = map(&[(
            "shape",
            Value::Record(BTreeMap::from([("side".to_owned(), Value::Whole(4))])),
        )]);
        let right = left.clone();
        assert!(comparator.equal(&left, &right));
        assert_eq!(comparator.digest(&left), comparator.digest(&right));
    }

    #[test]
    fn an_ignored_kind_is_dropped_from_the_comparison() {
        let comparator = Comparator::new(["wear".to_owned()]).expect("a small list");
        let left = map(&[("name", text("axe")), ("wear", Value::Whole(1))]);
        let right = map(&[("name", text("axe")), ("wear", Value::Whole(99))]);
        assert!(comparator.equal(&left, &right));
        assert_eq!(comparator.digest(&left), comparator.digest(&right));
    }

    #[test]
    fn a_side_holding_no_ignored_kind_is_not_filtered() {
        let comparator = Comparator::new(["wear".to_owned()]).expect("a small list");
        let plain = map(&[("name", text("axe"))]);
        assert!(!comparator.filters(&plain));
        assert!(comparator.filters(&map(&[("wear", Value::Whole(1))])));
    }

    #[test]
    fn differing_key_sets_compare_as_sorted_name_lists() {
        let comparator = Comparator::ignoring_nothing();
        let shorter = map(&[("a", Value::Whole(1))]);
        let longer = map(&[("a", Value::Whole(1)), ("b", Value::Whole(2))]);
        assert_eq!(comparator.compare(&shorter, &longer), Ordering::Less);
        assert_eq!(comparator.compare(&longer, &shorter), Ordering::Greater);
    }

    #[test]
    fn a_differing_name_decides_before_any_value_is_read() {
        let comparator = Comparator::ignoring_nothing();
        let left = map(&[("a", Value::Whole(9))]);
        let right = map(&[("b", Value::Whole(0))]);
        assert_eq!(comparator.compare(&left, &right), Ordering::Less);
    }

    #[test]
    fn shared_values_decide_when_the_names_agree() {
        let comparator = Comparator::ignoring_nothing();
        let left = map(&[("a", Value::Whole(1))]);
        let right = map(&[("a", Value::Whole(2))]);
        assert_eq!(comparator.compare(&left, &right), Ordering::Less);
    }

    #[test]
    fn the_order_is_total_across_shapes() {
        let ordered = [
            Value::Truth(false),
            Value::Truth(true),
            Value::Whole(-1),
            Value::Decimal(1.5),
            text("a"),
            Value::Sequence(vec![Value::Whole(1)]),
            Value::Record(BTreeMap::new()),
        ];
        for window in ordered.windows(2) {
            assert_eq!(window[0].cmp(&window[1]), Ordering::Less);
        }
    }

    #[test]
    fn a_decimal_that_is_not_a_number_sits_in_the_order_and_never_equals_itself_falsely() {
        let absent = Value::Decimal(f64::NAN);
        assert_eq!(absent.cmp(&absent.clone()), Ordering::Equal);
        assert_eq!(
            Value::Decimal(0.0).cmp(&Value::Decimal(-0.0)),
            Ordering::Greater
        );
        assert!(absent > Value::Decimal(f64::INFINITY));
    }

    #[test]
    fn a_decimal_that_is_not_a_number_digests_alike_for_the_same_bits() {
        let comparator = Comparator::ignoring_nothing();
        let left = map(&[("rate", Value::Decimal(f64::NAN))]);
        let right = left.clone();
        assert!(comparator.equal(&left, &right));
        assert_eq!(comparator.digest(&left), comparator.digest(&right));
    }

    #[test]
    fn depth_is_measured_and_a_deep_value_is_refused() {
        let mut value = Value::Whole(0);
        for _ in 0..MAX_VALUE_DEPTH {
            value = Value::Sequence(vec![value]);
        }
        assert_eq!(value.depth(), MAX_VALUE_DEPTH + 1);
        assert_eq!(
            value.bounded(),
            Err(Refused::TooDeep {
                limit: MAX_VALUE_DEPTH
            })
        );
    }

    #[test]
    fn a_value_at_the_bound_is_admitted() {
        let mut value = Value::Whole(0);
        for _ in 1..MAX_VALUE_DEPTH {
            value = Value::Sequence(vec![value]);
        }
        assert_eq!(value.depth(), MAX_VALUE_DEPTH);
        assert_eq!(value.bounded(), Ok(()));
    }

    #[test]
    fn a_map_past_the_component_ceiling_is_refused_before_it_is_built() {
        let entries = (0..=MAX_COMPONENTS).map(|index| (format!("k{index}"), Value::Whole(0)));
        assert_eq!(
            components(entries),
            Err(Refused::TooManyComponents {
                named: MAX_COMPONENTS + 1,
                limit: MAX_COMPONENTS
            })
        );
    }

    #[test]
    fn a_map_at_the_component_ceiling_is_admitted() {
        let entries = (0..MAX_COMPONENTS).map(|index| (format!("k{index}"), Value::Whole(0)));
        assert_eq!(components(entries).map(|map| map.len()), Ok(MAX_COMPONENTS));
    }

    #[test]
    fn an_ignore_list_past_its_ceiling_is_refused() {
        let names = (0..=MAX_IGNORED).map(|index| format!("k{index}"));
        assert_eq!(
            Comparator::new(names),
            Err(Refused::TooManyIgnored {
                named: MAX_IGNORED + 1,
                limit: MAX_IGNORED
            })
        );
    }

    #[test]
    fn a_repeated_name_does_not_count_twice_against_the_ceiling() {
        let entries = (0..MAX_COMPONENTS + 4).map(|index| {
            (
                format!("k{}", index % MAX_COMPONENTS),
                Value::Whole(i64::try_from(index).expect("a small index")),
            )
        });
        assert_eq!(components(entries).map(|map| map.len()), Ok(MAX_COMPONENTS));
    }

    #[test]
    fn refusals_say_what_was_wrong() {
        assert!(
            Refused::TooDeep { limit: 3 }
                .to_string()
                .contains("nests past")
        );
        assert!(
            Refused::TooManyComponents { named: 9, limit: 8 }
                .to_string()
                .contains("component kinds")
        );
        assert!(
            Refused::TooManyIgnored { named: 9, limit: 8 }
                .to_string()
                .contains("ignored kinds")
        );
    }
}
