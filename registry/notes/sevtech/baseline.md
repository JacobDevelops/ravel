# Original SevTech comparator baseline

## Scope and claim boundary

This is the original SevTech: Ages 3.2.3 distribution: Minecraft 1.12.2,
Forge 14.23.5.2860, all 254 acquired and five bundled mod jars, and the
publisher archive's complete overrides. The input ledger is
`registry/inventory/sevtech-ages-3.2.3/references.json`; preparation verifies
archive, manifest, identities, sizes, SHA-1 and SHA-256 before constructing
an isolated offline Prism instance. No optimisation mod is removed.
Java 8 is exclusively the original comparator runtime and observer compiler;
Ravel's checked-in toolchain remains Temurin 27.

The automated creative-world scenes measure a reproducible rendering and
world-generation workload, not progression completion or late-age machine
parity. A loaded-chunk stress route is not a factory simulation. These results
cannot establish WP9 behavioural acceptance or a speedup for Ravel.

## Host and launch envelope

| Item | Observed configuration |
| --- | --- |
| CPU | AMD Ryzen 7 9800X3D, 8 cores / 16 threads |
| CPU frequency policy | `performance`; boost enabled; no fixed clock |
| OS-visible RAM | 49,569,288,192 bytes |
| GPU | NVIDIA GeForce RTX 5090, 32,607 MiB reported VRAM |
| Driver / OpenGL | NVIDIA 580.178.04 / OpenGL 4.6.0 |
| Kernel | Linux 7.0.0-38-generic, x86_64 |
| Original JVM | Temurin 1.8.0_504-b01, HotSpot 25.504-b01 |
| Cleanroom comparator JVM | Temurin 27+35 (repository pin `temurin-27.0.0+35`) |
| Launch data format | Isolated Prism 11.0.3-compatible instance |

