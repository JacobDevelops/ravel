//! The five persistent records of §4, the stack they are written in terms of,
//! and the store the world keeps them in.
//!
//! Ravel owns saved data and its versioning, so what survives here is the part a
//! saved world and a dependent both observe: what each record holds, which of
//! them tolerate an absent entry on the way in, and which refuses one. §18 of the
//! specification withholds the serialised spellings deliberately, so the record
//! names below are this package's own and no alias is declared for them.
//!
//! [`Store`] is where §4's double registration stops being a predicate and
//! becomes behaviour. A record is written against the block or item that carries
//! it, and breaking that block forgets everything at its key *except* the one
//! record §4 registers twice, which is what makes infusion progress survive the
//! block being broken and replaced. The entry hands the store to Ravel's
//! saved-data seam rather than keeping a copy of the world.
//!
//! Every decoder is a trust boundary. A saved document is a file an operator
//! edits by hand and a payload is bytes a client sent, so each list is bounded
//! before it is allocated and each text field is bounded before it is kept.

use std::collections::BTreeMap;
use std::fmt;

use ravel_sdk::JsonValue;

/// The most entries any list-valued record may carry.
///
/// A saved list is untrusted input: a document declaring a count larger than the
/// machine can allocate ends the process in the allocator rather than failing a
/// decode. The ceiling is far above any store this package builds, and it is a
/// number to raise rather than a rule to rewrite.
pub const MAX_ENTRIES: usize = 4096;

/// The most characters an item identity, a tag name or a component name may
/// carry.
pub const MAX_NAME: usize = 256;

/// The most components one stack may carry.
pub const MAX_COMPONENTS: usize = 64;

/// The most keys one record is stored under at once.
///
/// A key is a block or an item in a world, so the store grows with the world
/// rather than with anything this entry controls, and a saved document naming
/// more than this is a document to refuse rather than allocate for. The ceiling
/// is a declared number to raise.
pub const MAX_STORED: usize = 4096;

/// The five records this entry persists, named for what each holds.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Record {
    /// An ordered list of stacks with the empty entries kept, §9.
    PatternContents,
    /// A list of stacks, a list of tag names and four mode flags, §8.
    MatchRule,
    /// One count of completed upgrade steps, §7.
    InfusionProgress,
    /// Six stacks and the index of the selected one, §10.
    CarrierContents,
    /// One dimension-qualified position, §11.
    SelectedPosition,
}

impl Record {
    /// Every record, in the order §4 numbers them.
    pub const ALL: [Self; 5] = [
        Self::PatternContents,
        Self::MatchRule,
        Self::InfusionProgress,
        Self::CarrierContents,
        Self::SelectedPosition,
    ];

    /// Returns this record's own name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PatternContents => "pattern_contents",
            Self::MatchRule => "match_rule",
            Self::InfusionProgress => "infusion_progress",
            Self::CarrierContents => "carrier_contents",
            Self::SelectedPosition => "selected_position",
        }
    }

    /// Returns whether an absent entry inside this record's persisted form is
    /// read as an empty slot rather than refusing the whole record.
    ///
    /// Three of the five tolerate one and the match rule does not, which is the
    /// division §4 states. Keeping the strict one strict is what makes a corrupt
    /// rule loud instead of silently permissive: a match rule that lost an entry
    /// answers a different question from the one the player wrote.
    #[must_use]
    pub const fn tolerates_absent(self) -> bool {
        !matches!(self, Self::MatchRule)
    }

    /// Returns whether this record is also kept against the block that carries
    /// it, so the value survives that block being broken and replaced.
    ///
    /// Exactly one is, which is §4's double registration read as behaviour
    /// rather than as two registry claims.
    #[must_use]
    pub const fn survives_replacement(self) -> bool {
        matches!(self, Self::InfusionProgress)
    }
}

impl fmt::Display for Record {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Returns the record one of this entry's names spells, or nothing.
#[must_use]
pub fn record_named(name: &str) -> Option<Record> {
    Record::ALL
        .into_iter()
        .find(|record| record.as_str() == name)
}

/// The five records as one world keeps them, keyed by what carries each.
///
/// The key is the caller's: a block position for a block entity's record and an
/// item's own identity for an item's. Nothing here interprets it beyond bounding
/// it, because what a key means is the caller's and what a store owes is that
/// the same key reads back what was written under it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Store {
    records: BTreeMap<String, BTreeMap<String, JsonValue>>,
}

