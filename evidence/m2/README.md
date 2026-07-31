# M2 evidence

M2 is not closed. This directory holds the per-work-package records M2 produces
as it goes; the milestone manifest that admits them arrives with the M2 gate.

| Record | Work package | What it shows |
| --- | --- | --- |
| [`noop-pipeline.json`](noop-pipeline.json) | M2-W6 | The no-op package validated, resolved, and driven through every `SPEC.md` §14 state, in order, on each side it declares. |

`noop-pipeline.json` is generated rather than written. `tests/conformance/tests/noop_pipeline.rs`
reruns the pipeline and fails if the file differs by a byte, so the record cannot
describe a loader that no longer behaves that way. Regenerate it by rerunning
that test and rewriting the file from the run it reports.

The record names files, mods, states, and digests. It names no filesystem path,
no credential, and no local directory, which the same test asserts.

The mod ran as a safe Rust value the loader owned in process. ADG-2 selects the
mod execution and distribution model and is deferred to M3, so no record here may
be read as evidence for a binary format: nothing loaded a library, started a guest
runtime, spawned a process, or exported a foreign symbol.
