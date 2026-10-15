//! Identity-keyed world admission, checkpoints, and data-pack JNI crossings.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::sync::{Arc, PoisonError};

use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{JNI_FALSE, jboolean, jint};
use jni::{Env, EnvUnowned};
use ravel_core::ModId;
use ravel_host_api::{HostMod, Outcome, Point};
use ravel_save::{MAX_AGGREGATE_BYTES, SavedDataAggregate, SavedDataError};
use semver::Version;

use crate::guard::{HookError, text};
use crate::runtime::LoaderRuntime;

#[derive(Clone, Debug)]
struct Checkpoint {
    identity: WorldIdentity,
    aggregate: SavedDataAggregate,
    dirty: bool,
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

fn runtime() -> Result<&'static Arc<LoaderRuntime>, String> {
    crate::runtime::installed().ok_or_else(|| "the loader runtime is not installed".to_owned())
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
        Ok(runtime) => outcome(runtime, identity_raw, bytes, fresh_world),
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

impl std::error::Error for PersistenceError {}

impl LoaderRuntime {
    fn produce(
        inner: &mut crate::runtime::Inner,
        prior: Option<&SavedDataAggregate>,
    ) -> Result<SavedDataAggregate, PersistenceError> {
        inner
            .machine
            .save_access()
            .map_err(PersistenceError::Lifecycle)?;
        inner
            .machine
            .world_access()
            .map_err(PersistenceError::Lifecycle)?;
        let mut aggregate = SavedDataAggregate::fresh();
        for host in &mut inner.mods {
            if inner
                .machine
                .state(host.id())
                .map_err(PersistenceError::Lifecycle)?
                == ravel_core::LifecycleState::Failed
            {
                let prior = prior.or_else(|| {
                    inner
                        .persistence
                        .live
                        .as_ref()
                        .map(|checkpoint| &checkpoint.aggregate)
                });
                if let Some(envelope) = prior.and_then(|aggregate| aggregate.envelope(host.id())) {
                    aggregate.replace(envelope.clone());
                }
                continue;
            }
            let state = inner
                .machine
                .state(host.id())
                .map_err(PersistenceError::Lifecycle)?;
            let saved = ravel_loader::callback(host.id(), state, || host.save_saved_data());
            match saved {
                Ok(Some(envelope)) => aggregate.replace(envelope),
                Ok(None) => {}
                Err(error) => {
                    let owner = host.id().clone();
                    inner
                        .record_failure(&owner, &error, None)
                        .map_err(PersistenceError::Lifecycle)?;
                    return Err(PersistenceError::Host(error));
                }
            }
        }
        Ok(aggregate)
    }

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
        ) -> Result<ravel_loader::Written, ravel_loader::PackError>,
    ) -> StagedPack {
        let path = Path::new(directory);
        let Some(launch) = &self.launch else {
            self.inner
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .report
                .note("no data pack was written: the runtime has no launch directory");
            return StagedPack::Nothing;
        };
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
        if inner.persistence.staged.is_none() && inner.persistence.live.is_some() {
            let produced = Self::produce(inner, None)?;
            if let Some(live) = &mut inner.persistence.live {
                live.dirty |= produced != live.aggregate;
                live.aggregate = produced;
            }
        }

        // Composition supplies no package versions, so declared requirements fail closed.
        let uncheckable: BTreeMap<ModId, Version> = BTreeMap::new();
        let opened = {
            let mut borrowed: Vec<&mut dyn HostMod> = inner
                .mods
                .iter_mut()
                .map(|host| -> &mut dyn HostMod { host.as_mut() })
                .collect();
            ravel_loader::open_saved_data(
                &mut inner.machine,
                &crate::runtime::environment(),
                &mut borrowed,
                bytes,
                &uncheckable,
            )
        };
        let aggregate = match opened {
            Ok(aggregate) => aggregate,
            Err(refused) => {
                Self::record_admission_failure(inner, &refused)?;
                return Err(PersistenceError::Admission(refused));
            }
        };
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

    fn record_admission_failure(
        inner: &mut crate::runtime::Inner,
        refused: &ravel_loader::SaveRefused,
    ) -> Result<(), PersistenceError> {
        match refused {
            ravel_loader::SaveRefused::Owner { mod_id, source } => {
                inner
                    .record_failure(mod_id, source, None)
                    .map_err(PersistenceError::Lifecycle)?;
            }
            ravel_loader::SaveRefused::ForeignDeclaration { mod_id, .. }
            | ravel_loader::SaveRefused::Data(
                SavedDataError::MigrationFailed { owner: mod_id, .. }
                | SavedDataError::MissingMigration { owner: mod_id, .. }
                | SavedDataError::NoVersionsDeclared { owner: mod_id },
            ) => {
                let error = ravel_host_api::HostError::Refused {
                    mod_id: mod_id.clone(),
                    state: inner
                        .machine
                        .state(mod_id)
                        .map_err(PersistenceError::Lifecycle)?,
                    detail: refused.to_string(),
                };
                inner
                    .record_failure(mod_id, &error, None)
                    .map_err(PersistenceError::Lifecycle)?;
            }
            ravel_loader::SaveRefused::Data(_) => {}
        }
        Ok(())
    }

    // A veto withholds only Ravel data and preserves dirty state for the next save.
    fn save_veto(inner: &mut crate::runtime::Inner) -> Result<Option<ModId>, PersistenceError> {
        let crate::runtime::Inner { machine, mods, .. } = inner;
        let mut borrowed: Vec<&mut dyn HostMod> = mods.iter_mut().map(|host| &mut **host).collect();
        let round = match ravel_loader::intercept(machine, &mut borrowed, Point::WorldIsSaved) {
            Ok(round) => round,
            Err(refused) => {
                if let ravel_loader::InterceptRefused::Mod { mod_id, source } = &refused {
                    inner
                        .record_failure(mod_id, source, None)
                        .map_err(PersistenceError::Lifecycle)?;
                } else if let ravel_loader::InterceptRefused::ForeignDeclaration {
                    mod_id, ..
                } = &refused
                {
                    let error = ravel_host_api::HostError::Refused {
                        mod_id: mod_id.clone(),
                        state: inner
                            .machine
                            .state(mod_id)
                            .map_err(PersistenceError::Lifecycle)?,
                        detail: refused.to_string(),
                    };
                    inner
                        .record_failure(mod_id, &error, None)
                        .map_err(PersistenceError::Lifecycle)?;
                }
                return Err(PersistenceError::Interception(refused));
            }
        };
        drop(borrowed);
        if let (Some(owner), Some(failure)) = (round.failed_by(), round.failure()) {
            inner
                .record_failure(owner, failure, None)
                .map_err(PersistenceError::Lifecycle)?;
            return Err(PersistenceError::Host(failure.clone()));
        }
        if round.outcome() == Outcome::Proceed {
            return Ok(None);
        }
        Ok(Some(
            round
                .asked()
                .last()
                .cloned()
                .expect("a refusal comes from a mod that was asked"),
        ))
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
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        match Self::save_veto(&mut inner) {
            Ok(None) => {}
            Ok(Some(refused)) => {
                inner.report.note(&format!(
                    "`{refused}` refused Ravel's world write at `{}`",
                    Point::WorldIsSaved
                ));
                return SaveOutcome::Withheld;
            }
            Err(error) => return SaveOutcome::Failed(error.to_string()),
        }
        // Reopening promotes the new stage before considering its old live checkpoint.
        if let Some(mut promoted) = inner
            .persistence
            .staged
            .take_if(|s| &s.identity == identity)
        {
            match Self::produce(&mut inner, Some(&promoted.aggregate)) {
                Ok(produced) => promoted.aggregate = produced,
                Err(error) => {
                    inner.persistence.staged = Some(promoted);
                    return SaveOutcome::Failed(error.to_string());
                }
            }
            let answer = Self::save_checkpoint(&mut promoted);
            if matches!(answer, SaveOutcome::Failed(_)) {
                inner.persistence.staged = Some(promoted);
            } else {
                inner.persistence.live = Some(promoted);
            }
            return answer;
        }
        if inner
            .persistence
            .live
            .as_ref()
            .is_some_and(|live| &live.identity == identity)
        {
            if inner.persistence.staged.is_none() {
                let produced = match Self::produce(&mut inner, None) {
                    Ok(produced) => produced,
                    Err(error) => return SaveOutcome::Failed(error.to_string()),
                };
                if let Some(live) = &mut inner.persistence.live {
                    live.dirty |= produced != live.aggregate;
                    live.aggregate = produced;
                }
            }
            if let Some(live) = &mut inner.persistence.live {
                return Self::save_checkpoint(live);
            }
        }
        SaveOutcome::IdentityMismatch(format!(
            "save receiver `{identity}` matches neither live nor staged world state"
        ))
    }
}

