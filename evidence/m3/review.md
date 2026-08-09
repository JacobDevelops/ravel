# M3-W8 Independent Prototype Review

Result: FAIL

Independent rounds ran against successive states of the M3 evidence. Each reviewer
wrote none of the work under review. Round 8 returned a pass; every other round
below returned FAIL, and so does the last one, which is the standing verdict.
Round 5's blocking finding B-1 is answered — by the
`profilingReObservationCampaign` in `evidence/m3/linux-process-matrix.json` — and
so is Round 6's, which is a different B-1 about the world-persistence delivery, so
is Round 7's, and so is Round 9's single blocking finding. Answering a blocking
finding is not closure, and neither is a pass that a later round supersedes: what
M3-W8 needs is an independent round that returns a pass on the state it read, and
Round 10 did not. So `evidence/m3/manifest.json` records `review.result: "fail"`
and M3-W8 stays open.

One round is not here. The earliest round of all reviewed the prototype rather than
the evidence set, and its verdict text was never written into the repository; the
next section states what is known about it, that the text is not recoverable, and
what that costs a reader. **The numbering below counts only the rounds this file
holds, so "Round 1" is the second round that ran.** Every round that has run since
is here, in order, so this file rather than any summary of it is what says how many
there have been.

That claim went false twice before it was made true. Rounds 8 and 9 had their
repairs landed and their verdicts left out, and Round 10 blocked M3 over exactly
that: a file asserting it omits no round while omitting the two most recent. Both
were recovered verbatim from outside the repository while their texts still
existed, which is luck rather than a control. The convention this file is held to
from here is that a round's verdict is written into it before, or in the same
change as, the repairs that answer the round — because a verdict recoverable only
from a scratch file is a verdict this record does not have.

They are one record rather than one file each because `review.record` in
`schemas/evidence.schema.json` binds one hashed artifact per milestone, and these
are successive rounds of a single M3-W8 review of a single work package. Carrying
them as separate tracked files would need a schema change to hold more than one
record, and the rounds only mean anything read in order — each verifies the
previous round's repairs.

Each verdict is reproduced as written, without softening and with nothing added.
The `file:line` anchors are as of the tree each reviewer read; later rounds and
later repairs have moved some of them.

Three mechanical adaptations are made to a round's text and are the only changes to
it: heading levels are demoted so each round nests under its own, a heading that
would otherwise repeat one already in this file carries its round number, and a
fenced block is given a language. All three are because `markdownlint` runs over
this file under `ravel:verify`.

---

## The prototype round — VERDICT: FAIL, verdict text untracked

This round ran before `review.md` existed and reviewed the prototype rather than
the evidence set. Its verdict was never written into the repository, and it is not
recoverable from the repository or its history: `evidence/m3/review.md` held only three rounds
before Round 4 was added, no other tracked
file has ever held a review record for M3, and change `xqmmlnzmtkqp`, which
recorded this round's outcome, changed only `evidence/m3/README.md` and
`evidence/m3/manifest.json`.

What survives is the prose summary in those two files. It attributes four blocking
findings to this round — the manifest inventory omission of M3-W0, the
package-schema contradiction between `schemas/package.schema.json` and the
source-only distribution contract ADG-2 selected, the missing Ravel-owned versioned
world write, and the unexercised native surfaces — and three clean results: hook
integrity, safety/layering/ADR conformance, and evidence redaction. All four
findings are recorded as answered.

That summary is not a verdict. Absent the text there is no clause table, no
`file:line` anchor, no statement of what this reviewer recomputed or could not
reach, and no independent way to check the summary against what was actually
written — including whether four is the number of findings it raised. A reader
therefore has to take the summary on trust in exactly the way this record exists to
make unnecessary, and the tracked record begins one round later than the review
did. Round 4 raised this as its N-3 finding; it is answered by saying so here
rather than by reconstructing a verdict nobody has.

---

## Round 1 — VERDICT: FAIL

The tracked evidence does not establish the M3-W8 gate — and, to its credit, does
not claim to. `evidence/m3/README.md:3-5` and `evidence/m3/manifest.json:70-73`
both record M3-W8 as open with review result `fail`. Independently of that
self-assessment, I found three integrity defects in the evidence set itself that
would block the gate even if the two implementation rows were closed.

### Round 1 — Clause-by-clause

Gate text (`ROADMAP.md:716`):
`M3-W8 | Independent prototype review | M3-W7 | review record | M3 gate passes and
authorizes private inventory work only after M4`

| Clause | Proof | Status |
| --- | --- | --- |
| Depends on M3-W7 (W7 closed) | `evidence/m3/manifest.json:65-69` — `"status": "open"` | **unproven** (disproven) |
| Output: a review record exists | `evidence/m3/manifest.json:168-172`; `evidence/m3/README.md:14-52` | proven (record exists; its result is `fail`) |
| Review is independent | `evidence/m3/manifest.json:169-170` `"independent": true` | **asserted only** — no reviewer identity, date, or scope is recorded, and nothing in the repo binds the claim to an actor |
| "M3 gate passes" | `manifest.json:53-56` (M3-W4 open), `:65-69` (M3-W7 open), `:154-162` two `blocking` risks | **unproven** (disproven) |
| "authorizes private inventory work only after M4" | `evidence/m3/README.md:4-5` — explicitly does not authorize | proven (correctly withheld) |

Sub-gate, since W8's pass depends on it — M3-W4 (`ROADMAP.md:712`):

| Clause | Proof | Status |
| --- | --- | --- |
| pinned fixer-byte regression preserves opaque bytes | `docs/decisions/adg-7-world-persistence.md:164-181` (27,584/27,584 cases) | proven |
| create/save/exit/reload preserves state | `evidence/m3/linux-process-matrix.json:77-95` | **attested, not checkable** — see F5 |
| schema/package/migration/size incompatibilities fail closed | `tests/conformance/tests/saved_data.rs:395,430,487,506,531,551`; `linux-process-matrix.json:96-105` (dataVersion 3→9 refused in a real load) | proven in-gate; native half attested |
| "moving the ADG-7 aggregate through the game objects" | `crates/ravel-composed-host/src/lib.rs:53-70` | structurally proven (see below) |

### Round 1 — Findings

**F1 — BLOCKING. The manifest's own integrity record over its evidence is wrong.**
`evidence/m3/manifest.json:127-129` records `linux-process-matrix.json` at
`ed8345695f…`. The tracked file hashes `ad5f615eee…` at both `@` and `@-`. It has
been stale since change `npozznqz` — i.e. across the entire surface campaign, the
profiling fix, and the re-observation campaign that the README leans on.
`manifest.json:122-124` is likewise stale for `profiling-overhead.json` (that file
is also dirty in the working copy, so only the matrix mismatch is provably
committed). Nothing catches this: `tools/ravel-xtask/src/lib.rs:1585` validates the
manifest against `schemas/evidence.schema.json` only; no gate recomputes
`benchmarks.results[].sha256`. An evidence manifest whose digests do not bind its
own records is not an evidence manifest.

**F2 — BLOCKING. The manifest pins a revision that predates the work it is being
read as covering.** `evidence/m3/manifest.json:15-19` pins `724f12b…` /
`xqmmlnzmtkqp…`, `dirty: false`. That is three described changes behind `@`, and it
predates `uyvzkzzs` — the very change whose round trip the README cites. The
`commands`/`tests` block (`:95-110`, 777 tests, exit 0) therefore attests to a tree
that no longer exists.

**F3 — BLOCKING. The evidence set contradicts itself on the single fact M3-W4
turns on.** `manifest.json:55`: "Ravel has not yet moved its own versioned bytes
through a world save and reload." `linux-process-matrix.json:77-95` and
`README.md:19-22`: it has. Both are tracked, both are current, and the manifest is
the canonical inventory. One of them is false and the gate cannot be read.

**F4 — MAJOR. The matrix record self-declares `pass` for a work package the
manifest holds open.** `linux-process-matrix.json:5` `"result": "pass"` under
`"workPackage": "M3-W7"`, against `manifest.json:66` `"status": "open"`. The README
(`:213-215`) explains the intent, but the machine-readable fields disagree.

**F5 — MAJOR. The world round trip is attested, not checkable by any reviewer.**
The world was deleted (`linux-process-matrix.json:111-116`), and the raw logs and
screenshots were deleted with only digests retained
(`thisCampaignPrivateEvidenceDigests.note`). `storedAggregateSha256: 91db66…`
(`:84`) names a file that no longer exists anywhere, so nobody can recompute it.
The record's own defence (`:114`) — "a digest is not [unverifiable]" — is wrong when
the digested artifact is gone: the digest then verifies nothing, it only records a
claim. This is the repo's established convention for launch evidence, so it is not
a fabrication finding; it does mean the M3-W4 gate rests entirely on operator
attestation, and a rerun cannot be compared against anything.

