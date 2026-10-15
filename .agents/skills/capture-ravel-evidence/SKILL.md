---
name: capture-ravel-evidence
description: Prepare and validate privacy-safe Ravel launch proof. Use for headless client checks, Prism launch evidence, isolated authentication recovery, display failures, or private launch-log handling.
---

# Capture Ravel Evidence

Run Ravel launch proof without taking global mouse or keyboard focus.
Fail closed when a source, phase, identity, or artifact cannot be proven.

## Start

1. Locate the Ravel root with `jj root`.
2. Read `AGENTS.md`, `ROADMAP.md`, and `docs/runbooks/prism-launch.md`.
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

New launch summaries require empty pre-launch, post-exit, and wrapper commands.
Historical supervisor-backed launch records remain evidence, not launch setup.

## Protect the operator

- Never use global mouse or keyboard automation.
- Prefer direct process launch, background processes, and process/window identifiers.
- Never activate a real desktop window without first obtaining a quiet window
  from the user.
- Use virtual displays for functional proof only; their timings are not native evidence.
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

## Validate before admission

- Record an exact command with a private raw log and schema-ready metadata:

  ```console
  .agents/skills/capture-ravel-evidence/scripts/record-command-evidence.sh \
    'PUBLIC COMMAND LABEL' PRIVATE_LOG PRIVATE_RECORD -- COMMAND ARG...
  ```

  Keep the public label path- and credential-free. The helper preserves the
  actual exit status and refuses non-owner-only or non-private output paths.
- Verify pinned inputs, lifecycle order, hashes, and raw/private paths.
- Run the narrow tests during iteration and `cargo make ravel:verify`
  before claiming a repository change is done.
- Obtain an independent code review after nontrivial changes.

## Promote every recovery

Whenever an obstacle is eventually overcome:

1. Add the smallest regression test that reproduces the failure.
2. Put fragile mechanics in a deterministic launch helper or preflight.
3. Add the concise diagnosis and recovery to the relevant reference file.
4. Remove any secret, machine-specific identity, or raw evidence from the
   reusable artifact.
5. Run the skill validator:

   ```console
   .agents/skills/capture-ravel-evidence/scripts/validate.sh
   ```

Do not leave a resolved issue only in chat history.
