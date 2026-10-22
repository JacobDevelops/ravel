# WP5 slices 6–8: bounded shaded baked comparisons and residency

## Status and boundary

Vanilla remains authoritative for ordinary world drawing. The off-by-default
`geometry-proof` selection submits independently emitted baked BLOCK vertices
to a bounded Java-owned offscreen GL comparison. Slice 8 additionally replays
complete static-camera world frames, substituting one acknowledged resident
section at a time while every other section and the rest of the world remain
vanilla. This is a selected acceptance proof, not a default renderer cutover.
It uses the live atlas and lightmap, model positions, UVs, shaded color, combined
light, alpha testing and translucent blending. Step two keeps every section
containing translucent vertices on vanilla, including its camera-dependent
resort. Step three also routes complete water/lava-containing sections to vanilla;
native fluid tessellation and replacement-renderer savings remain unsupported.

The input contract accepts actual face-culled vanilla model quads, not invented
cube approximations. Model resolution, connected/multipart selection, face
selection, tint providers, AO and light sampling remain vanilla work on the
owning compile thread. Rust receives only owned primitive values.

| Field | Provenance and computation |
| --- | --- |
| Adapter state | Version-local Java block-id/metadata encoding; Rust treats it as opaque |
| Model XYZ and atlas UV | Exact baked/builder source words before shading/translation |
| Sprite | Actual sprite identity, assigned a bounded generation-scoped Java numeric identity |
| Tint | Independently observed block-color provider result |
| AO | Independently observed AO-face brightness array |
| RGB multipliers | Actual per-vertex builder inputs, including vanilla tint/AO choices |
| Combined light | Actual per-corner packed lightmap inputs; not independent light propagation |
| Final XYZ | Rust adds source float to effective double offset rounded to float first |
| Final color | Rust reproduces little-endian integer component/float multiplication, truncation and shifts |
| Translucent order | Descending squared centroid distance, source ordinal for ties; cancellable in-place resort |
| Final vanilla buffer | Oracle after compile normal return, never substituted for emitted vertices |

Exact Feather-resolved inventory entries cover model rendering, baked vertices,
AO, tint, builder vertices/light/multipliers/position and reflected members. This
extends the 1.12.2 adapter, not the SDK game-object boundary. The Rust neutral
state API does not decode legacy block IDs or metadata.

## Bounds, selection and lifetime

The CPU value layer admits at most 65,536 quads, checking retained capacity and
length. Coordinates, UVs, multipliers/AO, offsets, transformed positions and
cameras must be finite. Actual completed-resource and atlas-install generations
are captured independently and must match, nonzero. Java rechecks resource and
lifecycle ownership after JNI emission and again before GL draining. Cancellation,
a failed resort or a retired generation never authorizes drawing.

Vanilla stitches its first block atlas from resource-listener registration after
the initial reload has completed. That out-of-reload install adopts the completed
resource generation; installs during a reload retain the active generation.
Before the first completed reload, generation zero remains inadmissible. This
avoids requiring an artificial F3+T reload before physical trials can admit
complete baked comparisons.

Selected capture admits one owning compile thread, 8,192 source quads and fixed
source/oracle/output buffers. Busy, oversized, uncolored, incomplete-provenance,
unaccounted final vertices (including fluid tessellation) and stale inputs are
explicitly refused, not truncated success. Game references are cleared at
retirement. A volatile queue avoids taking the comparison monitor on idle layer
hooks. GL capability refusal is an unsupported result, not a render-thread throw.
Temporary baked VBO storage is deleted after each comparison; no baked VBO remains
resident between comparisons. Client pointers and caller GL state are restored.

Dormant baked targets still undergo descriptor/shape validation but are neither
rewritten nor installed. Their transformer result is null, preserving original
class bytes rather than round-tripping the class. Both default and selected
harness modes exercise the eight typed hooks; malformed static/descriptor/array
shapes are refused without partial publication. Default-off live hook counters
measure zero baked native crossings. This is not a claim that all existing Ravel
instrumentation has zero overhead.

