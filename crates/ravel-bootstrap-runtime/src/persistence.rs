//! Identity-keyed world admission, checkpoints, and data-pack publication.

use std::fmt;
use std::path::Path;
use std::sync::{Arc, PoisonError};

use jni::sys::jint;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StagedPack {
    /// No pack was installed or the write was refused.
    Nothing = 0,
    /// Existing pack contents are unchanged.
    Unchanged = 1,
    /// Pack contents changed and must be reloaded.
    Written = 2,
}

impl From<StagedPack> for jint {
    fn from(pack: StagedPack) -> Self {
        match pack {
            StagedPack::Nothing => 0,
            StagedPack::Unchanged => 1,
            StagedPack::Written => 2,
        }
    }
}

/// What the Java adapter should do at a save crossing.
#[derive(Debug, Eq, PartialEq)]
pub enum SaveOutcome {
    Unchanged,
    /// A mod vetoed this write; pending state is retained for the next save.
    Withheld,
    Replacement(Vec<u8>),
    Failed(String),
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
pub fn stage(identity_raw: &str, bytes: Option<&[u8]>, fresh_world: bool) -> OpenOutcome {
    match runtime() {
        Ok(runtime) => outcome(&runtime, identity_raw, bytes, fresh_world),
        Err(error) => OpenOutcome::Refused(error),
    }
}

fn outcome(
    runtime: &LoaderRuntime,
    identity_raw: &str,
    bytes: Option<&[u8]>,
    fresh_world: bool,
) -> OpenOutcome {
    match runtime.stage_world(WorldIdentity::of(identity_raw), bytes, fresh_world) {
        Ok(()) => {
            runtime.write_world_pack(identity_raw);
            OpenOutcome::Staged
        }
        Err(error) => OpenOutcome::Refused(error.to_string()),
    }
}

/// Produces the aggregate selected by the save receiver's identity.
#[must_use]
pub fn save(identity_raw: &str) -> SaveOutcome {
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return SaveOutcome::Failed(error),
    };
    runtime.save_world(&WorldIdentity::of(identity_raw))
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PersistenceError {
    Lifecycle(ravel_loader::LifecycleError),
    MissingHost(ravel_loader::MissingHost),
    World(ravel_loader::WorldRefused),
    Admission(ravel_loader::SaveRefused),
    Host(ravel_host_api::HostError),
    Interception(ravel_loader::InterceptRefused),
}

impl fmt::Display for PersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lifecycle(error) => error.fmt(formatter),
            Self::MissingHost(error) => error.fmt(formatter),
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
            Self::MissingHost(error) => Some(error),
            Self::World(error) => Some(error),
            Self::Admission(error) => Some(error),
            Self::Host(error) => Some(error),
            Self::Interception(error) => Some(error),
        }
    }
}

impl LoaderRuntime {
    fn write_world_pack(&self, world: &str) {
        self.put_pack(world, crate::packs::write_world_pack);
    }

    fn write_staged_pack(&self, directory: &str) -> StagedPack {
        self.put_pack(directory, crate::packs::write_staged_pack)
    }

