//! Persistence: the durable per-package directory of §6.1 and the two saved-data
//! envelopes of §6.2.
//!
//! # §6.1 is declined rather than host-blocked
//!
//! The original computes a cache directory and a data directory keyed to the
//! host operating system, creates both outside the game directory, and writes a
//! fixed explanatory file into each. That state survives world deletion, is
//! shared between every instance the user runs, and on a shared machine is
//! per-user rather than per-installation. It is deliberate and it is also
//! exactly what Ravel's sandbox should not let a package do on its own
//! initiative.
//!
//! So this package exposes "a durable per-package directory" and lets Ravel
//! decide where it is: [`DurableStorage`] joins a host-supplied root to a
//! validated package name and touches no file at all. It writes no explanatory
//! file, reads no environment variable and knows nothing about the operating
//! system. Creating and reading the subdirectory stays the dependent's job,
//! which is the one part of §6.1 the original also leaves alone.
//!
//! Any dependent that had recorded an absolute path would break. None is known
//! to, and none was read.
//!
//! # §6.2 and what Ravel owns of it
//!
//! Ravel hands a package a payload for its saved data rather than a keyed
//! compound, so both envelopes are portable logic over a payload and the write
//! itself belongs to Ravel's saved-data lifecycle. No save-compatible identifier
//! mapping is specified, so this package's alias declaration is empty and no
//! existing world is loadable by it.
//!
//! Two changes are declared:
//!
//! - **Clearing is the default.** The original merges the encoded compound into
//!   whatever the host handed it and clears existing keys only under an opt-in
//!   mode, so a record that loses a field keeps the stale key forever. That is a
//!   data-rot source with no upside.
//! - **A parse failure is loud.** The original logs and falls back to the
//!   default. The fallback is kept, because refusing would cost a world its
//!   state, but [`Loaded::recovery`] makes the failure something a caller has to
//!   look at rather than something in a log.
//!
//! One refusal moves earlier: a codec whose output is not a compound is logged
//! and silently dropped at write time by the original, and is refused at
//! declaration time here, because a codec's output shape is knowable before any
//! world exists.

use ravel_sdk::save::{SavedDataDeclaration, SavedDataError, SavedDataVersion};
use ravel_sdk::{JsonValue, ModId, Side, json};

use crate::data::Tag;

/// The most characters a package name may occupy in a path segment.
pub const MAX_NAME_LENGTH: usize = 64;

/// Why storage refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageError {
    /// A package name is empty.
    EmptyName,
    /// A package name is longer than [`MAX_NAME_LENGTH`].
    NameTooLong(usize),
    /// A package name carries something that is not a name.
    ///
    /// A separator, a parent reference or anything outside lowercase letters,
    /// digits, `_` and `-`. This is what keeps a name a single segment under the
    /// root Ravel chose rather than a path that reaches out of it.
    NotASegment(String),
    /// A record's default is not a compound, which is knowable before any world
    /// exists.
    DefaultNotCompound,
    /// A value written to a record is not a compound.
    ValueNotCompound,
    /// A record name is empty or longer than [`MAX_NAME_LENGTH`].
    BadRecordName(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyName => write!(formatter, "a package name is empty"),
            Self::NameTooLong(len) => write!(formatter, "a package name is {len} characters"),
            Self::NotASegment(name) => {
                write!(formatter, "`{name}` is a path rather than a package name")
            }
            Self::DefaultNotCompound => {
                write!(formatter, "a record's default is not a compound")
            }
            Self::ValueNotCompound => write!(formatter, "a record's value is not a compound"),
            Self::BadRecordName(name) => write!(formatter, "`{name}` is not a record name"),
        }
    }
}

impl std::error::Error for StorageError {}

/// A durable place a package may keep things, wherever Ravel put it.
///
/// This is the whole of §6.1 that survives. The root arrives from the host; this
/// type decides only that a package's own subdirectory is one segment named
/// after the package, and never that the root is under a user's home.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableStorage {
    root: String,
}

impl DurableStorage {
    /// Names a durable root the host supplied.
    #[must_use]
    pub fn under(root: &str) -> Self {
        Self {
            root: root.trim_end_matches('/').to_owned(),
        }
    }

