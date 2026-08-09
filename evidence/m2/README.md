# M2 evidence

M2 built the pre-world loader and proved it on one no-op package.
[`manifest.json`](manifest.json) is the admission artifact; the records below are
what it admits, and `devenv tasks run ravel:verify-milestone:m2` reads both.

| Record | Work package | What it shows |
| --- | --- | --- |
| [`noop-pipeline.json`](noop-pipeline.json) | M2-W6 | The no-op package validated, resolved, and driven through the eight `SPEC.md` §14 states a successful run reaches, in order, on each side it declares. `discovered` precedes the transcript and `failed` cannot appear in a run that succeeded, so eight is every state this run could hold. |
| [`pre-world-gate.json`](pre-world-gate.json) | M2-W7 | Six invalid package sets refused before a world could open, each reported as its own `SPEC.md` §9.2 outcome, and what the redaction policy took out of the one message Ravel did not write. |
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

[`review.md`](review.md) is M2's closure review, and it returns FAIL. The review
that originally closed M2 is not here and cannot be: `review.record` named a
title and a sha256 matching no file this repository tracks, so that review could
not be read or hashed by anyone who did not run it. Withdrawing the digest and
keeping its `pass` verdict left the worse state of the two — a pass attributed by
name to a review the same manifest said nobody could obtain — so the verdict is
withdrawn as well. A cold audit has since reviewed the gate independently;
`review.md` is its verdict, reproduced without softening, and
`review.result` is `fail`. The orphaned digest stays in the manifest's deviations
so the claim is not lost. Every digest the manifest binds is recomputable — which is narrower than every digest
in this directory, because `prism-launch.json` names three of its own that nobody can:
the deleted launcher log, the native library, and the agent jar. And a recomputable
digest is not the same as an unaffected measurement: `loader-overhead.json` and
`prism-launch.json` both record a trial run on a dirty working copy whose commit
no visible head reaches, so M2-W8's budget and M2-W5's launch attestation are
readable and tied to no inspectable tree. The manifest discloses both pins under
`unobtainableRevisions` and classifies the finding, and the scaffold refuses any
pin a record carries that is neither obtainable nor disclosed there. What closes
M2 now is an independent review of the repaired state that passes.

The audit's blocking finding was that `LifecycleMachine::open_world` had no
caller outside tests, which made M2-W7's "cannot open a world" a statement about
the loader's model rather than about the game process. ADG-7's world-open
crossing now consults it, so a set the loader refuses opens no world in a launch
either. The manifest's deviations record what each of the audit's eight findings
became.

The records name files, mods, states, and digests. They name no filesystem path,
no credential, and no local directory. The conformance tests assert it over the
two generated records, and `ravel:verify-milestone:m2` asserts it over the
manifest and over all four records. That gate is the only check the overhead
record gets; the launch record also has a test in `ravel:test`, in its own
generator, holding the file to the prose and the attestation that generator
would write today, so a conclusion edited into the record and not into the runner
fails. The raw text the redaction policy was given is never recorded — only what
it left.

The mod ran as a safe Rust value the loader owned in process. At M2, ADG-2 had
not yet selected the execution and distribution model, so no record here is
evidence for a binary format: nothing loaded a library, started a guest runtime,
spawned a process, or exported a foreign symbol.
`loader-overhead.json` measures a linked pack because a linked set is the only
shape available while ADG-2 is open, and says so in its own words. The M2 schema
originally carried a per-target artifact assumption. ADG-2 later selected source
distribution, and the 2026-08-08 schema-v1-in-place migration
closed that mismatch: packages now declare integrity-protected portable source
files and the old `targets` form is refused. The generated M2 records retain the
historical measurements while their live fixtures use the current contract.
