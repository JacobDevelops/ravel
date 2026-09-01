//! A full progression walk driven against an in-test driver.
//!
//! This exercises the whole `ravel-acceptance` model the way a live run will,
//! with one difference that is the entire point of the crate: the [`GameView`]
//! and [`GameDriver`] here are a test's, backed by an in-memory world rather
//! than a launched game. The same [`Scenario`] runs unchanged against a live
//! driver once M8-W0 carries a directive outward and a hook value projects onto
//! an [`Observation`] — which is why the walk is written here now, to prove the
//! model is expressive enough for a real progression before either exists.
//!
//! What it demonstrates, and what a live run inherits:
//!
//! - a directive's effect lands *after* the answer, so a step that grants an
//!   item asserts nothing about it and a following `AdvanceTicks` step is where
//!   the world has settled enough to assert;
//! - the checkpoint spine is walked in order, and reaching a later checkpoint
//!   before an earlier one is a failure the transcript reports;
//! - a driver that cannot carry a directive turns the step into `Errored`, kept
//!   distinct from a game that simply did not reach a state (`Failed`).

use std::collections::BTreeMap;
use std::str::FromStr;

use ravel_acceptance::{
    Assertion, Directive, GameDriver, GameView, Observation, ProgressionCheckpoint, Scenario, Step,
    Transcript, Verdict, run,
};
use ravel_core::{RegistryKey, Side};
use ravel_host_api::GameEvent;

/// A registry key from `<kind>:atm10:<path>`.
fn key(kind: &str, path: &str) -> RegistryKey {
    RegistryKey::from_str(&format!("{kind}:atm10:{path}")).expect("valid registry key")
}

/// An in-memory world a test driver mutates and observes, standing in for the
/// live projection and outward crossing a launched game would supply.
#[derive(Default)]
struct FakeWorld {
    held: BTreeMap<String, u32>,
    blocks: Vec<RegistryKey>,
    reached: Vec<ProgressionCheckpoint>,
    ticks: u64,
    /// A directive kind the driver is told it cannot carry, to exercise the
    /// `Errored` path without pretending the game failed.
    refuse_kind: Option<&'static str>,
}

impl FakeWorld {
    fn snapshot(&self) -> Observation {
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
        observation
    }
}

impl GameView for FakeWorld {
    fn observe(&self) -> Result<Observation, String> {
        Ok(self.snapshot())
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
                // Granting the first machine item is what puts the block in the
                // world and advances the machine checkpoint, the way crafting it
                // would in a real walk.
                if key.to_string().contains("machine") {
                    self.blocks.push(key.clone());
                    self.reached.push(ProgressionCheckpoint::FirstMachine);
                }
            }
            Directive::RunCommand { key, .. } => {
                // The walk's one command reaches the pack's key dimension.
                if key.to_string().contains("dimension") {
                    self.reached
                        .push(ProgressionCheckpoint::KeyDimensionReached);
                }
            }
            Directive::AdvanceTicks { ticks } => {
                self.ticks += u64::from(*ticks);
                // Automation feeds itself once the machine has run a while.
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

/// The written walk: enter, craft a machine, let automation run, reach the key
/// dimension, complete. It runs on the integrated server, because every
/// progression decision is the server's.
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
        // The world is entered before the walk's first step asserts it, the way
        // a live runner enters the world before driving.
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
    // Every step recorded a verdict, none skipped.
    assert_eq!(transcript.entries().len(), scenario.steps().len());
}

#[test]
fn a_walk_that_never_reaches_a_checkpoint_fails_at_that_step() {
    let scenario = progression_walk();
    // A world that never enters: the first step's precondition is unmet, so the
    // walk fails at "enter-world" rather than limping on.
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
        // The driver cannot grant items — a harness fault, not a game that
        // declined to produce the machine.
        refuse_kind: Some("grant-item"),
        ..FakeWorld::default()
    };
    let transcript = run(&scenario, &mut world);

    match transcript.verdict() {
        Verdict::Errored(reason) => assert!(reason.contains("grant-item"), "{reason}"),
        other => panic!("expected an errored verdict, got {other:?}"),
    }
}

#[test]
fn the_effect_lands_after_the_answer_not_during_the_granting_step() {
    // The grant step asserts on the machine (which the grant produces), but the
    // ingot the machine yields is asserted only after the settle step, because
    // "an action happens after the answer, not during it". A world observed
    // immediately after the grant holds no ingot yet.
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
