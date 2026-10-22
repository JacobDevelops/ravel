# WP5 slices 6–8: bounded shaded baked comparisons and residency

## Status and boundary

Vanilla remains the default world renderer. Earlier slices 5–8 selection-on trials
measured **proof overhead**, not Ravel world drawing: `geometry-proof` submits to
the bounded offscreen comparison, and slice 8 substitutes one section only inside
explicit world-frame replay. Those timings are not renderer-replacement results.

The separate experimental `resident-world` selection intercepts vanilla's actual
per-section VBO draw during normal SOLID, CUTOUT_MIPPED, CUTOUT and TRANSLUCENT
rendering. It reuses vanilla's layer state, translation and section ordering,
including back-to-front translucent section order and vanilla blend state.
Every compiled section can publish to a generation-keyed native table and a Java-owned GL
table. Each table admits at most 4,096 resident sections; native pending entries
are separately keyed and capped at 64. Their render-distance byte budgets are
`clamp((2d + 1)^2 * 262144, 16 MiB, 256 MiB)`. Native accounting includes retained
mesh capacity and table storage; GL accounting includes the three comparison
scratch buffers. Both tables evict the section farthest from the newly admitted
section, preserving vanilla fallback when a lease is no longer resident. These
are residency limits, not a claim about total JVM heap, process memory or physical VRAM.
Fluids, stale generations and dirty or pending meshes remain vanilla.
Exact generation/key acknowledgement and current compiled-task,
section-origin, resource and atlas identities are required before replacement.
Reload, world unload and shutdown retire native entries and GL buffers.
The selection remains refused outside diagnostic opt-in
`RAVEL_LEGACY_PRODUCTION_VALIDATION=1`; vanilla remains the default.
`productionWorldDraw` separately counts per-layer replacement and fallback draws,
presented frames, frames with replacements, maximum replacements/frame and actual
acknowledged-then-stale/pending fallback draws. Normal-world raw triplets remain
separate from designated-section replay evidence, which cannot enter the normal
draw replacement path. Sidecars report per-frame replaced and fallback draws,
native/GL live and peak bytes and their configured budgets. Translucent native
sort, Java upload and refusal counters are recorded both for the designated
resident key and globally in each normal-world sidecar.
Vanilla's actual transparency-sort callback invokes the existing native Rust
sort against the production section key; its matching vanilla VBO upload
updates only that resident VBO's translucent range. Pending handoffs are bounded
to 64 byte-buffer/destination identities and 16 MiB of reusable output arrays.
Missing native residency, a refused handoff or failed GL identity lookup removes
that section's production admission instead of drawing stale translucent data.
Publication requires zero isolated-layer RGBA, coverage and depth violations
under the existing strict oracle for all four baked layers, not just the three
opaque/cutout layers.

The production translucent software-GL run captured four translated camera
positions plus five within-section moving stops and one section-crossing stop
in each of three lifecycle phases. Static A/B/C/D were respectively
`(-167.5, 200, 8.5)`, `(-166.5, 200, 7.5)`, `(-165.5, 200, 6.5)` and
`(-163.5, 200, 4.5)`, with yaw `-135/-90/-45/0` and pitch zero.
Moving M0–M5 were respectively `(-174,198,-8)`, `(-168,198,-13)`,
`(-169,198,-8)`, `(-162,198,-8)`, `(-168,198,-2)` and
`(-160,198,-8)`, yaw `-90/0/-90/90/180/90`, pitch 30.
M0–M4 retain one generation/key per phase and advance its native sort and upload
counters at every stop. The last applied matching upload also records vanilla's
sort camera; each cadence row requires its X/Z to equal that stop's camera,
including M0, so an earlier priming resort cannot satisfy a later stop.
All six moving rows require an actual TRANSLUCENT replacement of the designated
resident VBO, not merely another visible translucent section. M5 crosses the x
section boundary, causes a new acknowledged key and retains monotonic global counters.
These controlled scenes use ordinary world-layer submission, not the offscreen
or designated-section replay.

Each 1280×720 row below contains vanilla, replacement and omission arms captured
within its one recorded client tick. A separate raw-byte recount, independent
of the acceptance parser, found zero RGBA and coverage mismatches and zero depth
violations at the unchanged `2 / 16777215` tolerance in all thirty rows.
The omission footprint is positive in every row. Counts are section-layer draw
calls, not unique sections: S/M/C/T means replaced
SOLID/CUTOUT_MIPPED/CUTOUT/TRANSLUCENT; fallback columns count vanilla draws.

