# M1 architecture evidence

M1 measures how Ravel enters the game, runs mods, binds versions, owns state, and
reaches a launcher. This directory holds those measurements; the records that act
on them are in [`docs/decisions/`](../../docs/decisions/).

Four of the five gates decided. ADG-2, mod execution, deliberately did not: it
selected two different mechanisms on this same evidence before it became clear that
the deciding considerations — distribution, hot reload, what a package contains —
are properties of a loader that does not exist yet. M3 decides it against the
prototype. The measurements below are what that decision will use.

[`manifest.json`](manifest.json) is M1's admission artifact, and
`devenv tasks run ravel:verify-milestone:m1` is what checks it: every gate has a
recorded result, every gate has an accepted ADR, the spike results are hashed under
the `spike-v1` protocol, an ADG-5 result records an observed stock launch, and the
manifest names the change that introduced it. All of that holds, and so does the
last rule to be met: `review.result` records `pass`, and the gate requires the
record it binds to state the same verdict.

That verdict is a tracked file. [`review.md`](review.md) is the third cold round's
verdict as it was written — PASS on six findings, with the file and line for each,
what it established by breaking or re-doing rather than reading, and its own list
of what it read rather than exercised. `review.record` names it by path, the
scaffold recomputes its digest, and `verify_recorded_verdict` compares the
`Result:` line to `review.result`, so neither half can move without the other. The
field used to record a title and a hash of nothing obtainable, and the two rounds
before this one are what found that and what found the missing comparison.

What M1's measurements do not mean is that everything is clean. The manifest's
`deviations` and `risks` are recorded rather than repaired — including that the
selected native binding has never run in the real game, that seven of the first
audit's ten findings are enumerated nowhere anyone can read, and that the figures in
`tests` are an attestation about a past gate run rather than something a reader can
re-derive. Counting them belongs to `jq`, not to this paragraph. None is blocking,
and a passing review is not a claim that the corpus is clean. Read them before
relying on anything here.

M1-W4's Verification clause used to be the exception. It asks that a generated
binding be reproducible from pinned inputs and fail on drift; the drift half was
recorded — both measured ADG-3 results record `driftDetected` — and the
reproducibility half lived only in the instrument, which generated twice and refused
to report on a mismatch. A record therefore entailed one identical repeat and
observed nothing, and nothing recomputed it. It is now observed as a pair of
digests: each of the two re-measured ADG-3 results records
`bindingGeneration.outputSha256` and `repeatedOutputSha256` from two derivations
that each resolved the pinned jar for themselves, and
`ravel:verify-milestone:m1` requires an ADG-3 result to carry both and requires
them equal. The digest is what is gated rather than the boolean beside it,
because an operator holding the pinned jar can re-derive a digest and refute it
and can do nothing with a boolean. What that observes is narrow — two
derivations in one process on one machine agreed — and it says nothing about
another machine or another day. The re-measurement is a new pair of records
rather than a rewrite of the M1-era pair, because a result names the change that
introduced it; the older pair stands as measured, over the smaller inventory of
that run.

## What is recorded

One result per candidate under [`spikes/`](spikes/), all conforming to
[`schemas/spike-result.schema.json`](../../schemas/spike-result.schema.json) and
checked by `devenv tasks run ravel:verify-spikes`.

| Gate | Candidates | Outcome | Headline evidence |
| --- | --- | --- | --- |
| [ADG-1](spikes/adg-1/) | 4 measured, 2 rejected | agent plus exported JNI symbol | a primitive hook crossing costs 10.02 ns at the median and allocates nothing; the same cost as `RegisterNatives` |
| [ADG-2](spikes/adg-2/) | 4 measured | **deferred to M3** | leading candidate a linked mod at 2.50 ns a call with no unsafe in the mod boundary, against 1.88 ns for a dynamic library needing two unsafe calls, 10.64 ns for a WebAssembly sandbox, and 3167 ns for a helper process |
| [ADG-3](spikes/adg-3/) | 2 mechanisms measured, each re-measured once against today's pinned inputs, 4 rejected | class-file extraction | both extractors resolve every hook in the inventory to the same binding bytes, generating twice produces the same bytes, and a drifted input is refused |
| [ADG-4](spikes/adg-4/) | 3 measured | Java keeps its state | all three models within 1.09 ns at the median, so duplication buys nothing measurable |
| [ADG-5](spikes/adg-5/) | 1 measured | stock Prism arguments | the launcher exposes the two settings needed and requires no patch |

