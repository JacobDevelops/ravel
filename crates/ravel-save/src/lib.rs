#![forbid(unsafe_code)]

//! Versioned saved-data admission, migration and canonical aggregate contracts.

mod error;
mod migrate;
mod shaping;

pub use error::{PackageConflict, SavedDataError};
pub use migrate::{MigrationChain, MigrationFailed, MigrationStep};
pub use shaping::{ShapingError, ShapingMismatch, ShapingRecord, WorldShaping};

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZero;

use ravel_core::ModId;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The largest decompressed canonical aggregate allowed to cross JNI.
pub const MAX_AGGREGATE_BYTES: usize = 1024 * 1024;
const AGGREGATE_FORMAT_VERSION: u8 = 1;
const ENVELOPE_SCHEMA_VERSION: u8 = 1;

/// One version of one mod's saved data.
pub type SavedDataVersion = NonZero<u32>;

/// What a mod declares about the data it owns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedDataDeclaration {
    owner: ModId,
    versions: BTreeSet<SavedDataVersion>,
}

impl SavedDataDeclaration {
    /// Declares that `owner` can read every version in `versions`.
    ///
    /// # Errors
    ///
    /// Returns [`SavedDataError::NoVersionsDeclared`] when `versions` is empty.
    pub fn new(
        owner: ModId,
        versions: impl IntoIterator<Item = SavedDataVersion>,
    ) -> Result<Self, SavedDataError> {
        let versions: BTreeSet<SavedDataVersion> = versions.into_iter().collect();
        if versions.is_empty() {
            return Err(SavedDataError::NoVersionsDeclared { owner });
        }
        Ok(Self { owner, versions })
    }
    /// Returns the mod that owns this data.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        &self.owner
    }
    /// Returns every version this build can read, in ascending order.
    pub fn versions(&self) -> impl Iterator<Item = SavedDataVersion> + '_ {
        self.versions.iter().copied()
    }
    /// Returns the version this build writes.
    ///
    /// # Panics
    ///
    /// Panics if the declaration has no versions, which [`Self::new`] prevents.
    #[must_use]
    pub fn current(&self) -> SavedDataVersion {
        *self
            .versions
            .last()
            .expect("a declaration holds at least one version")
    }
}

/// What opening a world would do to one mod's saved data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedDataPlan {
    owner: ModId,
    steps: Vec<SavedDataVersion>,
}

impl SavedDataPlan {
    /// Decides whether a world at `stored` opens under `declaration`.
    ///
    /// # Errors
    ///
    /// Returns [`SavedDataError::FutureVersion`] for newer data or [`SavedDataError::NoMigrationPath`] for undeclared versions.
    pub fn resolve(
        declaration: &SavedDataDeclaration,
        stored: SavedDataVersion,
    ) -> Result<Self, SavedDataError> {
        let current = declaration.current();
        if stored > current {
            return Err(SavedDataError::FutureVersion {
                owner: declaration.owner.clone(),
                stored,
                current,
            });
        }
        let crossed: Vec<SavedDataVersion> = (stored.get()..=current.get())
            .filter_map(NonZero::new)
            .collect();
        if let Some(missing) = crossed
            .iter()
            .find(|version| !declaration.versions.contains(version))
        {
            return Err(SavedDataError::NoMigrationPath {
                owner: declaration.owner.clone(),
                stored,
                missing: *missing,
            });
        }
        Ok(Self {
            owner: declaration.owner.clone(),
            steps: crossed.into_iter().skip(1).collect(),
        })
    }
    /// Plans migration, treating absent data as fresh state.
    ///
    /// # Errors
    ///
    /// Returns the errors from [`Self::resolve`] when stored data exists.
    pub fn open(
        declaration: &SavedDataDeclaration,
        stored: Option<SavedDataVersion>,
    ) -> Result<Self, SavedDataError> {
        stored.map_or_else(
            || {
                Ok(Self {
                    owner: declaration.owner.clone(),
                    steps: Vec::new(),
                })
            },
            |stored| Self::resolve(declaration, stored),
        )
    }
    /// Returns the mod whose data this plan is for.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        &self.owner
    }
    /// Returns each migration version in order.
    #[must_use]
    pub fn steps(&self) -> &[SavedDataVersion] {
        &self.steps
    }
    /// Returns whether no migration is needed.
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.steps.is_empty()
    }
}

