# WP5 slices 6–7: bounded shaded baked comparisons and residency

## Status and boundary

Vanilla remains authoritative for world drawing. The off-by-default
`geometry-proof` selection now submits independently emitted baked BLOCK vertices
to a bounded Java-owned offscreen GL comparison, not a replacement world renderer.
It uses the live atlas and lightmap, model positions, UVs, shaded color, combined
light, alpha testing, translucent blending and independently sorted translucent
quad order. Full-world visual fidelity and replacement-renderer savings are not
established by matching admitted section layers.

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

Slice 7 is not yet acceptance-complete. The implementation and lifecycle tests
do not replace the required fresh, frozen-candidate three off/three on RTX 5090
trial matrix and separate Xvfb functional run. All physical/golden tables above
belong to slice 6; they are not slice-7 performance or golden evidence.

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

The selected live run recorded 46 native acknowledgements and 46 GL replacements.
Native combined retained capacity peaked at 740,040 bytes; fixed GL reservation
peaked at 2,752,512 bytes across three buffers per pool. Three pools retired,
and both native and GL live residency drained to zero. There were 10,626 repeated
resident submissions and zero recorded stale GL refusals. These are run-wide
functional counters, not per-trial physical memory measurements.

Workspace verification passed 4,383 tests with none skipped; focused generator,
renderer-validator and benchmark-validator suites passed 321, 9 and 14 tests.
Coverage was 93.10% regions, 92.54% functions and 93.85% lines. Workspace clippy
with all targets/features and warnings denied passed. Offline verification still
reports seven unavailable clean-room source trees out of eighteen; acquired-byte
checks requiring them were not exercised.