CPU comparison publishes quad count, mismatched BLOCK words, first mismatch,
CPU nanoseconds and retained/copied input/output bytes. Input bytes include the
full fixed JNI arrays plus the owned snapshot, not only used payload. CPU and GL
compared/stale/unsupported counters are separate. GL metrics report both arms'
actual uploaded bytes, peak transient VBO bytes, CPU upload-and-size-validation
nanoseconds and complete comparison nanoseconds; they are not GPU timer queries.
Private benchmark exports include the complete baked metric vector separately
from the earlier geometric upload/residency metrics. External Python validation
checks strict numeric types, bounds, independently captured generation agreement,
first mismatch, GL admission and every golden disposition, with malformed-evidence
negative tests. For controlled golden samples it independently recounts RGBA,
coverage, depth and foreground from fresh raw paired readbacks, checks the
reported vertex count against the raw header, and requires zero pixel mismatches.

## Golden-scene matrix retained, not narrowed

The final selected live Xvfb run measured all seven baked scenes below against
final vanilla buffers. Five designated 4-by-4-by-4 block arrangements occupy separate sections
within one chunk of the viewing platform, inside the harness's render-distance-2
window. Iron bars and glass panes exercise mipped cutout independently of fancy
graphics. The raw headers confirm layers 0/1/2/3 for opaque/mipped/unmipped/
translucent respectively. Counts are bounded 64-by-64 offscreen comparisons, not
controlled full-world screenshots. Python independently recounted every retained
paired RGBA/depth readback and agreed with every reported total.

| Required scene | Samples | RGBA / coverage / depth mismatches | Foreground pixels per arm | Vertices |
| --- | --- | --- | --- | --- |
| Opaque textured models | 1 | 0 / 0 / 0 | 256 | 384 |
| Cutout models, mipped | 1 | 0 / 0 / 0 | 188 | 5,184 |
| Cutout models, unmipped | 1 | 0 / 0 / 0 | 256 | 768 |
| Translucent models | 1 | 0 / 0 / 0 | 256 | 512 |
| Animated atlas sprites | 1 | 0 / 0 / 0 | 256 | 384 |
| Staged resource reload, all five sections | 5 | 0 / 0 / 0 | 1,212 | 7,232 |
| Staged world unload/reopen, all five sections | 5 | 0 / 0 / 0 | 1,212 | 7,232 |
| Translucent fluids | 0 | Unsupported, not a match | 0 | 0 |

Fluid tessellation does not carry baked-quad provenance and is explicitly
unsupported; it is not silently folded into the translucent-model result.
The final selected live run admitted 434 CPU comparisons and 23,454 quads with zero
mismatched BLOCK words and first mismatch -1. Busy/unsupported/stale counts were
335/211/0; all unsupported sections had unaccounted vertices, not Rust decoding
failures. Resource/atlas generations were both 2. The 59 GL comparisons uploaded
5,253,696 bytes with a 145,152-byte peak transient VBO, zero GL stale/unsupported
outcomes and zero pixel/coverage/depth mismatches. CPU upload validation took
843,283 ns cumulatively; complete comparisons took 43,889,224 ns.
These describe the exercised run, not an admission-rate guarantee.

## Frozen physical trials and verification

Six completed warm-filesystem trials used one frozen candidate with SHA256
`b65267c84b374df92ddc2f63ea81a58cadf7d66fff950fef98ef44ba9a54b8ea`:
three selection-off (`vanilla`) and three selection-on (`geometry-proof`).
The locked RTX 5090 display was `:0`; seed was 8675309, dimensions 1280-by-720,
render distance 12, unlimited frames, G1 and 4,096/8,192 MiB initial/maximum heap.
Temurin 27+35 and the null audio driver were identical across arms. These are
Ravel-versus-Ravel observations, not a vanilla speedup experiment. Private trial
names are `wp5s6d-off-warm-filesystem-1` through `-3` and the corresponding `on`
names; separate strict reductions retain every scene's compile/upload timing,
frame stalls and memory samples without pooling frame distributions.