| Phase | Pose | Tick | Replaced S/M/C/T | Fallback opaque/T | Key sort/upload | Global sort/upload/refusal | Omission footprint |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Initial | A | 757 | 3/1/0/1 | 0/0 | 0/0 | 7/7/0 | 279039 |
| Initial | B | 774 | 2/1/1/1 | 0/0 | 0/0 | 8/8/0 | 174128 |
| Initial | C | 791 | 2/1/1/0 | 0/0 | 0/0 | 8/8/0 | 97110 |
| Initial | D | 808 | 2/1/1/0 | 2/1 | 0/0 | 8/8/0 | 188658 |
| Initial | M0 | 837 | 3/0/0/1 | 3/1 | 1/1 | 10/10/0 | 448099 |
| Initial | M1 | 856 | 4/0/0/1 | 5/2 | 2/2 | 11/11/0 | 516171 |
| Initial | M2 | 875 | 3/0/0/1 | 2/0 | 3/3 | 12/12/0 | 888210 |
| Initial | M3 | 894 | 3/0/0/1 | 3/2 | 4/4 | 13/13/0 | 443976 |
| Initial | M4 | 913 | 3/0/0/1 | 1/1 | 5/5 | 14/14/0 | 443976 |
| Initial | M5 crossing | 932 | 3/0/0/1 | 4/2 | 0/0 | 15/15/0 | 388029 |
| Reload | A | 1354 | 3/1/0/1 | 0/0 | 0/0 | 24/24/0 | 279039 |
| Reload | B | 1372 | 2/1/1/1 | 0/0 | 0/0 | 25/25/0 | 174128 |
| Reload | C | 1389 | 2/1/1/0 | 0/0 | 0/0 | 25/25/0 | 97110 |
| Reload | D | 1407 | 2/1/1/0 | 2/1 | 0/0 | 25/25/0 | 188658 |
| Reload | M0 | 1434 | 3/0/0/1 | 3/1 | 1/1 | 27/27/0 | 448081 |
| Reload | M1 | 1453 | 4/0/0/1 | 5/2 | 2/2 | 28/28/0 | 516171 |
| Reload | M2 | 1472 | 3/0/0/1 | 2/0 | 3/3 | 29/29/0 | 888210 |
| Reload | M3 | 1490 | 3/0/0/1 | 3/2 | 4/4 | 30/30/0 | 443976 |
| Reload | M4 | 1510 | 3/0/0/1 | 1/1 | 5/5 | 31/31/0 | 443976 |
| Reload | M5 crossing | 1527 | 3/0/0/1 | 4/2 | 0/0 | 32/32/0 | 388029 |
| Reopened | A | 2479 | 3/1/0/1 | 0/0 | 0/0 | 39/39/0 | 280076 |
| Reopened | B | 2496 | 2/1/1/1 | 0/0 | 0/0 | 40/40/0 | 174130 |
| Reopened | C | 2513 | 2/1/1/0 | 0/0 | 0/0 | 40/40/0 | 97106 |
| Reopened | D | 2530 | 2/1/1/0 | 2/1 | 0/0 | 40/40/0 | 188663 |
| Reopened | M0 | 2555 | 3/0/0/1 | 3/1 | 1/1 | 42/42/0 | 448097 |
| Reopened | M1 | 2572 | 4/0/0/1 | 5/2 | 2/2 | 43/43/0 | 516171 |
| Reopened | M2 | 2590 | 3/0/0/1 | 2/0 | 3/3 | 44/44/0 | 888210 |
| Reopened | M3 | 2609 | 3/0/0/1 | 3/2 | 4/4 | 45/45/0 | 443976 |
| Reopened | M4 | 2628 | 3/0/0/1 | 1/1 | 5/5 | 46/46/0 | 443976 |
| Reopened | M5 crossing | 2646 | 3/0/0/1 | 4/2 | 0/0 | 47/47/0 | 388029 |

Key counters refer to the designated resident, not every visible translucent
section. All recorded key refusal counts were zero. The moving cadence keys
were `5/130`, `7/432` and `10/1087`; the crossing rows acknowledged fresh keys
`5/212`, `7/512` and `10/1163`. Their zero counters reflect initial mesh
publication, not a missing required resort of the previous mesh.
This reset matches vanilla's lifetime: the pinned 1.12.2 `RenderChunk` rebuild
creates a fresh `CompiledChunk`, emits geometry and records its initial sorted
vertex state. A resort instead restores the existing vertex state, stably sorts
its current quad order and stores the updated state. The native mesh likewise
physically retains each sorted order and uses that current ordinal to resolve
equal-distance ties; only a rebuild replaces the mesh with fresh emission.
Counter reset does not discard tie order vanilla would retain on a resort.

At render distance two, each independent table's budget was 16,777,216 bytes.
Recorded native peak was **3,679,324 bytes** and GL peak **4,785,200 bytes**,
including comparison scratch storage. These are residency peaks, not total
process or physical GPU memory measurements. Actual per-layer totals were
11,259/5,802/6,472/1,787 replacements and 24,285/10,772/8,539/15,109 fallbacks.

Live negative probes began with exact acknowledged resident identities:
generation/key `5/39` was made dirty, and `5/111` entered a new compile task.
The actual stale-fallback counter increased `4 → 5`; the actual pending-fallback
counter increased `2237 → 2238`. Both observed vanilla fallback without removing
the corresponding vanilla draw. Selection remains diagnostic-only.

The earlier single-resident experiment's first independent recount submitted
**one section/frame** in twelve 1280×720 samples, with three strict RGBA failures.
Coverage and depth were exact. Initial failure counts:

| Phase | Opaque RGBA mismatches | Cutout-mipped | Cutout | Animated |
| --- | ---: | ---: | ---: | ---: |
| Initial | 0 | 0 | 0 | 0 |
| Resource reload | 0 | 0 | 15006 | 17043 |
| Reopened | 0 | 0 | 14516 | 0 |

All failed pixels lie on the animated magma wall. In the two cutout-designated
failures, every mismatching pixel equals the omitted arm: the changed wall is
outside the replaced cutout section. Captures now require all three ordinary
frames inside one counted client tick, beginning just after that tick; drift,
camera/upload/FBO changes poison the triplet and retry without consuming it.
The independent tick-stable recount has **zero RGBA, coverage and depth
mismatches in all twelve frames**, each with one resident section and positive
omitted-section footprint. This supports animation drift as the original cause;
atlas phase and GL state were not independently sampled. No oracle changed.
Bounded multi-section residency extends that mechanism; physical trials remain
separate work. Full-world parity is not established.

The earlier offscreen/replay proof uses the live atlas and lightmap, model
positions, UVs, shaded color, combined light, alpha testing and translucent
blending. The production path reuses slice 8's native translucent re-sort with
generation/key ownership rather than its single replay slot. Whole fluid
sections remain on vanilla.
Native fluid tessellation and replacement-renderer savings remain unsupported.

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
| Translucent order | Descending float squared centroid distance; stable current-order ties on every cancellable in-place resort |
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

Selected capture uses pooled owning compile-thread captures, each limited to
8,192 source quads and fixed source/oracle/output buffers. Their render-distance
budget permits four captures at distance two and at most 64; uploads drain at most
two captures per frame. Per-quad observation is thread-confined; retirement
invalidates its epoch without mutating another thread's active buffers. While
designated comparison scenes are incomplete, that proof window remains serial;
afterward the existing compile seam captures every section within the bounds.
Busy, oversized, uncolored, incomplete-provenance, unaccounted final vertices
(including fluid tessellation), stale and over-budget inputs are refused, never
truncated success. Exact pending acknowledgements can arrive out of order without
settling another section. Native and GL entries retire immediately; an invalidated
active capture releases its game references on its owning thread's finish hook.
GL context ownership and caller GL state remain Java responsibilities;
world-resident uploads reuse existing bounded staging storage.

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

