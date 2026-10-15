# M2-W8: loader overhead and the regression budget

What Ravel's pre-world loader costs on Linux, what a linked pack-sized set costs
to build, and the numeric budget those measurements set for M3.

```console
cargo make ravel:measure-loader-overhead
```

It writes [`evidence/m2/loader-overhead.json`](../../evidence/m2/loader-overhead.json)
and nothing else. Run it on the `linux-reference-v1` hardware profile; a budget
without the machine it was measured on is not a budget, and the record names the
profile and its digest so a later run can tell whether it is comparable.

## What it measures

**The loader.** The tracked no-op package driven through
`ravel-testkit`'s pipeline — the same composition
`tests/conformance/tests/integration/noop_pipeline.rs` records — with manifest parsing,
integrity verification and resolution also timed on their own. Then the same
pipeline over 16, 64 and 256 copies of that package under distinct identities,
which separates what the loader costs per package from what any one package
costs. Percentiles are the shared nearest-rank definition from `spike-harness`,
and the budget is set from the 99th rather than the median to account for tails.

**The build.** The mod-execution spike measured none of the pack-sized link time,
binary growth, or compile time, because its linked trial linked one mod. This
generates a workspace of 256 one-struct mod
crates and a host that links them, builds it cold, builds a host with no mods
cold for the baseline, and rebuilds once after a single mod's source changed.
The generated workspace lives under `target/`, is never tracked, and nothing
production reads it.

## What it does not do

Nothing here loads a dynamic library, starts a guest runtime, spawns a
mod process, or exports a foreign symbol. The mods it generates are ordinary
safe Rust behind the same `HostMod` contract the in-process no-op package
already uses, and the record says in its own words that a linked shape was the
only one available rather than a preferred one.

It runs no JVM and launches no game. Reaching the loader from the game costs
what the JVM-entry spike measured and the two add; the launch itself is recorded separately in
[`evidence/m2/prism-launch.json`](../../evidence/m2/prism-launch.json).

The record carries its own list of what the numbers do not say — a cold page
cache, a real dependency graph, and a mod body that does something are all
outside it, and every figure is therefore a floor.
