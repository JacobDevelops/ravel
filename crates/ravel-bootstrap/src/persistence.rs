//! Identity-keyed world admission, checkpoints, and data-pack JNI crossings.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::path::Path;
use std::sync::{Arc, OnceLock, PoisonError};

use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{JNI_FALSE, jboolean, jint};
use jni::{Env, EnvUnowned};
use ravel_core::ModId;
use ravel_host_api::{HostMod, Outcome, Point};
use ravel_save::{
    MAX_AGGREGATE_BYTES, SavedDataAggregate, SavedDataDeclaration, SavedDataError, SavedDataPlan,
};
use semver::Version;

use crate::guard::{HookError, text};
use crate::runtime::LoaderRuntime;

static RUNTIME: OnceLock<Arc<LoaderRuntime>> = OnceLock::new();

#[derive(Clone, Debug)]
struct Checkpoint {
    identity: WorldIdentity,
    aggregate: SavedDataAggregate,
    dirty: bool,
}

// Keep the newest world notices within a fixed session-wide bound.
const OBSERVATION_CAPACITY: usize = 32;

#[derive(Default)]
pub(crate) struct PersistenceState {
    live: Option<Checkpoint>,
    staged: Option<Checkpoint>,
    observations: VecDeque<String>,
    declarations: Option<Vec<SavedDataDeclaration>>,
}

impl PersistenceState {
    /// Records one notice, dropping the oldest once the bound is reached.
    fn notice(&mut self, observation: String) {
        while self.observations.len() >= OBSERVATION_CAPACITY {
            self.observations.pop_front();
        }
        self.observations.push_back(observation);
    }
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

pub(crate) fn install(runtime: Arc<LoaderRuntime>) {
    // Installation is process-wide; the first runtime remains authoritative.
    drop(RUNTIME.set(runtime));
}

fn runtime() -> Result<&'static Arc<LoaderRuntime>, String> {
    RUNTIME
        .get()
        .ok_or_else(|| "the loader runtime is not installed".to_owned())
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
        Err(error) => OpenOutcome::Refused(error),
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

/// Returns the newest admitted-world notices, oldest first, within the session-wide bound.
#[must_use]
pub fn observations() -> Vec<String> {
    runtime().map_or_else(
        |error| vec![error],
        |runtime| runtime.persistence_observations(),
    )
}

impl LoaderRuntime {
    pub(crate) fn persistence_observations(&self) -> Vec<String> {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .persistence
            .observations
            .iter()
            .cloned()
            .collect()
    }

    fn declarations(inner: &mut crate::runtime::Inner) -> Result<(), String> {
        if inner.persistence.declarations.is_some() {
            return Ok(());
        }
        let mut declarations = Vec::with_capacity(inner.mods.len());
        for host in &inner.mods {
            if inner
                .machine
                .state(host.id())
                .map_err(|error| error.to_string())?
                == ravel_core::LifecycleState::Failed
            {
                let versions = inner
                    .machine
                    .saved_data_versions(host.id())
                    .map_err(|error| error.to_string())?;
                if !versions.is_empty() {
                    declarations.push(
                        SavedDataDeclaration::new(host.id().clone(), versions.iter().copied())
                            .map_err(|error| error.to_string())?,
                    );
                }
                continue;
            }
            match host.saved_data_declaration() {
                Ok(Some(declaration)) => {
                    if declaration.owner() != host.id() {
                        let owner = host.id().clone();
                        let refused = ravel_loader::SaveRefused::ForeignDeclaration {
                            mod_id: owner.clone(),
                            owner: declaration.owner().clone(),
                        };
                        let error = ravel_host_api::HostError::Refused {
                            mod_id: owner.clone(),
                            state: inner
                                .machine
                                .state(&owner)
                                .map_err(|error| error.to_string())?,
                            detail: refused.to_string(),
                        };
                        inner
                            .record_failure(&owner, &error)
                            .map_err(|error| error.to_string())?;
                        return Err(error.to_string());
                    }
                    declarations.push(declaration);
                }
                Ok(None) => {}
                Err(error) => {
                    let owner = host.id().clone();
                    inner
                        .record_failure(&owner, &error)
                        .map_err(|error| error.to_string())?;
                    return Err(error.to_string());
                }
            }
        }
        inner.persistence.declarations = Some(declarations);
        Ok(())
    }

