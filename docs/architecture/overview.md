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
encode Minecraft 26.1.2 internals.

Official-server interoperability is a later protocol layer; it does not turn
Ravel into a Java mod host.

## The selected architecture

M1 measured each mechanism and recorded the evidence in
[`evidence/m1/spikes/`](../../evidence/m1/spikes/); each decision is an ADR in
[`docs/decisions/`](../decisions/). What was selected:

```text
Prism instance (stock, two JVM arguments)
  └── official Minecraft 26.1.2 JVM
        ├── premain agent          rewrites hook targets with java.lang.classfile
        │     └── generated hook class
        │           └── exported JNI symbol ──> ravel-bootstrap (generated, safe)
        │                                          └── loader, resolver, host API
        └── the game's own state, which stays the game's
                                                   └── mod: execution model undecided
                                                         (M3 decides; see ADG-2)
```

- **[ADG-1](../decisions/adg-1-jvm-entry.md): a `java.lang.instrument` premain
  agent, rewriting hooks with the runtime's own class-file API, calling an
  exported JNI symbol.** Chosen over a Rust-hosted VM at equal call cost, because
  it launches on stock JVM arguments and needs no maintained unsafe operation
  rather than two. A primitive boundary crossing costs about 10 ns at the median and allocates
  nothing. That is a floor measured at a warm call site with a trivial body, not a
  prediction for a real hook, and it is not a frame budget: see
  [`evidence/m1/README.md`](../../evidence/m1/README.md) for what the measurements
  do not establish.
- **[ADG-2](../decisions/adg-2-mod-execution.md): measured, decision deferred to
  M3.** Four bindings were measured and the measurements stand; the choice between
  them is not made, because it turns on properties of a loader that does not exist.
  The leading candidate is a mod linked into the loader — 2.5 ns a call, no unsafe,
  survives a panicking mod, contract drift caught at build — with per-mod dynamic
  libraries the alternative to beat at 0.625 ns faster and drop-in distribution.
  WebAssembly costs 4.26× the leading candidate's call and a helper process 1267×.
- **[ADG-3](../decisions/adg-3-bindings.md): bindings are generated at build time
  from the pinned jar's class files.** 26.1.2 ships unobfuscated and publishes no
  mapping file, so the class files are the mapping. Two independent extractors
  agree on every target, and a drifted input is refused before a name is read.
- **[ADG-4](../decisions/adg-4-state-ownership.md): the game keeps its state.**
  All three ownership models measured within 1.09 ns of each other at the median,
  so the decision went to the axis that is not nanoseconds: one copy cannot go
  stale.
- **[ADG-5](../decisions/adg-5-prism-integration.md): stock Prism, two instance
  arguments, no patch.** The launcher surface is verified, and the pinned client
  has been observed launching with Ravel's `-javaagent` attached and no other
  loader. The agent argument is what the claim rests on; the library path is
  recorded but proves nothing, because the launcher appends its own after the
  instance's and the last one wins.
- **ADG-6 (configuration schema and UI)** has not started. It is due before M3
  closes.

Two consequences run through everything after M1. A boundary crossing is the
unit of cost, so reducing crossings matters and reducing bytes does not. And
no maintained crate holds unsafe code at all — the exception M1 drafted turned out
to be unnecessary; see [`safety-boundary.md`](safety-boundary.md).
