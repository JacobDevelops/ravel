//! Progression plans evaluated against explicitly supplied observations, not a live game.

use std::str::FromStr;

use ravel_acceptance::{Assertion, Directive, Observation, Scenario, Step, Transcript};
use ravel_core::{ContentId, RegistryKey, Side};
use ravel_host_api::GameEvent;
use ravel_pack::Progression;

fn key(kind: &str, path: &str) -> RegistryKey {
    RegistryKey::from_str(&format!("{kind}:atm10:{path}")).expect("valid registry key")
}

fn checkpoint(path: &str) -> ContentId {
    ContentId::new("atm10".parse().expect("valid pack id"), path).expect("valid content path")
}

fn progression_walk() -> Scenario {
    Scenario::new("atm10-tts-progression", Side::IntegratedServer)
        .with_checkpoint(checkpoint("world_entered"))
        .with_checkpoint(checkpoint("first_machine"))
        .with_checkpoint(checkpoint("first_automation"))
        .with_checkpoint(checkpoint("key_dimension_reached"))
        .with_checkpoint(checkpoint("progression_complete"))
        .with_step(Step::new(
            "enter-world",
            GameEvent::ServerTick,
            None,
            Assertion::CheckpointReached {
                checkpoint: checkpoint("world_entered"),
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
                    checkpoint: checkpoint("first_machine"),
                },
            ]),
        ))
        .with_step(Step::new(
            "let-automation-settle",
            GameEvent::ServerTick,
            Some(Directive::AdvanceTicks { ticks: 40 }),
            Assertion::All(vec![
                Assertion::CheckpointReached {
                    checkpoint: checkpoint("first_automation"),
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
                checkpoint: checkpoint("key_dimension_reached"),
            },
        ))
        .with_step(Step::new(
            "finish-progression",
            GameEvent::ServerTick,
            None,
            Assertion::CheckpointReached {
                checkpoint: checkpoint("progression_complete"),
            },
        ))
}

#[test]
fn a_full_walks_recorded_observations_satisfy_its_assertions() {
    let scenario = progression_walk();
    let observations = [
        Observation::empty().reaching(checkpoint("world_entered")),
        Observation::empty()
            .with_block(key("item", "machine_core"))
            .reaching(checkpoint("first_machine")),
        Observation::empty()
            .holding(key("item", "ingot"), 8)
            .reaching(checkpoint("first_automation")),
        Observation::empty().reaching(checkpoint("key_dimension_reached")),
        Observation::empty().reaching(checkpoint("progression_complete")),
    ];
    assert_eq!(scenario.steps().len(), observations.len());
    for (step, observation) in scenario.steps().iter().zip(&observations) {
        assert!(step.assertion().evaluate(observation), "{}", step.label());
    }
    let reached: Vec<_> = observations
        .iter()
        .flat_map(Observation::reached)
        .cloned()
        .collect();
    assert!(Transcript::checkpoints_in_order(
        scenario.checkpoints(),
        &reached
    ));
}

#[test]
fn a_missing_checkpoint_does_not_satisfy_the_planned_assertion() {
    let scenario = progression_walk();
    let step = &scenario.steps()[0];
    assert!(!step.assertion().evaluate(&Observation::empty()));
    assert_eq!(step.label(), "enter-world");
}

#[test]
fn acceptance_uses_the_pack_progression_step_identity() {
    let mut progression = Progression::default();
    let start = checkpoint("world_entered");
    let machine = checkpoint("first_machine");
    progression
        .declare(start.clone(), [], None)
        .expect("valid root");
    progression
        .declare(machine.clone(), [start], None)
        .expect("valid machine step");
    let state = progression.state(&machine).expect("declared pack step");
    let scenario = progression_walk();
    assert!(scenario.checkpoints().contains(state.id()));
    let observation = Observation::empty().reaching(state.id().clone());
    assert!(
        Assertion::CheckpointReached {
            checkpoint: state.id().clone(),
        }
        .evaluate(&observation)
    );
}
