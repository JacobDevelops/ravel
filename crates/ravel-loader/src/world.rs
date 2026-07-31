use std::fmt;

use ravel_observability::{PreWorldReport, WorldAccessRefused};

use crate::lifecycle::{LifecycleError, LifecycleMachine};

impl LifecycleMachine {
    /// Opens a world only when nothing anywhere refused one.
    ///
    /// [`LifecycleMachine::world_access`] answers for the set the machine is
    /// driving, and that is all it can answer for: a package whose manifest
    /// would not parse, whose digest did not match, or whose dependency graph
    /// had no order never became an entry in any plan, so no machine ever sees
    /// it. `SPEC.md` §9.2 makes Ravel fail closed on an invalid required-mod
    /// graph, which means those refusals have to withhold the world too, and
    /// the pre-world report is where they are.
    ///
    /// The report is consulted first because it holds the earlier failures, and
    /// the earliest problem is the one worth showing a player.
    ///
    /// # Errors
    ///
    /// Returns [`WorldRefused::PreWorld`] when the report holds any refusal, and
    /// [`WorldRefused::Lifecycle`] when the running set itself cannot open a
    /// world — a mod that is not running, or a failure that followed world
    /// mutation.
    pub fn open_world(&self, report: &PreWorldReport) -> Result<(), WorldRefused> {
        report.world_access().map_err(WorldRefused::PreWorld)?;
        self.world_access().map_err(WorldRefused::Lifecycle)
    }
}

/// Why no world opened.
///
/// The two variants are the two halves of the gate, kept apart because they are
/// different things to fix: one is a package that should not have been
/// installed, and the other is a load that did not finish.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorldRefused {
    /// Something was refused before the load reached a lifecycle machine.
    PreWorld(WorldAccessRefused),
    /// The running set cannot open a world.
    Lifecycle(LifecycleError),
}

impl fmt::Display for WorldRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PreWorld(refused) => write!(formatter, "{refused}"),
            Self::Lifecycle(error) => write!(formatter, "no world opens: {error}"),
        }
    }
}

impl std::error::Error for WorldRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PreWorld(refused) => Some(refused),
            Self::Lifecycle(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fmt;

    use ravel_core::{
        FailureEnvironment, FailureKind, LifecycleState, ModId, Side, SideSet, StructuredFailure,
    };
    use ravel_observability::{PreWorldReport, Subject};

    use super::WorldRefused;
    use crate::lifecycle::{LifecycleEntry, LifecycleError, LifecycleMachine, Requirement};

    /// A failure from a stage that runs long before any lifecycle machine.
    #[derive(Debug)]
    struct Unresolvable;

    impl fmt::Display for Unresolvable {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("`example_mod` requires `example_library`, which is not installed")
        }
    }

    impl std::error::Error for Unresolvable {}

    impl StructuredFailure for Unresolvable {
        fn failure_kind(&self) -> Option<FailureKind> {
            Some(FailureKind::MissingRequiredDependency)
        }

        fn lifecycle_state(&self) -> LifecycleState {
            LifecycleState::Validated
        }
    }

    fn id(name: &str) -> ModId {
        name.parse().expect("a valid mod id")
    }

    fn report() -> PreWorldReport {
        PreWorldReport::new(FailureEnvironment {
            ravel: "0.1.0".parse().expect("a valid version"),
            minecraft: "26.0.0".to_owned(),
            platform: "linux".to_owned(),
            architecture: "x86_64".to_owned(),
        })
    }

    /// A machine whose only mod is running, so the world is the report's to
    /// withhold.
    fn running() -> LifecycleMachine {
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![LifecycleEntry {
                mod_id: id("example_mod"),
                sides: SideSet::new([Side::Client]).expect("one side"),
                requirement: Requirement::Required,
                saved_data_versions: BTreeSet::new(),
            }],
        )
        .expect("one mod is a valid plan");
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
        machine
    }

    #[test]
    fn a_running_set_with_a_clean_report_opens_a_world() {
        assert_eq!(running().open_world(&report()), Ok(()));
    }

    #[test]
    fn a_refusal_the_machine_never_saw_still_withholds_the_world() {
        let machine = running();
        let mut report = report();
        report.refuse(&Subject::of(id("example_mod")), &Unresolvable);

        let refused = machine
            .open_world(&report)
            .expect_err("the report withholds it");
        assert!(matches!(refused, WorldRefused::PreWorld(_)), "{refused}");
        assert!(
            refused
                .to_string()
                .contains("`example_mod` requires `example_library`"),
            "{refused}"
        );
        assert!(
            machine.world_access().is_ok(),
            "the machine alone would have opened one, which is the point"
        );
    }

    #[test]
    fn a_set_that_is_not_running_is_refused_by_the_machine_with_a_clean_report() {
        let machine = LifecycleMachine::new(
            Side::Client,
            vec![LifecycleEntry {
                mod_id: id("example_mod"),
                sides: SideSet::new([Side::Client]).expect("one side"),
                requirement: Requirement::Required,
                saved_data_versions: BTreeSet::new(),
            }],
        )
        .expect("one mod is a valid plan");

        let refused = machine
            .open_world(&report())
            .expect_err("nothing is running");
        assert_eq!(
            refused,
            WorldRefused::Lifecycle(LifecycleError::WorldAccessBlocked {
                mod_id: id("example_mod"),
                state: LifecycleState::Discovered,
            })
        );
        assert!(refused.to_string().starts_with("no world opens: "));
    }

    #[test]
    fn the_earlier_refusal_is_the_one_reported_when_both_halves_object() {
        let machine = LifecycleMachine::new(Side::Client, Vec::new()).expect("an empty plan");
        let mut report = report();
        report.refuse(&Subject::unidentified(), &Unresolvable);

        assert!(matches!(
            machine
                .open_world(&report)
                .expect_err("the report objects first"),
            WorldRefused::PreWorld(_)
        ));
    }

    #[test]
    fn every_refusal_names_its_source_error() {
        use std::error::Error;

        let machine = LifecycleMachine::new(
            Side::Client,
            vec![LifecycleEntry {
                mod_id: id("example_mod"),
                sides: SideSet::new([Side::Client]).expect("one side"),
                requirement: Requirement::Required,
                saved_data_versions: BTreeSet::new(),
            }],
        )
        .expect("one mod is a valid plan");
        let mut refused_report = report();
        refused_report.refuse(&Subject::unidentified(), &Unresolvable);

        for refusal in [
            machine.open_world(&refused_report).expect_err("refused"),
            machine.open_world(&report()).expect_err("not running"),
        ] {
            assert!(refusal.source().is_some(), "{refusal}");
        }
    }
}
