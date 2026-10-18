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
Additional local records include `evidence-harness-agent/`,
`evidence-xvfb-agent-dedicated/`, `evidence-server-agent-restart/` and unique
`packets-*` directories. The JSON distinguishes original/supplemental hooks,
missing obligations, actual process exits, persistence visit counts and errors.
No tracked acceptance evidence is rewritten by these tasks.

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
