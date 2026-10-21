# Legacy vanilla launch

This procedure exercises the official Minecraft 1.12.2 client and dedicated
server on the Temurin 27 runtime pinned in `mise.toml`. It does not install
LaunchWrapper, Forge, Cleanroom or Java mods, and does not establish native
counterpart or progression acceptance.

## Platform boundary

`jvm/legacy-platform` is a Ravel-owned compatibility surface implemented on
LWJGL 3.3.6. Its eight Java/native artifacts are pinned by SHA-256 in
`dependencies.sha256`; official game, library and asset hashes are pinned in
the tracked `official-dependencies.sha256`. Downloaded artifacts, assets,
generated worlds, screenshots and logs remain under ignored
`tests/fixtures/local/legacy-platform/`. Build verification acquires no assets;
only client launch requests the pinned asset set.
Acquisition publishes verified manifest, native-library and class-path outputs
atomically; server completion is published only after acceptance succeeds.
The server implementation streams and flushes its log during execution to retain
diagnostic output even when no completion record is published.
Offline regression tests in `jvm/legacy-platform/tools/test_runtime.py` use
temporary fixtures and a mocked server process to exercise startup, save, seed,
exit and native-library acceptance conditions without launching Java or
downloading game inputs. These tests do not assert server-log retention.

The legacy class loader rewrites constant-pool class/descriptor references
according to the single `adapter-owners.txt` inventory into Ravel's
`dev.ravel.legacy` namespace, leaving LWJGL3 in named modules. At the single
integrated-server stop call in the pinned client's world-unload method, it also
waits for the server thread to finish: vanilla otherwise closes the shared
region cache while the server is still saving, producing `Stream Closed` errors.
The transform rejects missing or duplicate targets; it does not patch the game
jar on disk or modify the existing bootstrap agent. GLFW owns the display, input
and compatibility-profile OpenGL context;
OpenAL owns the sound device and context. Matching LWJGL3 APIs are reused rather
than reimplemented. No LWJGL2 Java or native artifact is placed on the runtime
class path. The dedicated server has a separate launch path.

Legacy launch and benchmark commands default `ALSOFT_DRIVERS` to `null`, so
OpenAL still opens and mixes audio without sending sound to speakers. An
explicit caller value is preserved. Live acceptance still checks the audio
device and context.

`vanilla-1.12.2-lwjgl.tsv` records the exact client constant-pool surface,
including class, descriptor, inherited member, superclass and invocation
references. Build verification regenerates and compares this inventory before
checking the complete adapted surface and game's Paulscode OpenAL companion
jar. JVM member-resolution tests independently exercise the recorded references.
`SURFACE.txt` records the upstream licence study; no upstream shim
implementation was copied.

## Reproduce

Run from the repository root with the pinned mise tools:

```console
mise exec -- bash jvm/legacy-platform/launch.sh build
mise exec -- bash jvm/legacy-platform/launch.sh client xvfb
DISPLAY=:0 mise exec -- bash jvm/legacy-platform/launch.sh client display
mise exec -- bash jvm/legacy-platform/launch.sh server
mise exec -- bash jvm/legacy-platform/launch.sh native-audit
```

The Xvfb mode selects software OpenGL. Display mode uses the active X display
and permits GPU-backed rendering. Client automation creates a world with seed
`8675309`, checks the live world's seed, and waits at least 100 simulation ticks.
Robot clicks real menu buttons and types the world name and More World Options
seed through GLFW character callbacks; each character is checked. Robot input drives
movement and look; wheel callbacks must change the live hotbar slot. A real
OpenAL source must reach `AL_PLAYING`. Resize to 960 × 540 and fullscreen
roundtrips wait for observed conditions, not fixed sleeps. Vanilla restores its
854 × 480 launch size after fullscreen. Automation saves, reopens, checks the
seed again, then saves and quits. Screenshots capture only the game window.
A missing fresh completion record, logged game error or unsuccessful child
process fails; the complete launch has a 300-second deadline.

`ravel:verify-jvm` compiles this layer with `javac -Xlint:all -Werror` and checks
the scanned surface. The explicit `ravel:legacy-client` and
`ravel:legacy-server` tasks perform the live workload; compilation alone is not
live acceptance.

### Explicit premain/JNI launches

