# M3 evidence

M3-W0 through M3-W7 are closed. M3-W8 is open: every independent round that has
run returned FAIL. The latest round's blocking finding, B-1, is answered by the
fifth launch campaign described below, and answering it is not closure — one of
that round's must-fix findings and all of its moderate and minor ones are still
open, and the review that has to pass is an independent one. M3 is not closed and
does not authorize private inventory work.

M3-W4 and M3-W7 close on operator attestation for their live half. The world
round trip and every native surface row were observed in a real client, and the
worlds, raw logs, and screenshots were deleted afterwards with only their digests
kept. A digest of a file that no longer exists records a claim rather than
verifies one, so a reviewer can recompute the in-gate half — the conformance
suites and the JVM harness, which run under `ravel:verify` on any machine — and
must take the operator's word for the rest. That is this repository's convention
for launch evidence, and it is stated up front so nobody mistakes an attested
result for a recomputable one.

M3-W0's evidence has a ceiling of the same shape and it is easy to miss, because
that gate closes on tracked files rather than on a launch.
`docs/architecture/m3-hook-targets.md` and `minecraft/versions/26.1.2/hooks.toml`
are both tracked and both readable, but every claim either makes about the game is
a claim about the pinned client jar, and that jar is a restricted local fixture
this repository never tracks. On a machine without it the chain terminates at
`minecraft/versions/26.1.2/adapter.toml`'s `inputs.clientJar` digest and byte
count: `mappings.lock` and `stubs.lock` can be compared against their own pinned
hashes, which is what catches a stale lock, but nothing can re-resolve a member
against the class files. `adapter.toml` states this in its own comments, and it is
restated here because no other M3 record does. So M3-W0 is recomputable exactly as
far as the inventory's internal consistency and no further.

| Record | Work package | What it shows |
| --- | --- | --- |
| [`manifest.json`](manifest.json) | M3-W0..W8 | Which gates are closed, what each one's evidence can and cannot prove to a reviewer, and the failed independent review that keeps M3-W8 open. |
| [`pack-compile.json`](pack-compile.json) | ADG-2 | What compiling a pack-sized set of mods with real bodies costs — cold, warm after one mod changes, in peak memory, and on disk. |
| [`profiling-overhead.json`](profiling-overhead.json) | M3-W6 | What opt-in profiling costs a tick when nobody turned it on, what it can and cannot attribute to a mod, and which metrics it cannot measure at all. |
| [`linux-process-matrix.json`](linux-process-matrix.json) | M3-W7 | What a real Linux launch proves about the game directory, the rewrite, and the instance — and, at least as importantly, what it could not reach. |
| [`class-loader-identity.json`](class-loader-identity.json) | M3-W3 | That the loader which loaded a rewritten `net.minecraft` class is the same object as `ClassLoader.getSystemClassLoader()`, which is the premise `appendToSystemClassLoaderSearch` rests on. Measured on an earlier build and an earlier JDK patch than the milestone's, and it says so. |
| [`review.md`](review.md) | M3-W8 | The five tracked independent M3-W8 rounds as they were written: verdict, findings with their anchors, what each reviewer recomputed and what it could not, and its stated coverage. Every round is FAIL, and the record says which round it does not hold and why. |

## M3-W8 review

Six independent rounds have run and all six returned FAIL.
[`review.md`](review.md) holds five of them verbatim and `review.record`
binds its digest, so those verdicts are readable rather than summarised from
memory. They are one file rather than one file each because `review.record` binds
one artifact per milestone and the rounds only mean anything read in order: each
round verifies the previous round's repairs.

One round is missing from it, and the fourth round is what found that out. The
earliest round reviewed the prototype rather than the evidence set, and its verdict
was never written into the repository — the summary below is all that survives of
it, which is exactly what a tracked review record exists to prevent. `review.md`
names the omission, states that the text is not recoverable from the repository or
its history, and says that its own numbering therefore begins at the second round.
The counts here and in `manifest.json` count the missing round rather than rounding
it away.

That earliest round returned **FAIL** with four blocking findings.
All four are answered, and they stay listed because a finding that disappears
once it is fixed leaves no way to tell it from one that was never raised:

- **Ravel-owned world write — answered, M3-W4 closed.** Ravel has moved its own
  versioned bytes through an official world save, exit, and reload, and a stored
  `dataVersion` rewritten to 9 outside the game refused the load with the
  diagnostic drawn in the client. See `linux-process-matrix.json` under
  `worldPersistenceRoundTrip`, and `reviewCeiling` in the same section for what a
  reviewer can and cannot recompute.
