# M4 evidence

M4 turned the prototype's internals into a documented, safe, reusable mod and
pack authoring surface. This directory is what that claim rests on.

Read [`manifest.json`](manifest.json) first. It carries the work-package
inventory, the gate run M4-W8 asks for, and every deviation and risk M4 records —
including the ones that say what M4 does not do, which are most of the
interesting ones.

## The records

| File | What it holds |
| --- | --- |
| [`manifest.json`](manifest.json) | The milestone record. Schema-validated, digest-bound, provenance-checked, and held to its own arithmetic on every `ravel:verify`. |
| [`linux-process-matrix.json`](linux-process-matrix.json) | Every process the global Linux gate runs, its exit status, what it proves, and what it does not. |
| [`sdk-surface.json`](sdk-surface.json) | What each of M4-W1 through M4-W7 shipped, what proves it, what it leaves unproven, and the inventory of crates, test mods, examples, guide documents and conformance suites the gate reconciles against the tree. |
| [`review.md`](review.md) | The record M4-W8's independent review rounds are written into. |

## What this milestone's matrix is, and is not

`linux-process-matrix.json` shares a name with
[`evidence/m3/linux-process-matrix.json`](../m3/linux-process-matrix.json) and
almost nothing else. M3's is an operator-run native launch: a person started the
game, drove the surfaces by hand, and attested to what they saw, and a reviewer
can recompute only the in-gate half of it. M4's is a table of gate processes. Every
row is a command a reader can run, so the whole record recomputes — and it reaches
nothing about Ravel inside a game, because no game process was started for M4.

That is recorded as `gameLaunch.occurred: false`, a field rather than a sentence,
because it is the thing about this record a reader is most likely to get wrong.

## Historical recomputation procedure

The commands below describe the recorded devenv-era gate, not the current
checkout's environment or task graph. Today use `cargo make ravel:verify` and
`cargo make ravel:verify-milestone m4`; do not rewrite the historical matrix to
claim it ran those commands. Performance/resource tooling has been retired.

Everything in the original procedure below ran inside the devenv shell.

```console
devenv tasks run ravel:verify
```

That is the gate `commands[0]` records. The rows under the matrix's `processes`
are the sub-tasks it runs — all of them and nothing else, which
`verify_m4_process_matrix` reconciles against `ravel:verify`'s own `after` list in
[`nix/common.nix`](../../nix/common.nix) in both directions. Each can be run on
its own by the command the matrix names in that row.

`ravel:verify-environment` is not one of them. `ravel:verify` is the fast
code-policy graph and `ravel:verify-environment` runs `devenv test` and
`nix flake check`, which is why the matrix carries it under
`checksOutsideTheGate` and not among the processes the gate ran.

The coverage figures under `gate.coverage` come from the same invocation
`ravel:coverage` makes, read off `cargo llvm-cov`'s own summary:

```console
devenv shell -- cargo llvm-cov nextest --workspace --all-features \
  --summary-only --ignore-filename-regex '(^|/)experiments/adg-'
```

Closure is a different question from the gate:

```console
devenv tasks run ravel:verify-milestone:m4
```

That exited nonzero for the whole life of this milestone, which was the gate
working. It is green now, and it turned green when — and only when — an
independent review round returned a pass with nothing repaired after it and
`review.result` recorded that.

## What the gate binds here

Beyond the rules every milestone manifest is held to, `verify_m4_manifest` in
[`tools/ravel-xtask/src/evidence/m4.rs`](../../tools/ravel-xtask/src/evidence/m4.rs) requires the
full M4-W1 through M4-W8 inventory, both records under the `m4-surface-v1`
protocol so each is covered by a recomputed digest, a `gateStatus` row for every
work package with each implementation row closed and naming its evidence, a native
Linux attestation for the gate run, recorded deviations and risks, and both
decision records M4 rests on.

The checks below exist because a record of in-gate work is the easiest kind to
let drift:

- `verify_m4_gate_figures` reconciles the manifest's `tests` block against the
  matrix's `gate` block field by field. The two documents describe one run, so an
  edit to one without the other is a violation rather than a discrepancy someone
  might notice.
- `verify_m4_surface` compares every list in `sdk-surface.json`'s `inventory`
  against the repository, in both directions. A crate added, a guide page renamed
  or a conformance suite deleted after the record was written fails the gate by
  name.
- `verify_m4_process_matrix` compares the matrix's `processes` against
  `ravel:verify`'s `after` list, in both directions, and refuses a nonzero
  `exitStatus` under a record whose own `result` is `pass`. The first round of
  M4-W8's independent review added an invented process row and flipped a row's
  exit status to 1, and the gate returned 0 for both; this is the repair.

Each is proven by mutation rather than by reading, which is the only proof this
repository accepts that a check is not vacuous.

## The ceiling

Everything M4 proves, it proves in-process, and the suites that prove it were
written by the same work that wrote the surface. The one part of M4 that was not
is the fresh-author run under M4-W7, whose findings are repaired in this tree
rather than summarised here.

An author can build a conforming mod against this surface. An author cannot yet
run one. [`docs/guide/limits.md`](../../docs/guide/limits.md) is where that is
said to an author before they plan around it, and the deviations in
[`manifest.json`](manifest.json) are where it is said to a reviewer.
