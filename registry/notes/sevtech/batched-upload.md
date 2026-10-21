# Bounded Java-owned mesh submission

WP5 slice 5 keeps vanilla authoritative. The explicit `geometry-proof` renderer
selection is off by default: it submits complete opaque-cube CPU meshes into a
Java-owned offscreen target, never substitutes a partial chunk or overlays the
world. This is a bounded transfer/submission experiment, not vanilla appearance
parity or a speedup claim.

## Transfer and ownership

Two Rust workers share the existing eight outstanding slots and byte reservations.
Operation 4 peeks completion metadata without consuming it. One JNI call copies a
retained generation/key completion into a Java-owned byte array; operation 5
acknowledges that exact completion. Allocation/publication failure retains native
ownership for retry. Generation invalidation cancels old work independently of
vanilla's task cancellation. Neither a stale copy nor incomplete geometry grants
permission to draw.

The little-endian version-1 wire format has a 48-byte header, five bytes per vertex
(local XYZ, face and raw light), four bytes per index, and twelve bytes per
material range (state key, first index, index count). Its maximum is 1,376,304
bytes, derived from 98,304 vertices, 147,456 indices and 24,576 material ranges.
The transfer does not add another retained native queue. JNI is batched per
completion, not per vertex. Wire-copy bytes, GL upload bytes, native staging
residency and GPU payload are separate quantities, not total process RSS.

