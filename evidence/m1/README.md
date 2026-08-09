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
manifest names the change that introduced it. All of that holds. The gate does not
pass, and says why: `review.result` records `fail`, because the last cold verdict
against this evidence is FAIL and none of the corrections that answered it has been
re-audited. M1's measurements stand and M1's review does not.

The review itself is a process artifact and is not tracked here; the manifest
records its reviewer, result, and the hash of the record, and that record is held
outside the repository. The gate checks the claim rather than the document, which
is a weaker guarantee than recomputing a hash and is stated so a reader knows
which one they are getting.

What M1's measurements do not mean is that everything is clean. The manifest carries
eleven deviations and four risks, all recorded rather than repaired — including that
the selected native binding has never run in the real game, that the last cold audit
of this evidence returned FAIL before its findings were resolved, and that this
manifest does not exist at the revision it pins. One of the eleven is **blocking**:
M1-W4's Verification clause asks that a generated binding be reproducible from
pinned inputs, and no M1 record contains a reproducibility observation. Read them
before relying on anything here.

That clause is stated as unmet rather than argued closed. What is recorded is the
drift half — both measured ADG-3 results record `driftDetected` — and a single
`outputSha256` per candidate. The two extractors' outputs differ (`0ecc36a9` over
3417 bytes against `8b093021` over 3413), so the cross-extractor digest is not a
repeated run, and the "resolved all six targets identically" line below is a prose
string with no per-target list behind it. What does enforce reproducibility is the
instrument, not the evidence: `experiments/adg-3-bindings/src/bin/generate.rs`
generates twice from the same resolved inventory and refuses to report if the digests
differ, so a recorded result entails one identical repeat — an entailment from
source, not a recorded observation, and nothing recomputes it. Recording one now is
not available either: the generator refuses unknown adapter inputs and
`minecraft/versions/26.1.2/adapter.toml` has since grown four, so it will not parse
today's pinned inputs, and editing it would measure a different instrument.

## What is recorded

Twenty results under [`spikes/`](spikes/), one per candidate, all conforming to
[`schemas/spike-result.schema.json`](../../schemas/spike-result.schema.json) and
checked by `devenv tasks run ravel:verify-spikes`.

| Gate | Candidates | Outcome | Headline evidence |
| --- | --- | --- | --- |
| [ADG-1](spikes/adg-1/) | 4 measured, 2 rejected | agent plus exported JNI symbol | a primitive hook crossing costs 10.02 ns at the median and allocates nothing; the same cost as `RegisterNatives` |
| [ADG-2](spikes/adg-2/) | 4 measured | **deferred to M3** | leading candidate a linked mod at 2.50 ns a call with no unsafe in the mod boundary, against 1.88 ns for a dynamic library needing two unsafe calls, 10.64 ns for a WebAssembly sandbox, and 3167 ns for a helper process |
| [ADG-3](spikes/adg-3/) | 2 measured, 4 rejected | class-file extraction | two independent extractors resolved all six targets identically; a drifted input is refused |
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
do not.

Every result records `trial.revision.dirty` as true, which is expected rather than
a warning. For thirteen of the twenty the measured tree is `id` plus the diff of
the change named by `changeId`; the exceptions are recorded below. The
[measurement gate](../../docs/architecture/m1-measurement-gate.md) explains why a
jj repository has no cleaner state to measure from.

## How the review went

M1-W8 took six passes and its record is held outside this repository.

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

Its findings were resolved and none was re-audited, so the last cold verdict
against this evidence is FAIL, and `review.result` in the manifest now records
`fail` to say so. That there were ten of them, and that all ten were resolved, is
attested here and not auditable from anything tracked: the audit record is held
outside the repository, this document names one of the ten — the M1 gate's failure
actions rejecting the candidate ADG-2 retained — and the manifest's deviations name
the `m1-measurement-gate.md` digest that matched the file at no revision. The rest
are in the untracked record. The claim is narrowed to that rather than repeated as
a round number nobody here can check. It previously recorded the iterative reviewer's pass
beside a deviation stating the opposite, which made a recorded `pass` mean one
thing in M1 and another in M3, where the same situation is recorded as `fail`.
The consequence is deliberate: `ravel:verify-milestone:m1` now reports
`milestone review must be independent and pass`, so M1's gate reads open until a
cold auditor re-verifies this evidence.

## What this evidence does not establish

It does not establish an M9 performance budget, and the review criterion above was
narrowed once that became clear. A budget is an inequality between what the
official pack leaves spare and what Ravel spends, and neither side is measured:

- **Nothing here counts crossings.** No document in this repository states or
  estimates how many boundary crossings a tick a real pack makes, and per-crossing
  costs cannot be turned into a frame cost without it. The rate is what M1 can
  say: at ~12.5 ns for a hook that calls a mod — ADG-1's 10.02 added to ADG-2's
  2.50, not a figure any trial measured, because no trial ran a JVM-to-Rust-to-mod
  chain — crossings reach 10% of a 33.3 ms frame at roughly 266,000 a frame. Whether a real pack is three orders
  of magnitude below that or at it is unmeasured. `M2-W8` owns the census.
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
change — could not be recomputed, and seven of the twenty results disagreed with it
for a week before a human noticed. A provenance claim nothing recalculates is a
claim, not provenance.

Those same seven are still wrong in the half that is not checked, and are recorded
rather than repaired. `changeId` is correct on all twenty and machine-verified, but
`id` is not: the six ADG-1 results record a predecessor commit of their own change
rather than of its parent, so reconstructing them as `id` plus that change's diff
counts the change twice. The ADG-5 result is worse — its `id` resolves to a change
*above* the one holding it, which contains documents written from the result
itself, so that commit did not exist when the trial ran. Repairing either means
re-running the gate, and for ADG-5 that means another authenticated launch, so the
honest record is this paragraph. Nothing depends on `id`: no figure, no decision,
and no check reads it.

The manifest's own `revision.id` is in the same position and was not disclosed until
the cold audit's second reading. `909240424db4` is an abandoned predecessor state of
change `potzkxqsxykq`, and `manifest.json` is absent from it: the manifest was
written into `rxovtruttmyk` after that commit existed, and jj rewrote `rxovtruttmyk`
afterwards, so no commit holding the file is reachable from the one it pins. Unlike
the spike results, something does read this field — `verify_recorded_inputs` hashes
all five recorded inputs at it, and all five match — so what is wrong is narrower
than it looks: the revision is real and the inputs are bound there; the manifest
simply is not in it. It is disclosed rather than repaired because both repairs trade
a true statement for a false one. The recorded commands ran on `909240424db4`, so
re-pinning to a commit that holds the file would falsify `commands` and `tests`;
re-running the gate to make those a later tree's numbers is a re-measurement of M1
nobody asked for, and it would land one change below whatever commit recorded it,
which is the circularity `evidence/m3/manifest.json` carries in its own revision
block.
