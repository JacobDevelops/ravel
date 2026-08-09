#![forbid(unsafe_code)]

//! Versioned saved-data admission, migration and canonical aggregate contracts.

mod migrate;

pub use migrate::{MigrationChain, MigrationFailed, MigrationStep};

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::num::NonZero;

use ravel_core::{FailureKind, LifecycleState, ModId, StructuredFailure};
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
    /// `SPEC.md` §14 requires persistent data to be versioned, so an owner that
    /// names no version owns data nothing can decide.
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
    /// Panics if this declaration holds no version, which [`Self::new`] refuses
    /// to build.
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
    /// Returns [`SavedDataError::FutureVersion`] when the world was written by a
    /// build that knows a version above the highest declared one, and
    /// [`SavedDataError::NoMigrationPath`] when a version between `stored` and
    /// the current one is undeclared, because migrating across a gap would be
    /// guessing at a format this build never read.
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
    /// Decides whether a world opens when it may hold no data for this owner.
    ///
    /// Absence initializes fresh state for any world, so only a version this
    /// build cannot read refuses.
    ///
    /// # Errors
    ///
    /// Returns whatever [`Self::resolve`] refuses when the world holds a
    /// version for this owner.
    pub fn open(
        declaration: &SavedDataDeclaration,
        stored: Option<SavedDataVersion>,
    ) -> Result<Self, SavedDataError> {
        match stored {
            Some(stored) => Self::resolve(declaration, stored),
            None => Ok(Self {
                owner: declaration.owner.clone(),
                steps: Vec::new(),
            }),
        }
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

/// One package an envelope requires to interpret its payload.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequiredPackage {
    pub id: ModId,
    pub version: Version,
}

/// One owner's entry inside the aggregate.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
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

    /// Returns this envelope at `data_version` carrying `payload`.
    ///
    /// A new value rather than a mutation, because a migration that refuses
    /// halfway must leave the aggregate holding exactly what it held. Everything
    /// else travels unchanged: a migration rewrites a payload's shape and decides
    /// nothing about who owns it or what it needs installed.
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
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
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

    /// Parses and admits at most one bounded aggregate.
    ///
    /// The 1 MiB ceiling is measured before anything is parsed, so oversized
    /// world input is refused rather than turned into native allocation.
    /// `bytes` of `None` is absence, which initializes a fresh aggregate.
    ///
    /// # Errors
    ///
    /// Returns [`SavedDataError::AggregateTooLarge`] above the ceiling,
    /// [`SavedDataError::InvalidAggregate`] when the bytes are not this format,
    /// and otherwise whatever admitting one owner refuses: an unsupported
    /// aggregate or envelope version, a duplicated or uninstalled owner, a
    /// version [`SavedDataPlan::resolve`] will not cross, or a required package
    /// that is missing or installed at another version.
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
    /// Returns [`SavedDataError::AggregateTooLarge`] when what this build
    /// produced exceeds the ceiling, so an oversized replacement is refused
    /// before its bytes reach Java, and [`SavedDataError::InvalidAggregate`]
    /// when an owner's payload cannot be serialized at all.
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

/// Why saved data could not be admitted or emitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SavedDataError {
    NoVersionsDeclared {
        owner: ModId,
    },
    FutureVersion {
        owner: ModId,
        stored: SavedDataVersion,
        current: SavedDataVersion,
    },
    NoMigrationPath {
        owner: ModId,
        stored: SavedDataVersion,
        missing: SavedDataVersion,
    },
    /// A version the plan crosses has no declared migration step into it.
    MissingMigration {
        owner: ModId,
        to: SavedDataVersion,
    },
    /// A declared step refused the payload it was handed.
    MigrationFailed {
        owner: ModId,
        to: SavedDataVersion,
        /// The mod's own account, which is why this is the one migration refusal
        /// that quotes untrusted text.
        detail: String,
    },
    AggregateTooLarge {
        limit: usize,
    },
    InvalidAggregate(String),
    UnsupportedAggregateVersion {
        stored: u8,
        current: u8,
    },
    UnsupportedEnvelopeVersion {
        owner: ModId,
        stored: u8,
        current: u8,
    },
    DuplicateOwner {
        owner: ModId,
    },
    UnknownOwner {
        owner: ModId,
    },
    MissingPackage {
        owner: ModId,
        package: ModId,
        required: Version,
    },
    /// Boxed because two versions and two identities together are most of this
    /// enum's size, and every refusal would otherwise be returned that wide.
    IncompatiblePackage(Box<PackageConflict>),
}

