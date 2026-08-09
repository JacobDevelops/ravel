# M1 Independent Cold Audit

Result: FAIL

Reviewer: independent cold auditor, no part in the reviewed work. Scope: the M1
milestone gate — `ROADMAP.md` M1 rows and their Verification column, the closure
rule at `ROADMAP.md:720-722`, `evidence/m1/README.md`,
`evidence/m1/manifest.json`, and every record under `evidence/m1/` against
`schemas/evidence.schema.json`.

The verdict below is the auditor's, reproduced without softening. Its `file:line`
anchors are as of the tree the auditor read, which precedes this record; the
files have been edited since, so a line number may have moved even where the
finding still stands.

## Verdict

M1's measurements are unusually honest and, where they are recomputable, they
recompute exactly. M1's **review** does not exist in any checkable form, one gate
row is only partly proven, and I found one undisclosed instance of a defect class
the corpus discloses elsewhere. `ROADMAP.md:720-722` closes a row by its
Verification column and by `ravel:verify`; `ravel:verify-milestone:m1` exits 1,
and `M1-W8`'s Verification says in terms "M1 gate passes".

## Row by row

| Row | Proof | Status |
| --- | --- | --- |
| M1-W1 shared harness, one metric schema | `schemas/spike-result.schema.json`; all 20 records conform, `ravel:verify-spikes` exit 0 (`{}`) | proven (observed) |
| M1-W2 ADG-1, metrics measured, rejection explicit | 6 records in `evidence/m1/spikes/adg-1/`; `disposition: "rejected"` in `adg1-external-bytecode-library.json`, `adg1-jvmti-native-agent.json`; ADR `docs/decisions/adg-1-jvm-entry.md:3` | proven, with F7 |
| M1-W3 ADG-2 rubric complete | 4 records in `spikes/adg-2/` carrying `isolation`, `abiEvolution`, `modToModCalls`; ADR `docs/decisions/adg-2-mod-execution.md:283` | proven; tails are batch averages (disclosed) |
| M1-W4 ADG-3 reproducible from pinned inputs, fails on drift | drift: `spikes/adg-3/adg3-classfile-extraction.json` deviation 1 (observed refusal). Reproducibility: **nothing** | **partly unproven — F4** |
| M1-W5 ADG-4 duplicated state and boundary traffic measured | 3 records in `spikes/adg-4/`; ADR `docs/decisions/adg-4-state-ownership.md:3` | proven; work confound and declared bytes disclosed |
| M1-W6 ADG-5 stock launch, no other loader, no tokens | `spikes/adg-5/adg5-stock-jvm-arguments.json` `prismIntegration.stockLaunch: true` + redacted-log hash | attested, not recomputable — F5 |
| M1-W7 synthesis traces to Linux evidence | `docs/architecture/overview.md:33-70`, `dependency-rules.md`, `safety-boundary.md:16` | proven (concluded from records, each traceable) |
| M1-W8 independent review confirms, M1 gate passes | `evidence/m1/manifest.json:239` `result: "fail"`; record digest names no file | **unproven — F1, F2** |

## Findings

**F1 — blocking.** M1-W8 is unproven by its own record.
`evidence/m1/manifest.json:239` records `fail`; `ravel:verify-milestone:m1` exits
1 with exactly one violation, `milestone review must be independent and pass`.
Every other M1 gate rule passes. The resolution in favour of the deviation was
correct.

**F2 — blocking for checkability.** `evidence/m1/manifest.json:240-243`:
`review.record.name` is a title ("M1 independent architecture review"), not a
path. `schemas/evidence.schema.json` types it as a free-text `hashedArtifact`
name, and `tools/ravel-xtask/src/lib.rs:900-905` checks only `has("record")`. So
the sha256 binds nothing obtainable: no reader — including me — can confirm the
review happened, who ran it, what it found, or that the digest hashes any
artifact at all. `evidence/m1/README.md:24-28` states this plainly, to its
credit. The same shape appears at `evidence/m0/manifest.json` and
`evidence/m2/manifest.json`, where `result` is `pass` — two milestones are
therefore closed on a review claim nobody can check. M3 dropped `record`
entirely, which is at least not a false comfort.

**F3 — medium, undisclosed instance of a disclosed class.**
`evidence/m1/manifest.json:16` pins `revision.id` `909240424db4`. That commit is
a predecessor state of change `potzkxqsxykq` ("chore: add a nested-display launch
probe"), a **descendant** of `rxovtruttmyk`, the change that holds the manifest,
and `evidence/m1/manifest.json` does not exist at it. That is precisely the
defect `README.md:199-206` calls "worse" about ADG-5, and it is disclosed for the
seven spike results at `manifest.json:198` but not for the manifest itself. It
matters more here: `verify_recorded_inputs`
(`tools/ravel-xtask/src/lib.rs:258-302`) *reads* `revision.id`, and its doc
comment (`lib.rs:243-249`) asserts it "names the commit the recorded commands ran
on". No live drift results — all five input digests match there — but the
provenance sentence is not true of this manifest.

**F4 — medium.** M1-W4's "reproducible from pinned inputs" is concluded, not
observed. No M1 record compares two runs of one generator;
`bindingGeneration.outputSha256` is written once per candidate, and the two
extractors' outputs **differ** (`0ecc36a9…`/3417 bytes vs `8b093021…`/3413
bytes), so the cross-extractor digest cannot stand in. `README.md:47`'s "two
independent extractors resolved all six targets identically" is lifted from
`operability.diagnostics`, a prose string in both records; no per-target list is
recorded, so the word is *attested*, not recomputable. Compare M2, which carries
an explicit `repeatedRunIsIdentical` field (`lib.rs` `M2_RECORDS`). Drift refusal
is genuinely observed, though induced by pinning an all-zero hash rather than by
a changed input.