Every measured candidate exercised the same five failure scenarios. Most failed
closed. Three outcomes did not, and they are the substance of ADG-2 rather than a
gap in the evidence:

- **`adg2-native-dylib`** records `continued` for both `callback-failure` and
  `crash`: a mod that panics and a mod that aborts each took the host down on
  signal 6. That a panicking mod is uncontainable across the C ABI is why this
  candidate was not selected.
- **`adg2-static-composition`**, the leading candidate, records `continued` for
  `crash` alone. Its `callback-failure` failed closed, because a panic in a linked
  mod unwinds inside one Rust program and the host catches it. A mod that aborts
  its own process still ends the game, and that residual cost is one of the things
  [ADG-2](../../docs/decisions/adg-2-mod-execution.md) leaves for M3 to weigh.

`adg5-stock-jvm-arguments` records `crash` and `timeout` as not evaluated, because
reading a launcher's settings runs no game: there is no process to end and nothing
to hang.

## Reproducing it

Each gate has one task. None of them authenticates, and none of them launches the
game. All of them need the pinned local fixtures from
[`tests/fixtures/local/README.md`](../../tests/fixtures/local/README.md).

```console
devenv tasks run ravel:spike-adg-1
devenv tasks run ravel:spike-adg-2
devenv tasks run ravel:spike-adg-3
devenv tasks run ravel:spike-adg-4
devenv tasks run ravel:spike-adg-5
devenv tasks run ravel:verify-spikes
```

Results are overwritten in place, so a re-run produces a diff whenever the
measurement moves. Sub-nanosecond figures move between runs; the boundary costs
do not. ADG-3 is the exception: its measured records carry `-remeasured` names its
own change introduced, and the M1-era measured pair beside them is not rewritten by
a re-run. Its four rejection records are rewritten by a re-run, with today's
revision and timestamp, and they are restored rather than committed: a rejection
records no measurement, and letting a later change claim a file an earlier change
introduced is the provenance `ravel:verify-spikes` refuses.

Every result records `trial.revision.dirty` as true, which is expected rather than
a warning. For most results the measured tree is `id` plus the diff of the change
named by `changeId`; the exceptions are recorded below, and how many there are is
not stated here, because nothing recomputes a count written in prose. The
[measurement gate](../../docs/architecture/m1-measurement-gate.md) explains why a
jj repository has no cleaner state to measure from.

## How the review went

M1-W8 ran as a sequence of passes rather than as one review, and one of them is
tracked: [`review.md`](review.md) is the third cold round's verdict. The earlier
passes are described below from records held outside this repository, so this
section is attested rather than auditable.

The first review ran three lenses and all three returned FAIL: the safety finding
removed an exception `SPEC.md` §3.3 had already been drafted around, the
performance finding withdrew a budget claim this evidence cannot support, and the
evidence-integrity finding listed numbers that disagreed with the JSON. Four
iterative passes followed, blocking findings falling from ten to three to one to
none, checking the recorded deviations against their ADRs along the way; one was
unmentioned and one misattributed, and both are now stated where they belong. An
earlier draft of this paragraph put a count on that corpus — thirty-one — that
matched no state of it: 34 stood at the revision the manifest pins, the spike
results hold 27 of the corpus and the manifest holds the rest, and the manifest has
grown since. No count is put back, because the corpus grows every time a deviation
is recorded and a fixed number in a sentence about four earlier passes is a claim
that goes stale by itself. What the passes checked was the corpus as each pass
found it.

That reviewer then disclosed its own limit: having driven every correction, it was
approving a corpus shaped by its own checklist, and a defect it never thought to
look for would survive all four passes. A cold auditor was engaged on that basis
and returned FAIL on something none of the four passes could have caught, because
every document involved was individually correct — the M1 gate's failure actions
reject a candidate needing a maintained unsafe call and name the dynamic library,
while ADG-2 retained that candidate for M3. It also reproduced the `jni_mangle`
zero-unsafe result from scratch rather than accepting it.

