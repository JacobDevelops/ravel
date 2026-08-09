# M2 Independent Closure Review — Cold Audit

Result: FAIL

Reviewer: an independent cold auditor who wrote no part of the work under review.
Scope: the M2 milestone gate — `ROADMAP.md`'s M2 rows and their Verification
column, the closure rule at `ROADMAP.md:719`, `evidence/m2/README.md`,
`evidence/m2/manifest.json`, and all four records under `evidence/m2/` against
`schemas/evidence.schema.json`.

This is the record M2's `review.record` binds, and it exists because the review
that closed M2 did not. The previous record named a title and a sha256 matching
no file this repository tracks, so `review.result: "pass"` rested on a verdict no
reader could obtain; that digest is kept in the manifest's deviations and the
verdict it carried is withdrawn. `review.result` is now `fail`, which is this
audit's.

The verdict below is the auditor's, reproduced without softening and with nothing
added. Its long lines are re-wrapped and its headings demoted one level so this
repository's markdown lint passes; no word is changed. Nothing in it needed
redacting: it names no path outside this repository, no account, and no launch
command.

Its `file:line` anchors are as of the tree the auditor read, which precedes this
record. F1 has since been repaired by wiring the gate to the world-open crossing
and F7 by correcting the wording, so those two anchors describe a state that has
been changed rather than one that stands; the rest stand. What each finding
became is recorded in `evidence/m2/manifest.json`.

---

## M2 independent closure review — cold audit

## VERDICT: FAIL

Independent of the work under review; I wrote none of it. The withdrawn `review.result: "pass"`
carried no weight in this pass — I could not obtain what it was based on, so I judged the tracked
evidence directly.

### Row-by-row

| Gate row | Verification (ROADMAP.md) | What proves it | Status |
| --- | --- | --- | --- |
| M2-W1 | unit/property tests cover valid and invalid values (`ROADMAP.md:608`) | `crates/ravel-core/src/identity.rs:169,188,258,275,303,315`; `sides.rs:115-175`; `diagnostics.rs:393-511`; `states.rs:152,163` (10×10 ordered-pair table) | proven |
| M2-W2 | valid packages round-trip; tamper and schema failures precede code execution (`ROADMAP.md:609`) | `crates/ravel-package/tests/package.rs:79,97,110,124`; `tests/conformance/tests/package_schema.rs:184,248,264,327,374` | proven |
| M2-W3 | required/optional/incompatible/discouraged, cycles, duplicates, ranges, deterministic order (`ROADMAP.md:610`) | `crates/ravel-resolve/tests/resolve.rs:87,112,134,149,219,242`; `crates/ravel-resolve/src/graph.rs:724,744,853,907,1130` | proven |
| M2-W4 | every legal and illegal transition plus every spec invariant (`ROADMAP.md:611`) | `crates/ravel-core/src/states.rs:163` and `crates/ravel-loader/src/lifecycle.rs:931` exhaust all 100 ordered pairs; `crates/ravel-loader/tests/spec_invariants.rs:128,200,287,354,416,493,606` = one test per SPEC.md:834-840 invariant (7/7) | proven |
| M2-W5 | official 26.1.2 process reaches Ravel without another loader (`ROADMAP.md:612`) | `evidence/m2/prism-launch.json:66,74,76,77` + `instance.withoutAnotherLoader`; `evidence/m2/manifest.json:174` attestation | proven, but **attested, not recomputable** (see F3) |
| M2-W6 | recorded evidence shows validate→stop in deterministic order (`ROADMAP.md:613`) | `evidence/m2/noop-pipeline.json:47` phases, three sides, `repeatedRunIsIdentical: true`; byte-bound to a live run by `tests/conformance/tests/noop_pipeline.rs:34` | proven — strongest item in the milestone |
| M2-W7 | invalid packages, sides, integrity and graphs **cannot open a world**; reports redact secrets/paths (`ROADMAP.md:614`) | `evidence/m2/pre-world-gate.json:6,21-81` (6 refusals, redaction at `:2-19`), byte-bound by `tests/conformance/tests/pre_world_gate.rs:41` | **unproven as worded** (F1) |
| M2-W8 | the measured loader cost establishes the numeric regression budget used from M3 onward (`ROADMAP.md:615`) | `evidence/m2/loader-overhead.json` budget block; enforcement now exists at `tools/ravel-xtask/tests/budget.rs:90-247` | proven, one arithmetic defect (F4) |

### Findings

**F1 — blocking. The world-access gate has no production caller, so "cannot open a world" is a model
answer, not an enforced one.** `crates/ravel-loader/src/world.rs:27`
(`LifecycleMachine::open_world`) is called from nowhere outside tests — I searched every `.rs` under
`crates/`, `tests/`, `test-mods/`; the only hits are `world.rs:140,150` and the conformance
generator. No hook connects it: at the pinned revision `minecraft/versions/26.1.2/hooks.toml` held 6
targets in `StringUtil` and `SmoothDouble`, none world-related, and today's `world.open-persistence`
(`hooks.toml:327`) is ADG-7 saved-data plumbing rather than a gate. So what is established is "if
something asks the loader, it answers no" — the report refuses (`pre-world-gate.json:6`) — and not
that an invalid package set is prevented from entering a world in the game process. *Concluded from
reading code, cross-checked against the pinned hook inventory.*