The vanilla commands above remain the baseline and do not load the agent.
Generate and package the legacy artifacts immediately before agent launches:
the shared `target/ravel-jvm` output is overwritten by generation for another version.

```console
mise exec -- cargo make ravel:legacy-agent-build
mise exec -- cargo make ravel:legacy-agent-client
mise exec -- cargo make ravel:legacy-agent-server
```

For complete agent acceptance, run the two commands below serially in the same
exclusive live-test session. Do not run display or server tasks concurrently;
the live task owns its server and connected-client lifecycle and port `25586`.
It uses Xvfb, not the user's `:0` desktop.

```console
mise exec -- cargo make ravel:legacy-agent-harness
mise exec -- cargo make ravel:legacy-agent-live
```

The harness is display-free. Its thirteen sections are `cluster`, `content`,
`hierarchy`, `initialization`, `load`, `members`, `outward`, `persistence`,
`refusal`, `screens`, `settings`, `side`, and `transformation`. Generic
transformation/refusal/loader/side fixtures retain the actual generated hook
and JNI boundary. Legacy-specific fixtures use the pinned official classes,
real admitted content and numeric IDs, metadata arguments/results, actual NBT
storage plus native migration/reopen/refusal, and generated settings/menu
screens. Settings includes a separate noneditable dedicated-authority row.
Offline `outward` proves JNI queue encoding, exactly-once drain, source
registration and refusal; it does **not** claim sound playback or rendering.
Sections are recorded only after their assertions pass, never from a requested
section list.

The live task reruns those fixtures, then drives an integrated world with actual
client/server ticks, paused simulation checks, resource/atlas reload, metadata,
fire-resistant item behavior, real sound-manager submission, actual settings
and mod-menu frames, exactly one title-menu entry after each of two real
resize-driven reinitializations, command permission/wrong-thread denial and
authorized execution, and two-way packet connections with accepted/negative
payload assertions. Integrated save/reopen must checkpoint exactly one additional
native fixture visit. A second official client connects to the actual dedicated
server; the controller waits for that client before issuing vanilla seed/save/stop
commands. Finally the same dedicated world restarts, saves, and must checkpoint
exactly one more native fixture visit. Each scenario records its measured duration
and exit status. All 22 original reviewed hooks and seven supplemental lifecycle,
command, title-init, palette-size and fire-policy bridges (29 total) must appear in fresh
successful process logs.
Presence of a hook alone is not a replacement for its semantic assertions.

The complete verdict is ignored local
`tests/fixtures/local/legacy-platform/evidence-agent-acceptance/result.json`.
The previous verdict is removed before launching; the new JSON is published
atomically only after all process results and obligations have been collected.
Additional local records include `evidence-harness-agent/`,
`evidence-xvfb-agent-dedicated/`, `evidence-server-agent-restart/` and unique
`packets-*` directories. The JSON distinguishes original/supplemental hooks,
missing obligations, actual process exits, persistence visit counts and errors.
No tracked acceptance evidence is rewritten by these tasks.

Minecraft 1.12.2 is live-eligible. The default verification matrix runs its
workspace tests and all thirteen JVM harness sections alongside the modern
versions. Package compatibility, hook inventory and the compiled Java source
closure determine the applicable obligations; legacy support does not require
modern-only content or reflection roles.

The agent commands add `-javaagent:target/ravel-jvm/ravel-agent.jar` and
`-Dravel.host=target/release/libravel_bootstrap.so` as absolute paths. To use
an independently retained pair, set `RAVEL_LEGACY_AGENT` and
`RAVEL_LEGACY_HOST`; both files must exist. The matching `ravel-screen.jar` must
remain beside the agent jar for the settings/screens fixtures. The agent must
have been generated for `1.12.2`, not merely renamed from a modern build.
Client worlds and logs use separate `client-xvfb-agent/` and
`evidence-xvfb-agent/` directories; dedicated output uses `dedicated-agent/`
and `evidence-server-agent/`. All remain under the ignored local fixture root.
Dedicated startup still checks the actual seed, vanilla save, successful exit,
and absence of LWJGL/GLFW/OpenAL/JInput native libraries.