**F5 — medium.** `adg5-stock-jvm-arguments.json` `prismIntegration.detail`
asserts "the pinned game launched with both arguments on its command line and no
other mod loader named." The recorded proof
(`experiments/adg-5-prism/src/launch.rs:87-91`) is three substring tests. Nothing
establishes the version or that the arguments sat on the game's command line.
`README.md:178-185` says so; the record does not, and `lib.rs:1000-1012` reads
only `stockLaunch == true`. The log is untracked, so this is *attested*. The
README's honest paragraph should be in the record.

**F6 — low.** `README.md:96-98` says the passes checked "all thirty-one recorded
deviations". The corpus holds 36 today (27 in spikes + 9 in the manifest) and held
34 at the pinned revision. No state of this evidence has 31. Nothing depends on
it, but it does not reconcile.

**F7 — low, disclosed.** "allocates nothing" (`README.md:43`) rests on
`allocatedBytesPerCall: 0.0` marked `measured`/`estimated` while
`allocationsPerCall` is `unavailable`; and `adg1-agent-java-callback.json` records
16 bytes/call for a callback that crosses no boundary.

## Inherited FAIL findings — fixed, with the file:line

Only three of the ten are identifiable from `README.md:99-113`; I verified all
three.

1. Gate-vs-ADG-2 contradiction (the decisive cold finding): fixed at
   `docs/architecture/m1-measurement-gate.md:79-82` — the dynamic library is now
   explicitly retained with the reason and the cost of selecting it, and mirrored
   as risk 3 at `manifest.json:229`.
2. Unsafe exception withdrawn: `docs/architecture/safety-boundary.md:16` (empty
   table), `ROADMAP.md:402-406`, and now re-derivable in production at
   `crates/ravel-bootstrap/src/lib.rs:1,30-31`.
3. Input-digest drift: `manifest.json:43` now `f6509fba…`. I hashed the gate doc
   at every revision that ever touched it — `f6509fba…` at `21377e074fc0`,
   `62eba2a1…`, `d4b5d4a4…`. The withdrawn `2bfce261…` matches none of them.
   **The disclosure at `manifest.json:210` is honest and verified.**

The remaining seven findings are enumerated nowhere in the repository.
`README.md:113` says "All ten of its findings were resolved" and lists three. That
is itself a gap: the claim is unauditable.

## Digests

**45 recomputed, 45 matched.** 20 `benchmarks.results[]` against the working copy
(20/20); the same 20 against the pinned revision `909240424db4` via
`jj file show` — byte-identical (20/20); 5 `inputs[]` against the pinned revision
(5/5). Plus 3 historical hashes of the gate doc to test F-fix 3.

Not recomputable: `review.record` (no file exists), `commands[].logSha256` /
`argvSha256` (no logs tracked), ADG-5 `artifacts[]` and every spike `artifacts[]`
(untracked local fixtures and build outputs), `tests: 135/135` and `84.36%` (a
historical run; the current gate is exit 0 but reports its own counts).

## Coverage

Read: `ROADMAP.md` M1 rows 587-594 and 720-722, `evidence/m1/README.md` in full,
`evidence/m1/manifest.json` in full, all 20 spike records programmatically plus 4
in full, `schemas/evidence.schema.json` review block,
`tools/ravel-xtask/src/lib.rs:195-1136` (all M1 gate rules and the
digest/provenance recompute), `docs/architecture/m1-measurement-gate.md:60-100`,
`safety-boundary.md`, `overview.md`, ADR status lines,
`experiments/adg-5-prism/src/launch.rs`. Ran `ravel:verify-milestone:m1` (exit 1,
one violation) and `ravel:verify-spikes` (exit 0). Reconciled every count and
figure in the README against the JSON — 20 results, 9 deviations, 4 risks,
10.02/2.50/1.88/10.64/3167 ns, 1.09 ns spread, 2.34 vs 2.97 mod-to-mod, batches
of 64 and 8, 0.6–13.6 ms and 50.7 ms timed regions, 12.5 ns and ~266,000
crossings/frame: **all correct.**

Not reached: the full text of the five ADRs beyond status lines and the ADG-2
table; the spike harness sources for ADG-1 through ADG-4 (so the "declared bytes"
and RSS-sampling deviations are taken on the README's word, which is the
conservative direction); `ravel:verify` and `verify-stack` (reported exit 0 by the
requester, not re-run); the seven unenumerated cold findings.

## What a PASS would have required

Not "fix the manifest field". Two things: a review record that can be obtained and
hashed by someone who did not write it — F2 makes M1-W8 unauditable in principle,
and it silently affects M0 and M2 as well — and a recorded reproducibility
observation for ADG-3 (F4), which is the one place a ROADMAP Verification clause
asks for something the evidence does not contain.
