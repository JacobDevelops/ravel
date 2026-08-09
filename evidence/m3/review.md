# M3-W8 Independent Prototype Review

Result: FAIL

Independent rounds ran against successive states of the M3 evidence. Each reviewer
wrote none of the work under review, and every round returned FAIL. The last round
below is the standing verdict. Round 5's blocking finding B-1 is answered — by the
`profilingReObservationCampaign` in `evidence/m3/linux-process-matrix.json` — and
so is Round 6's, which is a different B-1 about the world-persistence delivery.
Answering a blocking finding is not closure: what M3-W8 needs is an independent
round that returns a pass, and none has. So `evidence/m3/manifest.json` records
`review.result: "fail"` and M3-W8 stays open.

One round is not here. The earliest round of all reviewed the prototype rather than
the evidence set, and its verdict text was never written into the repository; the
next section states what is known about it, that the text is not recoverable, and
what that costs a reader. **The numbering below counts only the rounds this file
holds, so "Round 1" is the second round that ran.** Every round that has run since
is here, in order, so this file rather than any summary of it is what says how many
there have been.

They are one record rather than one file each because `review.record` in
`schemas/evidence.schema.json` binds one hashed artifact per milestone, and these
are successive rounds of a single M3-W8 review of a single work package. Carrying
them as separate tracked files would need a schema change to hold more than one
record, and the rounds only mean anything read in order — each verifies the
previous round's repairs.

Each verdict is reproduced as written, without softening and with nothing added.
The `file:line` anchors are as of the tree each reviewer read; later rounds and
later repairs have moved some of them.

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

### M3-W8 Independent Review — Round 7

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
