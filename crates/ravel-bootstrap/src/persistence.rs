//! Identity-keyed world persistence owned by the installed loader runtime.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock, PoisonError};

use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{JNI_FALSE, jboolean};
use jni::{Env, EnvUnowned};
use ravel_core::ModId;
use ravel_save::{
    MAX_AGGREGATE_BYTES, SavedDataAggregate, SavedDataDeclaration, SavedDataError, SavedDataPlan,
};
use semver::Version;

use crate::guard::{HookError, text};
use crate::runtime::LoaderRuntime;

static RUNTIME: OnceLock<Arc<LoaderRuntime>> = OnceLock::new();

#[derive(Clone, Debug)]
struct Checkpoint {
    identity: String,
    aggregate: SavedDataAggregate,
    dirty: bool,
}

#[derive(Default)]
pub(crate) struct PersistenceState {
    live: Option<Checkpoint>,
    staged: Option<Checkpoint>,
    /// What was worth noticing about a world that was still admitted.
    observations: Vec<String>,
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

/// What counts as the same world, decided in exactly one place.
///
/// The game hands Ravel whichever path shape the call site had, and the two
/// seams do not agree: a level path and a world path for one world differ by a
/// trailing separator and a `.` segment. Normalizing both ends here is what
/// keeps a save from missing the stage it belongs to — the failure that mode
/// produces is invisible, because nothing errors and nothing is written.
///
/// It is lexical only. A save happens while the world directory is being torn
/// down, so touching the filesystem could fail or answer differently than it did
/// at open, and `canonicalize` is exactly that.
fn identity(raw: &str) -> String {
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
    normalized
}

/// Replaces the staged world after admitting and delivering its aggregate.
///
/// `fresh_world` is the game's advisory marker for a world being created. It
/// never decides admission: an absent aggregate initializes fresh state for any
/// world, exactly as ADG-7 says. It is read only to tell the ordinary case apart
/// from an existing world that used to hold Ravel data and now does not, which
/// is worth noticing and is not worth refusing over.
#[must_use]
pub fn stage(identity_raw: &str, bytes: Option<&[u8]>, fresh_world: bool) -> OpenOutcome {
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return OpenOutcome::Refused(error),
    };
    match runtime.stage_world(identity(identity_raw), bytes, fresh_world) {
        Ok(()) => OpenOutcome::Staged,
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
    runtime.save_world(&identity(identity_raw))
}

/// What the open crossing noticed about worlds it admitted anyway.
#[must_use]
pub fn observations() -> Vec<String> {
    runtime().map_or_else(
        |error| vec![error],
        |runtime| runtime.persistence_observations(),
    )
}

impl LoaderRuntime {
    /// What the open crossing noticed about worlds it admitted anyway.
    pub(crate) fn persistence_observations(&self) -> Vec<String> {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .persistence
            .observations
            .clone()
    }

    fn declarations(inner: &crate::runtime::Inner) -> Vec<SavedDataDeclaration> {
        inner
            .mods
            .iter()
            .filter_map(|host| host.saved_data_declaration())
            .collect()
    }