/// A package and exact version required to read an envelope's payload.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequiredPackage {
    /// The package the payload cannot be read without.
    pub id: ModId,
    /// The version of it the payload was written against.
    pub version: Version,
}

/// One owner's entry inside the aggregate.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedDataEnvelope {
    schema_version: u8,
    owner: ModId,
    data_version: SavedDataVersion,
    required_packages: Vec<RequiredPackage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload_schema: Option<String>,
    payload: Value,
}

impl SavedDataEnvelope {
    /// Builds an envelope without package requirements or a payload schema.
    #[must_use]
    pub fn current(owner: ModId, data_version: SavedDataVersion, payload: Value) -> Self {
        Self::new(owner, data_version, Vec::new(), None, payload)
    }

    /// Builds an envelope in the only schema this release writes.
    #[must_use]
    pub fn new(
        owner: ModId,
        data_version: SavedDataVersion,
        mut required_packages: Vec<RequiredPackage>,
        payload_schema: Option<String>,
        payload: Value,
    ) -> Self {
        required_packages.sort();
        Self {
            schema_version: ENVELOPE_SCHEMA_VERSION,
            owner,
            data_version,
            required_packages,
            payload_schema,
            payload,
        }
    }
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        &self.owner
    }
    #[must_use]
    pub const fn data_version(&self) -> SavedDataVersion {
        self.data_version
    }
    #[must_use]
    pub const fn payload(&self) -> &Value {
        &self.payload
    }

    /// Returns a migrated copy, preserving ownership, requirements, and schema.
    #[must_use]
    pub fn migrated(&self, data_version: SavedDataVersion, payload: Value) -> Self {
        Self {
            schema_version: self.schema_version,
            owner: self.owner.clone(),
            data_version,
            required_packages: self.required_packages.clone(),
            payload_schema: self.payload_schema.clone(),
            payload,
        }
    }
}

/// The one canonical multi-owner value stored by Minecraft.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedDataAggregate {
    format_version: u8,
    owners: Vec<SavedDataEnvelope>,
}

impl SavedDataAggregate {
    #[must_use]
    pub const fn fresh() -> Self {
        Self {
            format_version: AGGREGATE_FORMAT_VERSION,
            owners: Vec::new(),
        }
    }

    /// Parses and admits a bounded aggregate, treating `None` as fresh state.
    ///
    /// # Errors
    ///
    /// Returns [`SavedDataError`] for oversized, invalid, unsupported, or incompatible saved data.
    pub fn open(
        bytes: Option<&[u8]>,
        declarations: &[SavedDataDeclaration],
        packages: &BTreeMap<ModId, Version>,
    ) -> Result<Self, SavedDataError> {
        let Some(bytes) = bytes else {
            return Ok(Self::fresh());
        };
        if bytes.len() > MAX_AGGREGATE_BYTES {
            return Err(SavedDataError::AggregateTooLarge {
                limit: MAX_AGGREGATE_BYTES,
            });
        }
        let mut aggregate: Self = serde_json::from_slice(bytes)
            .map_err(|error| SavedDataError::InvalidAggregate(error.to_string()))?;
        aggregate.admit(declarations, packages)?;
        Ok(aggregate)
    }

