//! Section thirteen: annotated field persistence and the per-world counters.
//!
//! Two mechanisms. A field marked persistable is written into and read out of a
//! compound under its own name, dispatching per field through a type table that
//! is loaded at common setup and extensible by dependents. The format is
//! name-keyed, which is the opposite of the wire format of section six and is
//! the right choice for stored data: a stored compound outlives the build that
//! wrote it, and a field added in the middle must not shift the ones after it.
//!
//! The counters are the entry's own saved-data object: named counters handing
//! out monotonically increasing integers unique within a world, which is the
//! guarantee a counterpart owes and which surviving a restart is the whole point
//! of.

use std::collections::{BTreeMap, BTreeSet};

use ravel_sdk::{JsonValue, json};

use core::fmt;

/// The most named counters one world will hold.
pub const MAX_COUNTERS: usize = 1024;

/// The most persistable fields one object may mark.
pub const MAX_FIELDS: usize = 256;

/// The counter the entry allocates from for itself, once per world opening.
///
/// Section thirteen owes a guarantee — unique, increasing, surviving a restart —
/// and a counter nothing ever allocates from demonstrates none of it in a
/// launched game. This is the entry's own use of its own mechanism, so the
/// guarantee is observable in the world rather than only in a test.
pub const OPENINGS: &str = "world_openings";

/// The field types the table holds before anything is loaded.
pub const PORTABLE_FIELD_TYPES: [&str; 4] = ["truth", "number", "text", "compound"];

/// The per-field type table.
///
/// The portable entries exist as soon as the table does; this platform's
/// additions arrive at common setup, and a dependent adds its own. A field of a
/// platform type persisted before common setup finishes has no entry to read,
/// which is the ordering section two calls load-bearing.
#[derive(Clone, Debug)]
pub struct FieldTypes {
    known: BTreeSet<String>,
    platform_loaded: bool,
}

impl Default for FieldTypes {
    fn default() -> Self {
        Self::new()
    }
}

impl FieldTypes {
    /// The table holding its portable entries and nothing else.
    #[must_use]
    pub fn new() -> Self {
        Self {
            known: PORTABLE_FIELD_TYPES
                .into_iter()
                .map(str::to_owned)
                .collect(),
            platform_loaded: false,
        }
    }

    /// Adds this platform's entries, which common setup does once.
    pub fn load_platform(&mut self, additions: &[&str]) -> bool {
        if self.platform_loaded {
            return false;
        }
        for name in additions {
            self.known.insert((*name).to_owned());
        }
        self.platform_loaded = true;
        true
    }

    /// Whether this platform's additions have been loaded.
    #[must_use]
    pub const fn platform_loaded(&self) -> bool {
        self.platform_loaded
    }

    /// Adds one entry for a dependent's own field type.
    pub fn add(&mut self, name: &str) {
        self.known.insert(name.to_owned());
    }

    /// Whether the table can persist a field of this type.
    #[must_use]
    pub fn holds(&self, name: &str) -> bool {
        self.known.contains(name)
    }

    /// How many entries the table holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.known.len()
    }

    /// Whether the table is empty, which it never is.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.known.is_empty()
    }
}

/// What persistence refuses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistError {
    /// The stored envelope has no counters object.
    NoCounterTable,
    /// The table holds no entry for a field's declared type.
    NoFieldType {
        /// The field.
        field: String,
        /// Its declared type.
        declared: String,
    },
    /// The object marks more fields than one may.
    TooManyFields {
        /// The published limit.
        limit: usize,
    },
    /// A stored compound is missing a marked field.
    MissingField {
        /// The field.
        field: String,
    },
    /// A world holds more named counters than one may.
    TooManyCounters {
        /// The published limit.
        limit: usize,
    },
    /// A marked field was written with no value.
    NoValue {
        /// The field.
        field: String,
    },
    /// A stored counter is not a whole number.
    UnreadableCounter {
        /// The counter.
        name: String,
    },
}

impl fmt::Display for PersistError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCounterTable => {
                formatter.write_str("the stored payload holds no counters table")
            }
            Self::NoFieldType { field, declared } => {
                write!(
                    formatter,
                    "no entry persists {field}, declared as {declared}"
                )
            }
            Self::TooManyFields { limit } => {
                write!(formatter, "an object marks at most {limit} fields")
            }
            Self::MissingField { field } => {
                write!(formatter, "the stored compound holds no {field}")
            }
            Self::NoValue { field } => {
                write!(formatter, "no value was written for {field}")
            }
            Self::UnreadableCounter { name } => {
                write!(
                    formatter,
                    "the counter {name} was stored as something other than a number"
                )
            }
            Self::TooManyCounters { limit } => {
                write!(formatter, "a world holds at most {limit} counters")
            }
        }
    }
}

