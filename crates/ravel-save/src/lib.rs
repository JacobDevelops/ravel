#![forbid(unsafe_code)]

//! Versioned saved-data ownership and fail-closed migration contracts.
//!
//! `SPEC.md` §14 requires persistent data to be versioned and owned by a stable
//! canonical mod id, and requires a failure after world mutation never to
//! continue silently or save partial state. This crate owns the deciding half:
//! what a saved-data version is, who owns it, and whether a world written under
//! one version may be opened under another. Moving bytes in a world belongs to
//! the adapter, and cannot begin until a decision admits the game objects it
//! would have to carry.
//!
//! Nothing here migrates data. It decides *whether* a migration exists and what
//! it would step through, because that is the decision that has to fail closed,
//! and it has to fail closed before a world opens rather than partway through
//! rewriting one.

use std::collections::BTreeSet;
use std::fmt;
use std::num::NonZero;

use ravel_core::{FailureKind, LifecycleState, ModId, StructuredFailure};

/// One version of one mod's saved data.
///
/// Versions are non-zero because zero is what an absent version reads as, and a
/// world claiming version 0 is a world whose version was never written. The
/// loader already carries them in this shape, so a declaration and a plan agree
/// with the lifecycle without converting.
pub type SavedDataVersion = NonZero<u32>;

/// What a mod declares about the data it owns.
///
/// The declared versions are every version this build can read, including the
/// one it writes. `SPEC.md` §14 ties ownership to the mod id, so the owner
/// travels with the declaration and no report has to infer it.
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
    /// Returns [`SavedDataError::NoVersionsDeclared`] for an empty set. A mod
    /// owning saved data and declaring no version of it could neither read a
    /// world nor write one, and treating that as "owns no saved data" would
    /// drop a mod's persistence silently rather than report it.
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
    /// It is the highest declared version. A build able to read a version newer
    /// than it writes would be declaring a format it never produces, and the
    /// world it wrote would then be older than the world it opened.
    ///
    /// # Panics
    ///
    /// Never. [`SavedDataDeclaration::new`] refuses an empty set, so a
    /// declaration always holds at least one version.
    #[must_use]
    pub fn current(&self) -> SavedDataVersion {
        *self
            .versions
            .last()
            .expect("a declaration holds at least one version")
    }
}

/// What opening a world would do to one mod's saved data.
///
/// This is the decision, not the migration. A plan that resolves names the
/// versions the world would be stepped through; a plan that refuses names why,
/// before anything is read or rewritten.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedDataPlan {
    owner: ModId,
    steps: Vec<SavedDataVersion>,
}

impl SavedDataPlan {
    /// Decides whether a world at `stored` opens under `declaration`.
    ///
    /// A world already at the current version resolves to a plan with no steps,
    /// which is a different answer from a plan that could not be built.
    ///
    /// # Errors
    ///
    /// Returns [`SavedDataError::FutureVersion`] when the world was written by
    /// a newer build: a format this one has never seen cannot be read, and
    /// guessing at it is the silent continuation `SPEC.md` §14 forbids. Returns
    /// [`SavedDataError::NoMigrationPath`] when the declared versions have a
    /// gap between `stored` and the current one.
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

        // Every version from the stored one forward has to be declared, or
        // nothing steps across the gap.
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
            // The stored version is where the world already sits, so the steps
            // are what follows it.
            steps: crossed.into_iter().skip(1).collect(),
        })
    }

    /// Returns the mod whose data this plan is for.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        &self.owner
    }

    /// Returns each version the world would be stepped to, in order.
    #[must_use]
    pub fn steps(&self) -> &[SavedDataVersion] {
        &self.steps
    }

    /// Returns whether the world already sits at the version this build writes.
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.steps.is_empty()
    }
}

/// Why saved data could not be admitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SavedDataError {
    /// A mod declared saved data and no version of it.
    NoVersionsDeclared {
        /// The mod that declared nothing.
        owner: ModId,
    },
    /// The world was written by a newer build than the one opening it.
    FutureVersion {
        /// The mod whose data cannot be read.
        owner: ModId,
        /// The version the world holds.
        stored: SavedDataVersion,
        /// The newest version this build writes.
        current: SavedDataVersion,
    },
    /// The declared versions have a gap the world cannot be stepped across.
    NoMigrationPath {
        /// The mod whose data cannot be migrated.
        owner: ModId,
        /// The version the world holds.
        stored: SavedDataVersion,
        /// The first undeclared version at or above `stored`.
        missing: SavedDataVersion,
    },
}

impl SavedDataError {
    /// The mod this failure is attributed to.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        match self {
            Self::NoVersionsDeclared { owner }
            | Self::FutureVersion { owner, .. }
            | Self::NoMigrationPath { owner, .. } => owner,
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
                "the world holds `{owner}` saved data at version {stored}, which is newer than \
                 the version {current} this build writes"
            ),
            Self::NoMigrationPath {
                owner,
                stored,
                missing,
            } => write!(
                formatter,
                "`{owner}` cannot step its saved data forward from version {stored}, because it \
                 declares no version {missing}"
            ),
        }
    }
}

