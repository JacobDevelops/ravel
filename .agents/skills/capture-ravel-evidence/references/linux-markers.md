# Linux lifecycle marker alignment

## Initial lifecycle-marker protocol version 2

The raw marker CSV is:

```text
event_index,elapsed_ns,kind,route_step,dimension
```

It contains exactly 17 ordered markers:

1. `process-start`
2. `game-ready`
3. `world-load-start`
4. `world-ready` in `minecraft:overworld`
5. `warm-up-start`
6. `measurement-start`
7. `route-step-start` for `base-observation`
8. `route-step-start` for `overworld-traverse`
9. `route-step-start` for `outbound-dimension-change`
10. `dimension-change-start` to `minecraft:the_nether`
11. `dimension-ready` in `minecraft:the_nether`
12. `route-step-start` for `dimension-traverse`
13. `route-step-start` for `return-and-base-observation`
14. `dimension-change-start` to `minecraft:overworld`
15. `dimension-ready` in `minecraft:overworld`
16. `measurement-end`
17. `process-exit`

Validate it with:

```console
cargo run --locked -p ravel-cli -- \
  performance verify-initial-markers \
  evidence/private/MILESTONE/markers.csv
```

The validator pins route order, route dimensions, the 300-second warm-up, the
900-second measurement, the route offsets, the 5 millisecond alignment
tolerance, and at least 120 seconds of cool-down.

## Focus-free recorder

Start the owner-only Unix-socket recorder before Prism:

```console
install -d -m 700 evidence/private/MILESTONE
cargo run --locked -p ravel-cli -- \
  performance record-initial-markers \
  evidence/private/MILESTONE/markers.sock \
  evidence/private/MILESTONE/markers.csv
```

Send an observed event without global mouse or keyboard input:

```console
cargo run --locked -p ravel-cli -- \
  performance mark-initial \
  evidence/private/MILESTONE/markers.sock process-start
```

Send `world-ready`, `dimension-change-start`, and `dimension-ready` with their
Minecraft dimension as the final argument. After `warm-up-start`, the server
emits measurement start, the five fixed route boundaries, and measurement end
from its own schedule. A wake delay above 5 milliseconds invalidates the run.
The server reserves the final 5 milliseconds before each scheduled boundary
from accepting requests and bounds each request read to 1 millisecond, so a
slow local client cannot consume the scheduling allowance. The final CSV is
created only after all 17 events validate.

The recorder refuses to bind outside an owner-only directory, sets the live
socket to mode `0600`, and only removes the exact socket device/inode it created
on normal return. If it is killed without unwinding and the socket remains,
first prove that no recorder process owns it; then remove that exact socket path
before retrying. Never recursively delete an evidence directory.

Final marker and Linux memory CSVs must also be created with mode `0600`;
relying on the operator's umask can expose evidence as `0664`. Failure cleanup
must compare the destination's device and inode with the file the recorder
created before unlinking it, so a replacement path is never deleted.

The socket protocol is one newline-terminated `kind,dimension` line. The server
must stop reading at that newline instead of waiting for client EOF, because
CLI clients may keep the write side open while waiting for the response. Bad
wire requests should receive `error:...` and leave recorder state intact; a
semantic marker/order error invalidates the run.

After both output streams verify, the files are authoritative and the final
`complete` socket acknowledgement is best-effort. A client that disconnects
after sending the final event must not turn the recorder into a failed run or
remove already-verified private evidence.

## Clock ownership

Do not join timestamps merely because every file reports monotonic
nanoseconds. Two processes can use different zero points even on the same host.

- One long-running recorder must own the monotonic origin for lifecycle
  markers and Linux interval samples.
- Marker commands send event data to that recorder; the recorder timestamps
  receipt. They do not create their own `Instant`.
- Emit logically simultaneous events as one recorder operation so equal
  timestamps remain intentional and ordered by `event_index`.
- Tracy frame timestamps begin at Tracy initialization. Do not align them to
  the recorder by numeric equality or by assuming process start is Tracy zero.
  Require a measured transform with a documented error bound before joining.

Until the recorder and Tracy transform are proven, the marker validator is a
contract preflight, not evidence that independently captured streams align. Do
not join the marker-only recorder to the standalone Linux memory command.

For Linux evidence, use the combined recorder:

```console
cargo run --locked -p ravel-cli -- \
  performance record-initial-linux \
  evidence/private/MILESTONE/markers.sock \
  evidence/private/MILESTONE/markers.csv \
  evidence/private/MILESTONE/linux-memory.csv
```

Configure the Prism wrapper only for an active performance capture after the
recorder has created the live socket. Leave the wrapper empty for launch smoke
tests, provisioning, and ordinary play: the supervisor intentionally fails the
Java launch when it cannot connect to the recorder, so a stale capture wrapper
can look like a Prism or Minecraft startup failure.

Set Prism's wrapper command to:

```console
/absolute/path/to/ravel-cli performance supervise-initial-linux \
  /absolute/private/path/markers.sock --
```

Prism appends Java and its arguments. The supervisor starts a marker child;
that child sends its own PID immediately before `exec` replaces the same PID
with the pinned Java runtime. The supervising parent sends process exit only
for a successful Java process and sends `abort` after a nonzero or crashed
process. It uses blocking Linux `waitid` with `WNOWAIT` rather than polling
`/proc` during the benchmark, sends the terminal marker while the JVM remains
an unreaped zombie, then reaps and returns the original exit or signal status to
Prism. The recorder prioritizes a pending terminal marker at a memory deadline
and never samples procfs after accepting `process-exit`. Wrapper resolution,
spawn, observation, and reap failures send a best-effort `abort`, so partial
outputs are removed rather than left waiting.
Tests for this ordering must inspect the unreaped PID before the fake recorder
sends `complete`; checking after acknowledgement races with the supervisor's
intentional reap.
Likewise, process-start tests must inspect any pre-`exec` side effect before
sending `ok`. A helper that acknowledges while accepting the request releases
the child before the assertion and turns the test into a scheduler-dependent
race. Drop the accepted stream after acknowledging it: the client reads until
EOF, so retaining that stream while waiting for the next connection deadlocks
both sides.
Do not rely on Prism's post-exit command because Prism can skip it when the
wrapped process fails. A short-lived marker-client PID or attachment after Java
startup invalidates startup timing. The combined recorder alone owns both marker
and one-second procfs timestamps and removes partial memory output when the run
fails.

## Phase boundaries

- `process-start` through the instant before `world-load-start`: `startup`
- `world-load-start` through the instant before `warm-up-start`: `world-load`
- `warm-up-start` through the instant before `measurement-start`: `warm-up`
- measurement samples: the active route step's `steady-state` or `transient`
- `measurement-end` through the instant before `process-exit`: `cool-down`

The beginning event owns an exact boundary timestamp. A sample before process
start or at/after process exit is invalid.
