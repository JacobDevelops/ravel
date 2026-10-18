# WP5 slice 3: bounded observational section snapshots

## Scope and format

This slice captures values at the pinned 1.12.2 `RenderChunk.compile` normal-return
seam. Vanilla still creates meshes, owns models and textures, performs visibility
work, and uploads/draws GL resources. This is not a replacement renderer or a
resource-saving claim.

Java reads the task's post-compile region on its vanilla compilation thread. A
nonblocking capture permit and the chunk's task lock guard admission and protect
the task/region/origin association. The game lock is released after those local
references and coordinates are captured, before value reads or JNI. Cancelled,
superseded, contended and stale captures are rejected rather than queued. Exact
Feather members are generated from the reviewed reflection surface; no guessed
aliases or block-level JNI calls are used.

Each batch contains 5,832 cells: the 16³ section and a complete one-cell border,
including every face, edge and corner. Coordinates run from section origin minus
one through origin plus sixteen, indexed `((y * 18) + z) * 18 + x`. An `int[]`
contains numeric block identity shifted left four bits plus four-bit metadata;
identities through 1,048,575 are accepted without historical twelve-bit ID
truncation. A `byte[]` contains raw stored sky light in the upper nibble and block
light in the lower nibble, not vanilla's combined mesh-light result. The
generation is a positive Java `long`.

Rust validates lengths, state range and generation before constructing immutable
owned values. A deduplicated four-byte palette stores state/model lookup keys,
not baked model identities or geometry; cells use two-byte palette indices and
one-byte light. A single native nonblocking permit bounds concurrent input
copies. Generation is checked under the lifecycle mutex before capture and at
final admission. Every decoded state and light byte is compared against the
Java-copied post-compile region values; the observational snapshot is then
discarded. This proves value-copy/packing fidelity, not equality to the values
vanilla read earlier while compiling. The region references underlying chunks,
so Java source reads are not an atomic frozen-world transaction.

Proof captures also reread the same vanilla region at 125 explicit
origin-relative positions, including 98 border positions across faces, edges and
corners. Those sampled source values are compared with the copied Java batch as
a coordinate/order oracle, not an independent member-mapping oracle. Only a
clean candidate is retained for stale-generation proof; failed candidates and
their differing cells are counted separately because the live source can
change between reads. They do not turn the entire region into a frozen
transaction. Hot-loop reads use cached primitive-adapted method handles rather
than per-cell reflective argument arrays and boxed coordinates.

## Explicit payload bounds

| Allocation or storage | Bound |
| --- | ---: |
| Cells and palette entries | 5,832 each |
| Java batch payload | 29,160 bytes |
| Java proof-mode batch payload, including retained stale input | 58,320 bytes |
| JNI input-copy payload | 29,160 bytes |
| Retained snapshot heap | `17,496 + 4 × palette_entries` bytes |
| Maximum retained snapshot heap | 40,824 bytes |
| Capture-only heap scratch palette | 23,328 bytes |
| Capture-only stack scratch lookup | 16,384 bytes |
| Total capture-only scratch payload | 39,712 bytes |
| Snapshot capture allocations | Four total; three retained |
| Fixed Java timing/metrics payload | 32,952 bytes |
| Percentile-report temporary sample payload | At most 32,768 bytes per query |

The receipt's snapshot byte count includes the Rust object and retained slice
payloads. Payload accounting excludes allocator metadata, JVM array/object
headers, cached reflection/method-handle objects, scalar locals and stack-frame
overhead. Fixed bookkeeping stores 4,096 timing samples and 23 scalar metrics;
it does not grow with world size. JNI receipts carry six longs (48 payload
bytes). Total RSS is not derivable from these payload bounds.

## Interpretation against slice 1

The slice-1 baseline in [vanilla-baseline.md](vanilla-baseline.md) measures
vanilla compilation under its disclosed physical-GPU workload. Snapshot timing
includes Java value reads and the native roundtrip, not only Rust palette
packing. Adding observational capture while vanilla still meshes cannot
demonstrate renderer CPU, heap, RSS or GL-resource savings. Acceptance timings
must not be treated as matched baseline trials or subtracted from those
compilation distributions.

