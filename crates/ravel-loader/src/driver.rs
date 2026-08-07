use std::fmt;

use ravel_core::{LifecycleState, ModId, Side};
use ravel_host_api::{HostError, HostMod};

use crate::{LifecycleEntry, LifecycleError, LifecycleMachine, executes_mod_code};

const START_ORDER: [LifecycleState; 6] = [
    LifecycleState::Validated,
    LifecycleState::Resolved,
    LifecycleState::Loaded,
    LifecycleState::Constructed,
    LifecycleState::Registered,
    LifecycleState::Running,
];

/// Drives a statically composed mod set to `running` in its stated order.
///
/// Each entry travels with the ordinary Rust value ADG-2 selected for it. This
/// function does not discover or load code and therefore creates no second mod
/// execution mechanism beside static source composition.
///
/// # Errors
///
/// Returns [`DriveError::Identity`] when a constructed value disagrees with the
/// entry paired with it, [`DriveError::Lifecycle`] when the lifecycle refuses a
/// transition or registration, and [`DriveError::Host`] when mod code refuses a
/// lifecycle step.
pub fn drive(
    side: Side,
    mut composed: Vec<(LifecycleEntry, Box<dyn HostMod>)>,
) -> Result<(LifecycleMachine, Vec<Box<dyn HostMod>>), DriveError> {
    for (entry, host) in &composed {
        if &entry.mod_id != host.id() {
            return Err(DriveError::Identity {
                declared: entry.mod_id.clone(),
                constructed: host.id().clone(),
            });
        }
    }

    let entries = composed.iter().map(|(entry, _)| entry.clone()).collect();
    let mut machine = LifecycleMachine::new(side, entries).map_err(DriveError::Lifecycle)?;

    for state in START_ORDER {
        machine.advance_all(state).map_err(DriveError::Lifecycle)?;
        if executes_mod_code(state) {
            for (_, host) in &mut composed {
                if machine.state(host.id()).map_err(DriveError::Lifecycle)? == state {
                    host.enter(state).map_err(DriveError::Host)?;
                }
            }
        }
        if state == LifecycleState::Constructed {
            register(&mut machine, &mut composed)?;
        }
    }

    Ok((
        machine,
        composed.into_iter().map(|(_, host)| host).collect(),
    ))
}

fn register(
    machine: &mut LifecycleMachine,
    composed: &mut [(LifecycleEntry, Box<dyn HostMod>)],
) -> Result<(), DriveError> {
    for (_, host) in composed {
        if machine.state(host.id()).map_err(DriveError::Lifecycle)? != LifecycleState::Constructed {
            continue;
        }
        let mut window = machine
            .registration_window(host.id())
            .map_err(DriveError::Lifecycle)?;
        let outcome = host.register(&mut window);
        let unexpected = window.unexpected().cloned();
        if let Some(error) = unexpected {
            return Err(DriveError::Lifecycle(error));
        }
        outcome.map_err(DriveError::Host)?;
    }
    Ok(())
}

/// Why a statically composed set could not start.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DriveError {
    /// The declaration and its constructed Rust value claim different owners.
    Identity {
        /// The identity in the resolved entry.
        declared: ModId,
        /// The identity returned by the constructed value.
        constructed: ModId,
    },
    /// The lifecycle machine refused a transition or registration.
    Lifecycle(LifecycleError),
    /// Mod code refused a lifecycle step.
    Host(HostError),
}

impl fmt::Display for DriveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity {
                declared,
                constructed,
            } => write!(
                formatter,
                "the entry declares `{declared}`, but its constructed mod is `{constructed}`"
            ),
            Self::Lifecycle(error) => write!(formatter, "the lifecycle refused the set: {error}"),
            Self::Host(error) => write!(formatter, "a mod refused the set: {error}"),
        }
    }
}

impl std::error::Error for DriveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity { .. } => None,
            Self::Lifecycle(error) => Some(error),
            Self::Host(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::{Arc, Mutex};

    use ravel_core::{LifecycleState, ModId, Side, SideSet};
    use ravel_host_api::{HostError, HostMod};

    use super::{DriveError, drive};
    use crate::{LifecycleEntry, Requirement};

    struct Recording {
        id: ModId,
        states: Arc<Mutex<Vec<LifecycleState>>>,
    }

    impl HostMod for Recording {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, state: LifecycleState) -> Result<(), HostError> {
            self.states.lock().expect("nothing panicked").push(state);
            Ok(())
        }
    }

    fn id(value: &str) -> ModId {
        value.parse().expect("a legal mod id")
    }

    fn entry(name: &str, sides: &[Side]) -> LifecycleEntry {
        LifecycleEntry {
            mod_id: id(name),
            sides: SideSet::new(sides.iter().copied()).expect("a non-empty side set"),
            requirement: Requirement::Required,
            saved_data_versions: BTreeSet::new(),
        }
    }

    #[test]
    fn a_composed_value_is_driven_and_returned_with_its_running_machine() {
        let states = Arc::new(Mutex::new(Vec::new()));
        let host = Recording {
            id: id("example_mod"),
            states: Arc::clone(&states),
        };

        let (machine, mods) = drive(
            Side::Client,
            vec![(
                entry("example_mod", &[Side::Client]),
                Box::new(host) as Box<dyn HostMod>,
            )],
        )
        .expect("the set should start");

        assert_eq!(
            machine.state(&id("example_mod")),
            Ok(LifecycleState::Running)
        );
        assert_eq!(mods.len(), 1);
        assert_eq!(
            *states.lock().expect("nothing panicked"),
            [
                LifecycleState::Loaded,
                LifecycleState::Constructed,
                LifecycleState::Registered,
                LifecycleState::Running,
            ]
        );
    }

    #[test]
    fn a_value_for_another_identity_is_refused_before_mod_code_runs() {
        let states = Arc::new(Mutex::new(Vec::new()));
        let host = Recording {
            id: id("other_mod"),
            states: Arc::clone(&states),
        };

        let Err(error) = drive(
            Side::Client,
            vec![(
                entry("example_mod", &[Side::Client]),
                Box::new(host) as Box<dyn HostMod>,
            )],
        ) else {
            panic!("the identities disagree");
        };

        assert_eq!(
            error,
            DriveError::Identity {
                declared: id("example_mod"),
                constructed: id("other_mod"),
            }
        );
        assert!(states.lock().expect("nothing panicked").is_empty());
    }

    #[test]
    fn a_value_not_loading_on_this_side_is_not_entered() {
        let states = Arc::new(Mutex::new(Vec::new()));
        let host = Recording {
            id: id("server_mod"),
            states: Arc::clone(&states),
        };

        let (machine, _) = drive(
            Side::Client,
            vec![(
                entry("server_mod", &[Side::DedicatedServer]),
                Box::new(host) as Box<dyn HostMod>,
            )],
        )
        .expect("a side-mismatched optional execution is parked");

        assert_eq!(
            machine.state(&id("server_mod")),
            Ok(LifecycleState::Resolved)
        );
        assert!(states.lock().expect("nothing panicked").is_empty());
    }
}