impl Store {
    /// Builds an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes one record against one key.
    ///
    /// The document is held to the record's own decoding rules before it is
    /// kept, so a store never holds a body the record would refuse on the way
    /// back out.
    ///
    /// # Errors
    ///
    /// Returns [`RecordRefused::Name`] for an empty or over-long key and
    /// [`RecordRefused::TooMany`] once the record holds [`MAX_STORED`] keys.
    pub fn write(
        &mut self,
        record: Record,
        key: &str,
        document: JsonValue,
    ) -> Result<(), RecordRefused> {
        check_name(key)?;
        let entries = self.records.entry(record.as_str().to_owned()).or_default();
        if !entries.contains_key(key) && entries.len() >= MAX_STORED {
            return Err(RecordRefused::TooMany {
                limit: MAX_STORED,
                found: entries.len().saturating_add(1),
            });
        }
        entries.insert(key.to_owned(), document);
        Ok(())
    }

    /// Reads one record back.
    #[must_use]
    pub fn read(&self, record: Record, key: &str) -> Option<&JsonValue> {
        self.records.get(record.as_str())?.get(key)
    }

    /// Returns how many keys one record is stored under.
    #[must_use]
    pub fn stored(&self, record: Record) -> usize {
        self.records.get(record.as_str()).map_or(0, BTreeMap::len)
    }

    /// Forgets everything stored at one key that does not survive replacement.
    ///
    /// §4 registers one of the five twice, once as an item record and once as a
    /// per-owner attachment, and says in as many words that that is how progress
    /// survives the block being broken and replaced. Here that is one branch:
    /// the records that do not survive are dropped and the one that does stays,
    /// so a block put back at the same position finds its progress where it left
    /// it. The count is what was forgotten.
    pub fn break_block(&mut self, key: &str) -> usize {
        let mut forgotten = 0;
        for record in Record::ALL {
            if record.survives_replacement() {
                continue;
            }
            if let Some(entries) = self.records.get_mut(record.as_str())
                && entries.remove(key).is_some()
            {
                forgotten += 1;
            }
        }
        forgotten
    }

    /// Writes the whole store as one document.
    #[must_use]
    pub fn to_saved(&self) -> JsonValue {
        let mut document = serde_map();
        for record in Record::ALL {
            let Some(entries) = self.records.get(record.as_str()) else {
                continue;
            };
            let mut written = serde_map();
            for (key, value) in entries {
                if let Some(object) = written.as_object_mut() {
                    object.insert(key.clone(), value.clone());
                }
            }
            if let Some(object) = document.as_object_mut() {
                object.insert(record.as_str().to_owned(), written);
            }
        }
        document
    }

    /// Reads a whole store back.
    ///
    /// A saved document is a file an operator edits by hand, so every key is
    /// bounded before it is kept and every record's key count is bounded before
    /// anything is allocated. A record name the document carries and this build
    /// does not know is refused rather than skipped: a world written by a build
    /// with a sixth record is a world this one does not read.
    ///
    /// # Errors
    ///
    /// Returns [`RecordRefused::Shape`] for a document that is not an object or
    /// carries a record this build does not know, [`RecordRefused::Name`] for an
    /// unusable key, and [`RecordRefused::TooMany`] past [`MAX_STORED`].
    pub fn from_saved(document: &JsonValue) -> Result<Self, RecordRefused> {
        let object = document
            .as_object()
            .ok_or(RecordRefused::Shape { record: None })?;
        let mut store = Self::new();
        for (name, written) in object {
            let record = record_named(name).ok_or(RecordRefused::Shape { record: None })?;
            let entries = written.as_object().ok_or(RecordRefused::Shape {
                record: Some(record),
            })?;
            if entries.len() > MAX_STORED {
                return Err(RecordRefused::TooMany {
                    limit: MAX_STORED,
                    found: entries.len(),
                });
            }
            for (key, value) in entries {
                store.write(record, key, value.clone())?;
            }
        }
        Ok(store)
    }
}

fn serde_map() -> JsonValue {
    ravel_sdk::json!({})
}

