#![forbid(unsafe_code)]

//! Version-neutral acceptance scenarios, observations, and transcript verdicts.
//!
//! This crate models plans and recorded evidence; it does not execute directives
//! or connect to a live game. Callers evaluate assertions against observations.

use core::fmt;

use ravel_core::{RegistryKey, Side};
use ravel_host_api::GameEvent;

/// A named, single-sided sequence of steps and expected progression checkpoints.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Scenario {
    name: String,
    side: Side,
    steps: Vec<Step>,
    checkpoints: Vec<ProgressionCheckpoint>,
}

impl Scenario {
    /// Creates a scenario with no steps or checkpoints.
    #[must_use]
    pub fn new(name: impl Into<String>, side: Side) -> Self {
        Self {
            name: name.into(),
            side,
            steps: Vec::new(),
            checkpoints: Vec::new(),
        }
    }

    /// Appends a step in execution order.
    #[must_use]
    pub fn with_step(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

    /// Appends an expected progression checkpoint in arrival order.
    #[must_use]
    pub fn with_checkpoint(mut self, checkpoint: ProgressionCheckpoint) -> Self {
        self.checkpoints.push(checkpoint);
        self
    }

    /// The scenario's name, as an evidence record and a report refer to it.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The side the scenario runs on.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// The steps in the order they run.
    #[must_use]
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// The progression checkpoints in the order they must be reached.
    #[must_use]
    pub fn checkpoints(&self) -> &[ProgressionCheckpoint] {
        &self.checkpoints
    }
}

/// An optional directive followed by an assertion, labelled with its intended event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Step {
    label: String,
    await_event: GameEvent,
    directive: Option<Directive>,
    assertion: Assertion,
}

impl Step {
    /// Creates a step; `None` makes it observation-only.
    #[must_use]
    pub fn new(
        label: impl Into<String>,
        await_event: GameEvent,
        directive: Option<Directive>,
        assertion: Assertion,
    ) -> Self {
        Self {
            label: label.into(),
            await_event,
            directive,
            assertion,
        }
    }

    /// The step's label, as a transcript names it.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The occasion the step waits for before it acts and asserts.
    #[must_use]
    pub const fn await_event(&self) -> GameEvent {
        self.await_event
    }

    /// The directive the step issues, if it issues one.
    #[must_use]
    pub const fn directive(&self) -> Option<&Directive> {
        self.directive.as_ref()
    }

    /// The assertion the step's success is decided by.
    #[must_use]
    pub const fn assertion(&self) -> &Assertion {
        &self.assertion
    }
}

/// A planned outward instruction, represented as data rather than a live adapter call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Directive {
    /// Runs a mod-owned command with uninterpreted, whitespace-split arguments.
    RunCommand {
        /// The claimed key whose root literal is invoked.
        key: RegistryKey,
        /// What follows the root literal.
        arguments: Vec<String>,
    },
    /// Grants prerequisite items without requiring the walk to obtain them.
    GrantItem {
        /// The declared key of the item to grant.
        key: RegistryKey,
        /// How many to grant; zero is a recorded no-op.
        count: u32,
    },
    /// Advances server ticks before the step asserts.
    AdvanceTicks {
        /// How many game ticks to advance, independent of wall-clock time.
        ticks: u32,
    },
}

impl Directive {
    /// The stable directive kind used by transcripts and reports.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::RunCommand { .. } => "run-command",
            Self::GrantItem { .. } => "grant-item",
            Self::AdvanceTicks { .. } => "advance-ticks",
        }
    }
}

/// A predicate over a captured [`Observation`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Assertion {
    /// The player holds at least `count` of the item declared under `key`.
    HoldsAtLeast {
        /// The declared key of the item to count.
        key: RegistryKey,
        /// The minimum acceptable count.
        count: u32,
    },
    /// The declared block is present at least once, without specifying a coordinate.
    BlockPresent {
        /// The declared key of the block to look for.
        key: RegistryKey,
    },
    /// The observation explicitly reports the checkpoint as reached.
    CheckpointReached {
        /// The checkpoint the observation must report reached.
        checkpoint: ProgressionCheckpoint,
    },
    /// Every inner assertion holds; an empty list holds.
    All(Vec<Self>),
}

