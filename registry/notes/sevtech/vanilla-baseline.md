# Vanilla 1.12.2 renderer baseline (WP5 slice 1)

This is a **vanilla-only** renderer baseline, not SevTech acceptance or a WP5
renderer replacement result. Both clients load the pinned official Minecraft
1.12.2 client without Forge, pack mods, or native fixture mods. The Ravel path
uses the generated legacy premain agent and an empty-composition native host,
JDK 27, and the LWJGL3 legacy platform;
the comparator uses the official Java 8/LWJGL2 runtime path. Runtime/JVM/platform
differences are intentionally part of this loader-path comparison, not an
isolated renderer optimization claim.

## Measurement contract

The existing `registry/sevtech/bench/` observer, private CSV/JSON protocol,
resolver, and reducer are reused. Candidate generation retains content-addressed
copies of the Ravel agent and screen sidecar, native host, platform, and pinned
LWJGL3 inputs before launch; commands record hashes for the actual runtime inputs. Resolution and
asset download finish before the launcher clock starts. All raw logs, runtime
pins, game saves, and account settings remain under ignored
`tests/fixtures/local/`; none are public evidence artifacts.

Candidate creation and every Ravel launch inspect the retained ELF dynamic
exports without executing the host: composed-host initialization exports are
required, and the fixture-only packet-receipt export is forbidden. The normal
version-matrix gate also tests the host with `--no-default-features` for each
live version, asserting that its composition contains no mods or registrations.

Each trial deletes its generated `RavelBenchmark` save. Settings are seed
8675309, 1280×720 windowed, vsync disabled, render distance 12, unlimited frame
cap, G1, and identical 4096 MiB initial / 8192 MiB maximum heaps. After world
readiness and 30 seconds settling, the common observer drives 60 seconds spawn
idle, 120 seconds exploration, and 120 seconds loaded-chunk stress. The motion
script and phase timings are shared with the existing SevTech baseline rather
than a second automation protocol. Legacy `ClientAutomation` is disabled only
for benchmark launches so its acceptance resize/fullscreen/quit sequence does
not interfere.

Frame intervals are measured at the existing client frame hook. The reducer
uses nearest-rank p50/p95/p99 **within each scene of each trial**; samples are
never pooled across repetitions. First-use proxies are the maximum frame
interval in each scene's first five seconds and the full-scene maximum, not
attribution of individual shader, texture, or model operations.

Chunk compilation/upload timings are observational benchmark-agent probes of
exact official owners and descriptors verified against the pinned Feather
mappings. These do not add renderer implementation hooks to the generated
loader contract. Timings include the method body and any work it invokes, not
GPU completion fences or exclusive CPU time. Probe availability and hook
installation are explicit; absent observations are not reported as zero work.

The one-second sampler records JVM heap, process RSS and `/proc` RSS high-water,
and device-wide NVIDIA VRAM usage. JVM nonheap/direct/mapped counters are
separately attributable counters, not estimates of all native memory. VRAM
includes driver/display/other device allocations and must not be described as
process-exclusive. GPU/driver, OpenGL renderer/display, CPU, and kernel identity
are retained privately. Functional-only Xvfb runs are excluded from physical
performance aggregation.

The mapping-bound observational probes are:

| Feather name | Official owner, name, descriptor | Meaning |
| --- | --- | --- |
| `RenderChunk.compile` | `bxr.b(FFFLbxl;)V` | Completed synchronous compilation call |
| `ChunkRenderDispatcher.uploadBuffer` | `bxm.a(Lbuk;Lcdz;)V` | Per-layer vertex-buffer submission |
| `ChunkRenderDispatcher.uploadGlList` | `bxm.a(Lbuk;ILbxr;)V` | Per-layer display-list submission |

Names/descriptors come from pinned Feather 1.12.2 build 31. Upload counts are
**calls, not unique chunks**; the two upload implementations are alternatives.
Shared observer overhead is present in both clients and is not subtracted.

These are observer-owned bytecode entry/exit timers, not new generated Ravel
renderer seams. The generated hook inventory has no compile/upload seam yet.
The compile timer surrounds `RenderChunk.compile`; the upload timer surrounds
the synchronous dispatcher submission, not its asynchronous enqueue wrapper.
Both upload alternatives contribute to the same call distribution; this does
not establish that unlike GL backends have equal work per call.

The JVM direct-buffer pool excludes native allocations not owned by that pool
(including LWJGL3 allocator/stack allocations), so it cannot establish total
off-heap savings or an allocator-equivalent comparison with LWJGL2. The runtime
log formats differ: JDK 27 records unified GC/safepoint logs, while Java 8 records
its supported GC log format. That observer/logging cost is included, not
subtracted or presented as an isolated renderer effect.

## Reproduction

