#![forbid(unsafe_code)]

//! A configurable journal with versioned world data and an entries service.
//!
//! Version 1 stores one string; version 2 stores an entry list. Migration
//! preserves the old entry and refuses malformed payloads without changing state.

use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration};
use ravel_sdk::content::ContentId;
use ravel_sdk::save::{
    MigrationChain, MigrationFailed, SavedDataDeclaration, SavedDataEnvelope, SavedDataPlan,
    SavedDataVersion, SavedPayload,
};
use ravel_sdk::services::{ServiceDeclaration, ServiceId, ServiceRequest, ServiceResponse};
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod, json};

/// Player- and pack-configurable journal settings.
const SETTINGS: &str = r#"{
  "schemaVersion": 1,
  "owner": "stateful",
  "version": 1,
  "settings": [
    {
      "key": "journal.capacity",
      "title": "Journal capacity",
      "description": "How many entries the journal keeps before the oldest is dropped.",
      "valueSchema": { "type": "integer", "minimum": 1.0, "maximum": 128.0 },
      "default": 16,
      "side": "common",
      "restartRequired": false
    }
  ]
}"#;

const CAPACITY: &str = "journal.capacity";

// Read oldest through newest; write the highest version.
const VERSIONS: [u32; 2] = [1, 2];

fn version(raw: u32) -> SavedDataVersion {
    SavedDataVersion::new(raw).expect("every declared version is non-zero")
}

/// The service this mod answers.
///
/// # Panics
///
/// Panics if `entries` stops being a legal content path.
#[must_use]
pub fn entries_service() -> ServiceId {
    ServiceId::new(
        ContentId::new(Journal::declared_id().clone(), "entries").expect("a legal content path"),
    )
}

// Preserve the single entry when upgrading to a list.
fn to_two(payload: &SavedPayload) -> Result<SavedPayload, MigrationFailed> {
    let entry = payload
        .get("entry")
        .and_then(SavedPayload::as_str)
        .ok_or_else(|| MigrationFailed::new("version 1 held no `entry` string"))?;
    let entries: Vec<&str> = if entry.is_empty() {
        Vec::new()
    } else {
        vec![entry]
    };
    Ok(json!({ "entries": entries }))
}

/// A mod that keeps a short journal and lets other mods read it.
///
/// # Examples
///
/// ```
/// use ravel_example_stateful_mod::Journal;
/// use ravel_sdk::config::LayeredConfiguration;
/// use ravel_sdk::save::{SavedDataPlan, SavedDataVersion};
/// use ravel_sdk::{Mod, json};
///
/// let mut journal = Journal::new();
///
/// // Settings resolve to the author's default when nothing overrides them.
/// let declaration = journal
///     .configuration_declaration()
///     .expect("this mod declares settings");
/// let resolved = LayeredConfiguration::resolve(declaration, None, None)?;
/// journal.configure(&resolved)?;
/// assert_eq!(journal.capacity(), 16);
///
/// // A version-1 world reaches version 2 through the declared step.
/// let one = SavedDataVersion::new(1).expect("1 is not zero");
/// let plan = SavedDataPlan::open(&journal.declaration(), Some(one))?;
/// assert_eq!(
///     journal.chain().apply(&plan, &json!({ "entry": "found copper" }))?,
///     json!({ "entries": ["found copper"] })
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Journal {
    capacity: usize,
    entries: Vec<String>,
}

impl Journal {
    /// Builds the mod instance the loader drives.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the versions this build reads.
    ///
    /// # Panics
    ///
    /// Panics if [`VERSIONS`] stops being a non-empty list of non-zero versions.
    #[must_use]
    pub fn declaration(&self) -> SavedDataDeclaration {
        SavedDataDeclaration::new(
            Self::declared_id().clone(),
            VERSIONS.into_iter().map(version),
        )
        .expect("`VERSIONS` is a non-empty list of non-zero versions")
    }

    /// Returns the step into every version above the oldest.
    #[must_use]
    pub fn chain(&self) -> MigrationChain {
        MigrationChain::new(Self::declared_id().clone()).reaching(version(2), to_two)
    }

