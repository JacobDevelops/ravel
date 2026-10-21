//! The saved recipe memory of §6.1, which in the original does not survive a
//! save and a load.
//!
//! The state machine of [`crate::recipes`] writes a nested document under a
//! fixed key holding up to two entries: the current recipe's identifier and the
//! remembered recipe's, each present only when set. This module writes that and
//! reads it back.
//!
//! **The three spellings are not invented here.** §14 makes the saved key and its
//! two entry keys compatibility-required, because they appear in an existing
//! world, and withholds them. [`Spellings::declared`] reads them from build-time
//! inputs and [`Spellings::placeholder`] carries this package's own, which agrees
//! with no existing world.
//!
//! # The changes declared here
//!
//! All three are §13's, and together they are the difference between a memory
//! that survives a restart and one that does not.
//!
//! - **The value is read from the document it was written into.** The original
//!   tests presence on the nested document and then reads the value from the
//!   *outer* one, which has no such entry, so both reads yield an empty string.
//!   An empty string is a well-formed identifier with an empty path, so parsing
//!   succeeds, the registry lookup misses, and each field is assigned absent with
//!   nothing raised. Every saved recipe memory is silently lost.
//! - **No level is not no saved memory.** The original's load returns before
//!   reading anything when its level supplier yields nothing.
//!   [`Memory::read`] does not consult a level at all: an identifier is text, and
//!   resolving it against a registry is [`Resolution`]'s caller's business.
//! - **The failure is narrow.** The original catches every throwable around the
//!   read, so a genuine error inside the recipe lookup is swallowed as though it
//!   were a parse failure. [`ReadFailure`] has one arm and it is about the shape
//!   of the document.
//!
//! An identifier the registry no longer holds is still absent rather than an
//! error, which is the original's behaviour and the right one: a world that lost
//! a mod should open.

use std::fmt;

use ravel_sdk::content::ContentId;
use ravel_sdk::save::{SavedDataDeclaration, SavedDataEnvelope, SavedDataError, SavedDataVersion};
use ravel_sdk::{JsonValue, ModId, json};

/// Why the saved spellings could not be built.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SavedSpellingsUnavailable {
    /// The build supplied nothing.
    NotSupplied,
    /// A spelling is empty.
    Empty,
    /// Two spellings are the same word, which would make one entry overwrite the
    /// other.
    NotDistinct,
}

impl fmt::Display for SavedSpellingsUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSupplied => formatter.write_str("the build supplied no saved spellings"),
            Self::Empty => formatter.write_str("a saved spelling is empty"),
            Self::NotDistinct => formatter.write_str("two saved spellings are one word"),
        }
    }
}

impl std::error::Error for SavedSpellingsUnavailable {}

/// The three keys an existing world holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Spellings {
    document: String,
    current: String,
    remembered: String,
}

impl Default for Spellings {
    fn default() -> Self {
        Self::placeholder()
    }
}

impl Spellings {
    /// The spellings the tests pin the document's **shape** with.
    #[must_use]
    pub fn placeholder() -> Self {
        Self {
            document: "search".to_owned(),
            current: "working".to_owned(),
            remembered: "recalled".to_owned(),
        }
    }

    /// Reads the three spellings from build-time inputs.
    ///
    /// # Errors
    ///
    /// Returns [`SavedSpellingsUnavailable::NotSupplied`] when the build supplied
    /// nothing, and what [`Spellings::parse`] returns otherwise.
    pub fn declared() -> Result<Self, SavedSpellingsUnavailable> {
        let document = option_env!("RVL_146_COUNTERPART_MEMORY_KEY")
            .ok_or(SavedSpellingsUnavailable::NotSupplied)?;
        let current = option_env!("RVL_146_COUNTERPART_CURRENT_KEY")
            .ok_or(SavedSpellingsUnavailable::NotSupplied)?;
        let remembered = option_env!("RVL_146_COUNTERPART_REMEMBERED_KEY")
            .ok_or(SavedSpellingsUnavailable::NotSupplied)?;
        Self::parse(document, current, remembered)
    }

