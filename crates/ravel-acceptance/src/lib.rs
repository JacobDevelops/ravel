#![forbid(unsafe_code)]

//! The version-neutral model for an agentic acceptance run.
//!
//! An acceptance run is how M8-W4's fresh-world scenarios and M10's progression
//! walk are driven by software rather than by a person at the keyboard: a
//! [`Scenario`] is a named sequence of [`Step`]s, each step observes the game
//! through the events the loader already reports and asks the game to do
//! something through the crossing the loader already carries, and a step's
//! success is an [`Assertion`] over an [`Observation`] the run captured. A
//! [`ProgressionCheckpoint`] is the coarse, ordered spine M10 asks a walk to
//! reach — the milestones a pack's intended progression passes through — and a
//! [`Transcript`] is the ordered record of what a run did and decided, which is
//! the evidence a gate reads.
//!
//! # What this crate is and is not
//!
//! Everything here is decidable without a game, exactly as
//! [`ravel_host_api::GameEvent`] ships the declaration of an event and no
//! payload and [`ravel_host_api::Point`] ships a place a host may call out from
//! and no call. This crate models the *shape* of an acceptance run — the steps,
//! the ordering, the assertions, the checkpoint spine, the verdict arithmetic —
//! and holds none of the two things a run needs that only a running game
//! supplies: a real [`Observation`] of live world state, and a driver that
//! turns a [`Directive`] into the outward instruction the Java adapter performs.
//!
//! Those two are not this crate's to hold, and the reason is the same one
//! `docs/architecture/dependency-rules.md` gives for every version boundary. A
//! live observation is a projection from a hook's value, and the wiring that
//! connects a hook to the loader has landed without making one possible: no hook
//! `crates/ravel-bootstrap/src/runtime.rs` binds carries world state a
//! projection could read, because every reference parameter crosses as a
//! `HookValue::Handle` offering a declared class and a null check, which is what
//! `ROADMAP.md` records as the open question a reviewed hook target has to
//! close; and a directive becomes an effect through
//! `docs/decisions/carrying-an-action-outward.md`'s mechanism, whose outbound leg
//! `ROADMAP.md`'s M8-W0 owns and which no launched game has performed yet.
//! This crate names both boundaries as traits a driver implements
//! — [`GameView`] and [`GameDriver`] — so a run is written, ordered and asserted
//! against them now, and the driver that makes them real is built against a
//! launched game later without this model changing. [`run`] is the loop that
//! drives a scenario against them, so what a live driver has to supply is the
//! two traits and nothing else: the order of carry, observe, evaluate and
//! record is decided here rather than re-derived by each caller.
//!
//! # What the run never does
//!
//! A run answers the game the way a mod does and no other way. It never calls a
//! game method: a [`Directive`] is data a [`GameDriver`] hands to the adapter,
//! which is `docs/decisions/carrying-an-action-outward.md`'s decision applied to
//! a test rather than to a mod, so no `#![forbid(unsafe_code)]` is weakened and
//! no new crossing is opened. It reads the modded world through structured
//! observations rather than by parsing rendered pixels, so a scenario names a
//! modded machine, item or quest by the key the pack declared it under rather
//! than by what it looks like on a screen — the fork
//! `docs/architecture/agentic-acceptance-harness.md` resolves. And it decides a
//! verdict from the transcript it recorded rather than from a claim, because a
//! green that measures the harness rather than the game is the failure
//! `ROADMAP.md`'s M8-W0 note exists to prevent.

use core::fmt;

use ravel_core::{RegistryKey, Side};
use ravel_host_api::GameEvent;

/// A named acceptance scenario: an ordered sequence of steps and the spine of
/// progression checkpoints it is expected to reach.
///
/// A scenario carries no game state and performs nothing on construction. It is
/// the written plan a [`GameDriver`] runs and a [`Transcript`] records against,
/// and it is `Clone` so a suite may run the same plan against more than one
/// world without a second author.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Scenario {
    name: String,
    side: Side,
    steps: Vec<Step>,
    checkpoints: Vec<ProgressionCheckpoint>,
}