    fn admit(
        &mut self,
        declarations: &[SavedDataDeclaration],
        packages: &BTreeMap<ModId, Version>,
    ) -> Result<(), SavedDataError> {
        if self.format_version != AGGREGATE_FORMAT_VERSION {
            return Err(SavedDataError::UnsupportedAggregateVersion {
                stored: self.format_version,
                current: AGGREGATE_FORMAT_VERSION,
            });
        }
        self.owners
            .sort_by(|left, right| left.owner.cmp(&right.owner));
        for pair in self.owners.windows(2) {
            if pair[0].owner == pair[1].owner {
                return Err(SavedDataError::DuplicateOwner {
                    owner: pair[0].owner.clone(),
                });
            }
        }
        for envelope in &self.owners {
            if envelope.schema_version != ENVELOPE_SCHEMA_VERSION {
                return Err(SavedDataError::UnsupportedEnvelopeVersion {
                    owner: envelope.owner.clone(),
                    stored: envelope.schema_version,
                    current: ENVELOPE_SCHEMA_VERSION,
                });
            }
            let declaration = declarations
                .iter()
                .find(|declaration| declaration.owner() == envelope.owner())
                .ok_or_else(|| SavedDataError::UnknownOwner {
                    owner: envelope.owner.clone(),
                })?;
            SavedDataPlan::resolve(declaration, envelope.data_version)?;
            for required in &envelope.required_packages {
                match packages.get(&required.id) {
                    Some(version) if version == &required.version => {}
                    Some(version) => {
                        return Err(SavedDataError::IncompatiblePackage(Box::new(
                            PackageConflict {
                                owner: envelope.owner.clone(),
                                package: required.id.clone(),
                                required: required.version.clone(),
                                installed: version.clone(),
                            },
                        )));
                    }
                    None => {
                        return Err(SavedDataError::MissingPackage {
                            owner: envelope.owner.clone(),
                            package: required.id.clone(),
                            required: required.version.clone(),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn envelope(&self, owner: &ModId) -> Option<&SavedDataEnvelope> {
        self.owners
            .binary_search_by(|envelope| envelope.owner.cmp(owner))
            .ok()
            .map(|index| &self.owners[index])
    }

    pub fn replace(&mut self, envelope: SavedDataEnvelope) {
        match self
            .owners
            .binary_search_by(|stored| stored.owner.cmp(envelope.owner()))
        {
            Ok(index) => self.owners[index] = envelope,
            Err(index) => self.owners.insert(index, envelope),
        }
    }

    /// Serializes the exact canonical bytes crossing JNI.
    ///
    /// # Errors
    ///
    /// Returns [`SavedDataError::AggregateTooLarge`] above the byte limit or [`SavedDataError::InvalidAggregate`] on serialization failure.
    pub fn to_bytes(&self) -> Result<Vec<u8>, SavedDataError> {
        let bytes = serde_json::to_vec(self)
            .map_err(|error| SavedDataError::InvalidAggregate(error.to_string()))?;
        if bytes.len() > MAX_AGGREGATE_BYTES {
            return Err(SavedDataError::AggregateTooLarge {
                limit: MAX_AGGREGATE_BYTES,
            });
        }
        Ok(bytes)
    }
}
impl Default for SavedDataAggregate {
    fn default() -> Self {
        Self::fresh()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_AGGREGATE_BYTES, PackageConflict, RequiredPackage, SavedDataAggregate,
        SavedDataDeclaration, SavedDataEnvelope, SavedDataError, SavedDataPlan, SavedDataVersion,
    };
    use ravel_core::ModId;
    use semver::Version;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::num::NonZero;
    pub(super) fn id(value: &str) -> ModId {
        value.parse().expect("a legal id")
    }
    pub(super) fn version(raw: u32) -> SavedDataVersion {
        NonZero::new(raw).expect("non-zero")
    }
    fn declaration() -> SavedDataDeclaration {
        SavedDataDeclaration::new(id("saved_data"), [version(1), version(2), version(3)])
            .expect("versions")
    }

    #[test]
    fn absence_initializes_fresh_state() {
        assert!(
            SavedDataPlan::open(&declaration(), None)
                .expect("absence is fresh")
                .is_current()
        );
        assert_eq!(
            SavedDataAggregate::open(None, &[declaration()], &BTreeMap::new()).expect("fresh"),
            SavedDataAggregate::fresh()
        );
    }
    #[test]
    fn an_envelope_from_the_ordinary_constructor_crosses_the_aggregate() {
        let mut aggregate = SavedDataAggregate::fresh();
        aggregate.replace(SavedDataEnvelope::current(
            id("saved_data"),
            version(1),
            json!({"entry": "found copper"}),
        ));
        let bytes = aggregate.to_bytes().expect("bounded");
        let declarations = [SavedDataDeclaration::new(id("saved_data"), [version(1)]).expect("v")];
        assert_eq!(
            SavedDataAggregate::open(Some(&bytes), &declarations, &BTreeMap::new()).expect("valid"),
            aggregate
        );
    }
    #[test]
    fn canonical_output_is_owner_sorted_and_round_trips() {
        let mut aggregate = SavedDataAggregate::fresh();
        aggregate.replace(SavedDataEnvelope::new(
            id("zeta"),
            version(1),
            Vec::new(),
            None,
            json!({"b": 2, "a": 1}),
        ));
        aggregate.replace(SavedDataEnvelope::new(
            id("alpha"),
            version(1),
            Vec::new(),
            None,
            json!([1, 2]),
        ));
        let declarations = [
            SavedDataDeclaration::new(id("alpha"), [version(1)]).expect("versions"),
            SavedDataDeclaration::new(id("zeta"), [version(1)]).expect("versions"),
        ];
        let bytes = aggregate.to_bytes().expect("bounded");
        let reopened =
            SavedDataAggregate::open(Some(&bytes), &declarations, &BTreeMap::new()).expect("valid");
        assert_eq!(reopened, aggregate);
        let text = String::from_utf8(bytes).expect("utf8");
        assert!(text.find("alpha").expect("alpha") < text.find("zeta").expect("zeta"));
    }
    #[test]
    fn required_packages_are_checked_exactly() {
        let mut aggregate = SavedDataAggregate::fresh();
        aggregate.replace(SavedDataEnvelope::new(
            id("saved_data"),
            version(1),
            vec![RequiredPackage {
                id: id("base"),
                version: Version::new(2, 0, 0),
            }],
            None,
            json!(null),
        ));
        let bytes = aggregate.to_bytes().expect("bytes");
        assert!(matches!(
            SavedDataAggregate::open(Some(&bytes), &[declaration()], &BTreeMap::new()),
            Err(SavedDataError::MissingPackage { .. })
        ));
    }
    // Build stored bytes directly to exercise unsupported schema versions.
    fn stored_under(format_version: u8, schema_version: u8) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "formatVersion": format_version,
            "owners": [{
                "schemaVersion": schema_version,
                "owner": "saved_data",
                "dataVersion": 1,
                "requiredPackages": [],
                "payload": null,
            }],
        }))
        .expect("an aggregate this test wrote serializes")
    }

    #[test]
    fn an_aggregate_format_this_build_cannot_read_fails_closed() {
        SavedDataAggregate::open(
            Some(&stored_under(1, 1)),
            &[declaration()],
            &BTreeMap::new(),
        )
        .expect("the format this build writes");
        assert_eq!(
            SavedDataAggregate::open(
                Some(&stored_under(2, 1)),
                &[declaration()],
                &BTreeMap::new()
            ),
            Err(SavedDataError::UnsupportedAggregateVersion {
                stored: 2,
                current: 1,
            })
        );
    }

    #[test]
    fn an_envelope_schema_this_build_cannot_read_fails_closed() {
        assert_eq!(
            SavedDataAggregate::open(
                Some(&stored_under(1, 2)),
                &[declaration()],
                &BTreeMap::new()
            ),
            Err(SavedDataError::UnsupportedEnvelopeVersion {
                owner: id("saved_data"),
                stored: 2,
                current: 1,
            })
        );
    }

    #[test]
    fn a_required_package_installed_at_another_version_fails_closed() {
        let mut aggregate = SavedDataAggregate::fresh();
        aggregate.replace(SavedDataEnvelope::new(
            id("saved_data"),
            version(1),
            vec![RequiredPackage {
                id: id("base"),
                version: Version::new(2, 0, 0),
            }],
            None,
            json!(null),
        ));
        let bytes = aggregate.to_bytes().expect("bytes");

        let installed = BTreeMap::from([(id("base"), Version::new(1, 0, 0))]);
        assert_eq!(
            SavedDataAggregate::open(Some(&bytes), &[declaration()], &installed),
            Err(SavedDataError::IncompatiblePackage(Box::new(
                PackageConflict {
                    owner: id("saved_data"),
                    package: id("base"),
                    required: Version::new(2, 0, 0),
                    installed: Version::new(1, 0, 0),
                }
            )))
        );

        let exact = BTreeMap::from([(id("base"), Version::new(2, 0, 0))]);
        SavedDataAggregate::open(Some(&bytes), &[declaration()], &exact).expect("an exact match");
    }

    #[test]
    fn the_limit_is_enforced_before_json_parsing() {
        let oversized = vec![b' '; MAX_AGGREGATE_BYTES + 1];
        assert!(matches!(
            SavedDataAggregate::open(Some(&oversized), &[declaration()], &BTreeMap::new()),
            Err(SavedDataError::AggregateTooLarge { .. })
        ));
    }
    #[test]
    fn future_and_gapped_versions_fail_closed() {
        assert!(matches!(
            SavedDataPlan::resolve(&declaration(), version(9)),
            Err(SavedDataError::FutureVersion { .. })
        ));
        let gap = SavedDataDeclaration::new(id("saved_data"), [version(1), version(3)])
            .expect("versions");
        assert!(matches!(
            SavedDataPlan::resolve(&gap, version(1)),
            Err(SavedDataError::NoMigrationPath { .. })
        ));
    }
}
