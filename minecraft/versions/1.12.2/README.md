# Minecraft 1.12.2 version data

This version carries 48 reviewed hooks and records 14 non-goals. The original
22 hooks are supplemented by seven bridges: `legacy.world-open`,
`legacy.world-open-integrated`, `legacy.command-install`, `legacy.command-execute`,
`legacy.item-fire-damage`, `legacy.title-init`, and `legacy.global-palette-size`.
`hooks.toml` records semantic targets and unsupported optional surfaces
separately; these are legacy operations, not modern member names substituted
into old jars. The scaffold checks these inventory counts.

Eleven additional client-only renderer observers cover the frame, terrain setup,
block-layer rendering, chunk updates, rebuild compile-body returns, task creation,
worker entry, callback-body returns, vertex-buffer uploads, world replacement,
and client shutdown. Task generations are associated at constructor completion,
before queueing; every world replacement invalidates outstanding work.
Compile-body and success-callback normal returns can include early aborts or
cancelled tasks, so neither proves successful compilation or publication.
These lifecycle observations do not replace the vanilla renderer or establish
WP5 renderer replacement or performance acceptance.

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

`reflection.toml` records explicit legacy runtime roles, not modern aliases.
Reflective methods use their exact Feather declaration roles: command execution
is declared on `CommandHandler` and damage on `Entity`; concrete override hooks
remain on their instrumented classes. Screen initialization is a stub lifecycle
method and hook target, not an unused reflected role.
Real-client harness sections use the live platform's `LegacyLoader`, including
its LWJGL bytecode remaps, with game jars isolated from the parent classpath.
The parent carries the same Ravel-owned LWJGL facade classes, pinned LWJGL 3
modules, native jars and scoped native-access grants as the live client launcher.
The wrapper establishes Agent's game loader and side through the actual
distribution entry class before invoking the fixture.
Packet descriptors name Feather's `net.minecraft.network.packet.Packet`.
Distribution entry roles name client `Main` and server `MinecraftServer`, matching
the legacy launcher and supplying Agent's version-neutral side-selection names.
The inventory's consumer closure includes bootstrap and harness sources, both of
which resolve the generated legacy names. The NBT `contains` stub role selects
the exact `(String, int)` overload; the one-argument predicate has another linked
name and cannot share that template reference.
Likewise, the screen `init` stub role selects the no-argument lifecycle method,
not the separately linked `(Minecraft, int, int)` initialization overload.
Stub hierarchy extraction retains the nearest seeded superclass and only actual
interface edges: `TitleScreen extends Screen` must not also implement `GuiElement`,
the class already inherited through `Screen`.
`stubs.toml` declares the version-local sibling dependencies. No modern screen,
game, driver, or sibling templates are inherited. Generated output and lock pins
must be refreshed together through the existing generator after source edits.
The standalone screen build rebuilds the agent first for Hook's full dependency
closure at the agent's pinned Java release, then compiles only screen siblings
at the game's Java release. The agent jar is a compile dependency, not screen
jar content; an existing agent jar is never assumed current.
Named-module harness launches explicitly root `java.instrument` alongside
`dev.ravel.agent`, including transformer checks that do not use `-javaagent`.
The version-local Java 8-compatible screen siblings provide the menu's Mods
button, mod list and settings navigation with mouse/key routing. These screens
use legacy immediate GUI drawing; they do not claim narration or WP5's renderer
replacement. Actual client UI launch remains an acceptance obligation.

`RavelWorldData` is a real legacy `SavedData` subclass, loaded through the sibling
artifact on both server types. Its `aggregate` NBT byte array is capped at the
same 1 MiB transport bound as Rust persistence. Staged reads use the legacy NBT
read limiter (2 MiB accounting allowance), before vanilla world creation; Rust
alone decides aggregate schema, package compatibility, and migration. A refusal
throws rather than silently starting a world with unreadable mod data.

After successful world loading, the adapter verifies canonical world-directory
and payload identity before binding the overworld's global `SavedDataStorage`.
Per-dimension stores remain distinct observations, not additional Rust world
opens. Saves stage and validate a replacement first, then update the existing
live object's payload and dirty bit; only the first checkpoint installs a new
object. A failed native answer or replacement retains the prior checkpoint and
does not interrupt vanilla saving. Integrated servers initialize their independent
server-thread native composition before admission, rather than borrowing client
saved-data state. `LegacyPersistenceDataMain` contains focused generated-sibling
staging, bound, identity and failure-preservation assertions.
`LegacyPersistenceNativeMain` uses the real JNI composition and official
file-backed legacy storage to assert schema-1 migration, schema-3 reopen,
same-object updates, global/dimension separation, identity refusal and exact NBT
retention after failed admission/save. Its internal storage-boundary calls do not
replace actual client/integrated and dedicated world-load seam acceptance.

After every role/hook/stub writer has finished, refresh pins with
`RAVEL_MINECRAFT_VERSION=1.12.2 cargo xtask generate locks`, then refresh Java
output with `RAVEL_MINECRAFT_VERSION=1.12.2 cargo xtask generate java`.
`adapter.toml` pins the reviewed hook, stub-seed and reflection source digests;
these must be refreshed before lock generation can admit changed source inputs.
Generated lock pins must likewise be refreshed together, never patched by hand.

Eligibility is explicitly `live`: locks, Java generation, adapter tests and all
thirteen capability-selected JVM harness sections are required alongside the
modern versions. Official integrated save/reopen and connected dedicated
server/restart acceptance exercised all 29 hooks; see the
[legacy launch runbook](../../../docs/runbooks/legacy-launch.md). This does not
establish native counterpart gameplay parity or progression acceptance.
Absent local inputs produce a named `SKIPPED` report, not live-support proof.
Lock-only generation permits all declared game/library jars absent; a partial
fixture set fails naming the missing jar instead of bypassing checks.
