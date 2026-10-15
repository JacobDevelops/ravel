# Carrying an action outward

Status: accepted during M6-W2. An action a mod asks for reaches the game as an
instruction the Java adapter performs, and Rust calls no game method. Measured:
[`evidence/m6/outward-crossing.json`](../../evidence/m6/outward-crossing.json),
re-runnable with `cargo make ravel:measure-outward-crossing`.

This record takes no ADG number, for the reason
[`README.md`](README.md) gives about the two records that already do not:
`SPEC.md` §15 owns those numbers and reserves its unused ones for other
subjects.

## Why this record exists at all

[ADG-6](adg-6-configuration-ui.md) rejected drawing a screen from Rust and said
exactly what would reopen the question: "If Ravel ever needs to call into the
game for another reason, that is a new gate with its own measurement, and this
decision should be revisited then rather than assumed."

Playing a sound a mod declared is that other reason. `RVL-GAP-08` records the
state that made it one: `AudioDeclaration::play` answers with a `PlayRequest`
describing what would be played rather than playing it, `StageDeclaration::resolve`
orders contributions at stages nothing draws at, and `JavaHandle` — the only type
in the workspace naming a JNI object — offers a declared class, a null check and
reference identity, with its own doc comment recording that "calling into the game
at all is ADG-6's to settle". So the question is Ravel's now rather than
hypothetical, and this is the gate ADG-6 asked for.

**This gate could measure, and ADG-6's could not.** ADG-6 recorded that no
benchmark produces a missing capability: one of its two candidates did not exist,
and building it would have been the decision rather than evidence for it. Both
candidates here exist already. JNI provides the outward call, and
`dev.ravel.jvm.Persistence` already resolves game classes reflectively and calls
their members on ADG-7's account, which is an adapter performing an action
Ravel decided. Neither had to be invented to be timed, so a decision taken here
without measuring would have been a preference wearing a gate's clothes.

## Decision

**An action a mod asks for crosses outward as data, and the Java adapter makes
the call.** Rust answers an inbound crossing with an instruction describing what
should happen; the receiver-aware Java adapter ADG-1's amendment already admits
decodes it and performs it against the game. No Rust code calls a game method,
and `JavaHandle` gains no method that calls into the JVM.

ADG-1's rule survives unchanged and is worth quoting because this record is the
first thing that could have broken it: "Nothing here admits a call from Rust into
the game." It still does not. What this record adds is a direction for *effects*,
carried by the artifact that was already allowed to make game calls, rather than
a second boundary running the other way.

## Evidence

One request carries some number of game actions outward. The performed action is
the same trivial arithmetic in every candidate, so what separates them is the
crossing rather than the work. Three candidates, at one action per request and at
eight, 20 000 timed batches each, p50 nanoseconds per request:

| Candidate | 1 action | 8 actions | Crossings per request |
| --- | --- | --- | --- |
| Rust calls the game method, resolving it per action | 154.7 | 988.6 | 1 + actions |
| Rust calls the game method through a cached identifier | 75.5 | 425.0 | 1 + actions |
| Rust answers with an instruction, Java performs it | 89.1 | 106.1 | 1 |

Per action at eight, that is 123.6, 53.1 and 13.3 nanoseconds. The instruction
candidate is four times cheaper per action than the fastest direct one, and its
cost is almost all fixed: 89.1 nanoseconds for one action and 106.1 for eight is
about 2.4 nanoseconds of marginal cost per additional action, because the array
carrying them crosses once whatever it holds.

**The single-action case goes the other way and this record does not hide it.**
At one action a cached direct call is 75.5 nanoseconds against the instruction's
89.1, and that difference is larger than the sampling noise: the two
distributions' p95 figures are 110.4 and 129.6, so they do not overlap into each
other. A mod asking for exactly one thing pays about fourteen nanoseconds for
this decision. The crossover is somewhere between one action and two, and every
count above it runs the other way and keeps widening.

**Resolving the method per call costs about as much again as making it.** The
lookup candidate is roughly twice the cached one at both counts. That figure is
in the record for a reason ADG-4's evidence section would recognise: pricing only
the naive form of the candidate this gate rejects would have made the rejection
look easier than it is, and nobody reading the number afterwards could have seen
it.

