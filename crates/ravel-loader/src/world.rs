use std::fmt;

use ravel_core::{FailureKind, LifecycleState, StructuredFailure};
use ravel_observability::{PreWorldReport, WorldAccessRefused};

use crate::lifecycle::{LifecycleError, LifecycleMachine};

impl LifecycleMachine {
    /// Requires both the pre-world report and the lifecycle to permit world access.
    ///
    /// # Errors
    ///
    /// Returns the earliest pre-world refusal, or a lifecycle refusal if the report permits access.
    pub fn world_access(&self, report: &PreWorldReport) -> Result<(), WorldRefused> {
        report.world_access().map_err(WorldRefused::PreWorld)?;
        self.lifecycle_world_access()
            .map_err(WorldRefused::Lifecycle)
    }
}

/// Why no world opened.
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

impl StructuredFailure for WorldRefused {
    fn failure_kind(&self) -> Option<FailureKind> {
        match self {
            Self::PreWorld(_) => None,
            Self::Lifecycle(error) => error.failure_kind(),
        }
    }

    fn lifecycle_state(&self) -> LifecycleState {
        match self {
            Self::PreWorld(_) => LifecycleState::Discovered,
            Self::Lifecycle(error) => error.lifecycle_state(),
        }
    }

    fn quotes_untrusted_text(&self) -> bool {
        match self {
            Self::PreWorld(_) => false,
            Self::Lifecycle(error) => error.quotes_untrusted_text(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fmt;

    use ravel_core::{
        FailureEnvironment, FailureKind, LifecycleState, Side, SideSet, StructuredFailure,
    };
    use ravel_observability::{PreWorldReport, Subject};

    use super::WorldRefused;
    use crate::lifecycle::{LifecycleEntry, LifecycleError, LifecycleMachine, Requirement};

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

        fn quotes_untrusted_text(&self) -> bool {
            false
        }
    }

    use crate::test_support::id;

    fn report() -> PreWorldReport {
        PreWorldReport::new(FailureEnvironment {
            ravel: "0.1.0".parse().expect("a valid version"),
            minecraft: "26.0.0".to_owned(),
            platform: "linux".to_owned(),
            architecture: "x86_64".to_owned(),
        })
    }

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
        crate::test_support::advance_through(&mut machine, LifecycleState::Running);
        machine
    }

    #[test]
    fn a_running_set_with_a_clean_report_opens_a_world() {
        assert_eq!(running().world_access(&report()), Ok(()));
    }

    #[test]
    fn a_refusal_the_machine_never_saw_still_withholds_the_world() {
        let machine = running();
        let mut report = report();
        report.refuse(&Subject::of(id("example_mod")), &Unresolvable);

        let refused = machine
            .world_access(&report)
            .expect_err("the report withholds it");
        assert!(matches!(refused, WorldRefused::PreWorld(_)), "{refused}");
        assert!(
            refused
                .to_string()
                .contains("`example_mod` requires `example_library`"),
            "{refused}"
        );
        assert!(
            machine.lifecycle_world_access().is_ok(),
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
            .world_access(&report())
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
                .world_access(&report)
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
            machine.world_access(&refused_report).expect_err("refused"),
            machine.world_access(&report()).expect_err("not running"),
        ] {
            assert!(refusal.source().is_some(), "{refusal}");
        }
    }

    #[test]
    fn world_refusals_preserve_structured_failure_classification() {
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![LifecycleEntry {
                mod_id: id("example_mod"),
                sides: SideSet::new([Side::Client]).expect("one side"),
                requirement: Requirement::Required,
                saved_data_versions: BTreeSet::new(),
            }],
        )
        .expect("valid plan");
        machine
            .advance_all(LifecycleState::Validated)
            .expect("validation succeeds");
        let lifecycle = machine.world_access(&report()).expect_err("not running");
        assert_eq!(lifecycle.failure_kind(), None);
        assert_eq!(lifecycle.lifecycle_state(), LifecycleState::Validated);
        assert!(!lifecycle.quotes_untrusted_text());

        let mut report = report();
        report.refuse(&Subject::unidentified(), &Unresolvable);
        let pre_world = running().world_access(&report).expect_err("report refused");
        assert_eq!(pre_world.failure_kind(), None);
        assert_eq!(pre_world.lifecycle_state(), LifecycleState::Discovered);
        assert!(!pre_world.quotes_untrusted_text());
    }
}