impl std::error::Error for SavedDataError {}

impl StructuredFailure for SavedDataError {
    /// None of these is one of `SPEC.md` §9.2's dependency-graph outcomes: the
    /// graph resolved and every mod is present. What failed is the world's
    /// data, so this reports no kind rather than borrowing one.
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    /// A version is decided before a world opens, which is
    /// [`LifecycleState::Registered`]: the mod set is committed and nothing has
    /// mutated a world yet, so a refusal here still fails closed.
    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Registered
    }

    /// Every message is this crate's own words about a validated [`ModId`] and
    /// integers. Nothing a mod wrote reaches any of them.
    fn quotes_untrusted_text(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZero;

    use ravel_core::{LifecycleState, ModId, StructuredFailure};

    use super::{SavedDataDeclaration, SavedDataError, SavedDataPlan, SavedDataVersion};

    fn owner() -> ModId {
        "example_mod".parse().expect("a legal mod id")
    }

    fn version(raw: u32) -> SavedDataVersion {
        NonZero::new(raw).expect("a non-zero version")
    }

    fn declaring(versions: &[u32]) -> SavedDataDeclaration {
        SavedDataDeclaration::new(owner(), versions.iter().copied().map(version))
            .expect("a non-empty declaration")
    }

    #[test]
    fn a_declaration_writes_its_highest_version_and_reads_all_of_them() {
        let declaration = declaring(&[1, 2, 3]);

        assert_eq!(declaration.owner(), &owner());
        assert_eq!(declaration.current(), version(3));
        assert_eq!(
            declaration.versions().collect::<Vec<_>>(),
            [version(1), version(2), version(3)]
        );
    }

    #[test]
    fn declaring_saved_data_with_no_version_is_refused_rather_than_ignored() {
        let error = SavedDataDeclaration::new(owner(), []).expect_err("no versions");
        assert_eq!(error, SavedDataError::NoVersionsDeclared { owner: owner() });
        assert!(error.to_string().contains("declares no version"));
    }

    #[test]
    fn a_world_already_at_the_current_version_needs_no_steps() {
        let plan = SavedDataPlan::resolve(&declaring(&[1, 2, 3]), version(3))
            .expect("the current version always resolves");

        assert!(plan.is_current());
        assert!(plan.steps().is_empty());
        assert_eq!(plan.owner(), &owner());
    }

    #[test]
    fn an_older_world_is_stepped_forward_one_version_at_a_time() {
        let plan =
            SavedDataPlan::resolve(&declaring(&[1, 2, 3]), version(1)).expect("every step exists");

        // The stored version is where the world sits, so it is not a step.
        assert_eq!(plan.steps(), [version(2), version(3)]);
        assert!(!plan.is_current());
    }

    #[test]
    fn a_world_newer_than_this_build_fails_closed() {
        let error = SavedDataPlan::resolve(&declaring(&[1, 2]), version(5))
            .expect_err("this build has never seen version 5");

        assert_eq!(
            error,
            SavedDataError::FutureVersion {
                owner: owner(),
                stored: version(5),
                current: version(2),
            }
        );
        assert!(error.to_string().contains("newer than"));
        assert_eq!(error.owner(), &owner());
    }

    #[test]
    fn a_gap_in_the_declared_versions_has_no_path_across_it() {
        // Version 2 is undeclared, so nothing steps a version-1 world to 3.
        let error = SavedDataPlan::resolve(&declaring(&[1, 3]), version(1))
            .expect_err("there is no step from 1 to 3");

        assert_eq!(
            error,
            SavedDataError::NoMigrationPath {
                owner: owner(),
                stored: version(1),
                missing: version(2),
            }
        );
        assert!(error.to_string().contains("declares no version 2"));
    }

    #[test]
    fn a_gap_below_the_stored_version_does_not_block_a_world_that_is_past_it() {
        // This build cannot read version 1, but the world already sits at 2.
        let plan = SavedDataPlan::resolve(&declaring(&[2, 3]), version(2))
            .expect("the gap is behind the world");
        assert_eq!(plan.steps(), [version(3)]);
    }

    #[test]
    fn every_refusal_reports_the_phase_it_failed_in_and_no_dependency_outcome() {
        for error in [
            SavedDataError::NoVersionsDeclared { owner: owner() },
            SavedDataError::FutureVersion {
                owner: owner(),
                stored: version(5),
                current: version(2),
            },
            SavedDataError::NoMigrationPath {
                owner: owner(),
                stored: version(1),
                missing: version(2),
            },
        ] {
            assert_eq!(error.failure_kind(), None);
            assert_eq!(error.lifecycle_state(), LifecycleState::Registered);
            assert!(!error.quotes_untrusted_text());
            assert!(error.diagnostic().text().contains("example_mod"));
        }
    }
}
