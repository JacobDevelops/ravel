# Architecture experiments

Maintained measurement instruments live here and use the common
benchmark/evidence contract. Instruments stay out of the production graph.
The six ADG-1 through ADG-5 source directories and their measurement tasks are
retired; their saved results remain in
[`evidence/m1/spikes/`](../evidence/m1/spikes/), with launch provenance also
retained under [`evidence/m2/`](../evidence/m2/).

## The experiments

| Experiment | Gate | What it measures |
| --- | --- | --- |
| [`spike-harness`](spike-harness/) | all | the shared measurement contract itself |
| [`m2-loader-overhead`](m2-loader-overhead/) | M2-W8, ADG-2 | what the pre-world loader costs, and what a linked pack-sized set costs to build |
| [`m3-pack-compile`](m3-pack-compile/) | ADG-2 | what a pack-sized set of mods with real bodies costs to compile, cold and cached |
| [`m3-profiling-overhead`](m3-profiling-overhead/) | M3-W6 | the cost of opt-in per-mod counters and attribution |
| [`m6-outward-crossing`](m6-outward-crossing/) | M6 | three ways for an action to reach the running game |

`m2-loader-overhead` belongs to a work
package rather than a gate and runs as
`cargo make ravel:measure-loader-overhead`; it writes an M2 record instead
of a spike result, because the budget it sets is not a candidate comparison.
`m3-pack-compile` runs as `cargo make ravel:measure-pack-compile` and
writes an M3 record for the same reason: it prices source compilation
rather than comparing candidates.
`m3-profiling-overhead` runs as `cargo make ravel:measure-profiling-overhead`;
its record is also checked by `ravel:verify-budget`. `m6-outward-crossing` runs as
`cargo make ravel:measure-outward-crossing` and writes the M6 comparison record.

## Common contract

[`spike-harness`](spike-harness/) implements the M1-W1 measurement contract from
[`docs/architecture/m1-measurement-gate.md`](../docs/architecture/m1-measurement-gate.md):
one percentile definition, one memory collector, one trial protocol, and one
result shape validated against
[`schemas/spike-result.schema.json`](../schemas/spike-result.schema.json).

A trial child prints one JSON report line, then parks on standard input so the
harness can read `/proc` while the process is still alive. A missing metric is
recorded as `unavailable` with a reason; it is never reported as zero.

The saved ADG results in `evidence/m1/spikes/adg-<n>/<candidate>.json` remain
checked by `cargo make ravel:verify-spikes`. This validates recorded evidence;
it does not require the retired candidate sources or rerun their measurements.

## Boundaries

These crates are measurement instruments, not production code:

- no production crate may depend on one, and `ravel:verify-scaffold` enforces it;
- measurement instruments are excluded from the workspace coverage floor where
  their workloads need a JVM, a launcher, or a game. The shared harness is not
  excluded: it is ordinary logic and earns the floor like anything else;
- they may contain the `unsafe` a candidate needs in order to be measured at all,
  while every maintained crate keeps `#![forbid(unsafe_code)]`.

## Pinned inputs

Trials read pinned versions from
[`tests/fixtures/descriptors/runtime-inputs-1.21.1.json`](../tests/fixtures/descriptors/runtime-inputs-1.21.1.json).
Set `RAVEL_SPIKE_JAVA` to the pinned Mojang runtime's `java` binary inside the
ignored launcher root to measure on the exact game runtime; without it a trial
falls back to the development JDK and records the difference as a deviation.
