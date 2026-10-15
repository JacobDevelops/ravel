---
name: capture-ravel-evidence
description: Prepare, run, recover, and validate privacy-safe Ravel launch and performance evidence. Use for Linux Minecraft 26.1.2 or ATM11 launch proof through Prism Launcher, later Ravel performance work, Mojang jtracy/Tracy frame and integrated-server tick capture, focus-free desktop operation, isolated Microsoft authentication recovery, raw-evidence handling, or recurring capture failures that should become reusable project tooling.
---

# Capture Ravel Evidence

Run Ravel benchmark evidence without taking global mouse or keyboard focus.
Fail closed when a source, phase, identity, or artifact cannot be proven.

## Start

1. Locate the Ravel root with `jj root`.
2. Read `AGENTS.md`, `ROADMAP.md`, the selected performance profile, and
   `tests/performance/collectors/README.md`.
3. Inspect the current `jj st`; preserve unrelated work.
4. Keep raw traces, launcher state, worlds, logs, and account material under an
   ignored local/private boundary. Admit only sanitized results and hashes.

For an already-completed official ATM11 M0 launch, generate the private
launch-proof summary without exposing launcher state:

```console
.agents/skills/capture-ravel-evidence/scripts/summarize-prism-launch.sh \
  official-atm11 ISOLATED_INSTANCE_DIR \
  evidence/private/m0/linux-w7-official-atm11-launch.json
```

The preflight fails unless the pinned components and fixture hashes match, the
log proves title screen, integrated world, save, and clean exit, exactly one
saved proof world exists, and the output directory is owner-only.

## Protect the operator

- Never use global mouse or keyboard automation.
- Prefer direct process launch, CLI collectors, background processes, and
  process/window identifiers.
- Never activate a real desktop window without first obtaining a quiet window
  from the user.
- Treat virtual displays as setup/recovery aids only. Do not benchmark on Xvfb.
- Never print account JSON, tokens, profile names, authorization URLs, callback
  URLs, or world contents.

Read [references/recovery.md](references/recovery.md) before repairing Prism
authentication, URI callbacks, blocked CurseForge downloads, or display
failures.

## Prove a client change without the operator's screen

`scripts/prism-headless-launch.sh` runs the pinned client through a full Ravel
proof on a virtual display it creates itself:

```console
.agents/skills/capture-ravel-evidence/scripts/prism-headless-launch.sh \
  --phase-mtime --phase-reopen \
  :99 PRISM_APPIMAGE HEADLESS_DATA_DIR INSTANCE_ID \
  evidence/private/headless-1.21.1/RUN_ID
```

It refuses to start against a display that already exists, against a launcher
root holding any account that is not `Offline`, and against an instance whose
`options.txt` leaves a sound category above `0.0`. Those three refusals are what
keep a run off the operator's screen, off their Microsoft account, and silent;
none of them is a procedure anyone has to remember. It backs up `instance.cfg`
and `options.txt` and restores both on every exit path, writes screenshots, the
game's own stdout and a verdict list into the private run directory, and writes
`target/headless-launch-verdict.txt` only when every assertion held.

The display renders through llvmpipe, so no number the run produces is timing
evidence. `--stop-after` halts at `title`, `world`, `commands` or `close` while
the click coordinates for a screen are being measured, and `--keep-display`
leaves the display up so a capture can be taken against it.

## Capture exact Minecraft timing

1. Launch the pinned Minecraft instance with its built-in `--tracy` argument.
2. Start the Ravel capture wrapper. Tracy attaches over localhost and does not
   need desktop input:

   ```console
   cargo run --locked -p ravel-cli -- \
     performance capture-tracy SECONDS \
     evidence/private/MILESTONE/capture.tracy
   ```

   Include startup allowance plus the full 300-second warm-up, 900-second
   measurement, and 120-second cool-down. The minimum post-attachment phase
   total is 1,320 seconds.
3. Keep warm-up, measurement, and cool-down within the recorded trace.
4. Export through Ravel's wrapper:

   ```console
   cargo run --locked -p ravel-cli -- \
     performance export-tracy-frames \
     evidence/private/MILESTONE/capture.tracy \
     evidence/private/MILESTONE/tracy-frames.csv
   ```

5. Recheck an existing CSV with:

   ```console
   cargo run --locked -p ravel-cli -- \
     performance verify-tracy-frames \
     evidence/private/MILESTONE/tracy-frames.csv
   ```

6. Require both exact streams: `Client Tick` for frame time and `Server Tick`
   for integrated-server tick time. Reject missing streams or parser drift.

Read [references/tracy.md](references/tracy.md) when changing the exporter,
capture duration, filter, frame mapping, or pinned versions.

## Normalize Linux memory evidence

Use the versioned Linux mapping in the selected Ravel performance profile.
Read [references/linux-memory.md](references/linux-memory.md) before changing
RSS, system-used-memory, pressure, swap, or phase normalization.

## Align lifecycle markers

Keep lifecycle events and interval samples on one recorder-owned monotonic
clock. Read [references/linux-markers.md](references/linux-markers.md) before
recording, validating, or joining marker, Linux memory, or Tracy timestamps.

## Validate before admission

- Record an exact command with a private raw log and schema-ready metadata:

  ```console
  .agents/skills/capture-ravel-evidence/scripts/record-command-evidence.sh \
    'PUBLIC COMMAND LABEL' PRIVATE_LOG PRIVATE_RECORD -- COMMAND ARG...
  ```

  Keep the public label path- and credential-free. The helper preserves the
  actual exit status and refuses non-owner-only or non-private output paths.
- Verify the selected profile, route markers, phase boundaries, collector
  versions, sample counts, hashes, and raw/private paths.
- Compare requested duration with trace span and expected sample volume.
- Reject partial, mismatched, unknown, or estimated data where the profile
  requires exact values.
- Run the narrow tests during iteration and `cargo make ravel:verify`
  before claiming a repository change is done.
- Obtain an independent code review after nontrivial changes.

## Promote every recovery

Whenever an obstacle is eventually overcome:

1. Add the smallest regression test that reproduces the failure.
2. Put fragile mechanics in a deterministic Ravel CLI command or preflight.
3. Add the concise diagnosis and recovery to the relevant reference file.
4. Remove any secret, machine-specific identity, or raw evidence from the
   reusable artifact.
5. Run the skill validator:

   ```console
   .agents/skills/capture-ravel-evidence/scripts/validate.sh
   ```

Do not leave a resolved issue only in chat history.