The records invite the stronger reading rather than stating the weaker one. `pre-world-gate.json:82`
says the cases were "refused before a world could open" and `README.md:10` says the same; both
describe a world that no code path in this repository was in a position to open or withhold. This is
the milestone's only safety claim, and it is the same failure shape this project's review history
has already hit three times: a claim written as accomplished over a mechanism never connected. Under
`ROADMAP.md:719` — a row is closed by its Verification column, never by a file being present — M2-W7
is not closed. Fixing it is small: either wire the gate to a world-open hook and record that run, or
amend row and record to say the loader withholds world access in its own model and that no game path
consults it yet. I would not accept the amendment silently; it changes what M2 is understood to have
proved.

**F2 — blocking (bookkeeping). `review.result: "pass"` survived the withdrawal of its own basis.**
`manifest.json:169-173` still records `independent: true` and `result: "pass"` attributed to a
reviewer string naming the very pass whose record `manifest.json:139` says "could not be obtained,
read, or hashed by anyone who did not run it". Only the digest was withdrawn; the verdict it carried
was kept. I confirmed the orphan is a true orphan: hashing every tracked file at the manifest's
pinned revision `93772540…` (304 files) and at the working revision (424 files) produced 0 matches
for `abc003fd…`. M2 is now also the only milestone claiming `pass` — M1
(`evidence/m1/manifest.json`) and M3 (`evidence/m3/manifest.json`) both record `fail` against
readable records.

**F3 — medium. Every load-bearing figure in M2-W5 is attested, and the manifest's own wording
overstates two of them.** `prism-launch.json:74,76,77` (2 classes, 5 methods, 144 native
invocations) and `instance.components/modFiles/coreModFiles` are transcribed from a launcher capture
that is deleted once hashed (`manifest.json` deviation 6), so `redacted-instance-launch.log`'s
digest is unrecomputable by anyone. The record labels `stockLauncher` honestly as attested
(`prism-launch.json:60`, `attestation.derived: false`) and rests `provesStockLaunch` on the
`-javaagent` argument rather than the inert library-path argument — both correct after the earlier
correction. Two counts do reconcile independently and this is the one place M2's numbers survive
recomputation from outside the capture: at the pinned revision `hooks.toml` holds exactly 6 entries
— 4 `static-method`, 1 `instance-method`, 1 `instance-field` — across exactly 2 classes, matching
`classesRewritten: 2` and `methodsRewritten: 5` precisely. `nativeInvocations: 144` reconciles
against nothing. Consistent with the M0/M1 precedent and gate-checked, so acceptable; it must be
read as *attested by an operator and not recomputable*, never as observed.

**F4 — low. One budget limit is not what the budget's own rule says it is.**
`loader-overhead.json`'s rule states "Every limit is the M2 measurement times two", and five limits
are exactly that (7084→14168; 880→1760; 598→1196; 1885→3770; 7769.25→15539, rounded up by 0.5).
`loader.modCall.p99` measured 5.51 with limit 12.0 (`loader-overhead.json:23-25`) — 2.18×, about 9%
looser than the stated rule. Everything else in the record reconciles on recomputation: the marginal
secant (1987431−6272)/255 = 7769.2509…; the residual 7084 − (1212+221+4318) = 1333; binary growth
930480−447744 = 482736, /256 = 1885 (floored); pack compile 3054−2174 = 880, /256 = 3.4375 → 3,
matching the "3.44 ms" at `ROADMAP.md:634`; and ADG-2's 0.625 ns × 160000 = 0.1 ms. `tests.total`
429 = 429+0+0.

**F5 — low. The manifest describes no single tree state.** `revision.id` pins `93772540…` with
`dirty: false` and `commands[0]` attests a `ravel:verify` at 429 tests there, but two of the four
records it hashes did not exist at that revision: `noop-pipeline.json` and `pre-world-gate.json`
were regenerated on 2026-08-08 by the schema-v1 migration (`d4dcf5af…`→`56e39268…`,
`1577b23d…`→`23b27ca3…`), while `loader-overhead.json` and `prism-launch.json` are byte-identical to
it. So the attested run did not verify the records now admitted. Heavily mitigated: both regenerated
records are byte-compared against a fresh run on every test run, and `ravel:verify` currently exits
0 — they describe today's loader, which is stronger than an archived one. But the recomputation
fixed digest↔file binding at the cost of record↔revision binding, and nothing in the manifest says
so.