- **Native surfaces — answered, M3-W7 closed.** Rewriting all 33 callable targets
  the campaign reviewed proves reachability, not behaviour, so each surface was
  observed on its own. The saved-data half is the world round trip campaign; the
  screen, settings, and command halves are the surface campaign that followed it,
  over the same inventory and on the same build. The one surface that campaign found *failing* — opt-in
  profiling, whose accuracy caveat was unreadable in the native client — was
  fixed by change `npozznqz` and re-observed whole in a later launch, so the
  matrix record reads `pass` and this gate is closed on it. See
  `profilingSurfaceDefect` and `caveatReObservationCampaign` in
  `linux-process-matrix.json`.
- **Package schema — resolved by `rkrpplvwpklskyuwvksvmuuyklkwsqlt`.**
  `schemas/package.schema.json` required compiled per-target artifacts and could
  not express the source-only distribution contract ADG-2 selected. That change
  migrated schema v1 in place: `targets` is no longer admitted, `source` declares
  a repository and at least one portable source file with its SHA-256 digest, and
  the `unsupported-target` failure kind went with the model it described.
  `SPEC.md` §3.4 and `ROADMAP.md` moved with it.
- **Manifest inventory — resolved.** The first manifest draft omitted M3-W0 from
  `workPackages` even though its gate status appeared elsewhere in the same
  document. It is now in the canonical inventory.

The same review found no defect in hook integrity, safety/layering/ADR conformance,
or evidence redaction. A later round reviewed the evidence set itself rather than
the prototype and found four more: `manifest.json` recorded result digests nothing
recomputed, it pinned a revision predating the work it covered, it contradicted
the other records on whether the world round trip had happened, and
`linux-process-matrix.json` self-declared a `pass` the manifest held open. The
digests are recomputed and every record now says the same thing.

The revision and the gate numbers are a narrower claim than that, and successive
drafts of this paragraph overstated it three times: first by calling the numbers
"this state's", then by counting how far below the tip the pinned revision sits, and
then by pinning a commit that only this working copy could resolve. `revision.id`
names the commit that `revision.changeId` is at, which is a commit below the tip of
this stack and an ancestor of it. How far below moves on the next commit, so the
count belongs to `jj log` and not to this file; what is stated here is the
relationship. It previously named a commit jj had abandoned when it rewrote that
change — `jj log` reported it hidden and `jj file show -r` read it out of the
operation log rather than out of visible history — and a cold reviewer's clone,
which carries no `refs/jj/*`, could resolve nothing. `verify_pinned_commit` now
requires `revision.id` to be visible and to be the commit its own change is at, so
that state fails the gate rather than passing here and failing elsewhere.

The gate numbers are an attestation and are read as one. `tests.total` and
`commands[0].logSha256` describe the run made on the commit this manifest pinned
before the re-pin; that is not the count `ravel:verify` reports on this tip and
cannot be, since every change above it that adds a test moves the tip's. Recording a
gate result changes the tree that produced it, so `revision.id` can never be the tip
commit, and a manifest asserting otherwise would be asserting an identity no evidence
file can hold. Nothing recomputes those numbers — the gate holds them to their own
arithmetic and to the coverage floor, and no tracked artifact binds either to a run —
so they are the operator's attestation that the gate passed, not evidence a reader
re-derives. The convention is to run the gate over the content, write the numbers,
then run both gates again over the file as it stands and report that second run in
the change description rather than recursively into these fields;
`linux-process-matrix.json` uses the same convention for its own gate digests.

What is bound rather than trusted is narrower. `verify_recorded_inputs` hashes every
recorded input at `revision.id`, so a pinned revision that no longer resolves fails
the gate instead of reading as verified, and `verify_pinned_commit` refuses a pinned
commit that visible history does not reach. And `verify_recorded_run`, in the same
file, holds every manifest's recorded run to its own arithmetic on every
`ravel-verify-scaffold` — counts that sum to the total, no failures, no skips,
coverage at or above the floor, and no manifest recording a gate run without a test
result. None of that binds the log, which is private, and none of it binds the
numbers to the tip. A `git clone --no-local` of this repository resolves the
re-pinned commit and recomputes every input digest there, and resolves the abandoned
commit it replaced not at all; a checkout with no jj repository skips both provenance
checks and says so on stdout. `manifest.json` records both limits as a deviation of
its own, along with the fact that nothing in this stack has been pushed.