    /// Returns the root, as the host named it.
    #[must_use]
    pub fn root(&self) -> &str {
        &self.root
    }

    /// Returns a package's own subdirectory under the root.
    ///
    /// Nothing here creates it, reads it or checks whether it exists. That is
    /// the dependent's job in the original too.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] for an empty name, one longer than
    /// [`MAX_NAME_LENGTH`], or one that is a path rather than a name.
    pub fn for_package(&self, package: &str) -> Result<String, StorageError> {
        check_segment(package)?;
        Ok(format!("{}/{package}", self.root))
    }
}

fn check_segment(name: &str) -> Result<(), StorageError> {
    if name.is_empty() {
        return Err(StorageError::EmptyName);
    }
    if name.chars().count() > MAX_NAME_LENGTH {
        return Err(StorageError::NameTooLong(name.chars().count()));
    }
    if !name.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
    }) {
        return Err(StorageError::NotASegment(name.to_owned()));
    }
    Ok(())
}

/// Which dimension a record is stored against.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum Scope {
    /// The dimension the request came from.
    #[default]
    Current,
    /// The primary dimension, giving one world-wide record.
    Primary,
}

/// A codec-described world save.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    name: String,
    fallback: JsonValue,
    clearing: bool,
    always_dirty: bool,
    scope: Scope,
}

impl Record {
    /// Declares a record under a storage name, with the value used when nothing
    /// is stored.
    ///
    /// Clearing is on by default, which is the declared change.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::BadRecordName`] for an empty or over-long name
    /// and [`StorageError::DefaultNotCompound`] for a default that is not a
    /// compound. The second is the refusal moved to declaration time.
    pub fn declare(name: &str, fallback: JsonValue) -> Result<Self, StorageError> {
        if name.is_empty() || name.chars().count() > MAX_NAME_LENGTH {
            return Err(StorageError::BadRecordName(name.to_owned()));
        }
        if !fallback.is_object() {
            return Err(StorageError::DefaultNotCompound);
        }
        Ok(Self {
            name: name.to_owned(),
            fallback,
            clearing: true,
            always_dirty: false,
            scope: Scope::Current,
        })
    }

    /// Returns the storage name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the value used when nothing is stored.
    #[must_use]
    pub const fn fallback(&self) -> &JsonValue {
        &self.fallback
    }

    /// Turns clearing off, keeping the original's merging write.
    ///
    /// It is here because a dependent that genuinely wants a merge should be
    /// able to say so, and because saying so is what makes the default a choice
    /// rather than a silent difference.
    #[must_use]
    pub fn merging(mut self) -> Self {
        self.clearing = false;
        self
    }

    /// Marks the record permanently dirty, so it is written on every save.
    #[must_use]
    pub const fn always_dirty(mut self) -> Self {
        self.always_dirty = true;
        self
    }

    /// Stores the record against the primary dimension.
    #[must_use]
    pub const fn world_wide(mut self) -> Self {
        self.scope = Scope::Primary;
        self
    }

    /// Returns which dimension this is stored against.
    #[must_use]
    pub const fn scope(&self) -> Scope {
        self.scope
    }

    /// Returns whether a save writes this record.
    #[must_use]
    pub const fn writes(&self, changed: bool) -> bool {
        self.always_dirty || changed
    }

    /// Reads what the world stored.
    ///
    /// Absence and a value that is not a compound both fall back to the
    /// default; the second is a recovery and says so.
    #[must_use]
    pub fn read(&self, stored: Option<&JsonValue>) -> Loaded {
        match stored {
            None => Loaded {
                value: self.fallback.clone(),
                recovery: None,
                fresh: true,
            },
            Some(value) if value.is_object() => Loaded {
                value: value.clone(),
                recovery: None,
                fresh: false,
            },
            Some(_) => Loaded {
                value: self.fallback.clone(),
                recovery: Some(Recovery::NotCompound),
                fresh: false,
            },
        }
    }

