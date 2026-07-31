# Architecture experiments

M1 experiments live here and must use the common benchmark/evidence contract.
No experiment is a production dependency before its architecture decision is
accepted. Measurement that a later milestone owes belongs here too, for the same
reason: it is an instrument, and instruments stay out of the production graph.

## The experiments

| Experiment | Gate | What it measures |
| --- | --- | --- |
| [`spike-harness`](spike-harness/) | all | the shared measurement contract itself |
| [`adg-1-jvm-entry`](adg-1-jvm-entry/) | ADG-1 | four ways into the game's JVM, and the cost of a hook callback |
| [`adg-2-guest`](adg-2-guest/) and [`adg-2-mod-execution`](adg-2-mod-execution/) | ADG-2 | one mod body bound four ways: dynamic library, WebAssembly, helper process, linked in |
| [`adg-3-bindings`](adg-3-bindings/) | ADG-3 | two readings of the pinned jar, and whether a drifted input is refused |
| [`adg-4-state`](adg-4-state/) | ADG-4 | one object flow under three owners |
| [`adg-5-prism`](adg-5-prism/) | ADG-5 | what the pinned launcher can be asked to do, without launching it |
| [`m2-loader-overhead`](m2-loader-overhead/) | M2-W8, ADG-2 | what the pre-world loader costs, and what a linked pack-sized set costs to build |

Each ADG experiment has a task: `devenv tasks run ravel:spike-adg-<n>`. None of
them authenticates or launches the game. `m2-loader-overhead` belongs to a work
package rather than a gate and runs as
`devenv tasks run ravel:measure-loader-overhead`; it writes an M2 record instead
of a spike result, because the budget it sets is not a candidate comparison.

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
