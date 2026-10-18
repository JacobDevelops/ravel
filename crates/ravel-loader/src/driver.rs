use std::fmt;

use ravel_core::{
    FailureAttribution, FailureEnvironment, FailureKind, LifecycleState, ModId, Side,
    StructuredFailure,
};
use ravel_host_api::{HostError, HostMod};

use crate::lifecycle::{
    LifecycleEntry, LifecycleError, LifecycleMachine, Requirement, executes_mod_code,
};
use crate::services::{ServiceRefused, open_services};

const START_ORDER: [LifecycleState; 6] = [
    LifecycleState::Validated,
    LifecycleState::Resolved,
    LifecycleState::Loaded,
    LifecycleState::Constructed,
    LifecycleState::Registered,
    LifecycleState::Running,
];

/// Drives statically composed mods to `running` in their stated order.
///
/// Optional callback failures are recorded and removed from later dispatch;
/// required failures stop startup before the world can open.
/// Returned host slots retain resolved entry order after service wiring.
///
/// # Errors
///
/// Returns [`DriveError`] for mismatched identities, refused admission, or required host failures.
pub fn drive(
    side: Side,
    environment: &FailureEnvironment,
    composed: Vec<(LifecycleEntry, Box<dyn HostMod>)>,
) -> Result<(LifecycleMachine, Vec<Box<dyn HostMod>>), DriveError> {
    for (entry, host) in &composed {
        if &entry.mod_id != host.id() {
            return Err(DriveError::Identity {
                declared: entry.mod_id.clone(),
                constructed: host.id().clone(),
            });
        }
    }

    let (entries, mut hosts): (Vec<_>, Vec<_>) = composed.into_iter().unzip();
    let mut machine = LifecycleMachine::new(side, entries).map_err(DriveError::Lifecycle)?;
    let mut loaded = crate::loaded::LoadedMods::admitted(&mut machine, &mut hosts);
    drive_loaded(&mut loaded, environment)?;
    crate::loaded::arrange(&mut hosts, machine.order());
    Ok((machine, hosts))
}

/// Drives a complete paired view through startup in resolved entry order.
///
/// Callback failures retain the supplied runtime environment in their attribution.
///
/// # Errors
///
/// Returns [`DriveError`] when startup is refused. Completeness is checked before
/// this operation by [`crate::LoadedMods::new`], without advancing any lifecycle state.
pub fn drive_loaded<H: std::ops::DerefMut<Target: HostMod>>(
    loaded: &mut crate::LoadedMods<'_, H>,
    environment: &FailureEnvironment,
) -> Result<(), DriveError> {
    let order: Vec<_> = loaded.machine().order().cloned().collect();
    for state in START_ORDER {
        enter_all(loaded, environment, &order, state)?;
        if state == LifecycleState::Constructed {
            register(loaded, environment, &order)?;
        }
        if state == LifecycleState::Registered {
            open_services(loaded, environment)?;
        }
    }
    Ok(())
}

/// Quiesces paired hosts through `stopping` and `stopped`.
///
/// # Errors
///
/// Every stopping callback runs before the first required failure is returned.
/// Returns [`DriveError`] for lifecycle refusal or required callback failure.
pub fn stop_all<H: std::ops::DerefMut<Target: HostMod>>(
    loaded: &mut crate::LoadedMods<'_, H>,
    environment: &FailureEnvironment,
) -> Result<(), DriveError> {
    let order: Vec<_> = loaded.machine().order().cloned().collect();
    let mut failure = None;
    for state in [LifecycleState::Stopping, LifecycleState::Stopped] {
        if let Err(error) = enter_all(loaded, environment, &order, state) {
            failure.get_or_insert(error);
        }
    }
    failure.map_or(Ok(()), Err)
}

fn enter_all<H: std::ops::DerefMut<Target: HostMod>>(
    loaded: &mut crate::LoadedMods<'_, H>,
    environment: &FailureEnvironment,
    order: &[ModId],
    state: LifecycleState,
) -> Result<(), DriveError> {
    let (machine, mods) = loaded.parts();
    machine.advance_all(state).map_err(DriveError::Lifecycle)?;
    if !executes_mod_code(state) {
        return Ok(());
    }
    let mut failure = None;
    for mod_id in order {
        if machine.state(mod_id).map_err(DriveError::Lifecycle)? != state {
            continue;
        }
        let host = mods
            .iter_mut()
            .find(|host| host.id() == mod_id)
            .expect("the paired view checked every loading owner");
        let entered = crate::callback(mod_id, state, || host.enter(state));
        if let Err(error) = entered
            && let Err(error) = fail(machine, environment, error)
        {
            if !matches!(state, LifecycleState::Stopping | LifecycleState::Stopped) {
                return Err(error);
            }
            failure.get_or_insert(error);
        }
    }
    failure.map_or(Ok(()), Err)
}