Use the repository-pinned toolchain and a physical NVIDIA-backed `DISPLAY=:0`.
Build once, then retain the runtime candidate before any other legacy build can
replace the shared platform artifacts:

```sh
baseline=tests/fixtures/local/sevtech-ages-3.2.3/baseline
export baseline
mise exec -- numen run -- bash -c '
  set -euo pipefail
  cargo make ravel:legacy-agent-build
  RAVEL_MINECRAFT_VERSION=1.12.2 cargo build --locked \
    -p ravel-composed-host --release --no-default-features
  python3 registry/sevtech/bench/run.py --build
  python3 registry/sevtech/bench/vanilla.py \
    --write-candidate "$baseline/wp5-candidate.json" \
    --java "$(mise where java)/bin/java"
'
```

The observer build also runs a JVM verifier regression on Java 8 and JDK 27:
the actual transformer must preserve valid monitor-cleanup stack maps in the
official frame-method shape.

The candidate path must be new; existing cohorts are not overwritten. Separate
instances `vanilla-java8` and `vanilla-ravel` prevent reuse of pack mods/options.
The following matrices each run three independently locked warm-filesystem
trials. They do not claim a globally cold host or compare Xvfb against hardware.

```sh
DISPLAY=:0 mise exec -- python3 registry/sevtech/bench/run.py \
  --matrix wp5-java8 --instance vanilla --workload vanilla
DISPLAY=:0 mise exec -- python3 registry/sevtech/bench/run.py \
  --matrix wp5-ravel --instance ravel --workload vanilla \
  --candidate "$baseline/wp5-candidate.json"
mise exec -- numen run -- xvfb-run -a -s '-screen 0 1280x720x24' \
  env LIBGL_ALWAYS_SOFTWARE=1 python3 registry/sevtech/bench/run.py \
  wp5-ravel-functional --instance ravel --workload vanilla \
  --candidate "$baseline/wp5-candidate.json" --functional-only --timeout 900
python3 registry/sevtech/bench/summarize.py \
  "$baseline"/runs/wp5-java8-warm-filesystem-* \
  "$baseline"/runs/wp5-ravel-warm-filesystem-* \
  "$baseline"/runs/wp5-ravel-functional \
  --output "$baseline/wp5-report"
```

Choose fresh trial prefixes when reproducing. Incomplete diagnostic attempts
must not be globbed into the strict reducer. Its private JSON contains all
per-trial metrics and aggregate distributions over trial-level metrics.

Offline regression checks run with
`python3 -m unittest discover -s registry/sevtech/bench -p 'test_*.py'`.
Preparation isolation checks live in `test_prepare.py`; synthetic summary
fixtures live in `summary_test_support.py`. Neither requires a game launch.

## Recorded matched trials

Six fresh, independently locked warm-filesystem trials completed with exit 0;
the strict reducer accepted all six. Java 8 trials ran first, then Ravel trials.
These are vanilla worlds, not the modded SevTech numbers in `baseline.md`.

The physical host was AMD Ryzen 7 9800X3D, NVIDIA GeForce RTX 5090
(32,607 MiB device memory), NVIDIA driver 580.178.04, native `DISPLAY=:0`.
All six identity captures have the same GPU/driver/OpenGL renderer:
NVIDIA GeForce RTX 5090/PCIe/SSE2, OpenGL 4.6.0 NVIDIA 580.178.04.
Ravel used Temurin 27+35 and LWJGL 3.3.6; the comparator used
Temurin 8.0.504+1 and official LWJGL2. Both used the same observer build.

Trial prefixes are `wp5-java8-resumed` and `wp5-ravel-resumed`, each with
`-warm-filesystem-1`, `-2`, and `-3`. The retained private candidate was
`wp5-candidate-vanilla.json`; its empty legacy host SHA256 is
`914d1fc0a51de53f0a46a88d633ee6abae265c4e3e39671866b22449135b45dc`.

### Frame intervals and first-use proxies

All values are milliseconds. `Early max` covers the first five seconds of
that scene; `Max` covers its full duration. Idle/explore/stress are 60/120/120 s.