/// What an envelope required of a package and what is installed instead.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageConflict {
    /// The owner whose data named the requirement.
    pub owner: ModId,
    /// The package that data requires.
    pub package: ModId,
    /// The version that data requires.
    pub required: Version,
    /// The version this installation holds instead.
    pub installed: Version,
}

impl SavedDataError {
    #[must_use]
    pub const fn owner(&self) -> Option<&ModId> {
        match self {
            Self::NoVersionsDeclared { owner }
            | Self::FutureVersion { owner, .. }
            | Self::NoMigrationPath { owner, .. }
            | Self::MissingMigration { owner, .. }
            | Self::MigrationFailed { owner, .. }
            | Self::UnsupportedEnvelopeVersion { owner, .. }
            | Self::DuplicateOwner { owner }
            | Self::UnknownOwner { owner }
            | Self::MissingPackage { owner, .. } => Some(owner),
            Self::IncompatiblePackage(conflict) => Some(&conflict.owner),
            Self::AggregateTooLarge { .. }
            | Self::InvalidAggregate(_)
            | Self::UnsupportedAggregateVersion { .. } => None,
        }
    }
}
impl fmt::Display for SavedDataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoVersionsDeclared { owner } => write!(
                formatter,
                "`{owner}` owns saved data and declares no version of it"
            ),
            Self::FutureVersion {
                owner,
                stored,
                current,
            } => write!(
                formatter,
                "the world holds `{owner}` saved data at version {stored}, newer than this build's {current}"
            ),
            Self::NoMigrationPath {
                owner,
                stored,
                missing,
            } => write!(
                formatter,
                "`{owner}` cannot migrate saved data from {stored}; version {missing} is undeclared"
            ),
            Self::MissingMigration { owner, to } => write!(
                formatter,
                "`{owner}` declares saved-data version {to} and no migration into it"
            ),
            Self::MigrationFailed { owner, to, detail } => write!(
                formatter,
                "`{owner}` could not migrate its saved data to version {to}: {detail}"
            ),
            Self::AggregateTooLarge { limit } => write!(
                formatter,
                "the canonical saved-data aggregate exceeds the {limit}-byte limit"
            ),
            Self::InvalidAggregate(detail) => {
                write!(formatter, "the saved-data aggregate is invalid: {detail}")
            }
            Self::UnsupportedAggregateVersion { stored, current } => write!(
                formatter,
                "aggregate format {stored} is unsupported; this build reads {current}"
            ),
            Self::UnsupportedEnvelopeVersion {
                owner,
                stored,
                current,
            } => write!(
                formatter,
                "`{owner}` envelope schema {stored} is unsupported; this build reads {current}"
            ),
            Self::DuplicateOwner { owner } => {
                write!(formatter, "the aggregate contains `{owner}` more than once")
            }
            Self::UnknownOwner { owner } => write!(
                formatter,
                "the aggregate contains data for uninstalled owner `{owner}`"
            ),
            Self::MissingPackage {
                owner,
                package,
                required,
            } => write!(
                formatter,
                "`{owner}` data requires missing package `{package}` {required}"
            ),
            Self::IncompatiblePackage(conflict) => write!(
                formatter,
                "`{}` data requires `{}` {}, but {} is installed",
                conflict.owner, conflict.package, conflict.required, conflict.installed
            ),
        }
    }
}
impl std::error::Error for SavedDataError {}
impl StructuredFailure for SavedDataError {
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }
    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Registered
    }
    fn quotes_untrusted_text(&self) -> bool {
        matches!(
            self,
            Self::InvalidAggregate(_) | Self::MigrationFailed { .. }
        )
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
    fn id(value: &str) -> ModId {
        value.parse().expect("a legal id")
    }
    fn version(raw: u32) -> SavedDataVersion {
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
    /// The bytes a world holds when it was written under other schema versions.
    ///
    /// [`SavedDataEnvelope::new`] stamps the only schemas this release writes,
    /// so a stored version this build cannot read has to be written as bytes
    /// rather than built through the constructor. That is also how such a world
    /// actually arrives: another build wrote it.
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
        // The owner inside is one this build reads, so the container is the
        // only thing left to have refused it.
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

        // Installed, and required, and not the same version: the case a partial
        // upgrade produces rather than the missing-package one already proven.
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

        // The exact version it asked for is the one thing that admits it.
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