Slice 7 introduced one acknowledged native section and at most one pending
replacement, superseded by the bounded native table described above. Each owned
its source bytes and emitted words. Its identity
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

Step four below addresses native translucent resort. Fluids deliberately remain
on vanilla for the preflighted frozen RTX 5090 trials and any default cutover.
No physical trials ran in step three, and drawing stays off by default.

## Slice 8, step four: resident translucent re-sort

Rust retains the independently shaded mesh behind the acknowledged renderer
generation and lease key instead of retaining duplicate source/emitted arrays.
Each re-sort requires the exact pending or acknowledged renderer generation and
lease key, and rejects stale, retired or non-finite requests. It reorders
the resident translucent quad vector in place without allocating; solid and
cutout vectors are unchanged. Quad centres and squared distances use vanilla's
float operation order. Ties preserve the input order of that particular sort,
including the order left by a preceding camera position, rather than resetting
to the original capture ordinal.

The adapter observes the pinned `BufferBuilder.sortQuads` call itself: vanilla
still owns camera thresholds, task scheduling and cancellation. Builder translation
is combined with the camera in double and rounded to float, as in vanilla.
Native ordering changes at the actual sort call, even if a later task is canceled.
Bounded, lazily populated Java staging slots associate the resulting words with
their exact builder buffer; each holds only its actual translucent words and a
four-layer count header. Only the corresponding translucent VBO upload applies
that snapshot to the Java-owned resident GL buffer. Publication registers the
pending lease under the same lock as sorting; uploads that precede acknowledgement
are retained and applied as owner-thread acknowledgement succeeds. The 205-slot ceiling
covers vanilla's at-most-204 pooled builders plus its local worker: its pool-size
calculation saturates a double-to-int conversion before dividing by 10 MiB.
Canceled tasks can retain a slot, but reuse of the same builder replaces it rather
than consuming an unbounded history. A missing acknowledged upload snapshot or
exhausted staging bound invalidates selected resident drawing instead of crashing
vanilla or accepting a stale order. A refused pending lease cannot subsequently
be acknowledged. The strict proof requires positive native sort and matching
upload counts and zero translucent refusals. Reload/retirement invalidates the
generation/key and clears pending staging. This does not create a parallel camera
polling cadence or let Rust own GL. Selected non-fluid translucent sections now
use resident compositing; whole sections containing fluids remain on vanilla,
and resident drawing remains off by default.

Pending proof replay does not freeze vanilla rebuilds. A rebuild of the exact
pending chunk replaces its pending comparison and lease; unrelated chunks cannot
replace the designated scene. Replay still requires the offered compiled chunk
to be current, the native lease to be acknowledged, and a positive resident draw,
so camera motion cannot silently continue with stale geometry or vanilla draws.

The selected Xvfb acceptance exited 0 with exact parity on the existing strict
oracle. An independent scalar recount, without the production proof parser,
checked all 24 static and 15 moving raw frames across initial, resource-reload
and reopened phases. Every frame had zero RGBA, coverage and genuine-world-depth
mismatches, with the unchanged two-24-bit-depth-step tolerance and a positive
omitted-section footprint. All 30 non-fluid frames substituted resident draws;
this includes all three static translucent rows and all 15 moving-camera stops,
each submitting 512 resident vertices. All nine fluid rows retained zero resident
draws and vertices. The selected run recorded 32 native translucent sorts,
35 matched uploads (including initial publication) and zero translucent refusals.
The first selected attempt timed out at initial moving stop 1; no mismatch
comparison was reported. Same-section rebuild recapture was then admitted, and
the completed run passed without changing the camera path, oracle or draw
requirement. No moving stop or static row differed in the completed run.

That selected software-GL replay alone does not prove physical-GPU parity,
default world cutover or renderer savings. Normal-world four-layer parity,
including production translucent replacement and bounded visible-section
residency, is reported separately above. The frozen RTX 5090 attempt and its
coverage/budget findings are recorded below; a complete accepted matrix remains.
No default cutover is admitted.

## Frozen RTX 5090 production-path comparison

The round-25 bench selects `resident-world` for on arms with acceptance **off**.
It records the required `ravel.renderer.validation=true` selection override, never
`ravel.legacy.acceptance=true`, in timed-arm provenance. Off arms select vanilla
drawing in the same Ravel runtime cohort; each trial starts a separate JVM.
The default remains vanilla. Scene settings are the existing slice-1 protocol:
seed 8675309, distance 12, uncapped frames, silent audio and the physical display.

The initial frozen functional preflight failed for a benchmark configuration
reason, not a renderer atlas-generation failure. Candidate preparation had applied
timed distance-12 options to the shared distance-2 golden scene. Diagnostic captures
had a valid matched triplet, stable uploads/sorts and replacements `[3,1,0,1]`,
matching the passing software-GL scene, but also 124 opaque and 16 translucent
fallbacks. The controlled-scene majority guard correctly rejected that capture.
The direct physical distance-2 harness passed. Candidate preflight now applies
and records all shared harness client options; the timed trials remain distance
12. No capture guard, production renderer or default was changed for this fix.
The clean frozen candidate subsequently passed physical preflight (exit 0).

### Frozen inputs

Candidate `baseline/wp5s8-production-r25c.json`, measured against the integrated
`lsonvklw` runtime with only the bench changes, has SHA-256
`ab0e1d8b65a560a3c00a9144acd800f039483df24157740dfb118ddd23f82591`.
Its component and benchmark SHA-256 values are:

