use std::fmt;

use ravel_core::{FailureKind, LifecycleState, ModId, StructuredFailure};
use semver::Version;

use crate::SavedDataVersion;

/// Why saved data could not be admitted or emitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SavedDataError {
    /// The owner declared saved data without any readable versions.
    NoVersionsDeclared {
        /// The mod that owns the data.
        owner: ModId,
    },
    /// Stored data is newer than the newest version this build can read.
    FutureVersion {
        /// The mod that owns the data.
        owner: ModId,
        /// The version recorded in the world.
        stored: SavedDataVersion,
        /// The newest version this build declares.
        current: SavedDataVersion,
    },
    /// The stored version or an intermediate version is undeclared.
    NoMigrationPath {
        /// The mod that owns the data.
        owner: ModId,
        /// The version recorded in the world.
        stored: SavedDataVersion,
        /// The first undeclared version required by the plan.
        missing: SavedDataVersion,
    },
    /// A version the plan crosses has no declared migration step into it.
    MissingMigration {
        /// The mod that owns the data.
        owner: ModId,
        /// The version requiring an incoming migration step.
        to: SavedDataVersion,
    },
    /// A declared step refused the payload it was handed.
    MigrationFailed {
        /// The mod that owns the data.
        owner: ModId,
        /// The destination version of the failed step.
        to: SavedDataVersion,
        /// The mod's untrusted failure detail.
        detail: String,
    },
    /// The serialized aggregate exceeds the admission or emission limit.
    AggregateTooLarge {
        /// The maximum permitted size in bytes.
        limit: usize,
    },
    /// The aggregate cannot be decoded or encoded as canonical saved data.
    InvalidAggregate(
        /// The untrusted serialization or structure failure detail.
        String,
    ),
    /// The aggregate uses a format this build does not read.
    UnsupportedAggregateVersion {
        /// The format version recorded in the world.
        stored: u8,
        /// The format version supported by this build.
        current: u8,
    },
    /// An owner's envelope uses a schema this build does not read.
    UnsupportedEnvelopeVersion {
        /// The mod that owns the data.
        owner: ModId,
        /// The envelope schema recorded in the world.
        stored: u8,
        /// The envelope schema supported by this build.
        current: u8,
    },
    /// The aggregate contains more than one envelope for an owner.
    DuplicateOwner {
        /// The owner declared more than once.
        owner: ModId,
    },
    /// An envelope or migration chain names an owner without a matching declaration.
    UnknownOwner {
        /// The owner whose saved-data declaration is unavailable or does not match.
        owner: ModId,
    },
    /// An envelope requires a package that is not installed.
    MissingPackage {
        /// The mod that owns the data.
        owner: ModId,
        /// The absent package.
        package: ModId,
        /// The exact package version required by the envelope.
        required: Version,
    },
    /// An envelope requires a different package version from the installed one.
    IncompatiblePackage(
        /// The requirement and installed version, boxed to keep errors compact.
        Box<PackageConflict>,
    ),
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
    /// Returns the affected owner, or `None` for aggregate-wide failures.
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
