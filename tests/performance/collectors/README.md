# Performance collectors

Each collector invocation emits one document conforming to
`schemas/performance-collector.schema.json`. The document represents one subject
run and uses a monotonic nanosecond timeline for event alignment.

Collectors must:

- emit process, world, measurement, route, and dimension event markers;
- declare all eight normalized metrics available, unavailable, or not applicable
  before samples;
- label available measurements as exact or estimated;
- retain cumulative swap counters so aggregation can calculate throughput;
- encode memory pressure as `0` normal, `1` warning, or `2` critical;
- omit unavailable values rather than writing zero;
- record the collector version, command hash, clock resolution, and platform.

Frame and integrated-server tick measurements are event sampled. RSS, pressure,
system-used memory, swap counters, and GPU allocation use the intervals in the
selected profile. Raw output remains outside tracked public evidence; only
sanitized results and hashes are admitted.

## Linux memory streams

The Linux collector reads process RSS from `/proc/<pid>/smaps_rollup` so the
required exact metric does not depend on the kernel's approximate `VmRSS`
counter. System-used memory is the estimated `MemTotal - MemAvailable` value
from `/proc/meminfo`.

Swap input and output retain the cumulative `pswpin` and `pswpout` page counters
from `/proc/vmstat`, converted with the kernel base page size. A decreasing
cumulative counter invalidates the sample.

Memory pressure is calculated over each sampling interval from the cumulative
`some` and `full` totals in `/proc/pressure/memory`:

- no increase is level `0` (normal);
- a `some` increase without a `full` increase is level `1` (warning);
- any `full` increase is level `2` (critical).

The normalizer rejects missing units, impossible memory totals, conversion
overflow, and counter regression. The exact platform mapping is versioned in
`tests/performance/profiles/initial.toml`.

Capture the PID-attached stream without desktop input:

```console
cargo run --locked -p ravel-cli -- \
  performance capture-linux-memory PID SECONDS \
  evidence/private/m9/linux-memory.csv
```

The command samples on fixed one-second monotonic deadlines and creates the
destination exclusively. `self` may replace `PID` for a one-process preflight;
M9 evidence uses the Minecraft JVM PID. The collector checks stat field
22 before and after every snapshot, so process exit and numeric PID reuse
invalidate the capture instead of switching the RSS source. A read,
normalization, timing, or write failure removes the partial CSV.

The raw CSV contract is:

```text
sample_index,elapsed_ns,process_rss_bytes,system_used_memory_bytes,memory_pressure_level,swap_in_total_bytes,swap_out_total_bytes
```

Indices begin at zero and remain contiguous. Elapsed timestamps are strictly
increasing. Phase and lifecycle markers are joined later on the same monotonic
timeline; this command alone does not claim a complete collector document.

## Lifecycle marker streams

The initial lifecycle-marker protocol version 2 requires exactly 17 ordered
lifecycle markers:
the eight singleton process/game/world/phase events, five canonical route-step
events, two dimension-change starts, and two dimension-ready events. Validate a
raw marker stream without desktop input:

```console
cargo run --locked -p ravel-cli -- \
  performance verify-initial-markers \
  evidence/private/m9/markers.csv
```

The pinned CSV contract is:

```text
event_index,elapsed_ns,kind,route_step,dimension
```

Indices begin at zero and remain contiguous. Elapsed timestamps never decrease;
equal timestamps allow one logical boundary to emit `measurement-start` and its
first `route-step-start` atomically. Route-step events carry both metadata
fields. `world-ready`, `dimension-change-start`, and `dimension-ready` carry
only a dimension. Every other event leaves both fields empty.

The validator pins the five route IDs, dimensions, and offsets from
`tests/performance/profiles/initial.toml`. Warm-up and measurement spans must
match 300 and 900 seconds within the profile's 5 millisecond alignment
tolerance. Process exit must follow at least 120 seconds of cool-down. Phase
assignment uses `world-load-start`, `warm-up-start`, the active route step, and
`measurement-end` as inclusive boundaries; samples at or after `process-exit`
are invalid.

Start the marker recorder in a background terminal before Prism:

```console
install -d -m 700 evidence/private/m9
cargo run --locked -p ravel-cli -- \
  performance record-initial-markers \
  evidence/private/m9/markers.sock \
  evidence/private/m9/markers.csv
```

Send operator-observed events from any shell without desktop automation:

