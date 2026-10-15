//! Progression scenarios exercised against an in-memory driver, not a live game.

use std::collections::BTreeMap;
use std::str::FromStr;

use ravel_acceptance::{
    Assertion, Directive, GameDriver, GameView, Observation, ProgressionCheckpoint, Scenario, Step,
    Transcript, Verdict, run,
};
use ravel_core::{RegistryKey, Side};
use ravel_host_api::GameEvent;

fn key(kind: &str, path: &str) -> RegistryKey {
    RegistryKey::from_str(&format!("{kind}:atm10:{path}")).expect("valid registry key")
}

/// An in-memory world with optional directive refusal.
#[derive(Default)]
struct FakeWorld {
    held: BTreeMap<String, u32>,
    blocks: Vec<RegistryKey>,
    reached: Vec<ProgressionCheckpoint>,
    ticks: u64,
    /// A directive kind to refuse as a driver fault.
    refuse_kind: Option<&'static str>,
}

impl GameView for FakeWorld {
    fn observe(&self) -> Result<Observation, String> {
        let mut observation = Observation::empty();
        for (k, count) in &self.held {
            observation =
                observation.holding(RegistryKey::from_str(k).expect("stored key"), *count);
        }
        for block in &self.blocks {
            observation = observation.with_block(block.clone());
        }
        for checkpoint in &self.reached {
            observation = observation.reaching(*checkpoint);
        }
        Ok(observation)
    }
}

impl GameDriver for FakeWorld {
    fn carry(&mut self, directive: &Directive) -> Result<(), String> {
        if self.refuse_kind == Some(directive.kind()) {
            return Err(format!("driver cannot carry {}", directive.kind()));
        }
        match directive {
            Directive::GrantItem { key, count } => {
                *self.held.entry(key.to_string()).or_default() += *count;
                if key.to_string().contains("machine") {
                    self.blocks.push(key.clone());
                    self.reached.push(ProgressionCheckpoint::FirstMachine);
                }
            }
            Directive::RunCommand { key, .. } => {
                if key.to_string().contains("dimension") {
                    self.reached
                        .push(ProgressionCheckpoint::KeyDimensionReached);
                }
            }
            Directive::AdvanceTicks { ticks } => {
                self.ticks += u64::from(*ticks);
                if self.ticks >= 40
                    && self
                        .blocks
                        .iter()
                        .any(|b| b.to_string().contains("machine"))
                    && !self
                        .reached
                        .contains(&ProgressionCheckpoint::FirstAutomation)
                {
                    self.reached.push(ProgressionCheckpoint::FirstAutomation);
                    *self
                        .held
                        .entry(key("item", "ingot").to_string())
                        .or_default() += 8;
                }
            }
        }
        Ok(())
    }
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
fn a_full_walk_passes_and_reaches_its_checkpoints_in_order() {
    let scenario = progression_walk();
    let mut world = FakeWorld {
        reached: vec![ProgressionCheckpoint::WorldEntered],
        ..FakeWorld::default()
    };
    let transcript = run(&scenario, &mut world);

    assert_eq!(transcript.verdict(), Verdict::Passed, "{transcript:?}");
    assert!(
        Transcript::checkpoints_in_order(&world.reached),
        "reached out of order: {:?}",
        world.reached
    );
    assert_eq!(transcript.entries().len(), scenario.steps().len());
}

#[test]
fn a_walk_that_never_reaches_a_checkpoint_fails_at_that_step() {
    let scenario = progression_walk();
    let mut world = FakeWorld::default();
    let transcript = run(&scenario, &mut world);

    assert_eq!(
        transcript.verdict(),
        Verdict::Failed("enter-world".to_owned())
    );
}

#[test]
fn a_driver_that_cannot_carry_a_directive_errors_rather_than_fails() {
    let scenario = progression_walk();
    let mut world = FakeWorld {
        reached: vec![ProgressionCheckpoint::WorldEntered],
        refuse_kind: Some("grant-item"),
        ..FakeWorld::default()
    };
    let transcript = run(&scenario, &mut world);

    match transcript.verdict() {
        Verdict::Errored(reason) => assert!(reason.contains("grant-item"), "{reason}"),
        other @ (Verdict::Passed | Verdict::Failed(_)) => {
            panic!("expected an errored verdict, got {other:?}")
        }
    }
}

#[test]
fn the_effect_lands_after_the_answer_not_during_the_granting_step() {
    // Ingot production requires ticks after the machine is granted.
    let mut world = FakeWorld {
        reached: vec![ProgressionCheckpoint::WorldEntered],
        ..FakeWorld::default()
    };
    world
        .carry(&Directive::GrantItem {
            key: key("item", "machine_core"),
            count: 1,
        })
        .expect("grant is carried");
    let after_grant = world.observe().expect("observe");
    assert_eq!(after_grant.held_count(&key("item", "ingot")), 0);
    assert!(after_grant.block_present(&key("item", "machine_core")));
}
