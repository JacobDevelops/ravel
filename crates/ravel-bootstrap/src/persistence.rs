//! Identity-keyed world persistence owned by the installed loader runtime.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock, PoisonError};

use ravel_core::ModId;
use ravel_save::{SavedDataAggregate, SavedDataDeclaration, SavedDataError, SavedDataPlan};
use semver::Version;

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

/// Replaces the staged world after admitting and delivering its aggregate.
#[must_use]
pub fn stage(identity: String, bytes: Option<&[u8]>) -> OpenOutcome {
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return OpenOutcome::Refused(error),
    };
    match runtime.stage_world(identity, bytes) {
        Ok(()) => OpenOutcome::Staged,
        Err(error) => OpenOutcome::Refused(error),
    }
}

/// Produces the aggregate selected by the save receiver's identity.
#[must_use]
pub fn save(identity: &str) -> SaveOutcome {
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return SaveOutcome::Failed(error),
    };
    runtime.save_world(identity)
}

impl LoaderRuntime {
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

    pub(crate) fn stage_world(&self, identity: String, bytes: Option<&[u8]>) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
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

    use super::{OpenOutcome, SaveOutcome};
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

        assert_eq!(runtime.stage_world(FIRST.to_owned(), None), Ok(()));
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
            .stage_world(FIRST.to_owned(), None)
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
            .stage_world(FIRST.to_owned(), None)
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
            .stage_world(FIRST.to_owned(), None)
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
            .stage_world(FIRST.to_owned(), None)
            .expect("absence stages");
        held.visits.store(3, Ordering::SeqCst);
        replacement(runtime.save_world(FIRST));

        // The prologue runs before the original body stops the prior server, so
        // the incoming world's state reaches the mods first and the prior
        // world's teardown save happens after it.
        held.visits.store(5, Ordering::SeqCst);
        runtime
            .stage_world(SECOND.to_owned(), None)
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
            .stage_world(FIRST.to_owned(), None)
            .expect("absence stages");
        runtime
            .stage_world(SECOND.to_owned(), None)
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
            .stage_world(FIRST.to_owned(), Some(&oversized))
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
                .stage_world(FIRST.to_owned(), Some(&bytes))
                .expect_err("this aggregate does not open a world");
            assert!(!refused.is_empty(), "`{name}` refused without saying why");
            assert!(
                matches!(runtime.save_world(FIRST), SaveOutcome::IdentityMismatch(_)),
                "`{name}` was refused and staged anyway"
            );
        }
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