impl Scenario {
    /// Names a scenario that runs on `side` with no steps and no checkpoints.
    ///
    /// The empty scenario is a valid plan: it reaches every checkpoint it names
    /// (none) and every step it holds passes (none), so an empty run is a
    /// [`Verdict::Passed`] over nothing rather than a special case. A suite that
    /// wants an empty scenario to mean "unwritten" records that as a checkpoint
    /// it fails to reach, not as a property of the empty plan.
    #[must_use]
    pub fn new(name: impl Into<String>, side: Side) -> Self {
        Self {
            name: name.into(),
            side,
            steps: Vec::new(),
            checkpoints: Vec::new(),
        }
    }

    /// Appends `step` and returns the scenario, so a plan reads as the order it
    /// runs in.
    #[must_use]
    pub fn with_step(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

    /// Appends `checkpoint` to the progression spine and returns the scenario.
    ///
    /// Checkpoints are the coarse ordered milestones M10 asks a walk to reach,
    /// and their order here is the order they must be reached in. A checkpoint
    /// reached out of order is a run that did something the plan did not expect,
    /// which [`Transcript::verdict`] reports rather than tolerates.
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
    ///
    /// A scenario is single-sided because a step's observation and directive are
    /// both a side's, and the side an event happens on is
    /// [`GameEvent::happens_on`]'s to say. A walk that needs both sides is two
    /// scenarios whose ordering a suite owns, not one scenario that straddles a
    /// boundary the loader keeps.
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

/// One step of a scenario: wait for an occasion, ask the game to do something,
/// then assert over what the game then showed.
///
/// A step is the smallest unit a transcript records a verdict for. Its three
/// parts map onto the three things an acceptance run is allowed to do — observe
/// through [`GameEvent`], act through a [`Directive`], and decide through an
/// [`Assertion`] — and a step never does a fourth.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Step {
    label: String,
    await_event: GameEvent,
    directive: Option<Directive>,
    assertion: Assertion,
}

impl Step {
    /// Records a step labelled `label` that waits for `await_event`, optionally
    /// issues `directive`, and passes when `assertion` holds over the
    /// observation captured after the directive.
    ///
    /// A step with no directive is an observation-only step: it waits for the
    /// occasion and asserts over the world as it then is, asking the game to do
    /// nothing. That is how a scenario checks a precondition or a settling state
    /// without perturbing it.
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

/// A thing a step asks the game to do, carried outward as data.
///
/// A directive is `docs/decisions/carrying-an-action-outward.md`'s crossing put
/// to a test's use: a [`GameDriver`] hands it to the receiver-aware Java adapter,
/// which decodes it and performs it against the game. No variant is a call Rust
/// makes, and every variant names its target by the key a pack declared it under
/// rather than by a game type this crate may not name.
///
/// The set is small on purpose. It holds the actions M10's progression walk is
/// written in terms of, and a walk that needs an action this does not name is a
/// gap this enum grows to admit under the same review a hook target gets, not a
/// pixel a driver clicks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Directive {
    /// Run a command a mod owns, by the claimed key of its root literal and the
    /// arguments that follow it.
    ///
    /// The arguments are text the way [`ravel_host_api::CommandInvocation`]'s
    /// are: split on whitespace and not otherwise interpreted, because the
    /// command's grammar is the mod's and not this crate's.
    RunCommand {
        /// The claimed key whose root literal is invoked.
        key: RegistryKey,
        /// What follows the root literal.
        arguments: Vec<String>,
    },
    /// Place the item a pack declared under `key` into the acting player's
    /// inventory, in the given count, so a scenario can reach a progression
    /// state its walk depends on without re-walking every prior step.
    ///
    /// This is a test affordance rather than a game mechanic, which is why it is
    /// a directive a driver may refuse: a run that must prove the walk itself
    /// declares no `GrantItem` and reaches the item by playing, and a run that
    /// only needs to reach a *later* state grants the prerequisite and says so
    /// in its transcript.
    GrantItem {
        /// The declared key of the item to grant.
        key: RegistryKey,
        /// How many to grant. Zero grants nothing and is a no-op the driver
        /// still records, so a transcript never has a silent gap where a count
        /// was meant.
        count: u32,
    },
    /// Wait for `ticks` server ticks to elapse before the step asserts, so a
    /// scenario can let a machine or a world settle without asserting on the
    /// exact tick an effect lands.
    AdvanceTicks {
        /// How many ticks to let elapse. This is a count, not a duration: a
        /// tick is the game's unit and a wall-clock time would drift with the
        /// host's speed, which M9 measures and this crate must not assume.
        ticks: u32,
    },
}

impl Directive {
    /// A short, stable kind name for the directive, as a transcript and a report
    /// group by.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::RunCommand { .. } => "run-command",
            Self::GrantItem { .. } => "grant-item",
            Self::AdvanceTicks { .. } => "advance-ticks",
        }
    }
}