Each frame cell below is the individual trial's p50/p95/p99 in milliseconds.
Memory columns are peak RSS, sampled used heap and whole-device VRAM in MiB;
VRAM is not per-process attribution.

| Trial | Spawn idle | Square exploration | Loaded chunk stress | RSS | Heap | VRAM |
| --- | --- | --- | --- | --- | --- | --- |
| Off 1 | 0.501/0.543/0.883 | 0.737/0.948/1.248 | 0.790/0.943/1.249 | 5443.2 | 2668.5 | 1437 |
| Off 2 | 0.500/0.547/0.870 | 0.735/0.954/1.257 | 0.789/0.949/1.282 | 5413.4 | 2688.6 | 1431 |
| Off 3 | 0.500/0.543/0.882 | 0.730/0.929/1.228 | 0.786/0.917/1.231 | 5416.9 | 2700.1 | 1437 |
| On 1 | 0.506/0.554/0.883 | 0.730/0.945/1.255 | 0.788/0.938/1.255 | 5484.1 | 2672.4 | 1437 |
| On 2 | 0.576/0.658/0.940 | 0.736/0.963/1.288 | 0.796/0.948/1.272 | 5502.5 | 2684.3 | 1447 |
| On 3 | 0.510/0.556/0.899 | 0.746/0.948/1.285 | 0.798/0.947/1.271 | 5517.4 | 2712.7 | 1443 |

Loaded-stress vanilla compile/upload hook durations below are observational CPU
durations, not GPU execution time. Each timing cell is p50/p95/p99 in milliseconds.
The geometry counters refer to the earlier geometric comparison, not shaded baked
residency. Its final resident/staging bytes and live buffer/framebuffer counts were
zero in every trial. Dormant selection had zero copies/uploads and zero baked
native comparisons in all three trials, in addition to the live hook-counter
check; no isolated nanosecond cost is inferred from whole-application differences.

| Trial | Compile | Upload | Geometry uploads / bytes | Peak geometric resident bytes |
| --- | --- | --- | --- | --- |
| Off 1 | 0.006/14.890/36.376 | 0.002/0.005/0.010 | 0 / 0 | 0 |
| Off 2 | 0.036/14.498/35.867 | 0.002/0.005/0.010 | 0 / 0 | 0 |
| Off 3 | 0.014/14.877/36.202 | 0.001/0.005/0.010 | 0 / 0 | 0 |
| On 1 | 0.206/14.456/36.843 | 0.002/0.005/0.010 | 392 / 6620640 | 68832 |
| On 2 | 0.229/14.617/36.326 | 0.002/0.005/0.011 | 391 / 6337056 | 68832 |
| On 3 | 0.211/14.295/36.593 | 0.002/0.005/0.010 | 357 / 6118896 | 68832 |

All six physical trials exited zero and passed the strict export gate. Each
selected trial admitted complete shaded comparisons; the dormant trials admitted
none. All selected BLOCK-word and RGBA/coverage/depth mismatch counts were zero,
with first mismatched word -1. All selected unsupported counts below are explained
by unaccounted vertices; Rust decode refusals were zero. Resource/atlas generations
were both 1, and no controlled-scene deferral ran in physical trials.

| Selected trial | CPU sections / quads | Busy / unsupported / stale | Shaded GL comparisons | Uploaded bytes / peak VBO bytes | Foreground pixels per arm |
| --- | --- | --- | --- | --- | --- |
| On 1 | 2914 / 362382 | 23275 / 882 / 112 | 533 | 81173568 / 359184 | 653877 |
| On 2 | 2934 / 378707 | 23292 / 921 / 115 | 538 | 84830368 / 378224 | 650948 |
| On 3 | 2927 / 361405 | 22485 / 911 / 103 | 532 | 80954720 / 360752 | 670549 |

