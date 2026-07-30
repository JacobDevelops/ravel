# M1 architecture measurement gate

Status: M0-W8 input to ADG-1 through ADG-5. This document defines comparable
Linux evidence and failure actions without selecting an architecture.

## Common trial

M1-W1 must turn this contract into one shared harness and machine-readable
result shape before candidate measurements begin. Every candidate uses:

- native `linux-x86_64` on hardware profile `linux-reference-v1`;
- the pinned Minecraft 26.1.2, Java 25.0.1, and Prism 11.0.3 inputs;
- the same no-op callback payload and lifecycle boundary;
- one measured process trial, with callback distributions collected inside
  that process rather than repeated game launches;
- identical warm-up, sampling, clock, and process-ownership rules;
- a recorded candidate revision and command hash, where the revision records both
  a commit that existed when the trial ran and the change that holds the result,
  and the measured tree is that commit plus that change's diff.

That last point replaces the "clean working tree" a git-shaped contract would ask
for. This is a jj repository: the working copy is itself a change with content, so
there is no state in which a trial runs against a tree identical to a recorded
revision. Evidence written by a run cannot exist before the run either. So a
result records a commit that existed when the trial ran, records the change that
holds it, records that the measured tree differed from that commit, and relies on
the reviewer seeing the difference — it is the diff of the change the result
belongs to. Only the change is recomputed; the commit is a note about what ran.

An invalid trial is recorded with its reason, corrected, and replaced. It is
never averaged into the result or silently discarded. M0 launch evidence is
not a performance baseline and supplies no numeric pass budget.

## Required result fields

Each viable candidate reports these fields with units, collector identity,
quality (`exact`, `estimated`, or `unavailable`), and the raw-artifact hash:

| Area | Required evidence |
| --- | --- |
| Identity | ADG, candidate ID, revision, target, Java and Minecraft versions, command hash |
| Memory | bootstrap peak RSS, post-bootstrap retained RSS, retained class count |
| Startup | process-start to game-ready milliseconds and candidate-attributed delta |
| Calls | invocation count plus p50, p95, and p99 callback nanoseconds |
| Boundary traffic | bytes transferred per call and total bytes |
| Allocation | allocations and allocated bytes per call where the runtime exposes them |
| Packaging | installed bytes, artifact count, required native/runtime components |
| Failure | invalid-input, unavailable-runtime, callback-failure, crash, and timeout outcomes |
| Safety | maintained unsafe-code scan and audited boundary dependency list |
| Operability | diagnostics, debug workflow, crash attribution, and Prism launch result |

ADG-specific additions remain required:

- ADG-2 records isolation, ABI evolution, and mod-to-mod call behaviour.
- ADG-3 records binding generation time, output hash, drift failure, and
  produced binding size.
- ADG-4 records duplicated state bytes and boundary traffic for the
  representative object flow.
- ADG-5 records stock-Prism launch proof or the size and ownership of a minimal
  required patch. Authentication data is never captured.

## Failure actions

- Reject a candidate that requires maintained unsafe Rust, another Java mod
  loader, unredacted credentials, or untracked restricted artifacts.
- Reject a candidate that cannot launch the pinned official game, preserve the
  required lifecycle boundary, or fail closed for invalid input and crashes.
- Mark evidence incomplete when a required metric, unit, collector hash,
  candidate revision, or failure scenario is missing. Do not substitute zero.
- Record `unavailable` only when the platform cannot expose a metric and the
  ADR explains the resulting comparison limit. Candidate implementation
  failure is not metric unavailability.
- Treat performance differences as comparative evidence, not an invented M0
  threshold. The ADR must explain each material regression and why the selected
  trade-off can plausibly meet M9.
- Keep the ADG open when evidence is ambiguous or incomparable. Do not select a
  mechanism by convenience.

M2-W8 measures the selected no-op loader and establishes the first numeric
regression budget. That later budget does not retroactively turn M0 into a
benchmark milestone.
