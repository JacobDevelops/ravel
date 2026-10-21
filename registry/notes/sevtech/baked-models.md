# WP5 slice 6: baked input and independent CPU emission

## Status and boundary

This note records implementation, not completed slice-6 acceptance. Vanilla still
owns world drawing. The renderer selection remains off by default. The existing
selected GL proof remains a position-only solid projection; it is not textured
vanilla-fidelity output. No replacement-renderer performance claim is made.

The new CPU contract accepts the actual face-culled vanilla model quads, not an
invented cube approximation. Model resolution, connected/multipart selection,
face selection, tint providers, AO and light sampling are still vanilla work on
the owning compile thread. Rust receives only owned primitive values.

| Field | Provenance and computation |
| --- | --- |
| Adapter state | Version-local Java block-id/metadata encoding; Rust treats it as opaque |
| Model XYZ and atlas UV | Exact `BakedQuad.getVertices`/builder source words, before shading/translation |
| Sprite | Actual `getSprite` object identity, assigned a bounded generation-scoped numeric identity in Java; no hash collision alias |
| Tint | Independently observed `BlockColors.get` result |
| AO | Independently observed `AmbientOcclusionFace.compute` brightness array |
| RGB multipliers | Actual per-vertex inputs to `BufferBuilder.multiplyColor`; these already include vanilla tint/AO choices |
| Combined light | Actual per-corner packed lightmap inputs, copied rather than recomputed from world samples |
| Final XYZ | Rust adds the source float to the effective double offset rounded to float first, matching vanilla's operation order |
| Final color | Rust performs the little-endian integer component/float multiplication and shift arithmetic |
| Final light word | Rust writes the supplied packed light word; it does not claim independent light propagation |
| Translucent order | Rust computes squared centroid distance and descending order, preserving source ordinal for equal distances; cancellable in-place resort |
| Final vanilla buffer | Oracle only, read after compile normal return; never substituted for Rust-emitted vertices |

Exact Feather-resolved inventory entries cover model rendering, baked vertices,
AO, tint, builder source vertices/light/multipliers/position and their reflected
members. This extends the 1.12.2 adapter, not the SDK's game-object boundary.
The Rust neutral state API no longer decodes legacy block IDs or metadata.

## Bounds and lifetime

The CPU value layer admits at most 65,536 quads, checking retained capacity as
well as length. Input and output payload ceilings derive from concrete Rust
value sizes. All model coordinates, UVs, AO/multipliers, offsets, transformed
positions and cameras must be finite. The value API requires matching resource
and atlas generations, but capture does not yet carry an independent atlas
generation; reload correctness is not established. Cancellation or failed resort
never authorizes drawing.

The selected-only Java comparison capture has one active owning compile thread,
8,192 source quads and fixed source/oracle buffers. Busy sections are refused.
Uncolored builder state, oversized input, missing provenance, pending quads,
unaccounted final vertices (including fluid tessellation), stale generations and
other capture failures are unsupported rather than truncated success. Java game
references remain bounded and are cleared by lifecycle retirement. The JNI gate
is scoped to its lifecycle, not a process-static permit detached from injected
state. The native CPU comparison returns quad count, mismatched BLOCK words,
first mismatch, elapsed CPU time and retained input/output bytes. Java evidence
currently omits the first-mismatch position. It never publishes a drawable baked
mesh. Baked per-vertex instrumentation still runs when selection is off; removing
that overhead remains an unresolved independent-review finding.

## Golden-scene matrix retained, not narrowed

No current textured golden-scene readbacks have been exercised. Unavailable
mismatch counts are **not zero** and are not carried forward from slice 5.

| Required scene | Current status | RGBA / coverage / depth mismatches | Exact missing behavior |
| --- | --- | --- | --- |
| Opaque textured models | Unsupported | Not measured | Selected GL submission still uses the old position-only contract, without baked UV/color/light vertices or atlas/lightmap sampling |
| Cutout models | Unsupported | Not measured | No independently emitted baked layer upload, texture sampling or matching alpha-test path |
| Translucent models/fluids | Unsupported | Not measured | CPU baked-quad sorting exists; no translucent shaded upload/blending path, and fluid immediate-mode tessellation is not a baked model input |
| Animated atlas sprites | Unsupported | Not measured | Actual animated sprite provenance is observed; Ravel has no animation-aware texture submission/readback comparison |
| Staged resource reload | Unsupported for shaded fidelity | Not measured | Generation rejection is represented, but no shaded resident atlas/model pair has been compared across replacement |
| Staged world unload/reopen | Unsupported for shaded fidelity | Not measured | No live shaded residency/retirement scene has been exercised in this slice |

## Verification and measurements

The current renderer/bootstrap contract test command passed: 52 renderer tests,
239 bootstrap unit tests and one bootstrap integration test, with no failures.
The selected legacy-agent build also exited zero after deliberately regenerating
the exact mapping lock and fixing reflection/native export integration.
These checks do not establish live rendering fidelity. Independent review still
requires exact typed-hook shape refusal, selection-gated instrumentation, real
atlas-generation validation and external baked-evidence validation.

Current six frozen-candidate RTX 5090 trials (three off, three on), separate Xvfb
functional acceptance, final clippy, three-version tests and coverage are not yet
reported. Historical slice-5 frame/upload/residency results are not evidence for
this changed candidate. Slice 6 remains incomplete until the remaining acceptance
is exercised and its unsupported scene dispositions are accepted explicitly.
