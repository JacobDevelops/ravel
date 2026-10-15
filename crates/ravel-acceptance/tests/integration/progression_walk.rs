//! Progression plans evaluated against explicitly supplied observations, not a live game.

use std::str::FromStr;

use ravel_acceptance::{
    Assertion, Directive, Observation, ProgressionCheckpoint, Scenario, Step, Transcript,
};
use ravel_core::{RegistryKey, Side};
use ravel_host_api::GameEvent;

fn key(kind: &str, path: &str) -> RegistryKey {
    RegistryKey::from_str(&format!("{kind}:atm10:{path}")).expect("valid registry key")
}

fn progression_walk() -> Scenario {
    Scenario::new("atm10-tts-progression", Side::IntegratedServer)
        .with_checkpoint(ProgressionCheckpoint::WorldEntered)
        .with_checkpoint(ProgressionCheckpoint::FirstMachine)
        .with_checkpoint(ProgressionCheckpoint::FirstAutomation)
        .with_checkpoint(ProgressionCheckpoint::KeyDimensionReached)
        .with_checkpoint(ProgressionCheckpoint::ProgressionComplete)
        .with_step(Step::new(
            "enter-world",
            GameEvent::ServerTick,
            None,
            Assertion::CheckpointReached {
                checkpoint: ProgressionCheckpoint::WorldEntered,
            },
        ))
        .with_step(Step::new(
            "craft-first-machine",
            GameEvent::ServerTick,
            Some(Directive::GrantItem {
                key: key("item", "machine_core"),
                count: 1,
            }),
            Assertion::All(vec![
                Assertion::BlockPresent {
                    key: key("item", "machine_core"),
                },
                Assertion::CheckpointReached {
                    checkpoint: ProgressionCheckpoint::FirstMachine,
                },
            ]),
        ))
        .with_step(Step::new(
            "let-automation-settle",
            GameEvent::ServerTick,
            Some(Directive::AdvanceTicks { ticks: 40 }),
            Assertion::All(vec![
                Assertion::CheckpointReached {
                    checkpoint: ProgressionCheckpoint::FirstAutomation,
                },
                Assertion::HoldsAtLeast {
                    key: key("item", "ingot"),
                    count: 8,
                },
            ]),
        ))
        .with_step(Step::new(
            "reach-key-dimension",
            GameEvent::ServerTick,
            Some(Directive::RunCommand {
                key: key("command", "dimension_travel"),
                arguments: vec!["sky".to_owned()],
            }),
            Assertion::CheckpointReached {
                checkpoint: ProgressionCheckpoint::KeyDimensionReached,
            },
        ))
}

#[test]
fn a_full_walks_recorded_observations_satisfy_its_assertions() {
    let scenario = progression_walk();
    let observations = [
        Observation::empty().reaching(ProgressionCheckpoint::WorldEntered),
        Observation::empty()
            .with_block(key("item", "machine_core"))
            .reaching(ProgressionCheckpoint::FirstMachine),
        Observation::empty()
            .holding(key("item", "ingot"), 8)
            .reaching(ProgressionCheckpoint::FirstAutomation),
        Observation::empty().reaching(ProgressionCheckpoint::KeyDimensionReached),
    ];
    assert_eq!(scenario.steps().len(), observations.len());
    for (step, observation) in scenario.steps().iter().zip(&observations) {
        assert!(step.assertion().evaluate(observation), "{}", step.label());
    }
    assert!(Transcript::checkpoints_in_order(scenario.checkpoints()));
}

#[test]
fn a_missing_checkpoint_does_not_satisfy_the_planned_assertion() {
    let scenario = progression_walk();
    let step = &scenario.steps()[0];
    assert!(!step.assertion().evaluate(&Observation::empty()));
    assert_eq!(step.label(), "enter-world");
}