| Runtime | Trial | Scene | Frames | p50 | p95 | p99 | Early max | Max |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Java 8 | 1 | idle | 95676 | 0.610 | 0.675 | 1.020 | 2.813 | 8.295 |
| Java 8 | 1 | explore | 127157 | 0.899 | 1.205 | 1.536 | 5.522 | 44.523 |
| Java 8 | 1 | stress | 123980 | 0.946 | 1.061 | 1.450 | 1.942 | 61.233 |
| Java 8 | 2 | idle | 74778 | 0.754 | 1.021 | 1.515 | 5.361 | 8.096 |
| Java 8 | 2 | explore | 134346 | 0.853 | 1.111 | 1.447 | 6.958 | 25.549 |
| Java 8 | 2 | stress | 129327 | 0.888 | 1.085 | 1.444 | 5.131 | 61.401 |
| Java 8 | 3 | idle | 87215 | 0.693 | 0.827 | 1.092 | 3.848 | 36.101 |
| Java 8 | 3 | explore | 129659 | 0.888 | 1.144 | 1.414 | 2.924 | 18.569 |
| Java 8 | 3 | stress | 124308 | 0.942 | 1.074 | 1.434 | 7.077 | 60.317 |
| Ravel | 1 | idle | 94903 | 0.593 | 0.836 | 1.236 | 4.354 | 9.494 |
| Ravel | 1 | explore | 158805 | 0.716 | 0.981 | 1.295 | 4.096 | 9.363 |
| Ravel | 1 | stress | 153010 | 0.747 | 0.963 | 1.280 | 5.851 | 70.397 |
| Ravel | 2 | idle | 114900 | 0.505 | 0.574 | 0.903 | 2.654 | 7.829 |
| Ravel | 2 | explore | 152579 | 0.741 | 1.013 | 1.296 | 4.030 | 58.380 |
| Ravel | 2 | stress | 146228 | 0.794 | 0.987 | 1.302 | 5.533 | 8.453 |
| Ravel | 3 | idle | 113252 | 0.511 | 0.584 | 0.911 | 3.199 | 9.182 |
| Ravel | 3 | explore | 152514 | 0.746 | 0.993 | 1.292 | 4.572 | 63.837 |
| Ravel | 3 | stress | 144861 | 0.802 | 0.998 | 1.296 | 5.450 | 6.946 |

### Memory

MiB; steady values are the final stress-scene 30-second mean. Peaks stop at
the measurement endpoint, excluding shutdown saving. RSS peak is sampled
`VmHWM`; heap peak is used, not committed. VRAM remains device-wide.

| Runtime | Trial | Heap steady / peak | RSS steady / HWM | Device VRAM steady / peak | Nonheap steady | Direct steady | Mapped steady |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Java 8 | 1 | 1513.166 / 2670.088 | 5251.442 / 5260.449 | 2078.500 / 2103.000 | 92.369 | 1639.493 | 0.000 |
| Java 8 | 2 | 1525.669 / 2676.706 | 5320.685 / 5322.863 | 2082.533 / 2099.000 | 92.341 | 1639.500 | 0.000 |
| Java 8 | 3 | 1465.874 / 2684.251 | 5278.963 / 5293.840 | 2074.267 / 2091.000 | 92.587 | 1639.500 | 0.000 |
| Ravel | 1 | 1452.862 / 2696.801 | 5441.398 / 5443.570 | 2078.759 / 2098.000 | 101.359 | 1638.273 | 0.000 |
| Ravel | 2 | 1458.168 / 2731.884 | 5421.131 / 5424.457 | 2076.655 / 2083.000 | 100.547 | 1638.273 | 0.000 |
| Ravel | 3 | 1544.062 / 2731.769 | 5475.001 / 5476.582 | 2074.276 / 2080.000 | 101.242 | 1638.273 | 0.000 |

### Chunk compilation and synchronous upload

Counts are completed instrumented calls, not unique chunks. Times are ms;
total time sums overlapping worker calls and is not wall-clock elapsed time.
Tiny compile calls can be cancelled/no-work calls; they are not filtered out.