### What a nanosecond cannot report

A refused action was exercised in every candidate, and the two directions fail
differently in kind:

- **Direct.** The game method throws, and the exception stays *pending* on the
  calling thread. Every JNI call the native side makes before clearing it is
  undefined — including the call that would build a string describing what went
  wrong. The trial reports `failed-closed` only because the experiment clears it
  by hand at exactly the right moment. That obligation is real, it is unbounded
  in the number of outward call sites, and no type in the workspace expresses
  it: a `Result` that must be handled before the next unrelated call is not
  something Rust's type system is being asked to enforce here.
- **Instruction.** The adapter catches an ordinary `IllegalStateException` with
  no native frame on the stack at all. The trial's own detail says so. There is
  no pending-exception state to be in, because the thread never left Java to
  make the call.

This is the part of the comparison that survives a faster machine.

## Rejected

- **Rust calls the game method through a cached identifier.** The fastest
  candidate at one action and the loser at every count above it, on a crossing
  count that grows with the actions a mod asks for rather than staying at one.
  It also carries the pending-exception obligation above and a second one the
  measurement records rather than prices: a cached method identifier is only
  sound while something holds the class it came from, so a loader caching across
  calls owes a global reference per cached target and owes its lifetime. Rejected
  for the risk, with the fourteen nanoseconds it wins at one action stated in
  "Evidence" rather than argued away.
- **Rust calls the game method, resolving it per action.** Same risks, twice the
  cost. Rejected outright.
- **A second generated boundary running outward.** Not measured, and deliberately
  not: it is the shape ADG-6 refused, and generating one would mean a second hook
  inventory, a second lifetime discipline and a second exception protocol before
  anything could be timed. If the instruction design fails on ordering — see
  "Consequences" — this is the alternative to reopen, and it should be built and
  measured then rather than assumed cheap now.

## Consequences

- `AudioDeclaration::play` keeps answering with a `PlayRequest` and gains a
  carrier rather than a caller. The type a mod writes against does not change,
  which is the point: the surface was already shaped for this decision before it
  was taken.
- **An action happens after the answer, not during it.** A mod cannot learn in
  the same call whether the game performed what it asked for, because the game
  has not performed it yet. Every surface built on this crossing has to be one
  where that is acceptable, and a surface where it is not — asking the game a
  question and needing the answer to decide — is `RVL-GAP-10`'s and is not
  settled by this record.
- **A dropped instruction is a silent failure.** The adapter performing nothing
  and the adapter performing everything look identical from Rust. Something has
  to check, and this record does not decide what; the persistence crossing's
  tagged answer is the nearest precedent in the repository.
- The instruction encoding is a wire between two artifacts Ravel ships, which
  makes it the same class of thing as `ravel_bootstrap::wire` and subject to the
  same reasoning about lengths and free-form text. It is not a `hooks.toml`
  entry and needs no re-pin ceremony.
- Every crate keeps `#![forbid(unsafe_code)]`, `ravel-bootstrap` included. This
  is the second reason the rejected candidates are expensive rather than merely
  slower: `call_static_method_unchecked` is unsafe by signature, so the cached
  form could not have been written in a maintained crate at all without an
  accepted exception to a table that is empty.

## Limits of this evidence

- **The performed action is arithmetic, not a game method.** A real call does
  work both directions pay identically, so the ratio measured here is the
  crossing's and the ratio a player would feel is smaller. Nothing in this
  record is a budget.
- **The instruction candidate's Java side calls the action directly.** An adapter
  resolving its target reflectively, as `dev.ravel.jvm.Persistence` does, pays a
  dispatch this record does not include. That cost lands on the winning candidate
  rather than the rejected ones, so the margin at eight actions is an upper bound
  on itself.
- **Measured on Java 25.0.4 against the pinned 25.0.1.** Both directions ran on
  the same runtime, so the comparison holds; the absolute figures are not the
  pinned runtime's.
- **One machine, one action shape, one thread.** Nothing here says what an
  outward call costs from a thread the JVM did not attach, which is a question a
  real audio surface will meet and this measurement never asked.
- **Ordering is unmeasured and undecided.** "An action happens after the answer"
  is a property of the mechanism, not a number, and whether any particular
  surface tolerates it is that surface's question.