/// One stack of items, as every record here writes one.
///
/// Ravel publishes no item stack, so this is the portable shape the decision
/// rules of §7 to §11 are stated over: an identity, a count, a damage value and
/// the components that make two otherwise equal stacks unequal. The two boolean
/// declarations are what §10 and §15 have a dependent supply, and they are read
/// here rather than cast to.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Stack {
    /// What the stack is, namespaced by whoever registered it.
    pub identity: String,
    /// How many, never zero for a present stack.
    pub count: u8,
    /// The wear on the stack, where the identity has any.
    pub damage: i32,
    /// The named components two stacks are compared on.
    pub components: BTreeMap<String, String>,
    /// Whether the identity declares itself installable in the carrier of §10.
    pub carrier_ready: bool,
    /// Whether the identity declares itself able to take a cycle step, §14.
    pub cyclable: bool,
    /// Whether the identity declares itself upgradable, which is what slot 1 of
    /// the machine in §7 accepts.
    pub upgradable: bool,
    /// How far this stack has been upgraded, where it is upgradable at all.
    pub infusion: u32,
}

impl Stack {
    /// Builds a single stack of one identity, refusing an unusable name.
    ///
    /// # Errors
    ///
    /// Returns [`RecordRefused::Name`] for an empty or over-long identity.
    pub fn of(identity: &str) -> Result<Self, RecordRefused> {
        check_name(identity)?;
        Ok(Self {
            identity: identity.to_owned(),
            count: 1,
            ..Self::default()
        })
    }

    /// Returns whether this stack holds nothing.
    ///
    /// A zero count and an unnamed identity are the same absence, so a record
    /// that read a malformed entry as an empty slot cannot then present it as
    /// something a rule can match.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0 || self.identity.is_empty()
    }

    /// Returns whether two stacks are the same identity carrying the same
    /// components, which is the equality §8's component flag tests.
    #[must_use]
    pub fn same_with_components(&self, other: &Self) -> bool {
        self.identity == other.identity && self.components == other.components
    }

    /// Returns the namespace half of the identity, empty when it has none.
    #[must_use]
    pub fn namespace(&self) -> &str {
        match self.identity.split_once(':') {
            Some((namespace, _)) => namespace,
            None => "",
        }
    }

    /// Writes the stack as a document.
    #[must_use]
    pub fn to_saved(&self) -> JsonValue {
        let components: BTreeMap<&str, &str> = self
            .components
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        ravel_sdk::json!({
            "identity": self.identity,
            "count": self.count,
            "damage": self.damage,
            "components": components,
            "carrier_ready": self.carrier_ready,
            "cyclable": self.cyclable,
            "upgradable": self.upgradable,
            "infusion": self.infusion,
        })
    }

    /// Reads a stack back from a document.
    ///
    /// # Errors
    ///
    /// Returns [`RecordRefused`] for a document that is not an object, for an
    /// identity that is missing, empty or over-long, and for a component map
    /// above [`MAX_COMPONENTS`] or carrying an over-long name.
    pub fn from_saved(document: &JsonValue) -> Result<Self, RecordRefused> {
        let object = document
            .as_object()
            .ok_or(RecordRefused::Shape { record: None })?;
        let identity = object
            .get("identity")
            .and_then(JsonValue::as_str)
            .ok_or(RecordRefused::Shape { record: None })?;
        check_name(identity)?;

        let mut components = BTreeMap::new();
        if let Some(declared) = object.get("components") {
            let map = declared
                .as_object()
                .ok_or(RecordRefused::Shape { record: None })?;
            if map.len() > MAX_COMPONENTS {
                return Err(RecordRefused::TooMany {
                    limit: MAX_COMPONENTS,
                    found: map.len(),
                });
            }
            for (name, value) in map {
                check_name(name)?;
                let text = value
                    .as_str()
                    .ok_or(RecordRefused::Shape { record: None })?;
                check_name(text)?;
                components.insert(name.clone(), text.to_owned());
            }
        }

        let flag = |name: &str| {
            object
                .get(name)
                .and_then(JsonValue::as_bool)
                .unwrap_or_default()
        };
        Ok(Self {
            identity: identity.to_owned(),
            count: u8::try_from(
                object
                    .get("count")
                    .and_then(JsonValue::as_u64)
                    .unwrap_or(1)
                    .min(u64::from(u8::MAX)),
            )
            .unwrap_or(u8::MAX),
            damage: i32::try_from(
                object
                    .get("damage")
                    .and_then(JsonValue::as_i64)
                    .unwrap_or_default()
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX)),
            )
            .unwrap_or_default(),
            components,
            carrier_ready: flag("carrier_ready"),
            cyclable: flag("cyclable"),
            upgradable: flag("upgradable"),
            infusion: u32::try_from(
                object
                    .get("infusion")
                    .and_then(JsonValue::as_u64)
                    .unwrap_or_default()
                    .min(u64::from(u32::MAX)),
            )
            .unwrap_or(u32::MAX),
        })
    }
}

