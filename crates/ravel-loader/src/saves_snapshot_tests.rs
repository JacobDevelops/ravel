use std::sync::{Arc, Mutex};

use ravel_core::{
    DiagnosticMessage, FailureAttribution, FailureEnvironment, LifecycleState, ModId, Side,
    StructuredFailure,
};
use ravel_host_api::{HostMod, ModError};
use ravel_save::{SavedDataAggregate, SavedDataEnvelope};
use serde_json::json;

use crate::lifecycle::{LifecycleEntry, LifecycleError, LifecycleMachine, Requirement};
use crate::loaded::LoadedMods;
use crate::saves::{SaveRefused, produce_saved_data};
use crate::tests::entry;

type Calls = Arc<Mutex<Vec<ModId>>>;

struct SnapshotHost {
    id: ModId,
    calls: Calls,
    refuses: bool,
    envelope: Option<SavedDataEnvelope>,
}

impl HostMod for SnapshotHost {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
        Ok(())
    }

    fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, ModError> {
        self.calls.lock().expect("test log").push(self.id.clone());
        if self.refuses {
            return Err(ModError::Unsupported);
        }
        Ok(self.envelope.clone())
    }
}

fn id(name: &str) -> ModId {
    name.parse().expect("legal id")
}

fn host(name: &str, calls: &Calls) -> SnapshotHost {
    SnapshotHost {
        id: id(name),
        calls: Arc::clone(calls),
        refuses: false,
        envelope: None,
    }
}

fn environment() -> FailureEnvironment {
    FailureEnvironment {
        ravel: "1.0.0".parse().expect("version"),
        minecraft: "snapshot-test".to_owned(),
        platform: "test".to_owned(),
        architecture: "test".to_owned(),
    }
}

fn machine(entries: Vec<LifecycleEntry>, running: bool) -> LifecycleMachine {
    let mut machine = LifecycleMachine::new(Side::DedicatedServer, entries).expect("legal plan");
    if running {
        for state in [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
            LifecycleState::Running,
        ] {
            machine.advance_all(state).expect("legal transition");
        }
    }
    machine
}

fn fail(machine: &mut LifecycleMachine, name: &str) {
    let owner = id(name);
    let attribution = FailureAttribution {
        mod_id: Some(owner.clone()),
        version: None,
        dependency_chain: Vec::new(),
        state: machine.state(&owner).expect("tracked"),
        kind: None,
        surface: None,
        environment: FailureEnvironment {
            ravel: "1.0.0".parse().expect("version"),
            minecraft: "snapshot-test".to_owned(),
            platform: "test".to_owned(),
            architecture: "test".to_owned(),
        },
        message: DiagnosticMessage::raw("owner failed"),
    };
    machine.fail(&owner, attribution).expect("failure recorded");
}

#[test]
fn snapshot_guards_refuse_before_callbacks_and_forward_lifecycle_errors() {
    for halted in [false, true] {
        let calls = Calls::default();
        let mut owner = host("owner", &calls);
        let mut machine = machine(vec![entry("owner", &[Side::DedicatedServer])], halted);
        let expected = if halted {
            machine.record_world_mutation().expect("running world");
            fail(&mut machine, "owner");
            LifecycleError::PartialStateRefused {
                mod_id: id("owner"),
            }
        } else {
            LifecycleError::WorldAccessBlocked {
                mod_id: id("owner"),
                state: LifecycleState::Discovered,
            }
        };
        let transcript = machine.transcript().to_vec();
        let refused = produce_saved_data(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut owner])
                .expect("complete hosts"),
            &environment(),
            None,
        )
        .expect_err("lifecycle access is denied");
        assert_eq!(refused, SaveRefused::Lifecycle(expected.clone()));
        assert_eq!(refused.to_string(), expected.to_string());
        assert_eq!(refused.failure_kind(), expected.failure_kind());
        assert_eq!(refused.lifecycle_state(), expected.lifecycle_state());
        assert_eq!(
            refused.quotes_untrusted_text(),
            expected.quotes_untrusted_text()
        );
        assert!(std::error::Error::source(&refused).is_some());
        assert!(calls.lock().expect("test log").is_empty());
        assert_eq!(machine.transcript(), transcript);
    }
}

#[test]
fn snapshot_refusal_follows_lifecycle_order_not_service_slot_order() {
    let calls = Calls::default();
    let mut alpha = host("alpha", &calls);
    let mut zeta = host("zeta", &calls);
    alpha.refuses = true;
    zeta.refuses = true;
    let mut machine = machine(
        vec![
            entry("alpha", &[Side::DedicatedServer]),
            entry("zeta", &[Side::DedicatedServer]),
        ],
        true,
    );
    let refused = produce_saved_data(
        &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut zeta, &mut alpha])
            .expect("complete hosts in service order"),
        &environment(),
        None,
    )
    .expect_err("alpha refuses first");
    assert!(matches!(refused, SaveRefused::Owner { mod_id, .. } if mod_id == id("alpha")));
    assert_eq!(*calls.lock().expect("test log"), vec![id("alpha")]);
    assert_eq!(machine.state(&id("alpha")), Ok(LifecycleState::Failed));
    let failure = machine
        .failure(&id("alpha"))
        .expect("known owner")
        .expect("recorded failure");
    assert_eq!(failure.mod_id, Some(id("alpha")));
    assert_eq!(failure.state, LifecycleState::Running);
    assert_eq!(failure.environment, environment());
}