    /// Produces what the host should hold after a write.
    ///
    /// Clearing replaces the host's compound outright, so a record that lost a
    /// field does not leave a stale key behind. Merging keeps the original's
    /// rule, for a dependent that asked for it.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::ValueNotCompound`] when either side is not a
    /// compound.
    ///
    /// # Panics
    ///
    /// Panics if a freshly constructed JSON object is not an object.
    pub fn write(&self, held: &JsonValue, encoded: &JsonValue) -> Result<JsonValue, StorageError> {
        let encoded_entries = encoded.as_object().ok_or(StorageError::ValueNotCompound)?;
        if self.clearing {
            return Ok(encoded.clone());
        }
        let held_entries = held.as_object().ok_or(StorageError::ValueNotCompound)?;
        let mut merged = json!({});
        let target = merged
            .as_object_mut()
            .expect("the value was just built as an object");
        for (name, value) in held_entries {
            target.insert(name.clone(), value.clone());
        }
        for (name, value) in encoded_entries {
            target.insert(name.clone(), value.clone());
        }
        Ok(merged)
    }

    /// Returns whether this record clears before writing.
    #[must_use]
    pub const fn is_clearing(&self) -> bool {
        self.clearing
    }

    /// Returns the saved-data declaration Ravel resolves this against.
    ///
    /// One version, because no world written by an earlier build of this package
    /// exists and inventing a migration for one would be a chain nothing
    /// crosses.
    ///
    /// # Errors
    ///
    /// Returns [`SavedDataError`] when the version set is empty, which the
    /// constant below makes impossible.
    pub fn declaration(owner: ModId) -> Result<SavedDataDeclaration, SavedDataError> {
        let version = SavedDataVersion::new(VERSION).ok_or(SavedDataError::NoVersionsDeclared {
            owner: owner.clone(),
        })?;
        SavedDataDeclaration::new(owner, [version])
    }
}

/// The one saved-data version this package writes.
pub const VERSION: u32 = 1;

/// Why a read fell back to the default.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Recovery {
    /// What the world held is not a compound.
    NotCompound,
}

impl std::fmt::Display for Recovery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotCompound => write!(
                formatter,
                "the stored record is not a compound and the default was used instead"
            ),
        }
    }
}

/// What a read produced.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    value: JsonValue,
    recovery: Option<Recovery>,
    fresh: bool,
}

impl Loaded {
    /// Returns the value in force.
    #[must_use]
    pub const fn value(&self) -> &JsonValue {
        &self.value
    }

    /// Returns why the default was used, when it was used after a failure.
    ///
    /// A caller that ignores this loses a world's state quietly, which is what
    /// the original does. A caller that reads it reports the failure.
    #[must_use]
    pub const fn recovery(&self) -> Option<Recovery> {
        self.recovery
    }

    /// Returns whether the world held nothing at all, which is not a failure.
    #[must_use]
    pub const fn is_fresh(&self) -> bool {
        self.fresh
    }
}

/// The older envelope: a dependent reads and writes the tag tree itself.
///
/// It optionally carries a client-side stand-in, returned when the request comes
/// from a world that is not a server world.
///
/// **Declared change.** The original's stand-in is a single shared instance
/// rather than a copy, so two client worlds share one mutable record and each
/// sees the other's writes. This hands back a copy per request.
#[derive(Clone, Debug, PartialEq)]
pub struct RawEnvelope {
    stand_in: Option<Tag>,
}

impl RawEnvelope {
    /// An envelope with no client-side stand-in.
    #[must_use]
    pub const fn new() -> Self {
        Self { stand_in: None }
    }

    /// An envelope with a client-side stand-in.
    #[must_use]
    pub const fn with_stand_in(stand_in: Tag) -> Self {
        Self {
            stand_in: Some(stand_in),
        }
    }

    /// Returns what a request from `side` should read.
    ///
    /// A server side reads the world's own record. Any other side reads a copy
    /// of the stand-in, or nothing when none was supplied.
    #[must_use]
    pub fn read(&self, side: Side, stored: Option<&Tag>) -> Option<Tag> {
        if matches!(side, Side::DedicatedServer | Side::IntegratedServer) {
            return stored.cloned();
        }
        self.stand_in.clone()
    }

