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

The JVM entry mechanism, Rust execution/ABI, bindings, state ownership, Prism
integration, and configuration UI remain open until ADG-1 through ADG-6 pass.
Official-server interoperability is a later protocol layer; it does not turn
Ravel into a Java mod host.