/// What a step asserts over the observation it captured.
///
/// An assertion is a decidable predicate over an [`Observation`], evaluated by
/// [`Assertion::evaluate`], which is the whole of how a step decides a verdict.
/// It names what it checks by a declared key so it reads against the modded
/// content a pack declared, and it holds no game type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Assertion {
    /// The player holds at least `count` of the item declared under `key`.
    HoldsAtLeast {
        /// The declared key of the item to count.
        key: RegistryKey,
        /// The minimum acceptable count.
        count: u32,
    },
    /// The world reports the block declared under `key` present at least once,
    /// which is how a scenario checks that a placement or a generation happened
    /// without naming a coordinate a version's world does not promise.
    BlockPresent {
        /// The declared key of the block to look for.
        key: RegistryKey,
    },
    /// The progression checkpoint named by `checkpoint` is reached in the
    /// observation, which is how a walk asserts it arrived rather than inferring
    /// arrival from an item it happens to hold.
    CheckpointReached {
        /// The checkpoint the observation must report reached.
        checkpoint: ProgressionCheckpoint,
    },
    /// Every one of the inner assertions holds. An empty `All` holds, so it is
    /// the identity a builder can start from rather than a special case a caller
    /// guards.
    All(Vec<Assertion>),
}

impl Assertion {
    /// Evaluates the assertion against `observation`, returning whether it holds.
    ///
    /// This is total and side-effect-free: it reads the observation and decides,
    /// and it neither waits nor acts. A driver evaluates it after the step's
    /// directive has been carried and the awaited event has been seen, so what
    /// it reads is the world as the step left it.
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

/// A coarse, ordered progression milestone a walk is expected to reach.
///
/// The set is a spine rather than an inventory: it names the milestones M10's
/// progression acceptance asks a walk to pass through, in the order a pack's
/// intended progression passes them, and a pack that reaches a milestone the
/// spine names out of order has done something the plan did not expect. The
/// concrete pack-specific milestones a pack adds live in the pack's own
/// declaration, not in this crate; what is here is the shape every pack's spine
/// shares — a start, a first machine, a first automation, the dimension the pack
/// is named for, and an end-state the walk is complete at.
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

    /// The checkpoint's canonical lowercase name, as a transcript and an
    /// evidence record refer to it.
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

    /// This checkpoint's position on the spine, counting from zero, so an
    /// ordering check reads as a comparison rather than a table lookup.
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

/// A structured reading of the game's state at a moment, as a driver captures it
/// and an [`Assertion`] reads it.
///
/// An observation is what a live [`GameView`] produces and what this crate lets
/// a test build directly so a scenario is decidable without a game. It holds
/// declared keys and counts rather than game objects, which is what keeps this
/// crate version-neutral: a real driver projects a hook's value onto these
/// fields, and the projection — not this type — is the part `ravel-host-api`
/// records as unstarted.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Observation {
    held: Vec<(RegistryKey, u32)>,
    present_blocks: Vec<RegistryKey>,
    reached: Vec<ProgressionCheckpoint>,
}