    /// The packages an envelope's `requiredPackages` is checked against.
    ///
    /// Nothing carries a package version into the runtime yet: the composed host
    /// builds mods from source and `LifecycleEntry` holds an identity rather
    /// than a version, so the installed set is genuinely empty here. That makes
    /// any declared requirement refuse, which is the direction `SPEC.md` §14
    /// asks for, and no mod this build composes declares one. The map is built
    /// here rather than inlined so the packaged set replaces one function.
    fn installed_packages(_inner: &crate::runtime::Inner) -> BTreeMap<ModId, Version> {
        BTreeMap::new()
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

    pub(crate) fn stage_world(
        &self,
        identity: String,
        bytes: Option<&[u8]>,
        fresh_world: bool,
    ) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        // Absence initializes fresh state for any world, exactly as ADG-7 says.
        // What the marker separates is the ordinary new world from an existing
        // one that used to hold Ravel data and now does not, which is worth
        // saying and is not worth refusing over.
        if bytes.is_none() && !fresh_world {
            inner.persistence.observations.push(format!(
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
        let packages = Self::installed_packages(&inner);
        let aggregate = SavedDataAggregate::open(bytes, &declarations, &packages)
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

    pub(crate) fn save_world(&self, identity: &str) -> SaveOutcome {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if inner
            .persistence
            .live
            .as_ref()
            .is_some_and(|live| live.identity == identity)
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

        if inner
            .persistence
            .staged
            .as_ref()
            .is_some_and(|staged| staged.identity == identity)
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
                SaveOutcome::Unchanged => vec![UNCHANGED],
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

impl From<SavedDataError> for OpenOutcome {
    fn from(error: SavedDataError) -> Self {
        Self::Refused(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::num::NonZero;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    use ravel_core::{LifecycleState, ModId, Side, SideSet};
    use ravel_host_api::{HostError, HostMod};
    use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement};
    use ravel_save::{
        MAX_AGGREGATE_BYTES, SavedDataAggregate, SavedDataDeclaration, SavedDataEnvelope,
        SavedDataPlan, SavedDataVersion,
    };
    use serde_json::json;

    use super::{
        FAILED, IDENTITY_MISMATCH, OpenOutcome, REPLACEMENT, SaveOutcome, UNCHANGED, identity,
        tagged,
    };
    use crate::runtime::LoaderRuntime;

    const OWNER: &str = "saved_data";
    const FIRST: &str = "worlds/first";
    const SECOND: &str = "worlds/second";

    fn id(name: &str) -> ModId {
        name.parse().expect("a legal mod id")
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
            _plan: &SavedDataPlan,
        ) -> Result<(), HostError> {
            let visits = envelope.map_or(0, |stored| {
                u32::try_from(stored.payload()["visits"].as_u64().unwrap_or(0)).unwrap_or(u32::MAX)
            });
            self.held.visits.store(visits, Ordering::SeqCst);
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

    /// A runtime holding one running mod that owns saved data.
    fn running() -> (LoaderRuntime, Arc<Held>) {
        let held = Arc::new(Held::default());
        let host = Owning {
            id: id(OWNER),
            held: Arc::clone(&held),
        };
        let entry = LifecycleEntry {
            mod_id: id(OWNER),
            sides: SideSet::new([Side::Client]).expect("a non-empty side set"),
            requirement: Requirement::Required,
            saved_data_versions: BTreeSet::from([version(1), version(2)]),
        };
        let mut machine =
            LifecycleMachine::new(Side::Client, vec![entry]).expect("one mod is a legal plan");
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
        (LoaderRuntime::new(machine, vec![Box::new(host)]), held)
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
        let declarations = [
            SavedDataDeclaration::new(id(OWNER), [version(1), version(2)]).expect("two versions"),
        ];
        SavedDataAggregate::open(
            Some(bytes),
            &declarations,
            &std::collections::BTreeMap::new(),
        )
        .expect("a produced aggregate is admissible")
        .envelope(&id(OWNER))
        .expect("the owner is in the aggregate")
        .payload()["visits"]
            .as_u64()
            .expect("the payload records a visit count")
    }

    #[test]
    fn an_absent_aggregate_stages_fresh_state_and_the_first_save_promotes_it() {
        let (runtime, held) = running();

        assert_eq!(runtime.stage_world(FIRST.to_owned(), None, true), Ok(()));
        // Staged is not live: nothing has bound this world to save emission yet.
        held.visits.store(4, Ordering::SeqCst);

        assert_eq!(visits(&replacement(runtime.save_world(FIRST))), 4);
        // Promoted, so the same identity now matches live rather than staged.
        assert_eq!(runtime.save_world(FIRST), SaveOutcome::Unchanged);
    }

    #[test]
    fn an_unmatched_save_receiver_never_replaces_the_aggregate() {
        let (runtime, _held) = running();
        runtime
            .stage_world(FIRST.to_owned(), None, true)
            .expect("absence stages");

        let outcome = runtime.save_world(SECOND);

        assert!(
            matches!(outcome, SaveOutcome::IdentityMismatch(_)),
            "a save from a world Ravel never opened wrote to it anyway: {outcome:?}"
        );
        // The unmatched save must not have promoted the stage either.
        assert_eq!(visits(&replacement(runtime.save_world(FIRST))), 0);
    }

    #[test]
    fn an_idle_autosave_leaves_the_stored_aggregate_alone() {
        let (runtime, held) = running();
        runtime
            .stage_world(FIRST.to_owned(), None, true)
            .expect("absence stages");
        replacement(runtime.save_world(FIRST));

        assert_eq!(runtime.save_world(FIRST), SaveOutcome::Unchanged);

        held.visits.store(1, Ordering::SeqCst);
        assert_eq!(visits(&replacement(runtime.save_world(FIRST))), 1);
    }

    #[test]
    fn a_production_failure_preserves_the_previous_aggregate() {
        let (runtime, held) = running();
        runtime
            .stage_world(FIRST.to_owned(), None, true)
            .expect("absence stages");
        held.visits.store(7, Ordering::SeqCst);
        let durable = replacement(runtime.save_world(FIRST));

        held.visits.store(9, Ordering::SeqCst);
        held.refuses_production.store(true, Ordering::SeqCst);
        let outcome = runtime.save_world(FIRST);
        assert!(
            matches!(outcome, SaveOutcome::Failed(_)),
            "a refused envelope was reported as something else: {outcome:?}"
        );

        // The world Java still holds is the one that was written last.
        assert_eq!(visits(&durable), 7);
        held.refuses_production.store(false, Ordering::SeqCst);
        assert_eq!(visits(&replacement(runtime.save_world(FIRST))), 9);
    }

    #[test]
    fn the_prior_world_saves_the_state_it_held_when_the_next_world_was_staged() {
        let (runtime, held) = running();
        runtime
            .stage_world(FIRST.to_owned(), None, true)
            .expect("absence stages");
        held.visits.store(3, Ordering::SeqCst);
        replacement(runtime.save_world(FIRST));

        // The prologue runs before the original body stops the prior server, so
        // the incoming world's state reaches the mods first and the prior
        // world's teardown save happens after it.
        held.visits.store(5, Ordering::SeqCst);
        runtime
            .stage_world(SECOND.to_owned(), None, true)
            .expect("the second world stages");
        assert_eq!(
            held.visits.load(Ordering::SeqCst),
            0,
            "staging the second world left the first world's state in the mod"
        );

        assert_eq!(
            visits(&replacement(runtime.save_world(FIRST))),
            5,
            "the prior world's teardown save wrote the incoming world's state"
        );
        assert_eq!(visits(&replacement(runtime.save_world(SECOND))), 0);
    }

    #[test]
    fn each_open_replaces_the_prior_stage() {
        let (runtime, _held) = running();
        runtime
            .stage_world(FIRST.to_owned(), None, true)
            .expect("absence stages");
        runtime
            .stage_world(SECOND.to_owned(), None, true)
            .expect("the second world stages");

        let outcome = runtime.save_world(FIRST);
        assert!(
            matches!(outcome, SaveOutcome::IdentityMismatch(_)),
            "a stage the next open replaced still answered a save: {outcome:?}"
        );
        replacement(runtime.save_world(SECOND));
    }

    #[test]
    fn an_oversized_aggregate_is_refused_before_it_is_parsed() {
        let (runtime, _held) = running();
        // Not JSON, so anything that parsed before measuring would fail differently.
        let oversized = vec![b'\0'; MAX_AGGREGATE_BYTES + 1];

        let refused = runtime
            .stage_world(FIRST.to_owned(), Some(&oversized), true)
            .expect_err("an aggregate over the ceiling does not open a world");

        assert!(refused.contains("limit"), "{refused}");
        assert_eq!(
            runtime.save_world(FIRST),
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
                .stage_world(FIRST.to_owned(), Some(&bytes), true)
                .expect_err("this aggregate does not open a world");
            assert!(!refused.is_empty(), "`{name}` refused without saying why");
            assert!(
                matches!(runtime.save_world(FIRST), SaveOutcome::IdentityMismatch(_)),
                "`{name}` was refused and staged anyway"
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
        assert_eq!(identity("saves/world/."), identity("saves/world"));
        assert_eq!(identity("saves/world/"), identity("saves/world"));
        assert_eq!(identity("saves//world//."), identity("saves/world"));
        assert_eq!(identity("/saves/world/."), identity("/saves/world"));
        assert_eq!(
            identity("/saves/world/."),
            "/saves/world",
            "a rooted path stays rooted, so two worlds under different roots stay apart"
        );
    }

    #[test]
    fn two_different_worlds_are_never_the_same_world() {
        assert_ne!(identity("saves/first"), identity("saves/second"));
        assert_ne!(identity("saves/world"), identity("other/world"));
        assert_ne!(
            identity("saves/world"),
            identity("/saves/world"),
            "one is under the game directory and one is not"
        );
        assert_ne!(
            identity("saves/world"),
            identity("saves/world/nether"),
            "collapsing separators must not collapse a path into its parent"
        );
    }

    #[test]
    fn an_existing_world_that_lost_its_data_is_noticed_and_admitted_anyway() {
        let (runtime, _held) = running();

        runtime
            .stage_world(FIRST.to_owned(), None, false)
            .expect("absence still initializes fresh state for any world");

        let observed = runtime.persistence_observations();
        assert_eq!(observed.len(), 1, "{observed:?}");
        assert!(observed[0].contains(FIRST), "{observed:?}");
        assert!(observed[0].contains("holds no Ravel data"), "{observed:?}");
        // Admission is untouched: the world staged and saves from it work.
        replacement(runtime.save_world(FIRST));
    }

    #[test]
    fn a_new_world_with_no_data_is_the_ordinary_case_and_says_nothing() {
        let (runtime, _held) = running();

        runtime
            .stage_world(FIRST.to_owned(), None, true)
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