| Input | SHA-256 |
| --- | --- |
| Ravel premain agent | `a5b544aff42f967590e7b761bfdca712d56df1e6de644342c10ee660bc23921b` |
| Native composed host, no fixture exports | `c34dd60e8f7f3e777d8453f5cb1949f877b23fe906c97c15ed6b06227633a18a` |
| Legacy platform | `73cbc243bb1a7dd2a5bb8d8c1445ba1246661ae15a14fe651c9ba6acf0b111f7` |
| Ravel screen | `ba1940fcfb4b0c57eb5bfb170bdefbbfebdad67b71a7bcb0ffd084ed937827be` |
| LWJGL 3.3.6 | `b00e2781b74cc829db9d39fb68746b25bb7b94ce61d46293457dbccddabd999c` |
| LWJGL Linux natives | `2f0e65d6985d602c0e5e5aab6576db113a9a378a36f8144ec93b77a4fde5876c` |
| LWJGL GLFW | `b29c938ecc4997ce256a831aeca2033dab9829b5c21e72ebeb64aecd9e08450c` |
| LWJGL GLFW Linux natives | `a1b60014597bc0e45bf39089f4d838c3aa87fd668f6fe4e7326aa314d2ec87c0` |
| LWJGL OpenGL | `bb0430e0a5fd7b7b5770ae517960b2ea9887bb08d04f3a0cb1aae145332e1310` |
| LWJGL OpenGL Linux natives | `8c0b5c081a7872a3cdb02b6448921da5ae5c23ab49656f299edc7a09b7e99b74` |
| LWJGL OpenAL | `5d8669ee52325080f500cf0060785a13135a10c602286cefc8ba285805fa8468` |
| LWJGL OpenAL Linux natives | `d30c407c1182f507c70ad12e21bac98189c767bf5c7a592dbfaa4f11d139545c` |
| Benchmark observer JAR | `071e9237f90692451cd5e5971fac4f24659863a379d4892fb4f3194529bddcb4` |
| Temurin 27.0.0+35 Java launcher | `306db8c7d74a1b2b8aadc0b82a52adfa564c4855d746a36b70c5304c45d23af6` |
| `bench/BenchmarkAgent.java` | `f618bb298a0820a61fe0ebd5393aa8f93cd3d7806b5b87f42895d537c2abb256` |
| `bench/run.py` | `629736e940e7abf58a3fb42bad595cd19336ae69efd19de96abc08e10a2547a6` |
| `bench/preflight.py` | `3a8ba4e86e804f31f1f9862a74b5dee084f38564de27afc0b77fa5e8bd3ea30c` |
| `bench/vanilla.py` | `cee2f2137d77b87d6fdec9aa8d2072b8801b0a49a37b8bbbfd19ed95cb4e5fcd` |
| `bench/launch.py` | `5c9b77e4ba3abec024b0ab1ffaa25f430f78acd863c5a99cd024b5d7b4e1ba71` |
| `bench/prepare.py` | `894d5bb0530d1bd2e94d0d334c8ffac830c7f12ce065efde906f6e34679633c6` |
| `bench/summarize.py` | `1ad5a2dad1b236698af784b9a5ff99c385292879489d05c7d92214d063b82100` |
| Shared functional harness settings | `e1652ce1e97a110173d53e3060207d1b3e2f5405d183938635a9e0d59e69f1d1` |

Bench source paths in this table are relative to `registry/sevtech/`. The bench
exports per-layer replacement and fallback draw counts, global and currently
owned section sort/upload counts, and native/GL live usage, peaks and budgets.
Selected arms with zero replacements in any non-fluid layer fail. The exact
pending and stale fallback counts are separated from the remainder, labelled
**not resident: fluid, unadmitted, evicted or over budget**. Admission refusals
and evictions are section events, not draw-attribution counters.

### Retained physical trials: stopped after the first pair

The locked, alternating matrix `wp5s8-trials-r25c` passed its frozen physical
functional preflight, completed the vanilla arm, then correctly failed its first
resident-world arm: TRANSLUCENT replacements and native sort/upload activity
were zero. The remaining four timed arms were **not run**, and the workload
was not changed to manufacture coverage. The separate frozen Xvfb functional
preflight `wp5s8-production-r25c-xvfb-functional` passed, exit 0.

These are the retained first-pair observations, not an accepted performance
comparison. Trial names below share prefix `wp5s8-trials-r25c-warm-filesystem-1-`.
Quantiles are frame milliseconds, rounded to three decimals:

| Trial suffix | Scene | p50 | p95 | p99 |
| --- | --- | ---: | ---: | ---: |
| `vanilla` | spawn_idle | 0.621 | 0.863 | 1.193 |
| `vanilla` | square_exploration | 0.783 | 1.131 | 1.475 |
| `vanilla` | loaded_chunk_stress | 0.824 | 1.107 | 1.471 |
| `resident-world` — invalid coverage | spawn_idle | 0.906 | 1.291 | 1.597 |
| `resident-world` — invalid coverage | square_exploration | 2.151 | 2.955 | 4.895 |
| `resident-world` — invalid coverage | loaded_chunk_stress | 2.521 | 3.217 | 4.467 |

| Trial suffix | Peak RSS MiB | Peak used heap MiB | Peak device VRAM MiB |
| --- | ---: | ---: | ---: |
| `vanilla` | 5433.695 | 2781.065 | 2329 |
| `resident-world` — invalid coverage | 6082.098 | 2849.795 | 2518 |

VRAM is sampled for the entire device, including the desktop and other allocations;
it is not a per-process or resident-VBO measurement. Resource sampling ends at
the measured shutdown marker, before the observer's final owned-section scan.

All vanilla production replacement/fallback and global/owned sort/upload counters
were zero. Its resident native/GL live and peak byte counts were zero against
their unused 16,777,216-byte initial budgets. The selected arm exported:

| Layer | Replacement draws | Vanilla fallback draws | Replaced / (replaced + fallback) |
| --- | ---: | ---: | ---: |
| SOLID | 30,618,519 | 32,561,652 | 48.4622% |
| CUTOUT_MIPPED | 29,117,480 | 23,294,482 | 55.5550% |
| CUTOUT | 18,882,975 | 16,728,246 | 53.0254% |
| TRANSLUCENT | 0 | 19,158,147 | 0% |

Of the selected arm's 91,742,527 fallback draws, 8,564,007 were stale and
2,536,060 pending. The remaining 80,642,460 are **not resident: fluid,
unadmitted, evicted or over budget**; existing telemetry does not split those
causes per draw. Global sort/upload/refusal counts were `[0,0,0]`; the corresponding
currently owned section counts were also `[0,0,0]`. Owned counts describe sections
still retained since their latest compile, not lifetime cumulative work.