impl Assertion {
    /// Evaluates the assertion without waiting or changing the observation.
    #[must_use]
    pub fn evaluate(&self, observation: &Observation) -> bool {
        match self {
            Self::HoldsAtLeast { key, count } => observation.held_count(key) >= *count,
            Self::BlockPresent { key } => observation.block_present(key),
            Self::CheckpointReached { checkpoint } => observation.checkpoint_reached(*checkpoint),
            Self::All(inner) => inner.iter().all(|a| a.evaluate(observation)),
        }
    }
}

/// A coarse milestone in a pack's ordered progression.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProgressionCheckpoint {
    /// A fresh world has been entered and the walk has begun.
    WorldEntered,
    /// The walk has crafted or obtained its first powered machine.
    FirstMachine,
    /// The walk has a working automation loop feeding itself.
    FirstAutomation,
    /// The walk has reached the dimension the pack's progression centres on.
    KeyDimensionReached,
    /// The walk has reached the pack's declared end-state and is complete.
    ProgressionComplete,
}

impl ProgressionCheckpoint {
    /// Every checkpoint, in the order a walk is expected to reach them.
    pub const ALL: [Self; 5] = [
        Self::WorldEntered,
        Self::FirstMachine,
        Self::FirstAutomation,
        Self::KeyDimensionReached,
        Self::ProgressionComplete,
    ];

    /// The checkpoint's canonical lowercase evidence name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorldEntered => "world-entered",
            Self::FirstMachine => "first-machine",
            Self::FirstAutomation => "first-automation",
            Self::KeyDimensionReached => "key-dimension-reached",
            Self::ProgressionComplete => "progression-complete",
        }
    }

    /// The checkpoint's zero-based position on the progression spine.
    #[must_use]
    pub const fn order(self) -> u8 {
        match self {
            Self::WorldEntered => 0,
            Self::FirstMachine => 1,
            Self::FirstAutomation => 2,
            Self::KeyDimensionReached => 3,
            Self::ProgressionComplete => 4,
        }
    }
}

impl fmt::Display for ProgressionCheckpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A snapshot of declared item counts, present blocks, and reached checkpoints.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Observation {
    held: Vec<(RegistryKey, u32)>,
    present_blocks: Vec<RegistryKey>,
    reached: Vec<ProgressionCheckpoint>,
}

impl Observation {
    /// An empty reading: no declared key is held or present.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Sets the held count for `key`, replacing any previous count.
    #[must_use]
    pub fn holding(mut self, key: RegistryKey, count: u32) -> Self {
        if let Some(entry) = self.held.iter_mut().find(|(k, _)| *k == key) {
            entry.1 = count;
        } else {
            self.held.push((key, count));
        }
        self
    }

    /// Records that the block declared under `key` is present in the world.
    #[must_use]
    pub fn with_block(mut self, key: RegistryKey) -> Self {
        if !self.present_blocks.contains(&key) {
            self.present_blocks.push(key);
        }
        self
    }

    /// Records that `checkpoint` has been reached.
    #[must_use]
    pub fn reaching(mut self, checkpoint: ProgressionCheckpoint) -> Self {
        if !self.reached.contains(&checkpoint) {
            self.reached.push(checkpoint);
        }
        self
    }

    /// How many of the item declared under `key` the player holds, or zero.
    #[must_use]
    pub fn held_count(&self, key: &RegistryKey) -> u32 {
        self.held
            .iter()
            .find(|(k, _)| k == key)
            .map_or(0, |(_, c)| *c)
    }

    /// Whether the block declared under `key` is present.
    #[must_use]
    pub fn block_present(&self, key: &RegistryKey) -> bool {
        self.present_blocks.contains(key)
    }

