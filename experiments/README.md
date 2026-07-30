# Architecture experiments

M1 experiments live here and must use the common benchmark/evidence contract.
No experiment is a production dependency before its architecture decision is
accepted.

## Common contract

[`spike-harness`](spike-harness/) implements the M1-W1 measurement contract from
[`docs/architecture/m1-measurement-gate.md`](../docs/architecture/m1-measurement-gate.md):
one percentile definition, one memory collector, one trial protocol, and one
result shape validated against
[`schemas/spike-result.schema.json`](../schemas/spike-result.schema.json).

A trial child prints one JSON report line, then parks on standard input so the
harness can read `/proc` while the process is still alive. A missing metric is
recorded as `unavailable` with a reason; it is never reported as zero.

Results belong in `evidence/m1/spikes/adg-<n>/<candidate>.json` and are checked
by `devenv tasks run ravel:verify-spikes`.

## Boundaries

These crates are measurement instruments, not production code:

- no production crate may depend on one, and `ravel:verify-scaffold` enforces it;
- the per-gate candidates are excluded from the workspace coverage floor, because
  what they measure needs a JVM, a launcher, or a game. The shared harness is not
  excluded: it is ordinary logic and earns the floor like anything else;
- they may contain the `unsafe` a candidate needs in order to be measured at all,
  while every maintained crate keeps `#![forbid(unsafe_code)]`. Each ADR records
  the unsafe surface a production implementation of its candidate would require.

## Pinned inputs

Trials read pinned versions from
[`tests/fixtures/descriptors/runtime-inputs-26.1.2.json`](../tests/fixtures/descriptors/runtime-inputs-26.1.2.json).
Set `RAVEL_SPIKE_JAVA` to the pinned Mojang runtime's `java` binary inside the
ignored launcher root to measure on the exact game runtime; without it a trial
falls back to the development JDK and records the difference as a deviation.