    /// Returns whether a write from `side` reaches the world.
    ///
    /// A stand-in is not the world's record, so writing through one changes
    /// nothing that is saved.
    #[must_use]
    pub const fn writes(side: Side) -> bool {
        matches!(side, Side::DedicatedServer | Side::IntegratedServer)
    }
}

impl Default for RawEnvelope {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::{ModId, Side, json};

    use super::{
        DurableStorage, MAX_NAME_LENGTH, RawEnvelope, Record, Recovery, Scope, StorageError,
    };
    use crate::data::Tag;

    #[test]
    fn a_durable_directory_is_the_hosts_root_and_the_packages_own_name() {
        let storage = DurableStorage::under("/wherever/ravel/put/it");
        assert_eq!(
            storage.for_package("example"),
            Ok("/wherever/ravel/put/it/example".to_owned())
        );
        assert_eq!(storage.root(), "/wherever/ravel/put/it");
    }

    #[test]
    fn a_trailing_separator_on_the_root_does_not_double() {
        let storage = DurableStorage::under("/root/");
        assert_eq!(storage.for_package("a"), Ok("/root/a".to_owned()));
    }

    #[test]
    fn a_package_name_that_is_a_path_is_refused_rather_than_joined() {
        let storage = DurableStorage::under("/root");
        for hostile in ["..", "a/b", "/etc", "a\\b", "A", "a b", "a.b"] {
            assert_eq!(
                storage.for_package(hostile),
                Err(StorageError::NotASegment(hostile.to_owned())),
                "`{hostile}` is not one segment"
            );
        }
        assert_eq!(storage.for_package(""), Err(StorageError::EmptyName));
        let long = "a".repeat(MAX_NAME_LENGTH + 1);
        assert_eq!(
            storage.for_package(&long),
            Err(StorageError::NameTooLong(MAX_NAME_LENGTH + 1))
        );
    }

    #[test]
    fn a_record_refuses_a_default_that_is_not_a_compound_before_any_world_exists() {
        assert_eq!(
            Record::declare("counts", json!(4)),
            Err(StorageError::DefaultNotCompound)
        );
        assert_eq!(
            Record::declare("counts", json!([])),
            Err(StorageError::DefaultNotCompound)
        );
        assert!(Record::declare("counts", json!({})).is_ok());
    }

    #[test]
    fn a_record_name_is_bounded_at_both_ends() {
        assert_eq!(
            Record::declare("", json!({})),
            Err(StorageError::BadRecordName(String::new()))
        );
        let long = "n".repeat(MAX_NAME_LENGTH + 1);
        assert_eq!(
            Record::declare(&long, json!({})),
            Err(StorageError::BadRecordName(long))
        );
    }

    #[test]
    fn nothing_stored_reads_the_default_and_is_not_a_failure() {
        let record = Record::declare("counts", json!({ "seen": 0 })).expect("a compound default");
        let loaded = record.read(None);
        assert_eq!(loaded.value(), &json!({ "seen": 0 }));
        assert!(loaded.is_fresh());
        assert_eq!(loaded.recovery(), None);
    }

    #[test]
    fn a_parse_failure_falls_back_to_the_default_and_says_so_rather_than_only_logging() {
        let record = Record::declare("counts", json!({ "seen": 0 })).expect("a compound default");
        let loaded = record.read(Some(&json!("not a record")));
        assert_eq!(
            loaded.value(),
            &json!({ "seen": 0 }),
            "the fallback is kept: refusing would cost a world its state"
        );
        assert_eq!(loaded.recovery(), Some(Recovery::NotCompound));
        assert!(!loaded.is_fresh());
        assert_ne!(Recovery::NotCompound.to_string(), "");
    }

    #[test]
    fn a_stored_record_reads_back_whole() {
        let record = Record::declare("counts", json!({ "seen": 0 })).expect("a compound default");
        let loaded = record.read(Some(&json!({ "seen": 12, "extra": true })));
        assert_eq!(loaded.value(), &json!({ "seen": 12, "extra": true }));
        assert_eq!(loaded.recovery(), None);
    }