## Recorded functional run

The fixed seed is `8675309`. Both runs exercised integrated world creation,
resource reload, unload and reopen. Generations advanced `4 → 5 → 6 → 8 → 9`;
an actual retained section batch was rejected after its generation retired.
The source-coordinate proof checked 125 positions, including 98 border
positions, with zero differing values and zero failed candidates in each run.
Every native-decoded cell matched the Java-copied state/light input.

| Observation | Xvfb functional acceptance | Physical display |
| --- | ---: | ---: |
| Admitted captures | 326 | 255 |
| Compared cells | 1,901,232 | 1,487,160 |
| Worker-thread captures | 278 | 178 |
| Non-air cell observations | 1,096,569 | 1,023,582 |
| Contended/cancelled/superseded skips | 180 | 87 |
| Decode mismatches / native rejections | 0 / 0 | 0 / 0 |
| Mean snapshot bytes, including Rust object | 17,605.61 | 17,615.58 |
| Maximum snapshot bytes | 17,648 | 17,648 |
| Mean capture time, ms | 0.364851 | 0.337408 |
| Capture p50 / p95 / p99, ms | 0.294912 / 0.860257 / 2.352147 | 0.218626 / 0.962120 / 2.119541 |
| Maximum capture time, ms | 6.575856 | 7.629555 |

The physical run used the RTX 5090, NVIDIA `580.178.04`, an actual OpenGL
`4.6.0 NVIDIA 580.178.04` context, LWJGL `3.3.6+1` and the pinned JDK 27
runtime. Acceptance defaults were 854×480, render distance 2, maximum 60 FPS,
vsync off, fancy graphics off and a 2 GiB heap. Its sample covers the scripted
load/reload/unload/reopen sequence, not a steady-state benchmark. No heavy
verification ran concurrently with this measurement. Xvfb numbers are
functional diagnostics, not GPU performance evidence.

For context only, slice 1's three Ravel idle compilation trials measured p50
`1.373 / 1.240 / 1.198 ms`, p95 `3.632 / 3.740 / 3.130 ms` and p99
`5.044 / 5.422 / 4.006 ms`. Those baseline trials used 1280×720, render distance
12, unlimited FPS and the disclosed 4/8 GiB heaps, with dedicated warmup and
idle/exploration/stress windows. Compile work and snapshot work also have
different denominators. These side-by-side observations do not measure added
frame-time overhead, resource savings or a renderer speedup.

The local functional proof is produced by `cargo make ravel:legacy-agent-live`;
its renderer counters are in
`tests/fixtures/local/legacy-platform/evidence-xvfb-agent/renderer.json`.
The physical-display launch used the existing `client-agent display` mode and
the `-wp5s3` evidence suffix, producing
`tests/fixtures/local/legacy-platform/evidence-display-agent-wp5s3/renderer.json`.
These are ignored local artifacts, not promoted milestone evidence.

## Validation

Strict workspace/all-targets/all-features clippy, `ravel:legacy-agent-build`,
`ravel:legacy-agent-live`, the physical-display launch and the clean
`ravel:verify` gate all exited zero. The full gate ran after deleting
`target/llvm-cov-target` and completed in 271.19 seconds.

Each of 1.12.2, 1.21.1 and 26.1.2 passed 4,242 workspace tests with zero
skips, plus its no-default-features composed-host test. Java generation,
compilation and capability-appropriate JVM harnesses passed for all three;
both modern versions ran all thirteen harness sections. Gate-version
reconciliation passed.

Clean Rust line coverage was 93.11% workspace-wide, 99.01% for
`ravel-renderer/src/lib.rs` and 84.67% for the native snapshot module.
Rust coverage does not include Java execution; the live batch/coordinate and
generation checks above supply that separate functional evidence.
