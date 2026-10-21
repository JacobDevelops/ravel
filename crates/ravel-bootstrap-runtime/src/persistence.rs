//! Identity-keyed world admission and saved-data checkpoints.

use std::fmt;
use std::sync::{Arc, PoisonError};

use ravel_core::ModId;
use ravel_host_api::Point;
use ravel_loader::LoadedMods;
use ravel_save::{SavedDataAggregate, SavedDataError};

use crate::runtime::LoaderRuntime;

pub(crate) mod crossings;

#[derive(Clone, Debug)]
struct Checkpoint {
    identity: WorldIdentity,
    aggregate: SavedDataAggregate,
    dirty: bool,
}

impl Checkpoint {
    fn refresh(&mut self, produced: SavedDataAggregate) {
        self.dirty |= produced != self.aggregate;
        self.aggregate = produced;
    }
}

#[derive(Default)]
pub(crate) struct PersistenceState {
    live: Option<Checkpoint>,
    staged: Option<Checkpoint>,
}

/// What the Java adapter should do after an open crossing.
#[derive(Debug, Eq, PartialEq)]
pub enum OpenOutcome {
    /// Admission and delivery completed; no Java-side replacement is needed yet.
    Staged,
    /// World access is refused with the diagnostic text.
    Refused(String),
}

/// What the Java adapter should do at a save crossing.
#[derive(Debug, Eq, PartialEq)]
pub enum SaveOutcome {
    /// Nothing changed since the last write; the adapter writes nothing.
    Unchanged,
    /// A mod vetoed this write; pending state is retained for the next save.
    Withheld,
    /// The complete aggregate the adapter should persist.
    Replacement(Vec<u8>),
    /// Unredacted failure diagnostic; publish only through `DiagnosticMessage::redacted`.
    Failed(String),
    /// Unredacted identity diagnostic; publish only through `DiagnosticMessage::redacted`.
    IdentityMismatch(String),
}

fn runtime() -> Result<Arc<LoaderRuntime>, String> {
    crate::runtime::selected().ok_or_else(|| "the loader runtime is not installed".to_owned())
}

/// Lexically normalized world identity shared by open and save crossings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorldIdentity(String);

impl WorldIdentity {
    // Avoid filesystem canonicalization: the world may be closing during a save.
    pub(crate) fn of(raw: &str) -> Self {
        let rooted = raw.starts_with('/') || raw.starts_with('\\');
        let mut normalized = String::with_capacity(raw.len());
        if rooted {
            normalized.push('/');
        }
        for segment in raw
            .split(['/', '\\'])
            .filter(|s| !s.is_empty() && *s != ".")
        {
            if !normalized.is_empty() && !normalized.ends_with('/') {
                normalized.push('/');
            }
            normalized.push_str(segment);
        }
        Self(normalized)
    }
}

impl fmt::Display for WorldIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Admits and delivers world data; `fresh_world` only controls missing-data notices.
#[must_use]
pub fn open_world_data(identity_raw: &str, bytes: Option<&[u8]>, fresh_world: bool) -> OpenOutcome {
    match runtime() {
        Ok(runtime) => {
            match runtime.stage_world(WorldIdentity::of(identity_raw), bytes, fresh_world) {
                Ok(()) => OpenOutcome::Staged,
                Err(error) => OpenOutcome::Refused(error.to_string()),
            }
        }
        Err(error) => OpenOutcome::Refused(error),
    }
}

/// Produces the aggregate selected by the save receiver's identity.
#[must_use]
pub(crate) fn save_world_data(identity_raw: &str) -> SaveOutcome {
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return SaveOutcome::Failed(error),
    };
    runtime.save_world(&WorldIdentity::of(identity_raw))
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PersistenceError {
    Lifecycle(ravel_loader::LifecycleError),
    World(ravel_loader::WorldRefused),
    Admission(ravel_loader::SaveRefused),
    Host(ravel_host_api::HostError),
    Interception(ravel_loader::InterceptRefused),
}

impl fmt::Display for PersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lifecycle(error) => error.fmt(formatter),
            Self::World(error) => error.fmt(formatter),
            Self::Admission(error) => error.fmt(formatter),
            Self::Host(error) => error.fmt(formatter),
            Self::Interception(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PersistenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lifecycle(error) => Some(error),
            Self::World(error) => Some(error),
            Self::Admission(error) => Some(error),
            Self::Host(error) => Some(error),
            Self::Interception(error) => Some(error),
        }
    }
}

impl LoaderRuntime {
    pub(crate) fn stage_world(
        &self,
        identity: WorldIdentity,
        bytes: Option<&[u8]>,
        fresh_world: bool,
    ) -> Result<(), PersistenceError> {
        let mut locked = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let inner = &mut *locked;
        // Refuse before reading or writing any world state.
        inner
            .loaded
            .machine()
            .world_access(&inner.report)
            .map_err(PersistenceError::World)?;
        if bytes.is_none() && !fresh_world {
            inner.report.note(&format!(
                "`{identity}` is not a new world and holds no Ravel data; \
                 fresh state was initialized"
            ));
        }
        let aggregate = Self::open_world_data(inner, bytes)?;
        inner
            .loaded
            .record_world_mutation()
            .map_err(PersistenceError::Lifecycle)?;
        inner.persistence.staged = Some(Checkpoint {
            identity,
            aggregate,
            dirty: true,
        });
        drop(locked);
        Ok(())
    }