The approved launch path invokes Java directly from resolved component
metadata, not through the Prism GUI. Official Prism 11.0.3's startup wizard
requires an owned non-offline account even when `--offline` is supplied:
[`Application.cpp`](https://github.com/Prism%4Cauncher/Prism%4Cauncher/blob/11.0.3/launcher/Application.cpp)
and [`MinecraftAccount.h`](https://github.com/Prism%4Cauncher/Prism%4Cauncher/blob/11.0.3/launcher/minecraft/auth/MinecraftAccount.h)
define those independent gates. No account-ownership flag, authentication
token, launcher binary, or server is altered to bypass authentication.
The generated offline identity is passed only to the local singleplayer
JVM. Ravel candidates use the same direct-process launch envelope.

## Reproduction

Private inputs and outputs live under
`tests/fixtures/local/sevtech-ages-3.2.3/baseline/`; none is distributable
proof or a checked-in game artifact. Install the comparator without modifying
`mise.toml`:

```sh
mise install java@temurin-8.0.504+1
mise exec -- numen run -- python3 registry/sevtech/bench/run.py --build
DISPLAY=:0 mise exec -- numen run -- python3 registry/sevtech/bench/run.py cold-1 --cache cold-data-cache
DISPLAY=:0 mise exec -- numen run -- python3 registry/sevtech/bench/run.py warm-1 --cache warm-filesystem
```

Alternatively, `DISPLAY=:0 mise exec -- python3 registry/sevtech/bench/run.py --matrix original`
runs three cold/warm pairs, acquiring the lock separately for each trial.
Each invocation independently holds the host-wide check lock; builds never overlap
measurements. Never substitute Xvfb, llvmpipe, a nested display, or Java 8 for
a Ravel run. `run.py` refuses any display other than `:0`. The launcher uses
only a generated offline `BenchOffline` identity; no Microsoft token is
read. The comparison launches a fresh JVM and a fresh fixed-seed world every
time, with a 30-minute explicit trial deadline. Existing trial names are
refused rather than silently overwritten.

`cold-data-cache` synchronises dirty pages and requests
`POSIX_FADV_DONTNEED` for the instance mod jars and cached Java libraries;
the exact advised files and sizes are recorded.
This is scoped data-cache eviction, **not** a reboot, global kernel cache
flush, or proof that metadata, native binaries, assets, or the storage device
cache are cold. `warm-filesystem` retains those caches, but never reuses a
JVM, loaded classes, world, or heap. First dependency downloads belong in a
separate setup trial, not a measured launch. Raw pins record all observed
artifact bytes; changed or missing immutable pins fail preparation. Mutable
launcher metadata is recorded separately and may refresh before a trial.

Reduce only completed trials, naming them explicitly so failed validation
attempts in the same `runs/` directory are excluded:

```sh
B=tests/fixtures/local/sevtech-ages-3.2.3/baseline
python3 registry/sevtech/bench/summarize.py --output $B/report \
  $B/runs/original-cold-data-cache-{1,2,3} $B/runs/original-warm-filesystem-{1,2,3}
```

The reducer requires at least three completed trials per instance/cache
group. Per-trial nearest-rank p50/p95/p99 are kept separate; report
quantiles are distributions of trial metrics, never pooled frames pretending
to be independent trials. Raw game/launcher logs remain private because
they can contain machine paths and offline-account identifiers.

## Fixed workload and configuration

- Windowed 1280 × 720, fullscreen false, vsync false, render distance 12,
  unlimited FPS, no pause on lost focus, audio muted.
- Minimum heap 4096 MiB, maximum 8192 MiB; no invented tuning beyond the
  publisher's [recommended Java arguments](https://github.com/DarkPacks/SevTech-Ages/wiki/Recommended-Java-Args):

```text
-XX:+UseG1GC -Dsun.rmi.dgc.server.gcInterval=2147483646 -XX:+UnlockExperimentalVMOptions -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M
```

Additional observer-only arguments are `-javaagent:<observer.jar>=<trial>`,
`-Xloggc:<trial>/gc.log`, `-XX:+PrintGCDetails`, `-XX:+PrintGCDateStamps`, and
`-XX:+PrintGCTimeStamps`. The agent compiles against ASM 9.6 with
`javac -Xlint:all -Werror` and packages ASM 9.9.1 for JDK 27 class support.
Both dependency SHA-256 values are pinned in `run.py`; the assembled observer
JAR has deterministic ZIP timestamps.

- Seed 8675309, creative, default world type, structures enabled, hardcore
  false, commands enabled. Each trial removes only its private
  `RavelBenchmark` world before launching.
- Main menu is detected through a rendered vanilla or CustomMainMenu frame;
  after five seconds the agent starts the integrated server directly.
- After world readiness: freeze daylight/weather, set noon/clear weather,
  enable creative flight, settle 30 seconds. SevTech's first-join welcome
  panel (`com.gendeathrow.mpbasic.client.gui.GuiInfoPanel`) opens about
  three seconds into the settle scene and pauses the integrated server. During
  settle the agent closes any game-pausing screen with
  `displayGuiScreen(null)`, the same action as a player closing it. It
  records `screen` and `screen_dismissed` events. The trial fails if the game
  is paused during any measured scene.
- Spawn idle: 60 seconds at world-spawn-centred x/z, y=140, pitch 20°.
- Exploration: 120-second 256-block square, 30 seconds per side with the
  source-defined smoothstep trajectory and heading changes.
- Loaded-chunk stress: a 128-block square for 120 seconds, remaining inside
  the 192-block view radius after exploration. Every actual position,
  heading, and scene-relative timestamp is written to `trajectory.csv`;
  execution timing does not claim fixed FPS.

## Observability and comparability

The premain observer instruments actual client game-loop and integrated
server tick boundaries; missing hook execution is fatal. `frames.csv`
records intervals between consecutive frame exits, including observer
bookkeeping between them, while `ticks.csv` records tick execution time.
The first frame interval is omitted; frame intervals retain the preceding
scene. Observer bookkeeping and trajectory writes are not zero overhead;
comparators must use the same observer and scenes before numerical claims.
A Ravel runtime is supplied with `--instance ravel --candidate SPEC.json`;
the specification identifies the actual JDK 27 executable, classpath, main
class and argument arrays, never a shell command. It does not substitute
Java 8 or pretend an unavailable Ravel runtime exists. Its original pack
inputs, fixed scenes, sampling and direct-process timing are unchanged.

The private candidate JSON requires `javaPath` (the actual JDK 27 executable),
`classpath` (existing absolute artifact paths), `sha256` (an object keyed by
those absolute paths), `mainClass`, `jvmArgs`, and `gameArgs` (token arrays).
Argument tokens may contain `{game}`, `{assets}`, `{natives}`, `{agent}`, or
`{output}`. Preparation verifies every classpath hash and the executable's
reported Java version. It adds the shared observer and GC logging unless
already supplied; a different or additional Java agent is rejected.
Use `--matrix ravel --instance ravel --candidate SPEC.json` for three
separately locked pairs. Keep the same observer JAR across comparators;
the reducer rejects changed observer, fixture, artifact count, or options.

`events.csv` records main-menu and world readiness, scene transitions, hook
confirmation, and orderly JVM shutdown. JVM heap and class-loading counters
are sampled each second. `/proc` provides RSS/high-water RSS and cumulative
process CPU ticks; CPU percent is wall-time-normalised and may exceed 100%
on multiple cores. GPU VRAM/utilisation is sampled with `nvidia-smi` once a
second and is **device-wide**, including the desktop and other processes.
Steady values are final-scene 30-second means ending at shutdown request,
not peak substitutes or shutdown/save measurements.
Java GC pause distributions come from the raw GC log and cover the whole JVM
run. Hardware, driver, renderer, CPU and kernel reports accompany every
trial. Wall-to-menu starts immediately before the direct JVM spawn, while
agent-start-to-menu excludes pre-premain work. Startup phases are gaps
between ordered Forge lifecycle log lines: FML launch, `LWJGL Version`
(display created), mods identified, the first `Injecting itemstacks` (end of
pre-initialisation), the first `textures-atlas` creation (texture stitching),
the second `Injecting itemstacks` (end of initialisation) and
`successfully loaded N mods` (end of post-initialisation). Log timestamps
have one-second resolution, and absent anchors are not inferred.

Python syntax and reducer contract tests are wired into the existing
`registry/sevtech/verify.sh` gate, reached by `ravel:registry-verify` and
`ravel:verify`.

## Original SevTech results (Temurin 8)

Six trials completed on 2026-10-18, in the order cold-1, warm-1, cold-2,
warm-2, cold-3, warm-3. Each trial held the host check lock separately.
Other agents' lock holders (builds and one other `:0` run) ran between some
trials, and the gaps between a cold trial ending and its warm partner
starting were 49–545 s. `warm-filesystem` therefore means only that nothing
deliberately evicted the caches. Each trial took 468.8–483.1 s of wall time.
Every trial recorded 48,662–85,733 frame intervals and 1,197–2,403 server
ticks per measured scene, plus all four expected scene transitions, both
hooks, and orderly shutdown.

Effective arguments, with private paths shown as placeholders:

```text
java -XX:+UseG1GC -Dsun.rmi.dgc.server.gcInterval=2147483646 -XX:+UnlockExperimentalVMOptions
  -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M
  -javaagent:<observer.jar>=<trial> -Xloggc:<trial>/gc.log -XX:+PrintGCDetails
  -XX:+PrintGCDateStamps -XX:+PrintGCTimeStamps
  -Dlog4j.configurationFile=<assets>/log_configs/client-1.12.xml -Xms4096M -Xmx8192M
  -Djava.library.path=<natives> -cp <52 pinned entries> net.minecraft.launchwrapper.Launch
  --username BenchOffline --version 1.12.2 --gameDir <instance game directory>
  --assetsDir <assets> --assetIndex 1.12 --uuid <BenchOffline offline UUID> --accessToken 0
  --userType legacy --versionType release --width 1280 --height 720
  --tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker
```

With three trials per group, the reducer's nearest-rank p50 is the median
trial and its p95 and p99 are both the maximum trial. The table therefore
shows p50 and max. Frame and MSPT rows are per-trial nearest-rank
percentiles of frame intervals or integrated-server tick durations. Steady
values are means over the final 30 seconds of `loaded_chunk_stress`.

| Metric | Cold p50 | Cold max | Warm p50 | Warm max |
| --- | ---: | ---: | ---: | ---: |
| Wall clock: JVM spawn → main menu (s) | 116.7 | 122.2 | 111.4 | 111.7 |
| Agent premain → main menu (s) | 116.4 | 120.8 | 111.2 | 111.2 |
| World creation → player in world (s) | 22.45 | 22.50 | 22.37 | 22.55 |
| Frame interval spawn_idle p50 (ms) | 0.671 | 0.703 | 0.668 | 0.719 |
| Frame interval spawn_idle p95 (ms) | 1.16 | 1.20 | 1.08 | 1.23 |
| Frame interval spawn_idle p99 (ms) | 13.11 | 13.51 | 12.93 | 13.66 |
| Frame interval square_exploration p50 (ms) | 0.966 | 0.968 | 0.964 | 0.967 |
| Frame interval square_exploration p95 (ms) | 3.70 | 3.80 | 3.69 | 3.76 |
| Frame interval square_exploration p99 (ms) | 13.98 | 14.58 | 14.09 | 14.11 |
| Frame interval loaded_chunk_stress p50 (ms) | 1.05 | 1.06 | 1.05 | 1.05 |
| Frame interval loaded_chunk_stress p95 (ms) | 3.69 | 3.76 | 3.69 | 3.69 |
| Frame interval loaded_chunk_stress p99 (ms) | 13.73 | 14.05 | 13.59 | 13.69 |
| MSPT spawn_idle p50 (ms) | 2.48 | 2.70 | 2.50 | 2.52 |
| MSPT spawn_idle p95 (ms) | 4.58 | 5.36 | 3.78 | 4.30 |
| MSPT spawn_idle p99 (ms) | 6.72 | 7.39 | 6.07 | 6.10 |
| MSPT square_exploration p50 (ms) | 3.39 | 3.45 | 3.27 | 3.55 |
| MSPT square_exploration p95 (ms) | 55.93 | 56.22 | 56.64 | 57.17 |
| MSPT square_exploration p99 (ms) | 72.26 | 72.61 | 71.65 | 71.72 |
| MSPT loaded_chunk_stress p50 (ms) | 2.86 | 2.91 | 2.64 | 2.97 |
| MSPT loaded_chunk_stress p95 (ms) | 5.16 | 5.54 | 4.86 | 5.10 |
| MSPT loaded_chunk_stress p99 (ms) | 19.20 | 20.37 | 17.58 | 18.14 |
| GC pause p50 (ms) | 17.40 | 18.47 | 16.71 | 17.76 |
| GC pause p95 (ms) | 65.11 | 73.66 | 64.13 | 76.33 |
| GC pause p99 (ms) | 2,321 | 2,354 | 2,341 | 2,380 |
| GC pause max (ms) | 2,519 | 2,531 | 2,440 | 2,510 |
| GC pause total (s) | 14.96 | 15.14 | 14.67 | 15.65 |
| Heap used peak (MiB) | 6,135 | 6,208 | 6,124 | 6,160 |
| Heap used steady (MiB) | 4,826 | 4,901 | 4,757 | 4,826 |
| Heap committed steady (MiB) | 7,968 | 7,968 | 7,968 | 7,968 |
| RSS peak (MiB) | 12,568 | 12,714 | 12,570 | 12,662 |
| RSS steady (MiB) | 12,566 | 12,707 | 12,569 | 12,661 |
| Process CPU, whole run (% of one CPU) | 207.4 | 211.7 | 210.9 | 215.6 |
| Process CPU steady (% of one CPU) | 174.6 | 175.6 | 169.6 | 176.5 |
| Device VRAM peak (MiB, device-wide) | 2,518 | 2,521 | 2,519 | 2,520 |
| Device VRAM steady (MiB, device-wide) | 2,517 | 2,520 | 2,518 | 2,519 |
| Loaded classes peak | 60,988 | 60,992 | 60,995 | 61,019 |
| Loaded classes steady | 60,986 | 60,991 | 60,994 | 61,018 |

Each trial logged 249–260 GC pauses, including exactly four
`Full GC (System.gc())` collections of 2.0–2.5 s. In every trial, the
JVM-uptime timestamps of all four fall inside the `world_creation` scene,
after the menu wait and before `world_ready`. They account for the GC
p99/max values and about 9 s of the roughly 22 s world creation. The
remaining pauses are G1 young or mixed collections.

Startup phases are gaps between consecutive Forge anchors, in seconds,
listed as median (min–max) over three trials:

| Gap ending at | Cold | Warm |
| --- | ---: | ---: |
| Display created (from FML launch) | 5 (4–5) | 4 (3–5) |
| Mods identified | 3 (3–4) | 3 (3–3) |
| Pre-initialisation complete | 36 (35–39) | 35 (34–35) |
| Texture atlas created | 20 (19–20) | 17 (17–17) |
| Initialisation complete | 27 (26–29) | 26 (26–27) |
| Post-initialisation complete (273 mods loaded) | 25 (24–25) | 24 (24–25) |

## Validation attempts and failure causes

| Trial | Runtime | JVM-process envelope (s) | Complete | Disposition |
| --- | --- | ---: | --- | --- |
| `original-validation-1` | Temurin 8 | 5.10 | no | Observer in the unnamed package collided with signed Minecraft classes during BetterFPS common-superclass resolution; observer moved to `dev.ravel.metrics` |
| `cleanroom-validation-1` | Temurin 27 | 31.07 | no | Observer ASM 9.6 rejected class-file major 69; runtime ASM is now 9.9.1 |
| `original-validation-2` | Temurin 8 | 480.31 | invalid | Welcome panel paused the integrated server for every measured scene; excluded |
| `original-validation-3` | Temurin 8 | 203.42 | no | New pause guard named the welcome panel at `spawn_idle` start |
| `original-validation-4` | Temurin 8 | 475.59 | yes | First valid run with dismissal; validation only, not in the matrix |
| `cleanroom-validation-2` | Temurin 27 | 4.08 | no | Cleanroom cannot load the unmodified pack; see below |

`original-validation-2` reached every scene, but the game log shows
`Saving and pausing game...` about three seconds after world readiness,
with no later resume. Its tick p50 was about 0.4 µs because
`IntegratedServer.tick` returns early while paused, so its frame and tick data
are not a workload measurement. `original-validation-3` also showed that
FML's security manager traps `Runtime.halt` (`ExitTrappedException`). The
second failure then overwrote the first failure in `error.txt`. The observer now
appends failures instead of truncating, and it tolerates a trapped halt.
`run.py` terminates the process group once `error.txt` exists.

Cleanroom 0.7.8-alpha on Temurin 27+35 (components `net.minecraft` 1.12.2,
`org.lwjgl` 3.4.3, `net.minecraftforge` 0.7.8-alpha; 259 unchanged jars)
**cannot launch SevTech: Ages 3.2.3 unmodified**. FML coremod injection
fails before any game class loads: Farseek-1.12-2.5's `FarseekCoreMod`
throws `NoClassDefFoundError: scala/collection/Seq`, and the earlier
transformer registration fails on `scala/Function1`. FML then logs
`Unable to launch`. The cause is the runtime classpath. Forge
14.23.5.2860's component supplies nine Scala 2.11 libraries, including
`scala-library-2.11.1.jar`, on the original 52-entry classpath. Cleanroom's
`net.minecraftforge` component supplies none of them on its 151-entry
classpath, and Farseek, which Streams requires, bundles no Scala runtime.
Upstream Cleanroom's [modpack guide](https://cleanroommc.com/wiki/end-user-guide/preparing-your-modpack)
does not list Farseek or Scala. Running it would require adding a Scala
runtime library or mod, which this comparator's no-mod-change rule
forbids. No Cleanroom frame, MSPT, GC or startup distribution exists.

Raw commands, input pins, JVM/GC/process samples, game logs and the private
reducer output (`report/summary.{json,md}`) are retained under the private
baseline's `runs/` and `report/` directories. Preparation resolved both
runtimes with all 259 original jars, without mod removals.
