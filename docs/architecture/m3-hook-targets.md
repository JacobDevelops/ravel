# M3 hook targets for Minecraft 26.1.2

Status: resolved against the pinned client jar and **inventoried**, except for
one target named below. Every target here is real.
The [state-ownership mechanism](overview.md#mechanisms) uses opaque handles and
the generator carries one, so nothing here is blocked by a missing shape.

This file records what the game actually offers
so the resolution work is not repeated. It is a
record, not a contract: `hooks.toml` remains the reviewed inventory and
`mappings.lock` remains what pins it.

## What landed, and what did not

`crates/ravel-minecraft-generate/src/render.rs` used to accept a hook
parameter only when it was a JVM primitive or `java/lang/String`, which is why
the first six entries are all `StringUtil` and `SmoothDouble` carrying `(I)Z`,
`(Ljava/lang/String;Z)Ljava/lang/String;`, `(DD)D` and `D`. It now accepts any
reference, including an array, and renders it as `ravel_minecraft::JavaHandle` —
a borrow of the local reference the JVM handed the exported frame, so a handle
that outlives its call is a compile error rather than a rule. The generated Java
declares every such parameter as `java.lang.Object`, so `Hook.java` declares no
game type and still compiles without the client jar. It does name game classes —
its target table carries the internal name of each one as a string — and a string
reference needs nothing on the class path. What is refused now is
only a descriptor that is not one: an unterminated class name, an array of
nothing, or a character that is not a descriptor at all.

`hooks.toml` therefore grew from 6 entries to 39 — 35 methods and 4 fields,
carrying 28 handle parameters between them — covering all six capabilities
below. Every entry was re-resolved against the class file's own member table for
name, descriptor, kind and static flag before it was written. The last two of
those entries are the world persistence seams, tabled under
[World save and load](#world-save-and-load--m3-w4); they arrived after this
paragraph was first written, which is why the four counts in it are now gated.
`verify_hook_inventory_document` in [`tools/ravel-xtask/src/inventory.rs`](../../tools/ravel-xtask/src/inventory.rs)
derives all four from `hooks.toml` itself on every `ravel:verify-scaffold` and
fails when this file disagrees, so the sentence above cannot go stale behind the
inventory again. A handle parameter there is a reference-type parameter of a
method entry's descriptor other than `java/lang/String`, which the generator
renders as a string rather than as a `JavaHandle`; a field entry carries no
parameters.

**`Commands.<init>` is the one target deliberately left out.** It resolves: the
class file declares `<init>` with the reviewed descriptor, so the generator's
`<init>` handling is no longer the open question. Two other things are. The
transformer prepends its call at the first instruction of the method, which in a
constructor is before the `super()` call; it loads only declared parameters and
never `this`, so the verifier should accept it, but "should" is not a claim this
repository makes without a launch behind it. And the seam is wrong for what it
was wanted for: vanilla registers every command inside that constructor, so a
prepended hook fires before any registration has happened and cannot observe
one. `Commands.dispatcher` and `performPrefixedCommand` cover commands
without either problem, and both are inventoried.

`Registry.freeze` and `WritableRegistry.register` are not candidates and are not
inventoried: both are `public abstract` on interfaces, so neither has a body to
rewrite.

Two things a handle still does not do, and neither is this generator's to fix.
It cannot be read through: reading a field and keeping the result would mirror
game state. And it cannot be used to call back into the game; actions are
instructions performed by the Java adapter.

## How these were resolved

Parsed from class-file constant pools and member tables in the pinned jar named by
[`adapter.toml`](../../minecraft/versions/26.1.2/adapter.toml), then cross-checked
with `javap -p -s`. Two independent passes agreed on every entry. No name here
comes from recall, and 26.1.2 ships unobfuscated with no mapping file, so the
class files are the mapping — the same basis the
[binding mechanism](overview.md#mechanisms) uses.

The generator's `Kind` supports `static-method`, `instance-method`,
`static-field` and `instance-field`.

## Registry and content registration — M3-W1

`MappedRegistry` is `public class MappedRegistry<T> implements WritableRegistry<T>`
and is the only concrete writable registry, so blocks, items and recipes all route
through one `register`. There is no per-kind registration method.

| Member | Descriptor | Kind |
| --- | --- | --- |
| `net.minecraft.core.MappedRegistry::register` | `(Lnet/minecraft/resources/ResourceKey;Ljava/lang/Object;Lnet/minecraft/core/RegistrationInfo;)Lnet/minecraft/core/Holder$Reference;` | instance-method |
| `net.minecraft.core.MappedRegistry::freeze` | `()Lnet/minecraft/core/Registry;` | instance-method |
| `net.minecraft.core.MappedRegistry::frozen` | `Z` | instance-field |
| `net.minecraft.core.registries.BuiltInRegistries::freeze` | `()V` | static-method |
| `net.minecraft.core.registries.BuiltInRegistries::bootStrap` | `()V` | static-method |
| `net.minecraft.world.item.crafting.RecipeManager::apply` | `(Lnet/minecraft/world/item/crafting/RecipeMap;Lnet/minecraft/server/packs/resources/ResourceManager;Lnet/minecraft/util/profiling/ProfilerFiller;)V` | instance-method |

`MappedRegistry.freeze` is the game's own registration barrier, which is the same
thing `SPEC.md` §14 requires of Ravel. Ravel's barrier can close with it rather
than beside it.

Two traps. `Registry.freeze` and `WritableRegistry.register` both exist and are
`public abstract` on interfaces, so neither has a body to rewrite. And
`RecipeManager.apply` is declared twice — the typed form above plus a bridge
taking `Ljava/lang/Object;` first. Descriptor is the only discriminator, the same
shape as the existing `filterText` pair, and it resolves: the typed form matches
exactly once and the bridge is never reached.

## Server and client tick — M3-W2, M3-W6

| Member | Descriptor | Kind |
| --- | --- | --- |
| `net.minecraft.server.MinecraftServer::tickServer` | `(Ljava/util/function/BooleanSupplier;)V` | instance-method |
| `net.minecraft.client.server.IntegratedServer::tickServer` | `(Ljava/util/function/BooleanSupplier;)V` | instance-method |
| `net.minecraft.server.MinecraftServer::tickCount` | `I` | instance-field |
| `net.minecraft.client.Minecraft::tick` | `()V` | instance-method |
| `net.minecraft.client.Minecraft::runTick` | `(Z)V` | instance-method |

The integrated server lives at `net.minecraft.client.server.IntegratedServer`.
That pair is the first case where name *and* descriptor match and only the class
differs. It resolves: `IntegratedServer` declares its own `tickServer` rather
than inheriting one, so each entry reads a different class file and each matches
exactly once. Both are inventoried, and because the override calls
`super.tickServer`, an integrated server fires both hooks per tick — one
invocation each, not a double count of one.

Profiling anchors on `MinecraftServer`, all verified:
`getAverageTickTimeNanos()J`, `getTickTimesNanos()[J`, `logTickMethodTime(J)V`,
`getTickTimeLogger()Lnet/minecraft/util/debugchart/SampleLogger;`, and the field
`tickFrame Lcom/mojang/jtracy/DiscontinuousFrame;`.

## Commands — M3-W2

| Member | Descriptor | Kind |
| --- | --- | --- |
| `net.minecraft.commands.Commands::<init>` | `(Lnet/minecraft/commands/Commands$CommandSelection;Lnet/minecraft/commands/CommandBuildContext;)V` | instance-method |
| `net.minecraft.commands.Commands::dispatcher` | `Lcom/mojang/brigadier/CommandDispatcher;` | instance-field |

There is no registration callback: vanilla registers every command inside the
constructor. So the seam is `<init>` or the dispatcher field, and
`performPrefixedCommand(Lnet/minecraft/commands/CommandSourceStack;Ljava/lang/String;)V`
is the execution-side alternative if interception is wanted instead.

The generator's `<init>` handling is no longer the open question — the class file
declares `<init>` with that descriptor and it resolves exactly once. What is
still unproven is the bytecode: a prepended call sits before the `super()` call,
where the verifier tracks `uninitializedThis`. The transformer never loads `this`
on an instance target, so it should verify, and a launch is the only thing that
would settle it. That, plus the seam firing before any command is registered, is
why `<init>` is the one resolved target left out of the inventory.

## Resources and data reload — M3-W2

| Member | Descriptor | Kind |
| --- | --- | --- |
| `net.minecraft.server.packs.resources.ReloadableResourceManager::registerReloadListener` | `(Lnet/minecraft/server/packs/resources/PreparableReloadListener;)V` | instance-method |
| `net.minecraft.server.packs.resources.ReloadableResourceManager::createReload` | `(Ljava/util/concurrent/Executor;Ljava/util/concurrent/Executor;Ljava/util/concurrent/CompletableFuture;Ljava/util/List;)Lnet/minecraft/server/packs/resources/ReloadInstance;` | instance-method |
| `net.minecraft.server.ReloadableServerResources::loadResources` | `(Lnet/minecraft/server/packs/resources/ResourceManager;Lnet/minecraft/core/LayeredRegistryAccess;Ljava/util/List;Lnet/minecraft/world/flag/FeatureFlagSet;Lnet/minecraft/commands/Commands$CommandSelection;Lnet/minecraft/server/permissions/PermissionSet;Ljava/util/concurrent/Executor;Ljava/util/concurrent/Executor;)Ljava/util/concurrent/CompletableFuture;` | static-method |
| `net.minecraft.client.Minecraft::reloadResourcePacks` | `()Ljava/util/concurrent/CompletableFuture;` | instance-method |

`loadResources` is the server datapack seam that also builds `Commands` and the
`RecipeManager`. `reloadResourcePacks` has a second overload taking
`(ZLnet/minecraft/client/Minecraft$GameLoadCookie;)`, another descriptor-only
distinction.

## Screens and GUI — M3-W3, M3-W5

Read this before planning either package. **26.1.2 has no `Screen.render`.**
Screens implement `Renderable` and draw through render-state extraction, so any
design assuming a `GuiGraphics`-taking `render(...)` is wrong for this version.

| Member | Descriptor | Kind |
| --- | --- | --- |
| `net.minecraft.client.gui.screens.Screen::init` | `()V` | instance-method |
| `net.minecraft.client.gui.screens.Screen::init` | `(II)V` | instance-method |
| `net.minecraft.client.gui.screens.Screen::extractRenderState` | `(Lnet/minecraft/client/gui/GuiGraphicsExtractor;IIF)V` | instance-method |
| `net.minecraft.client.gui.screens.Screen::tick` | `()V` | instance-method |
| `net.minecraft.client.Minecraft::setScreen` | `(Lnet/minecraft/client/gui/screens/Screen;)V` | instance-method |
| `net.minecraft.client.gui.screens.TitleScreen::createNormalMenuOptions` | `(II)I` | instance-method |
| `net.minecraft.client.gui.screens.options.OptionsScreen::openScreenButton` | `(Lnet/minecraft/network/chat/Component;Ljava/util/function/Supplier;)Lnet/minecraft/client/gui/components/Button;` | instance-method |

`init(int,int)` is `public final`, so it cannot be overridden and a hook there has
to be call interception. The overridable one is `protected void init()`.
`createNormalMenuOptions` builds the main-menu button column and returns the next
y, which is where a Mods button goes.

Rendering a screen *from Rust* would need Ravel to call **into** Minecraft,
whereas the crossing is Java-to-Rust only. Ravel-owned screens are declared in
data and drawn in Java; see the [mechanisms overview](overview.md#mechanisms).

## World save and load — M3-W4

| Member | Descriptor | Kind |
| --- | --- | --- |
| `net.minecraft.world.level.storage.SavedDataStorage::computeIfAbsent` | `(Lnet/minecraft/world/level/saveddata/SavedDataType;)Lnet/minecraft/world/level/saveddata/SavedData;` | instance-method |
| `net.minecraft.world.level.storage.SavedDataStorage::readSavedData` | `(Lnet/minecraft/world/level/saveddata/SavedDataType;)Lnet/minecraft/world/level/saveddata/SavedData;` | instance-method |
| `net.minecraft.world.level.storage.SavedDataStorage::tryWrite` | `(Lnet/minecraft/world/level/saveddata/SavedDataType;Lnet/minecraft/nbt/CompoundTag;)V` | instance-method |
| `net.minecraft.world.level.storage.SavedDataStorage::saveAndJoin` | `()V` | instance-method |
| `net.minecraft.world.level.saveddata.SavedData::setDirty` | `()V` | instance-method |
| `net.minecraft.server.MinecraftServer::saveEverything` | `(ZZZ)Z` | instance-method |

`SavedData` carries only `dirty`, `setDirty`, and `isDirty`. Serialization lives in
the `SavedDataType` record, so a version migration attaches to its `codec()` and
there is no read/write pair on `SavedData` to hook. `ravel-save` already models the
deciding half of that — which versions exist and whether a world may be opened —
and it needs none of these targets to do so.

The persistence adapter uses two of these as its own seams, and both
are now reviewed inventory entries resolved against the pinned jar:

| Member | Descriptor | Kind | Inventory id |
| --- | --- | --- | --- |
| `net.minecraft.client.Minecraft::doWorldLoad` | `(Lnet/minecraft/world/level/storage/LevelStorageSource$LevelStorageAccess;Lnet/minecraft/server/packs/repository/PackRepository;Lnet/minecraft/server/WorldStem;Ljava/util/Optional;Z)V` | instance-method | `world.open-persistence` |
| `net.minecraft.server.MinecraftServer::saveAllChunks` | `(ZZZ)Z` | instance-method | `world.save-persistence` |

These are adapter crossings rather than observations. The entries above watch what
the game does with saved data; these two carry Ravel's own aggregate into and out
of it. Each is the single common seam for its half:
`doWorldLoad` is where both the creation and existing-world paths converge before
the body's first persistent mutation, and `saveAllChunks` is the one call both
autosave and `stopServer` reach.

## What adding any of these costs

Editing `hooks.toml` means updating `inputs.hookInventory`'s `sha256` and `bytes`
in `adapter.toml`, then regenerating `mappings.lock`. An inventory edit without
the hash update is refused, and a resolved set that differs from the lock is
refused separately.

Regenerating the lock no longer runs the ADG-3 spike. That spike is M1's record
and has drifted from the adapter inputs it once read — it predates `stubSeeds`
and refuses to parse `adapter.toml` — so rewriting a lock through it would mean
editing M1's measurement to describe M3's inputs. The generator grew the
lock-only mode this section asked for instead:

```console
cargo run -p ravel-xtask -- generate locks
```

It resolves against the pinned jar and writes `mappings.lock` and `stubs.lock`
from what it found, along with the `sha256` and `bytes` that `inputs.mappingsLock`
and `inputs.stubsLock` pin for them, so a lock can only be rewritten by someone
holding the jar who asked for it. Those two pins are what a jar-less machine
checks: it has nothing to resolve against, so without them, generating from a
lock and comparing the result back to that same lock would pass any stale lock
that is still well formed. The explicit xtask runs independently of the adapter's
build script, so stale locks cannot prevent the writer from running. Ordinary
builds still refuse drift; nothing updates a lock silently.

The spike route also rewrote `evidence/m1/spikes/adg-3/` on every run, so a lock
regeneration used to arrive with M1's measurements edited underneath it. The
lock-only mode writes two files and nothing else, which is what retires that.