#[test]
fn optional_snapshot_refusal_records_the_owner_failure() {
    let calls = Calls::default();
    let mut owner = host("owner", &calls);
    owner.refuses = true;
    let mut optional = entry("owner", &[Side::DedicatedServer]);
    optional.requirement = Requirement::Optional;
    let mut machine = machine(vec![optional], true);
    let refused = produce_saved_data(
        &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut owner])
            .expect("complete hosts"),
        &environment(),
        None,
    )
    .expect_err("optional snapshot failure still refuses the save");
    assert!(matches!(refused, SaveRefused::Owner { mod_id, .. } if mod_id == id("owner")));
    assert_eq!(machine.state(&id("owner")), Ok(LifecycleState::Failed));
    let failure = machine
        .failure(&id("owner"))
        .expect("known owner")
        .expect("recorded failure");
    assert_eq!(failure.mod_id, Some(id("owner")));
    assert_eq!(failure.state, LifecycleState::Running);
    assert_eq!(failure.environment, environment());
    assert_eq!(*calls.lock().expect("test log"), vec![id("owner")]);
}

#[test]
fn failed_owners_keep_prior_envelopes_and_parked_and_extra_hosts_are_not_called() {
    let calls = Calls::default();
    let mut failed = host("failed", &calls);
    let mut parked = host("parked", &calls);
    let mut extra = host("extra", &calls);
    let mut healthy = host("healthy", &calls);
    failed.refuses = true;
    parked.refuses = true;
    extra.refuses = true;
    let mut optional = entry("failed", &[Side::DedicatedServer]);
    optional.requirement = Requirement::Optional;
    let mut machine = machine(
        vec![
            optional,
            entry("parked", &[Side::Client]),
            entry("healthy", &[Side::DedicatedServer]),
        ],
        true,
    );
    fail(&mut machine, "failed");
    let mut prior = SavedDataAggregate::fresh();
    prior.replace(SavedDataEnvelope::new(
        id("failed"),
        std::num::NonZeroU32::MIN,
        Vec::new(),
        None,
        json!({"opaque": [1, 2, 3]}),
    ));
    let snapshot = produce_saved_data(
        &mut LoadedMods::<&mut dyn HostMod>::new(
            &mut machine,
            &mut [&mut extra, &mut healthy, &mut parked, &mut failed],
        )
        .expect("all loading hosts are supplied"),
        &environment(),
        Some(&prior),
    )
    .expect("failed data remains opaque and healthy owners may return no envelope");
    assert_eq!(snapshot, prior);
    assert_eq!(*calls.lock().expect("test log"), vec![id("healthy")]);
    assert_eq!(machine.state(&id("failed")), Ok(LifecycleState::Failed));
    assert_eq!(machine.state(&id("healthy")), Ok(LifecycleState::Running));
}
#[test]
fn healthy_snapshots_replace_only_the_producers_own_envelope() {
    for foreign in [false, true] {
        let calls = Calls::default();
        let envelope = |owner: &str, count: u32| {
            SavedDataEnvelope::new(
                id(owner),
                std::num::NonZeroU32::MIN,
                Vec::new(),
                None,
                json!({"count": count}),
            )
        };
        let mut alpha = host("alpha", &calls);
        alpha.envelope = Some(envelope("alpha", 9));
        let mut zeta = host("zeta", &calls);
        zeta.envelope = Some(envelope(if foreign { "alpha" } else { "zeta" }, 13));
        let mut machine = machine(
            vec![
                entry("alpha", &[Side::DedicatedServer]),
                entry("zeta", &[Side::DedicatedServer]),
            ],
            true,
        );
        let mut prior = SavedDataAggregate::fresh();
        prior.replace(envelope("alpha", 1));
        let original = prior.to_bytes().expect("bounded fixture");
        let result = produce_saved_data(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut zeta, &mut alpha])
                .expect("complete reversed slots"),
            &environment(),
            Some(&prior),
        );
        if foreign {
            assert_eq!(
                result,
                Err(SaveRefused::ForeignEnvelope {
                    mod_id: id("zeta"),
                    owner: id("alpha"),
                }),
            );
        } else {
            let snapshot = result.expect("each producer writes its own envelope");
            assert_eq!(snapshot.envelope(&id("alpha")), Some(&envelope("alpha", 9)));
            assert_eq!(snapshot.envelope(&id("zeta")), Some(&envelope("zeta", 13)));
        }
        assert_eq!(prior.to_bytes().expect("bounded fixture"), original);
        assert_eq!(
            *calls.lock().expect("test log"),
            vec![id("alpha"), id("zeta")]
        );
        assert_eq!(machine.state(&id("alpha")), Ok(LifecycleState::Running));
        assert_eq!(
            machine.state(&id("zeta")),
            Ok(if foreign {
                LifecycleState::Failed
            } else {
                LifecycleState::Running
            })
        );
    }
}
