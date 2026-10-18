# WP5 slice 4: bounded geometry and visibility workers

## Scope and ownership

Slice 4 computes CPU geometry and visibility from the immutable slice-3 snapshot. It does not replace vanilla
rendering, upload buffers, save resources or establish visual/model parity. Java still compiles and renders the world.
Native completion buffers are owned until the checked diagnostic drain, then discarded. No Rust worker accesses
Minecraft, JNI, GL or unsafe code.

The 18³ snapshot contains a 16³ core and one-cell neighbour border, deduplicated state keys, u16 palette indices and
raw sky/block light nibbles. Capture now records the game's `isOpaqueCube && isFullCube` classification per state,
rather than recognizing only a particular block ID. Air is empty regardless of its supplied flag. Contradictory flags
for one key are rejected. Every unsupported palette key is reported, including border keys; partial geometry is
explicitly incomplete and cannot masquerade as a faithful section.

Each generation/key-tagged result has four layer slots (solid, cutout-mipped, cutout, translucent), currently only
solid geometry. Vertices contain local u8 XYZ, a face byte and the adjacent cell's packed raw light byte; indices are
u32. Quads have four vertices and six indices with deterministic winding. Material ranges are contiguous by state key.
Per-quad ranges carry key, origin, face and first index, with integer doubled centres for future sorting. Textures,
UVs, ambient occlusion, biome tint, baked models and translucent/animated geometry remain outside this slice. A 36-bit
symmetric face-to-face visibility matrix flood-fills the core; full opaque cells block connectivity, while unknown
cells conservatively remain open. Culling reads the snapshot border, never a live neighbour.

Two standard-library worker threads share eight fixed slots across queued, running and completed ownership. On this
64-bit target, the total reservation ceiling is **14,036,160 bytes**, or 1,754,520 bytes per accepted job. It reserves
worst-case snapshot, mesh, bounded mesh scratch and ownership records before admission. Mesh output is bounded by
1,694,928 bytes, five owned-buffer allocations and 12,288 bytes of fixed-array scratch. Snapshot retained heap payload
is bounded by 46,656 bytes. Captures use one nonblocking permit; JNI input copy payload is at most 34,992 bytes and
capture scratch plus input copy is at most 80,536 bytes. The Java work-key table has 16 slots; polls return at most
eight 11-long records and statistics have 18 longs.

These are logical owned payload/allocation ceilings, not a global allocator, RSS, VRAM or JVM measurement. Allocator
headers, thread stacks, scalar frames, fixed pool bookkeeping and Java objects are not included. The one concurrent
capture's JNI scratch is separate from pool reservation. A retained stale-proof Java batch adds 34,992 payload bytes
to the current batch, bounded at 69,984 Java batch payload bytes.

Admission refuses full, duplicate, closed or stale work rather than growing queues. Cancellation is checked during
scanning, emission and visibility flood fill; a final synchronized generation/slot check forbids late publication,
including generation ABA. A running cancelled owner retains its reservation until exit. Failed JNI publication retains
completed ownership for retry. Shutdown joins workers and proves zero queued/running/completed owners and reserved
bytes. Retired-pool diagnostics expose the actual old generation, not an invented empty current pool. Java closes
admission before shutdown and waits for already reserved receipt owners to settle. Worker panic is a terminal,
observable failure rather than a stranded reservation.

## Live route and observations

The official 1.12.2 agent ran the fixed seed **8675309** route: six 128-block teleports, at least 32 drained jobs at
each step, then return, reload/unload/reopen and close. The physical run used `DISPLAY=:0`, **NVIDIA GeForce RTX
5090**, driver **580.178.04**, OpenGL **4.6**. Xvfb used Mesa OpenGL 4.5 as a functional run, not GPU performance
evidence. Both used the acceptance harness's 854×480 window, render distance 2, 60-FPS cap, VSync off and fancy
graphics off. Both retained vanilla work and the capture/oracle instrumentation.

