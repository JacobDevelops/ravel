//! The global saved-data helper of §10.
//!
//! Data that belongs to a save as a whole rather than to one dimension. It is
//! always stored against the overworld's storage whichever dimension the caller
//! is in, keyed by a caller-supplied name, and created from a supplied factory
//! when absent.
//!
//! It **refuses any client-side access** rather than answering empty, and that
//! refusal is kept deliberately: the data has no client-side meaning and a
//! silent empty would look to a player exactly like a wiped save. In Ravel this
//! is [`ravel_sdk::save`] ownership with a server-only accessor, and the
//! overworld pinning is one saved-data scope rather than a per-dimension one.

use std::collections::BTreeMap;
use std::fmt;

use ravel_sdk::save::{SavedDataDeclaration, SavedDataEnvelope, SavedDataError, SavedDataVersion};
use ravel_sdk::{Declared, JsonValue, Side, json};

/// The one scope every global record is stored against.
pub const SCOPE: &str = "overworld";

/// The most records one save may hold.
pub const MAX_RECORDS: usize = 4_096;

/// The most characters a record's name may carry.
pub const MAX_NAME: usize = 128;

/// The version this build writes.
pub const VERSION: u32 = 1;

/// The records a save holds, keyed by the names their owners chose.
#[derive(Clone, Debug, Default)]
pub struct GlobalData {
    held: BTreeMap<String, JsonValue>,
}

impl GlobalData {
    /// Builds a store holding nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            held: BTreeMap::new(),
        }
    }

    /// Returns the record `name` holds, creating it from `factory` when absent.
    ///
    /// # Errors
    ///
    /// Returns [`GlobalRefused::NotOnAClient`] on a client side, which is the
    /// refusal §10 asks a counterpart to keep; [`GlobalRefused::Name`] for a
    /// name that is empty or over-long; and [`GlobalRefused::Full`] when the
    /// save already holds [`MAX_RECORDS`] records and this call would add one.
    pub fn open(
        &mut self,
        side: Side,
        name: &str,
        factory: &dyn Fn() -> JsonValue,
    ) -> Result<&JsonValue, GlobalRefused> {
        if !side.is_server() {
            return Err(GlobalRefused::NotOnAClient);
        }
        if name.is_empty() || name.len() > MAX_NAME {
            return Err(GlobalRefused::Name);
        }
        if !self.held.contains_key(name) && self.held.len() >= MAX_RECORDS {
            return Err(GlobalRefused::Full);
        }
        Ok(self.held.entry(name.to_owned()).or_insert_with(factory))
    }

    /// Replaces the record `name` holds.
    ///
    /// # Errors
    ///
    /// Returns whatever [`GlobalData::open`] refuses, for the same reasons.
    pub fn write(
        &mut self,
        side: Side,
        name: &str,
        record: JsonValue,
    ) -> Result<(), GlobalRefused> {
        self.open(side, name, &JsonValue::default)?;
        self.held.insert(name.to_owned(), record);
        Ok(())
    }

    /// Returns the record `name` holds, without creating one.
    ///
    /// # Errors
    ///
    /// Returns [`GlobalRefused::NotOnAClient`] on a client side. A read is an
    /// access like any other and the refusal does not soften for it.
    pub fn read(&self, side: Side, name: &str) -> Result<Option<&JsonValue>, GlobalRefused> {
        if !side.is_server() {
            return Err(GlobalRefused::NotOnAClient);
        }
        Ok(self.held.get(name))
    }

    /// Returns how many records the save holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Returns whether the save holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Returns every record's name, in name order.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.held.keys().map(String::as_str).collect()
    }

    /// Builds the declaration of every version this build can read.
    ///
    /// # Errors
    ///
    /// Returns [`SavedDataError`] when the declared version is not a legal one,
    /// which for a constant is a packaging failure.
    pub fn declaration() -> Result<SavedDataDeclaration, SavedDataError> {
        SavedDataDeclaration::new(crate::Shared::declared_id().clone(), [version()?])
    }

    /// Builds the envelope the loader stores.
    ///
    /// # Errors
    ///
    /// Returns [`SavedDataError`] when the declared version is not a legal one.
    pub fn envelope(&self) -> Result<SavedDataEnvelope, SavedDataError> {
        let version = version()?;
        let records: BTreeMap<&str, &JsonValue> = self
            .held
            .iter()
            .map(|(name, record)| (name.as_str(), record))
            .collect();
        Ok(SavedDataEnvelope::current(
            crate::Shared::declared_id().clone(),
            version,
            json!({ "scope": SCOPE, "records": records }),
        ))
    }

    /// Reads the store back out of a stored payload.
    ///
    /// # Errors
    ///
    /// Returns [`GlobalRefused::Saved`] when the payload names another scope,
    /// holds no record map, or holds more records than [`MAX_RECORDS`].
    pub fn from_payload(payload: &JsonValue) -> Result<Self, GlobalRefused> {
        if payload.get("scope").and_then(JsonValue::as_str) != Some(SCOPE) {
            return Err(GlobalRefused::Saved { field: "scope" });
        }
        let records = payload
            .get("records")
            .and_then(JsonValue::as_object)
            .ok_or(GlobalRefused::Saved { field: "records" })?;
        if records.len() > MAX_RECORDS {
            return Err(GlobalRefused::Saved { field: "records" });
        }
        let mut held = BTreeMap::new();
        for (name, record) in records {
            if name.is_empty() || name.len() > MAX_NAME {
                return Err(GlobalRefused::Saved { field: "records" });
            }
            held.insert(name.clone(), record.clone());
        }
        Ok(Self { held })
    }
}

