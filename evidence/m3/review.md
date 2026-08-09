# M3-W8 Independent Prototype Review

Result: FAIL

Three independent rounds ran against successive states of the M3 evidence. Each
reviewer wrote none of the work under review. All three returned FAIL, and the
third is the standing verdict: `evidence/m3/manifest.json` records
`review.result: "fail"` and M3-W8 stays open.

They are one record rather than three files because `review.record` in
`schemas/evidence.schema.json` binds one hashed artifact per milestone, and these
are successive rounds of a single M3-W8 review of a single work package. Carrying
them as three tracked files would need a schema change to hold three records, and
the rounds only mean anything read in order — round 2 verifies round 1's repairs,
round 3 verifies round 2's.

Each verdict is reproduced as written, without softening and with nothing added.
The `file:line` anchors are as of the tree each reviewer read; later rounds and
later repairs have moved some of them.

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