    fn produce(
        inner: &mut crate::runtime::Inner,
        prior: Option<&SavedDataAggregate>,
    ) -> Result<SavedDataAggregate, String> {
        inner
            .machine
            .request_save()
            .map_err(|error| error.to_string())?;
        inner
            .machine
            .world_access()
            .map_err(|error| error.to_string())?;
        let mut aggregate = SavedDataAggregate::fresh();
        for host in &mut inner.mods {
            if inner.machine.state(host.id()).expect("an admitted owner")
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
            match host.save_saved_data() {
                Ok(Some(envelope)) => aggregate.replace(envelope),
                Ok(None) => {}
                Err(error) => {
                    let owner = host.id().clone();
                    inner
                        .record_failure(&owner, &error)
                        .map_err(|error| error.to_string())?;
                    return Err(error.to_string());
                }
            }
        }
        aggregate.to_bytes().map_err(|error| error.to_string())?;
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
        write: fn(&Path) -> Result<Option<ravel_loader::Written>, ravel_loader::PackError>,
    ) -> StagedPack {
        let path = Path::new(directory);
        if let Err(refusal) = admissible_world(path) {
            self.inner
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .persistence
                .notice(format!(
                    "no data pack was written into `{directory}`, so this world reads none of \
                     the mods' recipes or tags: {refusal}"
                ));
            return StagedPack::Nothing;
        }
        match write(path) {
            Ok(None) => StagedPack::Nothing,
            Ok(Some(written)) if written.changed() => StagedPack::Written,
            Ok(Some(_)) => StagedPack::Unchanged,
            Err(error) => {
                self.inner
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .persistence
                    .notice(format!(
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
    ) -> Result<(), String> {
        let mut locked = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let inner = &mut *locked;
        // Refuse before reading or writing any world state.
        inner
            .machine
            .open_world(&inner.report)
            .map_err(|refused| refused.to_string())?;
        if bytes.is_none() && !fresh_world {
            inner.persistence.notice(format!(
                "`{identity}` is not a new world and holds no Ravel data; \
                 fresh state was initialized"
            ));
        }
        if inner.persistence.staged.is_none() && inner.persistence.live.is_some() {
            let produced = Self::produce(inner, None)?;
            let live = inner.persistence.live.as_mut().expect("checked above");
            live.dirty = produced != live.aggregate;
            live.aggregate = produced;
        }

        Self::declarations(inner)?;
        let declarations = inner
            .persistence
            .declarations
            .as_deref()
            .expect("collected declarations");
        // Composition supplies no package versions, so declared requirements fail closed.
        let uncheckable: BTreeMap<ModId, Version> = BTreeMap::new();
        let aggregate = SavedDataAggregate::open(bytes, declarations, &uncheckable)
            .map_err(|error| error.to_string())?;
        for declaration in declarations {
            let host = inner
                .mods
                .iter_mut()
                .find(|host| host.id() == declaration.owner())
                .expect("declaration belongs to an admitted host");
            if inner
                .machine
                .state(host.id())
                .map_err(|error| error.to_string())?
                == ravel_core::LifecycleState::Failed
            {
                continue;
            }
            let envelope = aggregate.envelope(declaration.owner());
            let plan = SavedDataPlan::open(
                declaration,
                envelope.map(ravel_save::SavedDataEnvelope::data_version),
            )
            .map_err(|error| error.to_string())?;
            if let Err(error) = host.open_saved_data(envelope, &plan) {
                let owner = host.id().clone();
                inner
                    .record_failure(&owner, &error)
                    .map_err(|error| error.to_string())?;
                return Err(error.to_string());
            }
        }
        inner.persistence.staged = Some(Checkpoint {
            identity,
            aggregate,
            dirty: true,
        });
        drop(locked);
        Ok(())
    }

    // A veto withholds only Ravel data and preserves dirty state for the next save.
    fn may_write(inner: &mut crate::runtime::Inner) -> Result<Option<ModId>, String> {
        let crate::runtime::Inner { machine, mods, .. } = inner;
        let mut borrowed: Vec<&mut dyn HostMod> = mods.iter_mut().map(|host| &mut **host).collect();
        let round = match ravel_loader::intercept(machine, &mut borrowed, Point::WorldIsSaved) {
            Ok(round) => round,
            Err(refused) => {
                if let ravel_loader::InterceptionError::Declaration { mod_id, source } = &refused {
                    inner
                        .record_failure(mod_id, source)
                        .map_err(|error| error.to_string())?;
                }
                return Err(refused.to_string());
            }
        };
        drop(borrowed);
        if let Some(failure) = round.failure() {
            let owner = round
                .asked()
                .last()
                .expect("failure belongs to an asked owner");
            inner
                .record_failure(owner, failure)
                .map_err(|error| error.to_string())?;
            return Err(failure.to_string());
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
        checkpoint.dirty = false;
        match checkpoint.aggregate.to_bytes() {
            Ok(bytes) => SaveOutcome::Replacement(bytes),
            Err(error) => SaveOutcome::Failed(error.to_string()),
        }
    }

    pub(crate) fn save_world(&self, identity: &WorldIdentity) -> SaveOutcome {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        match Self::may_write(&mut inner) {
            Ok(None) => {}
            Ok(Some(refused)) => {
                inner.persistence.notice(format!(
                    "`{refused}` refused Ravel's world write at `{}`",
                    Point::WorldIsSaved
                ));
                return SaveOutcome::Withheld;
            }
            Err(error) => return SaveOutcome::Failed(error),
        }
        // A reopened world must promote its new stage before considering its old live checkpoint.
        if inner
            .persistence
            .staged
            .as_ref()
            .is_some_and(|staged| &staged.identity == identity)
        {
            let mut promoted = inner.persistence.staged.take().expect("matched above");
            match Self::produce(&mut inner, Some(&promoted.aggregate)) {
                Ok(produced) => promoted.aggregate = produced,
                Err(error) => {
                    inner.persistence.staged = Some(promoted);
                    return SaveOutcome::Failed(error);
                }
            }
            promoted.dirty = false;
            let answer = match promoted.aggregate.to_bytes() {
                Ok(bytes) => SaveOutcome::Replacement(bytes),
                Err(error) => SaveOutcome::Failed(error.to_string()),
            };
            inner.persistence.live = Some(promoted);
            return answer;
        }

        if inner
            .persistence
            .live
            .as_ref()
            .is_some_and(|live| &live.identity == identity)
        {
            if inner.persistence.staged.is_none() {
                return match Self::produce(&mut inner, None) {
                    Ok(produced) => {
                        let live = inner.persistence.live.as_mut().expect("matched above");
                        if produced == live.aggregate && !live.dirty {
                            return SaveOutcome::Unchanged;
                        }
                        live.aggregate = produced;
                        live.dirty = false;
                        match live.aggregate.to_bytes() {
                            Ok(bytes) => SaveOutcome::Replacement(bytes),
                            Err(error) => SaveOutcome::Failed(error.to_string()),
                        }
                    }
                    Err(error) => SaveOutcome::Failed(error),
                };
            }
            let answer =
                Self::save_checkpoint(inner.persistence.live.as_mut().expect("matched above"));
            drop(inner);
            return answer;
        }
        drop(inner);

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
                OpenOutcome::Refused(diagnostic) => env.new_string(diagnostic)?,
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
            let answer = match save(&identity) {
                // Java writes nothing for both vetoed and unchanged saves.
                SaveOutcome::Unchanged | SaveOutcome::Withheld => vec![UNCHANGED],
                SaveOutcome::Replacement(bytes) => tagged(REPLACEMENT, &bytes),
                SaveOutcome::Failed(detail) => tagged(FAILED, detail.as_bytes()),
                SaveOutcome::IdentityMismatch(detail) => {
                    tagged(IDENTITY_MISMATCH, detail.as_bytes())
                }
            };
            Ok(env.byte_array_from_slice(&answer)?)
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

// Reject paths whose tree replacement could overwrite the game directory.
fn admissible_world(world: &Path) -> Result<(), String> {
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
    let Ok(game) = crate::game_directory::resolve() else {
        return Ok(());
    };
    if game.starts_with(world) {
        return Err(format!(
            "`{}` holds the game directory `{}`, and a pack is written by replacing a tree \
             inside the path it is given",
            world.display(),
            game.display()
        ));
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