/// The version this build writes, as the loader names one.
///
/// # Errors
///
/// Returns [`SavedDataError::NoVersionsDeclared`] when [`VERSION`] is zero,
/// which a version may not be and which for a constant is a packaging failure.
fn version() -> Result<SavedDataVersion, SavedDataError> {
    SavedDataVersion::new(VERSION).ok_or_else(|| SavedDataError::NoVersionsDeclared {
        owner: crate::Shared::declared_id().clone(),
    })
}

/// Why the global store refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlobalRefused {
    /// The access came from a client, where this data has no meaning.
    NotOnAClient,
    /// The name is empty or longer than a name may be.
    Name,
    /// The save already holds as many records as it may.
    Full,
    /// A stored field is missing or is not what it should be.
    Saved {
        /// Which field.
        field: &'static str,
    },
}

impl fmt::Display for GlobalRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotOnAClient => formatter.write_str(
                "global saved data has no client-side meaning, so a client access is refused \
                 rather than answered empty",
            ),
            Self::Name => write!(
                formatter,
                "a record's name is empty or longer than {MAX_NAME} characters"
            ),
            Self::Full => write!(formatter, "a save holds at most {MAX_RECORDS} records"),
            Self::Saved { field } => write!(
                formatter,
                "the stored `{field}` is missing or is not what it should be"
            ),
        }
    }
}

