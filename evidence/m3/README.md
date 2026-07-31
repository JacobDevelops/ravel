# M3 evidence

M3 is in progress. The records below are what it has produced so far; the
milestone manifest that admits them is written when M3 closes.

| Record | Work package | What it shows |
| --- | --- | --- |
| [`pack-compile.json`](pack-compile.json) | ADG-2 | What compiling a pack-sized set of mods with real bodies costs — cold, warm after one mod changes, in peak memory, and on disk. |

`pack-compile.json` comes from `devenv tasks run ravel:measure-pack-compile` and
is the measurement ADG-2 named as deciding its open mechanism: whether an
interchangeable mod ships as a per-platform binary or as source a bundled
toolchain compiles. It replaces the floor M2-W8 set from generated one-struct
crates, and it decides nothing on its own — the generated crates are ordinary
safe Rust behind the same `HostMod` contract the tracked test mods use, and
nothing in the run loaded a library, started a guest runtime, spawned a mod
process, or exported a foreign symbol.

Its `variants` section comes from a second run,
`devenv tasks run ravel:measure-pack-compile-variants`, which changes one thing
at a time about the curve — four build jobs instead of sixteen, a sixteen-deep
dependency spine, and whole-pack link-time optimization — and splices its results
in beside the curve rather than over it. That section carries its own trial
provenance, its own two-build protocol, and its own control, because a figure
measured in a later run is not the same measurement as the one it sits next to.

It is a measurement rather than a contract. Rerunning it moves the numbers, and
they hold on the `linux-reference-v1` hardware profile and nowhere else; the
record carries that profile's digest so a later run can tell whether it is
comparable. Its `whatThisDoesNotSay` list is part of the record, not commentary
on it: the generated mods are alike, have no dependency graph between them, and
pull nothing outside serde, so every figure is a floor.

The record names crates, sizes, versions, and digests. It names no filesystem
path, no credential, and no local directory.
