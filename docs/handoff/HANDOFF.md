# Ravel handoff

Context for the next session, written when work stopped after WP5 slice 4. It records where things
are, what comes next and what bit the last session. It is not a contract: `AGENTS.md` holds the
repository rules, `SPEC.md` the required behaviour, `ROADMAP.md` the work-package status and evidence
gates, and `registry/notes/sevtech/sevtech-plan.md` the SevTech design. Trust a fresh gate run over any
number written here.

## Standing goals

- Make Ravel the fastest, lowest-resource loader, proven on SevTech: Ages 3.2.3 (Minecraft 1.12.2)
  running on the pinned JDK 27 through native Rust counterparts, LWJGL3 and a replacement renderer.
  Java 8 runs comparators only, never Ravel. Minecraft 1.21.1 and 26.1.2 stay supported.
- Keep code quality high: the `numen refine` strict score is past its 85 target and the aim is to
  keep raising it with root-cause fixes, not dispositions.
- Prefer maintained crates over hand-rolled JVM interop where they are better.
- Work autonomously through the work packages in dependency order, committing locally with a green
  gate. Never push, open a pull request or move bookmarks.

## Where things stand

| Area | State |
| --- | --- |
| WP0–WP3 | As recorded in `ROADMAP.md`; WP0 still owes a replayable legacy graph producer. |
| WP4 | Live: official integrated and connected-dedicated acceptance pass with all 29 hooks, and 1.12.2 is in the workspace test matrix. |
| WP5 | Slices 1–4 done; slices 5–7 open. Vanilla still renders and no speedup or memory saving is claimed. |
| WP6, WP8, WP9 | Not started beyond what `ROADMAP.md` records. |
| WP7 | Cutover work landed; integrated acceptance evidence pending. |
| Code quality | `numen refine` strict 88.5, overall 89.0; round-11 review left 109 findings open. |
| Rights ledger | 31 permitted, 38 conditional, 18 prohibited, 172 unresolved. |

WP5 slice results live beside the plan: `vanilla-baseline.md` (matched JDK 27 and Java 8 vanilla
trials on the RTX 5090), the renderer lifecycle observations in `docs/runbooks/legacy-launch.md`,
`section-snapshots.md` and `mesh-workers.md`. In short:

1. Slice 1 measures the vanilla renderer under Ravel and under Java 8 with the same seed, scenes and
   settings. In the loaded-chunk scene Ravel frames were about 16% faster at p50 and 8–10% at p95
   and p99, with about 150 MiB more RSS; this is still the vanilla renderer, so it is a baseline,
   not a result.
2. Slice 2 pins twelve renderer lifecycle hooks by exact Feather name and descriptor, and keeps a
   renderer generation that reloads, world replacement and shutdown advance.
3. Slice 3 adds `ravel-renderer` with bounded immutable 18³ section snapshots (one-cell border,
   deduplicated state keys, packed light, generation stamp) captured at the compile seam.
4. Slice 4 meshes full opaque cubes and computes face visibility on two Rust workers behind an
   eight-job, byte-reserved queue with generation cancellation and drain. Meshes are checked and
   measured, then discarded.

## Next steps

### WP5 slice 5: Java-owned GL upload

Hand the generation-qualified CPU meshes to Java and draw them.

- Add bounded, batched buffer transfer across JNI with explicit ownership and acknowledgement,
  without weakening transactional completion or generation cancellation. Java alone owns the GL
  context, buffers and draw calls.
- Refuse incomplete or unsupported results rather than drawing missing geometry. Keep vanilla draw
  order and state, select GL by the verified LWJGL3 context's capability rather than assuming the
  development GPU, and clean up on reload, unload and close.
- Slice 4's state classification is geometry-only. Settle model, texture and ambient-occlusion
  fidelity for whatever slice 5 draws; until a pixel comparison against vanilla passes on the solid
  scene, keep Ravel drawing behind an explicit, off-by-default selection.
- Measure copy, upload and buffer residency and frame percentiles against slice 1 with three locked
  trials on the physical GPU plus one Xvfb functional run.

Slice 6 covers translucency, animated and connected models and reload correctness; slice 7 is the
default cutover with locked acceptance trials and ablations. WP6, WP8 and WP9 follow per `ROADMAP.md`.

### Refinement round 12

The open findings sit in the local `.numen-refine/state.json`, which is not tracked. Regenerate them
with a fresh scan and holistic review if that state is gone. These open findings are real defects
rather than style, and come first:

- Tags from different mods are not merged at launch. `ravel_loader::open_resources` treats each tag
  file as an ordinary resource where the later mod wins, although the SDK and the content guide
  promise merging; `TagSet::merged` is reached only from `ravel-pack` validation.
- The player build fingerprint hashes the literal toolchain channel, so a nightly update does not
  invalidate cached player builds. Hash the resolved `rustc -vV` output instead.
- rvl-120's `bounded_count` caps a length prefix at `i32::MAX`, so an oversized field produces a
  frame whose length disagrees with its body.
- rvl-002 and rvl-321 answer malformed service requests with errors, which fails the providing mod
  instead of refusing the request inside a normal reply.
- The xtask fixture descriptor admits lowercase-only SHA-256 text, discards the parsed digest and
  compares case-insensitively.
- Optional mods: a dependency's `order` defaults to "after" while the manifest docs say unordered,
  an optional dependency installed outside its range blocks resolution, and a broken settings layer
  stops every mod without naming one.