    fn put_pack(
        &self,
        directory: &str,
        write: fn(
            &ravel_loader::Supplied,
            &Path,
        ) -> Result<ravel_loader::Written, ravel_loader::GamePackError>,
    ) -> StagedPack {
        let path = Path::new(directory);
        let launch = &self.launch;
        if let Err(refusal) = admissible_world(path, &launch.game_directory) {
            self.inner
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .report
                .note(&format!(
                    "no data pack was written into `{directory}`, so this world reads none of \
                     the mods' recipes or tags: {refusal}"
                ));
            return StagedPack::Nothing;
        }
        match write(&launch.supplied, path) {
            Ok(written) if written.changed() => StagedPack::Written,
            Ok(_) => StagedPack::Unchanged,
            Err(error) => {
                self.inner
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .report
                    .note(&format!(
                        "`{directory}` could not be given the mods' data pack, so this world \
                         reads none of their recipes or tags: {error}"
                    ));
                StagedPack::Nothing
            }
        }
    }

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
            .machine
            .open_world(&inner.report)
            .map_err(PersistenceError::World)?;
        if bytes.is_none() && !fresh_world {
            inner.report.note(&format!(
                "`{identity}` is not a new world and holds no Ravel data; \
                 fresh state was initialized"
            ));
        }
        let aggregate = Self::open_world_data(inner, bytes)?;
        inner
            .machine
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
        let mut loaded = LoadedMods::new(&mut inner.machine, &mut inner.mods)
            .map_err(PersistenceError::MissingHost)?;
        if inner.persistence.staged.is_none()
            && let Some(live) = &mut inner.persistence.live
        {
            live.refresh(
                ravel_loader::produce_saved_data(
                    &mut loaded,
                    &crate::runtime::environment(),
                    Some(&live.aggregate),
                )
                .map_err(PersistenceError::Admission)?,
            );
        }
        ravel_loader::open_saved_data(
            &mut loaded,
            &crate::runtime::environment(),
            bytes,
            &inner.installed_packages,
        )
        .map_err(PersistenceError::Admission)
    }

    // A veto withholds only Ravel data and preserves dirty state for the next save.
    fn save_veto(
        loaded: &mut LoadedMods<'_, Box<dyn ravel_host_api::HostMod>>,
    ) -> Result<Option<ModId>, PersistenceError> {
        let round = ravel_loader::intercept(loaded, Point::WorldIsSaved)
            .map_err(PersistenceError::Interception)?;
        if let Some(failure) = round.failure() {
            return Err(PersistenceError::Host(failure.clone()));
        }
        Ok(round.refused_by().cloned())
    }

    fn record_interception_failure(
        inner: &mut crate::runtime::Inner,
        error: &PersistenceError,
    ) -> Result<(), ravel_loader::LifecycleError> {
        let failure = match error {
            PersistenceError::Host(source)
            | PersistenceError::Interception(ravel_loader::InterceptRefused::Mod {
                source, ..
            }) => source.clone(),
            PersistenceError::Interception(
                refused @ ravel_loader::InterceptRefused::ForeignDeclaration { mod_id, .. },
            ) => ravel_host_api::HostError::Refused {
                mod_id: mod_id.clone(),
                state: inner.machine.state(mod_id)?,
                detail: refused.to_string(),
            },
            PersistenceError::Lifecycle(_)
            | PersistenceError::MissingHost(_)
            | PersistenceError::World(_)
            | PersistenceError::Admission(_)
            | PersistenceError::Interception(_) => return Ok(()),
        };
        inner.record_failure(failure.mod_id(), &failure, None)
    }

    fn snapshot(
        loaded: &mut LoadedMods<'_, Box<dyn ravel_host_api::HostMod>>,
        prior: Option<&SavedDataAggregate>,
    ) -> Result<SavedDataAggregate, PersistenceError> {
        ravel_loader::produce_saved_data(loaded, &crate::runtime::environment(), prior)
            .map_err(PersistenceError::Admission)
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
            let mut loaded = LoadedMods::new(&mut inner.machine, &mut inner.mods)
                .map_err(PersistenceError::MissingHost)?;
            if let Some(refused) = Self::save_veto(&mut loaded)? {
                inner.report.note(&format!(
                    "`{refused}` refused Ravel's world write at `{}`",
                    Point::WorldIsSaved
                ));
                return Ok(SaveOutcome::Withheld);
            }
            Self::save_selected(&mut loaded, &mut inner.persistence, identity)
        })();
        let outcome = match saved {
            Ok(outcome) => outcome,
            Err(error) => match Self::record_interception_failure(inner, &error) {
                Ok(()) => SaveOutcome::Failed(error.to_string()),
                Err(recording) => SaveOutcome::Failed(recording.to_string()),
            },
        };
        drop(locked);
        outcome
    }

    fn save_selected(
        loaded: &mut LoadedMods<'_, Box<dyn ravel_host_api::HostMod>>,
        persistence: &mut PersistenceState,
        identity: &WorldIdentity,
    ) -> Result<SaveOutcome, PersistenceError> {
        // Reopening promotes the new stage before considering its old live checkpoint.
        if let Some(mut promoted) = persistence.staged.take_if(|s| &s.identity == identity) {
            match Self::snapshot(loaded, Some(&promoted.aggregate)) {
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
            if persistence.staged.is_none() {
                live.refresh(Self::snapshot(loaded, Some(&live.aggregate))?);
            }
            return Ok(Self::save_checkpoint(live));
        }
        Ok(SaveOutcome::IdentityMismatch(format!(
            "save receiver `{identity}` matches neither live nor staged world state"
        )))
    }
}

// Reject paths whose tree replacement could overwrite the game directory.
fn admissible_world(world: &Path, game: &Path) -> Result<(), String> {
    if !world.is_absolute() {
        return Err(format!(
            "`{}` is not an absolute path, so it names a directory relative to the game's own",
            world.display()
        ));
    }
    if !world.is_dir() {
        return Err(format!(
            "`{}` is not an existing directory",
            world.display()
        ));
    }
    let world = world
        .canonicalize()
        .map_err(|error| format!("world path cannot be resolved: {error}"))?;
    let game = game
        .canonicalize()
        .map_err(|error| format!("launch directory cannot be resolved: {error}"))?;
    if game.starts_with(&world) {
        return Err(format!(
            "`{}` holds the game directory `{}`, and a pack is written by replacing a tree \
             inside the path it is given",
            world.display(),
            game.display()
        ));
    }
    pack_targets_are_real(&world)
}

// The writer replaces these entries, so a link among them would redirect it outside `world`.
fn pack_targets_are_real(world: &Path) -> Result<(), String> {
    use ravel_loader::{GAME_PACK_DIRECTORY, WORLD_DATA_PACKS};
    let packs = world.join(WORLD_DATA_PACKS);
    let roots = [
        packs.join(GAME_PACK_DIRECTORY),
        world.join(GAME_PACK_DIRECTORY),
    ];
    for path in std::iter::once(packs).chain(
        roots
            .iter()
            .flat_map(|root| ravel_loader::replaced_paths(root)),
    ) {
        if std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(format!(
                "`{}` is a symbolic link, and a pack is written only to real directories \
                 inside the world",
                path.display()
            ));
        }
    }
    Ok(())
}

impl From<SavedDataError> for OpenOutcome {
    fn from(error: SavedDataError) -> Self {
        Self::Refused(error.to_string())
    }
}

#[cfg(test)]
mod tests;