    /// Whether `checkpoint` has been reached.
    #[must_use]
    pub fn checkpoint_reached(&self, checkpoint: ProgressionCheckpoint) -> bool {
        self.reached.contains(&checkpoint)
    }

    /// The checkpoints reached, in the order they were recorded.
    #[must_use]
    pub fn reached(&self) -> &[ProgressionCheckpoint] {
        &self.reached
    }
}

/// A decided pass or failure, or a harness fault that prevented a decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// The step or run met every assertion it was decided against.
    Passed,
    /// A named assertion did not hold. The string is the failing step's label.
    Failed(String),
    /// The run could not reach a verdict, naming why. This is a harness
    /// fault, not a game failure.
    Errored(String),
}

impl Verdict {
    /// Whether this verdict is [`Verdict::Passed`].
    #[must_use]
    pub const fn is_passed(&self) -> bool {
        matches!(self, Self::Passed)
    }
}

/// An append-only record of step outcomes that determines the run verdict.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Transcript {
    entries: Vec<StepOutcome>,
}

/// One step's recorded outcome within a [`Transcript`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepOutcome {
    /// The step's label.
    pub label: String,
    /// The verdict the step resolved to.
    pub verdict: Verdict,
}

impl Transcript {
    /// An empty transcript, before any step has run.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends `outcome` in the order steps ran.
    pub fn record(&mut self, outcome: StepOutcome) {
        self.entries.push(outcome);
    }

    /// The recorded step outcomes, in order.
    #[must_use]
    pub fn entries(&self) -> &[StepOutcome] {
        &self.entries
    }

