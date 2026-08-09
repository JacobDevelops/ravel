# M2 Independent Closure Review — Cold Audits

Result: PASS

Independent cold audits ran against successive states of the M2 evidence. No
reviewer wrote any part of the work under review, and the later ones had not seen
the project before reviewing it. The last audit below is the standing verdict and
it returns PASS. The audits before it returned FAIL; their blocking findings are
answered, and answering them was not closure, because what M2 needed was an
independent audit of the repaired state that returned a pass. Each audit's own
findings and what became of them are recorded in
`evidence/m2/manifest.json`, and the sections below are the verdicts themselves.
This record is where the audits are counted; nothing outside it restates how many
there have been.

The standing verdict passes conditional on four items, and all four are done. The
stale `guard.rs` coverage figure its F1 measured is gone from `manifest.json`,
with the two per-file percentages beside it, and a gate refuses the percent sign
in a milestone manifest's prose so the class cannot return there. The citation its
F2 corrects carries an editor's note below rather than an edit to the reviewer's
words. Both blocking deviations are reclassified — the withdrawn-review one on the
condition its own text names, and the dirty-trial one on the reasoning this audit
recorded, reproduced in the manifest as the record of that judgement rather than
paraphrased into approval. And `review.result` is `pass` because this record now
states PASS, which is the only order those two may be written in.

## The first cold audit's scope

Reviewer: an independent cold auditor who wrote no part of the work under review.
Scope: the M2 milestone gate — `ROADMAP.md`'s M2 rows and their Verification
column, the closure rule at `ROADMAP.md:719`, `evidence/m2/README.md`,
`evidence/m2/manifest.json`, and all four records under `evidence/m2/` against
`schemas/evidence.schema.json`.

> **Editor's note, not the reviewer's text.** The third cold audit's F2 found that
> this citation no longer resolves: `ROADMAP.md:719` sits inside M3's profiling
> budget discussion, and the M2 closure rule the first audit read is the paragraph
> in `ROADMAP.md` that records M2's closure review. Both of this audit's
> `ROADMAP.md:719` citations — here and inside its F1 — are left as the reviewer
> wrote them, and the rule is identified here by its text rather than by a line
> number, because a line number inside a digest-bound record goes stale the next
> time anything above it in that file changes.

This is the record M2's `review.record` binds, and it exists because the review
that closed M2 did not. The previous record named a title and a sha256 matching
no file this repository tracks, so `review.result: "pass"` rested on a verdict no
reader could obtain; that digest is kept in the manifest's deviations and the
verdict it carried is withdrawn. `review.result` was `fail` on this audit's
verdict and on the next one's, and is `pass` on the third audit's, which is the
last section below.

Each verdict below is its auditor's, reproduced without softening and with nothing
added. Long lines are re-wrapped, headings are demoted one level, and where two
audits label a section identically the later one's heading carries its audit name,
all so this repository's markdown lint passes; no word of either verdict is changed. Nothing in either needed
redacting: neither names a path outside this repository, an account, or a launch
command. Where a repair or a correction belongs beside a finding it is a marked
editor's note, outside the reviewer's text and never woven into it.

Every `file:line` anchor is as of the tree its auditor read, and each read a tree
that precedes its own section. In the first audit, F1 has since been repaired by
wiring the gate to the world-open crossing and F7 by correcting the wording, so
those two anchors describe a state that has been changed rather than one that
stands; the rest stand.

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

---

## Second cold audit — VERDICT: FAIL

### M2 Independent Closure Review — second cold audit

Result: FAIL

Reviewer: an independent reviewer who wrote no part of the work under review, had not
seen this project before this review, and judged the artifacts rather than the
repository's account of them. All mutation work was done in a private copy of the tree;
the repository under review was never written to (`jj st` shows the same four unrelated
M1 files at the end as at the start).

#### What I verified by breaking

| Claim | Method | Result |
| --- | --- | --- |
| M2 is subject to `verify_recorded_verdict` | set `.review.result="pass"` and downgraded the blocking deviation, left `review.md` at `Result: FAIL` | `verify milestone M2` → exit 1, `verify scaffold` → exit 1, both reporting "records review.result pass while the review record it binds states fail". M2 **is** subject to the rule, in both gates. |
| A record's digest binding is enforced | falsified `prism-launch.json` (`nativeInvocations` 144→999999, `classesRewritten` 2→77) with the manifest entry intact | `verify scaffold` → exit 1 on the digest mismatch. |
| The F1 repair is real, not decorative | deleted the `open_world` call from `stage_world` in `crates/ravel-bootstrap/src/persistence.rs` | `cargo test -p ravel-bootstrap` → 2 failures, one per half (`a_halted_set_cannot_open_a_world_at_the_crossing_the_game_calls`, `a_refusal_the_machine_never_saw_still_withholds_the_world`). The manifest's "both halves are covered by tests that fail when the call is removed" is true. |
| The two generated records are byte-bound to live runs | one-byte edit to each record | `noop_pipeline` and `pre_world_gate` each fail `the_recorded_evidence_is_what_the_{loader,gate}_does_now`. |
| `pre-world-gate.json` describes the gate rather than a fixture | mutated `PreWorldReport::world_access` to always admit | 2 failures: `no_invalid_package_set_can_open_a_world` **and** `the_recorded_evidence_is_what_the_gate_does_now`. The record follows the code's fail-closed polarity. |
| The redaction rule over records bites | injected a launcher-style path and a `password=` assignment into a record | `verify milestone M2` → exit 1, "evidence/m2/pre-world-gate.json contains sensitive or local data". |
| The clone claim in the last deviation | `git clone --no-local` of the repo into a scratch directory | `refs/jj/*` empty; the abandoned `93772540…` is not an object; `aac3bfd2…` is a commit; all five `inputs[]` digests recompute there with `git show`; `evidence/m2/review.md` is absent at `aac3bfd2…` and present at clone HEAD hashing `dc8e8b68…`. |

