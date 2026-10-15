# Ravel handoff: deslop, numen score, SevTech on 1.12.2

Repo: ravel (jj; public). Read AGENTS.md and docs/jj-stacked-changes.md before touching anything. Supporting reports
live beside this file in docs/handoff/.

## What Jacob asked for

1. **Deslop the repo hard.** The `numen refine` strict score has to go well past its 85 target, using the cleanest code
   available. Architecture and crate-layout redesign is allowed. Adopt `../loke-rs` conventions. `code-golf` and
   `code-refinement` skills are always on.
2. **Make the best and fastest mod loader with minimal resources.** The target modpack is now **SevTech: Ages 3.2.3**,
   which runs on **Minecraft 1.12.2**. Run it on the **latest Java (JDK 27, already pinned in mise.toml)**, never
   Java 8. Intrusion into Minecraft is fine: replace LWJGL2 with LWJGL3, replace the renderer, replace vanilla
   subsystems through the agent. Keep 1.21.1 and 26.1.2.
3. **Replace hand-rolled JVM interop code with existing crates** where others did it better.
4. **Make the open decisions on his behalf.** Decisions taken (all implemented unless noted):
   - A panic in mod code is caught per callback and attributed to its mod.
   - Commands and packets refuse once the set has halted.
   - A disowned interception declaration fails its declaring owner.
   - Error text can't name a forged mod id: mods return detail-only `ModError`, and the loader builds the attributed
     `HostError`.
   - A failed owner's saved data stays opaque byte-for-byte, and manifest `saved_data_versions` are never trusted.
   - Registry provenance records now say the specs are published in-tree.
   - SDK crate-root re-exports are kept; no Cranelift.
   - SevTech decisions:
     - native behavioural parity, not hosting Forge jars;
     - Feather (CC0) mappings, with Legacy Yarn as fallback;
     - LWJGL3 plus a replacement renderer;
     - ATM10 stays replayable;
     - 26.1.2 stays the build default.

## Hard rules (beyond AGENTS.md) learned this session

- jj only. One conventional, lowercase, one-line message per change with no body. Squash fixes into their owning change,
  never into fixup changes. Never push or open a PR without Jacob saying so in that turn.
- Every change in a stack must compile and pass the gate (docs/jj-stacked-changes.md:70). `ravel:verify-stack` rejects
  non-conventional descriptions and merge commits.
- **Evidence binding**:
  - Manifest `inputs` are hashed at the pinned revision via `jj file show` (records.rs:399), so editing their current
    content is fine.
  - Manifest **results** are hashed from the working tree (records.rs:680), so never edit them.
  - `evidence/m5/inventory-summary.json` binds `registry/proposals/**` as MATERIAL. Those files are frozen; nothing
    compiles them.
  - `machineryAmendments` (precedent: nwlmuoyz) cover machinery only, never counts or digests.
  - Never hand-edit digests. Package digests are refreshed with
    `mise exec -- cargo run -q -p ravel-xtask -- package digests <dir>`, then `--check`.
- Gate: `mise exec -- cargo make ravel:verify` (about 5 min, mbx-cached). Judge it by exit status. When comparing
  revisions, use a fresh `CARGO_TARGET_DIR` per revision; a shared one gives stale false failures.
- New files over 1 MiB are refused by the jj snapshot (`--config snapshot.max-new-file-size=2000000` only while
  staging). Keep data files under 1 MiB.
- Subagents in isolated mode hand back patches that sometimes land in the main working copy. Check `jj st` after each
  lane and split the changes out.

## Current state (as of this handoff)

### Committed stack (on main lineage)

`yzlyvrwu` (base) → slrlquqn → vpqsknsx → vtswomvy → wquruvls → nmrzorxp → zsqnuurs → kkszukzv → llsvotxk → **vmrzqpou**
(first deslop pass; full gate green at vmrzqpou). Details are in the `jj log` descriptions.

### Second deslop pass (on vmrzqpou)

Linear: `kvxkzwqr` API cutover (SDK, loader and bootstrap folded together so each change compiles) → `rkplrnrx`
registry → `tsllrspl` tooling → `lvvlpxum` SevTech pins → `ykoplozm` this handoff.

- Fixes all 151 issues from a scored 20-dimension holistic review, minus documented rejections (integration-report.md).
- **The full gate passed** on the tree that is now `tsllrspl`, before the pins and this handoff were stacked on it:
  - 3,720 tests per Minecraft version;
  - 92.95% line coverage;
  - registry: 12 packages, 243 digests and 1,957 tests.
- Every change passes `cargo clippy --workspace --all-targets --all-features` on its own, with a fresh target dir per
  change. The restructure left the tip tree byte-identical to the gated one. The temporary `integrate` workspace was
  removed.
- **Not yet run:** a full gate at the current tip in the main workspace, which also covers the pins and the docs.

### SevTech pins

`lvvlpxum` holds only new files:

- `tests/fixtures/descriptors/{sevtech-ages-3.2.3,runtime-inputs-1.12.2}.json`
- `registry/inventory/sevtech-ages-3.2.3/{references,graph,graph-evidence,rights,rights-evidence}.json`
- `registry/provenance/sevtech-ages-3.2.3.json`