Its findings were resolved, and a second cold round read the result and returned
FAIL again. That there were ten findings in the first audit, and that all ten were
resolved, is attested here and not auditable from anything tracked: the audit
record is held outside the repository, this document names one of the ten — the M1
gate's failure actions rejecting the candidate ADG-2 retained — and the manifest's
deviations name the `m1-measurement-gate.md` digest that matched the file at no
revision. The rest are in the untracked record, and that gap is not closed by
anything since. The claim is narrowed to that rather than repeated as a round
number nobody here can check.

A third cold round then read this evidence, and it is the one that is tracked. It
returned PASS on six findings, conditional on the first of them being fixed in the
change that closes M1: nothing compared `review.result` to the verdict inside the
record it binds, and `review.result` had been set to `pass` over a record reading
`Result: FAIL` with `verify milestone M1` exiting 0. It re-derived ADG-3's binding
three times from the pinned inputs and got the tracked digest each time, moved the
local fixtures away to confirm the runner cannot fabricate a measurement, and broke
the provenance, digest, redaction and closure rules one at a time to confirm each
fails closed. All six findings are fixed in the stack that closes M1, and
[`review.md`](review.md) is the verdict as delivered, including the reviewer's own
list of what it read rather than ran — the tracked-JAR rule is in that list and is
still untested.

## What this evidence does not establish

It does not establish an M9 performance budget, and the review criterion above was
narrowed once that became clear. A budget is an inequality between what the
official pack leaves spare and what Ravel spends, and neither side is measured:

- **Nothing here counts crossings.** No document in this repository states or
  estimates how many boundary crossings a tick a real pack makes, and per-crossing
  costs cannot be turned into a frame cost without it. The rate is what M1 can
  say: at ~12.5 ns for a hook that calls a mod — ADG-1's 10.02 added to ADG-2's
  2.50, not a figure any trial measured, because no trial ran a JVM-to-Rust-to-mod
  chain — crossings reach 10% of a 33.3 ms frame at roughly 266,000 a frame, and
  10% of a 16.7 ms frame at roughly 133,000. Both are given because nothing here
  chose a reference frame rate: no document in this repository states one, so the
  30 fps figure this paragraph used to give alone was a rate, not a target, and at
  60 fps materiality arrives at half the crossings. Whether a real pack is three
  orders of magnitude below either or at it is unmeasured. `M2-W8` owns the census.
- **There is no official-pack baseline.** M0 recorded launches, not performance —
  `evidence/m0/manifest.json` sets `performanceCaptureRequired` false and the
  launch records carry no frame time. The `SPEC.md` §6 pair is an M9 artifact.
- **The recorded tails are not per-call tails.** Percentiles are taken over batch
  totals before division — batches of 64 everywhere except the helper process,
  which uses 8 — so they describe averages of a batch and suppress the dispersion a
  frame budget depends on. The medians are sound. A budget needs unbatched
  measurement over a window long enough to contain collection and safepoint
  events, and no trial here comes close: multiplying each median by its invocation
  count gives timed regions between 0.6 ms and 13.6 ms, apart from the helper
  process at 50.7 ms.

- **Boundary-traffic figures are declared, not observed.** Every gate computes
  bytes-per-call from a constant in its own source — 16 in ADG-1 and ADG-2, a
  per-model arithmetic expression in ADG-4 — and records it as `measured` with a
  collector identity and `quality: "exact"`. The arithmetic is right and no
  conclusion rests on it, but no instrument counted a byte. The visible artefact is
  `adg1-agent-java-callback`, which records 16 bytes per call for a callback that
  never leaves the JVM and crosses no boundary at all.
- **Retained RSS is the bootstrap reading, not a post-workload one.** The harness
  samples `/proc` once, immediately after the ready marker and before any measured
  invocation, and fills both `bootstrapPeakRss` and `retainedRss` from it. The two
  are byte-identical in every ADG-1, ADG-2 and ADG-4 result for that reason. ADG-3
  is the exception and its ADR handles it. Nothing here measures memory a workload
  retains.
- **ADG-4's three models do not run an identical flow.** The ADR describes one flow
  under three owners, but the Java-authoritative and mirrored models each perform
  two Java field writes the Rust-authoritative model does not, and the
  Rust-authoritative model performs an addition the others do not. Argument size is
  therefore confounded with a small difference in work. The spread is ~1 ns and the
  conclusion does not turn on it, but the rows differ by more than who owns state.