Baseline before any mutation: `verify milestone M2` → exit 1 on exactly the two
documented violations.

#### Digests and arithmetic

All twelve bindings I could recompute, recomputed. Five `inputs[]` digests match at the
pinned `aac3bfd2…` (via jj, and again via git in a clean clone). All four
`benchmarks.results[]` digests and the `review.record` digest match the tracked files,
both in the working copy and at clone HEAD. `revision.changeId` `pumzmwvx…` is the
change that introduced `evidence/m2/manifest.json`, and `revision.id` is the one visible
commit that change is at; the pinning rule (`verify_pinned_commit`) therefore holds for
the manifest.

The recorded arithmetic is true, including the parts the manifest admits are awkward:

- 6 hook entries across 2 classes at `aac3bfd2…`, kinds `4 static-method / 1
  instance-method / 1 instance-field` → `classesRewritten: 2`, `methodsRewritten: 5`.
  This is the only independently recomputable figure behind M2-W5 and it reconciles
  exactly. Today's inventory holds 37 entries, which is why anchoring the count to the
  pinned revision matters and is done correctly.
- residual `7084 − (1212 + 221 + 4318) = 1333`; secant `(1987431 − 6272)/255 =
  7769.2509…`; binary growth `(930480 − 447744)/256 = 1885.6875` floored to 1885; pack
  compile `(3054 − 2174)/256 = 3.4375` ms, matching the 3.44 ms quoted in the roadmap;
  `0.625 ns × 160000 = 0.1 ms`; `429 = 429 + 0 + 0`.
- all six budget limits are exactly `ceil(measured × 2)`, and
  `experiments/m2-loader-overhead/src/main.rs:277` is `round((measured *
  HEADROOM).ceil())`, so the deviation explaining the 12.0-against-5.51 limit is
  accurate.
- the orphaned `abc003fd…` digest matches none of the 425 tracked files at clone HEAD,
  confirming the previous audit's orphan check at this revision.

M2's substance holds. The pre-world gate is genuinely consulted on a path the game
takes: `world.open-persistence` sits on `Minecraft.doWorldLoad` with `refuses = true`,
`Persistence.decide` calls `Hook.ravelOpenWorldPersistence`, and `stage_world` asks
`open_world` before it reads or writes a byte of world state, with
null-refuses/empty-admits polarity intact. I would have accepted M2 on the state of the
work.

#### Findings — second cold audit

**F1 — blocking. Two of the four admitted records pin a revision no reader can obtain,
and no rule looks.** `evidence/m2/prism-launch.json` (`trial.revision`) and
`evidence/m2/loader-overhead.json` (`trial.revision`) both record `id: df717025…`,
`changeId: wxzwmyll…`, `dirty: true`. `::visible_heads() & (df717025…)` resolves to
nothing — the commit survives only in this machine's operation log — and it is not an
object in a fresh clone. The change `wxzwmyll…` is at a *different* commit, `72c6fecc…`,
whose tree is not the one the launch and the measurement were made on. So M2-W5's launch
and M2-W8's budget each name a tree that does not exist anywhere a reader can reach, and
`dirty: true` means they were not made on any commit's tree in the first place.

This is not a hypothetical hole. It is the exact defect the manifest's own last two
deviations describe repairing — "a cold reviewer's fresh clone found every input digest
recorded here unreadable" — and the repair (`verify_pinned_commit`) stops at the
manifest boundary. `verify_recorded_change` exists and applies the change-provenance
rule to `.trial.revision.changeId`, but it is called only from `verify_spikes`, over
`evidence/m1/spikes/`. Established by breaking: I rewrote `loader-overhead.json`'s
`trial.revision.id` to forty zeroes and its `changeId` to `zzzz…`, updated the manifest
digest, and ran both gates — `verify scaffold` → **exit 0**, `verify milestone M2` →
exit 1 on the two pre-existing violations only. A record can pin a commit that has never
existed and nothing says a word.

Fixing it does not require regenerating anything, and cannot: the launcher log is
deleted by design and re-running the measurement would produce different numbers. The
smallest fix is (a) a rule — apply the `visible_commit` test and the
`changeId`-owns-`id` test to `.trial.revision` in every record a manifest admits, the
way `verify_pinned_commit` already does for `.revision`, and require `dirty` to be
declared rather than ignored; and (b) a deviation in `evidence/m2/manifest.json` stating
that these two records name an abandoned dirty working copy, that no reader can obtain
it, and what that costs — which is that M2-W5's attestation and M2-W8's budget cannot be
tied to any inspectable tree. Either fix on its own leaves half the problem; the rule
without the disclosure leaves a violation standing, and the disclosure without the rule
leaves the next record free to do it again.