fn fail(
    machine: &mut LifecycleMachine,
    environment: &FailureEnvironment,
    error: HostError,
) -> Result<(), DriveError> {
    let requirement = machine
        .fail_callback(&error, environment, None)
        .map_err(DriveError::Lifecycle)?;
    if requirement == Requirement::Required {
        let attribution = machine
            .failure(error.mod_id())
            .map_err(DriveError::Lifecycle)?
            .expect("the callback failure was recorded")
            .clone();
        return Err(DriveError::Host {
            source: error,
            attribution: Box::new(attribution),
        });
    }
    Ok(())
}

fn register<H: std::ops::DerefMut<Target: HostMod>>(
    loaded: &mut crate::LoadedMods<'_, H>,
    environment: &FailureEnvironment,
    order: &[ModId],
) -> Result<(), DriveError> {
    let (machine, mods) = loaded.parts();
    for mod_id in order {
        if machine.state(mod_id).map_err(DriveError::Lifecycle)? != LifecycleState::Constructed {
            continue;
        }
        let host = mods
            .iter_mut()
            .find(|host| host.id() == mod_id)
            .expect("the paired view checked every loading owner");
        let mut window = machine
            .registration_window(mod_id)
            .map_err(DriveError::Lifecycle)?;
        let outcome = crate::callback(mod_id, LifecycleState::Constructed, || {
            host.register(&mut window)
        });
        let unexpected = window.unexpected().cloned();
        if let Some(error) = unexpected {
            return Err(DriveError::Lifecycle(error));
        }
        if let Err(error) = outcome {
            fail(machine, environment, error)?;
        }
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
    /// The set's services could not be admitted before startup.
    Services(ServiceRefused),
    /// Mod code refused a lifecycle step.
    Host {
        /// The callback failure, stamped with its owner and phase.
        source: HostError,
        /// The attribution recorded before returning the failure.
        attribution: Box<FailureAttribution>,
    },
}

impl From<ServiceRefused> for DriveError {
    fn from(error: ServiceRefused) -> Self {
        Self::Services(error)
    }
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
            Self::Services(error) => write!(formatter, "the services refused the set: {error}"),
            Self::Host { source, .. } => write!(formatter, "a mod refused the set: {source}"),
        }
    }
}

impl std::error::Error for DriveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity { .. } => None,
            Self::Lifecycle(error) => Some(error),
            Self::Services(error) => Some(error),
            Self::Host { source, .. } => Some(source),
        }
    }
}

impl StructuredFailure for DriveError {
    fn failure_kind(&self) -> Option<FailureKind> {
        match self {
            Self::Identity { .. } => Some(FailureKind::DuplicateIdentity),
            Self::Lifecycle(error) => error.failure_kind(),
            Self::Services(error) => error.failure_kind(),
            Self::Host { source, .. } => source.failure_kind(),
        }
    }

    fn lifecycle_state(&self) -> LifecycleState {
        match self {
            Self::Identity { .. } => LifecycleState::Discovered,
            Self::Lifecycle(error) => error.lifecycle_state(),
            Self::Services(error) => error.lifecycle_state(),
            Self::Host { source, .. } => source.state(),
        }
    }

