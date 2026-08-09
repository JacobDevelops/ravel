# M1 Independent Review

Result: PASS

Reviewer: independent cold auditor, no part in the work under review. Scope: M1
closure — the `ROADMAP.md` M1 rows and their Verification column, `evidence/m1/`
in full, the M1 gate rules in `tools/ravel-xtask/src/lib.rs`, the ADG-3
instrument under `experiments/adg-3-bindings/`, and the spike and evidence
schemas as the gate exercises them.

The verdict is the reviewer's, reproduced without softening, and the six findings
below are theirs verbatim with their severities. It is pinned to change
`mwvktlzu`, which was commit `8ee7146b` when the review ran, with
`evidence/m1/manifest.json` at sha256 `98565d0cef18390061c1…`. Neither the commit
id nor that digest resolves any more: the verdict is conditional on F1 being
fixed in the commit that closes M1, and fixing it rewrote the change. The change
id is the durable half and is what to read this record against. The `file:line`
anchors are as of the tree the reviewer read, so a line may have moved even where
the finding still stands.

## Verdict

PASS for M1 closure, conditional on F1 being fixed in the same commit that closes
M1.

The baseline the reviewer reproduced: `ravel-xtask verify milestone M1` exits 1
with exactly one violation, `milestone review must be independent and pass`; every
other M1 rule passes; `verify spikes` and `verify scaffold` exit 0.

## What the review established by breaking or re-doing

- All five `inputs[]` digests recompute at `f0af4331…`, the visible commit of
  change `rxovtrutt…`. Three mutations of `revision.id` and one zeroed input
  digest each exit 1 fail-closed; a benchmark-result digest flip exits 1.
- ADG-3 reproducibility re-derived independently: the instrument was built and run
  three times from the pinned inputs and produced `outputSha256`
  `b9603d93d5717d9876ecaa95c958d18b8f695c2e55ecba0ac78dec040b6c5a4b` every time,
  byte-identical to the tracked record. The re-written record differed only in
  `durationMs`, the two RSS figures, `commandSha256`, `startedAt` and
  `trial.revision`. 37 resolved targets, five failed-closed scenarios, the drift
  refusal and both artifact digests all reproduced.
- With `tests/fixtures/local/` moved away the runner exits 1 ("the pinned
  client.jar is absent") and writes no record, leaving the existing record
  byte-unchanged. It cannot fabricate a measurement.
- Appending one line to `evidence/m1/review.md` makes `verify scaffold` exit 1 on
  the digest mismatch.
- `ravel-composed-host` builds a cdylib exporting 43 `Java_dev_ravel_jvm_Hook_*`
  symbols from crates all carrying `#![forbid(unsafe_code)]` on line 1. All 18
  crate roots carry it, and `safety-boundary.md`'s exception table is empty.
- Redaction holds: an injected token plus home path makes `verify scaffold`
  exit 1.
- `review.independent: false` with `result: "pass"` still fails; a null
  `review.record` fails on schema; emptying `deviations` or `risks` fails; setting
  `repeatedRunIsIdentical` to false on both re-measured records fails the M1 rule.

## Arithmetic

Recomputed and correct: median 10.015625 → "10.02 ns"; p95/p99 12.53; ADG-2
median 2.5; 10.02 + 2.5 = 12.52 → "~12.5 ns"; 10% of 33.3 ms ÷ 12.5 ns = 266,400
→ "roughly 266,000 a frame"; ADG-2 spread 3167.25/1.875/2.5/10.640625; ADG-4
medians span 1.09375 → "within 1.09 ns"; ADG-3 extractor delta
18946 − 18942 = 4 = len("classfile") − len("javap").

## Whether an unreproducible measurement can close a Verification clause

The review answered the question it was asked. Yes, conditionally: the record must
state what was and was not observed, the instrument must be tracked so that any
operator holding the restricted material can re-derive it, and the recorded fields
must be observations rather than conclusions. ADG-3 meets the first two. F2 is
where it meets the third only partly. The clause closes on `outputSha256`, which a
jar-holder can refute, not on the boolean.

## Findings

**F1 — MODERATE.** M1 passes only with this fixed.
`evidence/m1/manifest.json:236-244` and the review block near
`tools/ravel-xtask/src/lib.rs:900-905`. Nothing compares `review.result` to the
verdict stated inside the bound `review.record`. The reviewer set
`.review.result = "pass"` while `evidence/m1/review.md` still read `Result: FAIL`
on line 3 and `verify milestone M1` EXITED 0. Fix both halves: (a) record this
review properly — replace `evidence/m1/review.md` with this round's record,
stating the PASS verdict, all six findings verbatim with their severities, and the
reviewer's reading-only ceiling list in full (it is part of the record, not an
appendix to discard); rebind `review.record.sha256` to the new bytes; set
`review.result` to "pass" only because the record now says so. (b) Add a gate rule
that the bound record must state the same verdict as `review.result` — parse a
`Result:` line or equivalent front matter and compare — so agreement is
machine-checked and a future string edit cannot close a milestone. Prove (b) by
mutation both ways: result "pass" against a record saying FAIL must exit nonzero,
and the reverse too.

