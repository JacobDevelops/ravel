# M2 evidence

M2 built the pre-world loader and proved it on one no-op package.
[`manifest.json`](manifest.json) is the admission artifact; the records below are
what it admits, and `devenv tasks run ravel:verify-milestone:m2` reads both.

| Record | Work package | What it shows |
| --- | --- | --- |
| [`noop-pipeline.json`](noop-pipeline.json) | M2-W6 | The no-op package validated, resolved, and driven through every `SPEC.md` §14 state, in order, on each side it declares. |
| [`pre-world-gate.json`](pre-world-gate.json) | M2-W7 | Seven invalid package sets refused before a world could open, each reported as its own `SPEC.md` §9.2 outcome, and what the redaction policy took out of the one message Ravel did not write. |
| [`loader-overhead.json`](loader-overhead.json) | M2-W8 | What the loader costs on Linux for that package, what a linked pack-sized set costs to build, and the numeric budget M3 regresses against. |
| [`prism-launch.json`](prism-launch.json) | M2-W5, M2-W8 | The official pinned client launched from stock Prism with Ravel's agent attached, rewriting its target classes inside the real game process, with no other mod loader installed. |

The first two records are generated rather than written.
`tests/conformance/tests/noop_pipeline.rs` and
`tests/conformance/tests/pre_world_gate.rs` rerun the pipeline and the gate and
fail if either file differs by a byte, so a record cannot describe a loader that
no longer behaves that way. Regenerate one by rerunning its test and rewriting
the file from the run it reports.

`loader-overhead.json` comes from `devenv tasks run ravel:measure-loader-overhead`
and is a measurement rather than a contract: rerunning it moves the numbers
within the same order, which is why the budget it sets carries a factor of two.
They hold on the `linux-reference-v1` hardware profile and nowhere else.

`prism-launch.json` is the one record that needs the game. It is written by
`cargo run -p ravel-spike-adg-5-prism --bin m2-launch-record` from a launcher log
an operator captured outside this repository, and the log is deleted once its
digest is taken. That digest is the one hash here nobody can recompute, which is
the same trade M0 and M1 made for their launches.

The records name files, mods, states, and digests. They name no filesystem path,
no credential, and no local directory. The conformance tests assert it over the
two generated records, and `ravel:verify-milestone:m2` asserts it over the
manifest and over all four records. That gate is the only check the overhead
record gets; the launch record also has a test in `ravel:test`, in its own
generator, holding the file to the prose and the attestation that generator
would write today, so a conclusion edited into the record and not into the runner
fails. The raw text the redaction policy was given is never recorded — only what
it left.

The mod ran as a safe Rust value the loader owned in process. ADG-2 selects the
mod execution and distribution model and is deferred to M3, so no record here may
be read as evidence for a binary format: nothing loaded a library, started a guest
runtime, spawned a process, or exported a foreign symbol.
`loader-overhead.json` measures a linked pack because a linked set is the only
shape available while ADG-2 is open, and says so in its own words. The one place
M2 does carry a shape assumption is `schemas/package.schema.json`, which still
requires a per-target artifact list and therefore presumes a per-target mod
binary. That is inherited rather than chosen — `ravel-package` implements
identity, integrity and compatibility only — and M3 reconciles the schema with
whatever ADG-2 selects.