impl Observation {
    /// An empty observation: nothing held, no block present, no checkpoint
    /// reached. This is a fresh world's floor, not a missing reading — a driver
    /// that failed to read reports that as a driver error rather than as an
    /// empty observation.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Records that the player holds `count` of the item declared under `key`.
    ///
    /// A second record for the same key replaces the first rather than adding to
    /// it, because an observation is a snapshot of a count and not a running
    /// tally: two readings of the same inventory are two observations, not one
    /// that accumulates.
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

/// The verdict a step or a whole run resolves to.
///
/// A verdict is decided from a transcript, never claimed. `Passed` and `Failed`
/// are a decided outcome; `Errored` is a run that could not decide, which is a
/// distinct thing from a decided failure because it names a fault in the harness
/// or the driver rather than in the game — a gate that cannot tell the two apart
/// would count a broken driver as a passing game.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// The step or run met every assertion it was decided against.
    Passed,
    /// A named assertion did not hold. The string is the failing step's label.
    Failed(String),
    /// The run could not reach a verdict, naming why. This is a harness or
    /// driver fault, not a game failure.
    Errored(String),
}

impl Verdict {
    /// Whether this verdict is [`Verdict::Passed`].
    #[must_use]
    pub const fn is_passed(&self) -> bool {
        matches!(self, Self::Passed)
    }
}

/// The ordered record of what a run did and decided, and the evidence a gate
/// reads.
///
/// A transcript is append-only through [`Transcript::record`] and decides its
/// verdict from what it holds through [`Transcript::verdict`]. It is the one
/// place a run's outcome is computed, so a report and a gate read the same
/// arithmetic rather than two that can disagree.
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

    /// The run's verdict, decided from the recorded outcomes.
    ///
    /// A run errors if any step errored — a harness fault dominates, because a
    /// run that could not decide a step cannot honestly claim the steps after
    /// it. Otherwise a run fails at the first failing step, and passes only when
    /// every step passed. This is deliberately not "most steps passed": a walk
    /// with one broken checkpoint is a walk that did not complete, which is the
    /// verdict M10 asks for.
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

    /// Whether the recorded checkpoints were reached in the spine's order,
    /// against the `expected` spine a scenario declared.
    ///
    /// A walk may reach a checkpoint later than the plan hoped, but it may not
    /// reach a *later* checkpoint before an earlier one it also claims: that is
    /// a progression that happened in an order the pack's intended progression
    /// does not, which M10 treats as a failure to walk the progression rather
    /// than as a walk that finished early. An expected spine the run reached
    /// none of is vacuously in order.
    #[must_use]
    pub fn checkpoints_in_order(reached: &[ProgressionCheckpoint]) -> bool {
        reached.windows(2).all(|w| w[0].order() <= w[1].order())
    }
}

/// A read-only view of live game state a driver projects for a step to assert
/// over.
///
/// This is one of the two boundaries this crate names and does not cross. A real
/// implementation projects a hook's value — `ravel-host-api` records that
/// projection as the unstarted work that lands with hook-to-loader wiring — onto
/// an [`Observation`]. A test implements it directly by returning a built
/// observation, which is how a scenario is exercised without a game.
pub trait GameView {
    /// Captures the current game state as a structured observation, or returns
    /// why it could not. A failure here is a driver fault, and a run turns it
    /// into a [`Verdict::Errored`] rather than a [`Verdict::Failed`].
    ///
    /// # Errors
    ///
    /// Returns a message describing why the state could not be read — the view
    /// was asked on a side the run does not observe, or a projection a live
    /// driver needs is one the loader does not yet carry.
    fn observe(&self) -> Result<Observation, String>;
}

