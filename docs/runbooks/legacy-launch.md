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

## JVM permissions

The four LWJGL3 Java modules receive exactly
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