    /// Returns the first error, otherwise the first failure, otherwise a pass.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        if let Some(err) = self
            .entries
            .iter()
            .find(|e| matches!(e.verdict, Verdict::Errored(_)))
        {
            return err.verdict.clone();
        }
        if let Some(failed) = self
            .entries
            .iter()
            .find(|e| matches!(e.verdict, Verdict::Failed(_)))
        {
            return failed.verdict.clone();
        }
        Verdict::Passed
    }

    /// Whether reached checkpoints are nondecreasing; empty and partial spines qualify.
    #[must_use]
    pub fn checkpoints_in_order(reached: &[ProgressionCheckpoint]) -> bool {
        reached.windows(2).all(|w| w[0].order() <= w[1].order())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::str::FromStr;

    fn key(kind: &str, path: &str) -> RegistryKey {
        RegistryKey::from_str(&format!("{kind}:ravel_test:{path}")).expect("valid registry key")
    }

    #[test]
    fn empty_scenario_is_a_valid_plan() {
        let scenario = Scenario::new("empty", Side::Client);
        assert_eq!(scenario.name(), "empty");
        assert_eq!(scenario.side(), Side::Client);
        assert_eq!(scenario.steps(), []);
        assert_eq!(scenario.checkpoints(), []);
    }

    #[test]
    fn scenario_preserves_step_and_checkpoint_order() {
        let scenario = Scenario::new("walk", Side::IntegratedServer)
            .with_checkpoint(ProgressionCheckpoint::WorldEntered)
            .with_checkpoint(ProgressionCheckpoint::FirstMachine)
            .with_step(Step::new(
                "enter",
                GameEvent::ServerTick,
                None,
                Assertion::CheckpointReached {
                    checkpoint: ProgressionCheckpoint::WorldEntered,
                },
            ));
        assert_eq!(scenario.steps().len(), 1);
        assert_eq!(
            scenario.checkpoints(),
            &[
                ProgressionCheckpoint::WorldEntered,
                ProgressionCheckpoint::FirstMachine
            ]
        );
    }

    #[test]
    fn observation_holding_replaces_rather_than_accumulates() {
        let k = key("item", "widget");
        let observation = Observation::empty()
            .holding(k.clone(), 3)
            .holding(k.clone(), 7);
        assert_eq!(observation.held_count(&k), 7);
    }

    #[test]
    fn observation_block_and_checkpoint_are_deduplicated() {
        let block = key("block", "machine");
        let observation = Observation::empty()
            .with_block(block.clone())
            .with_block(block.clone())
            .reaching(ProgressionCheckpoint::FirstMachine)
            .reaching(ProgressionCheckpoint::FirstMachine);
        assert!(observation.block_present(&block));
        assert_eq!(
            observation.reached(),
            &[ProgressionCheckpoint::FirstMachine]
        );
    }

    #[test]
    fn holds_at_least_boundary() {
        let k = key("item", "ingot");
        let observation = Observation::empty().holding(k.clone(), 4);
        assert!(
            Assertion::HoldsAtLeast {
                key: k.clone(),
                count: 4
            }
            .evaluate(&observation)
        );
        assert!(
            Assertion::HoldsAtLeast {
                key: k.clone(),
                count: 3
            }
            .evaluate(&observation)
        );
        assert!(!Assertion::HoldsAtLeast { key: k, count: 5 }.evaluate(&observation));
    }

    #[test]
    fn empty_all_assertion_holds() {
        assert!(Assertion::All(Vec::new()).evaluate(&Observation::empty()));
    }

    #[test]
    fn all_assertion_requires_every_member() {
        let ingot = key("item", "ingot");
        let machine = key("block", "machine");
        let observation = Observation::empty()
            .holding(ingot.clone(), 2)
            .with_block(machine.clone());
        let all = Assertion::All(vec![
            Assertion::HoldsAtLeast {
                key: ingot,
                count: 1,
            },
            Assertion::BlockPresent { key: machine },
        ]);
        assert!(all.evaluate(&observation));
        let missing = Assertion::All(vec![Assertion::BlockPresent {
            key: key("block", "absent"),
        }]);
        assert!(!missing.evaluate(&observation));
    }

    #[test]
    fn transcript_verdict_prefers_error_then_failure_then_pass() {
        let mut all_pass = Transcript::new();
        all_pass.record(StepOutcome {
            label: "a".into(),
            verdict: Verdict::Passed,
        });
        all_pass.record(StepOutcome {
            label: "b".into(),
            verdict: Verdict::Passed,
        });
        assert_eq!(all_pass.verdict(), Verdict::Passed);

        let mut with_failure = all_pass.clone();
        with_failure.record(StepOutcome {
            label: "c".into(),
            verdict: Verdict::Failed("c".into()),
        });
        assert_eq!(with_failure.verdict(), Verdict::Failed("c".into()));

        let mut with_error = with_failure.clone();
        with_error.record(StepOutcome {
            label: "d".into(),
            verdict: Verdict::Errored("harness".into()),
        });
        assert_eq!(with_error.verdict(), Verdict::Errored("harness".into()));
    }

    #[test]
    fn checkpoint_order_spine_is_monotonic() {
        assert!(Transcript::checkpoints_in_order(
            &ProgressionCheckpoint::ALL
        ));
        assert!(Transcript::checkpoints_in_order(&[
            ProgressionCheckpoint::WorldEntered,
            ProgressionCheckpoint::KeyDimensionReached,
        ]));
        assert!(!Transcript::checkpoints_in_order(&[
            ProgressionCheckpoint::FirstAutomation,
            ProgressionCheckpoint::FirstMachine,
        ]));
        assert!(Transcript::checkpoints_in_order(&[]));
    }

    #[test]
    fn directive_kinds_are_stable() {
        assert_eq!(
            Directive::RunCommand {
                key: key("command", "cmd"),
                arguments: vec![]
            }
            .kind(),
            "run-command"
        );
        assert_eq!(
            Directive::GrantItem {
                key: key("item", "item"),
                count: 0
            }
            .kind(),
            "grant-item"
        );
        assert_eq!(
            Directive::AdvanceTicks { ticks: 20 }.kind(),
            "advance-ticks"
        );
    }

    #[test]
    fn checkpoint_names_and_order_are_consistent() {
        for (index, checkpoint) in ProgressionCheckpoint::ALL.into_iter().enumerate() {
            assert_eq!(usize::from(checkpoint.order()), index);
            assert_ne!(checkpoint.as_str(), "");
        }
    }
}
