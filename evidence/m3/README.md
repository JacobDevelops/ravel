# M3 evidence

M3-W0 through M3-W3 and M3-W5 through M3-W6 are closed. M3-W4 and M3-W7 remain
open and the M3-W8 independent prototype review returned FAIL. M3 is not closed
and does not authorize private inventory work.

| Record | Work package | What it shows |
| --- | --- | --- |
| [`manifest.json`](manifest.json) | M3-W0..W8 | Which gates are closed, the W4 and W7 gaps, and the failed independent review. |
| [`pack-compile.json`](pack-compile.json) | ADG-2 | What compiling a pack-sized set of mods with real bodies costs — cold, warm after one mod changes, in peak memory, and on disk. |
| [`profiling-overhead.json`](profiling-overhead.json) | M3-W6 | What opt-in profiling costs a tick when nobody turned it on, what it can and cannot attribute to a mod, and which metrics it cannot measure at all. |
| [`linux-process-matrix.json`](linux-process-matrix.json) | M3-W7 | What a real Linux launch proves about the game directory, the rewrite, and the instance — and, at least as importantly, what it could not reach. |

## M3-W8 review

The independent prototype review returned **FAIL** with four blocking findings.
Two are still open:

- M3-W4 **has now moved** Ravel-owned versioned bytes through an official world
  save, exit, and reload, and a tampered aggregate refused the load. See
  `linux-process-matrix.json` under `worldPersistenceRoundTrip`. The finding is
  kept here because the review that raised it has not been rerun.
- M3-W7 has not exercised every required surface natively. Rewriting all 33
  reviewed hooks proves reachability, not native block, item, recipe, event, and
  resource behaviour. The Ravel-owned saved-data half of this finding is answered
  by the current campaign; the rest is not.

Two are resolved, and stay listed because a finding that disappears once it is
fixed leaves no way to tell it from one that was never raised:

- **Package schema — resolved by `rkrpplvwpklskyuwvksvmuuyklkwsqlt`.**
  `schemas/package.schema.json` required compiled per-target artifacts and could
  not express the source-only distribution contract ADG-2 selected. That change
  migrated schema v1 in place: `targets` is no longer admitted, `source` declares
  a repository and at least one portable source file with its SHA-256 digest, and
  the `unsupported-target` failure kind went with the model it described.
  `SPEC.md` §3.4 and `ROADMAP.md` moved with it.
- **Manifest inventory — resolved.** The first manifest draft omitted M3-W0 from
  `workPackages` even though its gate status appeared elsewhere in the same
  document. It is now in the canonical inventory.

The same review found no defect in hook integrity, safety/layering/ADR conformance,
or evidence redaction. Those clean results do not offset the two remaining gaps.
M3-W8 stays open until the implementation gates close and a later independent
review passes.

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

## `profiling-overhead.json`

It comes from `devenv tasks run ravel:measure-profiling-overhead` and is two
things at once, which is the same shape `evidence/m2/loader-overhead.json` has:
the M3-W6 measurement, and a budget record whose limits a later run is compared
against. `cargo xtask verify budget` takes the union of that file's limits and
this one's, and runs inside `ravel:verify`.

Its `budget` limit is **decided rather than derived**. Every M2 limit is its own
measurement doubled; this one is one per cent of a tick, fixed before the
measurement, because what a player is promised is that profiling costs them
nothing when it is off and that promise is a fraction of a tick rather than
whatever the code happens to do today. A measurement far under it is the expected
result. Only the profiling-*off* figure is bounded: what the collectors cost while
they run is recorded here and gated at M9 against a real pack.

Its `collectors` section is generated out of `ravel-observability` rather than
written, and `tests/conformance/tests/profiling_report.rs` checks it against that
crate. The metric table, the attribution-accuracy statement, and the unsupported
list are the two things M3-W6's row asks to be explicit, and they cannot drift
from the code that ships: a metric that becomes supported, or a reason that
changes, fails the gate until the record is rewritten. The measured half is not
regenerated by that test — a timing differs on every run, and a tracked-file
assertion over one would fail on scheduler noise.