| Selected residency | Live accounted bytes | Peak accounted bytes | Budget bytes | Resident sections | Evicted sections |
| --- | ---: | ---: | ---: | ---: | ---: |
| Native | 163,280,008 | 163,839,744 | 163,840,000 | 1,249 | 1,063 |
| GL | 163,817,024 | 163,839,984 | 163,840,000 | 1,705 | 340 |

These eviction columns count section events, not fallback draws. Native pending
sections were zero at export. Baked capture separately recorded 21,642 successful
publication events, 532 begin/admission refusals, 4,042 finish refusals and 828
stale/resource skips. Every finish refusal used `UNACCOUNTED_VERTICES`; fluid
capture shares that code with other accounting mismatches, so this is **not**
a measured fluid-only refusal count.

About half the opaque-layer draw calls and all translucent draw calls remained
vanilla; this is not an all-resident world comparison. The retained selected
run's frame quantiles are higher, but **no faster/slower/within-noise verdict is
available**: the on arm is invalid and only one off arm exists, so off-versus-off
spread cannot be measured. No speedup, regression significance or default cutover
is claimed.

### What the retained world and allocation accounting establish

An offline census of the retained `RavelBenchmark` save decoded all four region
files: 1,714 chunks and 8,790 stored sections. There were zero glass, glass panes,
ice, frosted ice, stained glass, stained glass panes, slime, portal, beacon,
packed-ice, end-portal or end-gateway blocks. The saved view therefore supplies
no non-fluid translucent section to replace; this is a workload coverage gap,
not evidence that resident-world refused a present non-fluid translucent model.
The camera route spanned x = -172.5 to 83.5, z = 7.5 to 263.5 at y = 140.
There were 2,642 fluid-bearing stored sections, with 271 flowing-water,
416,811 water, 6,267 flowing-lava and 153,023 lava blocks. Those blocks remain
on the vanilla path by design. The original workload was left unchanged.

The residency budget is genuinely saturated, but **resident sections do not
reserve the 8,192-quad worst case**. Native `BakedSnapshot::mesh` reserves exactly
the observed quad count per layer; `BakedMesh::heap_bytes` accounts the resulting
vector capacities and quad provenance. GL `BakedWorldTable.publish` reserves
exactly `112 × emitted quads` bytes per section, plus a single shared 2,752,512-byte
scratch allowance. The observed GL snapshot therefore contains 161,064,512 actual
resident VBO bytes across 1,705 sections:

| Size measurement | Mean bytes/section | p95 | Maximum |
| --- | ---: | --- | --- |
| Actual GL mesh payload | 94,465.99 | Unmeasured | Unmeasured |
| Reserved resident GL VBO | 94,465.99 | Unmeasured | Unmeasured |
| Native mesh payload/capacity | Unmeasured | Unmeasured | Unmeasured |
| Native total accounted bytes, including shared table capacity, divided by resident count | 130,728.59 | Not a per-section distribution | Not a per-section distribution |

The retained exports contain aggregate bytes/counts, not a per-section census;
p95/max cannot be recovered from them. Native total accounting is not the native
mesh-only mean. The worst-case 917,504-byte GL payload is a theoretical input
bound, **not** an observed maximum or per-section reservation.

Vanilla's own VBO bytes for the same scene are **unmeasured**. Production keeps
Ravel resident VBOs in addition to vanilla's source VBOs; the device currently
holds both copies. On the CPU, native acknowledgement moves the full `Retained`
mesh from pending to resident without dropping opaque quad vectors. All four
layers, including their provenance, remain until replacement, eviction or
retirement, although subsequent native sorting uses only TRANSLUCENT.

Before rerunning timed trials, add a per-section census of logical mesh bytes,
native capacities, layer composition, Ravel VBO bytes and vanilla source-VBO bytes
for matched generation/compiled identities. Then evaluate dropping acknowledged
opaque CPU payloads while retaining valid lease metadata, and coordinate native
and GL eviction/lifetimes so one table does not keep unusable duplicates. Decide
whether a larger budget is justified only after measuring these duplicate copies;
right-sizing resident mesh allocation is already implemented and is not the
missing optimization. Exact per-draw fallback provenance remains a separate
observability follow-up. A future representative workload must prove non-fluid
translucent coverage without silently changing this slice-1 baseline.

### Bench change checks

The final bench suite passed 123 tests. `ravel:gate-lane`, explicitly scoped
against `lsonvklw`, exited 0, including formatting, all package-digest checks,
Python suites and registry checks. No Rust package changed, so the scoped gate
selected no Rust clippy/tests and skipped legacy live launches. The frozen
physical and separate Xvfb functional runs above supply the live evidence;
the rejected timed selected arm remains rejected.

## Acknowledged opaque release and measured duplicate residency

The round-25 allocation observations above are historical. Exact GL acknowledgement
now releases the native SOLID, CUTOUT_MIPPED and CUTOUT quad vectors, including their
capacity. The acknowledged identity and lease remain available for lifecycle checks;
vanilla's VBO remains the fallback. TRANSLUCENT retains its native quads and provenance
for the existing resort path. Refused or inexact acknowledgements do not release a
pending mesh. Native byte-budget accounting follows the remaining actual capacities.

Selected renderers and the timed benchmark now count live/peak vanilla BLOCK-format
section VBO upload bytes, replacing the previous size on reupload and retiring the
entry after successful buffer deletion. Normal unselected play does not maintain
this counter. The timed export records these bytes beside resident GL accounting.
Its `nativeDedupe` pair is an atomic comparison of the same live native population:
hypothetical bytes with its acknowledged opaque capacities still present, then actual
bytes after release, including pending payloads and shared table capacity. It is not
a before/after comparison of two independently populated runs.

Timed slice-1 arms require TRANSLUCENT replacement and owned sort/upload coverage
only when that run successfully admitted non-fluid translucent meshes. Missing,
invalid or negative admission counters are refused; an admitted translucent mesh
still makes zero replacement/sort coverage a failure. The frozen slice-1 world is
unchanged. Glass remains mandatory in the golden and preflight scenes, and their
strict pixel/coverage/depth and moving-sort oracles are unchanged. The live proof
waits for retained translucent payload after opaque release before sampling native
residency, rather than weakening its positive-live-byte requirement.

