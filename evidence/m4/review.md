# M4-W8 Independent SDK Review

Result: FAIL

**No independent review round has run against M4.** That line above is not a
verdict. It is the closure state `evidence/m4/manifest.json` records under
`review.result`, and `schemas/evidence.schema.json` admits exactly three values
for it — `pass`, `fail` and `not-required`. M4 is open, its review is required and
has not happened, and `fail` is the only one of the three that is true of a
milestone in that state. `verify_recorded_verdict` in
`tools/ravel-xtask/src/lib.rs` requires this document's first `Result:` line to
agree with that field, so the line exists to satisfy the agreement rule and says
nothing about any reviewer's conclusion, because there is no reviewer's conclusion
to say anything about.

The line changes to `PASS` only when an independent round returns a pass on the
state it read, and it changes at the same time as `review.result` and as the
verdict that round wrote, below.

## What this file is

The record the independent M4-W8 review's rounds are written into. It is one file
rather than one file per round because `review.record` in
`schemas/evidence.schema.json` binds one hashed artifact per milestone, and the
rounds only mean anything read in order — each verifies the repairs answering the
one before it. `evidence/m3/review.md` is the same shape and is worth reading
first for how a round is reproduced here.

Its digest is recorded in `evidence/m4/manifest.json` under `review.record`, and
`verify_recorded_review` recomputes it on every `ravel:verify`. So writing a round
into this file means rebinding that digest in the same change.

## The convention this file is held to

A round's verdict is written into this file **before, or in the same change as,
the repairs that answer it**. M3 learned this the hard way: two rounds had their
repairs landed with their verdicts left out, a later round blocked the milestone
over exactly that, and both texts were recovered from outside the repository only
because they still happened to exist. That was luck rather than a control, and
this file inherits the convention rather than the luck.

Each verdict is reproduced as written, without softening and with nothing added.
`file:line` anchors are as of the tree each reviewer read; later repairs move
some of them.

This file's completeness can only ever be as of its last write. The repository
cannot know a round has run until someone writes it down, so between a reviewer
finishing and its verdict landing here the file is missing a round and nothing in
the gate can detect it. The convention above is the only bound on that window and
it is a convention on the writer, not a check on the tree.

This file is the one place that says how many rounds have run. No other document
may restate that count — `verify_review_round_counts` in
`tools/ravel-xtask/src/lib.rs` enforces that for M2 and M3, and the same reason
applies here: a count restated somewhere else is a count that goes stale
somewhere else.

## What a reviewer is asked to read

- `evidence/m4/manifest.json` — the inventory, the recorded gate run, and every
  deviation and risk M4 records, including the ones that name what M4 does not do.
- `evidence/m4/linux-process-matrix.json` — every gate process, its exit status,
  and what each one does not prove. `gameLaunch.occurred` is `false` and that is
  the record's central claim about its own scope.
- `evidence/m4/sdk-surface.json` — what each work package shipped, what proves it,
  and what it leaves unproven.
- `evidence/m4/README.md` — how to recompute the above.
- `docs/guide/` — the author-facing surface M4-W7 created, `limits.md` in
  particular.
- `ROADMAP.md`'s M4 table.

## Rounds

*None recorded.*

<!--
A round is added as a `## Round N — VERDICT: <PASS|FAIL>` section holding the
reviewer's text verbatim, in the order the rounds ran. The heading above is
replaced by the first one. Rebind `review.record.sha256` in
`evidence/m4/manifest.json` in the same change.
-->