On 2026-10-18 `mise exec -- numen run -- cargo make ravel:legacy-agent-live`
exited 0 in 56 seconds on Temurin 27+35 under Xvfb. All thirteen harness
sections passed. The integrated client exited 0 after 30.3 seconds; its
automation completed menu clicks, typed world name and seed, creation, 100
ticks, `AL_PLAYING` audio, F3+T resource and atlas reload, walk, look, wheel,
resize, fullscreen, paused-tick check, save/quit and reopen in 28.0 seconds.
Integrated checkpoints recorded 1 then 2 native visits. The dedicated server with
a connected official client exited 0 after 13.8 seconds; the client run took
8.7 seconds. The dedicated restart exited 0 after 2.6 seconds, and checkpoints
advanced from 3 to 4 visits. No client native loaded on the server. All 29 hooks
appeared in fresh process logs.
A separate `client-agent display` run on the desktop display used an NVIDIA
OpenGL 4.6 context and exited 0 after 27.7 seconds, with the same automated
actions and `AL_PLAYING` audio. That run did not perform the F3+T reload,
paused-tick check or integrated checkpoints, so the Xvfb run above is the
acceptance record.

Vanilla play, Robot input and lifecycle drove 23 of the 29 hooks. These are
bootstrap, block and item registration, item metadata, all four tick hooks,
resource and atlas reload, recipe registration, the four saved-data hooks,
world save, screen open and draw, title init, both world-open hooks, command
install, and global palette size. The other six ran from acceptance probes
calling the real transformed methods on the owning thread: block-state-from-metadata,
block-metadata-from-state, fire damage, both payload hooks and command execution.
The payload packets crossed the real connections, and dedicated console commands
also executed. The Ravel mods and settings screens were opened by a probe.
Adapter callback counters alone are not gameplay proof. Missing fixture
blockstate and item models log warnings only. Rendering stayed vanilla, and no
performance claim is made.

## Renderer lifecycle observations

WP5 slice 2 adds pinned, client-only renderer observations without replacing
vanilla compilation, uploads or drawing. Feather's frame member is named
`Minecraft.runGame()V` in build 31; `runGameLoop` is a conceptual target name,
not a reviewed alias. The generator must resolve each named member exactly once
against the pinned official client jar, including its descriptor and side.

The reviewed Feather-to-official table is below. Named owners are under
`net.minecraft`: `Minecraft` is `client.Minecraft`, `WorldRenderer`,
`RenderChunk`, `ChunkCompileTask` and `ChunkRenderWorker` are under
`client.render.world`, and `VertexBuffer` is under `client.render.vertex`.
Descriptors retain full named packages in `hooks.toml`; here the official
descriptor makes each bytecode target unambiguous.

| Concept | Feather member | Named descriptor | Official target |
| --- | --- | --- | --- |
| Frame (`runGameLoop`) | `Minecraft.runGame` | `()V` | `bib.az()V` |
| Terrain (`setupTerrain`) | `WorldRenderer.setupRender` | `(Lnet/minecraft/entity/Entity;DLnet/minecraft/client/render/Culler;IZ)V` | `buy.a(Lvg;DLbxy;IZ)V` |
| Layer (`renderBlockLayer`) | `WorldRenderer.render` | `(Lnet/minecraft/client/render/block/BlockLayer;DILnet/minecraft/entity/Entity;)I` | `buy.a(Lamm;DILvg;)I` |
| Update (`updateChunks`) | `WorldRenderer.compileChunksUntil` | `(J)V` | `buy.a(J)V` |
| Rebuild (`rebuildChunk`) | `RenderChunk.compile` | `(FFFLnet/minecraft/client/render/world/ChunkCompileTask;)V` | `bxr.b(FFFLbxl;)V` |
| Task creation | `ChunkCompileTask.<init>` | `(Lnet/minecraft/client/render/world/RenderChunk;Lnet/minecraft/client/render/world/ChunkCompileTask$Type;D)V` | `bxl.<init>(Lbxr;Lbxl$b;D)V` |
| Worker | `ChunkRenderWorker.runTask` | `(Lnet/minecraft/client/render/world/ChunkCompileTask;)V` | `bxn.a(Lbxl;)V` |
| Upload callback return (not publication proof) | `ChunkRenderWorker$2.onSuccess` | `(Ljava/util/List;)V` | `bxn$2.a(Ljava/util/List;)V` |
| Upload | `VertexBuffer.upload` | `(Ljava/nio/ByteBuffer;)V` | `cdz.a(Ljava/nio/ByteBuffer;)V` |
| Resource reload (existing) | `Minecraft.reloadResources` | `()V` | `bib.f()V` |
| World teardown | `Minecraft.setWorld` | `(Lnet/minecraft/client/world/ClientWorld;Ljava/lang/String;)V` | `bib.a(Lbsb;Ljava/lang/String;)V` |
| Close | `Minecraft.shutdown` | `()V` | `bib.h()V` |