### Frozen distance-12 diagnostic after release

The fixture-free candidate `wp5s8-production-r26.json` passed the unchanged glass
preflight `wp5s8-resident-r26-preflight` (279 admissions, all five required scenes).
Trial `wp5s8-resident-r26` then completed the unchanged warm-filesystem slice-1
workload at 1280×720 and render distance 12, exit 0. This is an individual
**functional-only** Xvfb/Mesa 25.2.8 llvmpipe run, not an RTX 5090 performance arm
or a repeated performance comparison. Private candidate and run artifacts remain
under the ignored vanilla baseline directory.

The final production export reports bytes, not process RSS or device VRAM:

| Counter | Bytes |
| --- | ---: |
| Vanilla section VBOs, live | 199,466,064 |
| Vanilla section VBOs, peak | 199,469,984 |
| Resident GL, live, including shared scratch | 163,762,928 |
| Resident GL, peak, including shared scratch | 163,839,424 |
| Resident GL payload, live, excluding 2,752,512-byte shared scratch | 161,010,416 |
| Native without opaque release, counterfactual for the same live population | 238,557,524 |
| Native after opaque release, actual live | 698,880 |
| Native peak, actual | 10,168,808 |
| Native budget and GL budget, each | 163,840,000 |

The paired native counter accounts for 237,858,644 fewer bytes (99.7070%).
Its counterfactual is not an observed pre-release run: retaining all those copies
would exceed the unchanged native budget and change which entries could coexist.
The earlier physical trial's actual native snapshot was 163,280,008 bytes.
The new trial retained 1,936 native identities, with no pending entries or native
evictions. GL retained 1,735 layer buffers and recorded 212 evictions; vanilla
retained 5,065 counted section buffers. The paired actual-native value matches
the separate native-budget sample here; independent JNI exports are not required
to be atomically identical.

| Layer | Replacement draws | Vanilla fallback draws | Replaced / (replaced + fallback) |
| --- | ---: | ---: | ---: |
| SOLID | 1,494,254 | 1,594,030 | 48.3846% |
| CUTOUT_MIPPED | 1,395,243 | 1,146,802 | 54.8866% |
| CUTOUT | 899,567 | 813,715 | 52.5055% |
| TRANSLUCENT | 0 | 962,872 | 0.0000% |

This run's non-fluid translucent admission count and global/owned sort/upload
counts were all zero, so timed translucent coverage is not applicable. The
preflight and strict live golden runs still require glass replacement and resort.
Fallback counts above include fluids and unavailable residency, not just budget
evictions; they do not establish that every remaining fallback is budget-caused.

**Recommendation: size resident GL from the measured vanilla section footprint,
not by skipping vanilla uploads yet.** The observed vanilla peak plus shared
scratch is 202,222,496 bytes, already above the current 163,840,000-byte cap.
Both paths use 28-byte BLOCK vertices, but vanilla also retains fluid geometry:
this is a measured sizing baseline, not an exact minimum for resident non-fluid data.
A bounded 224 MiB (234,881,024-byte) distance-12 trial would add headroom within
the existing 256 MiB ceiling. This is a recommended sizing experiment, not a
budget increase made here or proof of full replacement. Native release has removed
the native bottleneck in this run; GL still reaches its cap. Skipping vanilla
uploads would remove the current safe fallback for pending, stale, refused or
evicted sections and needs a separately proven ownership/lifecycle cutover.
Vanilla remains the default and both VBO populations still coexist.

The source `ravel:gate-lane`, `ravel:legacy-agent-build`, geometry-proof live client,
resident-world live client, frozen glass preflight and diagnostic run all exited 0.
The strict golden oracle was not relaxed. Frozen runtime SHA256 values:

| Input | SHA256 |
| --- | --- |
| Agent | `19dd501bdcf9d101e38030ed15be4cd83a6d61f4ecd54625f69ac260f818ac77` |
| Fixture-free host | `2e275f2ed92f3149008f9fef1522335c42d03ad463170519524c31530cc6fe82` |
| Platform | `163881e66c383a7fe9345f806a8c9b5a5210ea6a545db7c0f85d74693b35456b` |
| Screen | `ba1940fcfb4b0c57eb5bfb170bdefbbfebdad67b71a7bcb0ffd084ed937827be` |

## Exact fallback attribution and capture-pool recapture

`geometry-upload.json` now exports `productionFallbackReasons`: four layers by sixteen
mutually exclusive reasons, counted at vanilla's draw site in the same branch as
`productionWorldDraw`'s per-layer fallback counter. The benchmark validator requires
each layer's reasons to sum exactly to that layer's fallback draws. In index order:

| Reason | Meaning for the bound vanilla section VBO |
| --- | --- |
| baseline | admitted, but drawn by vanilla in a baseline frame |
| stale | acknowledged resident, section dirty, moved or recompiled since |
| pending | acknowledged resident with a different pending compile task |
| unacknowledged | offered, waiting for its GL drain and exact acknowledgement |
| drain-refused | drained without exact acknowledgement |
| gl-evicted | acknowledged, but evicted from the GL table |
| gl-over-budget | acknowledged, but larger than the GL budget at admission |
| gl-stale-key | acknowledged, but no current GL entry for its generation, key, resource and atlas, or the GL context changed; GL keys evicted or refused before the latest 4,096 of each also land here |
| fluid | latest compile contains fluid, whole-section fallback |
| skipped | latest compile not captured: capture pool full, configuration refused or controlled window |
| refused | latest compile captured but refused, including resources changing before offer |
| in-flight | latest compile captured but not finished |
| dropped | offered, then removed from the 4,096-entry agent table or by resort refusal |
| untracked | no capture event since the last renderer clear |
| count | resident vertex count differs from vanilla's draw |
| shape | non-quad, offset or non-block-layer draw |

A stale resident whose recompile was skipped, refused or found fluid is labelled with
that latest outcome. Admission guards are unchanged; only labels were added.