The direct staging/readback allocation is bounded at 1,212,416 bytes; measured
GPU VBO payload is separately bounded at 1,179,648 bytes. Java expands wire XYZ
bytes into signed-short coordinates (six bytes per GPU vertex), because the
compatibility vertex-array API does not accept byte coordinates. Fixed framebuffer
attachment storage and driver overhead are not measured GPU payload. Copy,
upload and native draw durations are synchronous CPU timings, not GPU fences.
Copy counters count successful copy attempts, including a repeated copy after
transient oracle-lock contention, not distinct acknowledged completions.
The compatibility context selects core OpenGL 1.5 or ARB vertex buffers and
requires framebuffer support; it does not assume OpenGL 4.6. Missing capabilities
produce an explicit unsupported proof backend and no draw. Unexpected GL
allocation/submission failures and malformed native data fail the explicit
experiment rather than being acknowledged as successful geometry.
The accepted coordinate types are defined by the
[OpenGL vertex-pointer contract](https://registry.khronos.org/OpenGL-Refpages/gl2.1/xhtml/glVertexPointer.xml).

## Drawable scope

The native contract has no atlas sprite identifiers or UVs, biome/model tint,
per-vertex ambient occlusion or combined shading light. It does not establish
baked, animated, connected or translucent model fidelity. Only complete results
are candidates for the solid-geometry proof. Unsupported input is explicitly
refused rather than drawn with missing geometry as success.

The proof uses a 64-by-64 offscreen solid mask and orthographic section-local
projection along the negative Z axis. It observes nearest-surface silhouette
and depth, not hidden interior or back faces; matching this view is not complete
mesh equivalence. A CPU raster reference checks transfer/submission consistency
but is not a vanilla oracle. Vanilla comparison uses the game's own published
compiled section and actual solid-layer vertex buffer for the same generation,
identity and projection; sections with other nonempty layers are refused.
A missing, stale or unpublishable vanilla reference is a refusal, not a
zero-mismatch result. Both actual images must contain foreground pixels.
Texture and AO parity remain unproved even when solid-mask coverage agrees.

The association is section/task/publication identity, not an atomic historical
world snapshot: capture reads the mutable region after vanilla compilation.
There is no claim that both meshes saw identical block values at every earlier
instant. The actual published vanilla VBO remains the comparison authority, and
observed differences must not be reclassified as successful appearance parity.

## Measurement protocol

Compare selection off and on using the slice-1 vanilla comparator: seed
`8675309`, 1280-by-720 windowed, vsync off, render distance 12, unlimited frame
cap, G1, 4,096 MiB initial and 8,192 MiB maximum heap. Each fresh-save trial has
readiness plus 30 seconds settling, then spawn-idle, exploration and loaded-chunk
stress scenes. Three individually locked trials per selection use the same
retained candidate and settings on the physical RTX 5090 display. Report frame
p50/p95/p99 per scene and trial, never pooled percentile arithmetic. Selection-on
continues vanilla compilation and drawing, so its difference measures extra
proof overhead rather than replacement-renderer savings. Slice-1 results are
historical context, not a like-for-like speedup comparator for later worker and
copy additions. A software Xvfb run is functional-only.

Both new selections set `ALSOFT_DRIVERS=null`: OpenAL still opens and mixes
through its null output device without speaker output. Slice 1 did not force
this driver, another disclosed reason not to interpret the historical numbers
as like-for-like optimization evidence.

## Exercised verification and acceptance limits

Rebased standalone all-target/all-feature clippy and legacy-agent build exited 0.
The selected silent Xvfb integrated/dedicated/restart gate exited 0, including
the generic transactional/retention fixtures. Earlier attempts correctly
refused stale identities or a mixed-scope section rather than inventing an
oracle. Live automation now observes the existing first fresh-terrain stress
teleport while compiles are active, before later movement invalidates the
resident. No task/origin/publication checks or pixel assertions were relaxed.

The actual published vanilla comparison used generation 5/key 143, 52 vanilla
vertices and one SOLID layer. Native and vanilla each covered 32 of 4,096 pixels;
RGBA, coverage and depth mismatch counts were all zero. This is one actual
section/projection, not appearance parity. Whole-session totals were 360 copy
attempts / 2,469,036 bytes / 1,513,807 CPU nanoseconds; four uploads / 36,096 bytes /
640,815 CPU nanoseconds; six submissions / 1,260,808 CPU nanoseconds.
Peak VBO payload was 25,776 bytes and direct staging/readback 1,212,416 bytes;
final residency, staging and GL object counts were zero. Raw JSON, RGBA, PNG
and live logs are retained under ignored
`legacy-platform/evidence-wp5-s5-rebased-selected/` fixtures. These functional
totals are separate from physical-frame timing evidence.

The final rebuilt default silent live gate also exited 0. The selected gate's
platform hash below identifies its tested artifact; the subsequent default
rebuild has SHA-256
`304794aecd670024740fc2cf62247ccc239dcb28ede061138bdf2529755d13a6`.
Entry comparison again finds only `ClientAutomation.class` differs from the
frozen measured artifact; the same unchanged source was rebuilt for both gates.

After removing `target/llvm-cov-target`, the locked full `ravel:verify` gate
exited 0 on the rebased implementation. Each version (`1.12.2`, `1.21.1`,
`26.1.2`) passed 4,345 workspace tests and one separate no-default-features
composed-host test, with zero skips. Clean coverage passed 4,345 tests and
reported 94.22% lines, 93.64% regions and 92.77% functions. The gate also passed
strict clippy, generated Java/harness matrices, all 24 package digest checks,
benchmark/tool tests, registry checks and stack validation. Existing generated
source fingerprints and package digests already matched the combined rebased
output; no regeneration of their reviewed values was necessary.

## Rebased physical on/off cohort

Six fresh physical trials completed with exit 0: three selection-off trials
(`vanilla`, still the Ravel candidate) followed by three `geometry-proof` trials.
Every trial held its own `numen run` lock. Both selections used one retained
candidate, SHA-256
`4a2cc78809286ac2ac6f3b8b504c067e8b16375db95d07d7abadb924366b3334`,
on the RTX 5090 / Ryzen 7 9800X3D, NVIDIA 580.178.04, JDK 27+35 and
LWJGL 3.3.6. The protocol retained seed 8675309, physical display `:0`,
1280×720 windowed, VSync off, unlimited frames, distance 12 and G1 4096/8192 MiB;
30-second settle, 60-second idle, 120-second exploration and 120-second stress.
Both strict three-trial reductions passed separately; renderer argv is part of
the cohort identity, so the six runs are not pooled into a single distribution.

The measured platform JAR SHA-256 is
`e832047a2ea9fdcb1829b0ec7b14001f5d8a858864bfa05bafc62a903496c31d`.
The subsequent live-acceptance platform JAR SHA-256 is
`9128f282b13594e5b9de25c1bece889dfe3dd2deb6aca8f977be951a1ce885b3`.
Entry-by-entry comparison finds only
`dev/ravel/legacy/launch/ClientAutomation.class` differs, from
`jvm/legacy-platform/src/main/java/dev/ravel/legacy/launch/ClientAutomation.java`.
It observes the strict published-section oracle during the first fresh-terrain
stress teleport rather than after movement settles. The physical trials use
their separate benchmark automation, never this live-acceptance class; renderer,
upload, benchmark and launch code are unchanged, so this difference cannot
affect the measured frame paths. The frozen candidate was not rebuilt.

Frame intervals below are milliseconds, each cell p50/p95/p99 by nearest rank.
Trial IDs are `wp5-s5-rebased-{off,on}-warm-filesystem-{1,2,3}`.

| Selection/trial | Spawn idle | Square exploration | Loaded-chunk stress |
| --- | ---: | ---: | ---: |
| off/1 | 0.5237/0.6634/0.9648 | 0.7405/0.9215/1.2484 | 0.7892/0.8902/1.2626 |
| off/2 | 0.5870/0.7905/1.1984 | 0.6915/0.8948/1.2168 | 0.7332/0.8998/1.2077 |
| off/3 | 0.5210/0.5651/0.9110 | 0.7420/0.9331/1.2476 | 0.7988/0.9349/1.2555 |
| on/1 | 0.5875/0.7063/0.9782 | 0.7371/0.9419/1.2518 | 0.7911/0.9285/1.2595 |
| on/2 | 0.5115/0.5563/0.9097 | 0.7452/0.9627/1.2700 | 0.8034/0.9175/1.2619 |
| on/3 | 0.5020/0.5459/0.9171 | 0.7346/0.9344/1.2597 | 0.7890/0.9365/1.2502 |

Copy/upload counters cover the whole session, not only measured scenes. Times
are CPU milliseconds around copy/upload/submission, not GPU execution duration.
All off trials reported zero copies, uploads, submissions and GL residency.

| On trial | Copies / bytes / ms | Uploads / bytes / ms | Submissions / ms | Peak VBO / direct bytes |
| --- | ---: | ---: | ---: | ---: |
| 1 | 6110 / 83883016 / 24.821139 | 376 / 6442608 / 13.907300 | 376 / 24.092827 | 68832 / 1212416 |
| 2 | 6019 / 80743124 / 24.323980 | 386 / 6525792 / 20.138781 | 386 / 25.672467 | 68832 / 1212416 |
| 3 | 6083 / 82397716 / 24.102700 | 359 / 5828352 / 13.797705 | 359 / 22.752330 | 68832 / 1212416 |

All on trials selected OpenGL15; final residency, staging, GL objects and
generation/key were zero. Refused whole meshes numbered 5734/5633/5724.
Physical timing runs perform no vanilla pixel comparison; the separate selected
live gate supplies that oracle. No missing comparison is interpreted as parity.

Memory below is MiB, peak / final-30-second mean (before saving). RSS is process
memory, heap is JVM used heap, and VRAM is whole-device sampled usage, not
attributed mesh memory.

| Selection/trial | RSS | Heap used | Device VRAM |
| --- | ---: | ---: | ---: |
| off/1 | 5419.8/5418.1 | 2692.5/1483.9 | 1435.0/1429.9 |
| off/2 | 5419.7/5417.5 | 2745.0/1460.3 | 1437.0/1433.9 |
| off/3 | 5435.7/5433.5 | 2687.7/1471.2 | 1435.0/1428.0 |
| on/1 | 5487.6/5485.5 | 2744.2/1508.8 | 1439.0/1433.9 |
| on/2 | 5461.2/5459.5 | 2736.5/1453.1 | 1435.0/1430.9 |
| on/3 | 5476.5/5474.5 | 2696.5/1420.5 | 1437.0/1431.9 |

Observed on-versus-off differences of the three trial-level medians are:

| Scene | p50 change | p95 change | p99 change |
| --- | ---: | ---: | ---: |
| Spawn idle | −2.33% | −16.14% | −4.95% |
| Square exploration | −0.45% | +2.21% | +0.97% |
| Loaded-chunk stress | +0.24% | +3.19% | +0.32% |

Median peak RSS increased 56.7 MiB, used heap 44.0 MiB and device VRAM 2 MiB;
final-30-second means changed +56.4/−18.1/+2 MiB respectively. These are observed
cohort differences, not isolated causal costs or a speedup claim. In particular,
stress maximum frame intervals were 6.40/13.90/52.51 ms on versus
6.53/7.62/7.52 ms off; tail outliers are not hidden by the percentile table.
Sequential order, three repetitions and changing admission schedules limit
attribution. Compared with slice 1, this candidate also includes bounded native
workers, copied snapshots, retained wire output and Java GL submission plus the
rebased host cleanup; null audio is another difference. Neither cohort is an
unmodified vanilla speed comparator. No world substitution is authorized.

Raw per-trial exports and separate strict off/on reports remain under the
ignored benchmark fixture's `baseline/runs/` and
`baseline/wp5-s5-rebased-{off,on}-report/` directories.

## Next: slice 6

Extend the generation-qualified CPU contract with UVs and atlas sprite identity,
model/biome tint and AO plus combined light per vertex. Resolve baked, animated,
connected and translucent model/material inputs from Java-owned state, including
sorting and reload invalidation. Demonstrate actual vanilla pixel fidelity on
the required scenes before considering default selection. Slice 7 owns default
cutover and matched replacement-renderer acceptance/ablations; this experiment
does not authorize either.