/// A position qualified by the dimension it is in, which is what §11 stores.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Placement {
    /// The dimension the position belongs to.
    pub dimension: String,
    /// The position's three coordinates.
    pub at: (i32, i32, i32),
}

impl Placement {
    /// Builds a placement, refusing an unusable dimension name.
    ///
    /// # Errors
    ///
    /// Returns [`RecordRefused::Name`] for an empty or over-long dimension.
    pub fn new(dimension: &str, at: (i32, i32, i32)) -> Result<Self, RecordRefused> {
        check_name(dimension)?;
        Ok(Self {
            dimension: dimension.to_owned(),
            at,
        })
    }

    /// Writes the placement as a document.
    #[must_use]
    pub fn to_saved(&self) -> JsonValue {
        ravel_sdk::json!({
            "dimension": self.dimension,
            "at": [self.at.0, self.at.1, self.at.2],
        })
    }

    /// Reads a placement back from a document.
    ///
    /// # Errors
    ///
    /// Returns [`RecordRefused`] for a document that is not an object carrying a
    /// legal dimension name and three whole coordinates.
    pub fn from_saved(document: &JsonValue) -> Result<Self, RecordRefused> {
        let object = document.as_object().ok_or(RecordRefused::Shape {
            record: Some(Record::SelectedPosition),
        })?;
        let dimension =
            object
                .get("dimension")
                .and_then(JsonValue::as_str)
                .ok_or(RecordRefused::Shape {
                    record: Some(Record::SelectedPosition),
                })?;
        let at = object
            .get("at")
            .and_then(JsonValue::as_array)
            .filter(|coordinates| coordinates.len() == 3)
            .ok_or(RecordRefused::Shape {
                record: Some(Record::SelectedPosition),
            })?;
        let mut read = [0_i32; 3];
        for (slot, value) in read.iter_mut().zip(at) {
            let whole = value.as_i64().ok_or(RecordRefused::Shape {
                record: Some(Record::SelectedPosition),
            })?;
            *slot = i32::try_from(whole).map_err(|_| RecordRefused::Shape {
                record: Some(Record::SelectedPosition),
            })?;
        }
        Self::new(dimension, read.into())
    }
}

/// Writes a list of stacks with the empty entries kept as absences.
///
/// This is the tolerant form the pattern, the carrier and the selected position
/// are written in: an empty slot is a null and reads back as an empty stack.
#[must_use]
pub fn write_tolerant(stacks: &[Option<Stack>]) -> JsonValue {
    JsonValue::Array(
        stacks
            .iter()
            .map(|slot| slot.as_ref().map_or(JsonValue::Null, Stack::to_saved))
            .collect(),
    )
}

/// Reads a list of stacks back, treating an absent entry as an empty slot.
///
/// # Errors
///
/// Returns [`RecordRefused::TooMany`] for a list longer than `limit`, which is
/// bounded by [`MAX_ENTRIES`] whatever the caller asks for, and whatever
/// [`Stack::from_saved`] refuses for a present entry.
pub fn read_tolerant(
    record: Record,
    document: &JsonValue,
    limit: usize,
) -> Result<Vec<Option<Stack>>, RecordRefused> {
    let limit = limit.min(MAX_ENTRIES);
    let entries = document.as_array().ok_or(RecordRefused::Shape {
        record: Some(record),
    })?;
    if entries.len() > limit {
        return Err(RecordRefused::TooMany {
            limit,
            found: entries.len(),
        });
    }
    let mut read = Vec::with_capacity(entries.len());
    for entry in entries {
        if entry.is_null() {
            read.push(None);
            continue;
        }
        read.push(Some(Stack::from_saved(entry)?));
    }
    Ok(read)
}

