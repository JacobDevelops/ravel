# M3-W6: what opt-in profiling costs when it is off

M3-W6's row closes on three things: per-mod attribution accuracy is explicit,
unsupported metrics are explicit, and the normal-mode budget passes. The first
two are statements the collectors make about themselves and live in
[`ravel-observability`](../../crates/ravel-observability/). This experiment
measures the third.

```console
devenv tasks run ravel:measure-profiling-overhead
```

It writes
[`evidence/m3/profiling-overhead.json`](../../evidence/m3/profiling-overhead.json)
and nothing else. Pass `--output <path>` to write elsewhere; a later run must be
able to write its own record rather than overwrite the one the limit is read
from. Run it on the `linux-reference-v1` hardware profile, whose digest the
record carries so a later run can tell whether it is comparable.

## The budget is decided rather than derived

Every limit in [`evidence/m2/loader-overhead.json`](../../evidence/m2/loader-overhead.json)
is its own measurement doubled. This one is not.

> **Profiling-off costs at most 1% of a tick.** At 20 TPS a tick is 50 ms, so
> the limit is **≤ 500 µs of added time per tick, summed across every crossing
> that happens in that tick**, measured with profiling not enabled.

It is fixed in advance because what a player is promised is that profiling costs
them nothing when it is off, and that promise is a fraction of a tick rather than
whatever today's code happens to do. A measurement far under the limit is the
expected result and not a sign the limit is wrong.

It is anchored to the tick rather than to the loader pipeline because profiling
is sampled per tick, and it is stated as a per-tick total rather than a
per-crossing limit so that it does not depend on the crossings-per-tick count,
which nothing has recorded yet.

**Profiling-on is measured and not bounded.** What the collectors cost while they
run depends on how many mods and collectors are attached, and none of that exists
at M3. M9 sets that number against a real pack.

## What it measures

Three figures, each a *paired* difference. One sample times a thousand
instrumented calls and a thousand bare calls and records the gap between them,
alternating which of the two runs first. Differencing two separate runs'
percentiles would not be a percentile of anything, which the M2 record already
names as the weakness of its residual stage; pairing inside the sample makes the
distribution the distribution of the overhead itself.

| Figure | What it is |
| --- | --- |
| `off.perCrossing` | what one instrumented mod call adds with the collectors off — one flag test, no clock read, no map touched |
| `off.perTickBoundary` | what one tick boundary adds with them off, which is the same flag |
| `on.perCrossing` | what one instrumented call adds with them running: two clock reads and a map update |

A sample whose instrumented batch finished ahead of its control is recorded as
zero rather than as a negative time, so the figure is biased upward. That is the
safe direction for a limit.

## The per-tick figure and the one that replaces it

The budget is per tick and the measurement is per crossing, so the record forms
`perTick` from an **assumed** crossing count of ten thousand. That number is
stated as an assumption every time it appears. It is a ceiling rather than a
prediction: a 256-mod set would have to be entered thirty-nine times each in one
tick to reach it.

Beside it the record carries `breakEven.crossingsPerTick` — how many crossings a
tick would need before profiling being off costs a whole per cent of it. That
figure rests on no assumption at all, and it is what answers the budget outright
once the real crossing count is recorded: compare the two numbers.

## What it does not do

It runs no JVM, opens no world, and ticks no server. The tick is arithmetic over
measured per-call costs rather than an observed tick, and the crossings it times
are safe Rust calls through a trait object in one process — what profiling *adds*
to a crossing, not what a crossing costs. The record's `whatThisDoesNotSay` list
carries the rest.
