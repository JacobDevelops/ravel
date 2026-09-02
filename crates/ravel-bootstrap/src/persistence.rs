//! Identity-keyed world persistence owned by the installed loader runtime.
//!
//! This is also where the game asks whether a world may open at all. The open
//! crossing runs before the host method's own body, so refusing here withholds
//! the world rather than interrupting one, and `SPEC.md` §9.2's fail-closed rule
//! needs some path in the game process to ask the loader — this is that path.
//! [`LifecycleMachine::open_world`] answers it; nothing here decides it.
//!
//! [`LifecycleMachine::open_world`]: ravel_loader::LifecycleMachine::open_world

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::path::Path;
use std::sync::{Arc, OnceLock, PoisonError};

use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{JNI_FALSE, jboolean};
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

/// How many admitted-world notices are kept at once.
///
/// A notice is written per world open and nothing consumes one, so an unbounded
/// list is a leak measured in worlds per session rather than a record. The
/// oldest is dropped rather than the newest because what a player is diagnosing
/// is the world they just opened.
const OBSERVATION_CAPACITY: usize = 32;

#[derive(Default)]
pub(crate) struct PersistenceState {
    live: Option<Checkpoint>,
    staged: Option<Checkpoint>,
    /// What was worth noticing about a world that was still admitted.
    ///
    /// Bounded at [`OBSERVATION_CAPACITY`], because this lives for the whole
    /// run of a game.
    observations: VecDeque<String>,
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

/// What the Java adapter should do at a save crossing.
#[derive(Debug, Eq, PartialEq)]
pub enum SaveOutcome {
    Unchanged,
    /// A mod refused at `world_is_saved`, so Ravel offers nothing this pass.
    ///
    /// Distinct from [`SaveOutcome::Unchanged`], which says Ravel has nothing
    /// new to write. This says it has something and was told not to, and the
    /// difference is what a player reading a report needs.
    Withheld,
    Replacement(Vec<u8>),
    Failed(String),
    IdentityMismatch(String),
}

pub(crate) fn install(runtime: Arc<LoaderRuntime>) {
    let _ = RUNTIME.set(runtime);
}

fn runtime() -> Result<&'static Arc<LoaderRuntime>, String> {
    RUNTIME
        .get()
        .ok_or_else(|| "the loader runtime is not installed".to_owned())
}

/// One world, in the only shape the two crossings compare.
///
/// The game hands Ravel whichever path shape the call site had, and the two
/// seams do not agree: a level path and a world path for one world differ by a
/// trailing separator and a `.` segment. That divergence is why this is a type
/// rather than a convention. A raw path cannot be stored as a checkpoint's
/// identity or handed to a save, so a future crossing that forgets to normalize
/// fails to compile instead of comparing two spellings of one world and
/// answering [`SaveOutcome::IdentityMismatch`] — the mode in which Ravel
/// persists nothing and nothing errors.
///
/// The comparison is the derived one over normalized text, so there is one
/// notion of sameness rather than one per call site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorldIdentity(String);

impl WorldIdentity {
    /// Normalizes whichever path shape a crossing was handed.
    ///
    /// Lexical only. A save happens while the world directory is being torn
    /// down, so touching the filesystem could fail or answer differently than it
    /// did at open, and `canonicalize` is exactly that.
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

/// Replaces the staged world after admitting and delivering its aggregate.
///
/// The world-access gate is asked first, so a set the loader refuses opens no
/// world here even when its stored aggregate is perfectly readable.
///
/// `fresh_world` is the game's advisory marker for a world being created. It
/// never decides admission: an absent aggregate initializes fresh state for any
/// world, exactly as ADG-7 says. It is read only to tell the ordinary case apart
/// from an existing world that used to hold Ravel data and now does not, which
/// is worth noticing and is not worth refusing over.
#[must_use]
pub fn stage(identity_raw: &str, bytes: Option<&[u8]>, fresh_world: bool) -> OpenOutcome {
    match runtime() {
        Ok(runtime) => outcome(runtime, identity_raw, bytes, fresh_world),
        Err(error) => OpenOutcome::Refused(error),
    }
}

/// The same crossing against one runtime rather than the process's.
///
/// Split out so the polarity a refusal carries — a diagnostic rather than
/// admission — is testable without a test claiming the process-wide runtime.
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

/// What the open crossing noticed about worlds it admitted anyway.
///
/// Bounded: the most recent [`OBSERVATION_CAPACITY`] notices, oldest first.
#[must_use]
pub fn observations() -> Vec<String> {
    runtime().map_or_else(
        |error| vec![error],
        |runtime| runtime.persistence_observations(),
    )
}

impl LoaderRuntime {
    /// What the open crossing noticed about worlds it admitted anyway.
    ///
    /// Oldest first, and never longer than [`OBSERVATION_CAPACITY`].
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

    fn declarations(inner: &crate::runtime::Inner) -> Vec<SavedDataDeclaration> {
        inner
            .mods
            .iter()
            .filter_map(|host| host.saved_data_declaration())
            .collect()
    }

    fn produce(inner: &crate::runtime::Inner) -> Result<SavedDataAggregate, String> {
        inner
            .machine
            .request_save()
            .map_err(|error| error.to_string())?;
        let mut aggregate = SavedDataAggregate::fresh();
        for host in &inner.mods {
            if let Some(envelope) = host.save_saved_data().map_err(|error| error.to_string())? {
                aggregate.replace(envelope);
            }
        }
        aggregate.to_bytes().map_err(|error| error.to_string())?;
        Ok(aggregate)
    }

    /// Puts the data half of the installed set into the world being opened.
    ///
    /// After the world is admitted rather than before it: a world Ravel refuses is
    /// a world Ravel does not write into, and the pack would be for a session that
    /// does not happen.
    ///
    /// A refusal here is noticed rather than raised. The world opens either way,
    /// and withholding a world over a data file would cost a player their session
    /// for something the game would have started without. What it must not do is
    /// fail silently, which is what the observation channel is for.
    fn write_world_pack(&self, world: &str) {
        let _ = self.put_pack(world, crate::packs::write_world_pack);
    }

    /// Puts the data half where the create-world screen stages packs.
    ///
    /// The directory is the screen's own rather than a world's, and it is written
    /// into for the reason `packs.rs` records: a world being created is handed the
    /// resources the screen already loaded, so a pack that is not in that load is
    /// selected and unread for the whole first session.
    ///
    /// A refusal costs the pack and nothing else, exactly as above. The screen is
    /// a screen either way.
    ///
    /// Answers whether the screen now has a pack to load, which is what decides
    /// whether it is asked to load again. A launch whose mods supply no data
    /// stages nothing and pays neither the reload nor the pause in front of it.
    fn write_staged_pack(&self, directory: &str) -> bool {
        self.put_pack(directory, crate::packs::write_staged_pack)
    }

    /// Writes one pack into `directory`, noticing whatever refused.
    ///
    /// Answers whether a pack is there afterwards, which a refusal and an absent
    /// set both make false. An unchanged tree is true: the pack is present, and
    /// whether these bytes are the ones already on disk says nothing about
    /// whether the caller has read them.
    fn put_pack(
        &self,
        directory: &str,
        write: fn(&Path) -> Result<Option<ravel_loader::Written>, ravel_loader::PackError>,
    ) -> bool {
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
            return false;
        }
        match write(path) {
            Ok(written) => written.is_some(),
            Err(error) => {
                self.inner
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .persistence
                    .notice(format!(
                        "`{directory}` could not be given the mods' data pack, so this world \
                         reads none of their recipes or tags: {error}"
                    ));
                false
            }
        }
    }

    pub(crate) fn stage_world(
        &self,
        identity: WorldIdentity,
        bytes: Option<&[u8]>,
        fresh_world: bool,
    ) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        // The world-access gate, asked before anything here reads or writes a
        // byte of world state. `SPEC.md` §9.2 fails closed on an invalid
        // required-mod graph and §14 makes a required failure withhold the
        // world, and this crossing is where the game asks for one — so the
        // answer has to be the loader's rather than this module's. Refusing here
        // costs the world and nothing else: the game's own body never runs, so
        // no state a player would lose is touched.
        inner
            .machine
            .open_world(&inner.report)
            .map_err(|refused| refused.to_string())?;
        // Absence initializes fresh state for any world, exactly as ADG-7 says.
        // What the marker separates is the ordinary new world from an existing
        // one that used to hold Ravel data and now does not, which is worth
        // saying and is not worth refusing over.
        if bytes.is_none() && !fresh_world {
            inner.persistence.notice(format!(
                "`{identity}` is not a new world and holds no Ravel data; \
                 fresh state was initialized"
            ));
        }
        if inner.persistence.staged.is_none() && inner.persistence.live.is_some() {
            let produced = Self::produce(&inner)?;
            let live = inner.persistence.live.as_mut().expect("checked above");
            live.dirty = produced != live.aggregate;
            live.aggregate = produced;
        }