/// Reads a list of stacks back, refusing an absent entry outright.
///
/// This is the strict form §4 states for the match rule alone.
///
/// # Errors
///
/// Returns [`RecordRefused::AbsentEntry`] for a null anywhere in the list, plus
/// everything [`read_tolerant`] refuses.
pub fn read_strict(
    record: Record,
    document: &JsonValue,
    limit: usize,
) -> Result<Vec<Stack>, RecordRefused> {
    let read = read_tolerant(record, document, limit)?;
    read.into_iter()
        .enumerate()
        .map(|(index, slot)| slot.ok_or(RecordRefused::AbsentEntry { record, index }))
        .collect()
}

/// Refuses a name that is empty or longer than [`MAX_NAME`].
///
/// # Errors
///
/// Returns [`RecordRefused::Name`] carrying the length it found.
pub fn check_name(name: &str) -> Result<(), RecordRefused> {
    if name.is_empty() || name.len() > MAX_NAME {
        return Err(RecordRefused::Name { length: name.len() });
    }
    Ok(())
}

/// Why a persisted record was not read back.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordRefused {
    /// The document is not the shape the record is written in.
    Shape {
        /// Which record, where the reader knows.
        record: Option<Record>,
    },
    /// A list declared more entries than a record may hold.
    TooMany {
        /// The most it may hold.
        limit: usize,
        /// What the document declared.
        found: usize,
    },
    /// A record that does not tolerate one carried an absent entry.
    AbsentEntry {
        /// Which record refused.
        record: Record,
        /// Where the absence is.
        index: usize,
    },
    /// A name is empty or longer than [`MAX_NAME`].
    Name {
        /// How long the name was.
        length: usize,
    },
}

impl fmt::Display for RecordRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape { record: Some(name) } => {
                write!(
                    formatter,
                    "the {name} record is not the shape it is written in"
                )
            }
            Self::Shape { record: None } => {
                formatter.write_str("a stored stack is not the shape it is written in")
            }
            Self::TooMany { limit, found } => write!(
                formatter,
                "a record declares {found} entries and may hold {limit}"
            ),
            Self::AbsentEntry { record, index } => write!(
                formatter,
                "the {record} record carries an absent entry at {index} and tolerates none"
            ),
            Self::Name { length } => {
                write!(
                    formatter,
                    "a name of {length} characters is not a legal one"
                )
            }
        }
    }
}

impl std::error::Error for RecordRefused {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breaking_a_block_forgets_every_record_except_the_one_registered_twice() {
        let mut store = Store::new();
        for record in Record::ALL {
            store
                .write(record, "0,64,0", ravel_sdk::json!(1))
                .expect("a legal key");
        }

        assert_eq!(store.break_block("0,64,0"), 4);
        assert_eq!(
            store.read(Record::InfusionProgress, "0,64,0"),
            Some(&ravel_sdk::json!(1)),
            "\u{a7}4 registers this one twice so progress survives the block being replaced"
        );
        for record in Record::ALL {
            if record.survives_replacement() {
                continue;
            }
            assert!(store.read(record, "0,64,0").is_none());
        }
    }

    #[test]
    fn a_store_round_trips_through_the_document_it_is_saved_as() {
        let mut store = Store::new();
        store
            .write(Record::PatternContents, "held", ravel_sdk::json!([1, 2]))
            .expect("a legal key");
        store
            .write(Record::InfusionProgress, "0,64,0", ravel_sdk::json!(12))
            .expect("a legal key");

        let read = Store::from_saved(&store.to_saved()).expect("it reads back");
        assert_eq!(read, store);
    }

    #[test]
    fn a_saved_store_naming_a_record_this_build_does_not_know_is_refused() {
        let document = ravel_sdk::json!({ "sixth_record": { "held": 1 } });
        assert_eq!(
            Store::from_saved(&document),
            Err(RecordRefused::Shape { record: None }),
            "a world written by a build with another record is one this build does not read"
        );
    }