    /// Returns how many entries the settings in force allow.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the entries currently kept, oldest first.
    #[must_use]
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// Records `entry`, dropping the oldest once the journal is full.
    pub fn record(&mut self, entry: impl Into<String>) {
        self.entries.push(entry.into());
        while self.entries.len() > self.capacity {
            self.entries.remove(0);
        }
    }
}

#[declare_mod(id = "stateful")]
impl Mod for Journal {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }

    fn configuration_declaration(&self) -> Option<ConfigurationDeclaration> {
        ConfigurationDeclaration::parse(SETTINGS).ok()
    }

    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
        let refuse = |detail: String| ModError::Refused {
            mod_id: Self::declared_id().clone(),
            state: LifecycleState::Constructed,
            detail,
        };
        let key = CAPACITY
            .parse()
            .map_err(|_| refuse(format!("`{CAPACITY}` is not a legal setting key")))?;
        let capacity = resolved
            .value(&key)
            .and_then(SavedPayload::as_u64)
            .ok_or_else(|| {
                refuse(format!(
                    "`{CAPACITY}` resolved to something that is not a count"
                ))
            })?;
        self.capacity = usize::try_from(capacity).map_err(|_| {
            refuse(format!(
                "`{CAPACITY}` resolved to a count this platform cannot hold"
            ))
        })?;
        Ok(())
    }

    fn saved_data_declaration(&self) -> Option<SavedDataDeclaration> {
        Some(self.declaration())
    }

    fn saved_data_migrations(&self) -> Option<MigrationChain> {
        Some(self.chain())
    }

    /// Migrates the stored payload to the current version and reads it.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] for unreadable data, leaving state unchanged.
    fn open_saved_data(
        &mut self,
        envelope: Option<&SavedDataEnvelope>,
        plan: &SavedDataPlan,
    ) -> Result<(), ModError> {
        let Some(envelope) = envelope else {
            return Ok(());
        };
        let refuse = |detail: String| ModError::Refused {
            mod_id: Self::declared_id().clone(),
            state: LifecycleState::Registered,
            detail,
        };
        let current = self
            .chain()
            .apply(plan, envelope.payload())
            .map_err(|refusal| refuse(refusal.to_string()))?;
        let stored = current
            .get("entries")
            .and_then(SavedPayload::as_array)
            .ok_or_else(|| refuse("the stored journal holds no entry list".to_owned()))?;

        let mut entries = Vec::with_capacity(stored.len());
        for value in stored {
            let entry = value
                .as_str()
                .ok_or_else(|| refuse("a stored journal entry is not text".to_owned()))?;
            entries.push(entry.to_owned());
        }
        self.entries = entries;
        Ok(())
    }

    fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, ModError> {
        Ok(Some(SavedDataEnvelope::new(
            Self::declared_id().clone(),
            self.declaration().current(),
            Vec::new(),
            None,
            json!({ "entries": self.entries }),
        )))
    }

    fn service_declaration(&self) -> Option<ServiceDeclaration> {
        ServiceDeclaration::new(Self::declared_id().clone(), [entries_service()], []).ok()
    }

    fn answer_service(&mut self, request: &ServiceRequest) -> Result<ServiceResponse, ModError> {
        let _ = request;
        Ok(ServiceResponse::new(json!({
            "entries": self.entries,
            "capacity": self.capacity,
        })))
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::config::LayeredConfiguration;
    use ravel_sdk::save::{SavedDataEnvelope, SavedDataPlan, SavedPayload};
    use ravel_sdk::services::ServiceRequest;
    use ravel_sdk::{Declared, Mod, json};

    use super::{CAPACITY, Journal, VERSIONS, entries_service, version};

    fn configured() -> Journal {
        let mut journal = Journal::new();
        let declaration = journal
            .configuration_declaration()
            .expect("this mod declares settings");
        let resolved =
            LayeredConfiguration::resolve(declaration, None, None).expect("nothing to layer");
        journal.configure(&resolved).expect("its own default");
        journal
    }

    fn opened(at: u32, payload: SavedPayload) -> Result<Journal, ravel_sdk::ModError> {
        let mut journal = configured();
        let plan = SavedDataPlan::open(&journal.declaration(), Some(version(at)))
            .expect("a declared version");
        let envelope = SavedDataEnvelope::new(
            Journal::declared_id().clone(),
            version(at),
            Vec::new(),
            None,
            payload,
        );
        journal.open_saved_data(Some(&envelope), &plan)?;
        Ok(journal)
    }

    #[test]
    fn the_chain_covers_every_version_the_declaration_admits() {
        let journal = Journal::new();
        journal
            .chain()
            .covers(&journal.declaration())
            .expect("every declared version has a step into it");
        assert_eq!(
            journal.declaration().current(),
            version(*VERSIONS.last().expect("`VERSIONS` is not empty"))
        );
    }

    #[test]
    fn a_version_one_world_keeps_the_entry_it_held() {
        let journal = opened(1, json!({ "entry": "found copper" })).expect("the step runs");
        assert_eq!(journal.entries(), ["found copper"]);
    }

    #[test]
    fn a_version_one_world_that_held_nothing_becomes_an_empty_list() {
        let journal = opened(1, json!({ "entry": "" })).expect("the step runs");
        assert_eq!(journal.entries(), [] as [String; 0]);
    }

    #[test]
    fn a_shape_the_step_cannot_read_refuses_and_leaves_the_journal_empty() {
        let refusal =
            opened(1, json!({ "note": "found copper" })).expect_err("version 1 held no `entry`");
        assert_eq!(refusal.mod_id(), Journal::declared_id());
        assert!(refusal.to_string().contains("`entry`"), "{refusal}");
    }

    #[test]
    fn what_this_mod_writes_is_what_it_reads_back() {
        let mut journal = configured();
        journal.record("found copper");
        journal.record("lit a lantern");
        let envelope = journal
            .save_saved_data()
            .expect("this mod owns world data")
            .expect("and produces an envelope");

        let mut reopened = configured();
        let plan = SavedDataPlan::open(&reopened.declaration(), Some(envelope.data_version()))
            .expect("the current version");
        reopened
            .open_saved_data(Some(&envelope), &plan)
            .expect("its own shape");
        assert_eq!(reopened.entries(), journal.entries());
    }

    #[test]
    fn the_journal_never_grows_past_the_capacity_in_force() {
        let mut journal = configured();
        let capacity = journal.capacity();
        for index in 0..=capacity {
            journal.record(format!("entry {index}"));
        }
        assert_eq!(journal.entries().len(), capacity);
        assert_eq!(journal.entries()[0], "entry 1", "the oldest was dropped");
    }

    #[test]
    fn a_pack_may_state_a_capacity_and_the_mod_uses_it() {
        use ravel_sdk::config::ValueDocument;

        let pack = ValueDocument::parse(
            r#"{
              "schemaVersion": 1,
              "owner": "stateful",
              "values": { "journal.capacity": 2 }
            }"#,
        )
        .expect("a legal override document");

        let mut journal = Journal::new();
        let declaration = journal
            .configuration_declaration()
            .expect("this mod declares settings");
        let resolved = LayeredConfiguration::resolve(declaration, Some(&pack), None)
            .expect("the pack states a declared key");
        journal.configure(&resolved).expect("a legal value");

        assert_eq!(journal.capacity(), 2);
        let key = CAPACITY.parse().expect("a legal key");
        assert_eq!(
            resolved.source(&key),
            Some(ravel_sdk::config::ValueSource::Pack)
        );
    }

    #[test]
    fn the_service_is_named_in_this_mods_namespace_and_answers_what_it_holds() {
        let mut journal = configured();
        journal.record("found copper");
        assert_eq!(entries_service().owner(), Journal::declared_id());
        assert_eq!(entries_service().to_string(), "stateful:entries");

        let answer = journal
            .answer_service(&ServiceRequest::new(entries_service(), json!(null)))
            .expect("the mod answers what it declared");
        assert_eq!(
            answer.value(),
            &json!({ "entries": ["found copper"], "capacity": 16 })
        );
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(Journal::new());