The separate `wp5s6d-functional` Xvfb/software-GL trial also exited zero and
passed the strict gate: 2,601 CPU sections, 280,336 quads, 415 shaded GL comparisons,
62,795,264 uploaded bytes, a 354,816-byte peak VBO and 513,380 foreground pixels
per arm, with zero word/pixel/coverage/depth mismatches. It is excluded from the
physical reductions. These bounded comparisons do not establish full-world
fidelity or a resource saving versus vanilla.

Final gates exited zero: all-target/all-feature Clippy with warnings denied,
legacy agent build, default live, selected live and the complete verification
task after a fresh coverage-target removal. Each of Minecraft 1.12.2, 1.21.1 and
26.1.2 passed 4,377 workspace tests plus its empty-composition test; all three JVM
compile/harness rows and version gates passed. Coverage was 93.99% lines, 93.28%
regions and 92.59% functions. Independent read-only review found no blockers after
the hook, lifecycle, GL refusal and strict evidence-validation fixes.
An earlier selected live attempt timed out awaiting fresh geometry drains after
stress teleport 6; a fresh legacy build and selected live rerun passed without
changing that timeout, drain threshold or any golden assertion. A separate
attempt after verification had regenerated the modern agent and correctly refused
the non-legacy adapter before the successful legacy rebuild.

## Slice 7: generation-keyed shaded residency

Shaded comparison now retains one acknowledged native section and at most one
pending replacement. Each owns its source bytes and emitted words. Its identity
contains the renderer generation, monotonic lease key, resource and atlas
generations, and aligned section origin. The GL owner must acknowledge the exact
pending generation/key after comparison; refusal preserves the prior native
resident. Unload, atlas replacement, resource reload and shutdown retire both
slots, and stale acknowledgements cannot republish them.

Three Java-owned VBOs retain resident, pending and oracle layer ranges. Each reserves
917,504 bytes once per GL lifetime; later captures upload into the pending buffer,
then swap only after a complete comparison. A failed upload preserves the old resident.
All four layer ranges retain their source ordering, material data and translucency
state. Repeated render-layer observations reuse the acknowledged emitted VBO in
the existing offscreen proof target, not in place of vanilla world drawing.
The native reservation ceiling is 2,818,096 bytes per slot and 5,636,192 bytes for
resident plus pending; the retained metric counts vector capacities, not just
lengths. Temporary snapshot, mesh and staging storage remains separately bounded
and is not represented as retained residency. GL residency reserves 2,752,512
bytes for the resident, pending and oracle VBOs; this is not whole-device VRAM usage.

The controlled section-layer matrix is not full-world fidelity. Fluid
tessellation remains unsupported. Selection stays off by default and vanilla
remains authoritative. Default world cutover requires complete source provenance,
independent full-world golden scenes and fresh live acceptance; the documented
fallback remains the default vanilla renderer. No vanilla speedup is claimed.

Slice 7 bounded-residency acceptance is complete: one candidate frozen after the
round-16 fixes passed three off/three on locked RTX 5090 physical trials and a
separate Xvfb functional trial. The earlier physical tables remain slice-6
history; the fresh slice-7 results are below. This does not complete WP5 or
authorize world-renderer cutover.

The round-16 crossing cleanup pins the 232-byte record, 8,192-quad admission
limit, diagnostic codes and receipt offsets across Rust, Java and the Python
validator. A non-zero record fixture checks copied provenance, translation,
color multipliers, ambient occlusion and light. Capture admission, emission,
oracle comparison and publication accept isolated lifecycle state; tests inject
reload during comparison and refuse stale publication before any Java output
write. Worker completion publication remains transactional under invalidation.

### Slice-7 controlled live acceptance

The final three-buffer implementation passed legacy-agent build, default live,
selected geometry-proof live and the full verification task (all exit 0).
The selected Xvfb live gate independently validated the following controlled
readbacks. This is functional evidence, not the separate frozen-candidate
benchmark run or full-world acceptance.