| Observation | Physical RTX 5090 | Xvfb functional |
| --- | ---: | ---: |
| Captures / copied cells | 592 / 3,452,544 | 551 / 3,213,432 |
| Captured full-opaque cells | 962,198 | 988,679 |
| Admitted / drained | 581 / 580 | 542 / 542 |
| Complete / explicitly incomplete | 299 / 281 | 278 / 264 |
| Nonempty / occluding results | 271 / 126 | 259 / 115 |
| Stress admitted / drained | 286 / 286 | 257 / 257 |
| High-water jobs / reserved bytes | 8 / 14,036,160 | 8 / 14,036,160 |
| Refused admissions / Java table full | 11 / 0 | 9 / 0 |
| Native output bytes summed / largest | 8,788,152 / 75,748 | 8,919,336 / 55,896 |
| Owned buffer allocations summed / largest per mesh | 1,365 / 5 | 1,300 / 5 |
| Vertices / indices | 608,040 / 912,060 | 618,508 / 927,762 |
| Unsupported-key occurrences / largest per result | 1,326 / 12 | 1,384 / 12 |
| Actual admitted work retired before drain | 1 | 0 |

Both had zero copy mismatches, coordinate-oracle mismatches, capture failures, mesh errors, worker failures, stale
native publications, invalid completion records and drain failures. Both rejected an injected stale completion with a
real reserved current key and rejected retired-generation native calls. Unload verified the actual retired generation
5; three pool drains were checked, and final close reported stopped=1, all work counts/reserved
bytes/draining/failures=0 and Java keys held=0. The physical run observed one real cancellation; Xvfb cancellation
coverage comes from deterministic harness/race tests, not an invented live cancellation.

### Timings (milliseconds)

At most 4,096 samples per series are retained; these runs fit within that bound. Queue time is admission-to-worker
ownership; mesh time is the CPU mesh/visibility operation. Drain time includes owner-thread polling/JNI/diagnostic
handling, not a GL upload. Frame intervals are observational, not exclusive attribution to Rust.

| Series | Physical p50 / p95 / p99 / max | Xvfb p50 / p95 / p99 / max |
| --- | --- | --- |
| Rust mesh | 0.113897 / 0.244395 / 0.325219 / 0.361698 | 0.128705 / 0.308777 / 0.330519 / 0.358142 |
| Queue wait | 0.004219 / 0.015419 / 0.018144 / 0.876147 | 0.003877 / 0.126781 / 0.908329 / 1.014951 |
| Diagnostic drain | 13.809070 / 16.838380 / 19.155249 / 24.409250 | 12.900146 / 16.374667 / 18.307064 / 50.868956 |
| Capture | 0.235689 / 0.417113 / 1.289924 / 9.588762 | 0.251168 / 0.545368 / 1.437515 / 6.551166 |
| Stress frame interval | 16.666605 / 16.676313 / 16.683106 / 17.707395 | 16.666692 / 16.675097 / 16.681390 / 18.140877 |

Physical first capture/queue/mesh/drain were 9.585175 / 0.018946 / 0.078640 / 2.761633 ms. The maximum frame interval
in the first five stress seconds was 16.695930 ms. Xvfb first capture/queue/mesh/drain were 6.545976 / 0.017043 /
0.133384 / 50.868956 ms; its first-five-second maximum frame interval was 17.910740 ms. These disclose first-use
observations, not isolated stall attribution.

The [slice-1 baseline](vanilla-baseline.md) measured Ravel vanilla moving compile p50 0.001–0.041 ms, p95
14.337–18.970 ms, p99 35.501–38.268 ms and maxima 53.392–61.252 ms across three trials. Idle compile p50 was
1.198–1.373 ms, p95 3.130–3.740 ms and p99 4.006–5.422 ms. **No speedup comparison is valid:** those trials used
1280×720, distance 12, uncapped frames and 4–8 GiB, a different route, and all vanilla compile invocations (including
aborted/no-work calls). This slice uses admitted/completed geometry-only jobs, no model/texture/AO parity, and still
performs vanilla compilation. It is not a matched ablation or replacement compile benchmark.