The cause of the digest rot is addressed rather than the symptom, and that fix
reaches exactly this far:
`verify_evidence_records` in `tools/ravel-xtask/src/lib.rs` recomputes every
`benchmarks.results[]` digest and every `platformEvidence[].baselines[]` digest
that names a tracked file against the working copy, recomputes every `inputs[]`
digest against the revision its manifest pins, refuses a manifest that records its
own digest, and fails `ravel-verify-scaffold` on any mismatch. Digests outside that
reach are named rather than left to look verified. Two in
`evidence/m0/manifest.json` are outside it by construction: the private launch
validations name captures that never enter the repository, so no gate can hash them
and the check announces them as unbound instead of passing over them in silence.

`review.record.sha256` used to be outside that reach too, and less obviously. M0's
was exactly the digest of `evidence/m0/review.md`, a tracked file that nothing
recomputed; M1's and M2's matched no tracked file at all, which was read as those
records being deliberately private and turned out to mean the digests bound nothing
obtainable. One field name, three meanings, and no gate could tell a reader which
they were looking at — while the field is the one a milestone is closed on. That is
now a rule rather than a disclosure. `schemas/evidence.schema.json` requires
`review.record.name` to be a repository-relative path, and `verify_recorded_review`
recomputes its digest through the same `verify_tracked_digest` body as the results
and the baselines, so a review named by title fails the scaffold and so does one
whose bytes have moved. M0's record is renamed to `review.md` against its unchanged
digest, M1's and M3's name tracked verdicts, and M2's is withdrawn: that review was
never written into the repository, so `ravel:verify-milestone:m2` now reports the
missing record and exits non-zero. Absence stays legal in the schema, because a
milestone under construction has no review yet; the milestone gate is what refuses
to close on it.

Findings about that machinery are carried rather than fixed, and each is
recorded as a deviation in `manifest.json`. One of them is now fixed instead: input
provenance was pinned by commit id, which every jj rewrite abandons, so it resolved
only out of this machine's operation log and resolved nowhere in a clone. It is
re-pinned to the commit `revision.changeId` is at and the gate refuses any pinned
commit visible history does not reach, which turns the next rewrite into a gate
failure and a re-pin. The residual gap — that an unpublished stack is unreachable
from a clone whatever it pins, so the recompute is skipped rather than run there — is
recorded as a deviation of its own. Nothing requires a
manifest to list the records beside it, so a record added to a milestone directory
can be bound by nothing; `evidence/m0/manifest.json` records an empty
`benchmarks.results` beside two evidence records. And `manifest.json` here records
an empty `platformEvidence`, so the closure rule over
`platformEvidence[].commands[].exitStatus` contributes nothing for M3 even though
M3-W7 is an operator-run native Linux matrix. Two more were added by the third
round. `verify_evidence_records` discovers manifests by scanning `evidence/`, so it
has no floor: an unreadable or renamed directory yields no manifests, no
violations, and no message, which is the one failure mode a digest check cannot
report on itself.

One more is carried and it is about the measurements rather than the machinery.
All three M3 trials — `pack-compile.json`'s curve and variants runs and
`profiling-overhead.json`'s — record `trial.revision.dirty: true`, so no revision
identifies the code any of them measured, including the code behind the single
M3-W6 limit `cargo xtask verify budget` enforces on every gate run. It is
disclosed as a deviation rather than repaired: the two pack-compile runs took 174
and 1161 seconds and hold on `linux-reference-v1` alone, and a re-run would land
on a working copy that is dirty again for the same reason the manifest's own
revision block gives — recording a result changes the tree that produced it. The
limit and the arithmetic over it are bound on every gate; which source produced
the figure is not.

Two findings from that round are fixed rather than carried. The `review.record`
digests it found unbound in three different ways under one field name are bound, as
described above. And `verify_recorded_change` and `verify_manifest_change` returned
in silence on a
checkout with no `.jj`, so on a git-only clone the control their own doc comment
calls the repository's strongest provenance control evaporated without a word. Both
now announce the skip the way `verify_recorded_inputs` already did.

