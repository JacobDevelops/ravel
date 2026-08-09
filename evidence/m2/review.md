# M2 Independent Closure Review — Cold Audits

Result: FAIL

Two independent cold audits ran against successive states of the M2 evidence, and
both returned FAIL. Neither reviewer wrote any part of the work under review, and
the second had not seen the project before reviewing it. The last audit below is
the standing verdict; the blocking findings of both are answered, and answering
them is not closure, because what M2 needs is an independent audit that returns a
pass. Each audit's own findings and what became of them are recorded in
`evidence/m2/manifest.json`, and the sections below are the verdicts themselves.

## The first cold audit's scope

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