Three individual functional-only Xvfb/Mesa llvmpipe runs used the unchanged
warm-filesystem slice-1 workload at 1280×720 and render distance 12, each exit 0 in
343 seconds. Every layer's reasons summed exactly to its fallback count.

| Run | GL budget | SOLID | CUTOUT_MIPPED | CUTOUT | GL peak | GL evictions |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `wp5s8-r27-attrib` | 163,840,000 | 48.8349% | 55.1201% | 53.1903% | 163,839,984 | 259 |
| `wp5s8-r27-gl224` | 234,881,024 | 49.2372% | 56.0252% | 54.4680% | 183,934,464 | 0 |
| `wp5s8-r27-recapture2` | 163,840,000 | 60.4149% | 68.6659% | 64.9976% | 163,839,760 | 392 |

Percentages are replaced / (replaced + fallback) draws. TRANSLUCENT stayed at 0%:
these runs admitted no non-fluid translucent sections.

Opaque and cutout fallback draws by reason, summed over the three layers (zero rows omitted):

| Reason | Attribution, 163,840,000 | 224 MiB | After recapture |
| --- | ---: | ---: | ---: |
| fluid | 1,863,168 | 1,942,871 | 2,222,740 |
| skipped | 1,343,712 | 1,197,721 | 58,221 |
| pending | 127,465 | 154,098 | 133,904 |
| refused | 75,295 | 64,220 | 93,831 |
| unacknowledged | 43,911 | 42,647 | 43,987 |
| stale | 7,332 | 25,465 | 544 |
| untracked | 2,880 | 4,131 | 0 |
| in-flight | 189 | 155 | 186 |
| gl-evicted, gl-over-budget, gl-stale-key, others | 0 | 0 | 0 |
| **Total fallback draws** | 3,463,952 | 3,431,308 | 2,553,413 |

No fallback draw in any run hit a GL eviction, budget refusal or GL key miss. The
163,840,000-byte table evicts sections far from the newest admission, and none of those
was drawn afterward. At 224 MiB the GL peak was 183,934,464 bytes with no evictions, but
the ratios rose only 0.4–1.3 points. That is close to the 0.2–0.7-point spread between
`wp5s8-resident-r26` and `wp5s8-r27-attrib` at the same budget, and every gain came
from reasons other than GL eviction. **The GL budget stays unchanged:** the data does not
support raising it.

The dominant non-fluid cause was `skipped`. The capture pool holds
`captureLimit` sections (43 at distance 12) and the GL drain acknowledges two per
frame. During the initial distance-12 load most compiles found the pool full, and a
section is only captured again when vanilla recompiles it, so those sections stayed vanilla.

The fix records sections whose latest compile found the capture pool full, or whose
resources changed before the offer while the renderer generation was still accepted.
Other skips and refusals are not retried. When the drain queue is empty and no controlled
golden window is open, the resident-world seam marks at most two of them dirty per frame, without
forcing an immediate rebuild. Vanilla recompiles them and the normal capture, comparison,
exact acknowledgement and every admission guard apply unchanged. Skipped draws fell
from 1,343,712 to 58,221. Fluid sections are now 87.05% of the remaining opaque and
cutout fallbacks. Their whole-section fallback is a deliberate guard, kept here.
Some previously skipped sections turned out to contain fluid when recaptured, so the
fluid count rose. An earlier iteration that also retried persistent skips and
refusals (`wp5s8-r27-recapture`) reached 60.1720%/69.6454%/66.2676%; review narrowed the
retry set and bounded it per frame before the final run.

Frozen runtime SHA256 values (host and screen unchanged from the previous section;
the 224 MiB platform was a temporary uncommitted experiment build):

| Run | Agent | Platform |
| --- | --- | --- |
| `wp5s8-r27-attrib` | `ef68e25d2effac7abb43f3a652845512c0ebd4bea17a7df02da28c3d60f4e487` | `22a80e7d82ffcc8a0d230d5af9561984f0772a86d28b90688a46591158a67abb` |
| `wp5s8-r27-gl224` | `ef68e25d2effac7abb43f3a652845512c0ebd4bea17a7df02da28c3d60f4e487` | `de82d6583d0c5835c97085625e25cecc9ecec0da73cedb4d9fdebdf4348898fb` |
| `wp5s8-r27-recapture2` | `ca6ddc366d01fdac41560079b9c773c179d6a2c8a4fb93f635a3ba56a8b231c3` | `aba2e2d6ee4f876b58091aba9c59fa3e4c142c97dcb9882f62fef2c7e3e81a03` |

These are functional software-GL counts, not performance evidence. Vanilla remains the
default. The per-layer work below addresses the dominant fluid fallback without
adding native fluid tessellation. Physical frozen RTX 5090 trials remain next.

## Per-layer fluid provenance

The pinned 1.12.2 client bytecode establishes the layer boundary directly:
`aru.f()` (Feather `LiquidBlock.getRenderLayer`) compares its material with
`bcz.h` (`WATER`), returning `amm.d` (`TRANSLUCENT`) for water and
`amm.a` (`SOLID`) otherwise, including lava. `bxr` (`RenderChunk`) selects the
builder using that layer's ordinal; `bvm` (`BlockRenderDispatcher`) passes that
same builder to `bvn` (MCP `BlockFluidRenderer`). This was checked with `javap`
against the pinned client SHA256
`8ada07da5ee77dad3527bd7278fbd05ee1fc8a597813b216a871a2d7d64cc64f`
and Feather `1.12.2+build.31`, not assumed from material transparency.

The capture independently counts observed baked vertices in each layer and
compares them with each final vanilla builder's vertex count. Every retained
layer requires exact equality, not equality of a section-wide total. Water's
TRANSLUCENT layer and lava's SOLID layer are excluded in their entirety from
both the Rust input and the raw vanilla oracle. Their observed baked subset
must still fit the final builder count. The complete vanilla vertex budget,
generation/resource/atlas checks, exact upload acknowledgement, section identity,
pending/dirty guards and per-draw vertex checks remain in force. A fluid mask
travels with the resident entry so only excluded layers receive the fluid
fallback reason after all admission guards pass.