**F2 — MODERATE.** `evidence/m1/manifest.json`'s ADG-3 re-measurement deviation,
`docs/decisions/adg-3-bindings.md:64-66`,
`experiments/adg-3-bindings/src/bin/generate.rs:56,70` and `src/lib.rs:448-480`.
`bindings::resolve` runs once at `generate.rs:56`; the repeat at `:70` re-invokes
`bindings::generate`, a pure render over an already-computed `&[Resolved]` with no
hash-ordered container in the file (one `BTreeMap` at `lib.rs:293`). So
`repeatedRunIsIdentical` cannot be false once the first generation succeeded — it
witnesses renderer purity, not re-derivation from the pinned inputs, which is
where nondeterminism would live. The record also discards the second digest, so
the comparison the ADR credits is absent from the artifact. The reviewer proved
the rule is satisfiable by hand: adding the field to the M1-era
`adg3-classfile-extraction.json`, which never repeated anything, left
`verify spikes` at exit 0 and the M1 rule passing. Fix: record
`repeatedOutputSha256` beside `outputSha256`, have the gate require the two equal
(this replaces the boolean as the load-bearing check — keep or drop the boolean as
you judge, but the digest is what must be gated), and move the repeat ABOVE
`resolve` so it re-reads the pinned jar. Narrow the manifest and ADR prose to what
is actually observed. Re-measure both extractors afterwards and rebind digests. If
moving the repeat above `resolve` makes reproducibility fail, that is a finding —
report it and do not paper over it.

**F3 — MINOR.** `evidence/m1/README.md:104`, `:231`, `:236` say "thirteen of the
twenty", "seven of the twenty results disagreed", "changeId is correct on all
twenty". There are 22 spike records and 22 `benchmarks.results[]` entries — the
`-remeasured` pair made three denominators stale. Drop the denominators or make
them recomputed; do not just bump 20 to 22, because nothing fails when they drift
again. The substantive "seven" is still correct.

**F4 — MINOR.** `evidence/m1/manifest.json` revision block: `revision.id`
`f0af4331…` does not hold the two `-remeasured` records nor the deviations
describing them. The existing disclosure covers `commands[]` and tests but not
`benchmarks.results[]`. Extend it by one clause.

**F5 — MINOR.** `evidence/m1/manifest.json` has `attestations: []` while M1-W6
rests on an operator-run launch, and M2's gate requires exactly such an
attestation for its operator-run launch. Add a Linux attestation entry for ADG-5
consistent with how M2 does it. Separately `ROADMAP.md:594` still says the review
record "is held outside the repository", which the now-tracked `review.md`
contradicts — fix that cell.

**F6 — MINOR.** `evidence/m1/README.md:161-163` frames the materiality rate
against a 33.3 ms frame (30 fps); at 60 fps the same 10% is reached at roughly
133,000 crossings, half the headline. Give both figures or say why 30 fps is the
reference.

## Ceiling — what this review did not check

The reviewer's own list, in full, of what was read rather than exercised:

- The tracked-JAR rule (`lib.rs:2768-2823`) is UNTESTED. An attempted mutation
  could not get jj to track the planted `.jar`, so `verify scaffold`'s exit 0
  proved nothing, and the reviewer withdrew a finding rather than assert it.
- Every `commands[]` figure is unrecomputed.
- The ADG-1/2/4 harness sources were not run, so their RSS-sampling and
  declared-bytes deviations, and the estimated-quality and 1/64-ns quantization on
  every median, are taken on the record's word.
- ADR prose beyond the quoted passages is unverified rather than merely read.
- Both schemas were exercised through the gate but not audited as text.
- The clone-reachability claim was not re-checked with a fresh clone.
- `ravel:verify`, `verify-stack`, coverage and audit were not run, because they
  write to the tree.

## Disposition

All six findings were fixed in the stack that closes M1, F1 in the same change
that sets `review.result` to `pass`. That is the repository's claim and not the
reviewer's: this record is the verdict as delivered, and the evidence that each
fix holds is the mutation proof in the change that made it, not a sentence here.
