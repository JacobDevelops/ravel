# M2 evidence

M2 is not closed. This directory holds the per-work-package records M2 produces
as it goes; the milestone manifest that admits them arrives with the M2 gate.

| Record | Work package | What it shows |
| --- | --- | --- |
| [`noop-pipeline.json`](noop-pipeline.json) | M2-W6 | The no-op package validated, resolved, and driven through every `SPEC.md` §14 state, in order, on each side it declares. |
| [`pre-world-gate.json`](pre-world-gate.json) | M2-W7 | Seven invalid package sets refused before a world could open, each reported as its own `SPEC.md` §9.2 outcome, and what the redaction policy took out of the one message Ravel did not write. |

Both records are generated rather than written. `tests/conformance/tests/noop_pipeline.rs`
and `tests/conformance/tests/pre_world_gate.rs` rerun the pipeline and the gate
and fail if either file differs by a byte, so a record cannot describe a loader
that no longer behaves that way. Regenerate one by rerunning its test and
rewriting the file from the run it reports.

The records name files, mods, states, and digests. They name no filesystem path,
no credential, and no local directory, which the same tests assert. The raw text
the redaction policy was given is never recorded — only what it left.

The mod ran as a safe Rust value the loader owned in process. ADG-2 selects the
mod execution and distribution model and is deferred to M3, so no record here may
be read as evidence for a binary format: nothing loaded a library, started a guest
runtime, spawned a process, or exported a foreign symbol.
