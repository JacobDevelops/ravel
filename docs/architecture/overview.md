# Architecture overview

Ravel is a Rust mod loader and SDK hosted initially inside official Minecraft:
Java Edition. Discovery, package validation, dependency resolution, lifecycle
orchestration, version adapters, safe author APIs, registries, configuration,
saved data, diagnostics, profiling, and pack validation are Ravel
responsibilities. Authentication, entitlement, game installation, instance
management, downloads, and general launcher UI remain with Prism Launcher or
their existing owners.

The required lifecycle is:

```text
Discovered -> Validated -> Resolved -> Loaded -> Constructed
  -> Registered -> Running -> Stopping -> Stopped
```

`Failed` is terminal. No mod code runs before validation and resolution,
registry identifiers freeze at the registration barrier, required failures
prevent world access, side-incompatible code never loads, and lifecycle order
is deterministic for an identical package set.

Minecraft-specific hooks, mappings, and bindings live behind an explicit
version adapter. General loader, package, SDK, and pack contracts must not
encode any Minecraft version's internals.

Official-server interoperability is a later protocol layer; it does not turn
Ravel into a Java mod host.

## Mechanisms

- **JVM entry.** A `java.lang.instrument` premain agent rewrites hook targets with
  the runtime's `java.lang.classfile` API and binds callbacks through generated
  JNI-mangled exports; the generated `Hook` class owns native loading and the sole
  native declaration surface.
- **Entry-point initialization.** The agent jar joins the bootstrap loader's
  search path so class loaders share one copy of Ravel's classes.
  The first distribution entry point names the running side and initializes the
  Rust runtime; `premain` does not infer the side.
- **Refusable prologues.** An inventory-opted-in prepend on a void target can
  return early when its hook answers a boolean refusal verdict.
  The generated target data bounds that power, and non-void targets cannot use it.
- **Mod execution.** Mods ship independently as portable Rust source, and a
  Ravel-supplied toolchain compiles the installed set on the player's machine.
  Compiled mod artifacts are not distribution inputs, so Ravel owes no stable mod ABI.
- **Bindings.** Build-time generation resolves each reviewed hook exactly once
  against hash-pinned class files and records the result in `mappings.lock`.
  Names come from unobfuscated class files or the version's official ProGuard
  mappings; input drift or a mismatched shape prevents generation.
- **State ownership.** Minecraft keeps its state; hooks receive values or
  call-scoped opaque `JavaHandle` references and return results without retaining
  a mirror. A mirror needs measured justification and a way to detect divergence;
  batching reduces boundary traffic without duplicating state.
- **Launcher integration.** Prism owns authentication, downloads, Java selection
  and launch; Ravel uses stock instance arguments rather than a launcher fork.
  The generated `Hook` loads the native library with `System.load` from the
  absolute path named by `-Dravel.host`, beside the `-javaagent` argument.
- **Configuration and screens.** Mods declare configuration as data, and Java
  draws every Ravel-owned screen from declarations and values supplied by Rust.
  Values resolve from author defaults through per-setting pack overrides to
  player values, with `locked` or `required` pack values suppressing the player
  layer; the per-frame extraction hook observes rather than draws.
- **World persistence.** Minecraft stores one opaque Rust-owned, owner-sorted
  canonical JSON aggregate in one `SavedData` object, bounded to 1 MiB before
  parsing or replacement, through batched open and save crossings.
  Rust owns admission, migration and staged/live world identity; Java owns game
  calls and vanilla file handling, and a failed replacement preserves the prior
  aggregate without aborting vanilla saving.
- **Outward actions.** Rust answers an inbound crossing with data describing an
  action, and the receiver-aware Java adapter performs it against the game.
  Rust does not call game methods, and `JavaHandle` exposes no such capability.
- **Claims and checks.** Measured claims have an independent reading proved by
  mutation; judged claims carry per-input verdicts, reasons and bases in data,
  checked for two-way congruence with behaviour, and mechanisms disclose their
  actual bounds rather than asserting completeness.
  From M5 onward, deviations naming detectable Rust symbols carry checked
  caller-scope anchors; this does not turn the whole judgement into a measurement.
- **Evidence classes.** Material digests freeze findings, while machinery
  digests describe the measured tree and can be rebound with an amendment trail;
  later dispositions belong to later records rather than frozen findings.
  Closed inventories retain the names they measured without claiming to
  enumerate later additions.
- **Counterpart identity.** Counterparts use their own Ravel-native identity;
  load-time identity mapping belongs to the host.
  In the private compatibility workspace, a genuine compatibility alias lives
  only in `counterparts/<package>/aliases.json`, whose rows contain the upstream
  id, native id and canonical replacement id.
- **Toolchain and cache.** Player mod builds share one exactly pinned toolchain
  and one reconstructible Cargo build cache per machine across instances.
  Cargo's cache keys separate different builds, and its lock serializes concurrent
  access; cache size and eviction remain open work.
- **Acceptance runs.** A run perceives structured `GameEvent`/`Observation` data
  by registered content keys and acts through `Directive` data carried by a
  `GameDriver` over the existing outward instruction crossing.
  Verdict arithmetic lives in the recorded `Transcript`; a live game driver
  remains the launcher-integration dependency rather than a new crossing.
- **Version selection.** One build emits one version's artifacts and reads only
  that version directory; one tracked version defaults automatically, while
  multiple versions require `RAVEL_MINECRAFT_VERSION` and unknown names fail closed.
  The distribution carries artifacts per supported version, and the scaffold
  gate independently sweeps every tracked version directory.

The original Linux measurements live in
[`evidence/m1/spikes/`](../../evidence/m1/spikes/).
The ADG-1 through ADG-7 spike names refer respectively to JVM entry, mod execution,
bindings, state ownership, launcher integration, configuration and screens, and
world persistence above. The world-persistence byte-preservation measurement runs
with `./jvm/harness/run-datafix-byte-preservation.sh`; the outward-action crossing
measurement runs with `cargo make ravel:measure-outward-crossing`.
A primitive crossing's roughly 10 ns median and zero allocation are a warm,
trivial-call floor, not a frame budget or a prediction for a real hook; see
[`evidence/m1/README.md`](../../evidence/m1/README.md).
No maintained crate holds unsafe code; see
[`safety-boundary.md`](safety-boundary.md).
