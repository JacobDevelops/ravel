# Linux memory normalization

## Sources

- Process RSS: read `Rss` from `/proc/<pid>/smaps_rollup`. Do not substitute
  `VmRSS` from `/proc/<pid>/status`; the kernel documents it as approximate,
  while the Ravel profile requires exact RSS.
- System-used memory: calculate `(MemTotal - MemAvailable) * 1024` from
  `/proc/meminfo` and label it estimated.
- Swap: retain cumulative `pswpin` and `pswpout` page counters from
  `/proc/vmstat`, multiplied by the kernel base page size.
- Pressure: compare consecutive cumulative `some total` and `full total`
  counters from `/proc/pressure/memory`.

## Pressure levels

- `0` normal: neither cumulative total increased.
- `1` warning: `some` increased and `full` did not.
- `2` critical: `full` increased.

Reject decreasing cumulative PSI or swap counters. Do not turn a missing,
malformed, or unavailable value into zero.

## Phase coverage

The initial profile contains 300 seconds of warm-up, 900 seconds of
measurement, and 120 seconds of cool-down. Preserve `cool-down` as an explicit
sample phase after `measurement-end`; do not relabel it as steady-state or
drop it before clean process exit.

## Focus-free capture

Capture the PID-attached raw stream with:

```console
cargo run --locked -p ravel-cli -- \
  performance capture-linux-memory PID SECONDS \
  evidence/private/MILESTONE/linux-memory.csv
```

The command uses fixed one-second monotonic deadlines, creates the destination
exclusively, and removes partial output on capture failure. Use `self` instead
of a PID only for a collector preflight. It pins `/proc/<pid>/stat` field 22
before sampling and checks it around every snapshot; a changed start time means
the numeric PID was reused and invalidates the capture. The CSV still needs
lifecycle/phase markers and Tracy streams before it can become a complete
collector document.

## Recovery checks

- Missing exact RSS: confirm the collector can read
  `/proc/<pid>/smaps_rollup`; do not fall back silently.
- Impossible system memory: reject `MemAvailable > MemTotal`.
- Byte overflow or unknown page size: reject the sample and repair the source
  or page-size preflight.
- Counter regression: invalidate the run because the process/system source or
  collector continuity changed.
- Process identity changed under the same numeric PID: invalidate and restart
  the repetition; never continue RSS collection from the replacement process.
- Changed source, cadence, definition, threshold, or calculation: bump the
  profile version and update every pin together—the profile, collector and
  result schemas, dry-run fixtures, baseline task checks, and capture runbook.
  Search for the stale version before admitting evidence.