## Reproduction and evidence

Build with `mise exec -- numen run -- cargo make ravel:legacy-agent-build`; run `ravel:legacy-agent-live` through the
same wrapper for Xvfb/integrated/connected-dedicated/restart acceptance. Physical reproduction uses `mise exec --
numen run -- env DISPLAY=:0 RAVEL_LEGACY_EVIDENCE_SUFFIX=-wp5s4 bash jvm/legacy-platform/launch.sh client-agent
display`. The physical JSON also passed the same `live.renderer_proof` validator. No raw process logs are committed.

Raw evidence is under ignored `tests/fixtures/local/legacy-platform/`:

- `evidence-display-agent-wp5s4/renderer.json`: SHA-256
  `50b76273e6d8a52292a9f613d351c00edf444c6ccd1dbc00e39f5c79cbb704cf`.
- `evidence-display-agent-wp5s4/stress.txt`: SHA-256
  `705964edcf3f8897ed698192ffb332f72369538b5539879d9e97617693a61896`.
- `evidence-xvfb-agent/renderer.json`: SHA-256 `2df9bdde0d3d76fd4471aa8b6c67d1ff8ed69045c870456436505c313240f687`.
- Both evidence directories also retain `client.log`, `04a-stress.png`, lifecycle screenshots and seed/action records.
  Later default Xvfb runs may overwrite their ignored evidence directory; hashes identify this recorded run.

Final post-review Xvfb rerun passed with 566 submitted/drained jobs, 270 stress jobs, 265 nonempty/127 occluding
meshes, and the same 8-job/14,036,160-byte high-water marks. All error/drain/stale-publication counters stayed zero;
close retained zero native work/bytes and Java keys. Mesh p50/p95/p99 were 0.131269/0.316642/0.333804 ms; queue
0.004078/0.122753/0.911362 ms; output totaled 9,058,220 bytes and 1,335 allocations (maximum five/job).
The replacement Xvfb `renderer.json` SHA-256 is
`61df01d0d82d245500ca7e25719d46d87d9a7ead93159ce0638a9549e41197ea`.

Deterministic Rust tests cover primitive and bordered culling hashes, all face winding/light, high state keys,
unsupported keys, sealed walls/pockets, checkerboard output bounds, queue/byte ceilings, real worker wake,
same-generation and ABA cancellation, publication races, panic ownership, transactional JNI retry and shutdown drain.
Primitive geometry hash is `4fbc2b7e8a50c27d`; the shared/border-culling fixture hash is `bb7ae3029f02ece3`. The
executable Java/native harness additionally exercises exactly eight accepted of twelve submissions, malformed
receipts, stale identity, visibility and delayed shutdown receipt settlement. Upload remains explicitly absent. Slice
5 is next.

## Verification

Strict workspace/all-target/all-feature clippy, legacy agent build, the executable legacy harness and live acceptance
each exited 0. The offline live-validator suite passed 16 tests. Minecraft 1.12.2, 1.21.1 and 26.1.2 each passed
4,285 Rust tests plus the no-default-features composed-host test, generated Java compilation and all required JVM
harness sections. The per-version reconciliation gate and final clean `ravel:verify` both exited 0.

The final clean coverage run passed its 80% maintained-line floor: 92.97% overall lines, mesh 96.89%, workers 96.84%,
snapshot JNI 85.63% and worker JNI 93.88%. Renderer lifecycle JNI was 43.06%; live JVM checks exercise that boundary
outside the Rust coverage process. Independent Rust/worker/JNI/Java reviews were completed and their correctness
findings addressed. The advisory quality-delta scan exited 2; its heuristic findings were reviewed, not suppressed,
and it is not reported as a passed gate.