The large downloads live in the gitignored `tests/fixtures/local/sevtech-ages-3.2.3/`. The archive's sha256
`f872775f…83b8` was verified, and 254 + 5 jars are hashed. Re-fetch on a new machine; the WP0 report has the method.

### numen refine

Strict went **24.3 → 82.0**. Target is 85; Jacob wants much higher.

- The 20 scored dimensions came from independent Claude reviewers (each session log was checked for the model name) and
  were imported with `--attested-external`.
- Packet: `.numen-refine/reviews/holistic_packet_20261015T180717_*`. Merged import:
  `.numen-refine/reviews/merged-20261015T180717.json`.
- The 151 imported issues are fixed in the stack but **not yet resolved in numen**.

## Next steps, in order

1. Run `mise exec -- cargo make ravel:verify` at the tip and fix anything the pins or docs introduce.
2. **Resolve the numen findings.** For each fixed issue run (command below).

   ```console
   numen refine resolve ID --status fixed --note '<concrete change + verification>' --attest 'I have actually fixed and verified this issue; I am not gaming the score.'
   ```

   Use `wontfix` with a 50+ char reason for rejected ones (list in integration-report.md). Then `numen refine scan`,
   then `status`.

3. **Scored re-review** (needs the clean tree):
   `numen refine review prepare --lang rust --holistic --json --dimensions <all 20>`. Reviewers must be Claude: use
   `agent: code-reviewer`, which is pinned to anthropic/claude-opus-5-5, but fallback can swap models, so check each
   session's model before an attested import. Run one reviewer per batch. Merge with jq, strip `dismissed_concerns`
   unless you've verified them yourself, validate, then import with
   `--attested-external --attest '... without awareness ... unbiased ...'`. Fix new findings and repeat until strict is
   well above 85.
4. **Deferred deslop items:**
   - A command invoker/permission bridge. Commands currently run for any player; documented in docs/guide/limits.md. It
     needs new pinned hook targets per version, so build it together with the 1.12.2 hook port.
   - ripwire `--quality-delta` exited 2 on several lanes in the first pass. Run it once on the tip and either fix the
     findings or report them.
5. **Crate swaps** (jvm-crate-audit.md), done together with WP1 since they touch ravel-minecraft-generate:
   - classfile reader → `cafebabe` 0.9 (or `noak` 0.7). This fixes a modified-UTF-8 bug.
   - jar reader → `zip` 9, keeping the decompression cap.
   - Java descriptor building → `MethodType`/`Class.descriptorString`.
   - ProGuard record grammar → `proguard` 5.10 (validation stays ours).
   - Use Fabric `mapping-io` at build time for Feather Tiny.
   - Keep `jni` 0.22.4 and `java.lang.classfile`; they're already right.
6. **SevTech work packages** (sevtech-plan.md section 3), in this order:
   - WP7 schema cutover: Forge loader regex at schemas/fixture.schema.json:115, the runtime schema, Prism assumptions in
     registry/checks/artifact-set.sh:35-76, the `loader: ""` meaning, verify-rights accepting cited grants, and
     licence-families.tsv rows (sevtech-wp0-report.md and sevtech-rights-report.md list the exact fields).
   - WP1 mappings generalisation (Feather/Tiny, optional DFU).
   - WP2 minecraft/versions/1.12.2 data.
   - WP3 JDK 27 boot with LWJGL3.
   - WP4 hook ports: registries with numeric IDs and metadata, tick, network, WorldSavedData, resources.
   - WP5 renderer and model memory pipeline.
   - WP6 profiling-guided optimisation.
   - WP8 counterparts, wave A/B/C.
   - WP9 live proof and claim gate.

   Retarget SPEC/ROADMAP/AGENTS to SevTech and 1.12.2 alongside WP7. Keep ATM10 historical evidence intact.

7. SevTech rights summary for counterpart planning:
   - 91 permitted, 64 conditional, 22 prohibited and 82 unresolved source use.
   - 72 inputs still have an `absent` licence, all because of CurseForge 403s.
   - Three citations are weak: SpartanShields, SevTweaksNPC and Better With Mods.
   - Three conflict between repo and Modrinth: ClientTweaks, Waddles and RecipeStages.

## Rejected or kept findings (don't redo)

- Deleting the three placeholder test-mods: they are bound by M4 sdk-surface.
- Removing the rvl-146 singleton: it would change counts bound in M6.
- Crate renames for ravel-bootstrap and composed-host: the names the JVM library is loaded under can't change.
- Reverting the M2 "refused to enter" wording: it is bound by evidence.
- The loosened clippy allows (`must_use_candidate`, `missing_errors_doc`, `missing_panics_doc`, `missing_const_for_fn`,
  `redundant_pub_crate`) are deliberate loke-rs alignment.

## Reports in this directory

integration-notes.md (cross-lane checklist), integration-report.md (final dispositions + gate numbers), lane-*-report.md
(per-lane issue tables), sevtech-plan.md (1.12.2 design, WP0-WP9), sevtech-wp0-report.md, sevtech-rights-report.md,
jvm-crate-audit.md, followups.md.