**F6 — low. The regeneration silently dropped a refusal case, and the record still claims
completeness.** `pre-world-gate.json` went from 7 refusals to 6: `unsupported-target` was removed
with the `targets` schema form. Not in M2-W7's enumerated list, so the row is unaffected, but the
summary at `:82` reads as exhaustive and the README at `:10` asserts each case is "reported as its
own `SPEC.md` §9.2 outcome". Two of the six carry `failureKind: null` (`:22`, `:32`), and two of
§9.2's six conditions (SPEC.md:618-619) are absent from the record entirely — duplicate identities
and conflicting registry claims. Duplicates *are* implemented and tested
(`crates/ravel-resolve/src/graph.rs:111,1130`; `FailureKind::DuplicateIdentity` at `:562`), so this
is a gap in the record and not in the loader; conflicting registry claims is legitimately M3-W1's.

**F7 — low. Two claims of completeness that are not complete.** `README.md:9` and
`noop-pipeline.json:205` say the package ran "every `SPEC.md` §14 state"; the transcript holds 8 of
the 10 that `SPEC.md:821-830` and `crates/ravel-core/src/states.rs:36` define. `Discovered` precedes
the transcript and `Failed` cannot appear in a successful run, so 8 is the right number — "every
state" is the wrong words for it.

**F8 — informational. Present-tense prose in a historical manifest is now false.**
`manifest.json:127` says `schemas/package.schema.json` "still requires per-target artifacts" and
that "M3 reconciles the schema"; M3 did, on 2026-08-08. The risk at `manifest.json` about
`ravel-config` being an empty placeholder and `environment.settings.modExecutionModelSelected:
false` are likewise overtaken. `README.md` explains this; the manifest does not. No property-based
testing framework exists anywhere in the repository (no `proptest`, no `quickcheck`), so M2-W1's
"unit/property tests" is met by exhaustive enumeration over closed sets and table-driven
valid/invalid cases — stronger than random generation for these types, but the row's wording
promises something that is not there.

**What matched the tree.** M2 claims it filled the crates, and it did: `ravel-core` (identity,
sides, states, diagnostics, content), `ravel-package` (manifest, integrity, compatibility),
`ravel-resolve` (graph, plan), `ravel-loader` (lifecycle, driver, registry, world), plus
`ravel-bootstrap`, `ravel-host-api`, `ravel-minecraft-adapter` and the 26.1.2 version data. Nothing
in the records claims an implementation I could not find.

### Digests recomputed

12 recomputed, 12 matched.

- 4/4 `benchmarks.results[]` against their files in the working copy — all exact.
- 5/5 `inputs[]` against the pinned revision via `jj file show -r 93772540… --` — all exact. (All
  but `linux-reference.toml` differ in the working copy, which is expected: the manifest pins a
  revision.) **The two stale result digests reported earlier are confirmed recomputed and correct.**
- 3/3 internal digests of the `noop` fixture (`Cargo.toml`, `src/lib.rs`, `provenance.json`) against
  `tests/fixtures/public/packages/noop/package.json` — all exact.
- Orphan check: 728 further file digests computed across two revisions, 0 matches for `abc003fd…`.
- Not recomputable by anyone, correctly declared as such: `redacted-instance-launch.log`,
  `libravel_bootstrap.so`, the `logSha256` of the verify run, and `commands[].argvSha256`.

### Gate runs

`devenv shell -- devenv tasks run ravel:verify` → exit 0 (827 tests present today vs 429 recorded).
`ravel:verify-milestone:m2` → exit 1 on exactly two violations: `milestone must have no blocking
deviations or risks` and `milestone review must include a hashed record`. Judged by exit code only.

### Coverage

Read in full: `evidence/m2/` (README, manifest, all four records), `schemas/evidence.schema.json`
`review` shape, `ROADMAP.md` M2 section and closure rule, SPEC.md §9.1-9.3 and §14,
`tools/ravel-xtask/src/lib.rs` (`verify_milestone`, `verify_general_closure`, `verify_m2_manifest`,
`M2_RECORDS`, `verify_recorded_review`, `verify_redaction`, `verify_evidence_records`),
`crates/ravel-loader/src/world.rs`, `hooks.toml` at both revisions, the M0/M1/M3 manifest review and
command blocks for comparison. Read by targeted search: the test bodies behind W1-W4, both
conformance generators, `tools/ravel-xtask/tests/budget.rs`.

Not reached: I did not re-run the loader-overhead measurement or the pack-build figures, did not
attempt a launch, did not read `tests/conformance/tests/*.rs` end to end (I confirmed the
byte-comparison mechanism and the assertion names, not every assertion), did not verify the 429-test
count or the 86.43% coverage figure at the pinned revision, and did not audit the M0/M1/M3 evidence
beyond the fields I compared against.

### If this were reversed to PASS

It is not, but for the record: F1 is the only finding I would refuse to trade away, and F2 is a
one-line correction. Were both resolved, a maintainer accepting M2 would still be *trusting rather
than verifying* the whole of M2-W5 — that the launcher was the pinned stock build, unmodified, and
that 2 classes / 5 methods / 144 invocations came out of the official process — because the capture
behind it is deleted by design and the only tests over it hold the record against its own generator.
They would also be trusting the 429-test, 86.43%-coverage figures at a revision two of the admitted
records postdate, and every loader number as a floor on one machine against one no-op package, which
the record says plainly in `whatThisDoesNotSay`.