| Scene | Comparisons | Color/depth/silhouette mismatches | Foreground pixels, emitted/oracle | Vertices |
| --- | ---: | --- | --- | ---: |
| Opaque | 1 | 0/0/0 | 256/256 | 384 |
| Cutout-mipped | 1 | 0/0/0 | 188/188 | 5,184 |
| Cutout | 1 | 0/0/0 | 256/256 | 768 |
| Translucent | 1 | 0/0/0 | 256/256 | 512 |
| Animated | 1 | 0/0/0 | 256/256 | 384 |
| Staged reload | 5 | 0/0/0 | 1,212/1,212 | 7,232 |
| Unload | 5 | 0/0/0 | 1,212/1,212 | 7,232 |

Translucent fluid remains refused because its tessellation lacks baked-quad
provenance. Repeated resident submissions do not establish composited full-scene
fidelity or moving-camera translucent reordering.

The post-cleanup selected live run recorded 84 native acknowledgements and 84 GL
replacements, with 480 native entries retired. Native combined retained capacity
peaked at 746,232 bytes; fixed GL reservation peaked at 2,752,512 bytes across
three buffers per pool. Three pools retired, and native and GL live residency
drained to zero. There were 12,376 repeated resident submissions and zero recorded
stale GL refusals. These are run-wide functional counters, not per-trial physical
memory measurements.

Post-cleanup verification passed 4,393 workspace tests with none skipped for each
of 1.12.2, 1.21.1 and 26.1.2, plus the no-feature composed-host test for each
version. Bootstrap tests passed 253 cases and the panic-redaction case. Coverage
was 93.26% regions, 92.62% functions and 93.94% lines. Workspace clippy with all
targets/features and warnings denied passed. Offline verification still reports
seven unavailable clean-room source trees out of eighteen; acquired-byte checks
requiring them were not exercised.

### Slice-7 frozen physical trials

Candidate `wp5s7b-candidate.json` has SHA-256
`c62c02f3e32066e262893996d969be8403d20238604ca0b47bc188d0ec2c1239`.
It was frozen after the source fixes, independent review and verification:
legacy agent `45d1f2fb6a9bbb193c20154c0e956f2d4198733d1b89fad268f78526ea77ee9f`,
release host `d36a61bb0665201ba629959b3f84900b94a6b0dab6da38f2111fb8da5480f930`.
All seven runtime cohorts record that candidate hash; no runtime sources changed
after freezing. Observer settings record agent SHA-256
`41c9eb91a6d0031d801da89a0a7162da379108baaf558ba29aee091000b259de`.
Raw local runs are `baseline/runs/wp5s7b-{off,on}-warm-filesystem-{1,2,3}`
and `baseline/runs/wp5s7b-functional` beneath the acquired SevTech fixture.

The six physical trials ran serially under the host check lock on RTX 5090,
DISPLAY `:0`, seed 8675309, slice-1 settings: 1280×720, distance 12, unlimited
FPS/VSync off, Temurin 27+35, G1, 4,096–8,192 MiB heap, null audio and all sound
categories muted. Both cohorts use Ravel over the vanilla 1.12.2 fixture; “off”
selects vanilla rendering and “on” selects the additional geometry-proof path.
This is a diagnostic overhead ablation, not a comparison against an unmodified
vanilla process. Every launcher/trial and all three summary commands exited 0.

Nearest-rank frame quantiles in milliseconds; each cell is p50/p95/p99 for that
trial and scene, never pooled across trials. Idle lasts 60 seconds; exploration
and loaded-chunk stress last 120 seconds each.

| Trial | Spawn idle | Square exploration | Loaded-chunk stress |
| --- | --- | --- | --- |
| Off 1 | 0.5088/0.5549/0.9041 | 0.7420/0.9500/1.2732 | 0.7968/0.9014/1.2476 |
| Off 2 | 0.5210/0.7225/1.0658 | 0.7441/0.9601/1.2747 | 0.7926/0.9029/1.2605 |
| Off 3 | 0.5110/0.6049/0.9369 | 0.7504/0.9928/1.3167 | 0.8062/0.9916/1.3122 |
| On 1 | 0.9719/1.3106/1.7134 | 1.0820/1.5294/1.9927 | 1.0642/1.4489/1.8152 |
| On 2 | 0.9320/1.2592/1.5610 | 1.1551/1.6070/2.1758 | 1.1606/1.5829/2.0243 |
| On 3 | 0.9114/1.2374/1.5035 | 1.1061/1.5129/1.9528 | 1.1763/1.5321/1.9019 |