None of this weakens what was decided. ADG-1, ADG-3 and ADG-5 were decided on
ranking and on properties that are not nanoseconds, and the ranking holds under
every statistic recorded. ADG-2 is deferred to M3 for reasons that have nothing to
do with this section. What the section bounds is what may be claimed from the
numbers.

When those are closed, the manifest is mechanical: the spike results are already
hashed and the gate rules for M1 are already in
[`tools/ravel-xtask/src/lib.rs`](../../tools/ravel-xtask/src/lib.rs).

The ADG-5 launch is not outstanding, and what it establishes is narrower than a
launch. The recorded proof is three substring tests over a redacted log: that
`-javaagent:` appears, that `-Djava.library.path=` appears, and that none of five
known loader names does. Nothing checks that the launch was of 26.1.2, that the
launcher was the pinned Prism, that the two arguments sat on the game's command
line rather than elsewhere in the log, or that the client reached a world or exited
cleanly. `ravel:verify-milestone:m1` enforces that a result records the boolean and
the log hash, which is a check on the record rather than on the launch.

Until the cold audit's second reading, only this paragraph said that. The record's
own `prismIntegration.detail` asserted that the pinned game launched with both
arguments on its command line and no other mod loader named — a conclusion three
substring tests do not reach. It now states the three tests and what they leave
unchecked, and `experiments/adg-5-prism/src/main.rs` was changed with it so a re-run
writes the same sentence instead of restoring the old one.

Provenance is recorded twice, because the two halves answer different questions.
`trial.revision.id` is a commit that existed when the trial ran; every jj rewrite
abandons it, so it records what ran and cannot be resolved to anything.
`trial.revision.changeId` is the change that holds the result, which survives
rewrites, and `ravel:verify-spikes` recomputes it against the change that
introduced the file rather than trusting the claim.

That check exists because the earlier contract — that the field names the parent
change — could not be recomputed, and the seven results named below disagreed with
it for a week before a human noticed. A provenance claim nothing recalculates is a
claim, not provenance.

Those same seven are still wrong in the half that is not checked, and are recorded
rather than repaired. `changeId` is correct on every recorded result and
machine-verified, but
`id` is not: the six ADG-1 results record a predecessor commit of their own change
rather than of its parent, so reconstructing them as `id` plus that change's diff
counts the change twice. The ADG-5 result is worse — its `id` resolves to a change
*above* the one holding it, which contains documents written from the result
itself, so that commit did not exist when the trial ran. Repairing either means
re-running the gate, and for ADG-5 that means another authenticated launch, so the
honest record is this paragraph. Nothing depends on `id`: no figure, no decision,
and no check reads it.

The manifest's own `revision.id` was in the same position and is repaired rather than
disclosed. It named `909240424db4`, a commit jj abandoned when it rewrote the change
holding this evidence. That commit resolved here, out of this working copy's
operation log, and nowhere else, which is why the defect was invisible until a cold
reviewer cloned the repository and every recorded input digest in every milestone
became unreadable at once. `revision.id` now names the commit its own
`revision.changeId` is at, the inputs are re-hashed there, and
`verify_pinned_commit` requires both on every `ravel:verify`, so the next rewrite of
this change fails the gate instead of dangling.

What that does not repair, and what this file previously implied it did, is the run.
`commands` and `tests` were recorded against the abandoned commit, so `revision.id`
no longer names the tree they were measured on. Nothing recomputes them: the gate
holds `tests` to its own arithmetic and to the coverage floor, and no tracked
artifact binds either figure to a gate run. They are an attestation that M1's gate
was run and passed at that revision, not evidence a reader can re-derive, and the
manifest now says so in a deviation of its own. Binding a redacted gate summary was
the alternative and was refused: any summary shippable today comes from a re-run on
today's tree, whose numbers are not these, so it would bind a different run than the
one attested.

How far the recompute travels was measured, not assumed. A `git clone --no-local` of
this repository carries no `refs/jj/*`, and in that clone `909240424db4` does not
exist as an object while the re-pinned commit is visible and all five input digests
recompute there — so the re-pin is what makes this evidence auditable outside the
working copy that wrote it. What a clone does not carry is a jj repository, and
without one the provenance checks announce that they are skipping rather than report
a pass. Beyond that, nothing in this stack has been pushed, so these commits live
here and in clones taken from here. `manifest.json` records both limits as a
deviation; neither claims publishing is imminent.