The fourth round found the same shape one level down and it is fixed the same way.
`change_mismatch` treated empty output from `introducing_change` as agreement, so
any path jj could not attribute cleared the repository's strongest provenance
control without a word; it now announces that it cleared without a verdict. The
same round's two remaining findings were carried as deviations: that the pinned
revision was already an abandoned commit rather than one that might become abandoned,
which a later round repaired by re-pinning and by gating it, and that
`overhead.perTick`'s derivation could not be followed from the percentiles printed
beside it until the record stated its own rounding.

The fifth round found one blocking defect, B-1, and it is answered by
re-observation. The caveat re-observation campaign in `linux-process-matrix.json`
recorded the reading that proves the profiling surface renders whole — six
unmeasurable metric names, including `tick-count` — against screen-jar and
native-library digests written into the record before the change whose effect that
reading reports. So its `launches: 1` and its "exactly one artifact" claim could
not both hold, no digest named the build the reading came from, and the M3-W7
profiling row's provenance broke the rule the same record states about
re-observation. The answer is a fifth campaign rather than a reworded fourth: the
screen jar and the native library were built, staged privately, and hashed *before*
the game started, and the six-name line was read at both GUI scales against those
two digests. The unattributable reading is withdrawn where it stood. Two of the
same round's must-fix findings are fixed in the same pass — the repealed reviewer
note in `matrixNotCarriedForward` is rewritten, and the rewrite count now says
which of the three surface launches reported which pair. What is not answered is
the third must-fix finding, the five moderate ones including the absence of any
M3-specific manifest closure rule, and the six minor ones.

M3-W8 stays open. Every implementation gate is closed, but the review that has to
pass is an *independent* one, and every round so far has returned FAIL. Answering
B-1 is not closure: a sixth independent round decides that, and the fifth round's
remaining findings are still open.

`pack-compile.json` comes from `devenv tasks run ravel:measure-pack-compile` and
is the measurement ADG-2 named as deciding its open mechanism: whether an
interchangeable mod ships as a per-platform binary or as source a bundled
toolchain compiles. It replaces the floor M2-W8 set from generated one-struct
crates, and it decides nothing on its own — the generated crates are ordinary
safe Rust behind the same `HostMod` contract the tracked test mods use, and
nothing in the run loaded a library, started a guest runtime, spawned a mod
process, or exported a foreign symbol.

Its `variants` section comes from a second run,
`devenv tasks run ravel:measure-pack-compile-variants`, which changes one thing
at a time about the curve — four build jobs instead of sixteen, a sixteen-deep
dependency spine, and whole-pack link-time optimization — and splices its results
in beside the curve rather than over it. That section carries its own trial
provenance, its own two-build protocol, and its own control, because a figure
measured in a later run is not the same measurement as the one it sits next to.

It is a measurement rather than a contract. Rerunning it moves the numbers, and
they hold on the `linux-reference-v1` hardware profile and nowhere else; the
record carries that profile's digest so a later run can tell whether it is
comparable. Its `whatThisDoesNotSay` list is part of the record, not commentary
on it: the generated mods are alike, have no dependency graph between them, and
pull nothing outside serde, so every figure is a floor.

The record names crates, sizes, versions, and digests. It names no filesystem
path, no credential, and no local directory.

## `profiling-overhead.json`

It comes from `devenv tasks run ravel:measure-profiling-overhead` and is two
things at once, which is the same shape `evidence/m2/loader-overhead.json` has:
the M3-W6 measurement, and a budget record whose limits a later run is compared
against. `cargo xtask verify budget` takes the union of that file's limits and
this one's, and runs inside `ravel:verify`.

Its `budget` limit is **decided rather than derived**. Every M2 limit is its own
measurement doubled; this one is one per cent of a tick, fixed before the
measurement, because what a player is promised is that profiling costs them
nothing when it is off and that promise is a fraction of a tick rather than
whatever the code happens to do today. A measurement far under it is the expected
result. Only the profiling-*off* figure is bounded: what the collectors cost while
they run is recorded here and gated at M9 against a real pack.

Its `collectors` section is generated out of `ravel-observability` rather than
written, and `tests/conformance/tests/profiling_report.rs` checks it against that
crate. The metric table, the attribution-accuracy statement, and the unsupported
list are the two things M3-W6's row asks to be explicit, and they cannot drift
from the code that ships: a metric that becomes supported, or a reason that
changes, fails the gate until the record is rewritten. The measured half is not
regenerated by that test — a timing differs on every run, and a tracked-file
assertion over one would fail on scheduler noise.

