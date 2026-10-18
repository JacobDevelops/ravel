use std::fmt;

use ravel_core::{
    FailureKind, LifecycleState, ModId, Side, StructuredFailure, Version, VersionRange,
};

/// Why a validated package set has no load order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolutionError {
    /// Two packages claim the same mod id.
    DuplicateIdentity {
        /// The claimed identity.
        id: ModId,
    },
    /// A required dependency is not installed at all.
    MissingDependency {
        /// The mod that declared the requirement.
        dependant: ModId,
        /// The mod it requires.
        dependency: ModId,
        /// The versions it would have accepted.
        required: VersionRange,
    },
    /// A required dependency is installed but does not load on this side.
    SideMismatch {
        /// The mod that declared the requirement.
        dependant: ModId,
        /// The mod it requires.
        dependency: ModId,
        /// The side being resolved.
        side: Side,
    },
    /// A dependency is loading at a version outside the declared range.
    UnsatisfiedVersion {
        /// The mod that declared the dependency.
        dependant: ModId,
        /// The mod it depends on.
        dependency: ModId,
        /// The versions it accepts.
        required: VersionRange,
        /// The version that is installed.
        installed: Version,
    },
    /// Two mods that declare each other incompatible are both loading.
    IncompatiblePair {
        /// The mod that declared the incompatibility.
        declaring: ModId,
        /// The mod it refuses to load with.
        rejected: ModId,
        /// The versions it refuses.
        refused: VersionRange,
        /// The version that is installed.
        installed: Version,
    },
    /// The ordering constraints form a cycle.
    Cycle {
        /// The mods in the cycle, in load order, starting at the smallest.
        members: Vec<ModId>,
    },
}

impl fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateIdentity { id } => {
                write!(formatter, "two packages claim the mod id `{id}`")
            }
            Self::MissingDependency {
                dependant,
                dependency,
                required,
            } => write!(
                formatter,
                "`{dependant}` requires `{dependency}` `{required}`, which is not installed"
            ),
            Self::SideMismatch {
                dependant,
                dependency,
                side,
            } => write!(
                formatter,
                "`{dependant}` requires `{dependency}`, which does not load on the {} side",
                side.as_str()
            ),
            Self::UnsatisfiedVersion {
                dependant,
                dependency,
                required,
                installed,
            } => write!(
                formatter,
                "`{dependant}` requires `{dependency}` `{required}`, but `{dependency}` \
                 {installed} is installed"
            ),
            Self::IncompatiblePair {
                declaring,
                rejected,
                refused,
                installed,
            } => write!(
                formatter,
                "`{declaring}` cannot load with `{rejected}` `{refused}`, and `{rejected}` \
                 {installed} is installed"
            ),
            Self::Cycle { members } => write!(
                formatter,
                "the dependency cycle {} cannot be ordered",
                cycle_path(members)
            ),
        }
    }
}

impl std::error::Error for ResolutionError {}

impl StructuredFailure for ResolutionError {
    fn failure_kind(&self) -> Option<FailureKind> {
        Some(match self {
            Self::DuplicateIdentity { .. } => FailureKind::DuplicateIdentity,
            Self::MissingDependency { .. } => FailureKind::MissingRequiredDependency,
            Self::SideMismatch { .. } => FailureKind::SideMismatch,
            Self::UnsatisfiedVersion { .. } | Self::IncompatiblePair { .. } => {
                FailureKind::IncompatibleVersion
            }
            Self::Cycle { .. } => FailureKind::DependencyCycle,
        })
    }

    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Validated
    }

    fn quotes_untrusted_text(&self) -> bool {
        false
    }
}

fn cycle_path(members: &[ModId]) -> String {
    let mut rendered: Vec<String> = members.iter().map(|id| format!("`{id}`")).collect();
    if let Some(first) = rendered.first().cloned() {
        rendered.push(first);
    }
    rendered.join(" -> ")
}

#[cfg(test)]
mod tests;