impl std::error::Error for PersistError {}

/// The fields one object marks as persistable.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Marked {
    fields: BTreeMap<String, String>,
}

impl Marked {
    /// An object marking nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks one field, naming its declared type.
    ///
    /// # Errors
    ///
    /// Returns [`PersistError::TooManyFields`] past the published limit.
    pub fn mark(mut self, field: &str, declared: &str) -> Result<Self, PersistError> {
        if self.fields.len() >= MAX_FIELDS && !self.fields.contains_key(field) {
            return Err(PersistError::TooManyFields { limit: MAX_FIELDS });
        }
        self.fields.insert(field.to_owned(), declared.to_owned());
        Ok(self)
    }

    /// The marked fields and their declared types.
    pub fn fields(&self) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.fields
            .iter()
            .map(|(field, declared)| (field.as_str(), declared.as_str()))
    }

    /// Writes the marked fields into a compound, each under its own name.
    ///
    /// # Errors
    ///
    /// Returns [`PersistError::NoFieldType`] when the table holds no entry for a
    /// field's declared type, and [`PersistError::NoValue`] when a marked field
    /// has no value: a null written for a forgotten field reads back as a
    /// success on every later load, which is the one failure a stored format
    /// cannot recover from.
    pub fn write(
        &self,
        table: &FieldTypes,
        values: &BTreeMap<String, JsonValue>,
    ) -> Result<JsonValue, PersistError> {
        let mut compound = json!({});
        for (field, declared) in self.fields() {
            if !table.holds(declared) {
                return Err(PersistError::NoFieldType {
                    field: field.to_owned(),
                    declared: declared.to_owned(),
                });
            }
            let value = values
                .get(field)
                .cloned()
                .ok_or_else(|| PersistError::NoValue {
                    field: field.to_owned(),
                })?;
            if let Some(object) = compound.as_object_mut() {
                object.insert(field.to_owned(), value);
            }
        }
        Ok(compound)
    }

    /// Reads the marked fields back out of a compound.
    ///
    /// # Errors
    ///
    /// Returns [`PersistError::NoFieldType`] as above, and
    /// [`PersistError::MissingField`] when the stored compound is missing one.
    pub fn read(
        &self,
        table: &FieldTypes,
        compound: &JsonValue,
    ) -> Result<BTreeMap<String, JsonValue>, PersistError> {
        let mut values = BTreeMap::new();
        for (field, declared) in self.fields() {
            if !table.holds(declared) {
                return Err(PersistError::NoFieldType {
                    field: field.to_owned(),
                    declared: declared.to_owned(),
                });
            }
            let value = compound
                .get(field)
                .ok_or_else(|| PersistError::MissingField {
                    field: field.to_owned(),
                })?;
            values.insert(field.to_owned(), value.clone());
        }
        Ok(values)
    }
}

/// The entry's own saved-data object: named counters unique within a world.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Counters {
    held: BTreeMap<String, u64>,
}

impl Counters {
    /// A world with no counter yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Hands out the next value of one named counter.
    ///
    /// The values a counter hands out are unique within a world and increasing,
    /// and the sequence survives a restart because it is saved. A counter that
    /// reached the end of its range hands out nothing rather than repeating a
    /// value a dependent has already stored.
    ///
    /// # Errors
    ///
    /// Returns [`PersistError::TooManyCounters`] when a world would hold more
    /// named counters than the published limit.
    pub fn next(&mut self, name: &str) -> Result<Option<u64>, PersistError> {
        if !self.held.contains_key(name) && self.held.len() >= MAX_COUNTERS {
            return Err(PersistError::TooManyCounters {
                limit: MAX_COUNTERS,
            });
        }
        let held = self.held.entry(name.to_owned()).or_insert(0);
        let Some(next) = held.checked_add(1) else {
            return Ok(None);
        };
        *held = next;
        Ok(Some(next))
    }

    /// The last value one counter handed out, or zero if it never has.
    #[must_use]
    pub fn last(&self, name: &str) -> u64 {
        self.held.get(name).copied().unwrap_or(0)
    }

    /// How many named counters this world holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether no counter has handed out anything.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Writes the counters for saving.
    #[must_use]
    pub fn save(&self) -> JsonValue {
        let mut counters = json!({});
        if let Some(object) = counters.as_object_mut() {
            for (name, held) in &self.held {
                object.insert(name.clone(), json!(held));
            }
        }
        json!({ "counters": counters })
    }