Peaks below cover startup through measurement completion, excluding saving.
RSS is process peak RSS; heap is sampled JVM used heap; VRAM is sampled
whole-device usage, not attributed to this process. All heaps committed 4,096 MiB.
Native/GL columns are actual shaded retained-capacity/fixed-reservation peaks
in bytes, not total native memory and not RSS-minus-heap.

| Trial | Native bytes | GL bytes | RSS MiB | Heap-used MiB | Device VRAM MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| Off 1 | 0 | 0 | 5,421.52 | 2,724.64 | 1,437 |
| Off 2 | 0 | 0 | 5,465.16 | 2,732.63 | 1,516 |
| Off 3 | 0 | 0 | 5,383.86 | 2,684.50 | 1,503 |
| On 1 | 2,292,856 | 2,752,512 | 5,439.61 | 2,671.64 | 1,505 |
| On 2 | 1,819,512 | 2,752,512 | 5,546.86 | 2,708.27 | 1,507 |
| On 3 | 1,997,704 | 2,752,512 | 5,517.50 | 2,763.74 | 1,507 |

Every selected trial admitted independently emitted shaded comparisons and exact
native/GL residency replacement; every off trial kept shaded counters at zero.

| Selected trial | Emitted sections | GL-compared sections / replacements | Native retired | Resident submissions | Pixel comparisons |
| --- | ---: | ---: | ---: | ---: | ---: |
| On 1 | 2,196 | 584 | 2,196 | 1,211,568 | 1,095 |
| On 2 | 2,819 | 554 | 2,819 | 1,158,062 | 1,061 |
| On 3 | 2,882 | 556 | 2,882 | 1,180,000 | 1,038 |
| Xvfb functional | 2,622 | 446 | 2,622 | 39,206 | 863 |

All four selected runs had zero emitted-word and color/depth/silhouette
mismatches, positive and matching emitted/oracle foreground, zero stale GL
refusals, one three-buffer pool retired, and zero live native/GL reservations
after shutdown. Peaks remain below the 5,636,192-byte native ceiling and equal
the 2,752,512-byte fixed GL ceiling. The separate Xvfb software-GL run passed
with native peak 2,333,792 bytes; its timing and memory are not physical-GPU
performance evidence. The seven controlled golden rows above were independently
recounted by the selected live gate; workload pixel-comparison counts here are
not additional full-world golden scenes.

The selected path increases observed frame quantiles in these trials; no
speedup or resource saving is established. Off-3 stress also contains an
885.7 ms full-scene maximum, outside its first-five-second maximum of 5.548 ms;
quantiles must not conceal that outlier. Those frozen slice-7 readbacks compare
isolated section layers, not full-scene resident drawing. They do not establish
slice-8 composited fidelity or camera-dependent translucent reordering.
Fluids remain unsupported for missing baked-quad provenance. Ordinary world
drawing remains vanilla and selection remains off by default.

## Slice 8, step one: static composited resident world frames

The live harness replays vanilla's complete world render at the same fixed
partial tick, without advancing a game tick or changing player position/yaw/pitch
between the arms. A warmup draw precedes the vanilla arm. The second arm replaces
the published designated section's world-layer draw calls with its acknowledged
resident shaded VBO, retaining vanilla's actual camera transforms, material
state, surrounding terrain, sky and other world draws. It does not use the
oracle VBO as the replacement. The resident section is pinned until capture;
task publication and cancellation are checked before replay, and lifecycle
retirement clears the pending association. The selected transform skips vanilla
chunk-update work during replay, freezing main-thread chunk uploads/scheduling
across all arms rather than merely checking the game tick and camera.