| Runtime | Trial | Scene | Operation | Count | Total | p50 | p95 | p99 | Max |
| --- | ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Java 8 | 1 | idle | compile | 144 | 226.969 | 1.402 | 3.368 | 4.607 | 5.217 |
| Java 8 | 1 | idle | upload | 407 | 0.620 | 0.001 | 0.004 | 0.011 | 0.021 |
| Java 8 | 1 | explore | compile | 17805 | 34638.715 | 0.001 | 11.895 | 27.047 | 53.716 |
| Java 8 | 1 | explore | upload | 28101 | 48.850 | 0.001 | 0.005 | 0.011 | 0.074 |
| Java 8 | 1 | stress | compile | 7214 | 16151.952 | 0.001 | 13.961 | 35.546 | 60.629 |
| Java 8 | 1 | stress | upload | 11556 | 20.232 | 0.001 | 0.004 | 0.011 | 0.038 |
| Java 8 | 2 | idle | compile | 161 | 240.033 | 1.294 | 2.980 | 4.576 | 5.048 |
| Java 8 | 2 | idle | upload | 36446 | 31.851 | 0.001 | 0.002 | 0.003 | 0.018 |
| Java 8 | 2 | explore | compile | 12237 | 32868.914 | 0.041 | 14.638 | 31.332 | 55.928 |
| Java 8 | 2 | explore | upload | 82380 | 119.095 | 0.001 | 0.004 | 0.007 | 0.061 |
| Java 8 | 2 | stress | compile | 5213 | 16240.690 | 0.039 | 17.435 | 37.514 | 52.401 |
| Java 8 | 2 | stress | upload | 76086 | 121.003 | 0.001 | 0.004 | 0.006 | 0.045 |
| Java 8 | 3 | idle | compile | 150 | 212.244 | 1.184 | 3.095 | 4.147 | 4.515 |
| Java 8 | 3 | idle | upload | 424 | 0.568 | 0.001 | 0.004 | 0.007 | 0.013 |
| Java 8 | 3 | explore | compile | 17027 | 31717.298 | 0.001 | 11.794 | 27.375 | 50.824 |
| Java 8 | 3 | explore | upload | 25945 | 42.623 | 0.001 | 0.004 | 0.010 | 0.053 |
| Java 8 | 3 | stress | compile | 7189 | 15941.939 | 0.001 | 14.292 | 33.233 | 56.227 |
| Java 8 | 3 | stress | upload | 11548 | 26.424 | 0.001 | 0.004 | 0.011 | 6.589 |
| Ravel | 1 | idle | compile | 159 | 267.749 | 1.373 | 3.632 | 5.044 | 5.294 |
| Ravel | 1 | idle | upload | 36445 | 38.413 | 0.001 | 0.002 | 0.004 | 0.019 |
| Ravel | 1 | explore | compile | 12718 | 36378.156 | 0.048 | 15.429 | 31.552 | 54.092 |
| Ravel | 1 | explore | upload | 83563 | 134.031 | 0.001 | 0.004 | 0.007 | 0.096 |
| Ravel | 1 | stress | compile | 5114 | 16911.013 | 0.041 | 18.970 | 38.131 | 53.392 |
| Ravel | 1 | stress | upload | 76025 | 136.376 | 0.001 | 0.004 | 0.007 | 0.036 |
| Ravel | 2 | idle | compile | 158 | 243.284 | 1.240 | 3.740 | 5.422 | 5.452 |
| Ravel | 2 | idle | upload | 449 | 0.714 | 0.001 | 0.005 | 0.010 | 0.016 |
| Ravel | 2 | explore | compile | 17166 | 33259.746 | 0.001 | 11.997 | 28.283 | 53.684 |
| Ravel | 2 | explore | upload | 25757 | 45.122 | 0.001 | 0.004 | 0.010 | 0.065 |
| Ravel | 2 | stress | compile | 7373 | 17034.574 | 0.001 | 14.337 | 35.501 | 59.977 |
| Ravel | 2 | stress | upload | 11598 | 21.707 | 0.001 | 0.004 | 0.011 | 0.179 |
| Ravel | 3 | idle | compile | 160 | 234.244 | 1.198 | 3.130 | 4.006 | 5.218 |
| Ravel | 3 | idle | upload | 444 | 0.608 | 0.001 | 0.003 | 0.005 | 0.012 |
| Ravel | 3 | explore | compile | 17809 | 36378.282 | 0.001 | 12.392 | 28.815 | 51.154 |
| Ravel | 3 | explore | upload | 27180 | 48.304 | 0.001 | 0.004 | 0.010 | 0.098 |
| Ravel | 3 | stress | compile | 7397 | 17812.496 | 0.001 | 15.194 | 38.268 | 61.252 |
| Ravel | 3 | stress | upload | 11640 | 20.991 | 0.001 | 0.004 | 0.010 | 0.074 |

### Functional run and interpretation

`wp5-ravel-functional-resumed-1` completed under Xvfb `:99`, Mesa 25.2.8,
llvmpipe LLVM 20.1.2, OpenGL 4.5 compatibility profile, exit 0. It observed
19 rewritten classes, 29 methods and 63,719 native hook invocations, with a
distinct legacy class loader. Its timings are excluded from every comparison.

The physical trials show lower central frame intervals on this Ravel launch
path, but not an isolated renderer improvement: the renderer is still vanilla,
JVM/platform/logging differ, order was not randomized, and first-use maxima
vary. Ravel RSS was higher in all three trials. Compilation and upload work
counts also vary with asynchronous scheduling and scene execution; they are
not evidence of missing or saved unique chunks. No replacement-renderer,
total-native-memory saving, or general performance-superiority claim follows.

The commands actually used were the reproduction commands above with matrix
prefixes `wp5-java8-resumed` and `wp5-ravel-resumed`, candidate
`wp5-candidate-vanilla.json`, `--timeout 900`, and functional trial
`wp5-ravel-functional-resumed-1`. Reduction named exactly these six physical
directories and that functional directory, writing private `wp5-resumed-report`.
Interrupted diagnostic trials were not included.