    /// Reads the counters back.
    ///
    /// A counter that cannot be read is refused rather than reset. Reading it as
    /// zero would restart a sequence a dependent has already stored values out
    /// of, and the second run of that sequence hands out identifiers a world
    /// still holds. The same goes for a stored set larger than the limit: taking
    /// the first thousand and dropping the rest rewrites the world short at the
    /// next save, and the dropped counters restart from zero.
    ///
    /// # Errors
    ///
    /// Returns [`PersistError::NoCounterTable`] for a missing or malformed table,
    /// [`PersistError::UnreadableCounter`] for a non-integer counter, or
    /// [`PersistError::TooManyCounters`] for a set larger than [`MAX_COUNTERS`].
    pub fn load(&mut self, stored: &JsonValue) -> Result<(), PersistError> {
        self.held.clear();
        let Some(counters) = stored.get("counters").and_then(JsonValue::as_object) else {
            return Err(PersistError::NoCounterTable);
        };
        if counters.len() > MAX_COUNTERS {
            return Err(PersistError::TooManyCounters {
                limit: MAX_COUNTERS,
            });
        }
        for (name, held) in counters {
            let held = held
                .as_u64()
                .ok_or_else(|| PersistError::UnreadableCounter { name: name.clone() })?;
            self.held.insert(name.clone(), held);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_portable_field_types_exist_before_the_platform_ones_are_loaded() {
        let mut table = FieldTypes::new();
        assert_eq!(table.len(), PORTABLE_FIELD_TYPES.len());
        assert!(!table.platform_loaded());
        for name in PORTABLE_FIELD_TYPES {
            assert!(table.holds(name));
        }
        assert!(!table.holds("position"));
        assert!(table.load_platform(&["position"]));
        assert!(table.holds("position"));
        assert!(
            !table.load_platform(&["position"]),
            "common setup runs once"
        );
    }

    #[test]
    fn a_dependent_adds_its_own_field_type() {
        let mut table = FieldTypes::new();
        table.add("mine");
        assert!(table.holds("mine"));
    }

    #[test]
    fn a_field_of_a_type_the_table_lacks_is_refused_rather_than_written_untyped() {
        let table = FieldTypes::new();
        let marked = Marked::new()
            .mark("held", "nothing_declares_this")
            .expect("one field");
        assert_eq!(
            marked.write(&table, &BTreeMap::new()),
            Err(PersistError::NoFieldType {
                field: "held".to_owned(),
                declared: "nothing_declares_this".to_owned(),
            })
        );
    }

    #[test]
    fn a_marked_field_with_no_value_is_refused_rather_than_written_as_null() {
        let table = FieldTypes::new();
        let marked = Marked::new().mark("count", "number").expect("one field");
        assert_eq!(
            marked.write(&table, &BTreeMap::new()),
            Err(PersistError::NoValue {
                field: "count".to_owned()
            }),
            "a null written here reads back as a success on every later load"
        );
        let values = BTreeMap::from([("count".to_owned(), json!(4))]);
        assert_eq!(
            marked
                .write(&table, &values)
                .expect("a value for every field")["count"],
            json!(4)
        );
    }

    #[test]
    fn the_stored_form_is_name_keyed_so_a_new_field_shifts_nothing() {
        let table = FieldTypes::new();
        let values = BTreeMap::from([
            ("count".to_owned(), json!(4)),
            ("name".to_owned(), json!("one")),
        ]);
        let two = Marked::new()
            .mark("count", "number")
            .and_then(|marked| marked.mark("name", "text"))
            .expect("two fields");
        let stored = two.write(&table, &values).expect("both types are known");
        assert_eq!(stored["count"], json!(4));
        assert_eq!(stored["name"], json!("one"));

        let three = Marked::new()
            .mark("added", "truth")
            .and_then(|marked| marked.mark("count", "number"))
            .and_then(|marked| marked.mark("name", "text"))
            .expect("three fields");
        let mut later = values.clone();
        later.insert("added".to_owned(), json!(true));
        let stored = three.write(&table, &later).expect("every type is known");
        assert_eq!(
            stored["count"],
            json!(4),
            "a field added ahead of it shifted nothing"
        );
        assert_eq!(two.read(&table, &stored).expect("still readable"), values);
    }

    #[test]
    fn a_stored_compound_missing_a_marked_field_is_refused_rather_than_defaulted() {
        let table = FieldTypes::new();
        let marked = Marked::new().mark("count", "number").expect("one field");
        assert_eq!(
            marked.read(&table, &json!({})),
            Err(PersistError::MissingField {
                field: "count".to_owned()
            })
        );
    }

    #[test]
    fn an_object_refuses_more_marked_fields_than_it_publishes() {
        let mut marked = Marked::new();
        for field in 0..MAX_FIELDS {
            marked = marked
                .mark(&format!("field{field:04}"), "number")
                .expect("inside the limit");
        }
        assert_eq!(
            marked.mark("one_too_many", "number"),
            Err(PersistError::TooManyFields { limit: MAX_FIELDS })
        );
    }

    #[test]
    fn a_counter_hands_out_increasing_values_and_never_repeats_one() {
        let mut counters = Counters::new();
        let mut seen = Vec::new();
        for _ in 0..16 {
            let handed = counters
                .next("identifiers")
                .expect("inside the limit")
                .expect("inside the range");
            assert!(!seen.contains(&handed), "a counter repeated {handed}");
            if let Some(last) = seen.last() {
                assert!(handed > *last, "a counter went backwards");
            }
            seen.push(handed);
        }
        assert_eq!(counters.last("identifiers"), 16);
    }

    #[test]
    fn two_named_counters_are_kept_apart() {
        let mut counters = Counters::new();
        assert_eq!(counters.next("one"), Ok(Some(1)));
        assert_eq!(counters.next("two"), Ok(Some(1)));
        assert_eq!(counters.next("one"), Ok(Some(2)));
        assert_eq!(counters.len(), 2);
    }

    #[test]
    fn a_counter_survives_a_save_and_a_load_and_carries_on_where_it_was() {
        let mut counters = Counters::new();
        for _ in 0..5 {
            counters.next("identifiers").expect("inside the limit");
        }
        let saved = counters.save();
        let mut reopened = Counters::new();
        reopened.load(&saved).expect("a set this build wrote");
        assert_eq!(reopened.last("identifiers"), 5);
        assert_eq!(reopened.next("identifiers"), Ok(Some(6)));
    }

    #[test]
    fn a_stored_counter_that_is_not_a_number_is_refused_rather_than_restarted() {
        let mut counters = Counters::new();
        assert_eq!(
            counters.load(&json!({ "counters": { "one": "not a number" } })),
            Err(PersistError::UnreadableCounter {
                name: "one".to_owned()
            }),
            "reading it as zero hands out identifiers the world already stored"
        );
        assert_eq!(
            counters.next("one"),
            Ok(Some(1)),
            "the refusal leaves the counters empty, so a caller that ignores it \
             is not silently handed a used value from a half-loaded set"
        );
    }

    #[test]
    fn a_missing_or_malformed_counter_table_is_refused() {
        for stored in [json!({}), json!({ "counters": "not a table" })] {
            let mut counters = Counters::new();
            assert_eq!(counters.load(&stored), Err(PersistError::NoCounterTable));
        }
        let mut counters = Counters::new();
        assert_eq!(counters.load(&json!({ "counters": {} })), Ok(()));
        assert!(counters.is_empty());
    }

    #[test]
    fn a_stored_set_larger_than_the_limit_is_refused_rather_than_read_short() {
        let mut stored = json!({});
        if let Some(object) = stored.as_object_mut() {
            for name in 0..=MAX_COUNTERS {
                object.insert(format!("counter{name:04}"), json!(500));
            }
        }
        let mut counters = Counters::new();
        assert_eq!(
            counters.load(&json!({ "counters": stored })),
            Err(PersistError::TooManyCounters {
                limit: MAX_COUNTERS
            }),
            "reading the first thousand rewrites the world short at the next save"
        );
        assert!(counters.is_empty());
    }

    #[test]
    fn a_counter_at_the_end_of_its_range_hands_out_nothing_rather_than_repeating() {
        let mut counters = Counters::new();
        counters
            .load(&json!({ "counters": { "one": u64::MAX } }))
            .expect("one readable counter");
        assert_eq!(counters.next("one"), Ok(None));
        assert_eq!(counters.last("one"), u64::MAX);
    }

    #[test]
    fn a_world_refuses_more_named_counters_than_it_publishes() {
        let mut counters = Counters::new();
        for name in 0..MAX_COUNTERS {
            counters
                .next(&format!("counter{name:04}"))
                .expect("inside the limit");
        }
        assert_eq!(
            counters.next("one_too_many"),
            Err(PersistError::TooManyCounters {
                limit: MAX_COUNTERS
            })
        );
        assert_eq!(
            counters.next("counter0000"),
            Ok(Some(2)),
            "an existing counter is not refused by the limit"
        );
    }
}