Five designated sections cover opaque, cutout-mipped, cutout, translucent and
animated content in each of initial, staged-reload and reopened-after-unload
phases: fifteen complete-frame pairs. Substitution remains bounded to one
resident section per pair, not a whole-world resident cache. Raw files
`composited-<phase>-<designated>.raw` carry a 32-byte little-endian header
(scene, designated section, width, height, substituted draws, substituted
vertices, version 2, expected resident vertices), followed by vanilla RGBA/depth,
resident RGBA/depth and section-omitted RGBA/depth. RGBA is four bytes per pixel;
depth is float32; rows are bottom-up.

The independent Python oracle requires all fifteen fresh artifacts, exactly one
substituted layer per single-layer designated scene, the complete resident vertex
total, finite depth and nonempty world depth coverage. The section-omitted arm
must differ visibly from vanilla, rejecting clipped/occluded vacuous comparisons.
It recounts exact RGBA and depth-coverage differences, plus depth differences
at the existing two-24-bit-depth-unit tolerance. Any mismatch fails acceptance;
`composited-summary.json` records each row's independently recounted result.
This step does not authorize physical performance or memory claims.

The static selected live acceptance passed after the generated-agent build;
default live acceptance also passed. An independent raw-readback recount found
zero RGBA, depth-coverage and tolerated-depth differences in all fifteen
1280×720 pairs. Every pair substituted exactly one draw with the complete
resident vertex count and had a positive omitted-section footprint
(7,508–44,915 pixels). These are software-display live checks, not physical
RTX 5090 trials or a performance claim. An earlier staged-reload translucent
capture was correctly refused for a zero visible footprint: the camera still
faced the preceding section. Capture now waits for the requested client yaw
and a subsequent rendered frame; no oracle threshold was changed.

Step two identified a limitation in that original depth result. Pinned 1.12.2
`buq.a(int,float,long)` bytecode clears `GL_DEPTH_BUFFER_BIT` immediately before
the hand pass when field `C` is true. Original creative-player readbacks therefore
compared post-hand depth, not world depth; the zero-depth-difference result above
must not be read as world-depth fidelity evidence. Composited replay now disables
that hand pass for every arm and restores its prior value in `finally`. Both
matrices have now passed again with genuine world depth, without changing depth
tolerance, RGBA equality, coverage equality or footprint checks.

## Slice 8, step two: moving-camera translucent fallback

Sections containing any translucent vertices are never substituted with the
compile-time resident VBO. Their complete section draw stays vanilla, retaining
`RenderChunk.resortTransparency` and vanilla's camera-dependent ordering.
Opaque, cutout and animated resident substitution remains selected-only;
ordinary drawing stays off by default. This is a fidelity-preserving fallback,
not evidence that a stale resident translucent mesh can be drawn correctly.

The harness moves a spectator camera west, north, through the glass interior,
east and south of the stained-glass/ice scene, aiming toward its center with a
30-degree downward pitch. A stone backing floor lies in the section below the
designated scene, so opaque world-depth coverage is mandatory without changing
the designated section's glass/ice contents. Every stop must still differ from
its floor-only omitted-section arm.
It waits for exact position/rotation acknowledgement and a subsequent rendered
frame. The five positions repeat in initial, staged-resource and reopened-world
phases. Each point replays the complete world at a fixed tick/camera for vanilla,
selected fallback and omitted-section arms. The offered resident stays retained,
but fallback frames read current vanilla VBOs even after a section rebuild;
no resident data is submitted. Immutable section coordinates reject RenderChunk
relocation. Nontranslucent substitution still requires the exact offered compile.

The raw header is now version 3. Translucent scenes require zero substituted
draws and vertices; the other designated scenes still require one complete
resident draw. Fifteen additional `moving-<phase>-<point>.raw` files append a
little-endian point index and five float64 camera values (XYZ, yaw, pitch) to
the existing three-arm frame payload. The independent oracle requires every
fresh point, the exact through-and-around path, finite cameras/depths, positive
omitted-section footprint and zero RGBA/coverage/tolerated-depth mismatches.
Neither fallback nor camera movement relaxes the pixel oracle.

Ravel camera-dependent translucent re-sorting remains unimplemented: these
moving rows prove the vanilla fallback, not a native translucent ordering claim.

