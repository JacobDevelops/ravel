# Minecraft 1.12.2 version data

This version carries 22 reviewed hooks and records 17 non-goals. `hooks.toml`
records each semantic target and each unsupported optional surface separately;
these are legacy operations, not modern member names substituted into old jars.
The scaffold checks these counts against the inventory.

`adapter.toml` pins Mojang's official client and dedicated-server jars, Feather
build 31's merged Tiny mappings, the mapping-io normalizer, and the official
version manifest. Names translate from `official` runtime names to Feather's
`named` namespace. `mappings.lock` records both jar hashes. Shared hooks resolve
against both jars and must agree; client-only hooks are explicitly classified.
The source class-file release is Java 8 (`sourceJavaRelease = 8`); Ravel's
execution and generated-source target remain JDK 27 (`javaRelease = 27`).

The inventory covers numeric block/item registration, block-state metadata and
item metadata, client/base-server/integrated-server ticks, world ticks, vanilla
custom payload dispatch, client resource and atlas reload, recipe registration,
NBT saved-data storage, world saving, and GUI opening and base-screen drawing.
Tick observations do not claim deduplication or paused-world behaviour is
implemented. Payload entry observations occur before the thread check, including
both off-thread requeue and main-thread dispatch; runtime ports must select the
main-thread observation. Base-screen draw observation misses overrides that omit
super. Neither observation claims complete runtime coverage. Registration and
resource observations do not establish lifecycle barriers.

`reflection.toml` records only the legacy saved-data and map-storage classes and
members, verified against the client jar. It does not invent modern codec,
registry-key, or data-fixer bindings. `stubs.toml` declares the screen pause-query
surface. The version-local `templates/screen/` tree emits only its observation
helper; no modern screen, game, driver, or sibling templates are inherited.

Eligibility is explicitly `generation-only`: locks, Java generation, adapter
building, and Java compilation are checked, but adapter tests and all thirteen
live-harness sections remain named not-yet-run obligations. This is not evidence
of Minecraft starting, gameplay parity, or a runtime harness passing. The modern
versions retain their full test and harness requirements when acquired local
inputs are present. Absent local inputs produce a named `SKIPPED` report, not
live-support proof. Lock-only generation permits all declared game/library jars
absent; a partial fixture set fails naming the missing jar instead of bypassing
checks.