`overhead.perTick` multiplies a per-crossing cost by an *assumed* crossing count,
stated as an assumption everywhere it appears. `overhead.breakEven` beside it
rests on no assumption and is what answers the budget outright once the real
per-tick crossing count is recorded.

Both of those are derived, and following the derivation needs one fact the record
did not state until the fourth W8 review asked for it: every figure is rounded to
two decimals, while the derived pair is computed from the unrounded percentiles
and rounded afterwards. Applying `perTick.formula` to the two-decimal percentiles
therefore misses. In this record the per-crossing 99th percentile is `0.051`
before rounding and `0.05` as published, which is what
0.05 + 10000 × 0.051 = 510.05 and (500000 − 0.05) / 0.051 = 9803920.59 follow
from. The tick-boundary percentile is the one input that is published only rounded,
and the two derived figures recover it to very different precision. `perTick` is
1:1 sensitive to it — 10000 × 0.051 is exactly 510, so a published `510.05` puts
the unrounded boundary in [0.045, 0.055), which is precisely the interval that
rounds to `0.05` and not one digit better. `breakEven` divides by `0.051` instead,
so it moves 1/0.051 ≈ 19.6 per nanosecond of boundary time: across that same
interval it spans 9803920.4902 … 9803920.6863, a range of about 0.196 against a
figure published to two decimals. Read backwards, the published `9803920.59` pins
the unrounded boundary to about [0.0497, 0.0502] — roughly twenty times *narrower*
than the rounding interval, not wider. So the sensitive figure is the one that
recovers the rounded input rather than the one that loses it, and the number a
reader should reconcile against is `breakEven`. `overhead.rounding` states the
rounding rule in the record itself, and
`tests/conformance/tests/profiling_report.rs` fails the gate if that field goes
missing.

## `linux-process-matrix.json`

It comes from launches rather than from a task, because the thing it measures is
a launcher starting a game and no task can stand in for that. It holds these launch
campaigns, in order — the earlier 31-target matrix, the world round trip, the
surface campaign, the caveat re-observation, and the profiling re-observation —
and the most useful thing in it is why the second could not simply extend the
first. That list is the one enumeration of them; nothing else here or in the
record restates how many there are.

**The earlier matrix could not be carried forward.** Two ADG-7 world persistence
seams entered the reviewed inventory — `world.open-persistence` on
`Minecraft.doWorldLoad` and `world.save-persistence` on
`MinecraftServer.saveAllChunks` — and the count moved from 31 to 33 callable
targets over the same 15 classes in the reviewed inventory. A matrix is a claim
about one set of targets in one
build, so a count that moved invalidates the reconciliation rather than adding to
it. The second reason is stronger still: no build the earlier campaign ran
contained a mod that owned world data at all. `ravel-composed-host` carried only
the configuration mod until change `uyvzkzzs` added `ravel-test-mod-saved-data`
to it, so the ADG-7 round trip was unprovable by that campaign whatever input it
received — which is why M3-W4 and M3-W7 stayed open through it and why its
saved-data row persisted a vanilla scoreboard.

The world round trip campaign, the second in that list, is three launches at 33
callable targets, and its claim is narrow on purpose. The rewrite of both persistence methods was already proven
in-gate by `jvm/harness/PersistenceMain.java` over nine world lifecycles; what
these launches add is the real game's storage on both ends of it. A world was
created through the game UI, saved, and exited; a second launch reloaded it and
Ravel's own versioned aggregate came back byte-identical, written by the game's
own `SavedDataStorage` as `data/ravel/mods.dat` and carrying `saved_data` at data
version 3. A third launch loaded the same world with that version rewritten to 9
outside the game, and the load was refused before the integrated server started,
with the diagnostic drawn on a Ravel screen. That third trial is what separates a
reload that read the stored bytes from one that ignored them.

The world itself is gone. It was created, hashed, verified across both the reload
and the refusal, and then removed, which returns the instance to the single-world
shape the M0 launch-proof summary expects. The digest, the reload, and the refused
load are the evidence, and all three are claims rather than things a reviewer can
recompute — the same ceiling stated at the top of this file, not an exception to
it. Removing the world took nothing away from that and bought nothing back: a
private machine-local directory is no more checkable by another reader than a
digest of one.