    fn open_world_data(
        inner: &mut crate::runtime::Inner,
        bytes: Option<&[u8]>,
    ) -> Result<SavedDataAggregate, PersistenceError> {
        let mut loaded = inner.loaded.borrow();
        if let Some(live) = &mut inner.persistence.live {
            Self::refresh_unstaged_live(
                &mut loaded,
                live,
                inner.persistence.staged.is_some(),
                inner.report.environment(),
            )?;
        }
        ravel_loader::open_saved_data(
            &mut loaded,
            bytes,
            &inner.installed_packages,
            inner.report.environment(),
        )
        .map_err(PersistenceError::Admission)
    }

    // A veto withholds only Ravel data and preserves dirty state for the next save.
    fn save_veto(
        loaded: &mut LoadedMods<'_, Box<dyn ravel_host_api::Mod>>,
        interceptions: &ravel_loader::AdmittedPoints,
        environment: &ravel_core::FailureEnvironment,
    ) -> Result<Option<ModId>, PersistenceError> {
        let round =
            ravel_loader::intercept(loaded, interceptions, Point::WorldIsSaved, environment)
                .map_err(PersistenceError::Interception)?;
        if let Some(failure) = round.failure() {
            return Err(PersistenceError::Host(failure.clone()));
        }
        Ok(round.refused_by().cloned())
    }

    fn snapshot(
        loaded: &mut LoadedMods<'_, Box<dyn ravel_host_api::Mod>>,
        prior: Option<&SavedDataAggregate>,
        environment: &ravel_core::FailureEnvironment,
    ) -> Result<SavedDataAggregate, PersistenceError> {
        ravel_loader::produce_saved_data(loaded, prior, environment)
            .map_err(PersistenceError::Admission)
    }

    fn refresh_unstaged_live(
        loaded: &mut LoadedMods<'_, Box<dyn ravel_host_api::Mod>>,
        live: &mut Checkpoint,
        has_stage: bool,
        environment: &ravel_core::FailureEnvironment,
    ) -> Result<(), PersistenceError> {
        if !has_stage {
            live.refresh(Self::snapshot(loaded, Some(&live.aggregate), environment)?);
        }
        Ok(())
    }

    fn save_checkpoint(checkpoint: &mut Checkpoint) -> SaveOutcome {
        if !checkpoint.dirty {
            return SaveOutcome::Unchanged;
        }
        match checkpoint.aggregate.to_bytes() {
            Ok(bytes) => {
                checkpoint.dirty = false;
                SaveOutcome::Replacement(bytes)
            }
            Err(error) => SaveOutcome::Failed(error.to_string()),
        }
    }

    pub(crate) fn save_world(&self, identity: &WorldIdentity) -> SaveOutcome {
        let mut locked = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let inner = &mut *locked;
        let saved = (|| {
            let mut loaded = inner.loaded.borrow();
            if let Some(refused) = Self::save_veto(
                &mut loaded,
                &inner.interceptions,
                inner.report.environment(),
            )? {
                inner.report.note(&format!(
                    "`{refused}` refused Ravel's world write at `{}`",
                    Point::WorldIsSaved
                ));
                return Ok(SaveOutcome::Withheld);
            }
            Self::save_selected(
                &mut loaded,
                &mut inner.persistence,
                identity,
                inner.report.environment(),
            )
        })();
        let outcome = match saved {
            Ok(outcome) => outcome,
            Err(error) => SaveOutcome::Failed(error.to_string()),
        };
        drop(locked);
        outcome
    }

    fn save_selected(
        loaded: &mut LoadedMods<'_, Box<dyn ravel_host_api::Mod>>,
        persistence: &mut PersistenceState,
        identity: &WorldIdentity,
        environment: &ravel_core::FailureEnvironment,
    ) -> Result<SaveOutcome, PersistenceError> {
        // Reopening promotes the new stage before considering its old live checkpoint.
        if let Some(mut promoted) = persistence.staged.take_if(|s| &s.identity == identity) {
            match Self::snapshot(loaded, Some(&promoted.aggregate), environment) {
                Ok(produced) => promoted.aggregate = produced,
                Err(error) => {
                    persistence.staged = Some(promoted);
                    return Err(error);
                }
            }
            let answer = Self::save_checkpoint(&mut promoted);
            if matches!(answer, SaveOutcome::Failed(_)) {
                persistence.staged = Some(promoted);
            } else {
                persistence.live = Some(promoted);
            }
            return Ok(answer);
        }
        if let Some(live) = persistence
            .live
            .as_mut()
            .filter(|live| &live.identity == identity)
        {
            Self::refresh_unstaged_live(loaded, live, persistence.staged.is_some(), environment)?;
            return Ok(Self::save_checkpoint(live));
        }
        Ok(SaveOutcome::IdentityMismatch(format!(
            "save receiver `{identity}` matches neither live nor staged world state"
        )))
    }
}

impl From<SavedDataError> for OpenOutcome {
    fn from(error: SavedDataError) -> Self {
        Self::Refused(error.to_string())
    }
}

#[cfg(test)]
mod tests;