The receiver source is in `jvm/legacy-platform/src/agent/java`; the agent build
packages it in the shared bootstrap module so it can call `Hook` without a
duplicate platform class. It identifies the render/context owner, keeps vanilla
references on the Java side, and copies event, generation and thread scalars to Rust.
Bootstrap runtime owns the process/client generation independently of the
integrated server's selected composition. Resource reload and world teardown
invalidate prior generations. A completion check admits only the exact current
positive generation; it is a point-in-time check, not a lease authorizing later
GL submission. Observing stale vanilla work never suppresses that work.
Compile and upload-callback hooks observe normal method return, not proof of
successful mesh compilation or publication: vanilla can return early for an
aborted or cancelled task. Generation admission is independent of that outcome.
Task tokens are captured on successful construction before queueing, so queued
work cannot acquire a newer token merely by starting after invalidation. A
bounded, weak-key Java association is removed at upload-callback return; an
unknown or overflow-refused task never acquires a token at worker entry.

The live sequence drives frame observations, F3+T reload, world replacement,
unload and reopen. In proof mode only, one real task and its original association
are retained across reload for an injected stale observation through the normal
completion receiver; its rejection must not increase accepted completion counts.
This is a deterministic observer admission probe, not a claim that a naturally
racing vanilla callback was observed. The bounded proof reference is then
released. Renderer assertions supplement resource/atlas and persistence checks;
a hook's presence alone does not establish generation ordering or a replacement
renderer. Software-rendered Xvfb execution remains functional-only.

### Bounded section capture

WP5 slice 4 feeds the pinned `RenderChunk.compile` observation into bounded Rust
workers without suppressing vanilla compilation. The compiling task's Java owner
copies a 16³ section with a complete one-cell border: 18³ (5,832) cells, x
fastest, then z, then y. Each cell supplies numeric block identity and metadata,
packed raw stored sky/block light, and the game's full/opaque cube classification.
The border includes faces, edges and corners, not six face-only neighbours.
Java retains all game objects and performs source reads after compilation,
not during vanilla's mesh reads.

The primitive-array JNI crossing is one batch per admitted capture, not one
crossing per block. Rust validates fixed input lengths, classification consistency
and generation, then compares decoded values against the copied post-compile
region. This is not an atomic world transaction or proof of earlier vanilla mesh
inputs. Successfully admitted immutable snapshots transfer to Rust workers;
backpressure rejects work rather than growing queues. Numeric state/model lookup
keys are not baked textures or shading inputs.

The compact palette deduplicates state/model lookup keys and captured cube
classification; cells retain a two-byte palette index and packed-light byte.
Current payload bounds are exported by `ravel-renderer`; the historical slice-3
format and measurements remain in
[`section-snapshots.md`](../../registry/notes/sevtech/section-snapshots.md).
Java/JNI arrays, capture scratch, worker scratch, mesh allocations and completed
outputs have distinct accounting. Payload bounds exclude allocator metadata,
JVM headers and thread-runtime overhead and are not a claim about total RSS.

Reload/unload generation rejection supplements, rather than replaces, vanilla
task cancellation. An admitted snapshot is not permission for a later upload.
The existing live fixed-seed acceptance records capture observations alongside
the renderer lifecycle proof. Xvfb timing remains functional-only; physical
capture timings must disclose scene/settings and cannot establish renderer
savings while both the vanilla renderer and observational copies still run.

### Bounded mesh and visibility workers

Two Rust workers share eight outstanding slots covering queued, running and
completed work. Each admission reserves its maximum snapshot, output and scratch
payload until final consumption or cancellation. Duplicate outstanding keys,
stale generations, full capacity and shutdown reject admission. Generation
changes cancel in-flight work and reject late publication; teardown synchronously
drains workers before accepting another generation.