```console
cargo run --locked -p ravel-cli -- \
  performance mark-initial \
  evidence/private/m9/markers.sock process-start
```

`world-ready`, `dimension-change-start`, and `dimension-ready` take the
dimension as a final argument. The recorder automatically emits
`measurement-start`, all five route-step boundaries, and `measurement-end` from
the fixed schedule after `warm-up-start`. It requires an owner-only socket
directory before binding, creates the CSV only after all 17 events validate,
sets the socket to mode `0600`, rejects a scheduling wake delay above 5
milliseconds, reserves that final 5-millisecond window from socket request
handling, and removes only the socket inode it created on every normal return
path. It never activates a window or reads global input.

Marker and Linux memory timestamps can only be joined when both originate from
one recorder clock. Tracy frame timestamps use Tracy's own initialization
origin and require a separately proven alignment transform; matching numeric
nanoseconds from independent origins is invalid. Do not join the standalone
marker recorder to a separately started `capture-linux-memory` stream.

For Linux evidence, start the combined recorder before Prism:

```console
cargo run --locked -p ravel-cli -- \
  performance record-initial-linux \
  evidence/private/m9/markers.sock \
  evidence/private/m9/markers.csv \
  evidence/private/m9/linux-memory.csv
```

Its process-start command requires the PID that will become the Minecraft JVM:

```console
cargo run --locked -p ravel-cli -- \
  performance mark-initial-linux \
  evidence/private/m9/markers.sock process-start PID
```

Configure the Prism instance's wrapper command to end with:

```console
/absolute/path/to/ravel-cli performance supervise-initial-linux \
  /absolute/private/path/markers.sock --
```

Prism appends the Java executable and arguments. Ravel starts a marker child,
that child sends its own PID before replacing itself with the pinned Java
runtime, and the supervisor emits process exit after Java finishes. A nonzero
or crashed Java process sends `abort` instead, invalidating and removing the
partial capture and returns Java's exit status to Prism. The supervisor observes
the Linux zombie status without reaping first, so `process-exit` reaches the
recorder while `/proc/<pid>` still identifies the supervised JVM. Wrapper spawn
and wait failures send the same abort command. Do not send a short-lived marker
client's PID or attach after Java has already started. Later lifecycle commands
use `mark-initial-linux` with the same dimension arguments as the marker-only
preflight. The server samples procfs on fixed one-second deadlines from the same
monotonic origin, gives a pending terminal marker priority at a sample boundary,
rejects samples more than 5 milliseconds late, and removes partial memory output
on any invalid run.

## Tracy frame streams

Minecraft 26.1.2 bundles Mojang `jtracy` 1.0.37. Launching the game with its
built-in `--tracy` argument exposes the discontinuous `Client Tick` and
`Server Tick` frame sets. They surround every client loop and every integrated
server tick respectively, so one continuous trace covers the full measurement
window without an in-game key binding.

Start a focus-free capture after launching the game with `--tracy`:

```console
cargo run --locked -p ravel-cli -- \
  performance capture-tracy SECONDS evidence/private/m9/capture.tracy
```

The duration begins when Tracy attaches. It must cover startup allowance plus
the profile's complete warm-up, measurement, and cool-down phases. The command
refuses to overwrite an existing trace, rejects an empty result, and rejects
workspace-local output outside a repository ignore boundary.

Run `cargo make ravel:tracy` once to build Tracy 0.13.1's capture and export
tools with `tracy-csvexport-frames.patch`. mise adds their output directory,
`target/tools/tracy/bin/`, to PATH. Export and validate the two required
named frame sets with:

```console
cargo run --locked -p ravel-cli -- \
  performance export-tracy-frames \
  evidence/private/m9/capture.tracy \
  evidence/private/m9/tracy-frames.csv
```

The command creates the destination exclusively, removes partial or invalid
output, and supplies Tracy's required `--filter= Tick` argument form. It never
uses desktop input or activates the game window.

The pinned CSV contract is:

```text
frame_set,frame_index,start_ns,end_ns,duration_ns
```

`ravel-cli` rejects missing frame sets, unknown names, non-contiguous indices,
out-of-order starts, and any duration that differs from `end_ns - start_ns`.
Recheck an existing export with:

```console
cargo run --locked -p ravel-cli -- \
  performance verify-tracy-frames evidence/private/m9/tracy-frames.csv
```

The raw Tracy trace and CSV remain inside the ignored evidence boundary.