- `docs/guide/limits.md` says world shaping produces output, but no host path reads it.
- Doc drift: the `broadcast` comment still promises silent wrong-side completion, the first-mod guide
  teaches a path `ravel-sdk` dependency that player builds cannot install, and its
  `ModError::Refused` prose predates the sentence-only error.

The rest are design and consistency findings. Recurring themes:

- Rvl-120 and rvl-210 copy `read_shipped`.
- The twelve per-counterpart regenerate tests have drifted apart.
- Five loader subsystems copy the declaration-collection loop.
- Integrated-server settings are routed by a display-label suffix.
- Typed values are turned back into strings at crate edges.

## Working method

Parallel lanes were the effective shape: one jj workspace per lane on a shared base, each lane owning
a crate set and migrating every caller of an API it changes, then a single integrator stacking the
lanes linearly, resolving conflicts in the change where they first appear, regenerating digests and
running the gate. The parallel-workspace rules in `AGENTS.md` came from failures in that process.
Lane ownership that kept conflicts small:

- **Host:** bootstrap runtime, host runtime, composed host, Minecraft adapter, `jvm/`.
- **Loader:** the loader, SDK and support crates, examples, test mods and conformance tests.
- **Registry:** `registry/mods/`.
- **Tooling:** xtask, the generator, `ravel-build`, docs, root manifests and version data.

The refine loop:

1. Resolve each fixed or dispositioned finding with `numen refine resolve <key> --status ...`.
2. Rescan with `numen refine scan --force-rescan`.
3. Run `numen refine review prepare --holistic --force-review-rerun` over all twenty dimensions.
4. Launch one independent reviewer per prompt, without telling it the score target.
5. Merge their JSON and import it with `numen refine review import --attested-external --packet`
   and the blind packet.

Resolve, scan, review and import all require attestation text. Import fails on any issue that cites
an out-of-scope path (for example `jvm/build/` files), so drop those from `related_files` before
importing. Every assessment needs non-empty `strengths`, `score_rationale`, `dimension_character`
and `issues_preventing_higher_score`. The local state can collect duplicate `note` keys after
resolves; rewrite it through a JSON parser before the next command.

## Known hazards

- Run `cargo make ravel:legacy-agent-build` before `ravel:legacy-agent-live` in any checkout that did
  not build the current legacy artifacts; a stale agent jar fails as "not marked as a generated
  legacy adapter".
- `ravel:verify-stack` rejects an undescribed non-empty working copy, so describe the change before
  the final gate.
- Coverage totals are unreliable across checkouts. Cargo runs through a host-wide compile cache that
  is shared between workspaces, and coverage reports include files from other checkouts even after
  `target/llvm-cov-target` is removed; one tip measured 87.7% in the main checkout and 93% in a
  lane. Make the coverage build checkout-local before trusting a total; the `AGENTS.md` sentence
  advising removal of `target/llvm-cov-target` does not fix it.
- The toolchain tracks the moving `nightly` channel by request, so a new nightly can turn the gate red
  without a code change; fix new lints at the cause rather than re-pinning a date.
- Registry wording must stay inside the frozen lexicon: a `src/tests/` directory and the word
  "connectivity" both failed the registry checks.

## Durable decisions and evidence boundaries

- SevTech parity is native: Rust counterparts, not hosted Forge jars. Feather mappings are canonical
  only after pinned-artifact coverage. LWJGL3 replaces LWJGL2, and the narrator and JNA stay
  unsupported on 1.12.2.
- Native access stays limited to the named `dev.ravel.agent` module and the four LWJGL modules.
- Registry counterparts, examples and test mods depend on `ravel-sdk` alone, dev-dependencies
  included; cross-counterpart duplication cannot move into a shared crate, but copies must stay
  identical.
- `Composition` stays in `ravel-bootstrap-runtime`, imported directly. The rvl-146 singleton stays
  because removing it changes the M6-bound counts.
- Never edit `evidence/**`, `registry/proposals/**`, milestone manifests or the M5-bound registry
  checks. The census stays 40/27/95/37 and the M6 counts 550/432/74/44. Package digests change only
  through `cargo run -q -p ravel-xtask -- package digests <dir>` and its `--check`.
- Optional mod callback failures stay attributed diagnostics, never Java exceptions in vanilla
  ticks; every remote packet refusal drops without failing its owner.
- Legacy resource bytes are captured on the owning runtime thread before vanilla workers open
  streams. The global palette sizes from the highest packed state ID, since block ID 512 needs
  sixteen-bit state IDs. Legacy recipe sources are immutable for a launched composition.
- Renderer work keeps Minecraft authoritative: Rust owns bounded snapshots and mesh jobs, Java owns
  game access and GL, and no state crosses into the SDK. Performance claims compare equal scenes,
  settings and hardware, and keep Xvfb runs functional only.

## Research pointers

- `registry/notes/sevtech/sevtech-plan.md`: work-package design and dependencies.
- `registry/notes/sevtech/sevtech-wp0-report.md` and `sevtech-rights-report.md`: input provenance,
  rights research and disposition history.
- `registry/notes/sevtech/baseline.md`: the original optimized SevTech comparator on Java 8.
- `registry/notes/jvm-crate-audit-2026-10-15.md`: dated JVM library research; not current status.