Meshes carry section-work keys and generations, per-layer CPU vertices and
indices, state-key material ranges, face sort metadata and six-face visibility
reachability. Only captured opaque full cubes produce geometry. Other state
keys are explicitly unsupported; incomplete results must not become a renderer
cutover. Geometry does not establish texture, ambient-occlusion or translucent
parity. With the default `vanilla` selection, Java consumes checked diagnostic
metadata and vanilla remains the only renderer. The optional geometry proof
retains native ownership until Java explicitly acknowledges a generation/key.

### Opt-in bounded GL geometry proof

Set `RAVEL_LEGACY_RENDERER=geometry-proof` for the explicit client-agent launch
or live acceptance. The matched comparator uses `--renderer geometry-proof`
with `--instance ravel --workload vanilla`. Omit either option for the default
`vanilla` selection. This selection never replaces vanilla world drawing.

The proof transfers one bounded little-endian byte array per retained completion,
not per vertex. A failed copy leaves native ownership intact; successful Java
acceptance acknowledges the exact generation/key. Incomplete or unsupported
results are refused, not presented as missing geometry in a successful draw.
Java selects supported GL entry points from the verified current LWJGL3 context,
owns upload buffers and offscreen submission, and releases them on reload,
world teardown and close. No GL 4.6 requirement is inferred from the test GPU.

The resident candidate must have an associated current vanilla compile task with
exactly the nonempty SOLID layer. Mixed-layer and non-SOLID sections are refused
as whole meshes and counted; they are never reduced to a convenient comparison
subset. Association retention is bounded by the sixteen in-flight native work
slots plus one separately pinned resident identity. Settled slots are released
without evicting the resident, and teardown retires both pending and pinned
references. The comparison still requires that exact task's compiled object to
be published at the original section origin before drawing vanilla's own VBO.
Both native and vanilla readbacks must contain foreground; pixel and depth
mismatches are reported rather than hidden. Automation's `lastStatus` separates
absent residents/associations, stale publication/task/origin/generation,
mixed-scope sections, busy locks and invalid buffers/readbacks.
Busy vanilla compile locks keep the copied completion owned by native for a later
poll, without acknowledgement or a scope-refusal count. An eligible but not
uploaded mesh is not a successful comparison. Blank readbacks in the current
projection remain pending (`absent-foreground-in-current-projection`), and only
nonempty actual readbacks can become captured evidence.

The offscreen solid-geometry comparison is narrower than vanilla appearance:
atlas UVs and sprites, model/biome tint, per-vertex AO and combined lighting are
not submitted by that GL proof. The separate selected-only baked contract captures
exact model/shading inputs, independently emits BLOCK words and submits bounded
shaded offscreen comparisons using the live atlas/lightmap. Controlled sky sections
stay in the platform's one-chunk ring, inside the render-distance-two window;
iron bars and glass panes exercise mipped cutouts independently of fancy graphics.
Fresh raw emitted/oracle RGBA and depth readbacks are independently recounted.
Fluid tessellation remains explicitly unsupported because it lacks baked-quad
provenance. See the [slice-6 status](../../registry/notes/sevtech/baked-models.md)
for the complete golden matrix, physical admission and bounds. These section-local
comparisons are not full-world visual goldens. Native-versus-CPU
reference images alone are not a vanilla oracle. Actual vanilla-section
comparison and matched selection-on/off measurements must disclose their scope,
refusals and mismatch counts; they do not authorize a default renderer or a
speedup claim.

## JVM permissions

In the vanilla baseline, the four LWJGL3 Java modules receive exactly
`--enable-native-access=org.lwjgl,org.lwjgl.glfw,org.lwjgl.opengl,org.lwjgl.openal`
with `--illegal-native-access=deny`. Ravel, vanilla, JNA and text-to-speech stay
unnamed and receive no native-access grant. There are no module opens or
exports. Optional narrator support is explicitly unsupported on this boundary;
it is not a reason to widen native permission.

The dedicated server runs without a native-access grant or LWJGL jars and
records loaded libraries to prove that no client native is loaded. The native
audit checks the JVM module's native-access state, requires a real LWJGL native
allocation to fail without its module grant, then succeeds with the grant.
LWJGL wraps the denied load as `UnsatisfiedLinkError`; Ravel's unnamed module
must remain ungranted in both processes. The audit also exercises cross-thread
OpenAL calls and repeated device cleanup.

