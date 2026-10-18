use ravel_core::{FailureKind, LifecycleState, ModId, Redaction, Side, StructuredFailure};

use super::ResolutionError;

fn id(text: &str) -> ModId {
    text.parse().expect("a valid mod id")
}

#[test]
fn every_failure_reports_its_spec_outcome_and_the_state_it_failed_in() {
    for (error, expected) in [
        (
            ResolutionError::DuplicateIdentity { id: id("a") },
            FailureKind::DuplicateIdentity,
        ),
        (
            ResolutionError::MissingDependency {
                dependant: id("a"),
                dependency: id("b"),
                required: "*".parse().expect("a valid range"),
            },
            FailureKind::MissingRequiredDependency,
        ),
        (
            ResolutionError::SideMismatch {
                dependant: id("a"),
                dependency: id("b"),
                side: Side::Client,
            },
            FailureKind::SideMismatch,
        ),
        (
            ResolutionError::UnsatisfiedVersion {
                dependant: id("a"),
                dependency: id("b"),
                required: "*".parse().expect("a valid range"),
                installed: "1.0.0".parse().expect("a valid version"),
            },
            FailureKind::IncompatibleVersion,
        ),
        (
            ResolutionError::IncompatiblePair {
                declaring: id("a"),
                rejected: id("b"),
                refused: "*".parse().expect("a valid range"),
                installed: "1.0.0".parse().expect("a valid version"),
            },
            FailureKind::IncompatibleVersion,
        ),
        (
            ResolutionError::Cycle {
                members: vec![id("a")],
            },
            FailureKind::DependencyCycle,
        ),
    ] {
        assert_eq!(error.failure_kind(), Some(expected), "{error}");
        assert_eq!(error.lifecycle_state(), LifecycleState::Validated);
        assert_eq!(error.diagnostic().redaction(), Redaction::Raw);
        assert_eq!(error.diagnostic().text(), error.to_string());
        assert!(!error.quotes_untrusted_text(), "{error}");
        assert!(!error.to_string().contains('/'), "{error}");
    }
}

#[test]
fn every_failure_renders_the_rule_it_breaks() {
    for (error, expected) in [
        (
            ResolutionError::DuplicateIdentity {
                id: id("example_mod"),
            },
            "two packages claim the mod id `example_mod`",
        ),
        (
            ResolutionError::MissingDependency {
                dependant: id("example_mod"),
                dependency: id("example_library"),
                required: "^1.0.0".parse().expect("a valid range"),
            },
            "`example_mod` requires `example_library` `^1.0.0`, which is not installed",
        ),
        (
            ResolutionError::SideMismatch {
                dependant: id("example_mod"),
                dependency: id("example_library"),
                side: Side::IntegratedServer,
            },
            "`example_mod` requires `example_library`, which does not load on the \
             integrated-server side",
        ),
        (
            ResolutionError::UnsatisfiedVersion {
                dependant: id("example_mod"),
                dependency: id("example_library"),
                required: "^2.0.0".parse().expect("a valid range"),
                installed: "1.0.0".parse().expect("a valid version"),
            },
            "`example_mod` requires `example_library` `^2.0.0`, but `example_library` 1.0.0 \
             is installed",
        ),
        (
            ResolutionError::IncompatiblePair {
                declaring: id("example_mod"),
                rejected: id("example_rival"),
                refused: "*".parse().expect("a valid range"),
                installed: "1.0.0".parse().expect("a valid version"),
            },
            "`example_mod` cannot load with `example_rival` `*`, and `example_rival` 1.0.0 \
             is installed",
        ),
    ] {
        assert_eq!(error.to_string(), expected);
    }
}

#[test]
fn every_side_is_named_the_way_the_package_schema_spells_it() {
    for (side, expected) in [
        (Side::Client, "client"),
        (Side::IntegratedServer, "integrated-server"),
        (Side::DedicatedServer, "dedicated-server"),
    ] {
        assert_eq!(side.as_str(), expected);
    }
}