    /// Reads three spellings.
    ///
    /// # Errors
    ///
    /// Returns [`SavedSpellingsUnavailable::Empty`] for an empty spelling and
    /// [`SavedSpellingsUnavailable::NotDistinct`] when two of the three are the same
    /// word, which would make one entry overwrite the other.
    pub fn parse(
        document: &str,
        current: &str,
        remembered: &str,
    ) -> Result<Self, SavedSpellingsUnavailable> {
        if document.is_empty() || current.is_empty() || remembered.is_empty() {
            return Err(SavedSpellingsUnavailable::Empty);
        }
        if current == remembered {
            return Err(SavedSpellingsUnavailable::NotDistinct);
        }
        Ok(Self {
            document: document.to_owned(),
            current: current.to_owned(),
            remembered: remembered.to_owned(),
        })
    }

    /// Returns the key the nested document sits under.
    #[must_use]
    pub fn document(&self) -> &str {
        &self.document
    }

    /// Returns the key the current recipe sits under.
    #[must_use]
    pub fn current(&self) -> &str {
        &self.current
    }

    /// Returns the key the remembered recipe sits under.
    #[must_use]
    pub fn remembered(&self) -> &str {
        &self.remembered
    }
}

/// Why a saved document could not be read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadFailure {
    /// An entry is present and is not text. Nothing else is an error here: a
    /// missing entry is a memory that was never set, and text naming nothing the
    /// registry holds is a memory whose recipe is gone.
    NotText {
        /// Which entry.
        key: String,
    },
    /// A memory the world holds and this build could not: it sits past the
    /// ceiling on how many memories are held at once. The entry is still in the
    /// world, and this refusal is what stops a save from writing it out of one.
    Dropped {
        /// Whose memory was not held.
        owner: String,
    },
}

impl fmt::Display for ReadFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotText { key } => write!(formatter, "`{key}` is present and is not text"),
            Self::Dropped { owner } => {
                write!(
                    formatter,
                    "`{owner}`'s memory is past the ceiling on memories"
                )
            }
        }
    }
}

impl std::error::Error for ReadFailure {}

/// What one saved identifier turned out to be.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Resolution {
    /// The entry was not written.
    Unset,
    /// The entry named a recipe.
    Named(ContentId),
    /// The entry is text that names no legal identifier, or names one the
    /// registry no longer holds. Absent rather than an error: a world that lost
    /// a mod opens.
    Unresolvable(String),
}

impl Resolution {
    /// Returns the recipe, and absent for either of the other two.
    #[must_use]
    pub const fn recipe(&self) -> Option<&ContentId> {
        match self {
            Self::Named(id) => Some(id),
            Self::Unset | Self::Unresolvable(_) => None,
        }
    }
}

/// The two identifiers a world holds for one processing block.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Memory {
    /// The recipe being worked when the world was saved.
    pub current: Option<ContentId>,
    /// The last recipe that matched.
    pub remembered: Option<ContentId>,
}

impl Memory {
    /// The version this build writes.
    ///
    /// # Panics
    ///
    /// Never: one is not zero.
    #[must_use]
    pub fn version() -> SavedDataVersion {
        SavedDataVersion::new(1).expect("one is not zero")
    }

    /// Declares the versions this build can read.
    ///
    /// # Errors
    ///
    /// Returns [`SavedDataError`] only for a declaration naming no version,
    /// which this one does not.
    pub fn declaration(owner: ModId) -> Result<SavedDataDeclaration, SavedDataError> {
        SavedDataDeclaration::new(owner, [Self::version()])
    }

    /// Writes the nested document, holding only the entries that are set.
    #[must_use]
    pub fn write(&self, spellings: &Spellings) -> JsonValue {
        let mut nested = json!({});
        if let Some(current) = &self.current {
            nested[spellings.current()] = json!(current.to_string());
        }
        if let Some(remembered) = &self.remembered {
            nested[spellings.remembered()] = json!(remembered.to_string());
        }
        let mut document = json!({});
        document[spellings.document()] = nested;
        document
    }