    #[test]
    fn a_shrinking_record_leaves_no_stale_key_because_clearing_is_the_default() {
        let record = Record::declare("counts", json!({})).expect("a compound default");
        assert!(record.is_clearing());
        let held = json!({ "seen": 1, "removed": true });
        let written = record
            .write(&held, &json!({ "seen": 2 }))
            .expect("both sides are compounds");
        assert_eq!(written, json!({ "seen": 2 }));
    }

    #[test]
    fn a_merging_record_keeps_the_originals_rule_for_a_dependent_that_asked_for_it() {
        let record = Record::declare("counts", json!({}))
            .expect("a compound default")
            .merging();
        assert!(!record.is_clearing());
        let held = json!({ "seen": 1, "removed": true });
        let written = record
            .write(&held, &json!({ "seen": 2 }))
            .expect("both sides are compounds");
        assert_eq!(
            written,
            json!({ "seen": 2, "removed": true }),
            "the removed field persists forever, which is the defect this is not the default for"
        );
    }

    #[test]
    fn a_write_refuses_anything_that_is_not_a_compound() {
        let record = Record::declare("counts", json!({}))
            .expect("a compound default")
            .merging();
        assert_eq!(
            record.write(&json!({}), &json!(4)),
            Err(StorageError::ValueNotCompound)
        );
        assert_eq!(
            record.write(&json!(4), &json!({})),
            Err(StorageError::ValueNotCompound)
        );
    }

    #[test]
    fn an_always_dirty_record_is_written_whether_or_not_anything_changed() {
        let plain = Record::declare("counts", json!({})).expect("a compound default");
        assert!(!plain.writes(false));
        assert!(plain.writes(true));
        let always = plain.clone().always_dirty();
        assert!(always.writes(false));
        assert!(always.writes(true));
    }

    #[test]
    fn a_world_wide_record_is_stored_against_the_primary_dimension() {
        let record = Record::declare("counts", json!({})).expect("a compound default");
        assert_eq!(record.scope(), Scope::Current);
        assert_eq!(record.world_wide().scope(), Scope::Primary);
    }

    #[test]
    fn the_saved_data_declaration_names_one_version() {
        let owner: ModId = "example".parse().expect("a legal mod id");
        let declaration = Record::declaration(owner.clone()).expect("one version is not none");
        assert_eq!(declaration.owner(), &owner);
        assert_eq!(declaration.versions().count(), 1);
        assert_eq!(declaration.current().get(), super::VERSION);
    }

    #[test]
    fn the_older_envelope_hands_a_server_side_the_worlds_own_record() {
        let envelope = RawEnvelope::new();
        let stored = Tag::compound([("seen", Tag::Int(3))]);
        for side in [Side::DedicatedServer, Side::IntegratedServer] {
            assert_eq!(envelope.read(side, Some(&stored)), Some(stored.clone()));
            assert!(RawEnvelope::writes(side));
        }
    }

    #[test]
    fn a_client_world_reads_the_stand_in_rather_than_the_worlds_record() {
        let stand_in = Tag::compound([("seen", Tag::Int(0))]);
        let envelope = RawEnvelope::with_stand_in(stand_in.clone());
        let stored = Tag::compound([("seen", Tag::Int(9))]);
        assert_eq!(envelope.read(Side::Client, Some(&stored)), Some(stand_in));
        assert!(!RawEnvelope::writes(Side::Client));
    }

    #[test]
    fn two_client_worlds_get_two_copies_rather_than_one_shared_instance() {
        let envelope = RawEnvelope::with_stand_in(Tag::compound([("seen", Tag::Int(0))]));
        let Some(Tag::Compound(mut first)) = envelope.read(Side::Client, None) else {
            panic!("a stand-in was supplied");
        };
        first.insert("seen".to_owned(), Tag::Int(99));
        let second = envelope.read(Side::Client, None);
        assert_eq!(
            second,
            Some(Tag::compound([("seen", Tag::Int(0))])),
            "this is the declared change: the original shares one instance between client worlds"
        );
    }

    #[test]
    fn an_envelope_with_no_stand_in_hands_a_client_world_nothing() {
        let envelope = RawEnvelope::default();
        assert_eq!(envelope.read(Side::Client, Some(&Tag::Int(1))), None);
    }
}