`overhead.perTick` multiplies a per-crossing cost by an *assumed* crossing count,
stated as an assumption everywhere it appears. `overhead.breakEven` beside it
rests on no assumption and is what answers the budget outright once the real
per-tick crossing count is recorded.

## `linux-process-matrix.json`

It comes from launches rather than from a task, because the thing it measures is
a launcher starting a game and no task can stand in for that. It now holds two
launch campaigns, and the most useful thing in it is why the second could not
simply extend the first.

**The earlier matrix could not be carried forward.** Two ADG-7 world persistence
seams entered the reviewed inventory — `world.open-persistence` on
`Minecraft.doWorldLoad` and `world.save-persistence` on
`MinecraftServer.saveAllChunks` — and the callable-target count moved from 31 to
33 over the same 15 classes. A matrix is a claim about one set of targets in one
build, so a count that moved invalidates the reconciliation rather than adding to
it. The second reason is stronger still: no build the earlier campaign ran
contained a mod that owned world data at all. `ravel-composed-host` carried only
the configuration mod until change `uyvzkzzs` added `ravel-test-mod-saved-data`
to it, so the ADG-7 round trip was unprovable by that campaign whatever input it
received — which is why M3-W4 and M3-W7 stayed open through it and why its
saved-data row persisted a vanilla scoreboard.

The current campaign is three launches at 33 callable targets, and its claim is
narrow on purpose. The rewrite of both persistence methods was already proven
in-gate by `jvm/harness/PersistenceMain.java` over eight world lifecycles; what
these launches add is the real game's storage on both ends of it. A world was
created through the game UI, saved, and exited; a second launch reloaded it and
Ravel's own versioned aggregate came back byte-identical, written by the game's
own `SavedDataStorage` as `data/ravel/mods.dat` and carrying `saved_data` at data
version 3. A third launch loaded the same world with that version rewritten to 9
outside the game, and the load was refused before the integrated server started,
with the diagnostic drawn on a Ravel screen. That third trial is what separates a
reload that read the stored bytes from one that ignored them.

The world itself is gone. It was created, hashed, verified across both the reload
and the refusal, and then removed, which returns the instance to the single-world
shape the M0 launch-proof summary expects. Nothing was lost by removing it: the
digest, the reload, and the refused load are the evidence, and evidence that
needs a private machine-local directory to survive is evidence no reviewer can
check.

The earlier campaign's rows — the three Ravel screens, the settings round trip,
the profiling screen, the vanilla command and scoreboard — are kept and labelled
with their provenance. They were not re-observed at 33 targets. Neither campaign
on its own exercises every surface M3-W7 asks for, so the record's result is
`partial` and M3-W7 stays open.

The record separates the operator attestation — injected input followed the
ordinary title-screen and world-selection route — from facts derived from process
state, the shutdown report, persisted files, and the post-exit instance scan. Raw
logs and screenshots remain private; only sanitized facts and their digests enter
the repository.

Four deviations from the earlier title-only record are retained as explicit
resolutions rather than disappearing from the audit trail:

- **Open world — resolved.** The accepted second launch used injected input through
  Singleplayer and the real world-selection screen, then exercised command and
  saved-data paths, saved, relaunched, and verified the exact persisted value.
- **Launch-time caller — resolved.** The source-composed production host populated
  the runtime and configuration stores during premain. The Mods, settings, and
  profiling screens opened from the title menu and consumed production `Hook`
  data.
- **Unaccounted target — resolved.** `lwysptxkplzu` changed class-hierarchy
  resolution to parse resources without loading game classes. Per-class matched
  counts and failure reporting then reconciled `TitleScreen` and every other
  reviewed class: 15 classes, 31 callable methods, zero transformation failures.
  The current campaign reconciles the same 15 classes at 33 callable methods,
  again with zero transformation failures.
- **Title-only/SIGTERM — superseded.** The passing trial opened a world, used Save
  and Quit to Title, selected Quit Game, and let Java and Prism exit naturally.
  Earlier title-only or forced-cleanup runs remain private historical evidence and
  are not the accepted trial.

The game-directory footing is also measured directly: before Ravel was attached,
the stock Prism launch's process working directory was the instance game root and
remained so through the title screen. The post-exit scan found exactly the stock
Minecraft and LWJGL components, no other loader component, and no mod JAR.