    /// Reads the nested document this package wrote.
    ///
    /// # Errors
    ///
    /// Returns [`ReadFailure::NotText`] for an entry that is present and is not
    /// text. A document with no nested part, and a nested part with neither
    /// entry, are both an unset memory rather than a failure.
    pub fn read(
        document: &JsonValue,
        spellings: &Spellings,
        holds: &dyn Fn(&ContentId) -> bool,
    ) -> Result<(Resolution, Resolution), ReadFailure> {
        let Some(nested) = document.get(spellings.document()) else {
            return Ok((Resolution::Unset, Resolution::Unset));
        };
        Ok((
            resolve(nested, spellings.current(), holds)?,
            resolve(nested, spellings.remembered(), holds)?,
        ))
    }

    /// Builds the envelope a mod answers with.
    #[must_use]
    pub fn envelope(&self, owner: ModId, spellings: &Spellings) -> SavedDataEnvelope {
        SavedDataEnvelope::current(owner, Self::version(), self.write(spellings))
    }
}

fn resolve(
    nested: &JsonValue,
    key: &str,
    holds: &dyn Fn(&ContentId) -> bool,
) -> Result<Resolution, ReadFailure> {
    let Some(value) = nested.get(key) else {
        return Ok(Resolution::Unset);
    };
    let Some(spelling) = value.as_str() else {
        return Err(ReadFailure::NotText {
            key: key.to_owned(),
        });
    };
    Ok(match spelling.parse::<ContentId>() {
        Ok(id) if holds(&id) => Resolution::Named(id),
        Ok(_) | Err(_) => Resolution::Unresolvable(spelling.to_owned()),
    })
}

#[cfg(test)]
mod tests {
    use ravel_sdk::content::ContentId;
    use ravel_sdk::save::SavedDataEnvelope;
    use ravel_sdk::{JsonValue, json};

    use super::{Memory, ReadFailure, Resolution, SavedSpellingsUnavailable, Spellings};

    fn owner() -> ravel_sdk::ModId {
        "rvl_146_counterpart"
            .parse()
            .expect("the declared identity is a legal mod id")
    }

    fn recipe(path: &str) -> ContentId {
        ContentId::new(owner(), path).expect("a legal content path")
    }

    fn everything(_: &ContentId) -> bool {
        true
    }

    fn nothing(_: &ContentId) -> bool {
        false
    }

    #[test]
    fn what_was_written_is_what_is_read_back() {
        let spellings = Spellings::placeholder();
        let memory = Memory {
            current: Some(recipe("smelting")),
            remembered: Some(recipe("blasting")),
        };
        let document = memory.write(&spellings);
        let (current, remembered) =
            Memory::read(&document, &spellings, &everything).expect("a document this wrote");
        assert_eq!(current, Resolution::Named(recipe("smelting")));
        assert_eq!(remembered, Resolution::Named(recipe("blasting")));
        assert_eq!(current.recipe(), Some(&recipe("smelting")));
    }

    #[test]
    fn an_entry_that_was_not_set_is_absent_from_the_document_and_unset_on_the_way_back() {
        let spellings = Spellings::placeholder();
        let memory = Memory {
            current: None,
            remembered: Some(recipe("blasting")),
        };
        let document = memory.write(&spellings);
        assert!(
            document[spellings.document()]
                .get(spellings.current())
                .is_none(),
            "an unset entry is not written at all"
        );
        let (current, remembered) =
            Memory::read(&document, &spellings, &everything).expect("a document this wrote");
        assert_eq!(current, Resolution::Unset);
        assert_eq!(remembered, Resolution::Named(recipe("blasting")));
    }

    #[test]
    fn a_document_with_no_nested_part_is_an_unset_memory_rather_than_a_failure() {
        let spellings = Spellings::placeholder();
        let (current, remembered) = Memory::read(&json!({}), &spellings, &everything)
            .expect("a world that saved nothing opens");
        assert_eq!(current, Resolution::Unset);
        assert_eq!(remembered, Resolution::Unset);
    }

    /// A document holding one entry under the nested key, built the way a peer
    /// or an older world would rather than through [`Memory::write`].
    fn nested(spellings: &Spellings, key: &str, value: JsonValue) -> JsonValue {
        let mut inner = json!({});
        inner[key] = value;
        let mut document = json!({});
        document[spellings.document()] = inner;
        document
    }