enum Incoming {
    Absent,
    /// Empty arrays are present but unreadable, never fresh state.
    Present(Vec<u8>),
    Oversized(String),
}

// Check the JNI array length before allocating native storage.
fn incoming<'local>(
    env: &Env<'local>,
    aggregate: &JByteArray<'local>,
) -> Result<Incoming, HookError> {
    if aggregate.is_null() {
        return Ok(Incoming::Absent);
    }
    let length = aggregate.len(env)?;
    if length > MAX_AGGREGATE_BYTES {
        return Ok(Incoming::Oversized(
            SavedDataError::AggregateTooLarge {
                limit: MAX_AGGREGATE_BYTES,
            }
            .to_string(),
        ));
    }
    Ok(Incoming::Present(env.convert_byte_array(aggregate)?))
}

/// Returns empty text on admission or a diagnostic on refusal; null also refuses.
///
/// # Panics
/// Panics are translated into a JVM exception by the JNI guard.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelOpenWorldPersistence")]
pub fn ravel_open_world_persistence<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    aggregate: JByteArray<'local>,
    identity: JString<'local>,
    fresh_world: jboolean,
) -> JString<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JString<'local>, HookError> {
            let identity = text(env, &identity)?;
            let outcome = match incoming(env, &aggregate)? {
                Incoming::Absent => stage(&identity, None, fresh_world != JNI_FALSE),
                Incoming::Present(bytes) => {
                    stage(&identity, Some(&bytes), fresh_world != JNI_FALSE)
                }
                Incoming::Oversized(refusal) => OpenOutcome::Refused(refusal),
            };
            Ok(match outcome {
                OpenOutcome::Staged => env.new_string("")?,
                OpenOutcome::Refused(diagnostic) => env.new_string(publishable(&diagnostic))?,
            })
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

