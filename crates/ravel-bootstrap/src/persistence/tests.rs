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

fn scratch(name: &str) -> std::path::PathBuf {
    let at = std::env::temp_dir().join(format!("ravel-world-guard-{}-{name}", std::process::id()));
    if at.exists() {
        std::fs::remove_dir_all(&at).expect("remove the previous test directory");
    }
    std::fs::create_dir_all(&at).expect("a writable temporary directory");
    at
}

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
    StagedPack, UNCHANGED, WorldIdentity, tagged,
};
use crate::runtime::LoaderRuntime;

const OWNER: &str = "saved_data";
const OTHER_OWNER: &str = "other_saved_data";
const FIRST: &str = "worlds/first";
const SECOND: &str = "worlds/second";

fn id(name: &str) -> ModId {
    name.parse().expect("a legal mod id")
}

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

#[derive(Debug, Default)]
struct Held {
    visits: AtomicU32,
    refuses_production: AtomicBool,
    refuses_open: AtomicBool,
    refuses_interception: AtomicBool,
    refuses_interception_declaration: AtomicBool,
    opened_with: Mutex<Option<SavedDataPlan>>,
    error_owner: Mutex<Option<ModId>>,
    declaration_owner: Mutex<Option<ModId>>,
    refuses_declaration: AtomicBool,
}

impl Held {
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

    fn interception_declaration(&self) -> Result<Option<InterceptionDeclaration>, HostError> {
        if self
            .held
            .refuses_interception_declaration
            .load(Ordering::SeqCst)
        {
            return Err(HostError::Unsupported {
                mod_id: id(OTHER_OWNER),
                state: LifecycleState::Running,
            });
        }
        let mut declaration = InterceptionDeclaration::new(self.id.clone());
        declaration.answer_at(Point::WorldIsSaved.as_str());
        Ok(Some(declaration))
    }

    fn intercept(&mut self, _occasion: Occasion) -> Result<Outcome, HostError> {
        if self.held.refuses_interception.load(Ordering::SeqCst) {
            return Err(HostError::Unsupported {
                mod_id: id(OTHER_OWNER),
                state: LifecycleState::Running,
            });
        }
        Ok(Outcome::Proceed)
    }

    fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, HostError> {
        if self.held.refuses_declaration.load(Ordering::SeqCst) {
            return Err(HostError::Unsupported {
                mod_id: self.id.clone(),
                state: LifecycleState::Running,
            });
        }
        let owner = self
            .held
            .declaration_owner
            .lock()
            .expect("test lock")
            .clone()
            .unwrap_or_else(|| self.id.clone());
        Ok(Some(
            SavedDataDeclaration::new(owner, [version(1), version(2)]).expect("two versions"),
        ))
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
                mod_id: self
                    .held
                    .error_owner
                    .lock()
                    .expect("test lock")
                    .clone()
                    .unwrap_or_else(|| self.id.clone()),
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

fn running() -> (LoaderRuntime, Arc<Held>) {
    running_with(crate::runtime::pre_world_report())
}

fn running_with(report: ravel_observability::PreWorldReport) -> (LoaderRuntime, Arc<Held>) {
    let (runtime, mut held) = owners_running(&[OWNER], report);
    (runtime, held.remove(0))
}

fn running_pair() -> (LoaderRuntime, Arc<Held>, Arc<Held>) {
    let (runtime, mut held) =
        owners_running(&[OWNER, OTHER_OWNER], crate::runtime::pre_world_report());
    let second = held.remove(1);
    (runtime, held.remove(0), second)
}

fn owners_running(
    owners: &[&str],
    report: ravel_observability::PreWorldReport,
) -> (LoaderRuntime, Vec<Arc<Held>>) {
    owners_running_with_requirement(owners, report, Requirement::Required)
}

fn owners_running_with_requirement(
    owners: &[&str],
    report: ravel_observability::PreWorldReport,
    requirement: Requirement,
) -> (LoaderRuntime, Vec<Arc<Held>>) {
    let held: Vec<Arc<Held>> = owners.iter().map(|_| Arc::new(Held::default())).collect();
    let hosts: Vec<Box<dyn HostMod>> = owners
        .iter()
        .zip(&held)
        .map(|(owner, held)| -> Box<dyn HostMod> {
            Box::new(Owning {
                id: id(owner),
                held: Arc::clone(held),
            })
        })
        .collect();
    let entries: Vec<LifecycleEntry> = owners
        .iter()
        .map(|owner| LifecycleEntry {
            mod_id: id(owner),
            sides: SideSet::new([Side::Client]).expect("a non-empty side set"),
            requirement,
            saved_data_versions: BTreeSet::from([version(1), version(2)]),
        })
        .collect();
    let mut machine =
        LifecycleMachine::new(Side::Client, entries).expect("independent mods are a legal plan");
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

const GUARD: &str = "zephyr_guard";

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

    fn interception_declaration(&self) -> Result<Option<InterceptionDeclaration>, HostError> {
        let mut declared = InterceptionDeclaration::new(self.id.clone());
        declared.answer_at(Point::WorldIsSaved.as_str());
        Ok(Some(declared))
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
    let mut machine =
        LifecycleMachine::new(Side::Client, entries).expect("independent mods are a legal plan");
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

#[test]
fn a_mod_refusing_at_world_is_saved_withholds_ravels_write_and_a_later_save_offers_it() {
    let (runtime, held, refusing, asked) = guarded_running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("a fresh world is staged");
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

    refusing.store(false, Ordering::SeqCst);
    let bytes = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    assert_eq!(visits(&bytes), 7);
}

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

        fn interception_declaration(&self) -> Result<Option<InterceptionDeclaration>, HostError> {
            let mut declared = InterceptionDeclaration::new(self.id.clone());
            declared.answer_at(Point::WorldIsSaved.as_str());
            Ok(Some(declared))
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

#[test]
fn an_optional_owner_failed_before_first_open_is_admitted_without_callbacks() {
    let (source, _) = running();
    source
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("fresh world");
    let durable = replacement(source.save_world(&WorldIdentity::of(FIRST)));
    let (runtime, held) = optional_running();
    runtime
        .inner
        .lock()
        .expect("test lock")
        .machine
        .fail(&id(OWNER), attribution())
        .expect("optional owner fails before opening");
    held.refuses_declaration.store(true, Ordering::SeqCst);
    held.refuses_open.store(true, Ordering::SeqCst);
    runtime
        .stage_world(WorldIdentity::of(FIRST), Some(&durable), false)
        .expect("terminal optional owners are admitted from trusted lifecycle metadata");
    assert_eq!(
        replacement(runtime.save_world(&WorldIdentity::of(FIRST))),
        durable
    );
}

#[test]
fn a_foreign_saved_data_declaration_fails_its_producer_without_opening_another_owner() {
    for foreign in ["absent_owner", OTHER_OWNER] {
        let (runtime, held, other) = running_pair();
        *held.declaration_owner.lock().expect("test lock") = Some(id(foreign));
        assert!(
            runtime
                .stage_world(WorldIdentity::of(FIRST), None, true)
                .is_err()
        );
        let inner = runtime
            .inner
            .lock()
            .expect("declaration rejection does not panic");
        assert_eq!(inner.machine.state(&id(OWNER)), Ok(LifecycleState::Failed));
        assert_eq!(
            inner.machine.state(&id(OTHER_OWNER)),
            Ok(LifecycleState::Running)
        );
        drop(inner);
        assert!(other.opened_with.lock().expect("test lock").is_none());
    }
}

#[test]
fn interception_callback_failure_is_attributed_to_the_asked_owner() {
    let (runtime, held) = running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("fresh world");
    held.refuses_interception.store(true, Ordering::SeqCst);
    assert!(matches!(
        runtime.save_world(&WorldIdentity::of(FIRST)),
        SaveOutcome::Failed(_)
    ));
    let inner = runtime.inner.lock().expect("no test poisons the lock");
    assert_eq!(inner.machine.state(&id(OWNER)), Ok(LifecycleState::Failed));
    assert_eq!(
        inner
            .machine
            .failure(&id(OWNER))
            .expect("owner")
            .expect("failure")
            .mod_id,
        Some(id(OWNER))
    );
    drop(inner);
}

fn optional_running() -> (LoaderRuntime, Arc<Held>) {
    let (runtime, mut held) = owners_running_with_requirement(
        &[OWNER],
        crate::runtime::pre_world_report(),
        Requirement::Optional,
    );
    (runtime, held.remove(0))
}

#[test]
fn optional_save_failure_preserves_the_last_durable_owner_envelope() {
    let (runtime, held) = optional_running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("fresh world");
    held.visits.store(7, Ordering::SeqCst);
    let durable = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
    held.refuses_production.store(true, Ordering::SeqCst);
    assert!(matches!(
        runtime.save_world(&WorldIdentity::of(FIRST)),
        SaveOutcome::Failed(_)
    ));
    for _ in 0..2 {
        assert_eq!(
            runtime.save_world(&WorldIdentity::of(FIRST)),
            SaveOutcome::Unchanged
        );
    }
    assert_eq!(visits(&durable), 7);
}

#[test]
fn failed_optional_owner_never_opens_again_and_preserves_each_worlds_data() {
    let (runtime, held) = optional_running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("fresh world");
    held.visits.store(7, Ordering::SeqCst);
    let durable = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
    held.refuses_production.store(true, Ordering::SeqCst);
    assert!(matches!(
        runtime.save_world(&WorldIdentity::of(FIRST)),
        SaveOutcome::Failed(_)
    ));
    held.refuses_open.store(true, Ordering::SeqCst);
    runtime
        .stage_world(WorldIdentity::of(SECOND), Some(&durable), false)
        .expect("failed optional owner must not open again");
    assert_eq!(
        visits(&replacement(runtime.save_world(&WorldIdentity::of(SECOND)))),
        7
    );
}

#[test]
fn forged_save_error_identity_fails_the_callback_owner_without_panicking() {
    for forged in ["unknown_owner", OTHER_OWNER] {
        let (runtime, held, _) = running_pair();
        runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("fresh world");
        *held.error_owner.lock().expect("test lock") = Some(id(forged));
        held.refuses_production.store(true, Ordering::SeqCst);
        assert!(matches!(
            runtime.save_world(&WorldIdentity::of(FIRST)),
            SaveOutcome::Failed(_)
        ));
        let inner = runtime.inner.lock().expect("callback errors do not panic");
        assert_eq!(
            inner.machine.state(&id(OWNER)).expect("owner"),
            LifecycleState::Failed
        );
        assert_eq!(
            inner.machine.state(&id(OTHER_OWNER)).expect("other owner"),
            LifecycleState::Running
        );
        assert_eq!(
            inner
                .machine
                .failure(&id(OWNER))
                .expect("owner")
                .expect("failure recorded")
                .mod_id,
            Some(id(OWNER))
        );
        drop(inner);
    }
}

#[test]
fn interception_declaration_errors_fail_the_asking_owner_and_preserve_admission() {
    for requirement in [Requirement::Required, Requirement::Optional] {
        let (runtime, held) = owners_running_with_requirement(
            &[OWNER],
            crate::runtime::pre_world_report(),
            requirement,
        );
        runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("a new world stages");
        held[0]
            .refuses_interception_declaration
            .store(true, Ordering::SeqCst);
        assert!(matches!(
            runtime.save_world(&WorldIdentity::of(FIRST)),
            SaveOutcome::Failed(_)
        ));
        let inner = runtime
            .inner
            .lock()
            .expect("the callback returned an error");
        assert_eq!(
            inner.machine.state(&id(OWNER)).expect("known owner"),
            LifecycleState::Failed
        );
        let failure = inner
            .machine
            .failure(&id(OWNER))
            .expect("known owner")
            .expect("structured declaration failure");
        assert_eq!(failure.mod_id, Some(id(OWNER)));
        assert_eq!(failure.state, LifecycleState::Running);
        assert_eq!(
            inner.machine.world_access().is_ok(),
            requirement == Requirement::Optional
        );
        drop(inner);
        let retry = runtime.save_world(&WorldIdentity::of(FIRST));
        assert_eq!(
            matches!(retry, SaveOutcome::Failed(_)),
            requirement == Requirement::Required
        );
    }
}

#[test]
fn save_callback_errors_fail_the_owner_and_preserve_required_optional_admission() {
    for requirement in [Requirement::Required, Requirement::Optional] {
        let held = Arc::new(Held::default());
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![LifecycleEntry {
                mod_id: id(OWNER),
                sides: SideSet::new([Side::Client]).expect("a non-empty side set"),
                requirement,
                saved_data_versions: BTreeSet::from([version(1), version(2)]),
            }],
        )
        .expect("one owner is a legal plan");
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
        let runtime = LoaderRuntime::new(
            machine,
            vec![Box::new(Owning {
                id: id(OWNER),
                held: Arc::clone(&held),
            })],
        );
        runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("a new world stages");
        held.refuses_production.store(true, Ordering::SeqCst);
        assert!(matches!(
            runtime.save_world(&WorldIdentity::of(FIRST)),
            SaveOutcome::Failed(_)
        ));
        let inner = runtime
            .inner
            .lock()
            .expect("the callback returned an error");
        assert_eq!(
            inner.machine.state(&id(OWNER)).expect("known owner"),
            LifecycleState::Failed
        );
        let failure = inner
            .machine
            .failure(&id(OWNER))
            .expect("known owner")
            .expect("structured callback failure");
        assert_eq!(failure.mod_id, Some(id(OWNER)));
        assert_eq!(failure.state, LifecycleState::Running);
        assert!(
            failure
                .message
                .text()
                .contains("cannot produce an envelope")
        );
        assert_eq!(
            inner.machine.world_access().is_ok(),
            requirement == Requirement::Optional
        );
        drop(inner);
        let retry = runtime.save_world(&WorldIdentity::of(FIRST));
        assert_eq!(
            matches!(retry, SaveOutcome::Failed(_)),
            requirement == Requirement::Required
        );
    }
}

fn replacement(outcome: SaveOutcome) -> Vec<u8> {
    match outcome {
        SaveOutcome::Replacement(bytes) => bytes,
        other @ (SaveOutcome::Unchanged
        | SaveOutcome::Withheld
        | SaveOutcome::Failed(_)
        | SaveOutcome::IdentityMismatch(_)) => {
            panic!("this save should have replaced the aggregate: {other:?}")
        }
    }
}

fn visits(bytes: &[u8]) -> u64 {
    visits_for(bytes, &[OWNER], OWNER)
}

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

fn stored_visits(count: u32) -> Vec<u8> {
    stored_at(version(2), count)
}

fn stored_at(at: SavedDataVersion, count: u32) -> Vec<u8> {
    stored_owning(&[(OWNER, at, count)])
}

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
    assert_eq!(first.opened_with().owner(), &id(OWNER));
    assert_eq!(second.opened_with().owner(), &id(OTHER_OWNER));
    assert!(first.opened_with().is_current());
    assert_eq!(second.opened_with().steps(), [version(2)]);

    let written = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
    assert_eq!(visits_for(&written, &owners, OWNER), 5);
    assert_eq!(visits_for(&written, &owners, OTHER_OWNER), 9);
}

#[test]
fn a_world_reopened_from_what_its_save_emitted_holds_the_same_state() {
    let (first, held) = running();
    first
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("a new world stages");
    held.visits.store(11, Ordering::SeqCst);
    let stored = replacement(first.save_world(&WorldIdentity::of(FIRST)));

    let (second, reopened) = running();
    second
        .stage_world(WorldIdentity::of(FIRST), Some(&stored), false)
        .expect("the bytes a save emitted open the world they came from");

    assert_eq!(
        reopened.visits.load(Ordering::SeqCst),
        11,
        "the round trip lost the state the save emitted"
    );
    assert_eq!(
        visits(&replacement(second.save_world(&WorldIdentity::of(FIRST)))),
        11
    );
}

#[test]
fn a_world_reopened_in_one_session_writes_what_the_second_visit_held() {
    let (runtime, held) = running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("a new world stages");
    held.visits.store(11, Ordering::SeqCst);
    let first_visit = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
    assert_eq!(visits(&first_visit), 11);

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

#[test]
fn a_launch_that_stages_no_pack_does_not_ask_for_a_reload() {
    let staging = std::env::temp_dir().join(format!("ravel-unstaged-{}", std::process::id()));
    if staging.exists() {
        std::fs::remove_dir_all(&staging).expect("remove the previous staging directory");
    }
    std::fs::create_dir_all(&staging).expect("a writable temporary directory");

    let staged = shipped_running().write_staged_pack(&staging.to_string_lossy());

    assert_eq!(staged, StagedPack::Nothing);
    assert!(!staging.join("ravel").exists());
    std::fs::remove_dir_all(staging).expect("remove the staging directory");
}

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

#[test]
fn a_mod_refusing_its_stored_state_withholds_the_world_and_stages_nothing() {
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
    held.visits.store(4, Ordering::SeqCst);

    assert_eq!(
        visits(&replacement(runtime.save_world(&WorldIdentity::of(FIRST)))),
        4
    );
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

    assert_eq!(visits(&durable), 7);
    held.refuses_production.store(false, Ordering::SeqCst);
    assert!(matches!(
        runtime.save_world(&WorldIdentity::of(FIRST)),
        SaveOutcome::Failed(_)
    ));
}

#[test]
fn the_prior_world_saves_the_state_it_held_when_the_next_world_was_staged() {
    let (runtime, held) = running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("absence stages");
    held.visits.store(3, Ordering::SeqCst);
    replacement(runtime.save_world(&WorldIdentity::of(FIRST)));

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
    let oversized = vec![b'\0'; MAX_AGGREGATE_BYTES + 1];

    let refused = runtime
        .stage_world(WorldIdentity::of(FIRST), Some(&oversized), true)
        .expect_err("an aggregate over the ceiling does not open a world");

    assert!(refused.contains("limit"), "{refused}");
    assert_eq!(
        runtime.save_world(&WorldIdentity::of(FIRST)),
        SaveOutcome::IdentityMismatch(
            "save receiver `worlds/first` matches neither live nor staged world state".to_owned()
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
        assert_eq!(
            super::outcome(&runtime, FIRST, Some(&bytes), true),
            OpenOutcome::Refused(refused),
            "`{name}` reached Java as something other than a refusal"
        );
    }
}

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
    replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
}

#[test]
fn a_new_world_with_no_data_is_the_ordinary_case_and_says_nothing() {
    let (runtime, _held) = running();

    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("a new world stages");

    let empty: [String; 0] = [];
    assert_eq!(runtime.persistence_observations(), empty);
}

#[test]
fn the_save_answer_tags_what_java_should_do_and_only_one_tag_writes() {
    assert_eq!(vec![UNCHANGED], tagged(UNCHANGED, &[]));
    assert_eq!(tagged(REPLACEMENT, b"stored"), b"\x01stored".to_vec());
    assert_eq!(tagged(FAILED, b"why"), b"\x02why".to_vec());
    assert_eq!(tagged(IDENTITY_MISMATCH, b"whose"), b"\x03whose".to_vec());
    assert_eq!(
        BTreeSet::from([UNCHANGED, REPLACEMENT, FAILED, IDENTITY_MISMATCH]).len(),
        4
    );
}

#[test]
fn a_halted_set_cannot_open_a_world_at_the_crossing_the_game_calls() {
    let (runtime, _held) = running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("a running set opens a world");

    runtime.fail_while_running(&id(OWNER), attribution());

    let refused = runtime
        .stage_world(WorldIdentity::of(SECOND), None, true)
        .expect_err("a halted set opens no world");
    assert!(refused.starts_with("no world opens: "), "{refused}");
    assert!(refused.contains(OWNER), "{refused}");
    assert_eq!(
        super::outcome(&runtime, SECOND, None, true),
        OpenOutcome::Refused(refused)
    );
}

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

fn java_int(source: &str, name: &str) -> i32 {
    let (_, rest) = source
        .split_once(&format!("static final int {name} = "))
        .unwrap_or_else(|| panic!("the Java side names `{name}`"));
    rest.split(';')
        .next()
        .expect("a declaration ends in a semicolon")
        .trim()
        .parse()
        .expect("a Java int constant is an integer")
}

fn documented_staged_answers(generator: &str) -> Vec<i32> {
    let (rendered, _) = generator
        .split_once("public static native int ravelWriteStagedDatapack")
        .expect("the generator renders the staging crossing");
    let (_, answers) = rendered
        .rsplit_once("@return")
        .expect("the staging crossing documents what it answers");
    answers
        .split("{@code ")
        .skip(1)
        .filter_map(|tail| tail.split('}').next()?.trim().parse().ok())
        .collect()
}

#[test]
fn every_java_spelling_of_a_staged_answer_is_the_number_rust_gives_it() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read = |relative: &str| {
        std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|absent| panic!("{relative} is a tracked file: {absent}"))
    };
    let adapter = read("jvm/bootstrap/dev/ravel/jvm/Datapack.java");
    let standin = read("jvm/harness/initialization/dev/ravel/jvm/Hook.java");
    let generator = read("crates/ravel-minecraft-generate/src/render.rs");

    let nothing = i32::from(StagedPack::Nothing);
    let unchanged = i32::from(StagedPack::Unchanged);
    let written = i32::from(StagedPack::Written);

    assert_eq!(java_int(&adapter, "UNSTAGED"), nothing);
    assert_eq!(java_int(&adapter, "STAGED_ANEW"), written);
    assert_eq!(java_int(&standin, "UNSTAGED"), nothing);
    assert_eq!(java_int(&standin, "STAGED_UNCHANGED"), unchanged);
    assert_eq!(java_int(&standin, "STAGED_ANEW"), written);
    assert_eq!(
        documented_staged_answers(&generator),
        vec![nothing, unchanged, written]
    );
}