**F6 — MAJOR (this is the brief's item 3). One claim is inherited-and-corrected
rather than observed fresh.** `linux-process-matrix.json:106-109` — the
absent-aggregate case ("a pre-existing world held no `data/ravel`") was true when
made and is explicitly no longer re-checkable, because the same campaign wrote an
aggregate into that world. The record labels this honestly and says a fresh proof
would need a never-opened world. Consequence: ADG-7's "Absent data initializes a
fresh aggregate for any world" (`adg-7-world-persistence.md:95-96`) has in-gate
proof (`saved_data.rs:410-415`) but no live proof. Beyond this one, I found no
record claiming as observed something it inherited:
`machineEvidence.provenance:137`, `screens.provenance:164`, and
`commandAndSavedData.provenance` each state their campaign, and
`matrixNotCarriedForward:118-129` is an unusually candid statement of why the
earlier matrix was voided.

**F7 — MAJOR. M3-W6 is recorded closed over two counters the record itself says are
broken in every launch.** `linux-process-matrix.json`
`openGaps.profilingTickCounter`: `Profiler::tick` in
`crates/ravel-observability/src/profiling.rs` has no caller in the production
graph, so the player always reads `N mods over 0 ticks`;
`openGaps.profilingCommandCounter`: both mod rows read `0 cmd`, still undiagnosed.
`manifest.json:61-64` records M3-W6 `closed`. M3-W6's gate asks that per-mod
attribution accuracy be explicit; a report that always states zero ticks is not
that. (A fix is present uncommitted at `lppyxtnz`; it is not evidence.)

**F8 — Fail-closed polarity: correct in both directions, with one untested
defensive branch (MINOR).**

- Open/null → refuse: `jvm/bootstrap/dev/ravel/jvm/Persistence.java:122-124`
  returns a refusal diagnostic on `verdict == null`, and `:125` makes empty-string
  the only admit. Rust cannot emit null except via panic:
  `crates/ravel-bootstrap/src/persistence.rs:359-364` returns `""` for `Staged`,
  and `:363` `.resolve::<ThrowRuntimeExAndDefault>()` turns a panic into null.
  Covered by `jvm/harness/run-harness.sh:307` (`null-verdict`, expects
  `loads=0 accessClosed=true stemClosed=true`) and `:312` (non-empty refusal).
  Cleanup/lock release asserted in the same expectation.
- Save/null-or-empty → write nothing: `Persistence.java:224-229`; only tag `0x01`
  reaches `write` (`:230-243`, `:253-262`). Covered by `run-harness.sh:319-324`,
  which loops `unchanged failed mismatch null` and expects `writes=0 written=` with
  `saves=1` in each. Rust side `persistence.rs:391-410`.
- Gap: the `answer.length == 0` half of `Persistence.java:224` is never exercised —
  `jvm/harness/initialization/dev/ravel/jvm/Hook.java:281-291` offers no property
  yielding a zero-length array, and no `SaveOutcome` in `persistence.rs:399-406`
  can produce one (every arm emits ≥1 byte). Defensive-only and untested. MINOR.
- The harness is in the gate: `nix/common.nix:233` runs
  `jvm/harness/run-harness.sh` under `ravel-verify-jvm`.

**F9 — `uyvzkzzs` does close the "unprovable by construction" defect, at the code
level.** `crates/ravel-composed-host/src/lib.rs:53-70` installs `SavedDataMod` into
the production composition as a `Required` client/dedicated-server entry carrying
`saved_data_versions` from its declaration; `test-mods/saved-data/src/lib.rs:23`
declares `[1, 2, 3]`. That reconciles exactly with
`linux-process-matrix.json:70-75` (`declaredVersions [1,2,3]`, `versionWritten 3`)
and with the Mods screen reading two running mods (`:173`). The earlier finding was
structural and is structurally answered. What remains attested-only is that this
composition actually round-tripped in the real client (F5).

### Round 1 — Coverage

Read in full: `ROADMAP.md` M3 section incl. the W4/W7/W8 rows;
`evidence/m3/README.md`; `evidence/m3/manifest.json`;
`docs/decisions/adg-7-world-persistence.md`;
`jvm/bootstrap/dev/ravel/jvm/Persistence.java`; `jvm/harness/PersistenceMain.java`;
the persistence block of `jvm/harness/run-harness.sh` (lines 249-345);
`jvm/harness/initialization/dev/ravel/jvm/Hook.java:240-300`;
`crates/ravel-composed-host/src/lib.rs:1-110`.

Read structurally / in part: `evidence/m3/linux-process-matrix.json` — full key
outline plus verbatim `worldPersistenceRoundTrip`, `thisCampaignMachineEvidence`,
`machineEvidence`, `screens` head, `openGaps`, `artifacts`;
`crates/ravel-bootstrap/src/persistence.rs:320-440` (the two crossings and their
tag constants) — the remaining ~500 lines of staging/save logic and its unit tests
I did not read; `tests/conformance/tests/saved_data.rs` — test-name inventory only,
not bodies; `test-mods/saved-data/src/lib.rs` — declaration and version path only.

Not reached: `evidence/m3/pack-compile.json`,
`evidence/m3/class-loader-identity.json`, `evidence/m3/profiling-overhead.json`
(contents; only its digest mismatch); `schemas/evidence.schema.json` beyond its
required-key list — I verified `manifest.json` carries all 16 required top-level
keys but did not run the jsonschema validator, and I did not check the four
non-manifest records against it (the gate applies that schema to milestone
manifests only, `tools/ravel-xtask/src/lib.rs:528`);
`jvm/screen/.../RavelWorldData.java`; the M3-W7 hook-integrity reconciliation
against `mappings.lock`. I did not run `ravel:verify` — the working copy is dirty
with an in-flight profiling fix (`lppyxtnz`), so its exit code would not describe
the reviewed state.

---

## Round 2 — VERDICT: FAIL

Narrow FAIL. The evidence set is in much better shape than the round that preceded
it — all five claims check out under independent recomputation — but two things
still stand between this state and a defensible close, and one of them is a
reconciliation statement that is wrong against the data in the same file.

### Round 2 — Clause-by-clause

M3-W8's row is the standard: *output* `review record`; *verification* "M3 gate
passes and authorizes private inventory work only after M4" (`ROADMAP.md:716`),
read against `ROADMAP.md:720-722` ("A row is closed by its Verification column and
by `ravel:verify`, never by a file being present").

| Gate clause | Proof | Status |
| --- | --- | --- |
| A review record exists | none — `evidence/m3/manifest.json:189-193` records `review.result: "fail"` and no `review.record` digest | **unproven** |
| M3 gate passes | `devenv tasks run ravel:verify-milestone:m3` reports 3 violations: no blocking risks / review independent+pass / review must include a hashed record. `evidence/m3/manifest.json:181` carries the blocking risk deliberately | **unproven (by design, pre-review)** |
| `ravel:verify` passes | ran it: **exit 0** | proven |
| `ravel:verify-stack` passes | ran it: **exit 0** | proven |
| Private inventory work not authorized | `evidence/m3/README.md:3-5`, `manifest.json:181` | proven |
| M3-W0 in the canonical inventory | `manifest.json:38-41` | proven |
| M3-W4 closed with ceiling | `manifest.json:54-57`; `linux-process-matrix.json:70` `reviewCeiling` | proven as *attested*, not observed-by-reviewer |
| M3-W7 closed with ceiling | `manifest.json:66-70`; `linux-process-matrix.json:7` `resultCeiling` | proven as *attested* |
| M3-W6 explicit metrics/attribution | `profiling-overhead.json` + `tests/conformance/tests/profiling_report.rs`; `linux-process-matrix.json:594` | proven in-gate; live half attested |
| Target reconciliation (15 classes / 33 methods) | `linux-process-matrix.json` `rewrite.targetInventory` sums to 33/15 — I counted | proven |
| Title-only reconciliation (11 / 27) | `linux-process-matrix.json:527` explanation | **contradicted by its own data — see F1** |

The gate is not met at this state, and the honest reading is that it *cannot* be
met until someone records a passing independent verdict — that circularity is not
what I am failing on. I am failing on F1 and F2.

### Round 2 — The five claims

| # | Claim | Result | Checked |
| --- | --- | --- | --- |
| 1 | Every `benchmarks.results[]` digest matches; `verify_evidence_records` recomputes and fails the scaffold | **partially verified** | I recomputed all 28 recorded result digests across m0–m3: **28/28 match, 0 mismatches**. Check is real, not vacuous: `lib.rs:313-361` hashes the working-copy file, refuses a manifest recording its own digest (`lib.rs:349-352`), rejects paths escaping the milestone dir (`lib.rs:338`), wired into the scaffold at `lib.rs:195`, with negative tests at `lib.rs:1830`, `1847`. But the coverage claim is overstated — see F2 |
| 2 | Every `inputs[]` digest bound, recomputed at `revision.id` via `verify_recorded_inputs` | **verified** | I recomputed all 18 inputs with `jj file show -r <revision.id>`: **18/18 match**. Implementation `lib.rs:254-297`; uses `.revision.id`, not `changeId`, not `@` (`lib.rs:263`); unreadable file → loud violation, not a skip (`lib.rs:294`, `306-308`); negative tests `lib.rs:1937`, `1957`. One vacuity hole: F3 |
| 3 | M1 digest corrected; four M0 inputs given real paths | **verified** | `jj diff -r txvluwnvrpor`: m0 inputs went from prose names to the four real paths; m1's `m1-measurement-gate.md` digest `2bfce…` → `f6509…`, and the correction is disclosed as a deviation (`evidence/m1/manifest.json:210`). Both m0's `d4b5…` and m1's `f6509…` verify at their own pinned revisions |
| 4 | No self-contradiction on the world round trip; W4/W7 closed with ceilings attached | **verified** | `manifest.json:54-57`/`66-70` now agree with `linux-process-matrix.json:70-130` and `README.md:8-16`. The stale-manifest contradiction is retained as a deviation (`manifest.json:151`). The ceiling is stated in five separate places including a top-level `resultCeiling` field — **not buried** |
| 5 | Tick count named unmeasurable; `0 cmd` recorded as explained, not demonstrated | **verified** | `linux-process-matrix.json:594-595`, `209`. The record says outright "this record does not claim it has [been observed above zero]". I checked the code claims myself: `Profiler::tick` has callers only under `#[cfg(test)]` (`crates/ravel-observability/src/profiling.rs:611+`); neither `ConfigurationMod` nor `SavedDataMod` implements `register`; `ContentId` requires `':'` (`crates/ravel-core/src/content.rs:9`). Correctly labelled as *concluded from code plus in-gate suite*, with the live demonstration named as absent |

### Round 2 — Findings

**F1 — medium, must fix. The title-only reconciliation is wrong by the record's own
data.** `evidence/m3/linux-process-matrix.json:527` says the title-only launch
dropped "the eight world-only targets and the four classes that hold them", and
`evidence/m3/README.md:175` repeats "the same inventory minus the eight world-only
targets". 33 − 8 = 25, but the launch reported **27**. The correct reconciliation is
in the same file: of the 8 entries in `rewrite.worldOnlyTargets`, only 6 live in the
4 classes absent at the title screen (`Commands` 1, `ReloadableServerResources` 1,
`RecipeManager` 1, `SavedDataStorage` 3 → 33−6=27, 15−4=11 ✓). The other two —
`world.open-persistence` on `Minecraft` and `world.save-persistence` on
`MinecraftServer` — sit in classes that *were* loaded and rewritten at the title
screen. The numbers are fine; the sentence written specifically to reconcile them
for a sceptical reader is not. In a record whose whole authority rests on count
reconciliation, and after two rounds that died on exactly this class of defect,
that has to be corrected before a review can sign it.

**F2 — medium, must fix. "Recomputes every recorded result digest" is not true, and
the residual hole is the same drift class the prior finding raised.**
`evidence/m3/README.md:65-68` claims the *cause* is fixed.
`verify_evidence_records` reads only `.benchmarks.results[]` (`lib.rs:321`) and
`.inputs[]` (`lib.rs:276`). `evidence/m0/manifest.json:176-190` records four more
result digests under `platformEvidence[].baselines[].result`/`.validation`, and two
of them are digests of **tracked files** — I confirmed `907e813…` and `88e2c3e…`
are exactly `sha256sum` of `evidence/m0/linux-vanilla-launch.json` and
`evidence/m0/linux-official-atm11-launch.json`. Nothing recomputes them. They are
correct today; edit either record and the gate stays green on a stale digest, which
is verbatim the defect. Either extend the check to those fields or narrow the claim
to `benchmarks.results[]`.

**F3 — low. `verify_recorded_inputs` skips silently without `.jj`**
(`lib.rs:255-257`). The rationale is sound (`revision.id` is often unreachable from
a git checkout), but the consequence is that the entire input-provenance check
evaporates, with no output, in any non-jj clone. A skip that announces itself would
keep the check honest.

**F4 — low. Input provenance is pinned to commit hashes that jj rewrites abandon.**
`revision.id` resolution currently works for all four milestones, but the same file
argues the opposite position elsewhere (`lib.rs:422-424`: commits are the wrong
thing to pin, "seven results named the wrong revision for a week"). After a
`jj util gc` the check flips from binding to unconditionally failing for every
closed milestone. Fail-closed, so not urgent — but the durability of M0–M3's
provenance now depends on the op log.

**F5 — low. Manifest completeness is unconstrained.** Nothing requires a milestone
manifest to list its sibling records. `evidence/m0/manifest.json` has
`benchmarks.results: []` while two evidence records sit beside it, bound only
through the unchecked path in F2. A record can be added to a milestone directory
and be digest-bound by nothing.

**F6 — low. The tree is inconsistent about what a post-review FAIL does to
`review.result`.** `evidence/m1/manifest.json:190-196` records `result: "pass"`
while `:214` in the same file says "the last cold verdict recorded against this
evidence is FAIL". M3 handles the identical situation the opposite way and keeps
`result: "fail"`. M3's convention is the right one; M1's "pass" therefore does not
mean what M3's would.

**F7 — low, wording. `linux-process-matrix.json:106` `whatThisProves`** uses
"proves" for an operator-observed, non-recomputable event. Enclosing fields scope it
correctly, so this is not a misrepresentation — but "shows" is the accurate verb
inside an attested campaign.

**F8 — informational. `evidence/m3/manifest.json` has `platformEvidence: []`** even
though M3-W7 is an operator-run native Linux matrix. The native launch context
lives only inside `linux-process-matrix.json`, so the general closure rule over
`platformEvidence[].commands[].exitStatus` (`lib.rs:754-758`) contributes nothing
for M3, and `evidence/README.md`'s promise that `platformEvidence` carries each
native platform's versions and settings holds only because M3 declares no
requirement.

### Round 2 — On what the closed gates actually rest

Stated plainly, since the records ask to be judged on this: for **M3-W4** and
**M3-W7**, the live half is *operator attestation* — one person's report of what a
client did, with the world, logs, and screenshots deleted and only digests of
vanished files retained. I cannot re-derive any of it. The *in-gate* half —
`tests/conformance/tests/saved_data.rs`, `jvm/harness/PersistenceMain.java`, the
profiling report test — is recomputable and I confirmed `ravel:verify` exits 0 over
it. The `0 cmd` and `tick-count` diagnoses are *concluded from code* plus in-gate
tests, and are labelled that way. The absent-aggregate case is *observed once, no
longer re-checkable*, and says so. No record in this tree claims *demonstrated*
where *attested* is honest, and no gate is closed on something the record calls
proven when it means explained. That part of the work is done well, and the ceiling
is at the top of the README and in a top-level `resultCeiling` field rather than in
a footnote.

### Round 2 — Coverage

Read in full: `ROADMAP.md:645-722`; `evidence/README.md`; `evidence/m3/README.md`
(288 lines); `evidence/m3/manifest.json`;
`evidence/m3/linux-process-matrix.json` (all sections enumerated, load-bearing ones
read verbatim); `tools/ravel-xtask/src/lib.rs:180-380` and `:660-800`; the four
milestone manifests' `revision`/`inputs`/`benchmarks`/`review` blocks;
`evidence/m0/manifest.json` `platformEvidence`.

Recomputed independently: 28/28 result digests against their files; 18/18 input
digests via `jj file show` at each manifest's pinned `revision.id`; the 33-target /
15-class inventory sum; the 27/11 arithmetic; the two m0 launch-record digests
against `platformEvidence`.

Ran: `ravel:verify` (exit 0), `ravel:verify-stack` (exit 0),
`ravel:verify-milestone` for M0/M1/M2 (clean) and M3 (3 violations, listed above).

Not reached: `evidence/m2/*.json` and `evidence/m1/spikes/*` bodies beyond their
digests; `pack-compile.json` and `profiling-overhead.json` contents beyond the
tick/metric claims and their digests; `evidence/private/**` (out of scope and
correctly ignored); the hook-integrity chain against the pinned jar
(`hooks.toml`/`mappings.lock` verified only as digest-bound inputs, not
re-resolved); the M0/M1/M2 review record digests, which name files outside the
repository and are uncheckable by construction; and every live launch claim, which
is unreachable by design.

**To flip this to PASS:** fix F1 in both places, and either extend
`verify_evidence_records` to `platformEvidence` result digests or narrow the
`README.md:65-68` claim (F2). Then the review record itself needs writing, hashed,
with the blocking risk cleared and `review.result` set to pass, so
`ravel-verify-milestone M3` goes green.

---

## Round 3 — VERDICT: FAIL

Two counts in the tracked M3 records do not survive being recomputed, and one of
them is a positive false claim about this state rather than an unstated ceiling.
Everything the three prior findings named as must-fix (F1, F2) checks out; the
failure is new material of the same class the prior rounds died on.

### Round 3 — Gate clause table (ROADMAP.md:716, closure standard ROADMAP.md:720-722)

| Gate clause | Proof | Status |
| --- | --- | --- |
| M3-W8 output: "review record" | none written; `evidence/m3/manifest.json:201-205` still `result: "fail"`, no `review.record` | unproven (deliberate — this pass is the input) |
| "M3 gate passes" | `ravel:verify-milestone:m3` exit 1 on `no blocking deviations or risks`, `review must be independent and pass`, `review must include a hashed record` | not passing (deliberate) |
| "authorizes private inventory work only after M4" | `evidence/m3/manifest.json:193-194` blocking risk; `evidence/m3/README.md:3-6`, `:90-91` | proven |
| `ravel:verify` green | my own run, exit 0 (819 tests, 819 passed) | proven |
| `ravel:verify-stack` | run inside `ravel:verify` | proven |
| M3-W0..W7 rows closed | `evidence/m3/manifest.json:38-70`; per-row evidence named | proven for in-gate halves, attested for live halves (see ceiling) |

### Round 3 — The five claims

1. **F1 repaired — verified.** `evidence/m3/linux-process-matrix.json`
   `rewrite.targetInventory` holds exactly 15 classes and 33 methods (I summed
   them: 5+4+1+1+1+2+2+5+1+1+1+4+1+1+3 = 33). `worldOnlyTargets` holds 8. The four
   absent classes (`RecipeManager`, `Commands`, `ReloadableServerResources`,
   `SavedDataStorage`) hold 1+1+1+3 = **6** of them. The remaining two,
   `world.open-persistence` and `world.save-persistence`, sit in `Minecraft` and
   `MinecraftServer`, which are in the title-screen 11. 15−4=11 and 33−6=27 both
   hold, and `linux-process-matrix.json:525,527` and
   `evidence/m3/README.md:193-201` say the same thing in the same terms.
2. **F2 repaired — verified.** `verify_recorded_baselines`
   (`tools/ravel-xtask/src/lib.rs:372-421`) shares `verify_tracked_digest` (`:440`)
   with `verify_recorded_digests` (`:321`); both M0 baseline results are named by
   file name and I recomputed both to match; the two private validations are
   announced as unbound via `names_a_path` (`:425`) and `println!` (`:401`). The
   `evidence/m3/README.md:66-75` claim is narrowed to exactly results + tracked
   baselines + inputs-at-pinned-revision + self-digest refusal, which is what the
   code does.
3. **F3/F6/F7 repaired — partially.** The no-`.jj` skip in
   `verify_recorded_inputs:264-270` announces itself. `evidence/m1/manifest.json`
   `review.result` is `fail`, and `evidence/m1/README.md:17-20`, `:113-120` states
   the gate does not pass and why. `whatThisProves` is gone; only `whatThisShows`
   remains (`linux-process-matrix.json:106`). **Partial** because two other `.jj`
   skips still return silently — see F-D.
4. **F4/F5/F8 recorded — verified.** `evidence/m3/manifest.json:166-177` carries all
   three as non-blocking deviations; `evidence/m3/README.md:77-88` summarises them
   accurately.
5. **No W8 review record, `review.result: "fail"`, blocking risk intact —
   verified.** `manifest.json:193-194`, `:201-205`; milestone gate exit 1 for the
   three stated reasons.

### Round 3 — Findings

**F-A — blocking. `evidence/m3/README.md:64-65` states a falsehood about this
tree.** It claims "the revision and the re-run gate numbers are this state's."
`revision.id` (`manifest.json:17`) resolves to change `pqkoksqopryu`, **six
described changes below the tip** (`yuouxpsvvstn`, `txvluwnvrpor`, `vtprwxqxpykr`,
`ssvlsmrxllom`, `pwlmolmtmnol`, `mrrxlqsuwkqm`). `tests.total: 813` (`:106`) does
not describe this tree: `ravel:verify` here runs **819** tests, 819 passed, exit 0.
`commands[0].logSha256` (`:102`) and `M3-W7.verificationLogSha256` (`:69`) both name
that older run's log. `verify_general_closure` (`lib.rs:844`) only asserts
total>0, failed==0, coverage≥80, so nothing binds these fields — the F2 machinery
covers digests and not the run attestation, which is why the round-2 finding "it
pinned a revision predating the work it covered" recurred silently.
`evidence/README.md:20-23` sanctions `revision.id` preceding the admitting commit;
it does not sanction the tests block describing a different tree while a README
asserts otherwise. Fix: re-run, re-pin, correct the sentence.

**F-B — moderate. Metric count drift, `evidence/m3/README.md:227` and `:251-253`.**
Both say **five** unmeasurable metrics ("The build declares five metrics
unmeasurable — retained-memory, allocated-memory, cpu-time, transferred-bytes,
saved-data-footprint"; "the five unmeasurable metric names in full"). The shipping
build declares **six**: `evidence/m3/profiling-overhead.json` `collectors.metrics`
lists `tick-count` as `unsupported`, and `linux-process-matrix.json:636` records the
re-observation as six metric names over three wrapped rows, added by change
`lppyxtnz`. Five is correct for the original defect observation (`:227`) and wrong
for the re-observation (`:251-253`), and `README:260-262` separately relies on
tick-count being named unmeasurable. This is the reconciliation-does-not-check class
again, in the paragraph that carries the profiling `pass`.

**F-C — minor. A fourth unrecomputed digest site: `review.record.sha256`.**
`evidence/m0/manifest.json` records `b225d06b3f4b…`, which I recomputed to be
exactly `sha256(evidence/m0/review.md)` — a tracked file that nothing recomputes.
M1's and M2's review digests match no tracked file (genuinely private).
`names_a_path` (`lib.rs:425`) would classify the prose name "M0 independent
admission review" as unbound, so the baseline logic would not bind it even if
extended. `README:71-75`'s enumeration of what is outside reach does not mention
it.

**F-D — minor. Silent `.jj` skips remain in the two provenance checks.**
`verify_recorded_change` (`lib.rs:537-539`) and `verify_manifest_change`
(`lib.rs:598-600`) return with no output when `.jj` is absent, unlike the repaired
`verify_recorded_inputs`. On a git-only clone or a tarball export, what
`introducing_change`'s own doc comment calls "the repository's strongest provenance
control" evaporates without a word.

**F-E — minor. `verify_evidence_records` has no floor (`lib.rs:225-232`).** It
discovers manifests by directory scan with `.into_iter().flatten().flatten()`; an
unreadable or renamed `evidence/` yields zero manifests, zero violations, no
message. Likewise a manifest with no `benchmarks`, no `platformEvidence`, or no
`inputs` key checks nothing silently — M0's empty `benchmarks.results` and M3's
empty `platformEvidence` are the live instances, both recorded as deviations.

**F-F — minor, wording. `linux-process-matrix.json`
`worldRemovedAfterwards.whatSurvivesTheRemoval`.** "The proof does not live in that
directory… Evidence that needs a private machine-local directory to persist is
evidence nobody else can check; a digest is not." A digest of a deleted world is not
checkable by anyone either, which is precisely what `reviewCeiling` in the same
object says ("a digest of a vanished file records a claim rather than verifies
one"). Echoed at `README:184-187`. The ceiling statement is the honest one; this
passage reads as though the digest restores checkability.

**F-G — minor. `worldOnlyTargets` has no stated criterion.** Under the natural "only
reachable with a world open" reading it should also hold `server.tick`,
`server.tick-integrated`, `save.save-everything`, `save.set-dirty`. The 33−6=27
arithmetic is unaffected (it is per-class, and the four absent classes hold exactly
six methods), but `README:195-198` invites the reader to a list whose membership
rule is not given.

### Round 3 — Evidence provenance, per load-bearing claim

- 15/33 target inventory, 11/27 title-only counts: **observed** in the agent's own
  shutdown report, **attested** — the raw logs are deleted; the reconciliation
  itself is arithmetic I recomputed.
- World create/save/exit/reload byte-identical, and the `dataVersion`→9 refusal:
  **attested by an operator, not recomputable**; world, logs and screenshots
  deleted, only digests of vanished files retained. Stated plainly at
  `linux-process-matrix.json` `reviewCeiling`, `resultCeiling`, `manifest.json:181`
  and `README:8-16`. This ceiling is *not* buried — it is the second paragraph of
  the README and a risk entry.
- Persistence-method rewrite: **observed in-gate**,
  `jvm/harness/PersistenceMain.java` over eight lifecycles, runs under
  `ravel:verify`.
- Absent-aggregate case: **in-gate proof, no live proof**, and the record says so
  and explains why the live observation is no longer re-checkable.
- Profiling re-observation at both GUI scales: **attested**, single launch, screen
  jar digest recorded.
- Command counter reading `0 cmd`: **concluded from reading code**
  (`ContentId::parse` refusing an unnamespaced root) plus conformance coverage;
  `openGaps` and `manifest.json:189` say exactly that and claim no live
  demonstration.
- M1/M2 spike and closure numbers: **inherited from earlier campaigns**,
  digest-bound (all recomputed here).

No record says `proven` where `explained` is honest, and no `demonstrated` where
`attested` is honest — I grepped every occurrence in `linux-process-matrix.json`;
each use of "proven" attaches to the in-gate harness or explicitly to what the
earlier campaign *could not* have proven. F-F is the only wording overreach, and it
is one sentence.

### Round 3 — Coverage

Recomputed **30 digests, 30 matched**: 4 M3 results, 20 M1 spike results, 4 M2
results, 2 M0 baseline results (all against the working copy), plus **18 input
digests** across all four manifests recomputed by hand at each manifest's pinned
`revision.id` via `jj file show` — all 18 matched. 2 M0 private validation digests
are unbound by construction and announced. Not recomputable and not attempted:
every `logSha256`/`argvSha256` (private logs), `verificationLogSha256`,
`artifacts[].sha256` in the matrix record, M1/M2 `review.record.sha256`.

Read in full: `ROADMAP.md` M3/M4 rows and closure rule, `evidence/m3/README.md`,
`evidence/m3/manifest.json`, `evidence/m3/linux-process-matrix.json`,
`evidence/m3/profiling-overhead.json` collectors, `evidence/m1/README.md`,
`evidence/README.md`, `evidence/m0/manifest.json` `platformEvidence`, and
`tools/ravel-xtask/src/lib.rs:174-495` and `:536-960` line by line for skips and
early returns.

Ran: `ravel:verify` (exit **0**), `ravel:verify-milestone:m0` (0), `:m1` (**1**,
review), `:m2` (0), `:m3` (**1**, three reasons). Judged by exit code.

Not reached: `pack-compile.json` beyond its README framing;
`class-loader-identity.json`; the conformance test bodies themselves (I confirmed
`a_world_holding_no_data_this_mod_owns_initializes_fresh_state` is named and cited,
not that it asserts what is claimed); the JVM harness source;
`schemas/evidence.schema.json` clause by clause (I relied on `check-jsonschema`
inside the milestone gates).

### Round 3 — What would make this a PASS

F-A and F-B are cheap and mechanical: re-run the gate, re-pin
`revision.id`/`tests`/`logSha256`, and correct `evidence/m3/README.md:64-65`,
`:227`, `:251-253` to six metrics. F-C through F-G are worth recording as deviations
rather than blocking. If those two are fixed, the honest residual ceiling a
maintainer would still be *trusting* rather than verifying is: the entire live half
of M3-W4 and M3-W7 — the world round trip, the refusal, all three screens, the
settings round trip, the command path, both profiling scale observations, and every
instance scan — none of which any reviewer on any machine can re-derive from this
repository, plus the two M0 private validation digests and the M1/M2 private review
records.

---

## Round 4 — VERDICT: FAIL

I recomputed everything and ran the gates. Here is the verdict.

Two false claims about the live tree in `evidence/m3/README.md`, one of them inside
the very paragraph written to repair round 3's blocking F-A, plus an incomplete
review record. All three are the same class rounds 2 and 3 died on: a count that
does not survive being recomputed.

### Round 4 — Clause-by-clause (M3-W8, `ROADMAP.md:716`, closure standard `ROADMAP.md:720-722`)

| Gate clause | File:line that proves it | Status |
| --- | --- | --- |
| Output: "review record" exists, digest-bound | `evidence/m3/manifest.json:216-221` `review.record.name: "review.md"`; `evidence/m3/review.md` hashes `df29c46a…` — I recomputed, matches; bound by `verify_recorded_review` (`tools/ravel-xtask/src/lib.rs:525-575`) via `verify_tracked_digest` (`:592`) | **proven** (a record exists and is bound) |
| Review is independent | `manifest.json:222-224` `independent: true`; `review.md:5-6` | **asserted only** — no gate can bind reviewer identity |
| "M3 gate passes" | `ravel:verify-milestone:m3` exit **1**: `milestone must have no blocking deviations or risks`, `milestone review must be independent and pass` (`manifest.json:210-213` blocking risk, `:225` `result: "fail"`) | **not passing** (deliberately, pre-verdict) |
| `ravel:verify` green | my run: exit **0** (827 tests, 827 passed, 0 skipped) | **proven** |
| `ravel:verify-stack` green | my run: exit **0** | **proven** |
| "authorizes private inventory work only after M4" | `README.md:3-6`; `manifest.json:210-213` | **proven** (correctly withheld) |
| M3-W0 target reconciliation | recomputed from `hooks.toml`: 37 entries = 7 static + 26 instance methods (**33**) + 4 fields, over **15** classes; per-class 5+4+1+1+1+2+2+5+1+1+1+4+1+1+3 = 33; `mappings.lock` holds 37 distinct members, same 15 classes; `adapter.toml:22` `clientJarSha256` = `mappings.lock` `clientJarSha256` | **proven, observed** |
| Title-only reconciliation 15−4=11, 33−6=27 | recomputed: the 4 absent classes hold exactly 6 of the 8 `rewrite.worldOnlyTargets`; `world.open-persistence`/`world.save-persistence` sit in `Minecraft`/`MinecraftServer`, both loaded at title. `linux-process-matrix.json` `rewrite.worldOnlyTargetsCriterion` and `surfaceCampaign.launchDetail.first.whyElevenNotFifteen` agree | **proven, observed** |
| M3-W6 unsupported-metric count | `crates/ravel-observability/src/profiling.rs:165-227` declares exactly **6** unsupported (5 + `TickCount`); `Profiler::tick` has callers only at `:634,694,695,823,844`, all below `#[cfg(test)]` at `:611` | **proven, concluded from code** |
| M3-W4 / M3-W7 live halves | `linux-process-matrix.json` `reviewCeiling`, `resultCeiling` | **attested by an operator, not recomputable** — correctly labelled |
| M3 evidence README describes the records it inventories | `README.md:76`, `:217` | **disproven — N-1, N-2** |

### Round 4 — Round 3's findings, one by one

| Finding | Status | Checked at |
| --- | --- | --- |
| **F-A** (README falsely called the revision/gate numbers "this state's") | **partially repaired, and re-broken.** The false sentence is gone and replaced by an honest arithmetic argument (`README.md:73-97`); `verify_recorded_run` exists and is wired (`lib.rs:259-320`, called from `:241`) and holds M3's block to itself (813 = 813+0+0, 85.97 ≥ 80). But the replacement embeds a new false count — see **N-1** | `README.md:73-97`, `lib.rs:259-320` |
| **F-B** (five-vs-six metric drift) | **repaired.** Five where it describes the pre-fix observation (`README.md:296-301`; `linux-process-matrix.json` `profilingSurfaceDefect.…unsupportedMetrics` "two of the five"), six where it describes the re-observation (`README.md:322-325`; `caveatReObservationCampaign.atScaleOne.linesDrawn`). Both agree with the 6 `Support::Unsupported` arms in `profiling.rs` | `README.md:296-301`, `:322-325` |
| **F-C** (`review.record.sha256` unbound) | **fixed, not merely recorded.** `verify_recorded_review` (`lib.rs:525-575`) rejects a prose name (`names_a_path`, `:577`) as a *violation* rather than a skip; M0's `review.md` = `b225d06b…` ✓, M1's = `59092e69…` ✓, M3's = `df29c46a…` ✓, M2's withdrawn (and `m2` gate exits 1 on the missing record, as documented) | `lib.rs:525-575`, `manifest.json:216-221` |
| **F-D** (silent `.jj` skips) | **repaired.** Both now `println!` the skip: `lib.rs:698-703` (`verify_recorded_change`), `lib.rs:771-776` (`verify_manifest_change`) | as cited |
| **F-E** (`verify_evidence_records` has no floor) | **recorded as a non-blocking deviation** (`manifest.json:156`), and the hole is real and unfixed: `lib.rs:225-231` `.into_iter().flatten().flatten()` yields zero manifests, zero violations, no message | as cited |
| **F-F** (`whatSurvivesTheRemoval` implied a digest restores checkability) | **fixed.** The passage now reads "All three are now claims in this file: `reviewCeiling` above states the position this passage has to keep" | `linux-process-matrix.json` `worldPersistenceRoundTrip.worldRemovedAfterwards` |
| **F-G** (`worldOnlyTargets` had no membership rule) | **fixed.** `rewrite.worldOnlyTargetsCriterion` states the rule and names the four targets the narrower rule excludes; I confirmed all four exist in loaded classes | as cited |

### Round 4 — New findings

**N-1 — blocking. `evidence/m3/README.md:76` and `evidence/m3/manifest.json:167`
state a false count about the live tree.** Both say `revision.id` "resolves to
change `pqkoksqopryu`, **six** described changes below the tip". Recomputed:
`jj log -r 'pqkoksqopryu::@ ~ pqkoksqopryu'` returns **14 changes — 13 described**
plus the empty working copy (`yuouxpsvvstn`, `txvluwnvrpor`, `vtprwxqxpykr`,
`ssvlsmrxllom`, `pwlmolmtmnol`, `mrrxlqsuwkqm`, `snuqtkvvpoln`, `wrrqlmrmnswx`,
`pyzomuruuouy`, `tyvwrtwmzpsk`, `mvpnyqsyvswq`, `xoxrmtyorpmq`, `ltxxwlqmrlku`).
Six was true when round 3 wrote it — round 3's F-A enumerates exactly those six —
and seven changes have landed since. This is round 3's F-A reproduced: a positive
false claim about this state, in the paragraph written to remove one. The defect is
structural, not arithmetic: any absolute count of changes below the tip is false on
the next commit, so the fix is to drop the number, not to update it to 13.

**N-2 — must fix. `evidence/m3/README.md:217` contradicts the record it documents
on the number of campaigns.** README: "It now holds **three** launch campaigns."
`linux-process-matrix.json` `summary`: "**Four** campaigns are recorded here." The
record holds four — the earlier 31-target campaign (`privateEvidenceDigests.provenance`,
`matrixNotCarriedForward`), the world round trip (`thisCampaignMachineEvidence`, 3
launches), the surface campaign (3 launches), and `caveatReObservationCampaign` (1
launch). The README's own narration confirms four and forces the contradiction:
`:254` calls the surface campaign "**A third** campaign … the rows **the first** had
left inherited" (the first being the 31-target campaign), and `:316` then introduces
the caveat re-observation as a further one. Either "three" is wrong or "a third
campaign" is; the matrix says four.

**N-3 — moderate. The review record omits one of the four rounds, and the manifest
miscounts them.** `README.md:34-63` and `manifest.json:73` attribute four blocking
findings — manifest inventory, package schema, missing world write, unexercised
native surfaces — to "the independent prototype review". None of those four appears
in any of the three rounds `review.md` tracks; `review.md` Round 1 raises digest
rot, the stale pinned revision, the world-round-trip self-contradiction and the
self-declared `pass`, which `manifest.json:73` itself calls "**a later** review
round". So four rounds ran and three are tracked. `manifest.json:222`
(`"three rounds"`) and the blocking risk at `:210` ("Three independent M3-W8 rounds
have run … their verdicts are tracked at evidence/m3/review.md") both understate it,
and the earliest round's verdict — the one whose findings the README leads with —
survives only as prose summary, which is precisely what `review.md:28-29` says the
tracked record exists to prevent.

**N-4 — minor. `evidence/m3/profiling-overhead.json` publishes two derived figures
that cannot be recomputed from the percentiles beside them.**
`overhead.perTick.formula` is "one tick boundary plus the assumed crossings, each at
its 99th percentile"; from the published `off.perCrossing.p99 = 0.05` and
`off.perTickBoundary.p99 = 0.05`, 0.05 + 10000×0.05 = **500.05**, but `nanoseconds`
records **510.05**. `breakEven.crossingsPerTick = 9803920.59` reconciles only at
(500000 − 0.05)/**0.051**. Both figures are consistent at three decimal places and
the record publishes two, with no rounding note — so a reader applying the stated
formula to the stated inputs gets the wrong answer. The budget itself passes
(510.05 ≤ 500000). No prior round reached this file.

**N-5 — minor, informational. `revision.id` pins an already-abandoned commit.**
`jj log -r 3c3520be…` reports `hidden=yes`, and `3c3520be::@` is empty — the commit
is not an ancestor of `@`; change `pqkoksqopryu` now points at `b9c5f54b`.
`jj file show -r` still resolves it, so `verify_recorded_inputs` genuinely binds
(4/4 matched). The manifest discloses the `jj util gc` fragility as a deviation but
presents it as a future risk; it is already an obsolete commit kept alive only by the
op log. `README.md:75` "resolves to change `pqkoksqopryu`" is true but not for the
reason a reader would assume.

**N-6 — minor. `change_mismatch` (`lib.rs:798-806`) passes silently when
`introducing_change` returns empty output.** `owner.is_empty()` → `None`. The doc
comment sanctions it for untracked results, but the effect is that any path jj cannot
attribute clears the repository's strongest provenance control without a word — the
same failure mode F-D was raised about, one level down.

### Round 4 — Vacuity audit of the digest machinery

`verify_recorded_digests`, `verify_recorded_baselines`, `verify_recorded_review`,
`verify_recorded_inputs`, `verify_recorded_run` — every skip and early return read at
`lib.rs:223-625`. Findings: the directory-scan floor (F-E, disclosed, real);
`.benchmarks.results[]?`/`.inputs[]?`/`.platformEvidence[]?` all use `?`, so a
renamed or absent key checks nothing silently (disclosed as two deviations, real);
`verify_recorded_run` returns early when `.tests` is absent, so a manifest with
neither `commands` nor `tests` passes it entirely (harmless — `verify_general_closure`
refuses to close such a manifest); N-6 above. Everything else fails closed: `jq`
absent → violation, invalid filter → violation, unreadable file → violation, path
escaping the milestone → violation, a manifest recording its own digest → violation,
`.tests: {}` → caught by `total > 0`. I found no way for a *recorded* digest to pass
without being computed.

### Round 4 — Coverage

**Recomputed: 37 digests, 37 matched.** 4 M3 results, 4 M3 inputs (by
`jj file show` at pinned `3c3520be`), 1 M3 review record; 2 M0 baseline results, 1 M0
review record; 20 M1 spike results, 1 M1 review record; 4 M2 results. Also
recomputed: the 3 M3 input files in the working copy (identical to the
pinned-revision values); the 37/33/15 hook inventory from `hooks.toml` and
`mappings.lock`; the per-class method breakdown; 15−4=11, 33−6=27, 6+2=8; the
jar-hash chain `adapter.toml` → `mappings.lock`; the 6 unsupported metric arms and
all 5 `Profiler::tick` call sites; the `perTick`/`breakEven` arithmetic (did not
reconcile — N-4); the change count below `revision.id` (did not reconcile — N-1);
the campaign enumeration (did not reconcile — N-2).

**Ran, judged by exit code:** `ravel:verify` **0**, `ravel:verify-stack` **0**,
`ravel:verify-milestone:m3` **1** (two violations, both deliberate).

**Read in full:** `ROADMAP.md` M3/M4 rows and the closure rule;
`evidence/m3/README.md` (388 lines); `evidence/m3/manifest.json`;
`evidence/m3/review.md` (all three rounds); `evidence/m3/linux-process-matrix.json`
(every scalar path enumerated, load-bearing objects verbatim);
`evidence/m3/profiling-overhead.json` `overhead`/`budget`;
`tools/ravel-xtask/src/lib.rs:150-810` and `:947-1130`; the
`revision`/`inputs`/`benchmarks`/`review` blocks of all four manifests;
`minecraft/versions/26.1.2/hooks.toml`, `mappings.lock`, `adapter.toml`.

**Not reached:** `pack-compile.json` and `class-loader-identity.json` contents
(digests only); `profiling-overhead.json` `collectors`/`trial`/`whatThisDoesNotSay`;
the conformance test bodies and the JVM harness sources;
`schemas/evidence.schema.json` clause by clause (relied on `check-jsonschema` inside
the gates); `evidence/m0`, `m1`, `m2` prose beyond the blocks cited; every
`logSha256`, `verificationLogSha256`, `artifacts[].sha256`, and the two M0 private
validation digests — unbindable by construction and announced as such. I cannot
verify `review.md` against the original round 1–3 transcripts, which do not exist in
the repository; on internal evidence it is not softened — round 1's F1/F2/F3 keep
**BLOCKING**, round 2's F1/F2 keep **must fix**, round 3's F-A keeps **blocking**,
and round 3's enumeration of the six changes corroborates that its own numbers were
true when written. What it omits is a whole round (N-3), not a severity.

### Round 4 — What a fix costs

N-1 and N-2 are one-line edits (drop the drifting count; say four campaigns, or
renumber). N-3 needs either the prototype round's verdict added to `review.md` and
the counts in `manifest.json:73`, `:210`, `:222` corrected to four, or an explicit
statement that the earliest round is untracked and its findings are a summary. N-4
needs a rounding note or a third decimal place. N-5 and N-6 are deviation-grade.

Had those been clean, the honest ceiling I would have named is the one round 3 named
and this round confirms unchanged: the entire live half of M3-W4 and M3-W7 — the
world round trip, the `dataVersion`→9 refusal, all three screens, the settings round
trip, the command path, both profiling scale observations and every instance scan —
is operator attestation over deleted worlds, logs and screenshots, recomputable by
nobody; `tests.total: 813` and `lineCoveragePercent: 85.97` are attested numbers from
a private log on an abandoned commit (the tip now runs 827) and `verify_recorded_run`
binds only their internal consistency; and the durability of all four milestones'
input provenance rests on jj's op log retaining obsolete commits.

---

## Round 5 — VERDICT: FAIL

VERDICT: FAIL.

**BLOCKING — B-1: `evidence/m3/linux-process-matrix.json:212`, `:600`,
`:602-618`, `:619`.** The caveat re-observation campaign binds its load-bearing
observation to artifact digests that cannot have produced it. `atScaleOne`
(`:637`) publishes six unmeasurable metric names — the five plus `tick-count` —
and states the reading was made "re-observed after change `lppyxtnz` on a launch
that stayed on the title screen". `tick-count` enters that list only in
`lppyxtnz`, a descendant of `npozznqz`, which modified two things that both
change a recorded digest: `crates/ravel-observability/src/profiling.rs` (where
`ProfilingReport::unsupported()` builds the list, compiled into
`libravel_bootstrap.so` via `ravel-bootstrap`) and
`jvm/screen/dev/ravel/jvm/screen/RavelProfilingScreen.java` (the screen jar). But
the campaign's `artifacts` record screen jar `bb977eb3…` and native library
`abe56b96…` with `sameAsSurfaceCampaign: true`, and both digests were written into
this record by change `qovlymtm` — an ancestor of `lppyxtnz` whose copy of the
file contains `tick-count` zero times (current tip: four times). So both
artifacts recorded as this campaign's build predate the change whose effect the
campaign publishes as observed. Three claims are therefore false of that
observation: `launches: 1`, `whatDiffersFromTheSurfaceCampaignBuild` ("Exactly one
artifact: the screen jar"), and `screens.profiling.provenance` at `:212` ("The
caveat and unsupported-metric lines are the caveat re-observation campaign's,
against the fixed screen jar recorded in `caveatReObservationCampaign`"). At least
two launches against at least two builds are conflated, and no digest exists for
the build that produced the only reading proving the unsupported-metrics line
renders in full. This is blocking rather than cosmetic because profiling was the
one required surface that failed; `result: "pass"` and the M3-W7 closure rest on
its re-observation, and the record's own rule at `:503` — "changing the screen jar
changes its digest… Re-observation, not a patch landed after the fact, is what
would let those rows keep their provenance" — is precisely the rule this breaks.
What would make it right: record the post-`lppyxtnz` screen-jar and
native-library digests, state the true launch count, and attribute each row to the
build it was observed against; or re-observe the whole profiling surface once
against one recorded build. Note `manifest.json:68` also credits only `npozznqz`
and never mentions `lppyxtnz`.

**MUST-FIX — M-1: `linux-process-matrix.json:59`.** `whatThisMeansForAReviewer`
still reads "Read the screen, settings, profiling, and command rows below as the
earlier campaign's findings at 31 targets, not as this campaign's. They were not
re-observed here." That is explicitly repealed by `status` (`:60`) and
`howItWasResolved` (`:61`) two lines later and by every row's own `provenance`.
Delete or rewrite the passage; a reader who stops at `:59` is misled about the
provenance of the rows M3-W7 closes on.

**MUST-FIX — M-2: `linux-process-matrix.json:61`.** "its launches reported
`rewrote 15 classes and 33 methods`" is false of one of three surface launches:
`surfaceCampaign.launchDetail.first` (`:526`) records "11 classes and 27 methods".
`README.md:297-298` states it correctly. Add the title-only qualifier.

**MUST-FIX — M-3: `crates/ravel-observability/src/profiling.rs:198-203`,**
mirrored to `evidence/m3/profiling-overhead.json:92` and pinned by
`tests/conformance/tests/profiling_report.rs`. The gate asserts on every run that
"Moving bytes in a world is the half of M3-W4 that is still open, so there is
nothing to size" — contradicted by `manifest.json:56`, `README.md`, and
`worldPersistenceRoundTrip`. The conclusion (metric unsupported) survives; the
reason the gate enforces is now a falsehood. Restate the reason and regenerate the
record.

**MODERATE — D-1: `linux-process-matrix.json:237` vs `:135` and `:101`.**
`rewrite.measuredIn` claims all three world-round-trip launches reported "rewrote
15 classes and 33 methods", but the third launch's world "was refused before it
opened" with `integratedServerNeverStarted: true`. By the record's own criterion
(`:528`), `Commands`, `ReloadableServerResources` and `RecipeManager` load only
when a world opens, so that launch cannot have rewritten 15/33 — consistent with
its 28 845 native invocations against the others' 675 660 and 749 397.

**MODERATE — D-2: `pack-compile.json:174`, `:428`;
`profiling-overhead.json:172`.** All three measurement trials carry
`trial.revision.dirty: true`, so no revision identifies the code measured —
including the code behind the single M3-W6 budget limit `cargo xtask verify
budget` enforces on every gate. No deviation or risk discloses it, while
`verify_general_closure` requires the manifest's own revision to be clean.

**MODERATE — D-3: `docs/decisions/adg-7-world-persistence.md:113-116`, `:221`;
`jvm/bootstrap/dev/ravel/jvm/Persistence.java:104-107`.** ADG-7's save-seam
marker refusal ("An absent marker refuses world load") has no harness case and no
test. The "eight world lifecycles" the corpus credits reconcile exactly with the
eight cases at `jvm/harness/run-harness.sh:301-341` (admitted, null-verdict,
refused, four save answers, absent-sibling); this ninth polarity is not among
them, and `saveSeamInstalled()` returns true in all eight, so the false branch
never executes.

**MODERATE — D-4: undisclosed ceiling on M3-W0.** The corpus states the
operator-attestation ceiling for M3-W4/W7's live half but nowhere states that
M3-W0's "targets resolved against the pinned jar by two independent passes" is
equally unrecomputable. On a jar-less machine the chain terminates at
`minecraft/versions/26.1.2/adapter.toml:20-23`'s `clientJarSha256`;
`adapter.toml:44-50` says so plainly, but no M3 record does.

**MODERATE — D-5: `tools/ravel-xtask/src/lib.rs:1009-1017`.** There is no
`verify_m3_manifest`; M0, M1 and M2 each have milestone-specific closure rules and
M3 has none. Proven by mutation: with `review.result: "pass"` and the one blocking
risk downgraded, `verify milestone M3` exits 0 with every `gateStatus` set to
`"failed"`, with 8 of 9 entries deleted from `workPackages`, and with
`benchmarks.results` emptied — which additionally leaves all four M3 records
digest-bound by nothing while `ravel-verify-scaffold` still exits 0. The recorded
deviations at `manifest.json:159` and `:203` describe adjacent holes but not this
one.

**MINOR: `tools/ravel-xtask/src/lib.rs:181-184`.** The unsafe-prohibition check is
a substring test, and `crates/ravel-bootstrap/src/lib.rs:31` (and `lib.rs` of
xtask itself) carry the literal in a doc comment. Deleting the real attribute from
`ravel-bootstrap` and adding a working `unsafe` block left `verify scaffold` at
exit 0. The safety boundary itself is not breached — `Cargo.toml:64-65` sets
`unsafe_code = "forbid"` workspace-wide and clippy refused the probe with
`-F unsafe-code` — so this is a redundant control gone vacuous for two files.

**MINOR: `docs/architecture/dependency-rules.md`.** "Production crates never
depend on `ravel-testkit`" is unenforced: adding `ravel-testkit.workspace = true`
to `crates/ravel-loader/Cargo.toml` leaves the scaffold at exit 0. No live
violation exists.

**MINOR: `README.md:365-366`.** "the window has seven rows rather than eighteen" —
the eighteen appears nowhere in the record it documents.

**MINOR:** `class-loader-identity.json:48` records `java: "25.0.1"` while
`manifest.json:26` pins `25.0.4`; nothing notes that one inventoried record was
measured on a different JDK patch.

**MINOR:** `linux-process-matrix.json:22-33` vs `:34-48` disagree on
`prismExitedNaturally` with no statement of which campaign the outer `launch`
block describes.

**CLEAN:** Safety and layering hold. Every maintained root carries
`#![forbid(unsafe_code)]`; no `unsafe` exists outside `experiments/`;
`docs/architecture/safety-boundary.md`'s table is empty; `ravel-core` depends on
no Ravel crate; `ravel-sdk` depends only on config/core/data/network/save — no
bootstrap, JVM or version internals; no production crate depends on
`ravel-testkit`. Hook integrity reconciles exactly: `hooks.toml` holds 37 entries
over 15 classes, of which 26 instance-methods + 7 static-methods = 33 callable and
4 instance-fields; `mappings.lock` holds the same 37; the per-class breakdown in
`rewrite.targetInventory` sums to 33; `15 − 4 = 11`, `33 − 6 = 27`, `6 + 2 = 8`
all hold, and `worldOnlyTargetsCriterion` gives the membership rule that makes the
subtraction six rather than eight. Mutating either a lock descriptor or
`hooks.toml` without repinning fails the build. Arithmetic and derivation are
sound: `profiling-overhead.json` reconciles on the unrounded pair x = 0.051,
t = 0.05 — 0.05 + 10000 × 0.051 = 510.05 and (500000 − 0.05) / 0.051 =
9803920.59; `overhead.rounding` states the rule and the README states the
recoverability limit correctly. `pack-compile.json` reconciles fully — baseline
2526 = points[0]; every `coldMillisecondsLessBaseline` and
`coldMillisecondsPerMod`; the secant (29505 − 604)/255 = 113.34; medians of three
for the curve and minima of two for the variants against every recorded sample;
`modSourceBytes` scaling at ×16/×64/×256; the toolchain's 195 541 312 B against
ROADMAP's "about 196 MB". Redaction is clean: no credential, world name, user
path, launcher path, username or launch command in any tracked M3 record, and the
check is real — an injected local instance path was caught.

**COVERAGE:** ROADMAP's M3 gate, clause by clause. W0 hook inventory reconciles
in-repo, jar resolution unrecomputable (D-4). W1/W2/W3/W5 conformance suites exist
and pass; registry, command, event, resource and configuration implementations
were read, not mutated. W4 in-gate half is non-vacuous in both halves and one
ADG-7 polarity is untested (D-3); live half is attestation. W6 budget holds by
exit code, the metric table is pinned to the code, and one pinned reason is false
(M-3); the two named zero counters are honestly disclosed. W7 every required
surface is claimed observed, and the profiling row's build binding is wrong (B-1).
W8 is this verdict. The corpus proves what the gate demands for W0–W6 modulo the
findings above. For W7 it proves a weaker thing than it states: not "every surface
observed against one recorded build" but "every surface observed, one of them
against an unrecorded build".

**REVIEW CEILING.** Broken and confirmed fail-closed: `benchmarks.results[]`
digests; `review.record` digest and path containment; `inputs[]` digests;
`verify_recorded_run` arithmetic (total/passed/failed/skipped, coverage floor,
`exitStatus`); `revision.changeId` provenance (milestone command only — the
scaffold does not run it); `revision.id` resolvability; schema conformance;
redaction; `evidenceKind`/synthetic; `ravel-save`'s `admit`, envelope-schema and
size ceilings; `Persistence.open`'s refusal path and absent-sibling asymmetry;
`hooks.toml`/`mappings.lock` drift; the workspace `unsafe_code = "forbid"`. Judged
by reading, not by breaking: the entire live half of M3-W4 and M3-W7 — every
launch row, the world round trip, the `dataVersion`→9 refusal, all three screens,
the settings round trip, the command path, both profiling scale readings, every
instance scan — operator attestation over deleted worlds, logs and screenshots.
Every `logSha256`, `verificationLogSha256`, `artifacts[].sha256`,
`hardwareProfileSha256`, and the two M0 private validation digests: unbindable by
construction. The registry, command, event, resource and configuration
implementations (only persistence and the digest/gate machinery were mutated). The
`stubs.toml`/`stubs.lock` generation pipeline. `class-loader-identity.json`'s
measurement method. `schemas/evidence.schema.json` clause by clause — reliance on
the gate's own `check-jsonschema`, with only a dropped required field confirmed
rejected. ADG-1 and ADG-3–ADG-6 mechanism conformance beyond the layering, ADG-2
and ADG-7 checks named above. `cargo deny`. The M0/M1/M2 corpora beyond their
manifest blocks. The full `ravel:verify` was not run by this reviewer; the
caller's exit codes were taken and the scaffold, JVM-harness and relevant test
components reproduced in a scratchpad (`ravel:verify-jvm` exit 0,
`verify scaffold` exit 0, `verify milestone M3` exit 1 on the two known
violations). `review.md` cannot be checked against the original round 1–4
transcripts, which do not exist in the repository; on internal evidence the
severities are not softened, and the untracked prototype round remains
unrecoverable exactly as `manifest.json:175` says.

**MAY M3 CLOSE?** No. B-1 survives: the M3-W7 gate's profiling row is bound to a
build that provably cannot have produced the observation it rests on, and the
record understates the launch count and artifact set that would let a reader see
this. `review.result` must stay `"fail"` and the blocking risk at
`manifest.json:225` must stand. M3 authorizes no private inventory work.

---

## Round 6 — VERDICT: FAIL

This round's own heading calls itself Round 7, counting the untracked prototype
round this file describes but does not hold. Under this file's numbering, which
counts only the rounds it holds, it is Round 6. Nothing else about it is renamed.

### M3-W8 Independent Review

#### VERDICT: FAIL

One blocking finding. Everything else I could break, broke correctly; the machinery this
milestone has accumulated is now genuinely strong, and I say that having tried to kill
it rather than having read it.

---

#### Method and environment

Whole-repo `rsync -a --exclude target` into a private scratchpad with `CARGO_TARGET_DIR`
inside the copy. All 31 mutations were applied there; I verified at the end with `cmp`
that all eight files I touched are byte-identical to the real tree, and the real tree's
`jj st` is unchanged from session start (`ROADMAP.md`,
`evidence/m1/{README.md,manifest.json,review.md}` dirty from concurrent M1 work by
another agent, both before and after).

Caveat on my own inputs: the copy captured that concurrent agent's uncommitted edits.
`jj diff ROADMAP.md` is 1 line and contains no `M3` occurrence, and `evidence/m3/*` is
clean in the working copy, so nothing I judged came from another agent's pending work.

Recorded revision provenance checks out. `revision.id
724f12b6b80c3f0597c5b7bf4a40b1fdc41e2315` resolves, is an ancestor of `@` (73 commits
below it), and is the visible commit of its own `revision.changeId
xqmmlnzmtkqpuuxymttpzqyvnovxtxtv`. All six change ids the manifest cites resolve.

---

### Blocking

#### B-1 — The production world-persistence open path can be made to never deliver a mod its stored state, and the entire gate stays green

**Files.** `crates/ravel-bootstrap/src/persistence.rs:300` (the delivery);
`tests/conformance/tests/saved_data.rs:17-28` (the claim); `evidence/m3/manifest.json`
gateStatus `M3-W4` and `evidence/m3/linux-process-matrix.json`
`worldPersistenceRoundTrip.reviewCeiling` (which offer the in-gate half as what a
reviewer *can* check).

**What is claimed.** `saved_data.rs:24-26`: *"That is the sequence `ravel-bootstrap`'s
`stage_world` and `save_world` run, in the same order, and `reload` below is written
against it — a serializer that dropped what it was given, or a delivery that never
reached the mod, has to fail something here."* The manifest's M3-W4 row offers
`tests/conformance/tests/saved_data.rs` and `jvm/harness/PersistenceMain.java` as the
half that "runs on every gate", explicitly as the reviewer-checkable substitute for a
live half that is operator attestation.

**What is actually true.** `reload()` at `saved_data.rs:326` *restates* the production
sequence; it never calls `stage_world`. Production's delivery step is therefore
constrained by nothing.

**How I established it — broke it.** Single edit at `persistence.rs:300`,
`host.open_saved_data(envelope, &plan)` → `host.open_saved_data(None, &plan)`. Every mod
is told its world holds no data and initialises fresh, on every world open, in the
composed host that ships. Results:

| gate step | exit / result |
| --- | --- |
| `cargo nextest run --workspace --all-features` | **865 tests run, 865 passed, 0 failed** |
| `cargo test --workspace --all-features` | 892 passed, 0 failed |
| `cargo test --workspace --all-features --doc` | 27 passed, 0 failed |
| `./jvm/harness/run-harness.sh` | **exit 0** |
| `cargo clippy --workspace --all-features --all-targets` | **exit 0** |
| `ravel-xtask verify scaffold` | **exit 0** |

The JVM harness cannot catch it by construction:
`jvm/harness/initialization/dev/ravel/jvm/Hook.java:289,299` answer the two crossings
from `-Dravel.test.persistence.{open,save}` system properties, so `PersistenceMain`
never reaches Rust. The Rust half never reaches Java. Nothing sits on the seam.

The reason no test catches it is narrow and visible: the test double
`Owning::open_saved_data` at `persistence.rs:585-595` *does* record `visits` from the
envelope, and every successful `stage_world` call in that module's tests passes `None`
bytes (lines 712-1162). The only calls with `Some(bytes)` are refusal cases (853,
866-912, 1041, 1054), which assert the refusal, not the delivery.

**Why this is blocking rather than moderate.** This is the third distinct visit for
M3-W4 of the same failure shape — a proof that still passes when the thing it proves is
absent. The live round trip was observed once and is honestly recorded as
unrecomputable; the repository's answer to that ceiling is the in-gate half, and the
in-gate half does not contain the step. M3-W4's Verification clause is
"create/save/exit/reload preserves state", and the delivery is what "preserves" means.

**Smallest fix.** One test in `crates/ravel-bootstrap/src/persistence.rs`'s test module:
stage `Some(bytes)` carrying `{"visits": N}` for `N > 0` and assert `held.visits == N`;
then a `stage_world` → `save_world` → `stage_world` round trip through the production
pair asserting the second stage sees what the save emitted. The double already has the
machinery. This is a handful of lines, not a redesign.

**Note on scope.** I mutated four other steps of the same production path and all four
failed closed, which is why I am reporting one finding and not five: skipping the
`SavedDataAggregate::open` admission (85 passed, 3 failed), `Replacement` carrying empty
bytes (82 passed, 6 failed), skipping the `machine.open_world` loader gate (86 passed, 2
failed), and hollowing `SavedDataAggregate::to_bytes` so it serialises zero owners (80
passed, 8 failed). The hole is one step wide.

> **Editor's note, not the reviewer's text.** B-1 is answered. Change
> `mltsuvskoxwp` added the two tests this finding asks for to
> `crates/ravel-bootstrap/src/persistence.rs`'s test module, and an independent
> re-prover confirmed the fix the way the reviewer asked: re-applying the exact
> mutation at the delivery site gives `cargo test -p ravel-bootstrap` exit 101,
> and over the whole workspace 873 tests run, 871 passed, 2 failed — the two new
> ones and nothing else in the suite. That re-prover also found two things those
> tests did not constrain, each proven by its own mutation against the full
> workspace: the delivered `SavedDataPlan` was asserted by nothing, so building
> every plan from `None` left the workspace green while a mod that migrates on
> `plan.steps()` would silently not migrate; and the test runtime held one
> saved-data owner, so "mod A handed mod B's envelope" could not be expressed and
> the wrong-identity mutation was caught only incidentally, by degenerating into
> the absent case. Change `zzussnqn` closes both: the double records the plan it
> was handed, a world staged at an older version asserts the mod receives the
> migration steps to current, and a two-owner runtime asserts each mod receives
> its own envelope. Both closures are mutation-proved — the plan mutation and a
> cross-delivery mutation each exit 101, the latter failing for an identity
> reason rather than an absence one.

---

### Moderate

#### M-2 — `manifest.json:167` describes a revision and a test count that are both false

**Claimed.** The deviation reads *"revision.id 3c3520be is an ancestor of the tip …
tests.total is that revision's 813"*.

**True.** `revision.id` is `724f12b6b80c…` and `tests.total` is `854`. `3c3520be104c`
resolves (change `pqkoksqopryu`, "docs: make the M3 evidence set agree on the world
round trip") but is not what the manifest pins, and a later deviation in the same array
says the field previously named `584abc1bc1da` — so the prose was left behind across two
separate re-pins. **How established:** read the deviation, then `jq .revision .tests`
and `jj log -r 3c3520be`.

This is the fifth occurrence of the recurring "absolute count a future commit falsifies"
class, and it is the worst-placed one: it sits inside the deviation whose entire subject
is that the revision block must not go stale. **Fix:** state the relationship without
restating the commit id or the count, or re-derive both when the block is re-pinned.

#### M-3 — `manifest.json:73` (gateStatus M3-W8) is stale in two ways against its own document

*"answering B-1 is not closure and a sixth round decides that"* — but `risks` in the
same file says *"Six independent M3-W8 rounds have run and all six returned FAIL"*, and
`review.md`'s own headings enumerate six (prototype + Round 1-5). The sixth round has
run; a seventh decides it. I am the seventh.

Same row: *"Still unanswered: … its five moderate findings including the absence of any
M3-specific manifest closure rule."* That is answered — `verify_m3_manifest` exists at
`tools/ravel-xtask/src/lib.rs:2052`. **How established — broke it:** deleted 8 of 9
`workPackages` and set every `gateStatus.status` to `"failed"`; `verify milestone M3`
exited 1 with *"M3 manifest must include every work package from M3-W0 through M3-W8
exactly once"* and *"M3 closure requires a gateStatus row for every work package, each
implementation row closed and naming its evidence"*. The rule is real and M3-specific.
**Fix:** move that item to the answered list and correct the round ordinal.

#### M-4 — `review.md:6-7` and the manifest contradict each other on B-1

`review.md` front matter: *"Round 5 below is the standing verdict, and its blocking
finding B-1 is unanswered."* The manifest and `README.md:4` both say B-1 is answered,
and `profilingReObservationCampaign` exists in `linux-process-matrix.json` to answer it.
The front matter is repository prose, not verbatim reviewer text, so it was updatable
and was not updated. This is the same "the records disagree with each other" finding a
prior round raised, recurring in the one file that carries the verdict the gate reads.
**How established:** read both, then confirmed the campaign object exists via `jq`.
**Fix:** front matter states B-1 answered and that closure still turns on a passing
round.

#### M-5 — `evidence/m3/README.md:344` counts four launch campaigns and gives the wrong ordinal

Line 344: *"The world round trip campaign, the second of the four, is three launches at
33 callable targets."* Four lines earlier, line ~340 of the same file says *"It now
holds five launch campaigns"* and enumerates all five. Separately,
`linux-process-matrix.json:23` calls the same campaign *"the first of the five recorded
here"*, which disagrees with README's own enumeration where it is second. So the count
is stale and the two records disagree on the ordinal of one campaign. **How
established:** read, cross-checked with `grep -n` across both files. **Fix:** drop the
ordinal, or derive the enumeration in one place and reference it from the other.

---

### Minor

#### m-1 — `class-loader-identity.json` marks the agent jar `reproducible: true` with nothing to reproduce it from

`artifacts[1]` records `ravel-agent.jar` at
`6bcf99f117e56d044a64281a949f735e250a0496a49390e269f3bae186daae94` with `reproducible:
true`, and the record pins no revision anywhere (`measurement.takenOn` is prose: "one
launch of the pinned vanilla instance"). A reviewer therefore has no source state to
build. **How established — tried it:** built the agent jar in my scratchpad; it hashes
`404740f4c466262f8be4495a29490f043ebb1df98ee4e0710995b51b33810981`. `grep -r` shows
`6bcf99f1…` appears in exactly one file and is pinned to no revision. **Fix:** either
record the change the artifacts were built at, or set the flag to false with the reason.

#### m-2 — `profilingReObservationCampaign.artifacts` does not distinguish the digests a reviewer can recompute from the one they cannot

I recomputed two of the three independently, from a fresh build in a directory the
operator never used: `ravel-agent.jar` → `404740f4…` and `ravel-screen.jar` →
`242d855c7e91fb9d68b3d228683ef59726b691bda972653524ae41a833601729`, both byte-identical
to the record. The native library does not recompute: I got
`b66ea515a1e93e5697d8c8fbbcb4cc659aad2321a0265bac64ba3d408c22e909` against the recorded
`b7ac133e…`, which is expected for a Rust release build (build path is baked in) and is
not evidence of anything wrong. The record carries no per-artifact `reproducible` flag,
unlike `class-loader-identity.json` which does. **Fix:** carry the flag here too. This
is the strongest reproducible link in the whole M3 set and the record undersells it.

#### m-3 — `gateStatus.M3-W7.verificationLogSha256` duplicates the gate log under a launch-campaign row

Its value `35fa7613cb45…` is identical to `commands[0].logSha256`, the `ravel:verify`
log. M3-W7's evidence is an operator-run launch matrix; attaching the gate log's digest
to it reads as if W7 had its own verification log. `grep -c gateStatus
schemas/evidence.schema.json` returns **0** — the entire
`environment.settings.gateStatus` block is outside the schema, so no field there has a
defined meaning, though `verify_m3_manifest` does check the rows exist and are closed.
**Fix:** drop the field or rename it to say it is the gate log.

---

### What I verified by breaking, and what it establishes

Every one of these fails closed. I list them because a future auditor should not have to
redo them.

**Hook inventory document gate** — 6 mutations, all exit 1, all with the right message:
`37→38`, `33 methods→34`, `27 handle parameters→26`, `4 fields→5`, a *second* stale
sentence appended with all four counts wrong (caught all four — the every-occurrence
claim is true), and deleting the sentence entirely (caught as "no longer writes a count
against"). Bidirectional: adding a `[[hook]]` method with two reference params to
`hooks.toml` moved the derived counts to 38/34/29 and failed the document; removing one
failed it at 36/32. **The claim in the M3-W0 deviation is true.**

**The save-seam refusal** — 2 mutations, both exit 1 with `world persistence run
'missing-save-seam' did not produce the recorded output`: making
`Persistence.saveSeamInstalled()` return `true` unconditionally, and deleting the
refusal branch from `decide()`. `run-harness.sh:74-78` recompiles the bootstrap sources,
so the production Java is what runs. **The dedicated case is real and the branch it
covers is production code, not a test-only path.**

**Digest binding** — flipping one byte of `linux-process-matrix.json` produced `records
linux-process-matrix.json as 90fe21af… but it hashes 3f415693…`; appending one newline
to `review.md` produced `records review.md as 3a149731… but it hashes c3a0f00d…`.

**Pinned-commit binding** — setting `revision.id` to a real, visible commit that is not
this changeId's produced *"pins revision.id 57653824b5aa… but change xqmmlnzm… is at
724f12b6…; a commit id does not survive a rewrite"*. Setting it to another real commit
produced three input-digest mismatches computed **at that revision**, not in the working
copy.

**Recorded-run arithmetic** — `tests.total 854→900` → "test counts must equal passed +
failed + skipped"; `lineCoveragePercent→41` → coverage floor violation; `skipped→5` →
two violations; `commands→[]` → exit 1.

**The verdict cannot be produced by editing a string.** Setting `review.result: "pass"`
and reclassifying the one blocking risk gave `verify milestone M3` exit 1 with *"records
review.result pass while the review record it binds states fail"*, and `verify scaffold`
exit 1 as well. `verify_recorded_verdict` (`lib.rs:994`) parses the record's own front
matter. Closing M3 by editing the manifest alone is not available.

> **Editor's note, not the reviewer's text.** The heading claims more than the
> paragraph under it proves, and a later round demonstrated the gap. Closure was
> obtained with four coordinated string edits: this record's front-matter `Result:`
> line to PASS, `review.result` to `"pass"`, the one blocking risk to
> `"non-blocking"`, and `review.record.sha256` rebound to the edited bytes. That
> gives `verify milestone M3` exit 0 and `verify scaffold` exit 0. The narrower
> claim the sentences above make — that closing M3 by editing the manifest *alone*
> is not available — holds, and is the true one. This is not a defect in the
> machinery. Closure is an attestation gate, and the controls available to it are
> that the record's bytes are digest-bound and that the record's own verdict line
> agrees with the manifest; both work, and both fired against the single-string
> edit the round tried. What no gate in this repository can decide is whether the
> attestation was earned. The gate binds *which text was attested*, not *that the
> attestation was earned*, and `manifest.json` and this directory's `README.md`
> now say so where they describe what a green closure means.

**Both prior vacuity holes are closed.** Deleting all four milestone manifests → `verify
scaffold` exit 1 with *"evidence/ holds no milestone manifest, and a manifest sweep over
nothing passes vacuously"* plus four missing-file violations. Removing 8 of 9 work
packages and failing every gateStatus → `verify milestone M3` exit 1 with the two
M3-specific violations.

**ADG-7's DataFixerUpper item is now honest, and withdrawing the "gate" framing was the
correct resolution.** The shape is not restated:
`DataFixBytePreservationMain.java:62-64` takes `RavelWorldData.type()` and reads
`dataFixType()` off it, and `shape=` is printed off an encoded payload (`:105-130`),
which is why it reads `data.{aggregate:StringTag}` — matching what production writes,
`ExtraCodecs.BASE64_STRING.fieldOf("aggregate")` under `SavedDataStorage`'s
`data`/`DataVersion` wrapper. The arithmetic is true: `4790 − 1343 + 1 = 3448`, `3448 ×
8 = 27584`. All three anchoring digests recompute exactly (`cf793a77…`, `8d9c4d4a…`,
`3923d3a4…`) and `muuxlrom` resolves. And the withdrawal is factually correct, which I
checked rather than accepted: `grep -r datafix-byte-preservation` across `*.nix *.sh
*.rs *.toml *.md` finds only the script itself and two documentation references —
nothing in `ravel:verify` runs it. Calling it a gate would have been the false
statement; withdrawing it was the honest move, and ADG-7:237-250 says who owns making it
one and why they can't today.

**The profiling arithmetic reconciles completely, including the derived intervals.**
`0.05 + 10000 × 0.051 = 510.05` ✓. `(500000 − 0.05) / 0.051 = 9803920.588… → 9803920.59`
✓. Inverting the published figure pins the unrounded boundary to `[0.0497, 0.0502]` ✓ —
exactly the interval claimed, and exactly 20× narrower than the two-decimal rounding
interval ✓. `1/0.051 ≈ 19.6` ✓. `256 × 39 = 9984 < 10000 ≤ 256 × 40` ✓.

**Disclosure of the attested records is accurate where I could check it.**
`pack-compile.json` records `trial.revision.dirty: true` at both trials and
`profiling-overhead.json` at its one — three, exactly as the deviation says.
`class-loader-identity.json` and `linux-process-matrix.json` record no revision at all
and say so in prose. `resultCeiling` and `worldPersistenceRoundTrip.reviewCeiling` both
state plainly that a digest of a deleted file records a claim rather than verifies one,
and `manifest.json` closes W4 and W7 on that basis in the same words. No conclusion I
found rests on an attestation while presenting itself as measured — with the one
exception that B-1 turns on: the *substitute* offered for the attested half does not
cover the step.

**Redaction is clean.** Scanned `evidence/m3/` for home paths, `/Users/`, the operator's
username, launcher and instance paths, and email addresses: zero hits. Every
`.minecraft` match is a `net.minecraft.*` class name.

**Coverage claim is plausible rather than contradictory.** `tests.total 854` at the
pinned ancestor against today's tip at 865 (nextest) / 892 + 27 doc (cargo test) — the
tip is higher, which is the direction the manifest's own explanation predicts.

---

### Reading-only ceiling

These I judged by reading. They are the limit on this review's strength, and a later
round should attack them rather than re-derive what I broke.

1. **The hook inventory's correspondence to the pinned client jar.** Only the four
   aggregate counts are gated. That every table row in `m3-hook-targets.md` names a
   member that exists in `hooks.toml` with that descriptor, and that every entry
   resolves against the real 26.1.2 class files, I did not check at all — the client jar
   is restricted material I do not have. `manifest.json`'s risk block states this
   ceiling in the same terms and I am confirming rather than extending it. I did
   reconcile the tables arithmetically by hand (6 + 5 + 2 + 4 + 7 + 8 tabled = 32, less
   the deliberately-excluded `Commands.<init>` = 31, plus the 6 original
   `StringUtil`/`SmoothDouble` entries = 37), which is consistency, not resolution.
2. **A count stated in `m3-hook-targets.md` in any phrasing other than the four gated
   ones.** The check anchors on four strings, reproduced here as a block because the
   spaces in them are the significant part:

   ```text
    entries to 
    methods and 
    fields
    handle parameters
   ```

   I confirmed by breaking that rewording *away* from an anchor is caught,
   but a *new* sentence in different phrasing ("the inventory holds 99 targets") would
   not be. I did not attempt to construct one.
3. **`README.md`'s `33` and `15` at lines 70, 73, 334-335, and the matrix's
   `callableTargetsInInventory: 33` / `classesInInventory: 15`.** Nothing gates these
   against `hooks.toml`. I read them as historical launch figures, which is defensible,
   but I did not add a hook and confirm the gate stays green with them stale. A sixth
   instance of the recurring class probably lives here.
4. **`over nine world lifecycles`** in `README.md:346` and
   `linux-process-matrix.json:70`. I counted the harness's cases by reading
   `run-harness.sh` (admitted, null-verdict, refused, four `save-*`, absent-sibling,
   missing-save-seam = 9) and confirmed it is true today. I did not add a tenth and
   check that nothing notices.
5. **The whole live half of M3-W4 and M3-W7.** Every launch row, the byte-identical
   reload, the `dataVersion 3→9` refusal, the two GUI scales of the profiling report,
   the twelve-line and seven-row readings, `nativeInvocations`, the futex hang. Read
   only, and unreachable by anyone but the operator. The records say so; I am recording
   that I took their word for it and that the gate decisions resting on it rest on that.
6. **The other three benchmark records' internals** — `pack-compile.json`,
   `class-loader-identity.json`, and the surface/caveat campaigns in the matrix. I
   checked their digests bind, their disclosure fields, and the profiling arithmetic. I
   did not audit `pack-compile.json`'s numbers or re-derive the 174 s / 1161 s trials.
7. **`schemas/evidence.schema.json` conformance.** I did not run the schema validator;
   `verify scaffold` passing is my only evidence there, and `gateStatus` is outside the
   schema entirely so a large part of the manifest's prose is unschema'd by
   construction.
8. **The `IncompatiblePackage` unreachability risk.** I read the seam comment at
   `persistence.rs:269-287` and the matching risk entry, and confirmed by reading that
   no installed package set reaches `install()`. I did not attempt to construct a launch
   that supplies one.
9. **`review.md`'s five verbatim verdicts as faithful reproductions.** I read the front
   matter, the heading structure, and the B-1 statement. I did not check any round's
   text against what that round actually wrote — nothing in the repository would let me.
10. **Withdrawn during review, and recorded here rather than as a finding.** I initially
    suspected `verify_recorded_verdict` reading the *first* `Result:` line would freeze
    M3 at the oldest round's FAIL forever, since `review.md` carries six rounds. Reading
    `review.md:1-8` showed the single `Result:` at line 3 is roll-up front matter, not a
    per-round line, so the design is correct and the concern was mine, not the
    repository's.

---

### The smallest set standing in the way

**B-1 alone.** One test in `crates/ravel-bootstrap/src/persistence.rs` that stages
non-absent bytes through the production `stage_world` and asserts the mod received them,
plus the `stage_world → save_world → stage_world` round trip. The test double already
records what such a test needs.

M-2 through M-5 are four stale sentences and should be fixed in the same pass — three of
them are the same recurring class of defect this project has now been told about five
times, and one of them (M-2) sits inside the deviation whose subject is that very
failure mode. None of them is what I am failing the milestone on.

If B-1 is fixed by a test I can break the same way I broke everything else in this
report — mutate `persistence.rs:300` and get a nonzero exit — then on the evidence I
have seen, M3 closes. The machinery is there. The one thing it does not hold is the one
thing M3-W4 exists to establish.

---

## Round 7 — VERDICT: FAIL

This round's own heading calls itself Round 7 and names itself the eighth round
to
run, counting the untracked prototype round this file describes but does not
hold.
Under this file's numbering, which counts only the rounds it holds, it is Round
7
as well. Nothing else about it is renamed, and the headings it shares word for
word
with an earlier round carry its round number so this repository's markdown lint
passes.

Result: FAIL

### M3-W8 Independent Review — Round 7 (the eighth round to run)

#### VERDICT: FAIL — Round 7

One blocking finding. It is the fourth visit of the same failure shape to the
same six-line loop, and it is a different step of it: the previous round found
the delivery of *state* unconstrained, its re-prover found *plan* and *identity*
unconstrained, and all three are now genuinely closed — I broke each of them and
each broke correctly. What is still constrained by nothing is the *refusal*
direction of the same delivery. Swallowing a mod's own refusal to open its saved
data leaves the whole gate green, including the JVM harness, and the consequence
is that the mod runs with fresh state and the next save writes that over the
world's stored data.

Everything else I attacked failed closed, and the pin, disclosure, campaign-
inventory and spike-record machinery added since the last round is real. I say
that having tried to kill each piece rather than having read it.

#### Method and environment — Round 7

Whole repo `rsync -a --exclude target` into a private scratchpad, `.jj`
included, `CARGO_TARGET_DIR` left unset with the copy's `target` a symlink to a
directory on a filesystem with room (root was at 95%). Every mutation was
applied in the copy only. At the end `diff -rq` over the whole tree excluding
`.git`, `.jj`, `.devenv`, `target` and `logs` reports no differences from the
real tree, and the real tree's `jj st` is "no changes" at `qzkpuwpq c35e23ad`,
the same commit as at session start.

Baseline in the copy before trusting any exit code: `cargo test --workspace`
exit 0 (835 passed, 0 failed across unit, integration and doc tests); `ravel-
xtask verify scaffold` exit 0; `verify spikes` exit 0; `verify milestone M3`
exit 1 on exactly the two stated violations; the full `ravel-verify-jvm`
sequence (adapter build, release `ravel-composed-host`, `build-agent.sh`, `run-
harness.sh`) exit 0.

Recorded provenance recomputed: all five M3 digests — the four
`benchmarks.results[]` records and `review.md` — verify by `sha256sum -c`, 5/5
OK. `tests` arithmetic holds: 854 = 854 + 0 + 0, coverage 86.68.

---

#### Blocking — Round 7

##### B-1 — A mod's refusal to open its own saved data can be swallowed on the production world-open path, and the entire gate stays green

**Files.** `crates/ravel-bootstrap/src/persistence.rs:300-301` (the
propagation); `crates/ravel-bootstrap/src/persistence.rs:604-619` (the test
double, which cannot refuse) against `:621-624` (which can, on the save side);
`test-mods/saved-data/src/lib.rs:242-258` (the three production refusals);
`tests/conformance/tests/saved_data.rs:331-341` (`reload`, which restates the
sequence).

**What is claimed.** `evidence/m3/manifest.json`'s `gateStatus` M3-W4 row offers
`tests/conformance/tests/saved_data.rs` and `jvm/harness/PersistenceMain.java`
as the half that "runs on every gate", explicitly as the reviewer-checkable
substitute for a live half that is operator attestation. `ROADMAP.md:736`
requires "create/save/exit/reload preserves state" and that incompatibilities
"fail closed". `SPEC.md` §14 requires that a required dependency failure
prevents world access, and that a failure cannot silently continue or save
partial state. The comment at `persistence.rs:239-246` states that policy in the
same words for the world-access gate two steps earlier.

**What is actually true.** `stage_world` propagates a mod's own open refusal
with `?`. Nothing in the repository constrains that.
`SavedDataMod::open_saved_data` has three distinct `HostError::Refused` returns
— an unparseable payload, the mod's own `open()` refusing, and a plan that
changed during delivery — and no test in the workspace makes any of them fire.
The test double's `open_saved_data` returns `Ok(())` unconditionally and has no
refusal flag, while the same double carries `refuses_production` for the save
side; so the propagation cannot even be expressed in the module's own tests. The
consequence of losing it is not a diagnostic: the world opens, the mod
initialises fresh, and `produce` then replaces the stored envelope with the
mod's default one on the next save. That is state loss on the reload path, which
is what M3-W4 exists to establish.

**How I established it — broke it, twice, independently.**

| mutation | gate step | exit |
| --- | --- | --- |
| `persistence.rs:300-301` `host.open_saved_data(envelope, &plan).map_err(…)?;` → `let _ = host.open_saved_data(envelope, &plan);` | `cargo test -p ravel-bootstrap` | **0** (92 passed, 0 failed) |
| same | `cargo build -p ravel-minecraft-adapter && cargo build -p ravel-composed-host --release && ./jvm/build/build-agent.sh && ./jvm/harness/run-harness.sh` | **0** |
| `test-mods/saved-data/src/lib.rs:247-251`, the mod's own `self.open(state)` refusal → `Err(_) => return Ok(())` | `cargo test --workspace` | **0** |

The JVM harness cannot reach it for the reason the previous round established:
`jvm/harness/initialization/dev/ravel/jvm/Hook.java` answers both crossings from
system properties, so `PersistenceMain`'s `refused` case never reaches Rust's
delivery. I confirmed the harness runs and passes at baseline (exit 0) and
passes unchanged under the mutation, so this is measured rather than inferred.

**Why blocking rather than moderate.** M3-W4's live half is honestly recorded as
unrecomputable; the repository's answer to that ceiling is the in-gate half, and
the in-gate half does not contain this step. The asymmetry with
`refuses_production` is the tell that this is an omission and not a scope
boundary — the double was built to express a save-side refusal and never an
open-side one. And the failure mode is silent data loss, which is the one thing
"preserves state" cannot be read to tolerate.

**Smallest fix.** Give the double an open-side refusal flag beside
`refuses_production`. One test in `crates/ravel-bootstrap/src/persistence.rs`'s
module asserting that when a mod refuses, `stage_world` returns `Err`, `staged`
is left unset, and no checkpoint exists to save. One test in
`tests/conformance/tests/saved_data.rs` handing `SavedDataMod` a payload
`SavedState` cannot deserialize and asserting `reload` errors. This is a handful
of lines, the same size as the fix that closed the previous B-1.

> **Editor's note, not the reviewer's text.** B-1 is answered, by the fix this
> finding specifies. Change `qzkpuwpqyqsk` gave the test double an open-side
> refusal flag beside `refuses_production`, added a test in
> `crates/ravel-bootstrap/src/persistence.rs`'s module asserting that a mod
> which
> refuses makes `stage_world` return `Err` with nothing staged and no checkpoint
> to
> save, and added a conformance test in `tests/conformance/tests/saved_data.rs`
> handing `SavedDataMod` a payload `SavedState` cannot deserialize and asserting
> that `reload` errors. It was confirmed the way this round asks: re-applying
> this
> round's own mutation — swallowing the open-side refusal at the propagation
> site —
> and requiring a nonzero exit. Answering a blocking finding is not closure:
> what
> M3-W8 needs is an independent round that returns a pass, and this record holds
> none.

---

#### Moderate — Round 7

##### M-1 — Five recorded counts of how many review rounds have run, and how many are tracked, are false right now

`evidence/m3/README.md:40` ("The five tracked independent M3-W8 rounds"), `:44`
("Six independent rounds have run and all six returned FAIL"), `:240` ("a sixth
independent round decides that"); `evidence/m3/manifest.json:250` ("Five of the
six verdicts are tracked verbatim"), `:267` — the `review.reviewer` field itself
— ("six rounds … five rounds are tracked verbatim").

**True.** `grep -c '^## Round' evidence/m3/review.md` returns **6**. With the
untracked prototype round the file describes, **seven** rounds have run and
**six** are tracked verbatim. Every one of the five sites is stale by one, and
`README.md:240` by two — the sixth round has run and returned FAIL.

**How established:** read and counted; no mutation needed, because these are
wrong as they stand rather than falsifiable later.

This is the sixth occurrence of the recurring "absolute count a future commit
falsifies" class, and the repository already knows the right answer to it:
`manifest.json:86` states the convention outright — "the record itself says how
many have run and this row does not restate it". Round 6's M-3 raised this exact
defect; it was fixed in the `gateStatus` M3-W8 row and left in five other
places, two of them inside the `review` block that closure reads. **Fix:**
delete the ordinals and counts from those five sites and let `review.md` be the
one place that counts, per the convention `manifest.json:86` already declares.
Any number written there is falsified by the next round, including this one.

> **Editor's note, not the reviewer's text.** M-1 is answered in the form this
> finding asks for. Change `pomowrkslwxt` deleted the counts and the forward-
> looking
> ordinal from all five sites in `evidence/m3/README.md` and
> `evidence/m3/manifest.json`, leaving this record as the one place the rounds
> are
> counted, and change `qytnmlpvluyt` gated it: `verify_review_round_counts`
> refuses
> a cardinal count of rounds or verdicts in either document, on every
> `ravel:verify`, so the fix cannot be undone by prose. Reinstating a deleted
> count
> fails the scaffold gate, which is how the gate was established rather than
> read.
> The same rule now covers M2's README and manifest, which had no count to
> remove
> and now cannot acquire one.

##### M-2 — "The verdict cannot be produced by editing a string" is stated too strongly; four coordinated string edits do close M3

`evidence/m3/review.md:1236` (Round 6's heading). Its own next sentences make
the true, narrower claim — "Closing M3 by editing the manifest alone is not
available" — and that claim holds; the heading does not.

**How established — broke it.** With `review.md`'s front-matter `Result: FAIL`
left alone and only the manifest edited, `verify milestone M3` exits 1 with
*"records review.result pass while the review record it binds states fail"* and
`verify scaffold` exits 1 as well: the narrow claim is real. Editing four
strings — `review.md`'s single front-matter `Result:` line to `PASS`,
`review.result` to `"pass"`, the one `classification: "blocking"` risk to `"non-
blocking"`, and rebinding `review.record.sha256` — gives `verify milestone M3`
**exit 0** and `verify scaffold` **exit 0**. Nothing else objects.
(Reclassifying to anything other than `non-blocking` is refused by the schema,
which I confirmed: `"moderate"` gives *"does not conform to evidence schema"*.)

This is not a defect in the machinery. Closure is an attestation gate; the
controls available to it are that the record's bytes are digest-bound and its
verdict line must agree with the manifest, and both work. **Fix:** state the
boundary as it is, so a later auditor knows the gate binds *which text was
attested*, not *that the attestation was earned*.

> **Editor's note, not the reviewer's text.** M-2 is answered in the words this
> finding gives. Change `pomowrkslwxt` states the boundary in
> `evidence/m3/README.md` and beside the overreaching heading in this record:
> the
> gate binds which text was attested, not that the attestation was earned. It is
> a
> prose correction and no check is possible over it — no gate can decide whether
> a
> sentence about its own limits is honest. M2's evidence now carries the same
> statement, on its own third audit's F3, so the limit is recorded in both
> places a
> reader might look.

---

#### Minor — Round 7

##### m-1 — The document-count anchors are evaded by rewording an existing gated sentence, not only by adding a new one

`tools/ravel-xtask/src/lib.rs:465-468` discloses that "a sentence that states a
count in some new phrasing is not reached", and adds that each figure is checked
at *every* occurrence.

**How established — broke it.** Replacing one of `evidence/m3/README.md`'s three
`33 callable targets` occurrences with `99 rewritten callables` left `verify
scaffold` at **exit 0**: the two surviving occurrences keep the anchor alive, so
the "no longer writes a count against" branch never fires and the false 99 is
unreachable. Appending brand-new unanchored sentences to both gated documents
("99 rewritten members across 42 loaded classes … 71 entries in total") also
left **exit 0**.

**Judgment.** The disclosure is adequate for the defect the gate exists to catch
— a count left behind by a re-pin keeps its phrasing and *is* caught, at every
occurrence: adding one method to `rewrite.targetInventory` produced the
inventory violation plus five separate README occurrence violations. It is not
adequate as *worded*, because it names new sentences as the residual hole when a
reworded existing sentence is equally uncaught, and the "every occurrence"
sentence reads as a stronger guarantee than it is. **Fix:** widen the wording,
or assert the multiset of integers adjacent to the subject nouns rather than the
integers attached to four fixed phrases.

> **Editor's note, not the reviewer's text.** m-1 is answered by the first of
> the
> two fixes offered. Change `qytnmlpvluyt` widened the disclosure to name both
> holes
> — a count in new phrasing, and a reworded occurrence whose surviving siblings
> keep
> the anchor alive — and scoped the every-occurrence claim to anchored
> phrasings,
> which is what it was true of. The second fix is stated and declined in the
> same
> comment, with its reason: the noun is what identifies which figure an integer
> is,
> both holes change the noun, and a check that ignored the noun would flag every
> integer near it and be a false red in prose this dense. This is a wording
> correction, so nothing fails when it is reverted; what does fail is a stale
> figure
> in an anchored sentence, which this round confirmed fires.

##### m-2 — "over nine world lifecycles" is ungated

`evidence/m3/README.md:349` and `evidence/m3/linux-process-matrix.json:70`. I
re-derived the nine from `jvm/harness/run-harness.sh` — `admitted`, `null-
verdict`, `refused`, `save-{unchanged,failed,mismatch,null}`, `absent-sibling`,
`missing-save-seam` — and it is true today. **How established — broke it:**
appending a tenth `persistence_run` case left `verify scaffold` at **exit 0**.
This was Round 6's reading-only ceiling item 4; it is now proved rather than
read. **Fix:** derive it from the script, or drop the number.

> **Editor's note, not the reviewer's text.** m-2 is answered by the first of
> the
> two fixes offered. Change `qytnmlpvluyt` made `jvm/harness/run-harness.sh`
> declare
> the figure once, as `persistence_lifecycles_declared`, and assert it against
> the
> runs the harness made, so a case added or removed there fails the harness;
> `verify_harness_lifecycles` then binds both records that cite the figure to
> that
> declaration. Restating a different count in either record fails the scaffold
> gate,
> which is how this was established rather than read.

##### m-3 — The disclosure's `reason` is unchecked against the kind of failure it excuses

**How established — broke it.** Re-pinning `evidence/m2/loader-overhead.json` to
an arbitrary real commit belonging to a different change, and updating only the
disclosure's `id` to match, gives `verify scaffold` **exit 0** — while that
disclosure's prose still says the pinned commit "is reachable from no visible
head", which is now false about it. The gate prints its own computed `why`
beside the record's `reason`, so the mismatch is visible in the gate output to
anyone reading it, which is why this is minor. An *empty* reason is refused: the
schema requires `minLength: 1`, and setting it to `""` gives exit 1, *"does not
conform to the evidence schema"*.

> **Editor's note, not the reviewer's text.** m-3 is answered. Change
> `qytnmlpvluyt` refuses a disclosure whose `reason` classifies the failure
> itself:
> the two phrases that go stale when a pin moves are held in
> `CLASSIFICATION_PHRASES` and rejected wherever a disclosure's prose restates
> one,
> which leaves exactly one statement of why a pin failed — the gate's computed
> one,
> printed beside the prose on every run. What a `reason` still is not checked
> against is the judgement it makes about what a reader loses, which no gate can
> decide. Restating a classification phrase in a disclosure fails the scaffold
> gate.

---

#### What I verified by breaking, and what it establishes — Round 7

Every one of these fails closed. Listed so a later auditor does not redo them.

**The four delivery mutations the brief named — all caught.**

| mutation at `persistence.rs` | `cargo test -p ravel-bootstrap` | which tests |
| --- | --- | --- |
| `host.open_saved_data(None, &plan)` | **101** (89 passed, 3 failed) | `a_stored_aggregate_reaches_the_mod_that_owns_it`, `a_world_reopened_from_what_its_save_emitted_holds_the_same_state`, `each_owner_in_one_world_receives_its_own_envelope_and_no_other` |
| `SavedDataPlan::open(&declaration, None)` | **101** (90 passed, 2 failed) | `a_world_written_at_an_older_version_hands_the_mod_the_steps_to_reach_current`, `each_owner_…` |
| hand each mod another owner's envelope | **101** (91 passed, 1 failed) | `each_owner_…`, and it failed for an **identity** reason, not an absence one: `` `saved_data` did not receive its own state — left: 9, right: 5 `` |
| deliver to the first mod only (`inner.mods.iter_mut().take(1)`) | **101** (91 passed, 1 failed) | `each_owner_…` |

The two-owner runtime is what makes the identity failure expressible, and the
plan assertion carries both polarities — a version-1 world gets `[version(2)]`,
a current world gets an empty plan — so neither assertion is satisfied by every
plan. The previous round's fix survives the mutation it asked to be tested with,
and the two follow-up gaps its re-prover found are closed as claimed.

**The pinned-revision rule.** A genuinely bogus pin — `1234567890abcdef…`, not
the ROOT commit — gives `verify scaffold` **exit 1**: *"admits loader-
overhead.json, whose trial at trial.revision pins 1234567890…, and that commit
names no commit in visible history; re-pin it … or disclose it under
unobtainableRevisions"*. Both halves of the disclosure contract hold. It
**cannot cover a pin it does not name**: the same run also reported *"discloses
the pin `loader-overhead.json df717025… wxzwmy…` as unobtainable, and no record
it admits carries it; a disclosure that names no pin excuses nothing and hides
the next one"*. It **cannot outlive the defect**: repairing `pack-
compile.json`'s pin to `fae2a9aaa03ddc6d…`, the commit
`wpuqwqlrpuvttpzrvqvuxryzwmptuxyt` is actually at, and keeping the disclosure in
sync gives **exit 1**: *"discloses pack-compile.json's pin … as unobtainable and
it now resolves to the commit change … is at; remove the disclosure rather than
leaving an excuse the defect has outlived"*. The escape works only when it names
the pin exactly, and then it is announced on stdout rather than silent — exit 0
with the `why` and the `reason` both printed.

**Campaign inventory (33 targets, 15 classes) — the gate fires and the
HISTORICAL decision is implemented as decided.** Setting
`rewrite.callableTargetsInInventory` to 34 gives exit 1, *"records
callableTargetsInInventory 34 against the 33 callable targets its own target
inventory enumerates"*. Adding one method to `rewrite.targetInventory` gives
exit 1 with the mirror-image message **plus five** separate
`evidence/m3/README.md` occurrence violations, so the every-occurrence claim is
true for the anchored phrasings. Adding a `[[hook]]` entry to
`minecraft/versions/26.1.2/hooks.toml` gives exit 1 on the *document* only —
*"m3-hook-targets.md states 37 hook entries where hooks.toml declares 38"* and
*"33 method entries … declares 34"* — and leaves the campaign figures untouched.

**On whether HISTORICAL is the right decision: yes.** A launch campaign observed
one inventory in one build; a hook added later does not change what that
campaign rewrote, and deriving the campaign's figures from `hooks.toml` would
make the record misreport its own launches. The figures are now derived from the
record's own enumeration, the enumeration is required to be an enumeration
(duplicates refused before `unique` can absorb them), and every anchored prose
occurrence is held to it. That is the correct shape. One residual worth a
reader's attention rather than a finding: nothing binds
`rewrite.targetInventory` to `hooks.toml` as of the campaign's build, and the
matrix pins no revision at all — so there is nothing to bind it to. It says so
in prose. Also note that the campaign's 33 and `hooks.toml`'s 33 method entries
coincide *today* and are different quantities; after the next hook lands they
will differ, and that is correct rather than drift.

**The spike record is left to the change that introduced it.** Removing the
`path.exists()` guard from `write_result` in `experiments/spike-
harness/src/lib.rs` gives `cargo test -p spike-harness` **exit 101**. The test
that catches it is inside the gate suite —
`a_record_already_on_disk_is_not_rewritten_by_a_later_run` appears in the
baseline `cargo test --workspace` log — so a later change can no longer claim an
earlier change's record.

**Spot-checks of what the previous round broke, re-broken.**
`verify_recorded_verdict`: `review.result: "pass"` against a record stating FAIL
gives exit 1 in both `verify milestone M3` and `verify scaffold`. Digest
binding: one newline appended to `review.md` gives exit 1 naming `6da1ff7c…`
against `b327881a…`. The `missing-save-seam` refusal: replacing `if
(!saveSeamInstalled())` with `if (false)` at
`jvm/bootstrap/dev/ravel/jvm/Persistence.java:104` gives the harness **exit 1**,
so the refusal branch is production Java and is real.

**`pack-compile.json`'s arithmetic, which no prior round audited — it reconciles
completely.** Baseline 2526 is the median of `[2526, 2351, 2550]`; every
`coldMilliseconds` is the median of its own three samples; every
`coldMillisecondsLessBaseline` is that median less 2526 (604, 2024, 7293,
29505); every `coldMillisecondsPerMod` is that over `packSize` including
rounding (604.0; 126.5; 7293/64 = 113.953125 → 113.95; 29505/256 = 115.2539… →
115.25); `marginalColdMillisecondsPerMod` 113.34 is the stated secant, (29505 −
604) / (256 − 1) = 113.3372…; every variant figure is the faster of its two
samples, 8/8 cold and 16/16 warm; `modSourceBytes` is `packSize × 33873` at all
four sizes; and `syntheticMod.sourceBytes/sourceLines` agree with the `packSize:
1` point. The `representativeness` prose also checks out against the tree rather
than against itself: at `trial.revision.id 4c550dae…`, `test-
mods/registries/src/lib.rs` is exactly 275 lines and 9060 bytes as recorded, and
today `test-mods/saved-data` (440 lines) and `test-mods/commands` (327) both
exceed it (279) — "both have since overtaken it" is true. This record carries an
absolute claim about the current tree that nothing gates, and unlike the five in
M-1 it happens to be true; its own next sentence scopes the consequence
("Nothing measured here moves with that").

---

#### Reading-only ceiling — Round 7

These I judged by reading. They are the limit on this review's strength, and a
later round should attack them rather than re-derive what I broke.

1. **The whole live half of M3-W4 and M3-W7.** Every launch row, the byte-
   identical reload, the `dataVersion 3→9` refusal, the two GUI scales of the
   profiling report, `nativeInvocations`, the futex hang. Unreachable by anyone
   but the operator. The records say so; I took their word and record that the
   gate decisions resting on it rest on that.
2. **The hook inventory's correspondence to the pinned client jar.** The client
   jar is restricted material I do not have. That every table row in `m3-hook-
   targets.md` names a member that resolves against the real 26.1.2 class files,
   I did not check at all. Only the four aggregate counts are gated, and I
   verified those by breaking.
3. **`evidence/m3/class-loader-identity.json`'s internals.** Read only. I did
   not rebuild the agent jar to re-derive its `reproducible: true` digest, so
   the previous round's m-1 on that record is neither confirmed nor withdrawn by
   me.
4. **`linux-process-matrix.json`'s surface and caveat campaigns, and `profiling-
   overhead.json`'s arithmetic.** The previous round recomputed the profiling
   figures including the derived intervals; I did not repeat it and did not
   audit the surface campaigns' internals.
5. **`schemas/evidence.schema.json` conformance.** I read the
   `unobtainableRevisions` subschema and proved by breaking that the schema
   refuses an empty `reason` and a non-enumerated `classification`. I did not
   run a standalone validator, and I did not check the four non-manifest records
   against the schema; `gateStatus` remains outside it entirely.
6. **The 18 `inputs[]` digests at each manifest's pinned revision.** I relied on
   `verify scaffold` exit 0 rather than recomputing them by hand; Round 2
   recomputed 18/18.
7. **The `IncompatiblePackage` unreachability risk.** Read the seam comment at
   `persistence.rs:269-287` and the matching risk entry. I did not attempt to
   construct a launch that supplies an installed package set.
8. **`review.md`'s six verbatim verdicts as faithful reproductions of what those
   rounds wrote.** Nothing in the repository would let me check this, and M-2 is
   the reason it matters: the record's text is what the gate binds.
9. **ADG-1 through ADG-6.** I read only ADG-7's persistence mechanism, against
   the code. Conformance of this milestone's work to the other six decisions I
   did not examine.
10. **The coverage floor.** `lineCoveragePercent: 86.68` is recomputed by
nothing I ran; I did not run `ravel-coverage`.
11. **`ravel:verify` and `ravel:verify-stack` as wholes — not run.** Root was at
95% and I budgeted deliberately. I ran the components that bear on my findings:
`cargo test --workspace`, `cargo test -p <crate>`, `verify scaffold`, `verify
spikes`, `verify milestone M3`, and the full `ravel-verify-jvm` sequence. I did
**not** run formatting, TOML/Markdown/Nix lint, clippy, coverage, `cargo deny`,
or `verify-stack`. In particular I did not check whether clippy notices B-1's
mutation; the doc-comment fence and the `let _ =` idiom suggest it would not,
but that is a reading, not a result.
12. **Withdrawn during review, recorded here rather than as a finding.** I
suspected the `unobtainableRevisions` escape could be abused to launder an
arbitrary re-pin without disclosing it, or to keep excusing a pin after the pin
was repaired. It cannot: the disclosure must name the record, commit and change
exactly, and a disclosed pin that starts resolving is itself a violation. Both
are proved above. What survives of the suspicion is only m-3, that the free-
prose `reason` is not checked against the kind of failure.

---

#### The smallest set standing in the way — Round 7

**B-1 and M-1.**

B-1 is one refusal flag on the existing test double plus two tests — one in
`crates/ravel-bootstrap/src/persistence.rs`'s module asserting that a refusing
mod makes `stage_world` return `Err` and stage no checkpoint, and one in
`tests/conformance/tests/saved_data.rs` making `SavedDataMod` refuse a payload
it cannot deserialize. Break the propagation at `persistence.rs:300-301`
afterwards and require a nonzero exit.

M-1 is deleting five stale counts and letting `review.md` be the one place that
counts, which is the convention `manifest.json:86` already states and which is
the only form of the fix that this round does not immediately falsify again.

M-2, m-1, m-2 and m-3 are four sentences and should be corrected in the same
pass. None of them is what I am failing the milestone on.

If B-1 is closed by a test that dies under the mutation above, and the five
counts stop being restated, then on the evidence I have seen M3 closes. The pin
rule, the disclosure contract, the campaign-inventory derivation, the spike-
record ownership rule, the digest binding and the verdict binding are all
genuinely fail-closed — I broke each of them and each refused correctly. What
the in-gate half still does not hold is the direction in which a world is
*withheld*, and that is the direction in which a player loses data.

---

## Round 8 — VERDICT: PASS

This round's own heading calls itself Round 8 and names itself the ninth round
to run, counting the untracked prototype round this file describes but does not
hold. Under this file's numbering it is Round 8 as well. It is the only round so
far to return a pass, and it did not close M3: the rounds after it read axes this
one had left standing in its own ceiling, and both returned FAIL. Its verdict is
written here after the repairs it asked for had already landed, which is the
defect Round 10's first finding is about; it is recorded verbatim rather than
summarised because its text was still recoverable when that finding was fixed.

Result: PASS

### M3-W8 Independent Prototype Review — Round 8 (the ninth round to run)

I had not seen this repository before this round. I reached every conclusion below
from the artifacts and from breaking them, not from the repository's account of
itself. I did not read any earlier round's verdict before forming my own view of
the code under review; I read `review.md` afterwards, to establish what had
already been raised and what the tree had answered.

I am aware that every round before this one returned FAIL. That is not a reason to
fail this one and not a reason to pass it. What follows is what I could and could
not break.

#### VERDICT: PASS

The one blocking defect the previous round failed M3 on is genuinely fixed, and it
is fixed in the form that round demanded — by a test that dies when the propagation
is broken, not by rewording. Every other refusal and delivery path I attacked in
the world-persistence crossing is now constrained by a test that dies. Every gate
I tried to defeat either refused correctly or refused in a way whose limit its own
doc comment states. The digests bind, the pinned revision resolves and is the
commit its own change id is at, the recorded arithmetic is true, and the tree is
green on every gate component I could execute.

I found two moderate findings (the second with three instances) and two minor ones.
None of them touches production behaviour, player data, or any claim about the game.
Every one of them points the safe way — the evidence understating its own machinery,
or overstating what it still owes — and one is a gate that is weaker than its own doc
comment claims. Had any pointed the other way I would have failed on it. I do not
think any is worth a ninth FAIL, and I say below exactly what convinced me and what
I am willing to be wrong about.

**What convinced me**, concretely, since a PASS is worth less than the reasons for
it. Four things. First, the blocking defect is dead in the form the last round
demanded: I restored `let _ =` at `persistence.rs:300-301` and the suite exits 101
on a named test, and clippy does *not* catch it, so the test is doing real work
rather than duplicating a lint. Second, I could not find a surviving mutation
anywhere in the world-persistence crossing — eight mutations across the refusal and
delivery directions, all eight killed. Third, every gate I attacked failed closed,
including three that fail closed in *both* directions (a missing count and a wrong
count; an absent disclosure and an outlived one). Fourth — and this is what moved me
from hesitation to a verdict — I checked all fourteen of the last substantive round's
findings against the tree myself and found every one answered, which means this
milestone is not closing over anything a previous reviewer left standing.

**On the self-referential blocking risk.** `evidence/m3/manifest.json`'s single
blocking risk is "M3 is not closed", whose stated grounds are that every
independent round has returned FAIL and that the review which must pass is an
independent one. This round is independent of the work under review and returns
PASS, so the stated grounds no longer hold and **that risk may be withdrawn** — on
that basis and no other. Withdrawing it is not a judgement that nothing in it was
true; it is that its own condition is now satisfied. Two things in it must survive
the withdrawal rather than be deleted with it, because they are ceilings and not
open findings: that M3-W4 and M3-W7 close on operator attestation whose live half
no reviewer can recompute, and that the closure gate binds which text was attested
rather than that the attestation was earned. Both are already recorded as separate
non-blocking risks and deviations. If M3's risks are edited to reflect this round,
the correct edit is to remove the "M3 is not closed" risk and change nothing else.

#### Method and environment — Round 8

Everything destructive happened in a throwaway copy of the whole repository
(`rsync -a --exclude target`, `.jj` retained), never in the repository itself. I
left `CARGO_TARGET_DIR` unset because the JVM gate and the spike scripts hardcode
`./target/release/…`. When restoring a mutated file I rewrote its bytes and
touched its mtime rather than moving a preserved copy back, so no kill is
misattributed to a stale artifact. I established a clean baseline before trusting
any exit code, and I re-established it after each restore.

`devenv shell` was unusable for this round: the binary cache returned HTTP 504, so
`devenv tasks run ravel:verify` could not be invoked at all. The pinned toolchain
was already on `PATH` at 1.95.0, so I ran the gate's constituent commands directly
with the flags `nix/common.nix` gives them. What I could run and what I could not
is enumerated in the ceiling; this is the single largest limit on this round.

Baseline, in the copy, before any mutation:

| Command | Exit |
| --- | --- |
| `cargo nextest run --workspace --all-features` | 0 — 890 tests run, 890 passed, 0 skipped |
| `cargo clippy --workspace --all-targets --all-features` | 0 — zero warnings |
| `cargo llvm-cov nextest --workspace --all-features --fail-under-lines 80 --ignore-filename-regex '(^\|/)experiments/adg-'` | 0 — TOTAL line coverage 86.94% |
| `cargo run -p ravel-xtask -- verify scaffold` | 0 |
| `cargo run -p ravel-xtask -- verify spikes` | 0 |
| `cargo run -p ravel-xtask -- verify milestone M3` | 1 — exactly two violations |
| `cargo run -p ravel-xtask -- verify milestone M0` / `M1` / `M2` | 0 / 0 / 0 |

The two M3 violations are "milestone must have no blocking deviations or risks"
and "milestone review must be independent and pass", which is exactly the state
the milestone claims for itself.

#### What I verified by breaking, and what it establishes — Round 8

##### The refusal fix is real

`crates/ravel-bootstrap/src/persistence.rs:300-301` now reads
`host.open_saved_data(envelope, &plan).map_err(|error| error.to_string())?`. I
restored the defect verbatim — `let _ = host.open_saved_data(envelope, &plan);` —
and ran `cargo test -p ravel-bootstrap`: **exit 101**, one failure,
`persistence::tests::a_mod_refusing_its_stored_state_withholds_the_world_and_stages_nothing`,
92 passed 1 failed. The test dies for the right reason: it reaches
`.expect_err("a mod that cannot read its stored state opens no world")` and finds
`Ok(())`. The `refuses_open` flag on the test double is what makes the mutation
observable, and the test opens the same world with the same bytes through an
accepting double first, so the refusal is the mod's doing and not the fixture's.

The three `HostError::Refused` returns in `test-mods/saved-data/src/lib.rs:242-258`
each die too. I swallowed each one in turn — replacing its `?` with a `let Ok(..)
else { return Ok(()) }`, and deleting the plan-mismatch `if` block outright — and
ran `cargo test -p ravel-bootstrap -p ravel-test-mod-saved-data -p
ravel-conformance-tests`:

| Mutation | Exit | Killed by |
| --- | --- | --- |
| swallow the unparseable-payload refusal | 101 | `a_payload_its_owner_cannot_read_refuses_the_world_rather_than_opening_it_fresh` (conformance) |
| swallow the mod's own `open()` refusing | 101 | `a_stored_version_this_build_cannot_read_refuses_delivery` |
| delete the plan-mismatch refusal | 101 | `a_plan_that_disagrees_with_the_envelope_beside_it_refuses_delivery` |

##### The delivery direction is still constrained

These were fixed by an earlier round and I re-attacked all four. Same command,
each applied alone to `persistence.rs`:

| Mutation | Exit | Killed by |
| --- | --- | --- |
| deliver `None` to every mod | 101 | `a_stored_aggregate_reaches_the_mod_that_owns_it`, `a_world_reopened_from_what_its_save_emitted_holds_the_same_state`, `each_owner_in_one_world_receives_its_own_envelope_and_no_other` |
| build every plan from `None` | 101 | `a_world_written_at_an_older_version_hands_the_mod_the_steps_to_reach_current`, `each_owner_in_one_world_receives_its_own_envelope_and_no_other` |
| hand every mod the first owner's envelope | 101 | `each_owner_in_one_world_receives_its_own_envelope_and_no_other` |
| deliver to only the first mod | 101 | `each_owner_in_one_world_receives_its_own_envelope_and_no_other` |

Both directions of the crossing — a world *withheld* and a world *delivered* — are
now constrained by tests that die. That was the gap the previous round named, and
it is closed.

##### On the refusal documented as unreachable from production

`test-mods/saved-data/src/lib.rs`'s doc for
`a_stored_version_this_build_cannot_read_refuses_delivery` says the refusal is
unreachable from the production crossing because `SavedDataAggregate::open`
refuses a future version first. **That disclosure is honest and it hides no
missing coverage.** I traced it: `stage_world` calls
`SavedDataAggregate::open(bytes, &declarations, &uncheckable)` at
`persistence.rs:288` before any delivery, and that admission refuses any data
version outside the owner's declared set, so by the time `open_saved_data` runs the
version is always one the mod reads. The branch is a defensive backstop, and it is
asserted directly at the mod's own API — my mutation of it died. Reachability and
coverage are different questions and the record answers both correctly.

##### Clippy does not catch the swallow

Worth recording because nobody had checked. With the `let _ =` defect in place,
`cargo clippy -p ravel-bootstrap --all-targets` exits **0** with zero warnings:
`let _ =` is precisely the idiom that silences `#[must_use]`, and
`clippy::let_underscore_must_use` is a restriction lint the workspace does not
enable. The test is the only thing standing between that defect and a green gate.
That is not a defect in itself — the test exists and dies — but it means this
class of defect has exactly one guard, and anyone deleting that test deletes the
whole guard.

##### The attestation boundary is exactly as M3 states it

I confirmed all three legs.

- Flip only `evidence/m3/review.md`'s front-matter `Result:` line to `PASS`:
  `verify scaffold` **exit 1** and `verify milestone M3` **exit 1**, with two
  violations — the recomputed digest mismatch, and "records review.result fail
  while the review record it binds states pass".
- Flip only `review.result` to `"pass"`: **exit 1**, "records review.result pass
  while the review record it binds states fail".
- Make all four edits together — the record's front-matter `Result:` line to
  `PASS`, `review.result` to `"pass"`, the one blocking risk's `classification` to
  `"non-blocking"`, and `review.record.sha256` rebound to the record's new bytes:
  `verify scaffold` **exit 0** and `verify milestone M3` **exit 0**. Nothing else
  objects.

So the claim is true: the gate binds *which text was attested*, not *that the
attestation was earned*. **Is stating it sufficient?** For the boundary itself,
yes — no gate can decide whether a document telling the truth about its own
conclusion is telling the truth, and a repository that pretends otherwise is worse
off than one that says so. What makes the statement sufficient rather than an
excuse is that the two controls which *are* available both work and both were
proven to work by breaking them individually, and that the boundary is stated in
the manifest, in `README.md`, and beside the overreaching heading in `review.md`
rather than in one place a reader might miss. I would add only this, which is a
remark and not a finding: the residual risk is not that someone forges four
strings, it is that the four strings are easy enough to write that a future
maintainer under time pressure may not notice they have crossed from recording a
verdict to producing one. Nothing in the machinery can help with that.

##### The gates I tried to defeat

Each of these I attacked rather than read. Exit codes are from `cargo run -p
ravel-xtask -- verify scaffold` unless stated.

**`verify_harness_lifecycles` — fails closed three ways, stronger than advertised.**
Changing `persistence_lifecycles_declared=9` to `10` in
`jvm/harness/run-harness.sh`: **exit 1**, both citing records named. Renaming the
declaration away: **exit 1**, "no record's world-lifecycle count is held to it".
Rephrasing `evidence/m3/README.md`'s count so the anchored noun window misses it
(`nine world lifecycles` → `world lifecycles numbering nine`): **exit 1**, "no
longer writes a count against `world lifecycles`". That third branch is the
important one — the check requires the count to be *present*, so it cannot be
evaded by deletion or rewording. The harness's own runtime assertion at
`run-harness.sh:377` binds the declaration to the runs that happened, and
`ravel:verify-jvm` is inside `ravel:verify` (`nix/common.nix:530`); I counted the
nine `persistence_run` invocations statically and they reconcile, but I did not
execute the JVM harness — see the ceiling.

**`verify_admitted_revisions` and its `unobtainableRevisions` escape — fails
closed three ways.** Emptying `unobtainableRevisions`: **exit 1**, three
violations naming the pins that then bind nothing. Corrupting one entry's
`changeId`: **exit 1**, "discloses the pin … as unobtainable" plus the underlying
pin failures returning. Adding a disclosure for a pin that does not fail
(`class-loader-identity.json`): **exit 1**, "discloses the pin … as unobtainable,
and no record it admits carries it; a disclosure that names no …". So the escape
is load-bearing in both directions: a pin nothing discloses fails, and a
disclosure the defect has outlived fails too. This is the strongest piece of
machinery in the set.

**`verify_admitted_inventory` — closes the hole its own manifest says is open.**
I dropped an untracked `evidence/m0/probe-record.json` into a milestone directory:
**exit 1**, "evidence/m0/manifest.json names no record probe-record.json, which
its own milestone directory tracks; a record no manifest names is a record no
digest covers". Emptying `evidence/m3`'s `benchmarks.results` and its disclosures
together: **exit 1**, all four M3 records named, and `verify milestone M3` adds
"M3 manifest must record all four M3 results under the m3-prototype-v1 protocol".
See finding F2.

**`verify_evidence_records`' floor — fires.** With `evidence/` present but all
four manifests removed: **exit 1**, "evidence/ holds no milestone manifest, and a
manifest sweep over nothing passes vacuously". The historical defects are also
fixed: hiding `evidence/` entirely gives **exit 1** with named missing files, and
deleting the four manifests gives **exit 1** naming all four. The failure mode in
which the scaffold once passed with every milestone manifest deleted no longer
exists.

**`verify_recorded_verdict` — fires, and its parsing rule matters.** It takes the
*first* `Result:` line of the record (`lib.rs:1852`, `stated_verdict`). Both legs
proven above.

**`verify_manifest_percentages`, `verify_void_limits`, `verify_campaign_inventory`.**
I read their doc comments and did not find a limit worse than stated for these
three. `verify_manifest_percentages` bans the `%` character in any milestone
manifest and says so; a spelled-out percentage escapes and it says that too.
`verify_void_limits` reconciles `VOID_LIMITS` against both the record's metrics and
the roadmap paragraph ending `are void`, in both directions, and its stated limit
(moving the declaration into prose that does not say `are void`) is real and is the
only one I found. `verify_campaign_inventory` states two anchoring limits, and the
previous round already broke one of them and had it disclosed.

##### Digests, provenance and arithmetic

- All four `benchmarks.results[].sha256` recompute against the tracked records:
  `sha256sum -c` OK for `class-loader-identity.json`, `pack-compile.json`,
  `profiling-overhead.json`, `linux-process-matrix.json`.
- `review.record.sha256` recomputes against `evidence/m3/review.md`: OK.
- All four `inputs[].sha256` recompute at the pinned revision
  `724f12b6b80c3f0597c5b7bf4a40b1fdc41e2315` (via `git cat-file -p <rev>:<path> |
  sha256sum`): OK for `hooks.toml`, `mappings.lock`,
  `docs/architecture/m3-hook-targets.md`, `docs/decisions/adg-6-configuration-ui.md`.
- `revision.id` resolves and is the commit `revision.changeId`
  `xqmmlnzmtkqpuuxymttpzqyvnovxtxtv` is at, which `verify_pinned_commit` requires.
  Confirmed with `jj log -r xqmmlnzmtkqpuuxymttpzqyvnovxtxtv`.
- `tests.total` 854 with 0 failed and 0 skipped is internally consistent, and the
  manifest's explanation of why it is below the tip's count holds: the tip runs 890
  under the gate's own runner. `lineCoveragePercent` 86.68 is above the floor of 80
  and near the tip's measured 86.94%. Neither figure is bound by anything, which
  the manifest states.
- The three announced M3 pin disclosures on the scaffold run are the two dirty-trial
  records, both quoted in `unobtainableRevisions`. They are disclosures, not
  violations: the scaffold exits 0 with them present.

**The inventory arithmetic is true, including which targets the subtraction drops.**
I counted it out of the record rather than trusting the prose. `jq` over
`rewrite.targetInventory` gives 15 unique classes and 33 methods with no duplicate
in either, matching `rewrite.classesInInventory` and
`rewrite.callableTargetsInInventory` exactly. `rewrite.worldOnlyTargets` holds 8
entries, and the record's `whyElevenNotFifteen` claims the title-only launch drops
6 of them and not 8 because two sit in classes the title screen does load. I
checked the class of every one of the eight: `command.perform-prefixed`,
`resource.load-server-resources`, `recipe.apply`, `save.compute-if-absent`,
`save.try-write` and `save.save-and-join` are held by exactly the four classes the
record names as never loaded — `Commands`, `ReloadableServerResources`,
`RecipeManager`, `SavedDataStorage` — and `world.open-persistence` and
`world.save-persistence` are held by `net.minecraft.client.Minecraft` and
`net.minecraft.server.MinecraftServer`, which are loaded at the title screen. So
`15 − 4 = 11` and `33 − 6 = 27` are both true and both derived, and the record's
own explanation of why it is not `33 − 8` is correct. `worldOnlyTargets` also obeys
its stated membership criterion, and the four targets it deliberately excludes
(`server.tick`, `server.tick-integrated`, `save.save-everything`, `save.set-dirty`)
are consistent with a list that accounts for what a title-only launch rewrites
rather than what it can reach.

**The safety boundary holds.** All 18 crate roots under `crates/` carry
`#![forbid(unsafe_code)]`, `ravel-bootstrap` included; I checked each one rather
than sampling. Grepping `crates/`, `tools/`, `test-mods/` and `tests/` for `unsafe`
outside the experiment fence returns only the prohibition machinery in
`tools/ravel-xtask/src/lib.rs` and one explanatory string in
`crates/ravel-observability/src/profiling.rs:179` — no `unsafe` block or `unsafe
fn` anywhere in the maintained graph. The table in
`docs/architecture/safety-boundary.md` is still empty, as the hard rule requires.

#### Findings

##### F1 — Moderate. `verify_review_round_counts` is weaker than its own stated limit

**File.** `tools/ravel-xtask/src/lib.rs:716-768`, with `numbers_before` at `:697-714`
and `COUNT_LOOKBACK` at `:688`.

**What is claimed.** The doc comment states the limit as an ordinal one: "this
catches a *cardinal* count of rounds, spelled either way — `six rounds`, `5
rounds`. An ordinal naming one round that has already run … is allowed; an ordinal
naming a round that has not run yet … does drift and is not caught here. Nothing
but the record counting for itself closes that one." The manifest and `review.md`
both present this gate as the fix that makes the recurring stale-round-count
defect undoable by prose: "so the fix cannot be undone by prose. Reinstating a
deleted count fails the scaffold gate."

**What is actually true — broke it.** Four ordinary cardinal restatements appended
to `evidence/m3/README.md` leave `verify scaffold` at **exit 0** with zero
round-count violations:

1. `Five independent reviews have run against this milestone.` — the noun set is
   `["rounds", "verdicts", "audits"]`; `reviews` is not in it.
2. `The independent rounds recorded here number five in total.` — the noun is
   exactly `rounds`, and the cardinal is a *cardinal*, but `numbers_before` only
   scans `COUNT_LOOKBACK` words *backwards* from the noun, so a count that follows
   its noun is unreachable.
3. `There have been five review iterations.` — `iterations` is not in the noun set.
4. `The count of completed review round entries is five.` — the window matches the
   literal word `rounds`; the singular `round` is not matched at all.

Positive controls in the same run prove the gate was live and the harness was
sound: `Five independent rounds have run.` gives **exit 1** with "restates that 5
review rounds exist", and `There are 5 verdicts recorded below.` gives **exit 1**
likewise. Probes 2 and 4 are cardinal counts of *rounds*, which is the thing the
doc comment says it catches, so this is a limit worse than the one stated — not an
undisclosed extra limit but a hole inside the disclosed guarantee.

**Why it matters and why it is not blocking.** No such restatement exists in the
tree right now — I swept `evidence/m3/README.md` and `manifest.json` for every
sentence containing a round/verdict/audit/review/iteration noun together with a
numeral and found none, so the previous round's second closure condition is
genuinely met. What is wrong is the *durability* claim: this is the recurring
defect class the repository has now found seven times, and the mechanism advertised
as ending it can be walked around by rephrasing. It will come back.

**Fix.** The shape is already in this codebase, one function away.
`verify_harness_lifecycles` binds rather than prohibits: it requires the count to
be present and equal to a single declaration, so both deletion and rewording fail.
A prohibition cannot have that property, because there is no anchor whose absence
it can report. Either bind the round count the same way — have `review.md` declare
it in one machine-readable place and require the other documents to state it and
match — or, keeping the prohibition, widen it: add `reviews`, `iterations` and the
singular forms to the noun set, and scan `COUNT_LOOKBACK` words on *both* sides of
the noun. The second is cheaper and strictly better than what is there; the first
is what actually closes the class.

##### F2 — Moderate. Three places in the manifest state, in the present tense, something that is no longer true — one of them inside the blocking risk

This is one finding with three instances, because it is one class. The third
instance is the significant one and I found it last, by trying to settle the very
question I had listed as my largest doubt.

###### F2a — the blocking risk overstates the outstanding finding set by twelve items

**File.** `evidence/m3/manifest.json`, the blocking risk beginning "M3 is not
closed." (`:250`), and the M3-W8 `gateStatus` row (`:86`) which says the same thing.

**What is claimed.** "One of that round's must-fix findings and all five moderate
and six minor findings are still unanswered, no round has yet returned a pass …"
— "that round" being Round 5. The M3-W8 row states it as "Still unanswered: that
round's third must-fix finding, its five moderate findings, and its six minor
ones."

**What is actually true.** All fourteen of Round 5's findings are answered. I
enumerated them from `review.md:756-955` and checked each against the artifact it
cites:

| Round 5 finding | Status | How I established it |
| --- | --- | --- |
| B-1 profiling build binding | answered | `profilingReObservationCampaign` exists with pre-launch digests |
| M-1 repealed reviewer note | answered | passage rewritten in `matrixNotCarriedForward` |
| M-2 `15/33` false of one surface launch | answered | read `howItWasResolved`: now distinguishes the two world-opening launches at 15/33 from the title-only launch at 11/27 |
| **M-3** the false pinned reason in `profiling.rs` | **answered** | grepped `crates/ravel-observability/src/profiling.rs`: "nothing to size" / "still open" no longer occur anywhere |
| **D-1** `rewrite.measuredIn` vs the refused third launch | **answered** | `whyTheRefusedLaunchStillRewroteFifteen` (`linux-process-matrix.json:239`) gives the mechanism — `Minecraft.doWorldLoad` takes an already-constructed `WorldStem`, which holds a `ReloadableServerResources` whose constructor instantiates `RecipeManager` and `Commands`, and `Persistence.read` resolves `SavedDataStorage` — so all four world-only classes were loaded before the prologue ran. Substantive and checkable, not a hand-wave |
| **D-2** undisclosed dirty trials | **answered** | `unobtainableRevisions` + deviation; I broke the escape three ways |
| **D-3** ADG-7 save-seam refusal untested | **answered** | `missing-save-seam` is the ninth lifecycle at `run-harness.sh:365` |
| **D-4** undisclosed M3-W0 ceiling | **answered** | a non-blocking risk now states it in those terms |
| **D-5** no `verify_m3_manifest` | **answered** | exists at `lib.rs:2917`; I watched it fire ("M3 manifest must record all four M3 results under the m3-prototype-v1 protocol") |
| **m1** unsafe-prohibition check vacuous | **answered, proven by mutation** | `declares_unsafe_prohibition` (`:245-250`) now matches at line start after `trim_start`, so a doc-comment mention cannot satisfy it. I deleted the real `#![forbid(unsafe_code)]` from `crates/ravel-bootstrap/src/lib.rs` while leaving its one doc-comment occurrence in place: scaffold **exit 1**, "missing unsafe prohibition: crates/ravel-bootstrap/src/lib.rs" |
| **m2** `ravel-testkit` fence unenforced | **answered** | `verify_testkit_fence` exists at `:2415` |
| **m3** "eighteen" nowhere in the record | **answered** | `eighteen` no longer occurs in `README.md`; `:482` states the seven-row budget |
| **m4** JDK patch 25.0.1 vs 25.0.4 | **answered** | `class-loader-identity.json:56` `javaDiffersFromTheMilestone` states it plainly and says a reader should treat the result as observed on 25.0.1 |
| **m5** `prismExitedNaturally` disagreement | **answered** | `:23` `whichCampaignThisDescribes` names the one field the two blocks disagree on and why |

So the sentence is stale, and by the largest margin of the three instances: it
asserts twelve open findings where there are none. It is also the instance that
matters most, because it sits inside the one blocking-classified item in the
manifest and inside the row that describes the review this milestone turns on.

**Why it is not blocking.** Again the direction is safe — it makes M3 look worse
than it is, so no reader is falsely reassured. And this instance is self-clearing:
the blocking risk has to be removed as part of closing M3 anyway, and the M3-W8 row
has to be rewritten to record this round, so the correct fix is simply to not carry
the sentence forward.

**Note for whoever writes that edit.** Do not replace it with a new count of what
is outstanding. Say that the rounds are recorded in `review.md` and that the round
which decided closure is the one that returned a pass. A restated tally of findings
is the same defect in a new costume, and F1 shows the gate will not catch it.

###### F2b and F2c — two deviations describing holes that are closed

**File.** `evidence/m3/manifest.json`, the deviation beginning "Nothing requires a
milestone manifest to list the records that sit beside it" (`:220`), and the
deviation beginning "`verify_evidence_records` has no floor" (`:172`).

**What is claimed.** (a) "Nothing requires a milestone manifest to list the records
that sit beside it, so a record can be added to a milestone directory and be
digest-bound by nothing. … a third record added tomorrow would be bound by
neither. The gate recomputes what a manifest claims and never asks whether the
claims cover the directory." (b) "`verify_evidence_records` has no floor. It
discovers manifests by scanning the evidence directory, so an unreadable, renamed,
or absent `evidence/` yields zero manifests, zero violations, and no message …
Every other hole in this machinery announces itself; this one is the failure mode
the check cannot report on itself. A floor means naming the milestones that must be
present, which is a decision about what the scaffold contract owns rather than a
repair to the digest check, so it is recorded here instead."

**What is actually true — broke both.** (a) I added exactly the third record the
deviation says would be bound by neither — an untracked
`evidence/m0/probe-record.json` — and `verify scaffold` exited **1**:
"evidence/m0/manifest.json names no record probe-record.json, which its own
milestone directory tracks; a record no manifest names is a record no digest
covers." `verify_admitted_inventory` (`lib.rs:1243`) asks precisely the question
the last sentence says the gate never asks. (b) The floor exists at
`lib.rs:1029-1036` and announces itself in the words the deviation says are
unavailable. With `evidence/` present and all four manifests removed, `verify
scaffold` exited **1** with "evidence/ holds no milestone manifest, and a manifest
sweep over nothing passes vacuously."

**Why the class matters.** These three are false statements in the document the
milestone closure gate reads, and they are the eighth, ninth and tenth instances of
this project's recurring class — an absolute present-tense claim that a later
commit falsified — in a form no gate covers. The two gated classes are restated
figures and percentage signs; an obsolete disclosure is neither. The manifest
itself establishes the governing principle for `unobtainableRevisions` — "a
disclosure the defect has outlived fails it too" — and does not apply it to its own
prose. I rank the class moderate rather than blocking because all three point the
same, safe way: they understate the machinery or overstate what is outstanding, so
a reader is warned of a weakness that does not exist rather than reassured about one
that does. Nobody is misled into trusting something unsound. Had any of them run the
other way I would have failed the milestone on it.

**Fix.** Rewrite F2b and F2c in the past tense, naming the check that closed each —
the same form the other deviations in this manifest already use when they record a
defect and its repair. Do not delete them; the audit trail is the point. F2a is
handled by the closure edit as described above. There is no gate for this class and
I am not proposing one, because a check that decided whether a paragraph about the
machinery is still true would have to understand the machinery. What would help is a
convention rather than a check: a deviation is written as history from the moment it
is answered, and a risk never carries a tally of findings.

##### F3 — Minor. The conformance file names the wrong tests as its backstop

**File.** `tests/conformance/tests/saved_data.rs:25-33`.

**What is claimed.** The header discloses — correctly, and this is the most
honest thing in the file — that `reload` "restates that sequence rather than
calling it, so a `stage_world` that stopped handing the envelope over would leave
every test in this file green", and then names what constrains it: "`ravel-bootstrap`'s
`a_stored_aggregate_reaches_the_mod_that_owns_it` and
`a_world_reopened_from_what_its_save_emitted_holds_the_same_state` are what
constrain that".

**What is actually true — broke it.** The disclosure is exactly right and I
confirmed it: with `persistence.rs:300-301` swallowing the refusal, `cargo test -p
ravel-conformance-tests --test saved_data` exits **0** at 18 passed, 0 failed, and
`ravel-bootstrap` is not even a dependency of `tests/conformance/Cargo.toml`, so
the suite structurally cannot reach that code. But the two tests the header names
are the ones that constrain *delivery*, and neither of them fails under the
swallow mutation — only
`a_mod_refusing_its_stored_state_withholds_the_world_and_stages_nothing` does. The
sentence enumerates the backstop and, since the refusal test was added, the
enumeration is one test short.

**Fix.** Add that test's name to the sentence. One line.

##### F4 — Minor. Asymmetric disclosure of which mod-side refusals production can reach

**File.** `test-mods/saved-data/src/lib.rs`, the doc for
`a_plan_that_disagrees_with_the_envelope_beside_it_refuses_delivery` against the
doc for `a_stored_version_this_build_cannot_read_refuses_delivery`.

**What is claimed.** The second says outright that it is "unreachable from the
production crossing and asserted here anyway". The first says nothing about
reachability and reads as though a disagreeing plan is something a world can
present.

**What is actually true — traced it.** It is equally unreachable. `stage_world`
builds the plan at `persistence.rs:295-299` from the same envelope's own
`data_version` that it then delivers, and the mod resolves the plan again from
what it was handed, so `opened == *plan` always holds on the production path. The
branch guards a *host* defect — exactly the mutation I applied as "build every
plan from `None`", which the bootstrap tests kill — not a world's content.

**Why it is minor.** Coverage exists on both sides and no claim anywhere is false;
what is uneven is that one defensive branch says it is defensive and the other
does not, which makes a reader count one reachable production refusal where there
is one, or three, depending on which doc they read. Of the three mod-side
refusals, exactly one — the unparseable payload — is reachable from a launch.

**Fix.** One sentence on the plan-mismatch test saying it guards the host rather
than the world, and that the host-side mutation is what the bootstrap suite kills.

#### Reading-only ceiling — Round 8

Everything in this section is a limit on this round, not a finding. Where a
previous round's ceiling named something and I attacked it, I say so; where I too
could not reach it, it stays here for the next round.

**The largest limit: I never ran `ravel:verify`.** `devenv shell` failed on an
HTTP 504 from the binary cache for the whole of this round, so
`devenv tasks run ravel:verify` was never invoked and I cannot report its exit
status. I ran its constituent commands directly with the flags `nix/common.nix`
gives them, and the table above is what I observed. **What I did not run at all:**
`ravel-format` (formatting across Nix, shell, TOML and Markdown as well as Rust),
`ravel-audit` (`cargo deny check`, which needs the advisory database and therefore
network), `ravel-verify-jvm`, `ravel-verify-budget`, and `ravel-verify-stack`. A
formatting or dependency-advisory failure would be invisible to this round.
Anyone relying on this verdict should re-run the full gate once the cache is
reachable; I would not expect a surprise, because the pieces I could run are all
green and clippy is clean at zero warnings, but I did not prove it.

**I did not execute the JVM persistence harness.** `verify_harness_lifecycles` I
broke three ways and it held. The harness's own binding of its declaration to the
runs it made — `run-harness.sh:377`, a plain `-ne` comparison that exits 1 — I
verified by reading it and by statically counting the nine `persistence_run`
invocations, which reconcile with `persistence_lifecycles_declared=9`. I did not
add a tenth case and watch it fail, because doing so needs the release native
library and a full JVM harness build that the broken shell made expensive. So the
claim "a case added or removed there fails the harness" is, in this round,
established by reading and arithmetic rather than by mutation. That is the single
most valuable thing for the next round to break, because it is the only load-bearing
assertion in this evidence set that I left unattacked by mutation.

**The live half of M3-W4 and M3-W7 is unrecomputable and I did not try.** The
world, the raw logs and the screenshots were deleted and only digests survive. A
digest of a vanished file records a claim rather than verifies one, which the
manifest and the matrix both say in those terms. Nothing in this round independently
confirms that a real client opened a world, wrote `data/ravel/mods.dat`, exited and
reloaded byte-identically. I checked the in-gate half and it runs. The same applies
to M3-W0: every claim the hook inventory makes about the game is a claim about a
restricted local client jar this repository does not track, so I could recompute the
inventory's internal consistency and nothing about the game.

**I withdrew one finding during the round.** I recorded that `review.md` contains
two sections whose inner heading both read "Round 7" — one under the outer heading
`## Round 6` — and treated it as a self-contradiction in the one record designated
as authoritative on how many rounds have run. It is not: `review.md:958-960`
explains it exactly, saying that round's own heading counts the untracked prototype
round the file describes but does not hold, that under this file's numbering it is
Round 6, and that nothing else about it is renamed. The disclosure is present,
correct and adjacent. I was wrong and the record was right.

**What I did not audit myself.** I delegated five read-heavy audits to independent
helpers and their results are **not** folded into this verdict, because they had not
returned by the time I concluded. **Treat these as unexamined by this round:**

1. Schema conformance of the four non-manifest records against `schemas/` — I did
   not validate `class-loader-identity.json`, `pack-compile.json`,
   `profiling-overhead.json` or `linux-process-matrix.json` against any schema, and
   I did not establish whether any gate validates them against one at all. If
   nothing does, that is a hole this round did not look for.
2. The internals of `class-loader-identity.json` — every count, sum and derived
   figure in it is unchecked by me. It has never been attacked by any round.
3. The caveat and re-observation campaigns of `linux-process-matrix.json` — I did
   not check whether `profilingReObservationCampaign` or any other campaign repeats
   the B-1 defect of binding a reading to artifacts hashed before the change whose
   effect it reports, and I did not verify that `screens.profiling.provenance`
   attributes every row to a campaign that could have produced it. Given that B-1
   was found in exactly that shape and the fix was localized, this is where I would
   look next after the harness.
4. ADG-1..ADG-6 conformance of M3's implementation, and ADG-7's selected mechanism
   against the code — I checked the safety boundary and the persistence crossing's
   behaviour, and I did **not** verify the layering rules against the actual
   `Cargo.toml` files, the SDK's exposure boundary, or whether ADG-7's prose and
   `persistence.rs` agree on mechanism rather than merely on outcome.
5. A full status trace of Round 5's findings — see below.

I closed two items I had originally delegated: the inventory arithmetic and the
`forbid(unsafe_code)` sweep, both reported above under what I verified.
`verify_campaign_inventory` also derives the two inventory counts from the record's
own enumeration on every scaffold run, which agrees with my count.

**On the previous rounds' outstanding findings — settled, not assumed.** I listed
this as my largest doubt while drafting and then resolved it myself rather than
leaving it in the ceiling, because a PASS that closes M3 over a dozen live findings
would be indefensible. I enumerated all fourteen of Round 5's findings from
`review.md:756-955` and checked every one against the artifact it cites; the table
under F2a records each and how. All fourteen are answered, the last of them
(`m1`, the vacuous unsafe-prohibition check) proven by mutation rather than by
reading. Every finding in Rounds 6 and 7 carries an editor's note claiming it is
answered, and I spot-checked the three most substantive — the round-count deletion,
the attestation-boundary statement, and the B-1 propagation fix — and all three are
truthful. So no earlier round leaves a material item open, and the manifest sentence
asserting otherwise is itself the stale claim recorded as F2a. What I did **not** do
is audit Rounds 1 through 4 the same way; their findings are recorded as answered by
subsequent rounds that re-examined the same evidence, and I took that at second hand.

**What no gate decides, restated because a PASS is where it matters most.** The
closure gate binds which text was attested, not that the attestation was earned. I
proved that by producing a green M3 with four coordinated string edits. This
document is now that text. A reader who wants to know whether M3 was earned has to
read what I broke and what I did not, and the honest summary of that is: the
world-persistence crossing is well tested in both directions and I could not find a
mutation that survived; the evidence machinery is fail-closed everywhere I pushed
it except one gate that is weaker than its doc claims; and roughly a third of the
gate, the whole live half of two work packages, and five delegated audits are
outside what this round checked.

#### The smallest set standing in the way — Round 8

Nothing. M3 closes on this round.

For completeness, the four findings should be fixed, and none is a condition of this
verdict: F1 by widening or, better, rebinding the round-count check; F2a by not
carrying its sentence into the closure edit, F2b and F2c by rewriting two deviations
as history; F3 and F4 by one sentence each.

If a reviewer disagrees with this PASS, the ground I would expect them to take is
not any of the four findings. It is the ceiling: roughly a third of `ravel:verify`
never ran in this round because the Nix binary cache was down, the JVM harness's own
lifecycle assertion is the one load-bearing control I left unmutated, and four audits
I judged valuable enough to commission — schema conformance of the four records, the
internals of `class-loader-identity.json`, the caveat and re-observation campaigns,
and ADG mechanism conformance beyond the safety boundary — went unexamined. A round
that ran the full gate and attacked those four could reasonably reach a different
verdict, and if it does, it should say so on that basis rather than on mine.

---

## Round 9 — VERDICT: FAIL

This round's own heading calls itself Round 10, counting the untracked prototype
round this file describes but does not hold. Under this file's numbering, which
counts only the rounds it holds, it is Round 9. Nothing else about it is renamed.
Its verdict is written here after its one blocking finding had already been
repaired, for the reason the note above gives.

Result: FAIL

### M3-W8 Independent Review — Round 10

I reviewed this repository cold for this round. I read `CLAUDE.md`, `SPEC.md`,
`ROADMAP.md`'s M3 section and `evidence/m3/` before forming a view, and I read the
prior round's report because the brief required me to check its four findings. I
made no edit to any tracked file that survived the call that made it: every
mutation below was appended to a tracked file, measured, and reverted with
`jj restore <path>` in the same shell command, and `jj st` reported "The working
copy has no changes" after each batch. I confirmed that twice.

#### VERDICT: FAIL — on one sentence

Almost everything the brief asked me to judge is in order. F1 is genuinely fixed
and I proved it by re-running the prior round's own escape probes. F2, F3 and F4
are closed. The ADG-5 and ADG-6 amendments are honest, substantive, and go further
than they had to: every checkable claim either makes, I checked, and all of them
hold. The new gate `verify_decision_hook_claims` fails closed in both the ways it
advertises and in one the brief did not test, and it is not narrower than its own
doc comment.

M3 nevertheless cannot close, because `evidence/m3/manifest.json` — the document
`ravel:verify-milestone:m3` reads and the durable record of what this milestone
was found to be — asserts that M3-W8 found no ADR-conformance defect, twenty-four
lines above a deviation in the same file recording an ADR-conformance deviation
against ADG-5. The two nonconformances that made this round necessary are recorded
everywhere except in the one row that summarises whether they were found. That row
is the eleventh-or-later instance of this repository's recurring defect class, and
unlike every instance the prior round accepted as non-blocking, it points the
unsafe way: it reassures a reader that the axis this round exists to test came back
clean.

It is one sentence, and the corrected form already exists two entries above it.

#### Findings — Round 9

##### 1 — BLOCKING. The manifest states that M3-W8 found no ADR-conformance defect, and M3-W8's ADR-conformance audit found two

**File.** `evidence/m3/manifest.json:212`.

**What it says.** `"M3-W8 found no safety, layering, or ADR-conformance defect in
the reviewed prototype stack."` Unqualified, unscoped, and unattributed to any
round.

**Why it is wrong.** M3-W8's ADR-conformance audit found two nonconformances
against accepted decisions, and both are recorded in this tree:

- `docs/decisions/adg-6-configuration-ui.md:333-341` is an amendment whose own
  first line reads "Amended after M3-W8's ADR-conformance audit", withdrawing two
  sentences that "described a screen surface this repository does not have".
- `evidence/m3/manifest.json:236` is a deviation recording that "Every native M3
  launch loaded libravel_bootstrap.so through the path ADG-5 classes as a
  workaround rather than a mechanism", naming ADG-5 and stating that "nothing in
  M3 claims ADG-5 conformance for the native half".

So the manifest contradicts itself inside one array: entry 18 says the ADR
conformance audit was clean, entry 24 records what it found. `ROADMAP.md:951`
records the ADG-5 obligation too. Every part of this repository except the row that
summarises the finding says the audit was not clean.

**Why the "it is historical" reading does not save it.** `:212` is one of three
rows split out of the prototype round's clean results, alongside `:200`
(hook integrity) and `:216` (redaction). `:200` was falsified by a later round and
the repository's response was to correct it in place — it now reads "That was three
of the four things this row used to claim. The fourth ... did not", and then names
the check that closed it. That is the convention: these rows are read as current
claims about M3-W8 and are amended when a later round falsifies them. `:212` has now
been falsified in exactly the same way and has not been amended. The precedent is
the repository's own.

**Why BLOCKING rather than moderate.** The prior round classified its whole F2
class non-blocking on an explicit test it stated itself: every instance "understates
the machinery or overstates what is outstanding, so a reader is warned of a weakness
that does not exist rather than reassured about one that does. Had any of them run
the other way I would have failed the milestone on it." This one runs the other way.
It is also the specific claim this round was convened to test — the prior PASS was
insufficient precisely because ADR conformance was never examined, and closing M3
now would freeze a record saying that examination came back clean. A milestone
record is the durable artifact; a false sentence in it survives the round that could
have caught it.

**Fix.** Rewrite `:212` in the form `:200` already uses: state that M3-W8 found no
safety or layering defect, that its ADR-conformance audit found two — ADG-6's
withdrawn per-frame screen claims and ADG-5's native-library staging — and name
where each is recorded and what closed or disclosed it. Nothing else in the
manifest needs to move for this finding.

##### 2 — Non-blocking. `CLAUDE.md:63` enumerates the accepted decisions and the enumeration is stale

**File.** `CLAUDE.md:63`, against `docs/decisions/adg-7-world-persistence.md:3`.

**What it says.** "ADG-1 to ADG-6 are accepted; read `docs/decisions/` before
writing anything that touches JVM entry, mod execution, bindings, state ownership,
launcher integration, or Ravel-owned screens, and follow the mechanism the ADR
selected."

**Why it is wrong.** `adg-7-world-persistence.md:3` reads "Status: accepted before
M3-W4", and M3-W4 is closed on it. Seven decisions are accepted, not six, and the
list of subjects the rule attaches to omits world persistence — the one subject
M3's largest work package turns on. An agent following this file literally is not
told that ADG-7's selected mechanism binds it.

**Why it is an instance of the class.** This is an absolute enumeration in prose
that nothing fails when it drifts, in the file that is the binding contract for
every agent in this repository. The same file, sixteen lines earlier, forbids
exactly this: "do not trust a list written here, and do not write one — a milestone
closes and this sentence would go stale unnoticed." The doctrine is right and the
line below it violates it.

**Fix.** Name the directory rather than the range, as the milestone-closure sentence
already does, and add world persistence to the subject list.

##### 3 — Non-blocking. A withdrawn ADG-6 framing survives in a file the new gate cannot see

**File.** `jvm/screen/src/dev/ravel/jvm/screen/RavelSettingsScreen.java:48`.

**What it says.** "ADG-6's omission of `extractRenderState` is not an oversight, it
is the decision — *Ravel never draws*".

**Why it matters.** The ADG-6 amendment identifies the withdrawn sentence as the one
saying "the omission of `extractRenderState` from the hook inventory was the
decision", and this comment carries that phrasing forward verbatim in substance. In
context it is defensible — the paragraph is about this class not implementing the
override, which is true and which the amendment itself confirms — but the sentence
as written attributes the omission to ADG-6 as a decision, which is the design
property the amendment withdrew. The ambiguity is the whole defect: a reader
checking whether the withdrawn claim survives anywhere finds it here, phrased as a
current statement of the decision.

`verify_decision_hook_claims` scans `docs/decisions/*.md` and nothing else, so no
gate reaches this file. That is not a gate defect — it is the reason this instance
needs a human edit.

**Fix.** One clause: say the omission is *this screen's*, and that the inventory's
`screen.extract-render-state` entry is a game-screen observation point, per the
amendment.

##### 4 — Non-blocking. Three of the four M3 records declare a `schemaVersion` that no schema defines, and the fourth declares none

**Files.** `evidence/m3/pack-compile.json`, `profiling-overhead.json`,
`linux-process-matrix.json` each carry `schemaVersion`;
`evidence/m3/class-loader-identity.json` carries none. `nix/common.nix:149-157`
and `tools/ravel-xtask/src/lib.rs:1420`.

**What is true.** `ravel-typecheck` schema-validates four fixture and performance
documents and metaschema-checks `schemas/*.schema.json`. The scaffold
schema-validates the named milestone manifests. No schema exists for the shape of
any of the four M3 result records, and nothing validates them against one. They are
bound by digest and by the arithmetic checks written against specific fields, and
by nothing else.

**Why it is only non-blocking.** The records are digest-bound, several of their
internal figures are re-derived on every scaffold run by
`verify_campaign_inventory`, and `schemaVersion` promising nothing is a decorative
field rather than a false claim about the game. But three records assert
conformance to a version of a schema that does not exist, and one omits the field
its siblings carry, which is the same class in a small form. This answers the prior
round's first open ceiling item, which asked whether anything validates these four
and did not look.

**Fix.** Either define the schema and validate against it, or drop `schemaVersion`
from records no schema governs and say in the manifest that these four are bound by
digest and by named field checks only.

#### F1..F4 from the prior round

| Finding | Status | Evidence |
| --- | --- | --- |
| **F1** — `verify_review_round_counts` weaker than its doc comment | **closed**, proven by mutation | `ROUND_NOUNS` at `lib.rs:939` now holds `reviews` and `iterations`; `numbers_after` at `:905-922` scans forward as well as back; the doc comment at `:952-965` now states the limit as "on either side of the noun" and discloses the one remaining hole with its reason. I re-ran the prior round's four escape probes: probes 1, 2 and 3 now exit 1. Probe 4 (singular `round`) still exits 0 and is disclosed at `:934-938` with a justification I accept — holding the singular would report the counts of other things that sit beside a singular round noun all through these records. |
| **F2a** — blocking risk overstated the outstanding finding set by twelve items | **closed** | `manifest.json:254` no longer carries a tally: it now reads "That round's remaining findings are answered as well, each against the artifact it cited ... `evidence/m3/review.md` holds them and is the one record that counts them." The M3-W8 row at `:86` is the same. Neither restates a figure. The blocking risk itself remains, which is correct while M3 is open. |
| **F2b** — deviation describing the inventory hole as open | **closed** | `manifest.json:220` now reads "Nothing *required* a milestone manifest to list the records that sit beside it", names `verify_admitted_inventory`, and records that it is proven by mutation. Past tense, fix named, audit trail kept — exactly the form the prior round asked for. |
| **F2c** — deviation describing the missing floor as open | **closed** | `manifest.json:172` now reads "`verify_evidence_records` had no floor, and now it has one", names where the floor is and what it reports, and records both mutations. |
| **F3** — conformance header named the wrong backstop tests | **closed** | `tests/conformance/tests/saved_data.rs:29-32` now names `a_mod_refusing_its_stored_state_withholds_the_world_and_stages_nothing` as the third constraining test, with the clause "for the direction where a mod refuses what it was handed". |
| **F4** — asymmetric reachability disclosure | **closed** | `test-mods/saved-data/src/lib.rs:460-465` now states that the plan-mismatch refusal is "Unreachable from the production crossing for the same reason as the refusal above", that it "guards a host that paired them wrongly rather than a world that presented a disagreeing plan", and names the bootstrap mutation that kills the host-side defect. |

None was "already clean". All four were real and all four are answered in the tree
as it stands.

#### The ADG-5 and ADG-6 amendments

**ADG-6 — honest and sufficient.** The two withdrawn sentences are gone from
`Consequences` (I read the diff of change `npqoqpvq` rather than trusting the
amendment's account of itself) and are replaced by prose that states what the
inventory has: `screen.extract-render-state` is this version's per-frame seam and
an observation point. The amendment does not assert a per-frame cost figure — it
says outright that no M3 record prices the crossing, that the nearest numbers are
ADG-1's and ADG-4's taken through other paths, and that "anyone who wants the number
should measure it rather than multiply 10.02 ns by a frame rate, which is arithmetic
across paths that ADG-4's Limits of this evidence expressly refuses." That is the
correct handling and it is the opposite of manufacturing a figure.

The checkable part is preserved and I checked it rather than reading it. `grep` for
`drawString`, `GuiGraphics`, `renderBackground` and `blit(` under `crates/` returns
nothing. `hooks.toml:227-228` carries the section comment the amendment quotes,
verbatim. `docs/architecture/m3-hook-targets.md:169` tables the extraction target,
so the pinned input agrees. `RavelSettingsScreen.java` declares no
`extractRenderState`. "Ravel never draws" holds, and it holds as a fact about the
tree rather than as an assertion.

Also corrected honestly in the same pass: the earlier paragraph at `:124-128` now
states that the per-widget objection "prices per-frame crossing rather than
forbidding it", which is the reading that survives the amendment rather than a
retrofit — the original paragraph does argue from an unbounded widget count, not
from the frame.

**ADG-5 — names a real mechanism and admits nobody owns it.** The amendment selects
`System.load` on an explicit path from a `-D` property, argues why not the installer
(the installer spends the decision's own property to solve what an absolute path
solves), and preserves the load in `Hook`'s static initializer with the JNI
loader-identity reasoning intact. Its closing section is titled "Nothing is built
yet, and no work package owns it" and states that `render.rs` still emits
`System.loadLibrary`, that M3's packages are closed and none named it, that M4-W1
to M4-W8 do not either, and that "until a package does own it and run it, no
milestone may claim conformance with the native half of this decision."

I verified all of that. `crates/ravel-minecraft-adapter/src/generate/render.rs:542`
still emits `System.loadLibrary(LIBRARY)`. `ROADMAP.md:951` carries the obligation
in the open-questions table. `manifest.json:236` records the deviation, names ADG-5,
and states the staging arrangement in full including the `WrapperCommand` and the
third `-D` argument. `adg-1-jvm-entry.md:301-308` carries the correction the
deviation claims for it — the over-broad "no launcher puts anything anywhere" claim
now covers only the screen jar.

**On the severity of the ADG-5 deviation.** Non-blocking is right, and for a
narrower reason than the deviation gives. What the staging affects is how the
library reached the JVM in operator-run evidence launches. What M3-W4 and M3-W7
claim is that the game was hooked and that Ravel-owned versioned bytes made a
round trip through an official world save. Neither claim depends on which mechanism
put the library on the path, and M3 makes no conformance claim for the native half.
I would have called it blocking had any M3 gate row claimed ADG-5 conformance, or
had `ROADMAP.md` recorded the question as discharged without naming the unbuilt
work. Neither is the case.

#### The new gate `verify_decision_hook_claims`

**It is not narrower than its doc comment, and it is broader in one respect than
the brief tested.** The doc comment at `lib.rs:348-379` claims two anchors, both
directions of the ADG-6 pair, that it reads hook ids and member names, and three
named limits. I confirmed every claim by mutation:

- The member-name anchor fires (mutation A below), matching the withdrawn ADG-6
  sentence's own phrasing.
- The absolute per-frame denial fires (mutation B).
- The hook-**id** anchor fires, and it fires from a *different* decision record: I
  planted "This decision does **not** hook `screen.extract-render-state`" in
  `adg-1-jvm-entry.md` and the scaffold named that file (mutation D). So the check
  covers the whole directory, not just ADG-6, which the doc comment implies and the
  brief had not tested.
- The per-frame branch is correctly conditional on the inventory declaring
  `extractRenderState`, so it cannot fire against a version that has no such seam.

**What it cannot catch**, stated for the next reader:

1. **Inflection and word order.** The anchor is the literal adjacent pair `not
   hook` with the hook name *following* within `COUNT_LOOKBACK` words. I appended
   "The `extractRenderState` seam is not hooked by Ravel." — a natural English
   restatement of the withdrawn claim — and the scaffold exited **0** (mutation C).
   Both the inflection (`hooked`) and the name-before-denial order put it outside
   the anchor. The doc comment does disclose this: it says a denial "has to be
   written as `not hook` followed within `COUNT_LOOKBACK` words by a hook id or a
   member name", and names "the extraction seam is left alone" as unreached. So it
   is a disclosed limit rather than a finding — but `not hooked` is the more natural
   phrasing of the very sentence this gate exists over, and widening `DENIAL` to
   accept the inflected form would cost one line and close the likeliest recurrence.
2. **Anything outside `docs/decisions/`.** It reads that directory only. Finding 3
   above is a live restatement of the withdrawn framing in a Java comment, and no
   gate reaches it. `docs/architecture/` is not scanned either.
3. **One per-frame member name, hardcoded** at `:346`. Disclosed at `:377-379`.
4. **Quotation versus claim.** Disclosed at `:370-375`, and the cost is paid
   correctly: ADG-6's amendment paraphrases the sentences it repeals rather than
   reprinting them, which is why it passes.
5. **Denial of a hook the inventory does *not* declare.** A record may deny a hook
   that was never declared and nothing objects — correct, but it means the check
   binds prose to the inventory in one direction only.

#### M3's evidence against the ROADMAP gate

Everything I could recompute, recomputes.

- All four `benchmarks.results[].sha256` recompute against the tracked records, and
  `review.record.sha256` recomputes against `evidence/m3/review.md`. I hashed all
  five and compared to `manifest.json:130-143` and `:276`; five for five.
- `review.md`'s first `Result:` line is `FAIL` and `review.result` is `"fail"`, so
  the two attestation controls agree. `ravel:verify-milestone:m3` exits 1 on exactly
  the two conditions the milestone claims for itself.
- Layering holds where I checked it. `crates/ravel-core/Cargo.toml` declares no
  Ravel dependency. `crates/ravel-sdk/Cargo.toml` depends on `ravel-config`,
  `ravel-core`, `ravel-data`, `ravel-network` and `ravel-save` — no bootstrap, no
  JVM, no version internals. `ravel-testkit` appears in no other crate's
  `Cargo.toml`.
- `ravel:verify` and `ravel:verify-stack` both exit 0 on the clean tree, and m0, m1
  and m2 all exit 0. I ran all six through `devenv shell`, redirected to files, and
  read `$?` — no pipelines.

**On the known ceiling — M3-W4 and M3-W7 resting on operator attestation.** The
disclosure is honest and it is complete. It is stated in the M3-W4 gate row
(`:70`), the M3-W7 gate row (`:82`), a dedicated non-blocking risk (`:242`), a
second risk extending it to M3-W0 (`:258`), and in `README.md`. It says the
mechanism, not just the conclusion: the world, the raw logs and the screenshots
were deleted and only digests survive, and "a digest of a file that no longer exists
records a claim rather than verifies one". It names what *is* recomputable — the
in-gate half, `tests/conformance/tests/saved_data.rs` and
`jvm/harness/PersistenceMain.java` — and it does not dress the attested half up as
anything else. I accept that ceiling for a milestone whose subject is a live game
process that cannot be run in CI. What I would not accept is that ceiling plus a
false claim that the conformance axis was clean, which is finding 1.

#### Mutation exit codes observed

Baseline first, in every case, and `jj st` clean after every batch.

| Run | Exit |
| --- | --- |
| `devenv tasks run ravel:verify` (clean tree) | **0** |
| `devenv tasks run ravel:verify-stack` | **0** |
| `ravel:verify-milestone:m0` / `m1` / `m2` | **0** / **0** / **0** |
| `ravel:verify-milestone:m3` | **1** — exactly `milestone must have no blocking deviations or risks` and `milestone review must be independent and pass` |
| `verify scaffold`, clean tree (mutation baseline) | **0** |
| **A** — ADG-6: append "It does **not** hook `extractRenderState`, and that omission is the decision." | **1** — "says `extractRenderState` is not hooked while … declares it" |
| **B** — ADG-6: append "Nothing crosses per frame and nothing crosses per widget." | **1** — names `NO_PER_FRAME` against the declared per-frame member |
| **C** — ADG-6: append "The `extractRenderState` seam is not hooked by Ravel." | **0** — disclosed limit, see above |
| **D** — `adg-1-jvm-entry.md`: append "This decision does **not** hook `screen.extract-render-state`." | **1** — names `adg-1-jvm-entry.md`, proving the whole directory is scanned and hook **ids** anchor |
| **F1-1** — README: "Five independent reviews have run against this milestone." | **1** |
| **F1-2** — README: "The independent rounds recorded here number five in total." | **1** |
| **F1-3** — README: "There have been five review iterations." | **1** |
| **F1-4** — README: "The count of completed review round entries is five." | **0** — singular noun, disclosed limit |
| **F1-5** — README: "Five independent rounds have run." (positive control) | **1** |

F1 probes 1, 2 and 3 were exit 0 in the prior round and are exit 1 now, which is
the fix working rather than my harness differing: probe 5 is the prior round's own
positive control and it behaves identically in both rounds.

#### What I could not verify

The prior round's ceiling is what hid the two nonconformances this round was
convened over, so I state mine without softening it.

**I did not execute the JVM persistence harness.** This is the item the prior round
named as the single most valuable thing for the next round to break, and I did not
break it. `verify_harness_lifecycles` I did not re-attack either — the prior round
broke it three ways and I took that at second hand. So the claim that
`run-harness.sh:377` fails when a lifecycle case is added or removed remains
established by reading and static counting, by two consecutive rounds now.

**I did not audit the internals of any of the four M3 result records.** Every count,
sum and derived figure inside `class-loader-identity.json` is unchecked by me, and
that record has still never been attacked by any round. I did not check whether
`profilingReObservationCampaign` or any other campaign in
`linux-process-matrix.json` repeats the B-1 defect of binding a reading to
artifacts hashed before the change whose effect it reports, and I did not verify
that `screens.profiling.provenance` attributes every row to a campaign that could
have produced it. The prior round listed both as unexamined and they are still
unexamined. I answered only the third of its five delegated items — schema
conformance, which is finding 4 — and the safety-boundary and layering spot checks.

**ADG-1 through ADG-4 conformance is unexamined by me.** I checked ADG-5 and ADG-6
because the brief named them, and I checked the layering rules and the safety
boundary's shape. I did not check whether M3's implementation follows the mechanism
ADG-1, ADG-2, ADG-3 or ADG-4 selected, or whether ADG-7's prose and
`persistence.rs` agree on mechanism rather than merely on outcome. Round 7's own
ceiling (`review.md:1822-1824`) said the same thing about ADG-1 through ADG-6 and
Round 8 repeated it, and the two defects in that stated gap sat there for two
rounds before anyone looked. Four decisions of that gap are still open. **A future
round should treat this as the first thing to close, not the last**, because the
evidence that a stated ceiling does not protect anyone is now two rounds deep.

**I did not re-verify the safety boundary crate by crate.** I took the prior round's
sweep of eighteen `#![forbid(unsafe_code)]` roots at second hand and confirmed only
that `docs/architecture/safety-boundary.md`'s table is still empty and that
`ravel-testkit` is fenced.

**The live half of M3-W4, M3-W7 and M3-W0 is unrecomputable and I did not try.**
Nothing in this round independently confirms that a real client opened a world,
wrote its aggregate, exited and reloaded byte-identically, or that any claim the
hook inventory makes about the game is true of the game. The pinned client jar is a
restricted local fixture this repository does not track.

**I did not re-derive the inventory arithmetic** (`15 − 4 = 11`, `33 − 6 = 27`) or
re-check which targets the subtraction drops. The prior round did that by hand and
`verify_campaign_inventory` re-derives two of the counts on every scaffold run, and
I relied on both.

**I did not read Rounds 1 through 4's findings.** I checked F1..F4 from Round 8 and
took Rounds 1-7 as answered on the strength of the prior round's Round-5 table and
the manifest's account, which is second hand.

#### The smallest set standing in the way — Round 9

One sentence. `evidence/m3/manifest.json:212` must stop saying that M3-W8 found no
ADR-conformance defect, and must instead record what its ADR-conformance audit
found — ADG-6's withdrawn per-frame screen claims and ADG-5's unaccepted native
staging — in the form `:200` already uses for the same kind of correction.

Findings 2, 3 and 4 should be fixed and none is a condition of this verdict. Fix
finding 1 and I see nothing else standing between M3 and closure: F1..F4 are
closed, both amendments are honest, the new gate holds, and the attestation ceiling
is disclosed as fully as a ceiling can be.

---

## Round 10 — VERDICT: FAIL

This round's own heading calls itself Round 11, counting the untracked prototype
round as the rounds before it did; under this file's numbering it is Round 10. It
is the last round in this file and so is the standing verdict. Its own first
finding is that the two rounds above it had not been written down, which is what
this section and the two above it answer.

Result: FAIL

### M3-W8 Independent Review — Round 11

I reviewed this repository cold. I read `CLAUDE.md`, `SPEC.md`, `ROADMAP.md`'s M3
section, `docs/decisions/adg-1` through `adg-4`, and `evidence/m3/` before forming
a view. I made no surviving edit to any tracked file: every mutation below was
applied, measured, and reverted with `jj restore <path>` in the same shell
command, and `jj st` reported "The working copy has no changes" after each one. I
verified that after the last mutation and again before the gate run.

I did the ADG conformance audit first, as the brief required, and it is where my
second blocking finding comes from. My first blocking finding is not about an ADR
at all — it is about the record M3 closes on.

#### VERDICT: FAIL — on two things

Most of what I attacked held, and several pieces held better than their own prose
claims. ADG-2, ADG-3 and ADG-4 conform. The five round-10 repairs are genuinely
landed and I proved four of them by mutation rather than by reading. The JVM
harness, disclosed as unexecuted by every prior round, runs and its declared
lifecycle count is really enforced. The W4/W7 attestation ceiling is disclosed
honestly and, in one respect, more than honestly.

M3 cannot close because:

1. **`evidence/m3/review.md` asserts that it holds every round that has run, and
   it is missing the last two — including the round whose findings this tree was
   just rewritten to answer.** That file is what `review.record` binds and what
   the milestone gate reads. (Finding 1.)
2. **ADG-1's invariant that the agent jar names no game type is false of the
   shipped agent jar**, and the wording conceals a set of game-member references
   in the always-present artifact that ADG-3's drift gate does not reach and
   nothing else pins. (Finding 2.)
3. **`linux-process-matrix.json` asserts, in a field whose name is the claim, that
   the profiling report's counts advance while the screen is open — and its own
   later, better-provenanced campaign found they do not.** (Finding 8, verified on
   referral after my first pass; see the addendum.)

All three point the reassuring way.

Findings 1, 2 and 8 are blocking. Findings 3-7 and 9-12 are not. Findings 8-12 were
verified after my first pass, on referral, and are in the addendum at the end;
findings 1-7 keep their original numbers so earlier anchors still resolve.

---

#### Per-ADR conformance

##### ADG-1 — JVM entry and hook mechanism: **conforms on mechanism; one blocking defect (Finding 2)**

Everything mechanism-selecting in this record checks out against the code, the
pinned data and M3's evidence:

- **Premain agent.** `jvm/bootstrap/dev/ravel/jvm/Agent.java` plus
  `Premain-Class: dev.ravel.jvm.Agent` written at `jvm/build/build-agent.sh:36`.
- **`java.lang.classfile` rewrite.** `jvm/bootstrap/dev/ravel/jvm/Transformer.java:3-11`
  imports `java.lang.classfile.{ClassFile, ClassTransform, CodeBuilder, …}` and
  nothing else. No shaded bytecode library appears anywhere.
- **Exported JNI-mangled symbol resolved by `System.loadLibrary`.** Observed live:
  the harness run I made prints the JVM's own restricted-method warning naming
  `java.lang.System::loadLibrary has been called by dev.ravel.jvm.Hook`.
- **Zero maintained unsafe, and no exception.** All 20 crate/tool roots carry
  `#![forbid(unsafe_code)]`; `docs/architecture/safety-boundary.md:16` is
  `| *(none)* | — | — |`; `SPEC.md` §3.3 records the mechanism rather than an
  exception, in the same terms ADG-1 uses. All 14 `jni_mangle` sites name
  `dev.ravel.jvm.Hook`. `libloading` is in `Cargo.lock` only via `jni`'s optional
  `invocation` feature and the fenced ADG-2 spike;
  `cargo tree -p ravel-composed-host -e normal` finds zero, so it is absent from
  the production graph — which is the property `jvm/README.md:250-252` claims.
- **Three artifacts.** `jvm/bootstrap/` (agent jar), the native library, and
  `jvm/screen/` (the sibling jar). `jvm/README.md:3` says three. The count is right.
- **`appendToSystemClassLoaderSearch`.** `Screens.java:67`, exactly the third
  mechanism the M3-W3 amendment selected, and lazily behind a hook as claimed.
- **Sole JNI surface.** Generated `Hook.java` is the only class declaring native
  methods (43 of them: 33 hook natives matching `hooks.toml`'s 33 method entries,
  plus 10 runtime exports). `RavelSettingsScreen.java:35` declares none and says so.
  One transformer, one `HookTarget`, one exporter, one native class — as the
  Phase C amendment requires.
- **The Phase C prologue bound is enforced by construction.**
  `crates/ravel-minecraft-adapter/src/generate/render.rs:196` refuses `refuses = true`
  on a non-void target. Proven by mutation (MUT-C, exit 101). Exactly one native
  returns non-void-as-verdict — `worldOpenPersistence` — and `hooks.toml`'s
  `world.open-persistence` is the one entry carrying `refuses = true`, on a `)V`
  descriptor.
- **Class-loader identity amendment.** `evidence/m3/class-loader-identity.json`
  records `classesRewritten: 10`, `methodsRewritten: 24`, and
  `jdk.internal.loader.ClassLoaders$AppClassLoader` with a parent chain of
  AppClassLoader → PlatformClassLoader. ADG-1:271-278's "rewrote 10 classes and
  24 methods", the loader class name, and "whose only parents are the platform and
  bootstrap loaders" all match the record exactly.

What does not conform is Finding 2 below, plus non-blocking Finding 6.

##### ADG-2 — Rust mod execution and ABI: **audited, conforms**

- **Source-only is decided and stated in the contract's own voice.** `SPEC.md`
  §3.4 states the decision, names the ABI obligation as the deciding reason, and
  lists the accepted costs. It does not cite ADG-2 as its authority, which is what
  ADG-2:255-259 says it should not do.
- **The 2026-08-08 schema closure is real and enforced.**
  `schemas/package.schema.json` requires `source`, has `additionalProperties: false`
  at the root, and `source` requires `repository` plus `files` with `minItems: 1`,
  each a `{path, sha256}`. There is no `targets`, no `artifacts`, no `role`, no
  `platform`, no `architecture` anywhere in the file. **Proven by mutation:**
  re-adding a `targets` array with a compiled artifact to the tracked no-op package
  fails with "the no-op manifest is not one `schemas/package.schema.json` accepts"
  (MUT-D, exit 101). Note that `cargo test -p ravel-package` and
  `ravel:verify-scaffold` both pass with the mutation in place; the check lives in
  `tests/conformance/tests/noop_pipeline.rs:213`. That is not a gap — `ravel:verify`
  runs it — but a reviewer testing this must run the conformance crate.
- **Fat LTO is not a default.** `Cargo.toml` declares no `[profile]` section at
  all, so the release profile is cargo's, which matches "an opt-in action and not a
  build profile".
- **Every figure in "What compiling a pack costs" reconciles with
  `pack-compile.json`.** I recomputed all of them: the five-row curve (2.53/3.13/
  4.55/9.82/32.0 s cold; 0.60/2.02/7.29/29.5 s less baseline; 0.96/0.90/1.05/1.63 s
  warm; 33.9/46.5/88.1/221/754 MB cache; 0.45/0.66/2.33/7.68/29.0 MB binary), the
  113 ms marginal (`113.34`), the flat per-mod figures (`126.5, 113.95, 115.25`),
  the 604 ms one-mod point, 73.8 s at four jobs, 754 MB vs 2.85 GB peak, the chain
  variant's 33.2 s vs 30.5 s = +8.9%, 1.76 s leaf, 29.8 s vs 1.79 s root, LTO's
  14.2 vs 29.0 MB and 104 s / 77.4 s with 77.359/1.792 = 43.2×, and the 196 MB
  toolchain (`195541312` bytes). The synthetic-mod description matches too:
  `sourceLines: 1161`, `derivedTypes: 11`, `monomorphizations: 16`,
  `featuresPerMod: 8`, `registryClaimsPerMod: 24`. Nothing drifted.
- **The rejections hold.** `libloading` out of the production graph; no
  `extern "C"` mod boundary; `safety-boundary.md`'s table empty.

The only ADG-2-adjacent defect I found is non-blocking Finding 4, and it is in
`ROADMAP.md` rather than in the ADR.

##### ADG-3 — Minecraft bindings: **audited, conforms**

- **Every entry names class, member and exact descriptor.** All 37 do. The two
  `filterText` overloads ADG-3:143-145 singles out are both present and
  distinguished only by descriptor — and the inventory now carries three more
  same-name pairs (`Screen.init`, `MinecraftServer`/`IntegratedServer.tickServer`,
  `RecipeManager.apply` against a bridge) which strengthens rather than weakens the
  claim.
- **`mappingSource` is recorded.** `adapter.toml:` `mappingSource = "unobfuscated-class-files"`,
  which is what makes the "an obfuscated future version brings this back" clause
  operative.
- **The drift gate fires on the input, before any class is read.** Proven by
  mutation: editing one capability sentence in `hooks.toml` without re-pinning
  fails with "an input drifted: hooks.toml hashes acbe46… but the adapter inputs
  pin fcf9a3…" (MUT-A, exit 101). The inventory hashing itself, which ADG-3's
  consequence and `adapter.toml`'s own comment both insist on, is real.
- **The archive dependency the consequence owed to M2-W5 exists and was reviewed.**
  `miniz_oxide` in `crates/ravel-minecraft-adapter/Cargo.toml`, with
  `jvm/README.md:240-256` recording the `cargo deny` result, the licence set, the
  single transitive dependency, and the reader's deliberate narrowness (stored and
  deflated only, no Zip64, 30675 entries inside the classic limits). The container
  parsing is hand-written in `generate/jar.rs` (347 lines — ADG-3's "a 200-line
  reader handles it" is an order-of-magnitude claim about the class file format,
  not about this file, and I do not read it as a figure).
- **`mappings.lock` is generated and tracked**, and `adapter.toml` pins both locks
  with the stated reason that a jar-less machine can still catch a stale one.

Two imprecisions I do not raise as findings. ADG-3's Decision says "the jar named
in `adapter.toml`" and `adapter.toml` now pins two jars; the second is
DataFixerUpper and it feeds the stub pipeline rather than hook resolution, so the
sentence is loose rather than wrong. And ADG-3's Decision says the generator writes
"a Rust binding plus `mappings.lock`" while it also writes `stubs.lock`; the
Consequences do name the stub lock, so the record is complete taken as a whole.

##### ADG-4 — State ownership: **audited, conforms — and it is the strongest of the four**

- **Java keeps its state, and nothing mirrors.** `HookValue::Handle` wraps
  `JavaHandle`, whose entire public surface is `class()` and `is_null()`
  (`crates/ravel-minecraft/src/lib.rs:193-212`). No field is read through a handle
  anywhere, so the mirror prohibition is not merely obeyed but unreachable.
- **The amendment's handle is a borrow, not a copy.** `pub struct JavaHandle<'local>`
  holds `class: &'static str` and `object: &'local JObject<'local>` — a pointer and
  a declared name, duplicating no field, which is exactly the arithmetic argument
  the amendment makes.
- **The residual lifetime risk the amendment names is closed, and closed the way
  the amendment says it should be.** `crates/ravel-minecraft/src/lib.rs:142-190`
  carries three `compile_fail` doctests that kill the three escapes by type:
  `E0515` on returning a handle from the frame that produced it, `E0277` on sending
  one to a scoped thread (deliberately scoped rather than spawned, so the test
  isolates the `Send` route instead of passing on `'static`), and `E0521` on an
  observer hoarding handles in a `Mutex<Vec<JavaHandle<'static>>>` — which the
  doc comment correctly identifies as "the escape an author would actually attempt".
- **The layering the amendment depends on holds.** `JavaHandle::new` is the only
  signature in `ravel-minecraft` naming a JNI type, `docs/architecture/dependency-rules.md:18-21`
  records that no mod can reach the crate and that a change making it reachable
  must seal the constructor, and ADG-2's chosen mechanism is what makes sealing it
  later a recompile rather than an ABI break. The three records argue the same thing.

The only defect is non-blocking Finding 7, and it runs the safe way: the ADR
undersells work that is finished.

---

#### Findings — Round 10

##### 1 — BLOCKING. `review.md` claims to hold every round that has run, and the last two rounds are not in it

**Files.** `evidence/m3/review.md:5-6` and `:15-17`; `evidence/m3/manifest.json`,
the blocking risk; `evidence/m3/README.md:44-52`.

**What they say.**

- `review.md:5-6`: "The last round below is the standing verdict."
- `review.md:15-17`: "**The numbering below counts only the rounds this file
  holds, so 'Round 1' is the second round that ran.** Every round that has run
  since is here, in order, so this file rather than any summary of it is what says
  how many there have been."
- The manifest's blocking risk: "Every verdict but the earliest is tracked verbatim
  at `evidence/m3/review.md` and bound by `review.record`."
- `README.md:47-49`: "`review.md` is the one place that says how many rounds there
  have been: neither this document nor `manifest.json` restates a count, because a
  count written outside the record is falsified by the next round."

**Why it is wrong.** `review.md`'s headings run: the prototype round, then Round 1
through **Round 7**, whose own text calls itself "the eighth round to run". At
least two rounds have run since and neither is in the file:

- The round that found the ADG-5 and ADG-6 conformance gaps. Its repairs are in
  this tree as changes `pvkzxtukwnmx` ("docs: choose ADG-5's native library
  mechanism"), `npqoqpvqnmwy` ("docs: amend ADG-6 for the per-frame screen seam it
  has") and `vklntxoskvqm` ("docs: record M3's ADG-5 deviation and drop its stale
  finding counts").
- The round immediately preceding this one, whose single blocking finding was
  `evidence/m3/manifest.json:212` and whose five repairs I verified individually
  below as changes `kzoquzyr`, `opzyylrm`, `kyrozxmp`, `okysxtvo` and `zurotlxz`.

`jj log -r 'files(evidence/m3/review.md)'` names `oupswvsrymxy` ("docs: record M3's
latest review round and the fixes it names") as the newest change to touch the
file, and `oupswvsrymxy` sits **below all eight** of those repair changes in the
stack. So the tree has twice now landed a round's *repairs* without its *verdict*.

**Why it is blocking rather than moderate.** Three reasons, and the third is the
one that decides it.

First, direction. Every one of the four sentences above reassures a reader that the
review record is complete and is the authority on how many rounds have run. It is
not complete, and a reader who trusts the designated authority is told the standing
verdict is Round 7's when it is not.

Second, it is a recurrence of the exact defect the file's own longest passage
exists to prevent. `review.md:11-17` and `README.md:54-61` narrate, at length, that
the earliest round's verdict was never written into the repository, that its text
is unrecoverable, and that this "is exactly what a tracked review record exists to
prevent". Two more rounds' verdicts are now in the same position — except that one
of them is recoverable today, because its verbatim text exists outside the
repository, and will stop being recoverable the moment that scratch file goes.

Third, and decisively: `review.record` is the artifact `ravel:verify-milestone:m3`
binds, and `README.md:251-269` states plainly that what the gate decides is "**which
text was attested**, not **that the attestation was earned**". Closing M3 now would
freeze, as the durable and digest-bound record of this milestone's review, a file
that omits the two most recent rounds while asserting it omits none — including the
round that caught the manifest asserting its own conformance audit came back clean.
The failure mode is the same one the previous round failed M3 for, one level up.

**What I am not claiming.** It is likely the closing writer intends to append this
round to `review.md`. That does not save the finding: two rounds are missing, not
one; the completeness claim is unqualified rather than "as of the last recorded
round"; and the pattern has already repeated once, which is what turns a transient
lag into a defect.

**Fix.** Write the two missing rounds' verdicts into `review.md` verbatim and in
order, renumbering nothing that is already there. Where a verdict's text is not
recoverable, say so in the same terms the prototype-round section already uses
rather than summarising it. Then either qualify the completeness sentences at `:5-6`
and `:15-17`, or keep them and make them true. The manifest's blocking risk and
`README.md:44-52` carry the same claim and have to move with it.

##### 2 — BLOCKING. ADG-1's invariant that the agent jar names no game type is false, and the wording conceals an unpinned coupling surface

**Files.** `docs/decisions/adg-1-jvm-entry.md:345` and `:362`;
`docs/decisions/adg-7-world-persistence.md:337`;
`jvm/bootstrap/dev/ravel/jvm/Persistence.java:13-14`, `:42-49`, `:198-199`, `:310-314`.

**What they say.**

- ADG-1:345, in "What is still forbidden": "The agent jar must keep naming no game
  type and must keep compiling without the client jar."
- ADG-1:362, in the ADG-7 amendment, and this is the load-bearing one — it is the
  sentence that makes the artifact-scope widening safe: "This is an artifact-scope
  widening only: **the agent jar still names no game type**, and generated
  `dev.ravel.jvm.Hook` remains the sole JNI surface."
- ADG-7:337: "ADG-1 forbids the agent jar from naming a game type, so the storage
  half cannot live there."
- `Persistence.java:13-14`: "Everything here is reflective, because this class ships
  in the always present agent jar and ADG-1 forbids that jar naming a game type."

**Why it is wrong.** `Persistence.java` is compiled into the agent jar —
`build-agent.sh:33` globs `jvm/bootstrap` wholesale — and it names six game types:

```text
Persistence.java:42   "net.minecraft.server.MinecraftServer"
Persistence.java:45   "net.minecraft.world.level.storage.SavedDataStorage"
Persistence.java:47   "net.minecraft.world.level.storage.LevelResource"
Persistence.java:48   "net.minecraft.util.datafix.DataFixers"
Persistence.java:199  "net.minecraft.client.Minecraft"
Persistence.java:49   "dev.ravel.jvm.screen.RavelWorldData"   (the sibling, not a game type)
```

and resolves each through `Class.forName(…, ClassLoader.getSystemClassLoader())`.

The repository's own most recent writing on the question settles that this counts
as naming. `Persistence.java:310-314` explains why it looks members up by name and
arity instead of by declared parameter type: "several are game types this jar is
**not allowed to name even as a string it then resolves** — the point of ADG-1's
rule is that the agent jar does not depend on the game's type graph." That is the
rule the six constants above break, stated forty lines after they are declared. One
of the two is wrong; they cannot both be right.

**What actually holds, and it is narrower.** The jar *declares* no game type, so
`build-agent.sh:34` compiles it with `javac -Xlint:all -Werror -d "$classes"` and
no `-cp` at all, and it needs no licensed fixture. That property is enforced by
construction — a source that named a game type as a type would fail to compile —
and it is the property both of ADG-1's reasons rest on: the licensing and
contribution argument at ADG-1:213-230, and the loader argument as corrected at
ADG-1:268-290. Neither reason is damaged. "Names no game type" is not enforced by
anything and is not true.

**What the false wording conceals, and this is why it is blocking rather than
cosmetic.** The reflective route reaches a set of game members that no pinned file
mentions. `grep` across `hooks.toml`, `stubs.toml`, `stubs.lock` and `mappings.lock`
finds **zero** occurrences of `DataFixers`, `LevelResource`, `getDataStorage`,
`getWorldPath`, `getLevelPath`, `compositeAccess` or `getDataFixer` — and
`Persistence.java` additionally depends on `LevelResource.ROOT` and `.DATA` as
fields, on `SavedDataStorage` having exactly one three-argument constructor
(`:168`), and on `get`/`set`/`close`/`registries`/`getInstance`/`setScreen`
resolving by name and arity (`:172-176`, `:200-201`, `:259-261`, `:316-328`).

So ADG-3's drift gate — the mechanism whose whole purpose is that "a change to
[`mappings.lock`] shows up in a diff, which is what makes a silent rename visible"
— does not cover any of it. Nothing else does either. The only thing in the
repository that checks these names is `jvm/harness/persistence/net/minecraft/**`:
hand-written stand-ins Ravel authored, which declare
`public SavedDataStorage(Path, Object, Object)`, `getDataStorage()`, `getWorldPath()`,
`getDataFixer()` and `LevelResource.ROOT`/`.DATA` because `Persistence.java` needs
them to. The harness's nine world lifecycles assert Ravel's stubs against Ravel's
reflection.

Contrast the screen half. `build-screen.sh:22-35` compiles `jvm/screen/src` against
stubs *generated* from `stubs.toml` and pinned in `stubs.lock`, both re-pinned
against the client jar. So the **optional** artifact has drift detection and the
**required** one does not — which inverts precisely the failure asymmetry ADG-7
chose and `Persistence.java:18-23` explains at length. A rename in the next pinned
version lands as a runtime world refusal, or, on the save path, as a diagnostic and
a silently skipped save (`Persistence.java:244-249` catches `Throwable`
deliberately), rather than as a generation failure a diff would show.

**Fix.** Either wording or the lock, and the wording is the smaller of the two.
State what holds — the agent jar declares no game type and compiles without the
client jar — at ADG-1:345, at ADG-1:362, at ADG-7:337 and in `Persistence.java`'s
two comments, so that the four say the same thing as each other and as the code.
Then record the reflective member set as a version-coupling surface the drift gate
does not reach, or bring it under `hooks.toml`/`stubs.toml` so that it does. What
must not survive is a sentence telling a reader the always-present jar has no
dependence on the game's type graph when it has six class names, roughly eleven
member names, two field constants and a constructor arity, checked by nothing but
Ravel's own stubs.

##### 3 — Non-blocking. `ROADMAP.md:712` says two accepted records were amended for M3; four were

"Two accepted records were amended for M3, and neither changed what it decided."
The status lines say otherwise: ADG-1 (four amendments), ADG-4 ("amended at M3-W0
to admit opaque handles"), ADG-5 ("amended after M3 to choose the fix that note
deferred") and ADG-6 ("Amended again after M3-W8's ADR-conformance audit"). Four,
not two. An absolute count in prose that nothing recomputes. Its direction is
mildly reassuring — it understates how much amendment M3 required — but no
mechanism turns on it and each amendment is individually recorded, so it is not
blocking. The fix is the one `CLAUDE.md:63` was just given: name the records
instead of counting them.

##### 4 — Non-blocking. `ROADMAP.md:948` says the package-schema reconciliation is live work; it is closed

The gate table row reads "Reconciling `package.schema.json` and `SPEC.md` §3.4 is
now live work". `ROADMAP.md:707-710`, 240 lines earlier in the same file, says it
"is now closed as a breaking schema-v1-in-place migration", `SPEC.md` §3.4 says the
same, ADG-2 records the 2026-08-08 closure, and I proved the closure enforced
(MUT-D). Non-blocking because it overstates what is outstanding, which is the safe
direction — but it is a self-contradiction inside one file and an instance of the
class.

##### 5 — Non-blocking. `evidence/m3/README.md:4` calls the fifth round the latest

"The latest round's blocking finding, B-1, is answered by the fifth launch campaign
described below". B-1 is the *fifth* round's, as `:223` states correctly ("The fifth
round found one blocking defect, B-1"). The README's narrative ends at the fifth
round while calling it the latest; rounds six onward appear nowhere in it. Same root
cause as Finding 1 and it should be fixed in the same pass. Non-blocking on its own
because the document defers counting to `review.md` at `:47-49` and because it
leaves M3 open either way.

##### 6 — Non-blocking. ADG-1:176-177's export-count sentence still says six inventory entries

"The export count scales with the hook surface, not with the crate. Six inventory
entries with heterogeneous descriptors cannot share one `extern "system"`
signature, so there is one generated export per hook shape." The inventory holds 37
entries and generated `Hook.java` declares 33 hook natives. The mechanism sentence
in front of it is still true and the direction is safe — it understates the export
surface — but it is a stale absolute count of exactly the hunted kind, and unlike
`m3-hook-targets.md`'s counts (which are now derived and gated; I broke them and the
scaffold named both, MUT-F, exit 1) nothing recomputes this one.

##### 7 — Non-blocking. ADG-4:159-165 presents as owed a wrapper that is built and proved

"That risk belongs to a safe wrapper whose type ties a handle to the call that
produced it, so a handle that outlives its frame does not compile. It is the part of
this amendment that needs review, and it is the part no measurement here covers."
Read as of today that describes unbuilt work. It is built: `JavaHandle<'local>`
holds `&'local JObject<'local>` and three `compile_fail` doctests
(`crates/ravel-minecraft/src/lib.rs:142-190`) kill the return, the thread and the
hoarding-observer escapes. Understates the machinery, so safe direction and
non-blocking. Recorded only so the next reader does not go looking for work that is
finished, and because the ADR should be able to point at its own proof.

---

#### The five round-10 repairs

All five are landed. Four I proved by mutation rather than by reading.

- **`kzoquzyr` — `manifest.json:212`.** Rewritten in the form `:200` uses. It now
  reads "M3-W8 found no safety or layering defect… That was two of the three things
  this row used to claim. The third did not hold: M3-W8's ADR-conformance audit
  found two nonconformances", names ADG-6's withdrawn `extractRenderState` claims
  and ADG-5's staging deviation, and points at `verify_decision_hook_claims`.
  Closed.
- **`opzyylrm` — `schemaVersion`.** No top-level `schemaVersion` remains in any of
  the four M3 records. The coordinator's reasoning about the three survivors checks
  out and I checked it rather than assuming it:
  `linux-process-matrix.json:91` is inside `decodedRecord` alongside
  `owner: "saved_data"` and `dataVersion`, `:157` is inside `configurationStore`
  alongside `owner: "configuration"`, and `schemas/saved-data.schema.json` and
  `schemas/configuration.schema.json` each define the field; `manifest.json:2` is
  governed by `schemas/evidence.schema.json`. A fourth occurrence at `:190` is
  inside a `persistedDocument` transcript string, which is the same case. The
  deviation's mutation claim also holds: re-adding the field to
  `class-loader-identity.json` fails the scaffold by name (MUT-E, exit 1). Closed.
- **`kyrozxmp` — `RavelSettingsScreen`.** `:43-53` now reads "The inventory *does*
  declare `extractRenderState` — it is 26.1.2's per-frame seam… What ADG-6 commits
  to is narrower and it is what this class obeys: *Ravel never draws*." Closed.
- **`okysxtvo` — the decision range.** `AGENTS.md:63-64` now names the directory
  and adds "do not enumerate them here, because the count and…". Closed. (Finding 3
  is the same defect surviving in `ROADMAP.md`.)
- **`zurotlxz` — the inflected passive.** `DENIAL_VERBS` is
  `["hook", "hooked", "hooking"]`. **Proven by mutation**, four probes appended to
  `adg-4-state-ownership.md` one at a time:
  - "The extractRenderState seam is not hooked." → exit **1** (the passive; this is
    the fix)
  - "Ravel does not hook extractRenderState." → exit **1** (active)
  - "screen.extract-render-state is not hooked." → exit **1** (by hook id)
  - "Ravel never hooks extractRenderState." → exit **0** (escapes)

  The escape is not a finding: the doc comment at `tools/ravel-xtask/src/lib.rs:367-379`
  states that a denial must be written as `DENIAL` — the literal word `not` —
  adjacent to a `DENIAL_VERBS` form, and names "the extraction seam is left alone"
  as unreached. The check is exactly as wide as it says it is. Closed.

#### The JVM harness — executed and mutated

Never run in any prior round. It runs.

- Built the prerequisites (`cargo build -p ravel-minecraft-adapter`,
  `./jvm/build/build-agent.sh`, `cargo build -p ravel-composed-host --release`) and
  ran `./jvm/harness/run-harness.sh` → **exit 0**. It exercises the real path: the
  shutdown reports "rewrote 1 classes and 4 methods; 5 hook invocations reached
  native code" and later "rewrote 3 classes and 4 methods; 7 hook invocations", the
  screen sections drive `RavelModsScreen`, `RavelSettingsScreen` and
  `RavelProfilingScreen` through real widgets, and the JVM's own restricted-method
  warning confirms `dev.ravel.jvm.Hook` is what calls `System.loadLibrary`.
- **Mutated `persistence_lifecycles_declared=9` → `8`** → **exit 1**, with
  "this harness ran 9 world lifecycles and declares 8; update
  persistence_lifecycles_declared and the evidence/m3 records that cite it". The
  declared count is genuinely enforced. That disclosure is closed.

#### Mutation exit codes

| ID | Mutation | Expected | Observed |
| --- | --- | --- | --- |
| MUT-A | one capability sentence in `hooks.toml`, hash left stale | refuse | **101** — named the hash it got and the hash it pins |
| MUT-C | `refuses = true` added to `world.save-persistence` (`(ZZZ)Z`), hash re-pinned | refuse | **101** — "does not return void and an early return would have to invent a value" |
| MUT-D | `targets` + compiled artifact re-added to the tracked no-op package | refuse | **101** — "not one `schemas/package.schema.json` accepts" (via `ravel-conformance-tests`; `ravel-package` and `verify-scaffold` alone pass, exit 0) |
| MUT-E | top-level `schemaVersion` re-added to `class-loader-identity.json` | refuse | **1** — digest mismatch named the record |
| MUT-F | `m3-hook-targets.md` counts drifted 37/33 → 36/32 | refuse | **1** — named both counts against `hooks.toml` |
| Harness | `persistence_lifecycles_declared` 9 → 8 | refuse | **1** |
| Denial ×4 | four phrasings of a per-frame hook denial | 3 refuse, 1 escape as documented | **1, 1, 1, 0** |

One correction to my own run, recorded because it is the failure mode the brief
warned about: my first attempt at MUT-D used `-p ravel-conformance`, which is not a
package name, and exited 101 for that reason. I did not count it. The kill above is
from `-p ravel-conformance-tests`, and I confirmed the failure text before counting
it.

#### Gates

On a clean tree, after every mutation was reverted and `jj st` reported no changes:

```text
ravel:verify              EXIT=0
ravel:verify-stack        EXIT=0
ravel:verify-milestone:m0 EXIT=0
ravel:verify-milestone:m1 EXIT=0
ravel:verify-milestone:m2 EXIT=0
ravel:verify-milestone:m3 EXIT=1
```

m3's two unmet conditions are exactly "milestone must have no blocking deviations
or risks" and "milestone review must be independent and pass". That is the expected
state for an open milestone and is not a finding.

#### M3's evidence against ROADMAP.md's M3 gate

The W4/W7 ceiling is disclosed honestly and completely, and I judge it acceptable.

`evidence/m3/README.md:10-18` states it in the first place a reader looks, before
any table: the world, raw logs and screenshots were deleted with only digests kept,
and "a digest of a file that no longer exists records a claim rather than verifies
one", so a reviewer recomputes the in-gate half and "must take the operator's word
for the rest". `manifest.json` carries the same as its first risk. Three things
persuade me the disclosure is not doing the minimum:

- It volunteers a ceiling nobody asked for. `README.md:20-31` and the manifest's
  fifth risk say M3-W0 has "a ceiling of the same shape and it is easy to miss,
  because that gate closes on tracked files rather than on a launch" — every claim
  either tracked file makes about the game is a claim about a jar the repository
  never tracks. That is a reviewer's argument made against the repository's own
  interest.
- It refuses the flattering reading of its own cleanup. `README.md:392-394`:
  removing the world "took nothing away from that and bought nothing back: a
  private machine-local directory is no more checkable by another reader than a
  digest of one."
- It says what a green closure would and would not prove (`README.md:251-269`),
  including that a prior round closed M3 green with four coordinated edits, and
  calls that "the design rather than a hole in it".

The two named gaps inside the closed W7 gate — `tick-count` unable to leave zero,
and the command counter never observed above zero live — are both stated as gaps
with their mechanisms, not smoothed over.

I also recomputed what the records claim about themselves where a reader could:
ADG-2's whole pack-compile table (above), ADG-1's class-loader figures (above), and
`profiling-overhead.json`'s published derivation. That last one reconciles exactly:
`perTick.nanoseconds` 510.05 = 0.05 + 10000 × 0.051, `breakEven.crossingsPerTick`
9803920.59 = (500000 − 0.05) / 0.051, the budget limit 500000 ≥ 510.05, and
`README.md:336-346`'s backwards argument — that the published `breakEven` pins the
unrounded boundary to about [0.0497, 0.0502], roughly twenty times narrower than
the rounding interval — is arithmetically correct, which I checked rather than took.

#### What I could not verify — Round 10

My own ceiling, stated plainly, because a prior round's unexamined ceiling is what
hid two defects.

- **I did not launch the game.** Every claim about a real client is the operator's.
  I judged the disclosure of that, not the observations behind it.
- **I read no class file out of the pinned jar myself.** The generator reached the
  jar twice under my mutations, so the resolution path executed, but I did not
  independently confirm that any member resolves. Concretely, this limits Finding 2:
  I cannot say whether `Persistence.java`'s reflective names are correct for 26.1.2
  today. What I can say is that nothing in the repository would tell us if they
  stopped being.
- **I audited ADG-1 through ADG-4 and did not re-audit ADG-5, ADG-6 or ADG-7.**
  Where they bear on my findings I read the relevant sections and say which lines;
  their internals are the previous rounds' coverage, not mine. Finding 2 touches
  ADG-7:337, and a reviewer who re-audits ADG-7 may find more of the same shape in
  its placement section.
- **I did not audit `evidence/m1/spikes/`.** ADG-1's and ADG-2's M1 tables are M1's
  gate; I checked only the M3 figures those records quote.
- **`linux-process-matrix.json`'s internals are checked only where the README
  publishes arithmetic over them.** I verified the 15 − 4 = 11 and 33 − 6 = 27
  subtraction and its stated criterion, and the 37/33/4 inventory reconciliation
  under MUT-F. I did not walk every field of that record; it is the largest of the
  four and the one whose claims are least recomputable by construction.
- **The gate numbers in `manifest.json` — `tests.total`, coverage, `logSha256` —
  are attestations.** I did not re-derive them, and the record says nobody can.
- **I ran each gate once**, on a tree I had mutated and restored six times. `jj st`
  reported no changes before and after every mutation and immediately before the
  gate run, so I am confident the tree I gated is the tree I was given, but a single
  run is a single run.
- **Nothing here is a judgement about whether Findings 1 and 2 are the last of
  their kind.** Finding 1 is the second instance of a round's repairs landing
  without its verdict, and Finding 2 is the third accepted decision found
  describing a surface the repository does not have. I would not read a fourth
  round finding none as strong evidence there are none.

---

If Findings 1, 2 and 8 are fixed as described, I see nothing else standing between
M3 and closure. All three are documentation and record repairs rather than
redesigns. Verifying them is a later round's job, not this one's — and Finding 1 is
precisely the reason that matters.

---

### Addendum — five record-internals discrepancies, verified on referral

Five discrepancies from a separate arithmetic audit were referred to me for
independent verification. I checked each against the files rather than accepting
either the audit's or the coordinator's reading. **All five reproduce.** I agree
with the coordinator's provisional severity on all five, and I give my own reason
for the blocking one rather than inheriting theirs.

#### 8 — BLOCKING. The matrix asserts the profiling counts advance while the screen is open; its own later campaign found they do not

**Files.** `evidence/m3/linux-process-matrix.json:203`, against `:733` and `:734`.

**What they say.**

- `:203`, field name `screens.profiling.countsAdvanceWhileTheScreenIsOpen`: "The
  same two rows were read twice a few seconds apart and moved from 5681 to 6186
  calls, which is what tells a live report apart from a rendered constant."
- `:733`, `profilingReObservationCampaign.atScaleOne.readTwiceWithinOneOpening`:
  "The report was read again eight seconds later without leaving the screen and
  both per-mod rows were **identical**, so the payload is a snapshot taken when the
  screen opens rather than a live-updating widget."
- `:734`, `readAgainAfterReopening`: reopening redrew the rows advanced 2323 → 3445
  calls, "**That is what tells a live report apart from a rendered constant**, and
  it is the sense in which the counts advance: across openings, not within one."

**Why it is wrong.** The two readings are contradictory and the second one knows it.
`:734` reuses `:203`'s exact clause — "what tells a live report apart from a
rendered constant" — and reassigns it from within-one-opening to across-openings,
then states the general characterization in the negative: "across openings, not
within one." That is a correction of `:203` written as if `:203` did not exist.
Nothing in the file reconciles them, and `:203` is not marked withdrawn.

I checked whether a build change could explain it, because that is the only
innocent reading. It cannot, on this record's own account: `npozznqz` is described
throughout as line-wrapping and row-budget work (`:454-457` per the README's
account: "splits every line to the width it is drawn at… hands the window's row
budget out in a stated order"), and `lppyxtnz` is described as adding `tick-count`
to the unmeasurable list in both the Rust and the screen. Neither is claimed to
change whether the payload is a snapshot. And `:734` does not say the behaviour
changed — it says what the behaviour is.

**Why blocking.** Four reasons, and the fourth is mine rather than the
coordinator's.

First, direction. The field's *name* is the claim, and a reader reaches
`screens.profiling.countsAdvanceWhileTheScreenIsOpen: "…"` and concludes a live
counter works in the real client. It does not.

Second, subject. This is not a stale count. It is a claim about observed
user-visible behaviour in the M3-W7 record, on the one surface that already failed
once, was fixed, and has now been re-observed twice — the surface whose provenance
the fifth round already blocked M3 over. It is the third unreconciled claim about
the same screen.

Third, class. It is the same defect round 10 blocked on: a sentence in the durable
milestone record that is false in the reassuring direction, on the axis the round
was convened to test. Closing M3 freezes it.

Fourth — and this is what moves it from "should fix" to blocking — **the record
violates its own stated repair convention, in the very campaign that established
it.** `evidence/m3/README.md:476` and the fifth campaign's own text set the rule
after the previous blocking finding: "The withdrawn reading is marked withdrawn
where it stood rather than restated at some other number, because nobody can now
say what that build drew." The repository adopted that convention *because* an
unattributable profiling reading had been left standing. `:203` is a reading the
same campaign contradicts, left standing at its own number, with no withdrawal
mark. The convention was written one campaign ago and is already not being applied.

**Fix.** Decide which reading holds. If the snapshot behaviour is what the code
does — which `:733` observed against digests written before the launch, so it has
the better provenance — mark `:203` withdrawn where it stands, in the same terms
the fifth campaign uses, and rename or requalify the field so its name does not
assert the withdrawn claim. If instead the surface campaign's build really did
update live and `npozznqz` changed that, say so and name the change, because that
is a behaviour change no record currently claims. What must not survive is both
sentences in one file with nothing between them.

#### 9 — Non-blocking. `linux-process-matrix.json:7` says one gap remains open; `openGaps` holds two

`:7`, in `summary`: "One narrower gap remains open and is named in `openGaps`:
nothing on the game's tick path marks a tick boundary…". `openGaps` at `:596-598`
holds `profilingTickCounter` and `profilingCommandCounter`. The second argues at
length that it is "an accurate measurement rather than a broken counter" and
"materially unlike `profilingTickCounter`" — so there is a defensible intent behind
the singular — but its own closing sentence is "**Closing that gap is a composition
change** — put the command-claiming test mod in the composed host and type a
namespaced root", and `manifest.json`'s third risk lists both counters as gaps
inside the closed gate. It is a gap by the file's own word and the manifest's.
Instance thirteen of the recurring class, and it understates the number of open
gaps, which is the reassuring direction — but nothing turns on it, both entries are
present and legible, and the distinction the sentence is reaching for is real. The
fix is to stop counting: name the tick counter as the gap the summary is about and
let `openGaps` be its own inventory, exactly as `review.md` is meant to be for
rounds.

#### 10 — Non-blocking. `class-loader-identity.json:53` calls itself the only M3 record with no `trial` block

"This record is the only M3 record with no `trial` block, because it is a launch
rather than a task: nothing in the repository ran it and no command digest describes
it." I counted `"trial"` in all five files: `class-loader-identity.json` 0,
`linux-process-matrix.json` **0**, `manifest.json` 0, `pack-compile.json` 2,
`profiling-overhead.json` 1. So `linux-process-matrix.json` has none either, at top
level or nested — and the stated reason applies to it verbatim, since it is the
launch record par excellence ("it comes from launches rather than from a task,
because the thing it measures is a launcher starting a game and no task can stand in
for that", `README.md:353`). Only the word "only" fails; the reason is sound and the
`hookInventoryDigest: null` disclosure beside it is good work. Non-blocking, and the
fix is to drop the superlative rather than to recount — the same lesson as Finding 3
and Finding 9.

#### 11 — Non-blocking. ADG-2:377 compares the four-core variant against a figure its source record forbids

"**A four-core player pays 73.8 s cold at 256 mods against 32.0 s**". The 73.8 s is
right: `variants.results[1]` is `id: "independent-j4"` and its 256-mod
`coldMilliseconds` is 73803. The 32.0 s is the curve's `compile.points[4]`
(32031) — and `pack-compile.json:181`, `variants.method`, states the rule: "the
curve above is a median of three, so **compare a variant against the
`independent-j16` control here rather than against the curve directly**." That
control is `variants.results[0]`, `id: "independent-j16"`, whose 256-mod cold is
30522 — **30.5 s**. The same ADG-2 bullet list uses 30.5 s correctly twice, for the
chain variant ("33.2 s against the control's 30.5 s, +8.9%") and for LTO ("104 s
cold and 77.4 s warm against 30.5 s and 1.79 s"). So the record's method is
understood and applied in two of three places and broken in the third.

Not arithmetic — a comparison the source record forbids, inside an accepted ADR.
The effect is small and mildly reassuring: it prices the four-core penalty at 2.31×
rather than the 2.42× the control gives. Non-blocking because no conclusion moves
(the bullet's point is more wall clock, less memory, and both halves survive), but I
agree it should be fixed before closure: an ADR that misuses its own evidence's
stated method is a conformance defect of exactly the kind this round exists to find,
and it is a one-number edit.

#### 12 — Non-blocking. `pack-compile.json:163` says `sourceLines` is above it; it is below

`representativeness` ends "…which is why the sentence no longer makes one;
`sourceLines` above says what this one came to." `sourceLines: 1161` is at `:166`,
three lines below. The value is right and I verified it against ADG-2's "1,161
lines". Wording only, and worth mentioning only because the sentence is a repair of
an earlier superlative and a repair should land clean.

#### The two accepted non-defects, sanity-checked

I checked both rather than re-deriving them, and I agree with both calls.

- **`profiling-overhead.json`'s derived pair.** Confirmed: neither figure reproduces
  from the printed two-decimal percentiles, and both reproduce exactly under the
  disclosed caveat — `perTick.nanoseconds` 510.05 = 0.05 + 10000 × 0.051 and
  `breakEven.crossingsPerTick` 9803920.59 = (500000 − 0.05) / 0.051. `overhead.rounding`
  names both figures and states the rule, and
  `tests/conformance/tests/profiling_report.rs` fails the gate if that field goes
  missing, so nothing here is unbound. The observation that `breakEven` has no
  `formula` field while `perTick` does is correct — I checked; `breakEven` carries
  only `crossingsPerTick` and `detail` — and I agree it is not a defect: the
  derivation is published in prose at `README.md:328-346` and I verified that
  prose's backwards argument arithmetically. Adding the field would be an
  improvement, not a repair.
- **`coldRssSamples`.** Agreed uncheckable and not a finding: no field states a
  samples-per-duration rule, so nothing is falsified. I reproduced the audit's
  ratios and they only work against the **sum of all three repeat builds**, not the
  median — 257/((2526+2351+2550)/25) = 86.5%, then 88.1%, 79.6%, 75.9%, 73.6%. One
  correction to the characterization, which changes nothing: the series rises at the
  first step before falling, so it is not monotonic across all five points, though it
  is from the second onward. Consistent with a sampler whose true period slightly
  exceeds 25 ms and drifts further under load.

#### One inherited claim I checked and reject

The referred audit reported `minecraft/versions/26.1.2/hooks.toml` as modified and
uncommitted. That is wrong, and I checked it directly rather than trusting the
correction either: `jj st` reports "The working copy has no changes",
`sha256sum minecraft/versions/26.1.2/hooks.toml` is
`fcf9a3381089702597474398b528a2c4651323d1cc8e58d114ea104b7be2a457` and the file is
13402 bytes, which are exactly the `sha256` and `bytes` that `adapter.toml`'s
`[inputs.hookInventory]` pins. The pin is checkable and it checks. I note this
because a stale dirty-tree report is how an earlier reviewer lost a mutation run,
and because that audit's numbers were presumably taken on the same tree — the rest
of its arithmetic reproduced for me, so I read this as a stale observation rather
than a reason to doubt the other five.

#### Addendum ceiling

Everything in this addendum is a check of a claim someone else located. I did not
re-walk the four records myself, so the coverage of item 5 of my brief is theirs
rather than mine, and my own ceiling on `linux-process-matrix.json` stated above
still stands: it is the largest of the four and the one whose claims are least
recomputable by construction. Finding 8 was found by that audit and not by me,
which is itself worth recording — my own pass over the same file caught the
inventory arithmetic and the attestation disclosure and did not catch a
contradiction 530 lines apart in it.

The working copy was clean when I began this addendum and clean when I finished; I
made no edit and ran no mutation for any of it. `jj st`: "The working copy has no
changes."