> **Editor's note, not the reviewer's text.** F1 is answered, and by both halves it
> asks for. The rule now exists: `verify_admitted_records` applies the
> `visible_commit` and `changeId`-owns-`id` tests to `.trial.revision` in every
> record a manifest admits, and requires `dirty` to be declared rather than
> ignored. The disclosure exists too: `evidence/m2/manifest.json` records the two
> records that name an abandoned dirty working copy under
> `unobtainableRevisions`, states that no reader can obtain it, and states what it
> costs — M2-W5's attestation and M2-W8's budget are tied to no inspectable tree.
>
> **A second editor's note on the demonstration, attributed to the fix rather than
> to the reviewer.** The mutation above used forty zeroes as the commit id and
> thirty-two `z`s as the change id. Those are not absent values: they are jj's
> root commit and root change, and they resolve and agree with each other, so that
> particular mutation passed the first version of the rule rather than being
> caught by it. The finding stands on its own — the hole was real, and once the
> rule was written correctly it condemned 27 pins across the evidence set — but
> this one demonstration was weaker than it reads.

**F2 — blocking. M2 closure does not require its four records to be named in
`benchmarks.results`, so a one-line edit detaches a record from its digest and both
gates stay green.** `verify_m3_manifest` requires `benchmarks.results` to name all four
M3 records exactly, and its docstring says why: "a record no manifest names is a record
no digest covers". `verify_m2_manifest` has no equivalent. `M2_RECORDS` requires each
file to exist and to satisfy a content rule, which is not the same thing. Established by
breaking: I removed the `prism-launch.json` entry from `benchmarks.results` and rewrote
the record's `nativeInvocations` to 999999 and `classesRewritten` to 77 — `verify
scaffold` → **exit 0**, `verify milestone M2` → exit 1 on the two pre-existing
violations only. `cargo test -p ravel-spike-adg-5-prism` also passed 6/6 under that
mutation, because the generator's tests hold the record's prose and its attested pair,
not its counts. The attested figures in M2's only production-behaviour record are
protected by exactly one thing, and that thing can be deleted by a jq edit at the moment
the milestone closes — "the numbers a closed milestone records are exactly the ones
nobody looks at again", as the repository's own docstring puts it. Fix: mirror M3's rule
— require `benchmarks.results` to name `noop-pipeline.json`, `pre-world-gate.json`,
`loader-overhead.json` and `prism-launch.json` exactly.

> **Editor's note, not the reviewer's text.** F2 is answered.
> `verify_admitted_inventory` requires `benchmarks.results` to name
> `noop-pipeline.json`, `pre-world-gate.json`, `loader-overhead.json` and
> `prism-launch.json` exactly, at the scaffold level rather than only at closure,
> so removing an entry is a violation rather than a silent detachment.

**F3 — minor. `evidence/m2/manifest.json` deviation F5 says two records "did not exist
at that revision"; they existed with different bytes.** At `93772540…`,
`noop-pipeline.json` hashes `d4dcf5af…` and `pre-world-gate.json` hashes `1577b23d…`.
The audit record M2 binds states this correctly (regenerated, with both digest pairs);
the manifest's restatement of it does not. The conclusion drawn from it — that the
attested run did not verify the bytes the manifest now admits — is unaffected and true.
Fix: say "held different bytes" rather than "did not exist".

> **Editor's note, not the reviewer's text.** F3 is answered: the deviation now says
> the two records held different bytes at that revision and gives both digests.

**F4 — minor. `evidence/m2/README.md:41` says "every digest here is recomputable"; line
29 of the same file says the opposite about one of them, and there are two.**
`prism-launch.json` carries `redacted-instance-launch.log` and `libravel_bootstrap.so`
at `reproducible: false`, plus `ravel-agent.jar` at `reproducible: true` whose only
named provenance is the dirty, unobtainable revision of F1 — so in practice three. The
sentence is in the paragraph about the review and plausibly means "every digest the
manifest binds", which is true, but as written it contradicts the record it summarises.
Fix: scope the sentence to the bound digests.

> **Editor's note, not the reviewer's text.** F4 is answered: `evidence/m2/README.md`
> scopes the sentence to the digests the manifest binds and names the three in
> `prism-launch.json` that nobody can recompute.

**F5 — minor. "The machine's half is live in a launch" is an inference presented as an
observation.** `ROADMAP.md`'s M2-W7 note and the corresponding manifest deviation both
assert that a required mod that fails halts the set so that no world opens thereafter,
in a launch. The crossing firing in a real process is recorded —
`evidence/m3/linux-process-matrix.json` has three launches through
`world.open-persistence`, one of which refused a world and drew the refusal screen — but
that refusal came from the saved-data version check inside `SavedDataAggregate::open`,
not from `open_world`, which admitted in every recorded launch. No recorded launch has a
required mod fail. The mechanism is connected and demonstrably firing, which is why this
is minor rather than a repeat of F1's original shape, but the wording claims a launch
that is not in evidence, and the cited support lives in a milestone whose own review
records `fail`. Fix: say the gate is consulted on the crossing and that the machine half
has not yet refused in a recorded launch.

> **Editor's note, not the reviewer's text.** F5 is answered: `ROADMAP.md`'s M2-W7
> note and the matching deviation now say the gate is consulted on the crossing and
> that the machine half has not yet refused in a recorded launch.

**F6 — minor, carried rather than new. The one budget limit that is not what the
record's rule says it is.** `loader-overhead.json`'s `budget.rule` reads "Every limit is
the M2 measurement times two"; `loader.modCall.p99` is 12.0 against 5.51. The true rule
is `ceil(measured × 2)`, which I confirmed against the generator and against all six
limits. The manifest's deviation states this and the record is deliberately left
unedited. Correct as handled; recorded here so the next reader is not left to rediscover
it.

#### Why this is FAIL and not PASS with notes

The work is proven. Every row's substance survived the checks I could run, the blocking
finding of the previous audit is genuinely repaired and the repair is mutation-covered,
and the two generated records are bound to live behaviour in a way I broke twice to
confirm. What fails is closure, on two controls that a one-line edit clears — one of
which is not merely clearable but already cleared, right now, in the two records that
carry M2's launch attestation and the budget M3 regresses against. This project's
standing lesson is that a control which can be greened by editing a string is not a
control, and the milestone gate is where that lesson is supposed to be enforced. Closing
M2 today would freeze both holes into the one milestone nobody re-examines.

The path is short and fully specified: the record-level pinning rule plus its disclosure
(F1), the four-record naming rule (F2), the three wording corrections (F3-F5), and a
re-run of this review against the repaired state.

#### Reading-only ceiling

Everything below I judged by reading, not by breaking. This list is the ceiling on this
review's strength.

- **M2-W1 through M2-W4 in their entirety.** I did not read the test bodies behind
  canonical IDs, sides, versions, diagnostics, package round-trip and tamper refusal,
  dependency resolution cases, or the lifecycle transition table and spec invariants,
  and I mutated none of them. I relied on the bound audit record's row-by-row citations.
  If any of those four rows is proven by a test that would pass with its subject
  removed, this review would not have found it. This is the largest gap in my coverage.
- `schemas/evidence.schema.json`. I read it only through `tools/ravel-xtask/src/lib.rs`
  and greps. I did not check that schema validation rejects a prose-named
  `review.record`, and I did not test the `hashedArtifact` / `privateCapture` typing the
  baseline docstring rests on.
- `tests.total: 429` and `lineCoveragePercent: 86.43`. Not verified, and not verifiable:
  they were recorded against an abandoned commit and nothing recomputes them. Read as an
  attestation.
- The unrecomputable digests, correctly declared as such:
  `redacted-instance-launch.log`, `libravel_bootstrap.so`, `ravel-agent.jar`,
  `commands[].logSha256`, `commands[].argvSha256`, `trial.commandSha256`.
- The whole of M2-W5's launch. I ran no launch and read no capture. `stockLauncher`,
  `launcherChangedLines`, the instance component list, and `nativeInvocations: 144` are
  the operator's word; only the 2-classes/5-methods pair reconciles against tracked
  data, and I did check that.
- `jvm/bootstrap/dev/ravel/jvm/Persistence.java`. Read only. That a refusal string
  actually stops the world load in a running game is code I read, not behaviour I
  observed.
- `evidence/m3/linux-process-matrix.json`. Read only, not audited, and it belongs to a
  milestone recording `review.result: fail`.
- The `verify_recorded_inputs` no-jj skip path. I confirmed the clone facts with git but
  never ran the xtask inside a jj-less clone to see the announced skip.
- `verify_redaction`'s pattern coverage. I broke it with one path-and-credential string;
  I did not probe what it misses.
- `SPEC.md` §9.2's six conditions and §14's ten states. I read the record and the
  audit's account of the gap (two conditions absent, two refusals with `failureKind:
  null`) and did not enumerate the spec myself.
- Full `ravel:verify` and the whole test suite. Not run; I ran targeted packages and the
  two xtask gates only.
- M2-W6's "source-only" claim against the fixture's actual contents. Read from the
  record, not checked against the package.
- **Withdrawn findings**, recorded here rather than above. (i) That `noop-pipeline.json`
  and `pre-world-gate.json` do not hash to their recorded digests *at the pinned
  revision* — withdrawn once I established that record digests are bound in the working
  copy by design and that the record-to-revision decoupling is disclosed. (ii) That
  `review.md` does not exist at the pinned revision — withdrawn: the
  changeId-owns-introducing-change rule makes that structural, and the digest binds a
  tracked file that reproduces at clone HEAD. (iii) That the "eight of the ten §14
  states" prose is a hardcoded absolute count in the generator — withdrawn as
  immaterial: a new success-path state would break the byte comparison and force the
  prose to be updated, and only a new terminal state would go stale silently. (iv) That
  `verify milestone M2` alone does not recompute record digests — withdrawn: `verify
  scaffold` does, and it runs in `ravel:verify`.

---

## M2 Independent Closure Review — Third Cold Audit

Result: PASS

Reviewer: an independent cold auditor who wrote no part of the work under review
and had not seen this repository before this audit. Scope: `ROADMAP.md`'s M2 rows
and their Verification column, `SPEC.md` §9.2 and §14, `evidence/m2/README.md`,
`evidence/m2/manifest.json`, all four records under `evidence/m2/`, the two
scaffold controls the previous round's blocking findings introduced, and the
implementation behind M2-W1 through M2-W4 — which the previous round said it never
examined and named as the place a later reviewer should look.

Method: every verdict below marked *broke* was established by mutating a copy of
the repository (`rsync -a --exclude target`, `.jj` retained, cwd inside the copy,
`CARGO_TARGET_DIR` unset) and requiring a nonzero exit. Reading a gate or a test
proved nothing and was not treated as proof. The audited tree was never written
to; `jj st` reports no changes and the copy's source tree is byte-identical to the
original after every mutation was reverted.

Baseline in the copy before any mutation: `verify scaffold` exit **0**,
`verify milestone M2` exit **1** on exactly two violations — "milestone must have
no blocking deviations or risks" and "milestone review must be independent and
pass" — and `cargo nextest run --workspace --all-features` **888 tests run, 888
passed, 0 failed**.

### What the previous round left unexamined, and what breaking it showed

The previous round warned that if any row of M2-W1..W4 were proven by a test that
would pass with its subject removed, it would not have found it. Seven mutations
of the subject code, each run against the whole suite with `--no-fail-fast`:

| Mutation | Subject | Exit | Killed by |
| --- | --- | --- | --- |
| An illegal transition made legal (`Discovered -> Running` added to the table) | `crates/ravel-core/src/states.rs:96` | 100 | `ravel-core states::…matches_the_spec_transition_table`, `ravel-loader lifecycle::…accepted_or_refused_by_the_spec_table`, `ravel-loader::spec_invariants no_mod_code_runs_before_validation_and_dependency_resolution` (3 failed) |
| `ModId` grammar accepts an uppercase leading letter | `crates/ravel-core/src/identity.rs:61` | 100 | 7 tests across four crates, including `ravel-conformance-tests::package_schema the_mod_id_grammar_agrees_with_the_schema_pattern_under_a_real_engine` and `ravel-config document::…` (an unrelated crate caught `owner: "Example"` being admitted) |
| Package integrity never checks a digest (`verify_package` returns `Ok` immediately) | `crates/ravel-package/src/integrity.rs:63` | 100 | 17 tests, including `ravel-testkit::pipeline an_altered_source_file_stops_the_run_before_any_mod_code` and five `ravel-conformance-tests::pre_world_gate` tests |
| An incompatible dependency pair is allowed | `crates/ravel-resolve/src/graph.rs:166` | 100 | 9 tests, including `two_incompatible_mods_loading_together_fail_the_graph_closed` and `an_incompatibility_that_names_a_range_only_fires_inside_it` |
| Load order stops being the lexicographically smallest topological order (`pop_first` → `pop_last`) | `crates/ravel-resolve/src/graph.rs:361` | 100 | 7 tests, including `the_same_package_set_resolves_identically_from_every_input_order` and `a_package_set_that_declares_nothing_loads_in_identity_order` |
| `world_access` always opens a world | `crates/ravel-loader/src/lifecycle.rs:474` | 100 | 5 tests, including `ravel-loader::spec_invariants required_dependency_failure_prevents_world_access` and `failure_after_world_mutation_cannot_silently_continue_or_save_partial_state`, and `ravel-bootstrap persistence::…a_halted_set_cannot_open_a_world_at_the_crossing_the_game_calls` |
| ADG-7's crossing removed (the `open_world` call deleted from `stage_world`) | `crates/ravel-bootstrap/src/persistence.rs:249` | 100 | `ravel-bootstrap persistence::a_refusal_the_machine_never_saw_still_withholds_the_world` and `…a_halted_set_cannot_open_a_world_at_the_crossing_the_game_calls` (2 failed) |

None of these rows is proven by a test that survives its subject's removal. Two
further results matter:

- `SPEC.md` §14 names ten states and seven required invariants.
  `crates/ravel-loader/tests/spec_invariants.rs` carries one test per invariant,
  named in the spec's own words, plus one more; four of the seven were killed by
  the mutations above. The transition table is checked over all 10×10 ordered
  pairs against an oracle re-derived in the test, with the legal count asserted at
  16.
- The two generated records are genuinely byte-bound to behaviour, not merely
  digest-bound to themselves. Changing one insignificant byte of whitespace in
  `evidence/m2/noop-pipeline.json` failed
  `ravel-conformance-tests::noop_pipeline the_recorded_evidence_is_what_the_loader_does_now`
  and `ravel-xtask::scaffold required_scaffold_is_complete` (exit 100); the same in
  `pre-world-gate.json` failed with exit 100. Mutating the loader also broke
  `pre_world_gate the_recorded_evidence_is_what_the_gate_does_now`, so the record
  tracks the loader in both directions.

### The two controls the previous round's blocking findings introduced

Both hold, including against the flaw in the previous round's own proof.

1. **Record-level `trial.revision`.** `verify_admitted_revisions` in
   `tools/ravel-xtask/src/lib.rs:1179`. Broke it four ways, each with the manifest
   digest rebound so the digest control could not be what refused:
   - a genuinely bogus pin (`1111…1111`, undisclosed) → `verify scaffold` exit
     **1**, reported as "that commit names no commit in visible history";
   - the previous reviewer's own probe — id forty zeroes, changeId thirty-two
     z's, which are jj's ROOT commit and root change → exit **1**. `visible_commit`
     resolves against `(::visible_heads() & (rev)) ~ root()`, so the root escape
     the previous proof accidentally used is closed;
   - a disclosure covering a pin it does not name (record re-pinned, disclosure
     left at the old commit) → exit **1**, twice: once for the undisclosed pin and
     once for the disclosure nothing carries;
   - a disclosure outliving a repaired pin (record re-pinned to a real, obtainable
     commit of its own change, disclosure updated to name it) → exit **1**,
     "remove the disclosure rather than leaving an excuse the defect has outlived".

   Both escapes are refused for M2's records, as they were shown to be for M3's.

2. **`benchmarks.results` naming.** Removing `prism-launch.json` from
   `benchmarks.results` and rewriting its `nativeInvocations` to 999999 and
   `classesRewritten` to 42 → `verify scaffold` exit **1**
   (`verify_admitted_inventory`: "names no record prism-launch.json, which its own
   milestone directory tracks") and `verify milestone M2` exit **1** with the extra
   violation "M2 manifest must record all four M2 results under the
   m2-loader-budget-v1 protocol". The gap is closed at both levels.

### The three wording corrections

All three are made, and the factual claims behind the first two check out.

- Deviation F5 (`manifest.json:169`) now says the two records "held different
  bytes at that revision", not that they did not exist. Verified against the
  abandoned commit itself: at `93772540cf84400f7f2eb0b5d9ae71490851ca65`,
  `noop-pipeline.json` hashes `d4dcf5af…` and `pre-world-gate.json` `1577b23d…`,
  exactly the values the deviation quotes, while `loader-overhead.json`
  (`19e8b019…`) and `prism-launch.json` (`2896719b…`) are byte-identical to what
  the manifest binds today. The deviation is precisely true.
- `README.md:41` scopes the claim: "Every digest the manifest binds is
  recomputable — which is narrower than every digest in this directory, because
  `prism-launch.json` names three of its own that nobody can". All four bound
  record digests recompute (`sha256sum evidence/m2/*.json` matches
  `benchmarks.results` and `review.record` matches `review.md` at
  `c2889007a70ad99192e58547e6cedea92b0158d95deafc64bcd263d08d4bf21e`).
- M2-W7 (`ROADMAP.md:617-633`) no longer claims the machine's half is live in a
  launch: "The machine's half is consulted on that crossing in a launch, and it has
  not yet refused in one … that a halted set withholds a world is proven in-gate by
  tests that fail when the call is removed." That last clause is now a measured
  statement rather than an assertion — see the ADG-7 mutation above.

### Arithmetic and counts

Every absolute figure I could recompute, recomputes.

- Six budget limits, each exactly `ceil(measured × 2)`: 7084→14168, 7769.25→15539,
  5.51→12, 880→1760, 598→1196, 1885→3770. Deviation F4's account of the integer
  ceiling is correct and complete.
- M2-W5's reconciling counts: at the pinned revision `aac3bfd2…`,
  `minecraft/versions/26.1.2/hooks.toml` holds exactly 6 entries across exactly 2
  distinct classes — 4 `static-method`, 1 `instance-method`, 1 `instance-field` —
  matching `classesRewritten` 2 and `methodsRewritten` 5. (The same file at the
  working copy now holds 37 entries, which is why pinning the claim to a revision
  was the right call.) `nativeInvocations` 144 reconciles against nothing, as
  deviation F3 says.
- `pre-world-gate.json` holds 6 refusals, 2 with `failureKind` null
  (`unparsable-manifest`, `digest-mismatch`); `SPEC.md` §9.2 lists 6 conditions and
  the record covers 4 of them by kind. Deviation F6's accounting is exact.
- `noop-pipeline.json`'s transcript holds 8 distinct `SPEC.md` §14 states; §14
  defines 10. The README row's explanation of the missing two is correct.
- No `proptest` or `quickcheck` anywhere in `Cargo.lock` or any manifest, as
  deviation F8 states.
- Workspace line coverage measured at **87.06%** against the 80% floor.

### Findings — third cold audit

#### F1 — moderate. A bound coverage figure in the manifest is false, and the deviation whose job is to catch that missed it

`evidence/m2/manifest.json:125` states, in the present tense:
"crates/ravel-bootstrap/src/guard.rs sits at 54.55% lines and stays there."

Actually true: 28.72% lines, 39.80% regions, 41.67% functions. Established by
measuring, not reading:
`cargo llvm-cov nextest --workspace --all-features --summary-only --ignore-filename-regex '(^|/)experiments/adg-'`,
exit **0**, `crates/ravel-bootstrap/src/guard.rs … 94 lines, 67 missed, 28.72%`.
The file grew as M3 added hook surface, so the figure drifted downward by 26
points. The other half of the same sentence still holds exactly — the delegated
logic is `wire.rs` at 100.00% and `report.rs` at 80.00%.

What makes this more than a stale number is deviation F8 at `manifest.json:181`,
whose stated purpose is precisely this sweep: "Present-tense prose in this
historical manifest has gone stale and is left standing rather than rewritten, so
this sentence says what is true now." It then enumerates three stale items —
`schemas/package.schema.json`, the `ravel-config` placeholder risk, and
`modExecutionModelSelected` — and misses this one, while its wording reads as a
complete sweep. This is the seventh absolute number found stale across this
project and the first found inside the sweep that was written to end them.

The deviation's *conclusion* is unaffected: the classification is non-blocking
because the path is verified by the JVM harness and the recorded launch rather than
by coverage, and that reasoning does not depend on the percentage. Nothing M2
claims about loader behaviour rests on this figure.

Fix: one sentence in `manifest.json`, in the same shape the manifest already uses
for stale prose — either extend F8 to name this figure and give the current one, or
add a sentence to the guard.rs deviation itself. Do not rewrite the original
number; the manifest's own convention is to leave historical prose standing and add
what is true now.

#### F2 — minor. A citation in the bound review record no longer resolves

`evidence/m2/review.md:17` cites "the closure rule at `ROADMAP.md:719`". The M2
closure rule is now at `ROADMAP.md:635-639`; line 719 is inside M3's profiling
budget discussion. Established by reading. Because `review.md` is bound by digest
and reproduced without softening, correcting it means rebinding the digest, which
is the right trade only if the record is being rewritten anyway — this review's
verdict does replace it, so the citation can be fixed by reference to the rule's
text rather than its line number.

#### F3 — disclosure, not a defect. Closure is an attestation gate, and M2's evidence does not say so

I confirmed by breaking that four coordinated string edits close M2 with both
gates green: `review.md`'s `Result: FAIL` → `Result: PASS`, `review.result` →
`pass`, both blocking classifications → `non-blocking`, and the `review.record`
digest rebound. Result: `verify milestone M2` exit **0** ("Ravel milestone
verification passed"), `verify scaffold` exit **0**.

The controls that do exist are real and I verified both:

- the record's bytes are digest-bound — I confirmed all five digests recompute;
- the record's verdict line must agree with the manifest. Flipping only
  `review.result` to `pass` and the two classifications, leaving `review.md`
  saying `Result: FAIL`, gives `verify milestone M2` exit **1** with
  "records review.result pass while the review record it binds states fail". That
  control is not decorative.

So the residue is inherent: no gate can distinguish an attestation a person stands
behind from one they fabricated. I record it here rather than treat it as a
finding, because M2's evidence nowhere states this limit — the statement lives only
in `evidence/m3/review.md:1226` — and because this record becoming M2's review
record is what puts the statement in M2's evidence.

#### F4 — minor observation. Three void budget limits still pass the gate as live limits

`ROADMAP.md:641-653` declares `build.packCompile`, `build.relink` and
`build.binaryGrowthPerMod` void, on the record's own `budget.expiry` clause, because
M3 chose source compilation over linking. `loader-overhead.json` is deliberately
not edited, which is right — but the M2 gate rule at
`tools/ravel-xtask/src/lib.rs:2684` validates all six limits identically, and
nothing inside the record marks three of them dead. A reader who runs the gate and
not the roadmap gets a green on three limits the roadmap says describe a shape that
no longer exists. This is disclosed in prose and I am not asking for it to be
mechanised; it is noted so the next budget does not inherit the pattern.

### On the remaining blocking deviation, which is a judgement and therefore mine to make

`verify milestone M2` fails on two violations. One is this review, which now
passes. The other is "no blocking deviations or risks", and two deviations carry
`blocking`:

**`manifest.json:153` — the withdrawn original review and its withdrawn `pass`.**
Its own closing sentence is "What closes M2 is an independent review of the
repaired state that returns pass, not an edit to this file." That condition is met
by this record. Reclassifying it to non-blocking is executing the deviation as
written, and the orphaned digest
`abc003fdd77ede9be08f0ddf36c0b10e381431ff684c8bb7a874420513abea78` must stay in the
sentence so the withdrawn claim is not lost.

**`manifest.json:193` — both measurement trials pin a dirty tree no reader can
obtain.** I judge this **non-blocking**, and the reasoning has to be on the record
because the manifest currently argues the opposite.

The finding is true: `loader-overhead.json` and `prism-launch.json` both record
`trial.revision.dirty: true` on `df717025…`, a commit no visible head reaches, under
a change that has moved off it. The gate says so out loud on every run — I saw both
lines in the passing baseline — and the disclosure mechanism fails closed in both
directions, which I proved rather than read.

What I weighed against the manifest's own argument that "a launch attestation and a
performance budget whose source is unidentified are the two claims M2 closes on":

- M2-W5's claim is a historical event in a process this repository cannot re-enter,
  and the artifact behind it is deleted by design to keep an account's session
  arguments out of the repository. No pin, clean or dirty, could make that
  recomputable. Requiring one would make M2-W5 unprovable by construction — the
  same defect in the opposite direction. What matters instead is that the record
  *labels* the unrecomputable parts attested, and it does: `launch.attestation`
  carries `derived: false` and says in its own detail that the runner reads the
  log and not the launcher. And the one part that can be corroborated,
  corroborates — 2 classes and 5 methods reconcile against `hooks.toml` at the
  pinned revision, independently of the operator's word.
- M2-W8's budget is a wall-clock measurement already scoped to one hardware profile,
  already carrying a factor of two for run-to-run variance, already half-void, and
  re-measurable at any time. Its three surviving loader limits are a baseline for a
  like-for-like comparison, and the property that makes that comparison valid — the
  metric is pinned to a no-op package that registers no content — is stated and
  independent of which tree was measured.
- Nothing else in M2 depends on either trial. Every capability claim in M2-W1
  through M2-W4, W6 and W7 is proven by tests that run on today's tree and that I
  killed by mutation.

So the honest classification is a disclosed limitation on two attested figures, not
a blocking defect in what M2 built. It is already disclosed three times over — in
the deviation, in `unobtainableRevisions`, and on the gate's own stdout — and it
cannot be repaired, only re-measured into a different set of numbers that would
attest a different run.

### What convinced me, and what still has to change

What convinced me: every load-bearing claim in the four rows the previous round
never examined survives being attacked and dies when its subject is removed —
canonical IDs against the schema pattern under a real regex engine, package
integrity and tamper-before-execution, all four dependency relationship kinds
including range-scoped incompatibility, deterministic load order, the full 10×10
transition table, all seven `SPEC.md` §14 invariants named one-to-one, and both
halves of the world-access gate including ADG-7's crossing in the composed host.
The two controls the previous round demanded exist and refuse every escape I could
construct, including the one that reviewer's own flawed proof would have missed.
Every digest the manifest binds recomputes; every piece of arithmetic I could
redo is true; F5's byte-level claim about an abandoned commit is exactly right; and
the redaction control refuses a fabricated local path and credential at the
milestone gate, precisely where the README says it lives and not where it does not.

For `devenv tasks run ravel:verify-milestone:m2` to exit 0, the smallest set is:

1. correct the guard.rs coverage figure per F1, in the manifest's own
   leave-and-append convention;
2. reclassify `manifest.json:153` to non-blocking, keeping the orphaned digest in
   its text, because this review is the condition that deviation names;
3. reclassify `manifest.json:193` to non-blocking on the reasoning above, and add a
   sentence recording that an independent review judged it so — not a silent flip;
4. record this verdict as `review.md` with `review.result: "pass"`, rebinding
   `review.record.sha256`, and update `ROADMAP.md:635-639` and
   `evidence/m2/README.md:32-50`, which both currently state that M2's review
   returns FAIL.

Nothing in that list changes a measurement, a record, or a line of loader code.
That is the point: M2's work is sound, and what was left was bookkeeping and one
judgement.

### Ceiling — what this review judged by reading rather than by breaking

This is the limit on how much weight the verdict above can carry. Every item here
was read, or trusted to a gate I did not independently falsify.

- **The launch itself.** That an official 26.1.2 client started from a stock Prism
  build, reached a world, accepted input and exited cleanly is an operator
  attestation. I did not launch anything. `nativeInvocations` 144, the redacted
  launcher log digest, the native library digest and the agent jar digest reconcile
  against nothing and I confirmed only that the record says so.
- **`commands[0]` and `tests`.** The `ravel:verify` at 429 tests, 86.43% coverage,
  exit 0 on `93772540…` is an attestation about an abandoned tree. The current tree
  runs 888 tests and 87.06% line coverage, which is a different run and not a
  check of that one.
- **The three wording corrections** are prose claims and reading is the only
  available check, though I did break out and verify the two factual sub-claims
  underneath F5 and the README scoping.
- **`schemas/evidence.schema.json` conformance** is trusted to the passing scaffold
  baseline. I did not construct a manifest that violates the schema to confirm the
  schema check refuses it.
- **`review.md`'s 472 lines of prior-audit content.** I read the two earlier
  verdicts and did not re-derive their findings. Where this review overlaps them it
  re-established the result independently; where it does not, those findings rest on
  their own authors.
- **The Java side.** `jvm/bootstrap/dev/ravel/jvm/Persistence.java`'s refusal
  handling and the fail-closed polarity ADG-7 describes were read, not exercised. I
  broke the Rust half of that crossing; I did not run the Java half.
- **`ravel:verify` as a whole.** I ran `verify scaffold`, `verify milestone M2` and
  the test suite directly rather than the composite task, and I did not verify that
  the composite propagates each sub-task's failure.
- **F4's expiry reasoning** — that source compilation is not linking and therefore
  fires the clause — is read from `ROADMAP.md` and `evidence/m3/pack-compile.json`;
  M3's evidence was out of scope and I did not audit the 113.34 ms figure.
- **Withdrawn during this review, and therefore not a finding:** an earlier pass of
  my own mutation harness restored files with `shutil.move`, which preserves the
  backup's mtime, so cargo reused artifacts built from the previous mutation and
  three "kills" were attributed to the wrong mutation. Every result reported above
  comes from the re-run harness, which touches each restored file and uses
  `--no-fail-fast`, and whose clean baseline was re-established at 888/888 before
  the first mutation. Recorded here because a reviewer's own harness is exactly the
  kind of thing that passes for the wrong reason.
- **Coverage of the M2 crates individually.** I measured the workspace total and
  `guard.rs`; I did not check whether any other per-file figure quoted anywhere in
  M2's evidence has drifted the way that one did. Given that one had, the others
  are worth a sweep by whoever fixes F1.