    fn quotes_untrusted_text(&self) -> bool {
        match self {
            Self::Identity { .. } => false,
            Self::Lifecycle(error) => error.quotes_untrusted_text(),
            Self::Services(error) => error.quotes_untrusted_text(),
            Self::Host { source, .. } => source.quotes_untrusted_text(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use ravel_core::{LifecycleState, ModId, Side};
    use ravel_host_api::{HostError, HostMod, ModError, Registrar, ServiceDeclaration, ServiceId};

    use super::{DriveError, drive, drive_loaded, stop_all};
    use crate::lifecycle::Requirement;
    use crate::services::ServiceRefused;
    use crate::tests::entry;

    struct Recording {
        id: ModId,
        states: Arc<Mutex<Vec<LifecycleState>>>,
    }

    impl HostMod for Recording {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
            self.states.lock().expect("nothing panicked").push(state);
            Ok(())
        }
    }

    fn id(value: &str) -> ModId {
        value.parse().expect("a legal mod id")
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
            &environment(),
            vec![(entry("example_mod", &[Side::Client]), Box::new(host))],
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
            &environment(),
            vec![(entry("example_mod", &[Side::Client]), Box::new(host))],
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
            &environment(),
            vec![(
                entry("server_mod", &[Side::DedicatedServer]),
                Box::new(host),
            )],
        )
        .expect("a side-mismatched optional execution is parked");

        assert_eq!(
            machine.state(&id("server_mod")),
            Ok(LifecycleState::Resolved)
        );
        assert!(states.lock().expect("nothing panicked").is_empty());
    }

    struct RequiringService(Recording);

    impl HostMod for RequiringService {
        fn id(&self) -> &ModId {
            self.0.id()
        }

        fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
            self.0.enter(state)
        }

        fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
            assert_eq!(
                self.0.states.lock().expect("nothing panicked").as_slice(),
                [
                    LifecycleState::Loaded,
                    LifecycleState::Loaded,
                    LifecycleState::Constructed,
                    LifecycleState::Constructed,
                    LifecycleState::Registered,
                    LifecycleState::Registered,
                ]
            );
            let required = ServiceId::new("absent:prices".parse().expect("a legal service id"));
            Ok(Some(
                ServiceDeclaration::new(self.id().clone(), Vec::new(), vec![required])
                    .expect("a legal requirement"),
            ))
        }
    }

    #[test]
    fn service_refusal_prevents_every_running_callback() {
        let states = Arc::new(Mutex::new(Vec::new()));
        let healthy = Recording {
            id: id("healthy_mod"),
            states: Arc::clone(&states),
        };
        let consumer = RequiringService(Recording {
            id: id("consumer_mod"),
            states: Arc::clone(&states),
        });
        let outcome = drive(
            Side::Client,
            &environment(),
            vec![
                (entry("healthy_mod", &[Side::Client]), Box::new(healthy)),
                (entry("consumer_mod", &[Side::Client]), Box::new(consumer)),
            ],
        );
        assert!(
            !states
                .lock()
                .expect("nothing panicked")
                .contains(&LifecycleState::Running),
            "service admission must finish before any mod starts"
        );
        assert!(matches!(
            outcome,
            Err(DriveError::Services(ServiceRefused::NoProvider { .. }))
        ));
    }

    struct Failing {
        recording: Recording,
        register: bool,
        panics: bool,
    }

    impl Failing {
        fn refuse(&self) -> Result<(), ModError> {
            assert!(!self.panics, "callback panic must not escape");
            Err(ModError::refused("startup refused"))
        }
    }

    impl HostMod for Failing {
        fn id(&self) -> &ModId {
            self.recording.id()
        }

        fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
            self.recording.enter(state)?;
            if !self.register && state == LifecycleState::Constructed {
                return self.refuse();
            }
            Ok(())
        }

        fn register(&mut self, _: &mut dyn Registrar) -> Result<(), ModError> {
            if self.register {
                return self.refuse();
            }
            Ok(())
        }

        fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
            panic!("a failed mod must not declare services");
        }
    }

    fn startup_failure(requirement: Requirement, register: bool, panics: bool) {
        let failed_states = Arc::new(Mutex::new(Vec::new()));
        let healthy_states = Arc::new(Mutex::new(Vec::new()));
        let mut failed_entry = entry("failing_mod", &[Side::Client]);
        failed_entry.requirement = requirement;
        let failure = Failing {
            recording: Recording {
                id: id("failing_mod"),
                states: Arc::clone(&failed_states),
            },
            register,
            panics,
        };
        let healthy = Recording {
            id: id("healthy_mod"),
            states: Arc::clone(&healthy_states),
        };
        let outcome = drive(
            Side::Client,
            &environment(),
            vec![
                (failed_entry, Box::new(failure)),
                (entry("healthy_mod", &[Side::Client]), Box::new(healthy)),
            ],
        );
        if requirement == Requirement::Required {
            assert!(matches!(
                &outcome,
                Err(DriveError::Host { source: error, .. })
                    if error.mod_id() == &id("failing_mod")
                        && error.state() == LifecycleState::Constructed
            ));
            assert!(
                !healthy_states
                    .lock()
                    .expect("not poisoned")
                    .contains(&LifecycleState::Running)
            );
        } else {
            let (machine, _) = outcome.expect("optional callback failure drops only its owner");
            assert_eq!(
                machine.state(&id("failing_mod")),
                Ok(LifecycleState::Failed)
            );
            assert_eq!(
                machine.state(&id("healthy_mod")),
                Ok(LifecycleState::Running)
            );
            let failure = machine
                .failure(&id("failing_mod"))
                .expect("known owner")
                .expect("failure recorded");
            assert_eq!(failure.mod_id, Some(id("failing_mod")));
            assert_eq!(failure.state, LifecycleState::Constructed);
            machine
                .world_access()
                .expect("optional failure allows the world");
        }
        assert!(
            !failed_states
                .lock()
                .expect("not poisoned")
                .contains(&LifecycleState::Running)
        );
    }

    #[test]
    fn optional_enter_panic_drops_only_its_owner() {
        startup_failure(Requirement::Optional, false, true);
    }

    #[test]
    fn required_enter_panic_halts_startup() {
        startup_failure(Requirement::Required, false, true);
    }

    #[test]
    fn optional_register_panic_drops_only_its_owner() {
        startup_failure(Requirement::Optional, true, true);
    }

    #[test]
    fn required_register_panic_halts_startup() {
        startup_failure(Requirement::Required, true, true);
    }

    #[test]
    fn returned_startup_errors_follow_the_same_failure_policy() {
        for requirement in [Requirement::Optional, Requirement::Required] {
            for register in [false, true] {
                startup_failure(requirement, register, false);
            }
        }
    }

    #[test]
    fn required_callback_failure_is_observable_through_the_public_driver() {
        let mut host = Failing {
            recording: Recording {
                id: id("required_mod"),
                states: Arc::new(Mutex::new(Vec::new())),
            },
            register: false,
            panics: false,
        };
        let mut machine = crate::LifecycleMachine::new(
            Side::Client,
            vec![entry("required_mod", &[Side::Client])],
        )
        .expect("valid lifecycle");
        let error = drive_loaded(
            &mut crate::LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                .expect("complete host set"),
            &environment(),
        )
        .expect_err("required callback failure stops startup");
        let DriveError::Host {
            source,
            attribution,
        } = error
        else {
            panic!("expected a recorded host failure");
        };
        assert_eq!(source.mod_id(), &id("required_mod"));
        assert_eq!(attribution.mod_id, Some(id("required_mod")));
        assert_eq!(attribution.state, LifecycleState::Constructed);
        assert_eq!(attribution.environment, environment());
    }

    #[test]
    fn missing_borrowed_host_is_refused_before_its_first_transition() {
        let mut machine =
            crate::LifecycleMachine::new(Side::Client, vec![entry("missing_mod", &[Side::Client])])
                .expect("valid lifecycle");
        let error = crate::LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [])
            .expect_err("a missing host refuses the common view");
        assert_eq!(error.mod_id(), &id("missing_mod"));
        assert_eq!(
            machine.state(&id("missing_mod")),
            Ok(LifecycleState::Discovered)
        );
    }

    #[test]
    fn shutdown_drives_borrowed_hosts_through_stopped() {
        let states = Arc::new(Mutex::new(Vec::new()));
        let mut host = Recording {
            id: id("example_mod"),
            states: Arc::clone(&states),
        };
        let mut machine =
            crate::LifecycleMachine::new(Side::Client, vec![entry("example_mod", &[Side::Client])])
                .expect("valid lifecycle");
        let mut slots: [&mut dyn HostMod; 1] = [&mut host];
        let mut loaded =
            crate::LoadedMods::new(&mut machine, &mut slots).expect("complete host set");
        drive_loaded(&mut loaded, &environment()).expect("startup succeeds");
        stop_all(&mut loaded, &environment()).expect("shutdown succeeds");
        assert_eq!(
            machine.state(&id("example_mod")),
            Ok(LifecycleState::Stopped)
        );
        assert_eq!(
            states.lock().expect("not poisoned").last(),
            Some(&LifecycleState::Stopping)
        );
    }

    struct PanickingService {
        id: ModId,
        declaration_panics: bool,
    }

    impl HostMod for PanickingService {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
            assert!(!self.declaration_panics, "service declaration panic");
            let absent = ServiceId::new("absent:clock".parse().expect("a legal service id"));
            Ok(Some(
                ServiceDeclaration::with_optional(self.id.clone(), [], [], [absent])
                    .expect("a valid optional service"),
            ))
        }

        fn use_services(
            &mut self,
            _: &mut dyn ravel_host_api::ServiceDirectory,
        ) -> Result<(), ModError> {
            panic!("service consumer panic");
        }
    }

    #[test]
    fn service_callback_panics_follow_the_owner_requirement() {
        for requirement in [Requirement::Optional, Requirement::Required] {
            for declaration_panics in [false, true] {
                let mut owner = entry("service_mod", &[Side::Client]);
                owner.requirement = requirement;
                let host = PanickingService {
                    id: owner.mod_id.clone(),
                    declaration_panics,
                };
                let states = Arc::new(Mutex::new(Vec::new()));
                let healthy = Recording {
                    id: id("healthy_mod"),
                    states: Arc::clone(&states),
                };
                let outcome = drive(
                    Side::Client,
                    &environment(),
                    vec![
                        (owner, Box::new(host)),
                        (entry("healthy_mod", &[Side::Client]), Box::new(healthy)),
                    ],
                );
                if requirement == Requirement::Optional {
                    let (machine, _) = outcome.expect("optional service failure drops its owner");
                    assert_eq!(
                        machine.state(&id("service_mod")),
                        Ok(LifecycleState::Failed)
                    );
                    let failure = machine
                        .failure(&id("service_mod"))
                        .expect("known owner")
                        .expect("recorded service failure");
                    assert_eq!(failure.state, LifecycleState::Registered);
                    assert_eq!(
                        machine.state(&id("healthy_mod")),
                        Ok(LifecycleState::Running)
                    );
                } else {
                    let Err(DriveError::Services(ServiceRefused::Consumer { mod_id, source })) =
                        outcome
                    else {
                        panic!("required service callback failure refuses startup");
                    };
                    assert_eq!(mod_id, id("service_mod"));
                    assert_eq!(
                        source,
                        HostError::Panicked {
                            mod_id: id("service_mod"),
                            state: LifecycleState::Registered,
                        }
                    );
                    assert!(
                        !states
                            .lock()
                            .expect("not poisoned")
                            .contains(&LifecycleState::Running)
                    );
                }
            }
        }
    }

    struct StoppingFailure {
        recording: Recording,
        panics: bool,
    }

    impl HostMod for StoppingFailure {
        fn id(&self) -> &ModId {
            self.recording.id()
        }

        fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
            self.recording.enter(state)?;
            if state == LifecycleState::Stopping {
                assert!(!self.panics, "shutdown panic");
                return Err(ModError::refused("shutdown refusal"));
            }
            Ok(())
        }
    }

    #[test]
    fn shutdown_failures_do_not_leave_other_owners_in_stopping() {
        for requirement in [Requirement::Optional, Requirement::Required] {
            for panics in [false, true] {
                shutdown_failure(requirement, panics);
            }
        }
    }

    fn shutdown_failure(requirement: Requirement, panics: bool) {
        let states = Arc::new(Mutex::new(Vec::new()));
        let mut first = StoppingFailure {
            recording: Recording {
                id: id("first"),
                states: Arc::clone(&states),
            },
            panics,
        };
        let mut second = StoppingFailure {
            recording: Recording {
                id: id("second"),
                states: Arc::clone(&states),
            },
            panics,
        };
        let mut healthy = Recording {
            id: id("healthy"),
            states: Arc::clone(&states),
        };
        let mut first_entry = entry("first", &[Side::Client]);
        first_entry.requirement = requirement;
        let mut second_entry = entry("second", &[Side::Client]);
        second_entry.requirement = requirement;
        let mut hosts: [&mut dyn HostMod; 3] = [&mut first, &mut second, &mut healthy];
        let mut machine = crate::LifecycleMachine::new(
            Side::Client,
            vec![first_entry, second_entry, entry("healthy", &[Side::Client])],
        )
        .expect("valid lifecycle");
        let mut loaded =
            crate::LoadedMods::new(&mut machine, &mut hosts).expect("complete host set");
        drive_loaded(&mut loaded, &environment()).expect("startup succeeds");
        let result = stop_all(&mut loaded, &environment());
        if requirement == Requirement::Required {
            assert!(matches!(result, Err(DriveError::Host { source, .. })
                if source.mod_id() == &id("first")));
        } else {
            result.expect("optional failures do not refuse shutdown");
        }
        for name in ["first", "second"] {
            assert_eq!(machine.state(&id(name)), Ok(LifecycleState::Failed));
            let failure = machine
                .failure(&id(name))
                .expect("known owner")
                .expect("recorded");
            assert_eq!(failure.state, LifecycleState::Stopping);
        }
        assert_eq!(machine.state(&id("healthy")), Ok(LifecycleState::Stopped));
        assert_eq!(
            states
                .lock()
                .expect("not poisoned")
                .iter()
                .filter(|state| **state == LifecycleState::Stopping)
                .count(),
            3
        );
    }

    fn environment() -> ravel_core::FailureEnvironment {
        ravel_core::FailureEnvironment {
            ravel: "0.1.0".parse().expect("valid version"),
            minecraft: "Synthetic26.1.5".to_owned(),
            platform: "test-platform".to_owned(),
            architecture: "test-architecture".to_owned(),
        }
    }
}
