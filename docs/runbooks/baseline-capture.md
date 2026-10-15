# Performance capture

This procedure captures one official-pack comparison and one Ravel-pack
acceptance under `tests/performance/profiles/initial.toml` for Linux M9. M0
admission uses the Linux launch proof in `docs/runbooks/prism-launch.md` and
does not run this performance procedure. Keep launcher tokens and identifying
paths out of the tracked tree.

## Preconditions

1. Use the native target machine and the pinned Prism-owned instance. Do not use
   a virtual machine or translate the process architecture.
2. Put worlds and raw collector output under an ignored
   `evidence/<milestone>/raw/` directory.
3. Record sanitized hardware, operating-system, Java, Prism, game, fixture,
   launch-argument, display, power, and thermal-control inputs. Hash the hardware
   profile and launch input.
4. Prepare a fresh world for each subject from the profile seed and the
   setup recipe. Hash the prepared world and recorded route
   without admitting either private artifact.
5. Verify the common active-content counts and the subject-specific vanilla or
   complete-pack counts before warm-up. A failed count invalidates the
   subject run.

The M9 acceptance machine is the native Linux x86_64 reference host at
3440 x 1440. Apple Silicon capture starts only after Linux M10 is complete and
uses a separate later profile.

## Validate the protocol

Run these checks before any capture:

```console
taplo format --check tests/performance/profiles/initial.toml
check-jsonschema --check-metaschema schemas/performance-collector.schema.json
check-jsonschema --check-metaschema schemas/performance-result.schema.json
check-jsonschema --schemafile schemas/performance-collector.schema.json \
  tests/performance/fixtures/collector-dry-run.json
check-jsonschema --schemafile schemas/performance-result.schema.json \
  tests/performance/fixtures/result-dry-run.json
```

Calculate SHA-256 hashes for the profile, prepared-world descriptor, route,
hardware profile, launch input, collector executable or script, and exact
collector command. Record the profile as ID `initial`, version `3`, protocol
`initial-v1`, plus its hash. A hash mismatch blocks capture.

## Capture one subject

1. Start every collector before launching Prism. Record its version, command
   hash, monotonic clock resolution, wall-clock start, and metric capabilities.
2. Emit `process-start`, `game-ready`, `world-load-start`, and `world-ready`
   markers. Startup is `game-ready - process-start`; world load is
   `world-ready - world-load-start`.
3. Begin the 300-second warm-up only after the content-count check and
   `world-ready`. Do not include warm-up frames in reported frame statistics.
4. Emit `measurement-start`, then follow the five route steps in sequence for
   exactly 900 seconds. Stay within 2 blocks, 5 degrees, and 10 percent of the
   recorded route position, heading, and speed.
5. Emit route-step and dimension-change markers. Continue collecting through
   the 120-second cool-down and clean process exit.
6. Normalize collector output against
   `schemas/performance-collector.schema.json`. Preserve exact and estimated
   quality labels; never substitute zero for unavailable data.
7. Invalidate the subject run for a missing required metric, event-order error,
   content or settings drift, route-tolerance breach, clock resolution above
   1 millisecond, collector loss, crash, or unreviewed deviation.

Capture the official pack once, then the Ravel pack once. Record an invalid
attempt under the result's `invalidAttempts` with its collector-output hash and
reasons, correct the cause, and retry that subject. Only one valid run per
subject enters the result.

## Aggregate and decide

Use the nearest-rank method for median, p95, and p99. Report average FPS,
frame-time distribution, integrated-server tick-time distribution, maximum
eligible steady-state RSS, peak transient RSS, peak system-used memory, memory
pressure, swap throughput, GPU allocation availability, and the best startup and
world-load durations.

The profile fixes the decisions:

- every eligible rolling 60-second FPS window must average at least 30 FPS;
- every eligible steady-state RSS sample must be at most 8 GiB;
- every loading or dimension-change transient RSS sample must be at most 10 GiB;
- no warning-or-critical memory-pressure interval may reach 30 seconds;
- no 30-second window may average at least 64 MiB/s combined swap-in and
  swap-out throughput.

M9 acceptance passes only when the Ravel run meets every criterion. Startup and
world-load have no fixed target: report each observed value and the Ravel
difference from the same-profile official result. The pair makes no run-to-run
reproducibility claim.

Validate the sanitized aggregate against
`schemas/performance-result.schema.json`. A Ravel acceptance result must identify
the official result hash and startup/world-load deltas.

## Admit evidence

Hash every raw collector output and the sanitized result. Scan the result for
credentials, launcher tokens, absolute paths, private-world data, and identifying
hardware details. Keep raw data outside the tracked tree.
Admit only the schema-valid sanitized result hash and small decision-relevant
summary through `docs/runbooks/evidence-admission.md`.

Official and Ravel comparisons must use the same profile hash. Any protocol
change requires review, a new profile version, and reruns of both results.