    #[test]
    fn a_store_is_bounded_by_key_and_by_count_before_anything_is_kept() {
        let mut store = Store::new();
        assert_eq!(
            store.write(Record::MatchRule, "", ravel_sdk::json!(1)),
            Err(RecordRefused::Name { length: 0 })
        );
        assert!(matches!(
            store.write(
                Record::MatchRule,
                &"k".repeat(MAX_NAME + 1),
                ravel_sdk::json!(1)
            ),
            Err(RecordRefused::Name { .. })
        ));

        for index in 0..MAX_STORED {
            store
                .write(Record::MatchRule, &format!("k{index}"), ravel_sdk::json!(1))
                .expect("a legal key");
        }
        assert_eq!(
            store.write(Record::MatchRule, "one_too_many", ravel_sdk::json!(1)),
            Err(RecordRefused::TooMany {
                limit: MAX_STORED,
                found: MAX_STORED + 1,
            })
        );
    }

    fn stack(identity: &str) -> Stack {
        Stack::of(identity).expect("a legal identity")
    }

    #[test]
    fn the_five_records_are_named_once_each_and_one_of_them_is_strict() {
        let mut names: Vec<&str> = Record::ALL.iter().map(|record| record.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 5);

        let strict: Vec<Record> = Record::ALL
            .into_iter()
            .filter(|record| !record.tolerates_absent())
            .collect();
        assert_eq!(strict, [Record::MatchRule]);
    }

    #[test]
    fn exactly_one_record_survives_the_block_being_broken_and_replaced() {
        let surviving: Vec<Record> = Record::ALL
            .into_iter()
            .filter(|record| record.survives_replacement())
            .collect();
        assert_eq!(surviving, [Record::InfusionProgress]);
    }

    #[test]
    fn a_stack_round_trips_through_its_document_with_every_declaration_kept() {
        let mut written = stack("counterpart:material");
        written.damage = 7;
        written.carrier_ready = true;
        written.upgradable = true;
        written.infusion = 3;
        written
            .components
            .insert("label".to_owned(), "first".to_owned());

        let read = Stack::from_saved(&written.to_saved()).expect("what was written reads back");
        assert_eq!(read, written);
    }

    #[test]
    fn a_tolerant_list_keeps_its_empty_entries_and_a_strict_one_refuses_them() {
        let slots = vec![Some(stack("counterpart:first")), None];
        let document = write_tolerant(&slots);

        let tolerant = read_tolerant(Record::PatternContents, &document, 8)
            .expect("the tolerant form keeps an absence");
        assert_eq!(tolerant, slots);

        assert_eq!(
            read_strict(Record::MatchRule, &document, 8),
            Err(RecordRefused::AbsentEntry {
                record: Record::MatchRule,
                index: 1,
            })
        );
    }

    #[test]
    fn a_hostile_list_is_refused_by_its_count_before_anything_is_read() {
        let entries: Vec<JsonValue> = (0..=MAX_ENTRIES)
            .map(|_| stack("counterpart:first").to_saved())
            .collect();
        let document = JsonValue::Array(entries);

        assert_eq!(
            read_tolerant(Record::CarrierContents, &document, usize::MAX),
            Err(RecordRefused::TooMany {
                limit: MAX_ENTRIES,
                found: MAX_ENTRIES + 1,
            })
        );
    }

    #[test]
    fn a_stack_with_an_over_long_identity_or_too_many_components_is_refused() {
        let long = "a".repeat(MAX_NAME + 1);
        assert_eq!(
            Stack::of(&long),
            Err(RecordRefused::Name {
                length: MAX_NAME + 1
            })
        );

        let mut crowded = stack("counterpart:first");
        for index in 0..=MAX_COMPONENTS {
            crowded
                .components
                .insert(format!("name{index}"), "value".to_owned());
        }
        assert!(matches!(
            Stack::from_saved(&crowded.to_saved()),
            Err(RecordRefused::TooMany { .. })
        ));
    }

    #[test]
    fn a_placement_round_trips_and_a_coordinate_outside_the_range_is_refused() {
        let written = Placement::new("counterpart:deep", (-15, 40, 12)).expect("a legal dimension");
        assert_eq!(
            Placement::from_saved(&written.to_saved()).expect("what was written reads back"),
            written
        );

        let hostile = ravel_sdk::json!({
            "dimension": "counterpart:deep",
            "at": [0, i64::from(i32::MAX) + 1, 0],
        });
        assert!(Placement::from_saved(&hostile).is_err());
    }

    #[test]
    fn a_namespace_is_the_half_before_the_separator_and_empty_without_one() {
        assert_eq!(stack("counterpart:first").namespace(), "counterpart");
        assert_eq!(stack("first").namespace(), "");
    }
}