Remaining acceptance work includes native translucent resort and native fluid
tessellation, then preflight followed by frozen physical trials before considering
a default cutover. No physical trial belongs to this moving-camera step.

The rebuilt selected live acceptance exited 0. An independent raw recount
(separate scalar `struct`/`memoryview` implementation, without importing the
production oracle) confirmed zero RGBA, depth-coverage and tolerated-world-depth
mismatches in all thirty 1280×720 rows. The static genuine-world-depth result
therefore held on rerun: twelve nontranslucent rows each submitted one complete
resident draw, while three translucent rows used vanilla fallback. Static
omitted-section footprints were 7,508–23,409 pixels, and every row contained
positive genuine world-depth coverage. All fifteen moving rows submitted zero
resident vertices. Their positive designated-section color-only footprints
against the floor-only omitted arm were:

| Lifecycle phase | West | North | Glass interior | East | South |
| --- | ---: | ---: | ---: | ---: | ---: |
| Initial | 82,390 | 107,096 | 775,496 | 82,209 | 82,209 |
| Staged resources | 82,146 | 107,083 | 775,496 | 82,209 | 82,209 |
| Reopened world | 82,390 | 107,096 | 775,496 | 82,209 | 82,209 |

These moving rows prove fidelity of the translucent vanilla fallback under
camera motion, not resident translucent ordering or whole-world native drawing.
Opaque/cutout resident substitution is proven by the twelve static rows; the
moving rows do not claim additional resident opaque/cutout substitutions.

## Slice 8, step three: whole-section fluid fallback

The boring option is to keep fluid sections entirely on vanilla. Minecraft
1.12.2's `BlockFluidRenderer` does not emit baked-model quads, so water and lava
cannot acquire source provenance by treating their final buffers as model input.
The selected compile capture checks every interior cell using the pinned vanilla
water/flowing-water/lava/flowing-lava block IDs (8–11), including enclosed fluids
that emit no visible vertices. A match refuses native baked publication and routes
the entire designated section, including its opaque basin, to vanilla.
This does not implement native fluids or claim their provenance.

Three additional static sections contain a still-water basin, a continuously fed
falling/flowing-water basin and a lava basin. They repeat across initial, staged
resource reload and reopened-world phases. Fluid frames use version 4 of the
three-arm format: version 3's unchanged pixel payload plus four little-endian
counts (fluid-ID mask, water sources, flowing surface cells, lava cells). Every
fluid row requires matching scene-specific fluid evidence, zero substituted draws
and zero resident vertices, positive world-depth coverage, a visible omitted-section
footprint and the same exact RGBA/coverage and two-24-bit-depth-unit checks.
The complete matrix now requires 24 static and 15 moving frames; missing fluid
rows, native fluid-section draws and fluid pixel/depth mismatches fail acceptance.
The flowing scene waits for non-falling, non-source water levels before retaining
a frame; fluid offers also wait 250 ms without a replacing compile. One-block
basin rims and a raised, supported camera expose the surfaces. Superseding fluid
compiles replace pending fluid offers for the same section. The first replay waits
for the exact offered compiled chunk upload; later fallback frames use current
vanilla buffers.

The lane gate, legacy agent build and selected `geometry-proof` live acceptance
all exited 0. The 31 harness unit tests include fluid-row refusal mutations.
An independent scalar raw recount, importing no acceptance implementation,
checked all 39 frames at 1280×720: zero RGBA, coverage or depth mismatches under
the unchanged strict oracle. Its nine fluid rows had zero substituted draws and
vertices and 12,222–25,458 omitted-section footprint pixels. All phases contained
16 still-water source cells, one source plus 15 flowing surface cells in the
flowing scene, and 16 lava cells. These counts prove the fixture states and
whole-section routing, not native fluid tessellation or steady-state simulation.

Native translucent resort and fluid tessellation remain before the preflighted
frozen RTX 5090 trials and any default cutover. No physical trials run in this
step, and drawing stays off by default.