The third campaign then re-observed the rows the first had left inherited, and it
did so against the *same build* as the world round trip: the agent jar, screen
jar, and native library were rebuilt from the working tree and came out
byte-identical to the digests already recorded, so there is one build and one
inventory of 33 callable targets behind both. Its three launches reported
`rewrote 15 classes
and 33 methods` once a world was open, and `11 classes and 27 methods` on the
title-only launch. That is the same inventory minus the four classes a world loads
— `RecipeManager`, `Commands`, `ReloadableServerResources`, `SavedDataStorage` —
and the six of the eight world-only targets those four hold, so 15 − 4 = 11 and
33 − 6 = 27. The remaining two world-only targets, `world.open-persistence` and
`world.save-persistence`, live in `Minecraft` and `MinecraftServer`, which the
title screen loads and Ravel rewrote there, which is why the subtraction is six
rather than eight. The reason is recorded beside the smaller pair, and so is the
rule that decides what belongs in it: `worldOnlyTargetsCriterion` says a target is
listed when opening a world is what puts it on the path, which is narrower than
being unreachable at the title screen. `server.tick`, `server.tick-integrated`,
`save.save-everything`, and `save.set-dirty` never execute on a title-only launch
either and are deliberately not in the list, because the list accounts for what
gets rewritten rather than for what can run. Neither reading changes the
arithmetic, which is per class and per rewritten method.

Three of the four inherited row groups now stand on their own evidence. All three
Ravel screens were opened from the title menu. The Mods screen listed two running
client mods rather than one, because it reads the loader's runtime store and the
composed host now carries the world-owning mod as well. The restart-required
setting was edited to `quiet`, marked `(next launch)` on the spot, and came back
applied and unmarked after a cold relaunch, with the player document on disk
holding only the value that was changed. A scoreboard objective was created and
mutated through chat in a world created for the purpose, saved through Save and
Quit, and read back at its exact mutated value on a later launch. All three
launches ended through the in-game Quit Game button with no signal, and the
post-exit instance scan again found no other loader.

The fourth **was reached and failed, and now passes.** Profiling is turned on by
the `-Dravel.profiling=true` launch property rather than by a control on the
screen, and with it on the report collects real per-mod call counts and
wall-clock times that advance while the screen is open. But the screen drew each
line as a fixed-width `EditBox`, and an `EditBox` scrolls to follow its cursor,
which `setValue` leaves at the end of the string. Every line wider than the
widget therefore showed its tail and hid its head. Enlarging the window did not
help, because the widget width is fixed rather than derived from the screen.

Both halves of what M3-W6 asks to be explicit were lost to that. The build that
campaign ran declared five metrics unmeasurable — `retained-memory`,
`allocated-memory`, `cpu-time`, `transferred-bytes`, `saved-data-footprint`;
`tick-count` joined them later, in change `lppyxtnz` — and the client showed
`-memory, cpu-time, transferred-bytes, saved-data-footprint`, a tail that begins
inside the word `allocated-memory`. Two of the five names never reached the
player at all, and `-memory` names neither of them. The accuracy caveat fared
worse: what reached the player was `y key is charged for the lookup as well as
for its own code.`, a fragment with no subject, in place of the sentence
explaining that a mod is charged for host work it calls back into.

The defect was deliberately not patched in the same campaign that found it.
Changing the screen jar changes its digest, and every row above was observed
against the digest this record names; the honest way to close it is a fix
followed by re-observation, not a patch landed underneath evidence already
written. So it was fixed on its own — change `npozznqz`, which splits every line
to the width it is drawn at using the game's own font and hands the window's row
budget out in a stated order, claiming the summary and the accuracy statement
first — and then re-observed in the real client against the fixed jar.

That re-observation is the fourth campaign, the `caveatReObservationCampaign`
section of the record, and it is deliberately narrow: one launch, claiming the
profiling surface and nothing else. The agent jar and the native library are
byte-identical to the ones every other row was observed against, so only the class
that drew the clipped lines changed. With profiling on and the instance's existing
world played for a couple of minutes first, both formerly clipped lines wrap
instead of truncating, at GUI scale 1 and at the default scale alike.

**The metric list then changed again, and that needed its own launch.** Change
`lppyxtnz` added `tick-count` to the unmeasurable five, and it changed both the
Rust that builds the list and the screen that draws it. An earlier version of this
record reported that six-name line inside the caveat campaign, whose two recorded
digests were written before `lppyxtnz` — so the campaign was made to publish an
effect its own build could not have produced. The fifth review round found that
and it is what the fifth campaign answers. `profilingReObservationCampaign` builds
the screen jar and the native library, copies both into a private directory,
hashes them there, and only then launches; the claim it records is bound to those
two digests and to nothing else. The withdrawn reading is marked withdrawn where it
stood rather than restated at some other number, because nobody can now say what
that build drew.

