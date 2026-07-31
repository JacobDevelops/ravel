# ADG-2: what a pack-sized mod set costs to compile

ADG-2 decided that a mod is independently shipped and interchangeable, and left
the mechanism open. A mod can arrive as a per-platform compiled binary, or as
source that a bundled toolchain compiles on the player's machine. The second
removes Rust's unstable ABI from the problem and gives one portable artifact per
mod; what it costs is a build, and
[the record](../../docs/decisions/adg-2-mod-execution.md) names that build as the
deciding measurement.

M2-W8 measured 880 ms for a 256-mod pack compile and said in its own words that
the figure came from one-struct crates and is a floor rather than a prediction.
This replaces it with a higher floor.

```console
devenv tasks run ravel:measure-pack-compile
```

It writes [`evidence/m3/pack-compile.json`](../../evidence/m3/pack-compile.json)
and nothing else. Pass `--output <path>` to write elsewhere; it never touches
M2's record. Run it on the `linux-reference-v1` hardware profile, whose digest
the record carries so a later run can tell whether it is comparable.

## What it measures

At pack sizes 0, 1, 16, 64 and 256 — the sizes M2-W8's loader scaling curve
used, so the two records line up point for point:

- **Cold compile**, against a cleared build directory. Size 0 is a host linking
  no mod, and is subtracted so the rest is attributable to the mods.
- **Warm rebuild** after one mod's source changes, three times per size. This is
  the number a player pays when updating a mod against a cache.
- **Peak resident memory**, sampled every 25 ms across the whole cargo process
  tree. A build that needs many gigabytes on a player's machine is itself a
  finding, and `getrusage` would report the largest single `rustc` rather than
  what the tree needs at once.
- **On-disk size**: the host binary, the build cache the machine has to keep,
  and the `rustc --print=sysroot` tree a compile-on-install mechanism would have
  to ship.

## The variants

```console
devenv tasks run ravel:measure-pack-compile-variants
```

Three things about the curve above are not what a player gets, and each is
measured at 64 and 256 mods by changing exactly one of them. The run splices its
results into the existing record under `variants` and rewrites nothing else, so
the curve keeps the numbers it was measured with rather than a rerun of them.

| Variant | What it changes | The question |
| --- | --- | --- |
| `independent-j16` | nothing | the control, so the others compare against a run under the same two-build protocol |
| `independent-j4` | `cargo --jobs 4` | what a four-core player pays, in time and in peak memory |
| `chain-j16` | a 16-deep dependency spine | what a graph costs when cargo cannot start a crate before its dependencies finish |
| `lto-fat-j16` | `lto = "fat"`, `codegen-units = 1` | what optimizing the pack as one unit costs, and whether the warm path survives it |

Two warm rebuilds are measured per variant: one changing `mod-000` and one
changing the last mod. Under the chain shape those are the spine's root, which
everything depends on transitively, and a leaf that nothing depends on — the
worst and best cases a player can hit. Under the independent and link-time
shapes they are two ordinary mods, and the pair is a repeat.

Each variant figure is the faster of two builds rather than a median of three,
because a fat-link build of 256 crates costs about two minutes and the matrix is
already forty of them. Both samples are recorded. Compare a variant against the
`independent-j16` control rather than against the curve.

## The generated mod

The gap between M2-W8's figures and reality was the body. Each mod here carries
a configuration type it deserializes from JSON at construction, a graded enum
with match-based methods, an internally tagged result enum, a generic `Rule`
trait with an associated input type, and eight feature blocks — each a
serde-derived input struct, a rule implementation with branching logic, three
registry claims, and a driver. That is `test-mods/registries` scaled up, and it
is scaled by derives and monomorphizations rather than by lines, because that is
what Rust actually charges for.

It is still generated and still alike, so it is still a floor. The record's
`syntheticMod.representativeness` field says how it was chosen and what it
misses, and `whatThisDoesNotSay` lists the rest — no dependency graph between
mods, no dependency tree outside serde, one cold sample per size.

## What it does not do

It selects nothing. The generated crates are ordinary safe Rust behind the same
`HostMod` contract the tracked test mods use; nothing loads a dynamic library,
starts a guest runtime, spawns a mod process, or exports a foreign symbol. The
generated workspaces live under `target/m3-pack-compile`, are never tracked, and
nothing production reads them.
