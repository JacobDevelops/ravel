use ravel_core::{FailureEnvironment, LifecycleState, ModId, Side, SideSet};

use crate::{LifecycleEntry, LifecycleMachine, Requirement};

pub(crate) fn id(name: &str) -> ModId {
    name.parse().expect("a legal mod id")
}

pub(crate) fn entry(name: &str, sides: &[Side]) -> LifecycleEntry {
    LifecycleEntry {
        mod_id: id(name),
        sides: SideSet::new(sides.iter().copied()).expect("a non-empty side set"),
        requirement: Requirement::Required,
        saved_data_versions: std::collections::BTreeSet::new(),
    }
}

pub(crate) fn environment() -> FailureEnvironment {
    FailureEnvironment {
        ravel: "0.1.0".parse().expect("valid version"),
        minecraft: "Synthetic26.1.5".to_owned(),
        platform: "test-platform".to_owned(),
        architecture: "test-architecture".to_owned(),
    }
}

pub(crate) fn advance_through(machine: &mut LifecycleMachine, last: LifecycleState) {
    const SUCCESS_PATH: [LifecycleState; 6] = [
        LifecycleState::Validated,
        LifecycleState::Resolved,
        LifecycleState::Loaded,
        LifecycleState::Constructed,
        LifecycleState::Registered,
        LifecycleState::Running,
    ];
    assert!(
        SUCCESS_PATH.contains(&last),
        "{last:?} is not on the success path"
    );
    for state in SUCCESS_PATH {
        machine.advance_all(state).expect("a legal step");
        if state == last {
            break;
        }
    }
}