    #[test]
    fn text_naming_no_legal_identifier_is_absent_rather_than_an_error() {
        let spellings = Spellings::placeholder();
        let document = nested(&spellings, spellings.current(), json!(""));
        let (current, _) =
            Memory::read(&document, &spellings, &everything).expect("an empty spelling opens");
        assert_eq!(current, Resolution::Unresolvable(String::new()));
        assert_eq!(current.recipe(), None);
    }

    #[test]
    fn text_naming_a_recipe_the_registry_lost_is_absent_rather_than_an_error() {
        let spellings = Spellings::placeholder();
        let memory = Memory {
            current: Some(recipe("smelting")),
            remembered: None,
        };
        let document = memory.write(&spellings);
        let (current, _) = Memory::read(&document, &spellings, &nothing)
            .expect("a world whose recipe is gone opens");
        assert_eq!(
            current,
            Resolution::Unresolvable("rvl_146_counterpart:smelting".to_owned())
        );
    }

    #[test]
    fn an_entry_that_is_present_and_is_not_text_is_the_one_reported_failure() {
        let spellings = Spellings::placeholder();
        let document = nested(&spellings, spellings.current(), json!(7));
        assert_eq!(
            Memory::read(&document, &spellings, &everything),
            Err(ReadFailure::NotText {
                key: spellings.current().to_owned()
            })
        );
    }

    #[test]
    fn a_value_written_under_the_outer_document_is_not_read_as_the_nested_one() {
        // The original's defect, written as a document: the outer object carries
        // the entry and the nested one does not.
        let spellings = Spellings::placeholder();
        let mut document = json!({});
        document[spellings.document()] = json!({});
        document[spellings.current()] = json!("rvl_146_counterpart:smelting");
        let (current, _) = Memory::read(&document, &spellings, &everything).expect("a document");
        assert_eq!(
            current,
            Resolution::Unset,
            "this reads the document it writes, and nothing else"
        );
    }

    #[test]
    fn the_envelope_carries_the_document_and_the_version_this_build_writes() {
        let spellings = Spellings::placeholder();
        let memory = Memory {
            current: Some(recipe("smelting")),
            remembered: None,
        };
        let mut expected = json!({});
        expected[spellings.document()] = nested(
            &spellings,
            spellings.current(),
            json!("rvl_146_counterpart:smelting"),
        )[spellings.document()]
        .clone();
        assert_eq!(
            memory.envelope(owner(), &spellings),
            SavedDataEnvelope::current(owner(), Memory::version(), expected)
        );
        let declaration = Memory::declaration(owner()).expect("one version is a version");
        assert_eq!(declaration.current(), Memory::version());
    }

    #[test]
    fn three_ways_a_supplied_set_of_spellings_is_refused_and_the_declared_one_is_absent() {
        assert_eq!(
            Spellings::parse("", "a", "b"),
            Err(SavedSpellingsUnavailable::Empty)
        );
        assert_eq!(
            Spellings::parse("d", "a", ""),
            Err(SavedSpellingsUnavailable::Empty)
        );
        assert_eq!(
            Spellings::parse("d", "a", "a"),
            Err(SavedSpellingsUnavailable::NotDistinct)
        );
        assert_eq!(
            Spellings::declared(),
            Err(SavedSpellingsUnavailable::NotSupplied),
            "no build here supplies an existing world's keys"
        );
    }

    #[test]
    fn supplied_spellings_change_the_document_a_world_holds() {
        let ours = Spellings::placeholder();
        let theirs = Spellings::parse("memory", "now", "last").expect("three distinct words");
        let memory = Memory {
            current: Some(recipe("smelting")),
            remembered: None,
        };
        assert_ne!(memory.write(&ours), memory.write(&theirs));
        assert_eq!(
            Memory::read(&memory.write(&theirs), &theirs, &everything)
                .expect("a document this wrote")
                .0,
            Resolution::Named(recipe("smelting"))
        );
    }
}