What the fifth campaign read, on one title-only launch, is the whole report at
both scales. At GUI scale 1, twelve lines: the summary `2 mods entered` with no
tick figure in it, the wall-clock line, both mod rows with real call counts and
nanosecond totals, all six unmeasurable metric names over three wrapped rows —
`retained-memory`, `allocated-memory`, `cpu-time`, `transferred-bytes`,
`saved-data-footprint`, `tick-count` — the heading, and the accuracy statement
whole from its subject to its full stop over four. At the default scale the record
puts the window's budget at seven rows, and the screen spends them as designed —
summary, wall-clock line, heading, statement — dropping the table and the metric
names rather than truncating anything. Nothing is clipped at either scale, and
this window offers no third one. Reopening the screen redrew the same twelve lines
with the counts advanced, which is what tells a live report from a rendered
constant; within a single opening the payload is a snapshot and does not move. The
record's result is therefore `pass`.

The `M3-W7` gate in [`manifest.json`](manifest.json) is closed on that record
together with the world round trip M3-W4 turns on. Two counters stay named as
gaps inside the closed gate. `tick-count` cannot leave zero, because nothing on
the game's tick path marks a tick boundary at M3; the screen names it unmeasurable
rather than stating a figure it cannot produce. The command counter is a different
case and is not a defect: its increment path runs from the JNI hook entry through
`route_profiled` to `Profiler::charge` and the conformance suite exercises it, and
it read `0 cmd` because the commands typed were vanilla and unnamespaced —
`ContentId::parse` refuses a root literal with no `:`, so no command work is
constructed — and because neither mod in the composed host claims a command.
The counter is accurate. What has never happened is a real client showing it above
zero, and nothing here claims otherwise; demonstrating it live means putting a
command-claiming mod in the composed host and typing a namespaced root, which is a
composition change rather than a change to what the collectors do.

One earlier claim is corrected rather than removed. The record noted a
pre-existing world that held no `data/ravel` directory, the absent-aggregate case
ADG-7 initializes fresh state for. That world has since been opened with Ravel
attached and now holds one, so the observation is no longer re-checkable by
looking at the instance; it stands as something observed at the time, and proving
the absent case fresh would need a world that has never been opened with Ravel
attached. The case is not unproven — it has in-gate proof and no live proof.
`a_world_holding_no_data_this_mod_owns_initializes_fresh_state` in
`tests/conformance/tests/saved_data.rs` runs on every gate and asserts that a
world holding nothing this mod owns initializes fresh rather than refusing.

The record separates the operator attestation — injected input followed the
ordinary title-screen and world-selection route — from facts derived from process
state, the shutdown report, persisted files, and the post-exit instance scan. Raw
logs and screenshots remain private; only sanitized facts and their digests enter
the repository.

Four deviations from the earlier title-only record are retained as explicit
resolutions rather than disappearing from the audit trail:

- **Open world — resolved.** The accepted second launch used injected input through
  Singleplayer and the real world-selection screen, then exercised command and
  saved-data paths, saved, relaunched, and verified the exact persisted value.
- **Launch-time caller — resolved.** The source-composed production host populated
  the runtime and configuration stores during premain. The Mods, settings, and
  profiling screens opened from the title menu and consumed production `Hook`
  data.
- **Unaccounted target — resolved.** `lwysptxkplzu` changed class-hierarchy
  resolution to parse resources without loading game classes. Per-class matched
  counts and failure reporting then reconciled `TitleScreen` and every other
  reviewed class: 15 classes, 31 callable methods, zero transformation failures.
  The campaigns recorded here reconcile the same 15 classes in the reviewed
  inventory at 33 callable targets, again with zero transformation failures.
- **Title-only/SIGTERM — superseded.** The passing trial opened a world, used Save
  and Quit to Title, selected Quit Game, and let Java and Prism exit naturally.
  Earlier title-only or forced-cleanup runs remain private historical evidence and
  are not the accepted trial.

The game-directory footing is also measured directly: before Ravel was attached,
the stock Prism launch's process working directory was the instance game root and
remained so through the title screen. The post-exit scan found exactly the stock
Minecraft and LWJGL components, no other loader component, and no mod JAR.