impl std::error::Error for GlobalRefused {}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> JsonValue {
        json!({ "count": 0 })
    }

    #[test]
    fn a_client_access_is_refused_rather_than_answered_empty() {
        let mut data = GlobalData::new();
        assert_eq!(
            data.open(Side::Client, "counter", &empty),
            Err(GlobalRefused::NotOnAClient)
        );
        assert_eq!(
            data.read(Side::Client, "counter"),
            Err(GlobalRefused::NotOnAClient)
        );
        assert_eq!(
            data.write(Side::Client, "counter", empty()),
            Err(GlobalRefused::NotOnAClient)
        );
        assert!(data.is_empty(), "a refused access creates nothing");
    }

    #[test]
    fn both_server_sides_reach_the_same_store() {
        let mut data = GlobalData::new();
        for side in [Side::IntegratedServer, Side::DedicatedServer] {
            data.open(side, "counter", &empty)
                .expect("a server reaches it");
        }
        assert_eq!(
            data.len(),
            1,
            "the dimension a caller is in decides nothing"
        );
        assert_eq!(data.names(), ["counter"]);
    }

    #[test]
    fn an_absent_record_is_created_from_the_factory_and_a_present_one_is_not() {
        let mut data = GlobalData::new();
        assert_eq!(
            data.open(Side::DedicatedServer, "counter", &empty)
                .expect("a server reaches it"),
            &empty()
        );

        data.write(Side::DedicatedServer, "counter", json!({ "count": 7 }))
            .expect("a server reaches it");
        assert_eq!(
            data.open(Side::DedicatedServer, "counter", &empty)
                .expect("a server reaches it"),
            &json!({ "count": 7 }),
            "a present record is not replaced by the factory"
        );
    }

    #[test]
    fn a_name_is_bounded_because_it_comes_from_a_caller() {
        let mut data = GlobalData::new();
        assert_eq!(
            data.open(Side::DedicatedServer, "", &empty),
            Err(GlobalRefused::Name)
        );
        let long = "n".repeat(MAX_NAME + 1);
        assert_eq!(
            data.open(Side::DedicatedServer, &long, &empty),
            Err(GlobalRefused::Name)
        );
        assert!(data.is_empty());
    }

    #[test]
    fn a_save_holds_at_most_the_declared_number_of_records() {
        let mut data = GlobalData::new();
        for number in 0..MAX_RECORDS {
            data.open(Side::DedicatedServer, &format!("record_{number}"), &empty)
                .expect("inside the bound");
        }
        assert_eq!(data.len(), MAX_RECORDS);
        assert_eq!(
            data.open(Side::DedicatedServer, "one_more", &empty),
            Err(GlobalRefused::Full)
        );
        data.open(Side::DedicatedServer, "record_0", &empty)
            .expect("a record already held is still reachable at the bound");
    }

    #[test]
    fn the_envelope_pins_the_store_to_the_overworld_scope_and_reads_back() {
        let mut data = GlobalData::new();
        data.write(Side::DedicatedServer, "counter", json!({ "count": 3 }))
            .expect("a server reaches it");

        let envelope = data.envelope().expect("the declared version is legal");
        let payload = envelope.payload();
        assert_eq!(payload["scope"], SCOPE);

        let read = GlobalData::from_payload(payload).expect("what was written reads back");
        assert_eq!(read.names(), ["counter"]);
        assert_eq!(
            read.read(Side::DedicatedServer, "counter")
                .expect("a server reaches it"),
            Some(&json!({ "count": 3 }))
        );
    }

    #[test]
    fn a_payload_naming_another_scope_or_holding_no_records_is_refused() {
        assert_eq!(
            GlobalData::from_payload(&json!({ "scope": "the_nether", "records": {} })).err(),
            Some(GlobalRefused::Saved { field: "scope" })
        );
        assert_eq!(
            GlobalData::from_payload(&json!({ "records": {} })).err(),
            Some(GlobalRefused::Saved { field: "scope" })
        );
        assert_eq!(
            GlobalData::from_payload(&json!({ "scope": SCOPE })).err(),
            Some(GlobalRefused::Saved { field: "records" })
        );
        assert_eq!(
            GlobalData::from_payload(&json!({ "scope": SCOPE, "records": [] })).err(),
            Some(GlobalRefused::Saved { field: "records" })
        );
    }

    #[test]
    fn the_declaration_names_this_package_and_the_one_version_it_writes() {
        let declaration = GlobalData::declaration().expect("the declared version is legal");
        assert_eq!(declaration.versions().count(), 1);
        assert_eq!(
            declaration.current(),
            SavedDataVersion::new(VERSION).expect("one is not zero")
        );
    }
}