        let declarations = Self::declarations(&inner);
        // No installed package set reaches this process. `install` is handed a
        // machine, a mod set and a pre-world report; `LifecycleEntry` and
        // `HostMod` carry an identity and no version; and the composed host
        // builds its mods from source rather than from package manifests. So
        // nothing here can say a package is installed, and every declared
        // requirement refuses as missing, which is the direction `SPEC.md` §14
        // asks for.
        //
        // What that costs is stated rather than hidden: the exact-version
        // comparison `SavedDataError::IncompatiblePackage` reports cannot be
        // reached from this crossing, because reaching it needs a package
        // installed at some version. `ravel-save` keeps the comparison — it is
        // reachable and asserted through that crate's own API and through
        // tests/conformance/tests/saved_data.rs, which supplies an installed set
        // — and the gap is that no launch supplies one. Supplying it means
        // carrying package identity and version through composition into
        // `install`, which is the private-inventory work M3 does not authorize
        // yet; evidence/m3/manifest.json records it as a risk under that name.
        let uncheckable: BTreeMap<ModId, Version> = BTreeMap::new();
        let aggregate = SavedDataAggregate::open(bytes, &declarations, &uncheckable)
            .map_err(|error| error.to_string())?;
        for host in &mut inner.mods {
            let Some(declaration) = host.saved_data_declaration() else {
                continue;
            };
            let envelope = aggregate.envelope(declaration.owner());
            let plan = SavedDataPlan::open(
                &declaration,
                envelope.map(ravel_save::SavedDataEnvelope::data_version),
            )
            .map_err(|error| error.to_string())?;
            host.open_saved_data(envelope, &plan)
                .map_err(|error| error.to_string())?;
        }
        inner.persistence.staged = Some(Checkpoint {
            identity,
            aggregate,
            dirty: true,
        });
        Ok(())
    }

    /// Asks the mods whether Ravel may write its data for this save.
    ///
    /// `Point::WorldIsSaved` is the one published point a pinned hook already
    /// reaches, and this is what makes it a point rather than a name. What a
    /// refusal governs is exactly what this seam does: the hook offers *Ravel's*
    /// aggregate for the saving world, so a mod that refuses withholds that
    /// offer for this pass. It does not stop Minecraft saving the world, and
    /// nothing here pretends it could — the capability the pinned target carries
    /// is the capability a mod gets.
    ///
    /// The aggregate stays dirty when a round refuses, so the next save offers
    /// it again. A refusal that silently discarded a mod's unwritten state would
    /// be data loss dressed as a veto.
    fn may_write(inner: &mut crate::runtime::Inner) -> Result<Option<ModId>, String> {
        let crate::runtime::Inner { machine, mods, .. } = inner;
        let mut borrowed: Vec<&mut dyn HostMod> = mods.iter_mut().map(|host| &mut **host).collect();
        let round = ravel_loader::intercept(machine, &mut borrowed, Point::WorldIsSaved)
            .map_err(|refused| refused.to_string())?;
        if let Some(failure) = round.failure() {
            return Err(failure.to_string());
        }
        if round.outcome() == Outcome::Proceed {
            return Ok(None);
        }
        // The mod that refused is the last one asked, because the round ends
        // there and no later mod is reached.
        Ok(Some(
            round
                .asked()
                .last()
                .cloned()
                .expect("a refusal comes from a mod that was asked"),
        ))
    }

    pub(crate) fn save_world(&self, identity: &WorldIdentity) -> SaveOutcome {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        match Self::may_write(&mut inner) {
            Ok(None) => {}
            // The live aggregate is left as it is, dirty flag included, so the
            // next save asks again and offers whatever was withheld.
            Ok(Some(refused)) => {
                inner.persistence.notice(format!(
                    "`{refused}` refused Ravel's world write at `{}`",
                    Point::WorldIsSaved
                ));
                return SaveOutcome::Withheld;
            }
            Err(error) => return SaveOutcome::Failed(error),
        }
        // The staged checkpoint is asked first, and that order is the whole
        // answer to a world reopened in one session. A stage is the world the
        // game has just delivered to the mods; a live checkpoint of the same
        // name is the one the previous visit left behind. Asking the live one
        // first stranded the stage: the save read a dirty flag nothing had set
        // since the reopen, answered `Unchanged` for the rest of the run, and
        // left the world's file holding whatever the first visit wrote. That is
        // what the first real launch saw on its second and third saves.
        if inner
            .persistence
            .staged
            .as_ref()
            .is_some_and(|staged| &staged.identity == identity)
        {
            let mut promoted = inner.persistence.staged.take().expect("matched above");
            match Self::produce(&inner) {
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
                return match Self::produce(&inner) {
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
            let live = inner.persistence.live.as_mut().expect("matched above");
            if live.dirty {
                live.dirty = false;
                return match live.aggregate.to_bytes() {
                    Ok(bytes) => SaveOutcome::Replacement(bytes),
                    Err(error) => SaveOutcome::Failed(error.to_string()),
                };
            }
            return SaveOutcome::Unchanged;
        }

        SaveOutcome::IdentityMismatch(format!(
            "save receiver `{identity}` matches neither live nor staged world state"
        ))
    }
}

/// The stored aggregate as the JVM handed it over.
enum Incoming {
    /// No array at all, which is a world holding no Ravel data.
    Absent,
    /// An array, whatever its length. Zero-length is present and unreadable
    /// rather than absent, which is what stops a truncated file resetting a
    /// world.
    Present(Vec<u8>),
    /// Longer than Ravel reads, measured before anything was copied.
    Oversized(String),
}

/// Reads the aggregate without ever copying more than Ravel would accept.
///
/// The length is asked for first and on its own. An oversized array is refused
/// on one integer read, so a hostile one never allocates natively, and the
/// ceiling is applied here rather than in Java because ADG-7 reserves every
/// admission decision for Rust.
fn incoming<'local>(
    env: &mut Env<'local>,
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

/// Admits or refuses one world's stored aggregate.
///
/// The answer is empty when the world is admitted and carries the diagnostic
/// when it is refused. Null is neither, and it is not this function's to give:
/// it is what `ThrowRuntimeExAndDefault` leaves behind when the body panicked,
/// and Java refuses on it. Empty-means-admitted rather than null-means-admitted
/// is the whole reason a panic here refuses a world rather than admitting one
/// nothing checked.
///
/// # Panics
///
/// Does not panic. The guard around the body turns a panic into a thrown
/// `RuntimeException` rather than an unwind into the JVM.
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

/// Tag byte at element zero, payload in the remainder.
const UNCHANGED: u8 = 0x00;
const REPLACEMENT: u8 = 0x01;
const FAILED: u8 = 0x02;
const IDENTITY_MISMATCH: u8 = 0x03;

/// One tagged answer.
fn tagged(tag: u8, payload: &[u8]) -> Vec<u8> {
    let mut answer = Vec::with_capacity(payload.len() + 1);
    answer.push(tag);
    answer.extend_from_slice(payload);
    answer
}

/// Offers the aggregate the saving world should now store.
///
/// The opposite failure priority to the open crossing, and deliberately so.
/// Null here is what a panic leaves behind and Java treats it as a failure that
/// writes nothing, so a panic costs one Ravel checkpoint and never touches
/// vanilla saving. Only a replacement is ever written.
///
/// # Panics
///
/// Does not panic, for the same reason as [`ravel_open_world_persistence`].
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
                // A withheld write and an unchanged one are the same
                // instruction to the game — write nothing — and they are kept
                // apart on this side because a report a player reads needs them
                // apart. Adding a tag would be a protocol the Java adapter has
                // no use for.
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

/// Whether one world path is a directory Ravel may replace a pack inside.
///
/// The path arrives as a string the game produced and Java passed through
/// unexamined: `Persistence` builds it with `String.valueOf`, which answers
/// `"null"` for a getter that returned null rather than failing. Writing a pack
/// means removing a directory tree and creating another, and a relative path
/// does that against the working directory, which is the game directory itself.
/// So `"null"` would have made `./null` a Ravel-managed tree, and a path naming
/// an ancestor of the game directory would have put one over the instance.
///
/// Both write seams go through the caller, so this is the one place the rule
/// lives. It refuses rather than repairing: a world path Ravel cannot make sense
/// of is one it has no business guessing the intent of.
///
/// The game directory is only consulted when this process can name one. A launch
/// that cannot is a launch where the first two rules still hold, and refusing
/// every world over it would cost a player their packs for a check that is the
/// weaker of the three.
///
/// # Errors
///
/// Returns why the path is not one a pack may be written into.
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

/// Writes the mods' data pack into a world before the game reads its pack list.
///
/// The seam this crosses from is the call that builds the pack repository over
/// the world's `datapacks` directory, which is the window `packs.rs` records as
/// the ceiling on the open crossing: a pack written there is on disk for the
/// world's next open, so the session that wrote it has none of the mods'
/// recipes, tags or loot tables.
///
/// Writing here does not replace the later write. Both put the same tree from the
/// same installed set, so the second is a whole-tree replace over identical
/// bytes; keeping it means a world admitted through a path this seam never ran on
/// still gets its pack.
///
/// A failure is noticed rather than raised, exactly as the later write's is: this
/// runs inside a call the game makes on its way into a world, and refusing a
/// world over a data file would cost a player their session for something the
/// game would have started without.
///
/// # Panics
///
/// Does not panic, for the same reason as [`ravel_open_world_persistence`].
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

/// Writes the mods' data pack where the create-world screen stages packs.
///
/// The other crossing is handed a world and puts the pack under its `datapacks`
/// directory. This one is handed the screen's own staging directory, whose
/// *contents* the game copies into the world it creates, so the pack directory
/// goes in directly and the world ends up with the same tree.
///
/// It exists because being selected and being loaded are different things.
/// Creating a world runs no data load of its own; it is handed the resources the
/// screen loaded when it opened. So a pack that is only on disk is a pack the
/// world lists and knows nothing about, and the fix is to be in the load the
/// screen is about to run rather than to write earlier.
///
/// A failure is noticed rather than raised, for the reason the other write's is,
/// and here the cost is even smaller: the screen opens either way.
///
/// # Returns
///
/// Whether the screen now has a pack to load. The caller asks the screen to load
/// again only when this is true, so a launch whose mods supply no data pays
/// neither the reload nor the pause the player would see in front of it. False is
/// also what a refusal answers, which is the same choice: a pack that is not
/// there is not worth reloading for.
///
/// # Panics
///
/// Does not panic, for the same reason as [`ravel_open_world_persistence`].
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelWriteStagedDatapack")]
pub fn ravel_write_staged_datapack<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    directory: JString<'local>,
) -> jboolean {
    env.with_env(|env: &mut Env<'local>| -> Result<jboolean, HookError> {
        let directory = text(env, &directory)?;
        let Ok(runtime) = runtime() else {
            return Ok(JNI_FALSE);
        };
        Ok(jboolean::from(runtime.write_staged_pack(&directory)))
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

impl From<SavedDataError> for OpenOutcome {
    fn from(error: SavedDataError) -> Self {
        Self::Refused(error.to_string())
    }
}

#[cfg(test)]
mod tests {

    /// A world path the game did not really give is not a directory to write in.
    ///
    /// `"null"` is the one this is written for: `String.valueOf` answers it for a
    /// getter that returned null, so it arrives looking like an ordinary name and
    /// would have made `./null` a tree Ravel removes and recreates, inside the
    /// game directory.
    #[test]
    fn a_world_path_that_names_no_absolute_directory_is_refused() {
        for named in ["null", "", "world", "./world", "../world"] {
            let refusal = super::admissible_world(std::path::Path::new(named))
                .expect_err("a relative path is not a world directory");
            assert!(
                refusal.contains("not an absolute path"),
                "{named} was refused for the wrong reason: {refusal}"
            );
        }
    }

    /// A writable directory of this test's own, made the way `packs.rs` makes one.
    fn scratch(name: &str) -> std::path::PathBuf {
        let at =
            std::env::temp_dir().join(format!("ravel-world-guard-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        std::fs::create_dir_all(&at).expect("a writable temporary directory");
        at
    }

    /// An absolute path is not enough; it has to be there and be a directory.
    #[test]
    fn a_world_path_that_is_absent_or_is_a_file_is_refused() {
        let directory = scratch("absent");
        let absent = directory.join("no-such-world");
        let refusal = super::admissible_world(&absent)
            .expect_err("a path that is not there is not a world directory");
        assert!(refusal.contains("not an existing directory"), "{refusal}");

        let file = directory.join("world");
        std::fs::write(&file, b"not a directory").expect("the file should be written");
        let refusal = super::admissible_world(&file).expect_err("a file is not a world directory");
        assert!(refusal.contains("not an existing directory"), "{refusal}");
    }

    /// An ordinary world directory is admitted, so the rule refuses a shape and
    /// not every world.
    #[test]
    fn an_existing_absolute_world_directory_is_admitted() {
        let world = scratch("admitted").join("Driven");
        std::fs::create_dir_all(&world).expect("the world should be created");

        super::admissible_world(&world).expect("an ordinary world directory is writable");
    }

    use std::collections::BTreeSet;
    use std::num::NonZero;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    use ravel_core::{LifecycleState, ModId, Side, SideSet};
    use ravel_host_api::{HostError, HostMod, InterceptionDeclaration, Occasion, Outcome, Point};
    use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement};
    use ravel_save::{
        MAX_AGGREGATE_BYTES, SavedDataAggregate, SavedDataDeclaration, SavedDataEnvelope,
        SavedDataPlan, SavedDataVersion,
    };
    use serde_json::json;

    use super::{
        FAILED, IDENTITY_MISMATCH, OBSERVATION_CAPACITY, OpenOutcome, REPLACEMENT, SaveOutcome,
        UNCHANGED, WorldIdentity, tagged,
    };
    use crate::runtime::LoaderRuntime;

    const OWNER: &str = "saved_data";
    const OTHER_OWNER: &str = "other_saved_data";
    const FIRST: &str = "worlds/first";
    const SECOND: &str = "worlds/second";

    fn id(name: &str) -> ModId {
        name.parse().expect("a legal mod id")
    }

    /// What the runtime attributes a delivery failure to, as `record` builds one.
    fn attribution() -> ravel_core::FailureAttribution {
        ravel_core::FailureAttribution {
            mod_id: Some(id(OWNER)),
            version: None,
            dependency_chain: Vec::new(),
            state: LifecycleState::Running,
            kind: None,
            surface: None,
            environment: crate::runtime::pre_world_report().environment().clone(),
            message: ravel_core::DiagnosticMessage::raw("this mod stopped working"),
        }
    }

    fn version(raw: u32) -> SavedDataVersion {
        NonZero::new(raw).expect("a non-zero version")
    }

    /// What a test can see and change inside the mod the runtime owns.
    ///
    /// The runtime owns its mods and `HostMod` is not downcastable, so the
    /// state a save is built from is shared rather than reached for.
    #[derive(Debug, Default)]
    struct Held {
        visits: AtomicU32,
        refuses_production: AtomicBool,
        /// Whether this mod refuses the state a world handed it.
        ///
        /// The counterpart of [`Held::refuses_production`] on the open side. A
        /// mod refuses delivery for reasons only it can see — a payload it
        /// cannot read, or its own open step declining — and a crossing that
        /// swallowed that would initialize the mod fresh and let the next save
        /// write the default over the stored envelope.
        refuses_open: AtomicBool,
        /// The plan the last open handed this mod, which is otherwise unobservable.
        ///
        /// A mod that migrates reads `SavedDataPlan::steps`, so the plan is part
        /// of what the crossing delivers and not a detail of it. Version
        /// refusals happen inside `SavedDataAggregate::open`, so a crossing that
        /// built every plan from `None` would refuse nothing and migrate nothing.
        opened_with: Mutex<Option<SavedDataPlan>>,
    }

    impl Held {
        /// The plan the last open delivered, or a panic saying none arrived.
        fn opened_with(&self) -> SavedDataPlan {
            self.opened_with
                .lock()
                .expect("no test poisons this lock")
                .clone()
                .expect("this mod was never opened")
        }
    }

    struct Owning {
        id: ModId,
        held: Arc<Held>,
    }

    impl HostMod for Owning {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn saved_data_declaration(&self) -> Option<SavedDataDeclaration> {
            Some(
                SavedDataDeclaration::new(self.id.clone(), [version(1), version(2)])
                    .expect("two versions"),
            )
        }

        fn open_saved_data(
            &mut self,
            envelope: Option<&SavedDataEnvelope>,
            plan: &SavedDataPlan,
        ) -> Result<(), HostError> {
            if self.held.refuses_open.load(Ordering::SeqCst) {
                return Err(HostError::Refused {
                    mod_id: self.id.clone(),
                    state: LifecycleState::Registered,
                    detail: "this mod cannot read what the world stored".to_owned(),
                });
            }
            let visits = envelope.map_or(0, |stored| {
                u32::try_from(stored.payload()["visits"].as_u64().unwrap_or(0)).unwrap_or(u32::MAX)
            });
            self.held.visits.store(visits, Ordering::SeqCst);
            *self
                .held
                .opened_with
                .lock()
                .expect("no test poisons this lock") = Some(plan.clone());
            Ok(())
        }

        fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, HostError> {
            if self.held.refuses_production.load(Ordering::SeqCst) {
                return Err(HostError::Refused {
                    mod_id: self.id.clone(),
                    state: LifecycleState::Running,
                    detail: "this mod cannot produce an envelope".to_owned(),
                });
            }
            Ok(Some(SavedDataEnvelope::new(
                self.id.clone(),
                version(2),
                Vec::new(),
                None,
                json!({ "visits": self.held.visits.load(Ordering::SeqCst) }),
            )))
        }
    }

    /// A refusal from a stage that runs long before any lifecycle machine.
    ///
    /// The pre-world half of the gate is the half no machine can see for itself:
    /// a package whose graph had no order never became an entry in any plan.
    #[derive(Debug)]
    struct Unresolvable;

    impl std::fmt::Display for Unresolvable {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("`example_mod` requires `example_library`, which is not installed")
        }
    }

    impl std::error::Error for Unresolvable {}

    impl ravel_core::StructuredFailure for Unresolvable {
        fn failure_kind(&self) -> Option<ravel_core::FailureKind> {
            Some(ravel_core::FailureKind::MissingRequiredDependency)
        }

        fn lifecycle_state(&self) -> LifecycleState {
            LifecycleState::Validated
        }
    }

    /// A runtime holding one running mod that owns saved data.
    fn running() -> (LoaderRuntime, Arc<Held>) {
        running_with(crate::runtime::pre_world_report())
    }

    /// The same runtime, carrying whatever the pipeline before it refused.
    fn running_with(report: ravel_observability::PreWorldReport) -> (LoaderRuntime, Arc<Held>) {
        let (runtime, mut held) = owners_running(&[OWNER], report);
        (runtime, held.remove(0))
    }

    /// A runtime holding two running mods that each own their own saved data.
    ///
    /// One owner cannot express a delivery that crosses owners: a lookup for the
    /// wrong identity misses and degenerates into the absent case, which is
    /// already covered. Two owners with distinct stored state make handing one
    /// mod another's envelope a distinguishable failure.
    fn running_pair() -> (LoaderRuntime, Arc<Held>, Arc<Held>) {
        let (runtime, mut held) =
            owners_running(&[OWNER, OTHER_OWNER], crate::runtime::pre_world_report());
        let second = held.remove(1);
        (runtime, held.remove(0), second)
    }

    /// A running runtime whose mods are exactly `owners`, in that order.
    fn owners_running(
        owners: &[&str],
        report: ravel_observability::PreWorldReport,
    ) -> (LoaderRuntime, Vec<Arc<Held>>) {
        let held: Vec<Arc<Held>> = owners.iter().map(|_| Arc::new(Held::default())).collect();
        let hosts: Vec<Box<dyn HostMod>> = owners
            .iter()
            .zip(&held)
            .map(|(owner, held)| {
                Box::new(Owning {
                    id: id(owner),
                    held: Arc::clone(held),
                }) as Box<dyn HostMod>
            })
            .collect();
        let entries: Vec<LifecycleEntry> = owners
            .iter()
            .map(|owner| LifecycleEntry {
                mod_id: id(owner),
                sides: SideSet::new([Side::Client]).expect("a non-empty side set"),
                requirement: Requirement::Required,
                saved_data_versions: BTreeSet::from([version(1), version(2)]),
            })
            .collect();
        let mut machine = LifecycleMachine::new(Side::Client, entries)
            .expect("independent mods are a legal plan");
        for state in [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
            LifecycleState::Running,
        ] {
            machine.advance_all(state).expect("a legal step");
        }
        (
            LoaderRuntime::profiling(machine, hosts, ravel_observability::Profiler::off(), report),
            held,
        )
    }

    /// The identity of the mod that guards the save, chosen to sort *after* the
    /// owner it must be asked before.
    ///
    /// `world_owner` sorts before `zephyr_guard`, and the guard is declared
    /// `after` nothing while the owner is declared after the guard. So the
    /// asked order is only ever right because the declared edge was read: drop
    /// the edge and `precedence` answers the lexicographic order, which puts the
    /// owner first and the guard's refusal after the write it was meant to stop.
    const GUARD: &str = "zephyr_guard";

    /// A mod that answers at `world_is_saved` and owns no world data.
    struct Guarding {
        id: ModId,
        refusing: Arc<AtomicBool>,
        asked: Arc<AtomicU32>,
    }

    impl HostMod for Guarding {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn interception_declaration(&self) -> Option<InterceptionDeclaration> {
            let mut declared = InterceptionDeclaration::new(self.id.clone());
            declared.answer_at(Point::WorldIsSaved.as_str());
            Some(declared)
        }

        fn intercept(&mut self, occasion: Occasion) -> Result<Outcome, HostError> {
            assert_eq!(occasion.point(), Point::WorldIsSaved);
            self.asked.fetch_add(1, Ordering::SeqCst);
            Ok(if self.refusing.load(Ordering::SeqCst) {
                Outcome::Refuse
            } else {
                Outcome::Proceed
            })
        }
    }

    /// A runtime whose set is one world owner and one mod that guards the save.
    fn guarded_running() -> (LoaderRuntime, Arc<Held>, Arc<AtomicBool>, Arc<AtomicU32>) {
        let held = Arc::new(Held::default());
        let refusing = Arc::new(AtomicBool::new(false));
        let asked = Arc::new(AtomicU32::new(0));
        let hosts: Vec<Box<dyn HostMod>> = vec![
            Box::new(Owning {
                id: id(OWNER),
                held: Arc::clone(&held),
            }),
            Box::new(Guarding {
                id: id(GUARD),
                refusing: Arc::clone(&refusing),
                asked: Arc::clone(&asked),
            }),
        ];
        let entries: Vec<LifecycleEntry> = [OWNER, GUARD]
            .iter()
            .map(|owner| LifecycleEntry {
                mod_id: id(owner),
                sides: SideSet::new([Side::Client]).expect("a non-empty side set"),
                requirement: Requirement::Required,
                saved_data_versions: if *owner == OWNER {
                    BTreeSet::from([version(1), version(2)])
                } else {
                    BTreeSet::new()
                },
            })
            .collect();
        let mut machine = LifecycleMachine::new(Side::Client, entries)
            .expect("independent mods are a legal plan");
        for state in [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
            LifecycleState::Running,
        ] {
            machine.advance_all(state).expect("a legal step");
        }
        (
            LoaderRuntime::profiling(
                machine,
                hosts,
                ravel_observability::Profiler::off(),
                crate::runtime::pre_world_report(),
            ),
            held,
            refusing,
            asked,
        )
    }

    /// The carried half: a mod's answer decides whether Ravel writes.
    ///
    /// Both directions in one test, against one runtime, because the claim is
    /// that the *answer* decides: the same set that withholds a save produces
    /// one the moment the mod stops refusing. A test that only refused would
    /// pass against a seam that never wrote at all.
    #[test]
    fn a_mod_refusing_at_world_is_saved_withholds_ravels_write_and_a_later_save_offers_it() {
        let (runtime, held, refusing, asked) = guarded_running();
        runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("a fresh world is staged");
        // After staging, because opening a fresh world is what hands the mod its
        // starting state.
        held.visits.store(7, Ordering::SeqCst);

        refusing.store(true, Ordering::SeqCst);
        let withheld = runtime.save_world(&WorldIdentity::of(FIRST));
        assert_eq!(withheld, SaveOutcome::Withheld);
        assert_eq!(asked.load(Ordering::SeqCst), 1, "the round was not asked");
        assert!(
            runtime
                .persistence_observations()
                .iter()
                .any(|notice| notice.contains(GUARD) && notice.contains("world_is_saved")),
            "a withheld write is invisible to the player: {:?}",
            runtime.persistence_observations()
        );

        // The same runtime, the same unwritten state, with the mod no longer
        // refusing. What was withheld is offered rather than lost.
        refusing.store(false, Ordering::SeqCst);
        let bytes = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
        assert_eq!(asked.load(Ordering::SeqCst), 2);
        assert_eq!(visits(&bytes), 7);
    }

    /// A mod that fails while answering fails the save rather than writing.
    #[test]
    fn a_mod_that_fails_at_world_is_saved_fails_the_save_rather_than_writing() {
        struct Failing {
            id: ModId,
        }

        impl HostMod for Failing {
            fn id(&self) -> &ModId {
                &self.id
            }

            fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
                Ok(())
            }

            fn interception_declaration(&self) -> Option<InterceptionDeclaration> {
                let mut declared = InterceptionDeclaration::new(self.id.clone());
                declared.answer_at(Point::WorldIsSaved.as_str());
                Some(declared)
            }

            fn intercept(&mut self, _occasion: Occasion) -> Result<Outcome, HostError> {
                Err(HostError::Refused {
                    mod_id: self.id.clone(),
                    state: LifecycleState::Running,
                    detail: "the guard broke".to_owned(),
                })
            }
        }

        let hosts: Vec<Box<dyn HostMod>> = vec![Box::new(Failing { id: id(GUARD) })];
        let entries = vec![LifecycleEntry {
            mod_id: id(GUARD),
            sides: SideSet::new([Side::Client]).expect("a non-empty side set"),
            requirement: Requirement::Required,
            saved_data_versions: BTreeSet::new(),
        }];
        let mut machine = LifecycleMachine::new(Side::Client, entries).expect("a legal plan");
        for state in [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
            LifecycleState::Running,
        ] {
            machine.advance_all(state).expect("a legal step");
        }
        let runtime = LoaderRuntime::profiling(
            machine,
            hosts,
            ravel_observability::Profiler::off(),
            crate::runtime::pre_world_report(),
        );
        runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("a fresh world is staged");

        let outcome = runtime.save_world(&WorldIdentity::of(FIRST));
        let SaveOutcome::Failed(detail) = &outcome else {
            panic!("a mod that failed while answering should fail the save: {outcome:?}");
        };
        assert!(detail.contains(GUARD), "{detail}");
        assert!(detail.contains("the guard broke"), "{detail}");
    }

    /// The bytes a save produced, or a panic naming what it did instead.
    fn replacement(outcome: SaveOutcome) -> Vec<u8> {
        match outcome {
            SaveOutcome::Replacement(bytes) => bytes,
            other => panic!("this save should have replaced the aggregate: {other:?}"),
        }
    }

    /// What `bytes` records for [`OWNER`], through the same admission a world uses.
    fn visits(bytes: &[u8]) -> u64 {
        visits_for(bytes, &[OWNER], OWNER)
    }

    /// The same, for one `owner` of an aggregate whose owners are `owners`.
    fn visits_for(bytes: &[u8], owners: &[&str], owner: &str) -> u64 {
        let declarations: Vec<SavedDataDeclaration> = owners
            .iter()
            .map(|name| {
                SavedDataDeclaration::new(id(name), [version(1), version(2)]).expect("two versions")
            })
            .collect();
        SavedDataAggregate::open(
            Some(bytes),
            &declarations,
            &std::collections::BTreeMap::new(),
        )
        .expect("a produced aggregate is admissible")
        .envelope(&id(owner))
        .expect("the owner is in the aggregate")
        .payload()["visits"]
            .as_u64()
            .expect("the payload records a visit count")
    }

    /// The bytes of a world whose one envelope records `count` visits.
    fn stored_visits(count: u32) -> Vec<u8> {
        stored_at(version(2), count)
    }

    /// The same, written by a build whose current version was `at`.
    fn stored_at(at: SavedDataVersion, count: u32) -> Vec<u8> {
        stored_owning(&[(OWNER, at, count)])
    }

    /// The bytes of a world holding one envelope per entry.
    fn stored_owning(entries: &[(&str, SavedDataVersion, u32)]) -> Vec<u8> {
        let mut aggregate = SavedDataAggregate::fresh();
        for (owner, at, count) in entries {
            aggregate.replace(SavedDataEnvelope::new(
                id(owner),
                *at,
                Vec::new(),
                None,
                json!({ "visits": count }),
            ));
        }
        aggregate.to_bytes().expect("bounded")
    }

    /// The delivery half of the crossing, which nothing else here drives.
    ///
    /// Every other staging test that succeeds passes absent bytes, and the ones
    /// that pass bytes assert a refusal, so a crossing that delivered `None` to
    /// every mod — telling each one its world holds nothing, on every open —
    /// would satisfy all of them. This asserts the mod received what the world
    /// stored.
    #[test]
    fn a_stored_aggregate_reaches_the_mod_that_owns_it() {
        let (runtime, held) = running();

        runtime
            .stage_world(WorldIdentity::of(FIRST), Some(&stored_visits(5)), false)
            .expect("a readable aggregate opens its world");

        assert_eq!(
            held.visits.load(Ordering::SeqCst),
            5,
            "the world's stored state never reached the mod that owns it"
        );
    }

    /// The plan half of the delivery, which the envelope cannot stand in for.
    ///
    /// A mod migrates by reading `SavedDataPlan::steps`, so an open that hands
    /// over the right envelope with a plan built from `None` tells a mod its data
    /// is already current. Nothing refuses that: the version checks all live in
    /// `SavedDataAggregate::open`, which has already run by then. So an old
    /// world's data would arrive unmigrated and be written back as if it were
    /// current, which is the data-loss shape this asserts against.
    #[test]
    fn a_world_written_at_an_older_version_hands_the_mod_the_steps_to_reach_current() {
        let (runtime, held) = running();

        runtime
            .stage_world(
                WorldIdentity::of(FIRST),
                Some(&stored_at(version(1), 5)),
                false,
            )
            .expect("a version this build declares a migration for opens its world");

        let plan = held.opened_with();
        assert_eq!(plan.owner(), &id(OWNER));
        assert_eq!(
            plan.steps(),
            [version(2)],
            "the mod was told a world written at version 1 needed no migration"
        );
        assert!(!plan.is_current());

        // The other polarity: a world already at the current version really does
        // get an empty plan, so the assertion above is the stored version's
        // doing and not something every plan satisfies.
        let (current, current_held) = running();
        current
            .stage_world(
                WorldIdentity::of(FIRST),
                Some(&stored_at(version(2), 5)),
                false,
            )
            .expect("a current world opens");
        assert!(current_held.opened_with().is_current());
    }

    /// Two owners in one world, so a crossed delivery is expressible.
    ///
    /// With a single saved-data owner, handing a mod the wrong envelope is a
    /// lookup that misses, and a miss is the absent case some other test already
    /// covers. Two owners with different stored state make the wrong envelope
    /// arrive as another mod's payload, which is the failure this names.
    #[test]
    fn each_owner_in_one_world_receives_its_own_envelope_and_no_other() {
        let owners = [OWNER, OTHER_OWNER];
        let stored = stored_owning(&[(OWNER, version(2), 5), (OTHER_OWNER, version(1), 9)]);

        let (runtime, first, second) = running_pair();
        runtime
            .stage_world(WorldIdentity::of(FIRST), Some(&stored), false)
            .expect("a world holding both owners' data opens");

        assert_eq!(
            first.visits.load(Ordering::SeqCst),
            5,
            "`{OWNER}` did not receive its own state"
        );
        assert_eq!(
            second.visits.load(Ordering::SeqCst),
            9,
            "`{OTHER_OWNER}` did not receive its own state"
        );
        // The plan is addressed the same way, and the two owners stored
        // different versions, so a crossed plan is visible in its steps too.
        assert_eq!(first.opened_with().owner(), &id(OWNER));
        assert_eq!(second.opened_with().owner(), &id(OTHER_OWNER));
        assert!(first.opened_with().is_current());
        assert_eq!(second.opened_with().steps(), [version(2)]);

        // And the save side keeps them apart, so the two envelopes are not one
        // value that happened to be read twice.
        let written = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
        assert_eq!(visits_for(&written, &owners, OWNER), 5);
        assert_eq!(visits_for(&written, &owners, OTHER_OWNER), 9);
    }

    /// Open, save, open again, through the production pair both times.
    ///
    /// The second stage is handed nothing but the bytes the first world's save
    /// emitted, so a save that wrote a mod's state and an open that discarded it
    /// cannot both pass: the round trip is what binds the two crossings to one
    /// value.
    #[test]
    fn a_world_reopened_from_what_its_save_emitted_holds_the_same_state() {
        let (first, held) = running();
        first
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("a new world stages");
        held.visits.store(11, Ordering::SeqCst);
        let stored = replacement(first.save_world(&WorldIdentity::of(FIRST)));

        // A later session: the same world, opened from what its save wrote.
        let (second, reopened) = running();
        second
            .stage_world(WorldIdentity::of(FIRST), Some(&stored), false)
            .expect("the bytes a save emitted open the world they came from");

        assert_eq!(
            reopened.visits.load(Ordering::SeqCst),
            11,
            "the round trip lost the state the save emitted"
        );
        // And the reopened world writes it back rather than resetting it.
        assert_eq!(
            visits(&replacement(second.save_world(&WorldIdentity::of(FIRST)))),
            11
        );
    }

    /// The same world opened twice in one session still writes what it holds.
    ///
    /// The reopen test above starts a second runtime, which is a second launch:
    /// nothing is live when its stage lands, so the promotion path is reached
    /// whatever order the two checkpoints are asked in. A player who quits to
    /// the title screen and plays the same world again does not restart the
    /// process, and that is the sequence the first real launch ran. With the
    /// live checkpoint asked first, the reopened world's stage was never
    /// promoted, every save after the reopen answered `Unchanged`, and the state
    /// the second visit accumulated reached no file.
    #[test]
    fn a_world_reopened_in_one_session_writes_what_the_second_visit_held() {
        let (runtime, held) = running();
        runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("a new world stages");
        held.visits.store(11, Ordering::SeqCst);
        let first_visit = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
        assert_eq!(visits(&first_visit), 11);

        // Quit to the title screen and open the same world again, in the process
        // that is still running, from the bytes the save above emitted.
        runtime
            .stage_world(WorldIdentity::of(FIRST), Some(&first_visit), false)
            .expect("the world its own save wrote reopens");
        assert_eq!(held.visits.load(Ordering::SeqCst), 11);
        held.visits.store(12, Ordering::SeqCst);

        let second_visit = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
        assert_eq!(
            visits(&second_visit),
            12,
            "the second visit's state never reached the world's file"
        );

        // And the session keeps answering for the world after the promotion,
        // rather than the reopen having been a one-off.
        assert_eq!(
            runtime.save_world(&WorldIdentity::of(FIRST)),
            SaveOutcome::Unchanged
        );
        held.visits.store(13, Ordering::SeqCst);
        assert_eq!(
            visits(&replacement(runtime.save_world(&WorldIdentity::of(FIRST)))),
            13
        );
    }

    /// A runtime holding the packaged world-owning mod a launch composes.
    ///
    /// Every other test here drives [`Owning`], which is a fixture written to be
    /// steered: a test sets its visit count and the crossing carries it. That
    /// proves the crossing and says nothing about whether the mod a launch
    /// actually ships ever has state to carry. This one holds the shipped mod
    /// and steers nothing, so what reaches a world's file is what a running game
    /// would put there.
    fn shipped_running() -> LoaderRuntime {
        let shipped = ravel_test_mod_saved_data::SavedDataMod::new();
        let entries = vec![LifecycleEntry {
            mod_id: shipped.id().clone(),
            sides: SideSet::new([Side::Client]).expect("a non-empty side set"),
            requirement: Requirement::Required,
            saved_data_versions: shipped.declaration().versions().collect(),
        }];
        let mut machine =
            LifecycleMachine::new(Side::Client, entries).expect("one mod is a legal plan");
        for state in [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
            LifecycleState::Running,
        ] {
            machine.advance_all(state).expect("a legal step");
        }
        LoaderRuntime::profiling(
            machine,
            vec![Box::new(shipped)],
            ravel_observability::Profiler::off(),
            crate::runtime::pre_world_report(),
        )
    }

    /// A launch that staged nothing says so, and the screen is not reloaded.
    ///
    /// The answer is the whole point of the crossing returning one. Asking the
    /// create-world screen to load again costs a data load and puts the game's
    /// loading screen in front of the player, and a launch whose mods supply no
    /// data half has nothing for that load to find. Nobody installed a set here,
    /// which is the state every launch is in before a host installs one.
    #[test]
    fn a_launch_that_stages_no_pack_does_not_ask_for_a_reload() {
        let staging = std::env::temp_dir().join(format!("ravel-unstaged-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(&staging).expect("a writable temporary directory");

        let staged = shipped_running().write_staged_pack(&staging.to_string_lossy());

        assert!(!staged);
        assert!(!staging.join("ravel").exists());
        let _ = std::fs::remove_dir_all(staging);
    }

    /// What `bytes` records for the shipped owner, through its own declaration.
    fn shipped_visits(bytes: &[u8]) -> u64 {
        let declaration = ravel_test_mod_saved_data::SavedDataMod::new().declaration();
        let owner = declaration.owner().clone();
        SavedDataAggregate::open(
            Some(bytes),
            std::slice::from_ref(&declaration),
            &std::collections::BTreeMap::new(),
        )
        .expect("a produced aggregate is admissible")
        .envelope(&owner)
        .expect("the shipped owner is in the aggregate")
        .payload()["visits"]
            .as_u64()
            .expect("the payload records a visit count")
    }

    /// The shipped owner stores a different world after a second visit.
    ///
    /// Three failures produce one symptom here, and the launch that found it
    /// could tell them apart from neither the log nor the file: a reopen that
    /// hands the mods nothing, a mod whose state never changes, and a save that
    /// promotes a stage of unchanged content all write a payload identical to
    /// the last one. This separates them. A reopen delivering nothing counts the
    /// second visit from zero and answers one; a mod that never counts answers
    /// zero twice and writes the same bytes; only a reopen that restores what
    /// the world stored, to a mod that has something to store, answers two.
    #[test]
    fn the_shipped_world_owner_stores_a_different_world_after_a_second_visit() {
        let runtime = shipped_running();
        runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("a new world stages");
        let first = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
        assert_eq!(
            shipped_visits(&first),
            1,
            "opening a world left the shipped owner with nothing to store"
        );

        runtime
            .stage_world(WorldIdentity::of(FIRST), Some(&first), false)
            .expect("the world its own save wrote reopens");
        let second = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));

        assert_eq!(
            shipped_visits(&second),
            2,
            "the reopen did not hand the shipped owner back what the world stored"
        );
        assert_ne!(
            first, second,
            "two sessions of one world stored byte-identical payloads"
        );
    }

    /// A mod refusing delivery withholds the world instead of opening it fresh.
    ///
    /// This is the failure with the quietest wrong answer. The mod is the only
    /// thing that reads its own payload, so a refusal from it is the only signal
    /// that a world's stored state could not be understood. Dropping it opens the
    /// world with the mod holding its default state, and the next save asks the
    /// mod for an envelope and writes that default over what the world stored —
    /// state loss on the reload path, with nothing reported anywhere. `SPEC.md`
    /// §14 forbids exactly that: a failure must withhold the world rather than
    /// continue and save partial state.
    #[test]
    fn a_mod_refusing_its_stored_state_withholds_the_world_and_stages_nothing() {
        // The same mod and the same bytes open a world while delivery is
        // accepted, so the refusal below is the mod's doing and not the
        // fixture's.
        let (accepting, _held) = running();
        accepting
            .stage_world(WorldIdentity::of(FIRST), Some(&stored_visits(5)), false)
            .expect("a mod that accepts delivery opens its world");

        let (runtime, held) = running();
        held.refuses_open.store(true, Ordering::SeqCst);

        let refused = runtime
            .stage_world(WorldIdentity::of(FIRST), Some(&stored_visits(5)), false)
            .expect_err("a mod that cannot read its stored state opens no world");
        assert!(refused.contains(OWNER), "{refused}");
        assert!(
            refused.contains("cannot read what the world stored"),
            "{refused}"
        );

        // `staged` was left unset, so no checkpoint exists for a save to promote
        // and the stored aggregate is never replaced by the mod's default.
        assert!(
            matches!(
                runtime.save_world(&WorldIdentity::of(FIRST)),
                SaveOutcome::IdentityMismatch(_)
            ),
            "a world the mod refused was staged anyway"
        );
    }

    #[test]
    fn an_absent_aggregate_stages_fresh_state_and_the_first_save_promotes_it() {
        let (runtime, held) = running();

        assert_eq!(
            runtime.stage_world(WorldIdentity::of(FIRST), None, true),
            Ok(())
        );
        // Staged is not live: nothing has bound this world to save emission yet.
        held.visits.store(4, Ordering::SeqCst);

        assert_eq!(
            visits(&replacement(runtime.save_world(&WorldIdentity::of(FIRST)))),
            4
        );
        // Promoted, so the same identity now matches live rather than staged.
        assert_eq!(
            runtime.save_world(&WorldIdentity::of(FIRST)),
            SaveOutcome::Unchanged
        );
    }

    #[test]
    fn an_unmatched_save_receiver_never_replaces_the_aggregate() {
        let (runtime, _held) = running();
        runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("absence stages");

        let outcome = runtime.save_world(&WorldIdentity::of(SECOND));

        assert!(
            matches!(outcome, SaveOutcome::IdentityMismatch(_)),
            "a save from a world Ravel never opened wrote to it anyway: {outcome:?}"
        );
        // The unmatched save must not have promoted the stage either.
        assert_eq!(
            visits(&replacement(runtime.save_world(&WorldIdentity::of(FIRST)))),
            0
        );
    }

    #[test]
    fn an_idle_autosave_leaves_the_stored_aggregate_alone() {
        let (runtime, held) = running();
        runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("absence stages");
        replacement(runtime.save_world(&WorldIdentity::of(FIRST)));

        assert_eq!(
            runtime.save_world(&WorldIdentity::of(FIRST)),
            SaveOutcome::Unchanged
        );

        held.visits.store(1, Ordering::SeqCst);
        assert_eq!(
            visits(&replacement(runtime.save_world(&WorldIdentity::of(FIRST)))),
            1
        );
    }

    #[test]
    fn a_production_failure_preserves_the_previous_aggregate() {
        let (runtime, held) = running();
        runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("absence stages");
        held.visits.store(7, Ordering::SeqCst);
        let durable = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));

        held.visits.store(9, Ordering::SeqCst);
        held.refuses_production.store(true, Ordering::SeqCst);
        let outcome = runtime.save_world(&WorldIdentity::of(FIRST));
        assert!(
            matches!(outcome, SaveOutcome::Failed(_)),
            "a refused envelope was reported as something else: {outcome:?}"
        );

        // The world Java still holds is the one that was written last.
        assert_eq!(visits(&durable), 7);
        held.refuses_production.store(false, Ordering::SeqCst);
        assert_eq!(
            visits(&replacement(runtime.save_world(&WorldIdentity::of(FIRST)))),
            9
        );
    }

    #[test]
    fn the_prior_world_saves_the_state_it_held_when_the_next_world_was_staged() {
        let (runtime, held) = running();
        runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("absence stages");
        held.visits.store(3, Ordering::SeqCst);
        replacement(runtime.save_world(&WorldIdentity::of(FIRST)));

        // The prologue runs before the original body stops the prior server, so
        // the incoming world's state reaches the mods first and the prior
        // world's teardown save happens after it.
        held.visits.store(5, Ordering::SeqCst);
        runtime
            .stage_world(WorldIdentity::of(SECOND), None, true)
            .expect("the second world stages");
        assert_eq!(
            held.visits.load(Ordering::SeqCst),
            0,
            "staging the second world left the first world's state in the mod"
        );

        assert_eq!(
            visits(&replacement(runtime.save_world(&WorldIdentity::of(FIRST)))),
            5,
            "the prior world's teardown save wrote the incoming world's state"
        );
        assert_eq!(
            visits(&replacement(runtime.save_world(&WorldIdentity::of(SECOND)))),
            0
        );
    }

    #[test]
    fn each_open_replaces_the_prior_stage() {
        let (runtime, _held) = running();
        runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("absence stages");
        runtime
            .stage_world(WorldIdentity::of(SECOND), None, true)
            .expect("the second world stages");

        let outcome = runtime.save_world(&WorldIdentity::of(FIRST));
        assert!(
            matches!(outcome, SaveOutcome::IdentityMismatch(_)),
            "a stage the next open replaced still answered a save: {outcome:?}"
        );
        replacement(runtime.save_world(&WorldIdentity::of(SECOND)));
    }

    #[test]
    fn an_oversized_aggregate_is_refused_before_it_is_parsed() {
        let (runtime, _held) = running();
        // Not JSON, so anything that parsed before measuring would fail differently.
        let oversized = vec![b'\0'; MAX_AGGREGATE_BYTES + 1];

        let refused = runtime
            .stage_world(WorldIdentity::of(FIRST), Some(&oversized), true)
            .expect_err("an aggregate over the ceiling does not open a world");

        assert!(refused.contains("limit"), "{refused}");
        assert_eq!(
            runtime.save_world(&WorldIdentity::of(FIRST)),
            SaveOutcome::IdentityMismatch(
                "save receiver `worlds/first` matches neither live nor staged world state"
                    .to_owned()
            )
        );
    }

    #[test]
    fn a_world_this_build_cannot_read_refuses_rather_than_staging() {
        let mut future = SavedDataAggregate::fresh();
        future.replace(SavedDataEnvelope::new(
            id(OWNER),
            version(9),
            Vec::new(),
            None,
            json!({ "visits": 1 }),
        ));
        let mut foreign = SavedDataAggregate::fresh();
        foreign.replace(SavedDataEnvelope::new(
            id("uninstalled"),
            version(1),
            Vec::new(),
            None,
            json!(null),
        ));

        for (name, bytes) in [
            ("future", future.to_bytes().expect("bounded")),
            ("foreign", foreign.to_bytes().expect("bounded")),
            ("torn", b"{\"formatVersion\":".to_vec()),
        ] {
            let (runtime, _held) = running();
            let refused = runtime
                .stage_world(WorldIdentity::of(FIRST), Some(&bytes), true)
                .expect_err("this aggregate does not open a world");
            assert!(!refused.is_empty(), "`{name}` refused without saying why");
            assert!(
                matches!(
                    runtime.save_world(&WorldIdentity::of(FIRST)),
                    SaveOutcome::IdentityMismatch(_)
                ),
                "`{name}` was refused and staged anyway"
            );
            // The route a saved-data refusal actually travels: the diagnostic
            // becomes `OpenOutcome::Refused` for Java to clean up and present. It
            // never enters the pre-world report, which is built before any world
            // is selected and would refuse every later world if it held this.
            assert_eq!(
                super::outcome(&runtime, FIRST, Some(&bytes), true),
                OpenOutcome::Refused(refused),
                "`{name}` reached Java as something other than a refusal"
            );
        }
    }

    /// The two shapes one world reaches Ravel as.
    ///
    /// The open seam is handed the level path and the save seam the world path,
    /// and for the same world they differ by a `.` segment and a trailing
    /// separator. This is the load-bearing test of the pair: if these two ever
    /// stop agreeing, every save answers `IdentityMismatch`, Ravel persists
    /// nothing, and nothing anywhere errors — the failure is invisible.
    #[test]
    fn the_two_shapes_one_world_arrives_as_are_the_same_world() {
        let world = WorldIdentity::of("saves/world");
        assert_eq!(WorldIdentity::of("saves/world/."), world);
        assert_eq!(WorldIdentity::of("saves/world/"), world);
        assert_eq!(WorldIdentity::of("saves//world//."), world);
        assert_eq!(
            WorldIdentity::of("/saves/world/."),
            WorldIdentity::of("/saves/world")
        );
        assert_eq!(
            WorldIdentity::of("/saves/world/.").to_string(),
            "/saves/world",
            "a rooted path stays rooted, so two worlds under different roots stay apart"
        );
    }

    /// The divergence, driven end to end rather than asserted about a function.
    ///
    /// The type is what makes this the only shape either crossing can hold, and
    /// this is what the type buys: the level path the open seam is handed and the
    /// world path the save seam is handed select one checkpoint. Without the
    /// normalization the save answers `IdentityMismatch`, writes nothing, and
    /// reports nothing anywhere.
    #[test]
    fn a_save_addressed_in_the_other_seams_shape_still_finds_its_own_world() {
        let (runtime, held) = running();
        runtime
            .stage_world(WorldIdentity::of("saves/world/."), None, true)
            .expect("the level path stages a world");
        held.visits.store(6, Ordering::SeqCst);

        let outcome = runtime.save_world(&WorldIdentity::of("saves/world"));

        assert!(
            !matches!(outcome, SaveOutcome::IdentityMismatch(_)),
            "the two shapes of one world stopped selecting one checkpoint: {outcome:?}"
        );
        assert_eq!(visits(&replacement(outcome)), 6);
    }

    #[test]
    fn two_different_worlds_are_never_the_same_world() {
        assert_ne!(
            WorldIdentity::of("saves/first"),
            WorldIdentity::of("saves/second")
        );
        assert_ne!(
            WorldIdentity::of("saves/world"),
            WorldIdentity::of("other/world")
        );
        assert_ne!(
            WorldIdentity::of("saves/world"),
            WorldIdentity::of("/saves/world"),
            "one is under the game directory and one is not"
        );
        assert_ne!(
            WorldIdentity::of("saves/world"),
            WorldIdentity::of("saves/world/nether"),
            "collapsing separators must not collapse a path into its parent"
        );
    }

    /// The notice list is a bounded record rather than a leak.
    ///
    /// One notice is written per world open and nothing consumes one, so an
    /// unbounded list grows for as long as the game runs. The bound is asserted
    /// by overrunning it: the count stops at the capacity and the first world
    /// noticed is gone while the last is still there.
    #[test]
    fn the_notices_a_long_session_accumulates_stay_bounded() {
        let (runtime, _held) = running();
        let overrun = OBSERVATION_CAPACITY + 3;
        for index in 0..overrun {
            runtime
                .stage_world(WorldIdentity::of(&format!("worlds/w{index}")), None, false)
                .expect("an existing world with no Ravel data still stages");
        }

        let observed = runtime.persistence_observations();

        assert_eq!(
            observed.len(),
            OBSERVATION_CAPACITY,
            "the notices a session writes are bounded, and this session wrote more"
        );
        assert!(
            observed
                .iter()
                .all(|notice| !notice.contains("`worlds/w0`")),
            "the oldest notice outlived the bound: {observed:?}"
        );
        assert!(
            observed
                .last()
                .expect("the bound is not zero")
                .contains(&format!("`worlds/w{}`", overrun - 1)),
            "the newest notice was dropped instead of the oldest: {observed:?}"
        );
    }

    /// What a world requiring a package gets, in both polarities.
    ///
    /// No installed package set reaches this process, so the requirement cannot
    /// be compared and the crossing refuses rather than guessing — `SPEC.md` §14
    /// fails closed. The other polarity is the same world without the
    /// requirement, which is what makes the refusal the requirement's doing
    /// rather than the envelope's. The exact-version comparison
    /// `SavedDataError::IncompatiblePackage` reports is unreachable from here for
    /// the same reason and is recorded as a risk rather than left looking live.
    #[test]
    fn a_world_requiring_a_package_is_refused_and_the_same_world_without_one_is_not() {
        let requiring = stored_requiring(vec![ravel_save::RequiredPackage {
            id: id("base"),
            version: ravel_core::Version::new(2, 0, 0),
        }]);
        let requiring_none = stored_requiring(Vec::new());

        let (runtime, _held) = running();
        let refused = runtime
            .stage_world(WorldIdentity::of(FIRST), Some(&requiring), true)
            .expect_err("a requirement this process cannot check opens no world");
        assert!(refused.contains("base"), "{refused}");
        assert!(
            matches!(
                runtime.save_world(&WorldIdentity::of(FIRST)),
                SaveOutcome::IdentityMismatch(_)
            ),
            "a refused world was staged anyway"
        );

        let (runtime, _held) = running();
        runtime
            .stage_world(WorldIdentity::of(FIRST), Some(&requiring_none), true)
            .expect("the same world declaring no requirement opens");
    }

    /// The bytes of a world whose one envelope declares `required`.
    fn stored_requiring(required: Vec<ravel_save::RequiredPackage>) -> Vec<u8> {
        let mut aggregate = SavedDataAggregate::fresh();
        aggregate.replace(SavedDataEnvelope::new(
            id(OWNER),
            version(1),
            required,
            None,
            json!({ "visits": 1 }),
        ));
        aggregate.to_bytes().expect("bounded")
    }

    #[test]
    fn an_existing_world_that_lost_its_data_is_noticed_and_admitted_anyway() {
        let (runtime, _held) = running();

        runtime
            .stage_world(WorldIdentity::of(FIRST), None, false)
            .expect("absence still initializes fresh state for any world");

        let observed = runtime.persistence_observations();
        assert_eq!(observed.len(), 1, "{observed:?}");
        assert!(observed[0].contains(FIRST), "{observed:?}");
        assert!(observed[0].contains("holds no Ravel data"), "{observed:?}");
        // Admission is untouched: the world staged and saves from it work.
        replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
    }

    #[test]
    fn a_new_world_with_no_data_is_the_ordinary_case_and_says_nothing() {
        let (runtime, _held) = running();

        runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("a new world stages");

        assert!(runtime.persistence_observations().is_empty());
    }

    #[test]
    fn the_save_answer_tags_what_java_should_do_and_only_one_tag_writes() {
        assert_eq!(vec![UNCHANGED], tagged(UNCHANGED, &[]));
        assert_eq!(tagged(REPLACEMENT, b"stored"), b"\x01stored".to_vec());
        assert_eq!(tagged(FAILED, b"why"), b"\x02why".to_vec());
        assert_eq!(tagged(IDENTITY_MISMATCH, b"whose"), b"\x03whose".to_vec());
        // The four tags are distinct, which is what lets Java tell a mod's fault
        // from a lifecycle bug rather than reporting both as one.
        assert_eq!(
            BTreeSet::from([UNCHANGED, REPLACEMENT, FAILED, IDENTITY_MISMATCH]).len(),
            4
        );
    }

    /// The gate's machine half, asked by the crossing the game actually calls.
    ///
    /// A required mod that failed halts the set, and from then on the loader
    /// withholds worlds. Before this crossing consulted the gate, the loader
    /// answered that question and nothing asked it.
    #[test]
    fn a_halted_set_cannot_open_a_world_at_the_crossing_the_game_calls() {
        let (runtime, _held) = running();
        // The same set stages a world while it is healthy, which is what makes
        // the refusal below the failure's doing rather than the fixture's.
        runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("a running set opens a world");

        runtime.fail_while_running(&id(OWNER), attribution());

        let refused = runtime
            .stage_world(WorldIdentity::of(SECOND), None, true)
            .expect_err("a halted set opens no world");
        assert!(refused.starts_with("no world opens: "), "{refused}");
        assert!(refused.contains(OWNER), "{refused}");
        // Refused rather than admitted-and-empty, so Java refuses the load.
        assert_eq!(
            super::outcome(&runtime, SECOND, None, true),
            OpenOutcome::Refused(refused)
        );
    }

    /// The gate's pre-world half, which no machine can answer for.
    #[test]
    fn a_refusal_the_machine_never_saw_still_withholds_the_world() {
        let mut report = crate::runtime::pre_world_report();
        report.refuse(
            &ravel_observability::Subject::of(id("example_mod")),
            &Unresolvable,
        );
        let (runtime, _held) = running_with(report);

        let refused = runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect_err("a report holding a refusal withholds the world");
        assert!(
            refused.contains("`example_mod` requires `example_library`"),
            "{refused}"
        );
        // The running set alone would have opened one, which is the point: this
        // refusal is the report's and reaches the crossing anyway.
        assert!(
            running()
                .0
                .stage_world(WorldIdentity::of(FIRST), None, true)
                .is_ok()
        );
    }

    #[test]
    fn a_refusal_carries_the_diagnostic_to_the_java_adapter() {
        assert_eq!(
            OpenOutcome::from(ravel_save::SavedDataError::AggregateTooLarge {
                limit: MAX_AGGREGATE_BYTES
            }),
            OpenOutcome::Refused(
                "the canonical saved-data aggregate exceeds the 1048576-byte limit".to_owned()
            )
        );
    }
}