The explicit agent launches resolve the same agent jar on the module path as
the automatic module `dev.ravel.agent`, then pass it to `-javaagent`.
In this mode `Boot` does not append the jar to the bootstrap search: the legacy
game loader delegates Ravel classes to the system loader's named agent module.
JNI's `Hook` consequently receives only `--enable-native-access=dev.ravel.agent`.
The client additionally grants the four named LWJGL modules above; the dedicated
server has no LWJGL module or client native. `--illegal-native-access=deny`
remains enabled. Neither process grants `ALL-UNNAMED`, opens/exports modules,
uses Java 8 or Forge, or suppresses warnings. The agent launch checks the actual
`Hook` module identity/native access and the platform unnamed module's denied
access; the client additionally checks JNA and narrator remain ungranted.
The vanilla `native-audit` remains an independent narrower baseline.

## Recorded vanilla acceptance

The following runs passed on Temurin 27+35 with LWJGL 3.3.6. Durations below
are the automation interval for clients and the complete process interval for
the dedicated server, not a performance comparison.

| Run | Context / permissions | Duration | Result |
| --- | --- | --- | --- |
| Xvfb, software GL | OpenGL 4.5 compatibility, Mesa 25.2.8 | 25,351 ms | All client actions; 10 screenshots; exit 0 |
| Display `:0`, RTX 5090 | OpenGL 4.6, NVIDIA 580.178.04 | 28,492 ms | All client actions; 10 screenshots; exit 0 |
| Dedicated server | No native-access grant; no client native loaded | 2,573 ms | Started, seed checked, saved, stopped; exit 0 |

Both clients checked seed `8675309` in the live world and again after reopening.
The first tick wait observed 100 ticks under Xvfb and 100 on the GPU. Robot
input changed both position and yaw: software yaw changed from 0 to 49.95
degrees; GPU yaw changed from 0 to -64.05 degrees. Both passed actual GLFW
monitor-state checks for entering and leaving fullscreen, not just a Java flag.
Both logs contain `OpenAL initialized.` and `Sound engine started`, followed
by world saves and `Stopping!`, with no game ERROR/FATAL or stderr records.
The dedicated log contains `Seed: 8675309`, `Saved the world`, and the three
dimension-save messages before a successful process exit.

Both clients typed the world name and seed through actual GUI fields, observed
an OpenAL source in `AL_PLAYING`, and changed the selected hotbar slot from
0 to 1 with one wheel callback. Input covers delayed input-method character
commits as well as immediate key-associated characters; the GPU run exercised
the desktop's IBus input path. Robot clicks require actual GLFW window focus
and acknowledged button callbacks rather than assuming a mouse press arrived.

Evidence paths are relative to `tests/fixtures/local/legacy-platform/`:

- `evidence-xvfb/` and `evidence-display/`: `client.log`,
  `automation.complete`, `seed.txt`, `ticks.txt`, `player-states.txt`,
  `world-name.txt`, `seed-input.txt`, `wheel.txt`, and `audio.txt`.
- Each client directory contains `01-menu.png`, `02-create-world.png`,
  `03-world.png`, `04-walk-look.png`, `04b-resized.png`, `05-fullscreen.png`,
  `05b-windowed.png`, `06-pause.png`, `07-saved-menu.png`, and
  `08-reopened.png`.
- `evidence-server/`: `server.log`, `server.complete`, and `libraries.log`.
- `evidence-native-access/`: denied/granted allocation and audio lifecycle logs.
- `vanilla-linkage.tsv` and `audio-linkage.tsv`: complete adapted linkage gates.
- Tracked `jvm/legacy-platform/official-dependencies.sha256`: official input pins.

JDK 27's default terminal-deprecation warnings for `sun.misc.Unsafe` remain
visible; no warning-suppression or unsafe-memory-access compatibility flag is
added. Offline test credentials produce an informational Realms authorization
message; this procedure does not claim online authentication or Realms access.

The proof does not exercise delayed input-method commits across screen changes
such as opening chat. GLFW reports keys and committed characters separately;
late character-only events are preserved rather than blocking the keyboard on
an unmatched printable key. Cross-screen input-method parity is not established
by these runs.
