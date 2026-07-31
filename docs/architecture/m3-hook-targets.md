# M3 hook targets for Minecraft 26.1.2

Status: resolved against the pinned client jar, **not yet inventoried**. Every
target below is real. [ADG-4](../decisions/adg-4-state-ownership.md) now admits
opaque handles, so nothing about these targets is blocked by a decision; what
remains is teaching the generator to carry one.

This file exists so that decision was made against what the game actually offers
rather than against a guess, and so the resolution work is not repeated. It is a
record, not a contract: `hooks.toml` remains the reviewed inventory and
`mappings.lock` remains what pins it.

## What still has to happen before these land

`crates/ravel-minecraft-adapter/src/generate/render.rs` accepts a hook parameter
or return only when it is a JVM primitive or `java/lang/String`. Anything else is
refused with:

```text
a hook cannot carry <class> by value, and ADG-4 does not admit a handle
```

That message is now out of date in its second clause. ADG-4 admits a handle; the
generator has not been taught to emit one. Until it is, the inventory can hold
only what it holds today — which is why the six current entries are all
`StringUtil` and `SmoothDouble` carrying `(I)Z`,
`(Ljava/lang/String;Z)Ljava/lang/String;`, `(DD)D` and `D`.

Of the targets below, only `MappedRegistry.frozen` (`Z`), `Minecraft.tick`
(`()V`), `MinecraftServer.tickCount` (`I`),
`MinecraftServer.getAverageTickTimeNanos` (`()J`) and `BuiltInRegistries.freeze`
(`()V`) would pass as written. A tick callback carrying nothing is useful; a
registry hook that cannot name what is being registered is not.

The generator work is therefore the enabling change for M3-W2 onward: a handle
type in the rendered binding, a safe wrapper that ties a handle's life to the
call that produced it, and the refusal message corrected to say what is actually
unsupported.

## How these were resolved

Parsed from class-file constant pools and member tables in the pinned jar named by
[`adapter.toml`](../../minecraft/versions/26.1.2/adapter.toml), then cross-checked
with `javap -p -s`. Two independent passes agreed on every entry. No name here
comes from recall, and 26.1.2 ships unobfuscated with no mapping file, so the
class files are the mapping — the same basis
[ADG-3](../decisions/adg-3-bindings.md) records.

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
shape as the existing `filterText` pair.

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
differs; the resolution key already includes the class, so it should resolve, but
nothing has proven it yet.

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

A constructor has never been used as a hook target here, so whether the generator
handles the JVM's `<init>` name is untested and worth proving early.

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

Separately from ADG-4: rendering a screen *from Rust* needs Ravel to call **into**
Minecraft, and no accepted ADR provides that. ADG-1 established Java-to-Rust only.
That is ADG-6's to settle.

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

## What adding any of these costs

Editing `hooks.toml` means updating `inputs.hookInventory`'s `sha256` and `bytes`
in `adapter.toml`, then regenerating `mappings.lock`. That is the deliberate step
ADG-3 designed in, and it works: an inventory edit without the hash update is
refused, and a resolved set that differs from the lock is refused separately.

Regenerating the lock currently runs the ADG-3 spike, which also rewrites
`evidence/m1/spikes/adg-3/`. Those files are M1's record of what M1 measured and
should be restored rather than carried forward, or the regeneration path should
grow a mode that writes only the lock.
