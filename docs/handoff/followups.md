# Integration follow-ups

- Refresh package.json source digests for examples/_and test-mods/_ touched (xtask refresh_digests).
- docs/architecture/agentic-acceptance-harness.md:77-79,162-163 quotes host-api prose that LaneLoader trimmed; reconcile
  in docs lane.
- Conformance tests moved to tests/conformance/tests/integration/: update docs/architecture/dependency-rules.md:19,
  ROADMAP.md:768,1059,1060,1063, examples/* references; xtask conformance inventory scan must follow (check LaneXtask).
  Evidence records stay historical.
- More conformance path refs: tests/fixtures/public/packages/noop/README.md:21, evidence/m2/README.md:15-16,
  evidence/m3/README.md:345,378 (evidence READMEs: decide historical vs live), examples/* README/Cargo/source refs.
- LaneCli net only -79: second pass candidate (mostly moved code).
- Also: tests/fixtures/public/packages/client-only/README.md:19, tests/save-reload/README.md:4,
  examples/guide-snippets/src/lib.rs:621, test-mods/optional-service/src/lib.rs:254 (optional_services path). Source
  edits in examples/test-mods => refresh digests again.
- ROADMAP.md:401 verify_version_boundary -> tools/ravel-xtask/src/version_boundary.rs;
  docs/architecture/m3-hook-targets.md:36 verify_hook_inventory_document -> tools/ravel-xtask/src/inventory.rs.
- GATE BUG (pre-existing): mise.toml [env] RAVEL_MINECRAFT_VERSION=26.1.2 re-applies inside mise/mbx, so
  `RAVEL_MINECRAFT_VERSION=1.21.1 cargo nextest` in ravel:test likely tested 26.1.2 twice (Integrator2 saw ravel-build
  nested cargo pick 26.1.2). Fix: drop it from mise env; verify the matrix truly selects each version.
- User decisions: delete ravel-cli entirely; host build + fixture verbs -> xtask behind cargo make; delete all
  perf/resource tooling (tracy, linux memory, markers, measure-*, verify-budget, perf schemas/fixtures). Move
  ../ravel-registry in as-is (publish), remove public/private boundary doc + gate rule.
- mbx breaks cargo llvm-cov: gate coverage under mbx reports stale pre-split mappings (41.77%; baseline 73.8% likely
  same artifact). Real cargo, clean target: 87.60% lines, pass. mbx also has read-only outputs in default target. Gate
  must run cargo-llvm-cov/nextest without the mbx wrapper or mbx must support instrumentation.
- Integrator must prove: under mbx (no PATH bypass), `cargo make` with RAVEL_MINECRAFT_VERSION=1.21.1 generates
  MINECRAFT_VERSION "1.21.1" (adapter OUT_DIR bindings.rs) and ravel-build's nested cargo too; only ravel:coverage
  bypasses mbx. adapter build.rs:24 emits rerun-if-env-changed for the selection var.
- RegistryMove: added one M7 machineryAmendment in evidence/m5/inventory-summary.json (machinery-only hashes: verify.sh,
  verify-reads.sh, verify-primitives.sh, read-primitives.tsv; figures []); restored all 3 guards incl. m6 notes-rows.
- BLOCKED for user: panicking mod bypasses per-mod failure attribution (SPEC §9.1 requires it). Existing JNI guard wraps
  whole dispatch; fixing needs loader per-callback catch + attribution + resume/failed-transition redesign. Not fixed;
  ask user.