/// A driver that carries a directive outward and lets the game advance.
///
/// This is the second boundary this crate names and does not cross. A directive
/// leaves as `docs/decisions/carrying-an-action-outward.md`'s instruction, which
/// the Java adapter performs; the outbound leg is `ROADMAP.md`'s M8-W0 and no
/// host performs it yet, so the only implementations that exist are a test's.
/// The trait is here so a scenario is written and ordered against it now.
pub trait GameDriver {
    /// Carries `directive` outward as an instruction the adapter performs, or
    /// returns why it could not.
    ///
    /// An action happens after the answer, not during it — the property
    /// `docs/decisions/carrying-an-action-outward.md` records — so a successful
    /// return means the instruction was carried, not that its effect is already
    /// visible. A step that needs the effect to have landed waits for it with a
    /// following [`Directive::AdvanceTicks`] step and asserts then.
    ///
    /// # Errors
    ///
    /// Returns a message describing why the directive could not be carried,
    /// which a run turns into a [`Verdict::Errored`].
    fn carry(&mut self, directive: &Directive) -> Result<(), String>;
}

/// Runs `scenario` against `world`, returning the [`Transcript`] of what it did
/// and decided.
///
/// For each step in order: carry its directive if it has one, capture an
/// [`Observation`], evaluate the step's [`Assertion`] over it, and record the
/// verdict. A driver or view fault becomes [`Verdict::Errored`] and stops the
/// run there, because a run that could not decide one step cannot honestly
/// claim the steps after it. An assertion that simply does not hold becomes
/// [`Verdict::Failed`] and the run continues, so a transcript shows every step's
/// verdict rather than only the first disappointment; [`Transcript::verdict`] is
/// what reduces them to one.
///
/// One type supplies both halves rather than two, because the thing that can
/// carry a directive outward is the thing that can read what came back: a live
/// driver holds one connection to the running game, and splitting the arguments
/// would only let a caller pair a view of one game with a driver of another.
///
/// This does not wait. A step names the [`GameEvent`] it waits for and this
/// asserts immediately after the directive, so a scenario that needs an effect
/// to have landed says so with a following [`Directive::AdvanceTicks`] step —
/// the property `docs/decisions/carrying-an-action-outward.md` records, kept in
/// the scenario where a reader sees it rather than hidden in a runner's sleep.
pub fn run<W>(scenario: &Scenario, world: &mut W) -> Transcript
where
    W: GameView + GameDriver,
{
    let mut transcript = Transcript::new();
    for step in scenario.steps() {
        if let Some(directive) = step.directive()
            && let Err(reason) = world.carry(directive)
        {
            transcript.record(StepOutcome {
                label: step.label().to_owned(),
                verdict: Verdict::Errored(reason),
            });
            break;
        }
        let observation = match world.observe() {
            Ok(observation) => observation,
            Err(reason) => {
                transcript.record(StepOutcome {
                    label: step.label().to_owned(),
                    verdict: Verdict::Errored(reason),
                });
                break;
            }
        };
        let verdict = if step.assertion().evaluate(&observation) {
            Verdict::Passed
        } else {
            Verdict::Failed(step.label().to_owned())
        };
        transcript.record(StepOutcome {
            label: step.label().to_owned(),
            verdict,
        });
    }
    transcript
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::str::FromStr;

    /// Builds a key from `<kind>:<namespace>:<path>`, e.g. `item:ravel_test:ingot`.
    fn key(kind: &str, path: &str) -> RegistryKey {
        RegistryKey::from_str(&format!("{kind}:ravel_test:{path}")).expect("valid registry key")
    }

    #[test]
    fn empty_scenario_is_a_valid_plan() {
        let scenario = Scenario::new("empty", Side::Client);
        assert_eq!(scenario.name(), "empty");
        assert_eq!(scenario.side(), Side::Client);
        assert!(scenario.steps().is_empty());
        assert!(scenario.checkpoints().is_empty());
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
            Assertion::BlockPresent {
                key: machine.clone(),
            },
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
            verdict: Verdict::Errored("driver".into()),
        });
        assert_eq!(with_error.verdict(), Verdict::Errored("driver".into()));
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
            assert!(!checkpoint.as_str().is_empty());
        }
    }
}