const UNCHANGED: u8 = 0x00;
const REPLACEMENT: u8 = 0x01;
const FAILED: u8 = 0x02;
const IDENTITY_MISMATCH: u8 = 0x03;

fn tagged(tag: u8, payload: &[u8]) -> Vec<u8> {
    let mut answer = Vec::with_capacity(payload.len() + 1);
    answer.push(tag);
    answer.extend_from_slice(payload);
    answer
}

fn publishable(diagnostic: &str) -> String {
    ravel_observability::redact(diagnostic).text().to_owned()
}

fn save_answer(outcome: SaveOutcome) -> Vec<u8> {
    match outcome {
        // Java writes nothing for both vetoed and unchanged saves.
        SaveOutcome::Unchanged | SaveOutcome::Withheld => vec![UNCHANGED],
        SaveOutcome::Replacement(bytes) => tagged(REPLACEMENT, &bytes),
        SaveOutcome::Failed(detail) => tagged(FAILED, publishable(&detail).as_bytes()),
        SaveOutcome::IdentityMismatch(detail) => {
            tagged(IDENTITY_MISMATCH, publishable(&detail).as_bytes())
        }
    }
}

/// Returns a tagged save result; null or failure leaves vanilla saving untouched.
///
/// # Panics
/// Panics are translated into a JVM exception by the JNI guard.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelSaveWorldPersistence")]
pub fn ravel_save_world_persistence<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    identity: JString<'local>,
) -> JByteArray<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JByteArray<'local>, HookError> {
            let identity = text(env, &identity)?;
            let answer = save_answer(save(&identity));
            Ok(env.byte_array_from_slice(&answer)?)
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
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
    use ravel_loader::{PACK_DIRECTORY, WORLD_DATA_PACKS};
    // `write_pack` stages beside its root under this name.
    let pending = format!("{PACK_DIRECTORY}.pending");
    let packs = Path::new(WORLD_DATA_PACKS);
    for target in [
        packs.to_path_buf(),
        packs.join(PACK_DIRECTORY),
        packs.join(&pending),
        Path::new(PACK_DIRECTORY).to_path_buf(),
        Path::new(&pending).to_path_buf(),
    ] {
        let path = world.join(&target);
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

/// Writes the world data pack before pack discovery; failures become notices.
///
/// # Panics
/// Panics are translated into a JVM exception by the JNI guard.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelWriteWorldDatapack")]
pub fn ravel_write_world_datapack<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    world: JString<'local>,
) {
    env.with_env(|env: &mut Env<'local>| -> Result<(), HookError> {
        let world = text(env, &world)?;
        if let Ok(runtime) = runtime() {
            runtime.write_world_pack(&world);
        }
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

/// Stages the data pack and returns 0 for absent, 1 for unchanged, or 2 for changed.
///
/// # Panics
/// Panics are translated into a JVM exception by the JNI guard.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelWriteStagedDatapack")]
pub fn ravel_write_staged_datapack<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    directory: JString<'local>,
) -> jint {
    env.with_env(|env: &mut Env<'local>| -> Result<jint, HookError> {
        let directory = text(env, &directory)?;
        let Ok(runtime) = runtime() else {
            return Ok(jint::from(StagedPack::Nothing));
        };
        Ok(jint::from(runtime.write_staged_pack(&directory)))
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Returns the game-facing identifier of Ravel's data pack.
///
/// # Panics
/// Panics are translated into a JVM exception by the JNI guard.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelPackId")]
pub fn ravel_pack_id<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JString<'local>, HookError> {
            Ok(env.new_string(ravel_loader::pack_id())?)
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

impl From<SavedDataError> for OpenOutcome {
    fn from(error: SavedDataError) -> Self {
        Self::Refused(error.to_string())
    }
}

#[cfg(test)]
mod tests;
