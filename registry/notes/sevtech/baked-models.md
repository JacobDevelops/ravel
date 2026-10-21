# WP5 slice 6: bounded shaded baked comparisons

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
negative tests. It validates the exported evidence schema; it does not independently
recompute the cumulative baked pixel counts from raw readbacks.

## Golden-scene matrix retained, not narrowed

The selected Xvfb run measured the following admitted section-layer comparisons
against final vanilla buffers and RGBA/depth readbacks. A scene with no samples is
**not measured**, not a zero-mismatch success. Counts below are cumulative over
nonempty 64-by-64 offscreen comparisons, not controlled full-world screenshots.

| Required scene | Samples | RGBA / coverage / depth mismatches | Disposition and exact limit |
| --- | --- | --- | --- |
| Opaque textured models | 39 | 0 / 0 / 0 | Unsupported as full-world golden: section-layer comparison only; 30,000 foreground pixels per arm |
| Cutout models, mipped | 16 | 0 / 0 / 0 | Unsupported as full-world golden: section-layer comparison only; 7,376 foreground pixels per arm |
| Cutout models, unmipped | 18 | 0 / 0 / 0 | Unsupported as full-world golden: section-layer comparison only; 6,143 foreground pixels per arm |
| Translucent models/fluids | 0 | Not measured | No admitted complete baked section; fluid tessellation is not baked-quad provenance |
| Animated atlas sprites | 0 | Not measured | No admitted complete baked section with animated provenance |
| Staged resource reload | 0 | Not measured | No admitted shaded section after the first compared resource generation |
| Staged world unload/reopen | 0 | Not measured | Retired-generation drawing refused; no post-unload pixel comparison |

That functional run admitted 290 CPU comparisons and 16,939 quads, with zero
mismatched BLOCK words and first mismatch -1; 190 busy, 267 unsupported and one
stale section remained explicit. Independent resource/atlas generations were both
2. It drained 39 GL comparisons, with zero GL stale/unsupported outcomes, uploading
3,794,336 bytes with a 103,712-byte peak transient VBO. CPU upload validation took
463,364 ns cumulatively; complete GL comparisons took 26,016,262 ns. These numbers
describe the exercised run, not an admission-rate guarantee.

## Frozen physical trials and verification

Six completed warm-filesystem trials used one frozen candidate with SHA256
`4eb84d5fe498a3de8be4e74df38b97d7f8c36a94093f7ac28ea874d340279e40`:
three selection-off (`vanilla`) and three selection-on (`geometry-proof`).
The locked RTX 5090 display was `:0`; seed was 8675309, dimensions 1280-by-720,
render distance 12, unlimited frames, G1 and 4,096/8,192 MiB initial/maximum heap.
Temurin 27+35 and the null audio driver were identical across arms. These are
Ravel-versus-Ravel observations, not a vanilla speedup experiment. Private trial
names are `wp5s6b-off-warm-filesystem-1` through `-3` and the corresponding `on`
names; separate strict reductions retain every scene's compile/upload timing,
frame stalls and memory samples without pooling frame distributions.

Each frame cell below is the individual trial's p50/p95/p99 in milliseconds.
Memory columns are peak RSS, sampled used heap and whole-device VRAM in MiB;
VRAM is not per-process attribution.

| Trial | Spawn idle | Square exploration | Loaded chunk stress | RSS | Heap | VRAM |
| --- | --- | --- | --- | --- | --- | --- |
| Off 1 | 0.502/0.548/0.887 | 0.732/0.925/1.236 | 0.790/0.913/1.235 | 5409.6 | 2684.3 | 1435 |
| Off 2 | 0.646/0.868/1.291 | 0.702/0.930/1.242 | 0.743/0.935/1.253 | 5478.7 | 2711.8 | 1443 |
| Off 3 | 0.511/0.559/0.902 | 0.741/0.945/1.241 | 0.796/0.943/1.247 | 5454.7 | 2708.2 | 1439 |
| On 1 | 0.593/0.711/0.997 | 0.735/0.937/1.262 | 0.790/0.874/1.246 | 5550.0 | 2736.4 | 1437 |
| On 2 | 0.577/0.696/0.998 | 0.737/0.940/1.286 | 0.790/0.921/1.290 | 5582.0 | 2740.8 | 1435 |
| On 3 | 0.516/0.561/0.898 | 0.743/0.964/1.280 | 0.798/0.932/1.254 | 5511.7 | 2691.7 | 1437 |

Loaded-stress vanilla compile/upload hook durations below are observational CPU
durations, not GPU execution time. Each timing cell is p50/p95/p99 in milliseconds.
The geometry counters refer to the earlier geometric comparison, not shaded baked
residency. Its final resident/staging bytes and live buffer/framebuffer counts were
zero in every trial. Dormant selection had zero copies/uploads and zero baked
native comparisons in all three trials, in addition to the live hook-counter
check; no isolated nanosecond cost is inferred from whole-application differences.

| Trial | Compile | Upload | Geometry uploads / bytes | Peak geometric resident bytes |
| --- | --- | --- | --- | --- |
| Off 1 | 0.0063/14.6126/35.3492 | 0.0015/0.0047/0.0103 | 0 / 0 | 0 |
| Off 2 | 0.0680/19.1526/41.0130 | 0.0016/0.0043/0.0072 | 0 / 0 | 0 |
| Off 3 | 0.0360/15.0665/38.2957 | 0.0015/0.0047/0.0100 | 0 / 0 | 0 |
| On 1 | 0.2478/14.5757/37.1494 | 0.0016/0.0047/0.0106 | 384 / 6558768 | 68832 |
| On 2 | 0.0714/15.1110/36.9885 | 0.0015/0.0046/0.0104 | 401 / 6811296 | 68832 |
| On 3 | 0.2060/14.7226/35.6131 | 0.0015/0.0046/0.0105 | 383 / 6568560 | 68832 |

Crucially, **none of the three physical selected trials admitted a complete baked
CPU comparison or shaded GL upload**. Their busy/unsupported/stale section counters
were respectively 22927/4050/127, 22978/3735/92 and 23155/4101/106. Baked emitted
quads, uploaded bytes and peak baked VBO bytes were zero because no baked result
was admitted, not because shaded rendering is free. These trials measure selected
instrumentation/refusal overhead and geometric work, not shaded throughput or
fidelity. The Xvfb functional run above is the exercised shaded GL evidence.
The physical admission gap remains an explicit slice-7 prerequisite.

Final gates exited zero: all-target/all-feature Clippy with warnings denied,
legacy agent build, default live, selected live and the complete verification
task after a fresh coverage-target removal. Each of Minecraft 1.12.2, 1.21.1 and
26.1.2 passed 4,360 workspace tests plus its empty-composition test; all three JVM
compile/harness rows and version gates passed. Coverage was 94.04% lines, 93.35%
regions and 92.64% functions. Independent read-only review had no remaining
findings after the hook, lifecycle, GL refusal and evidence-validation fixes.

## Next: slice 7

Keep selection off and vanilla authoritative until controlled full-world scenes
cover opaque, both cutout modes, translucent/fluids, animated sprites, resource
replacement and unload/reopen. Preserve exact refusal reasons rather than treating
empty scenes as matches. Independent raw baked readback reduction remains distinct
from schema validation. Establish complete shaded residency/resource replacement,
then evaluate default world cutover only against faithful golden scenes and fresh
like-for-like measurements. The current bounded comparison is not authorization
to remove vanilla drawing or claim a speedup.