The retained transparency callbacks skip sorting and upload when the resident
TRANSLUCENT count is zero: vanilla water must not sort or upload an excluded
fluid layer through the opaque resident lease. Acknowledgement then performs no
transparency upload. All nonempty translucent guards remain unchanged.
Unpublished captures with non-fluid geometry retain their refusal reason;
captures whose only geometry is in excluded fluid layers retain the fluid reason.

Two additional static composited scenes exercise a water/stone section and a
lava/cactus section in initial, staged-reload and reopen lifecycles. The raw
format records actual substituted draw counts by layer; its strict oracle
requires exactly SOLID for water/stone and exactly CUTOUT for lava/cactus.
Existing pure-fluid fallback controls remain unchanged. These controls do not
change the default renderer or the slice-1 workload.

The distance-12 Xvfb before run, `wp5s8-r28-before`, used the frozen round-27
recapture candidate and exited 0. Replacements/fallbacks by layer were
SOLID 1,866,812/1,193,517 (61.0004%), CUTOUT_MIPPED 1,728,251/790,033
(68.6281%), CUTOUT 1,111,666/588,699 (65.3781%), and TRANSLUCENT
0/955,213 (0%). The complete per-layer reason vectors are retained in the
private diagnostic export; fluid accounted for 1,035,896, 644,213, 499,070
and 944,782 fallback draws respectively. These are functional counts, not
physical timing or a resource-win verdict.

The selected `geometry-proof` live gate exited 0. All 45 composited rows
(30 static, including six new mixed rows, and 15 moving) passed the unchanged
strict oracle. A separate reader recounted the raw three-arm RGBA/depth
buffers without importing the repository oracle: zero RGBA, coverage or
depth mismatches in all 45 rows, with positive omission footprints throughout.
The new raw rows recorded:

| Lifecycle | Water/stone: replaced layers, vertices, visible pixels | Lava/cactus: replaced layers, vertices, visible pixels |
| --- | --- | --- |
| Initial | SOLID only, 544, 5,264 | CUTOUT only, 68, 2,281 |
| Staged reload | SOLID only, 544, 5,264 | CUTOUT only, 68, 2,264 |
| Reopen | SOLID only, 544, 5,265 | CUTOUT only, 68, 2,281 |

Every water row contained 16 water source cells and no flowing/lava cells;
every lava row contained 16 lava cells and no water cells. The lava block
settled from its flowing ID to its still ID across lifecycles, without changing
the layer exclusion. No fluid layer was substituted.

The final frozen diagnostic, `wp5s8-r28-after4`, also exited 0 on Xvfb
at distance 12 with the same functional-only workload. This diagnostic does
not supersede the failed resident-world acceptance below. It used agent
`5938b6eaf55faca42466a24dc736f4dfcddf57c592b9e4064517b65578558974`,
platform `f10fb2ca824676ef7ff1d978813fa8d6b537b31c10a662da1ae0a8c6a3315070`
and the unchanged pinned native host
`2e275f2ed92f3149008f9fef1522335c42d03ad463170519524c31530cc6fe82`.

| Layer | Before replaced / fallback | Before ratio | After replaced / fallback | After ratio |
| --- | --- | --- | --- | --- |
| SOLID | 1,866,812 / 1,193,517 | 61.0004% | 2,482,098 / 354,392 | 87.5060% |
| CUTOUT_MIPPED | 1,728,251 / 790,033 | 68.6281% | 2,090,152 / 186,563 | 91.8056% |
| CUTOUT | 1,111,666 / 588,699 | 65.3781% | 1,433,443 / 119,159 | 92.3252% |
| TRANSLUCENT | 0 / 955,213 | 0% | 0 / 924,081 | 0% |

Exact fallback counts follow, with each cell `before → after`:

| Reason | SOLID | CUTOUT_MIPPED | CUTOUT | TRANSLUCENT |
| --- | --- | --- | --- | --- |
| Stale | 2,333 → 61 | 2,639 → 597 | 2,511 → 405 | 387 → 313 |
| Pending | 82,106 → 66,237 | 76,141 → 57,863 | 43,688 → 33,252 | 1,296 → 19,247 |
| Unacknowledged | 17,486 → 40,373 | 14,448 → 31,350 | 9,326 → 22,868 | 0 → 14,790 |
| Fluid | 1,035,896 → 111,880 | 644,213 → 0 | 499,070 → 0 | 944,782 → 850,211 |
| Capture skipped | 33,004 → 74,952 | 27,758 → 57,844 | 17,674 → 36,150 | 8,737 → 18,072 |
| Capture refused | 22,616 → 60,841 | 24,783 → 38,890 | 16,417 → 26,482 | 0 → 21,445 |
| In flight | 76 → 48 | 51 → 19 | 13 → 2 | 11 → 3 |
| Untracked | 0 → 0 | 0 → 0 | 0 → 0 | 0 → 0 |

Drain refusal, GL eviction, GL over-budget, GL stale key, dropped residents,
draw-count mismatch and draw-shape reasons were zero in every layer of both
runs. The reason columns sum exactly to their fallback totals. The opaque/
cutout fluid share fell from 2,179,179 of 2,572,249 fallback draws to
111,880 of 660,114; only lava's SOLID and water's TRANSLUCENT
layers retain fluid-labelled fallbacks, never either cutout layer.
The trials have different frame/draw denominators; their counts and ratios
are functional observations, not paired timing measurements or a causal
performance estimate. No native fluid tessellation or default cutover was
introduced. Physical frozen vanilla-versus-resident-world trials remain next
only after the resident-world regression below is resolved.

The final candidate is not accepted. The same resident-world live command
passes on base `rkluwvkl` but the feature run exits 105 after emitting only the
initial moving-translucent capture. That capture has section sort counters
`[1,1,0]`, global counters `[10,10,0]`, origin `[-176,192,-32]`, and camera
`[-174,198,-24,-90,30]`; it does not prove later moving points or lifecycles.
Zero-resident-translucency callbacks now skip excluded water sorting/upload,
but this did not resolve the moving stall. The narrowing step is to restore
byte-for-byte pre-change admission, lease and sort behaviour for fluid-free
sections, retaining per-layer handling only where the capture contains fluid.
No timeout, camera path or strict oracle has been relaxed.
