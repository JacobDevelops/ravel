# Ravel lane integration report

## Result and final stack

The integrate workspace is left at tooling `tsllrspl`. No merge or extra empty change was created; descriptions remain
conventional one-line messages. Nothing was pushed. `lvvlpxum` and the main workspace were not edited.

Linear ancestry, oldest first:

| Lane                                   | Change   | Final commit |
| -------------------------------------- | -------- | ------------ |
| Base                                   | vmrzqpou | 3d68f96cf2e2 |
| API cutover (SDK + loader + bootstrap) | kvxkzwqr | 302b70364028 |
| Registry                               | rkplrnrx | b68ce8275eaf |
| Tooling                                | tsllrspl | d23d53f4ba94 |

`jj log --no-graph -r 'conflicts()'` returned no entries. The original five-lane integration was reorganized into the
three independently compiling changes above with explicit approval. Cargo refreshed missing restored-placeholder entries
in ancestor Cargo.lock files during isolated clippy runs. The final tip tree is byte-identical to the successful
full-gate tree; the exact pre/post comparison and fresh-target clippy results are recorded in the appended section.

## Full gate evidence

`mise exec -- cargo make ravel:verify`, run asynchronously from `ravel-integrate`, completed successfully: **exit 0**,
Build Done in **237.69 seconds**. Full output: the gate log (job .

- Minecraft **1.21.1: 3,720 passed, 0 skipped**.
- Minecraft **26.1.2: 3,720 passed, 0 skipped**.
- Coverage run: **3,720 passed, 0 skipped**; **92.95% lines**, **92.49% regions**, **91.69% functions**. Coverage
  totals: 40,382 lines / 2,847 missed; 43,499 regions / 3,265 missed; 4,838 functions / 402 missed.
- Format, typecheck, clippy with `-D warnings`, audit, scaffold, example package/pack, Java, JVM, version sweep,
  registry offline verification and stack verification passed.
- Registry: 484 tracked files; 15 milestone-bound files (7 material, 8 machinery); 40 scanned check files; 27 declared
  read primitives (12 refused / 15 accounted), 95 uses over 37 rows.
- Proposals: 6 capabilities over 221 of 235 entries, 10 families, 12 historical cases, 1,027 published words. Wave: 12
  entries.
- Notes: **550 rows**, **432 exercised / 74 declarable-only / 44 excluded / 506 denominator**, over 12 packages and 22
  verdicts; 12 evidence rows validated. One section marker outside a section table is reported, not silently counted.
- Counterparts: **237 source files**, 39 generated artefacts, 890 upstream identifiers, 87 published names; **12
  packages / 243 recomputed package digests / 1,957 passing tests**. Per-package tests: 002=139, 042=86, 321=157,
  025=211, 120=192, 060=244, 086=154, 132=276, 210=179, 146=147, 180=69, 287=103.
- Specifications: 43 files. Rights: 10 surfaces, 324 artifacts, 3 digests, 12 wave entries, 1 wave.
- The full gate intentionally uses registry `--offline`: acquired pack reconciliation and absent clean-room source trees
  were **SKIPPED, not passes**. 0 of 18 fetched source trees were checked. The separate `ravel:registry-verify-fixture`
  task was not claimed.

## Independent per-change clippy

The former four-exception table is superseded by the cohesive API-cutover stack and fresh-target results appended below.
SDK, loader and bootstrap were folded into `kvxkzwqr` with explicit approval because their detail-only callback
migration is one compile-time contract. Registry and tooling remain separate changes. The original independent failures
are historical investigation evidence, not accepted exceptions.

## Every integration-notes item

Line references below refer to `docs/handoff/integration-notes.md`.

| Notes item                                           | Disposition                                                                                                                                                                                                                                         |
| ---------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1-2 integration method                               | REJECTED: superseded by the explicit linear-rebase assignment; no merge change.                                                                                                                                                                     |
| 3 workspace placement/cleanup                        | DONE for isolated integrate workspace and leaving main untouched; REJECTED workspace deletion at handoff because the deliverable must remain available for inspection.                                                                              |
| 6 .name() migration                                  | DONE in current production callers, tests, examples and tooling; frozen historical proposal fixtures are intentionally unchanged.                                                                                                                   |
| 7 explicit quote flags                               | DONE in bootstrap, loader and package diagnostics.                                                                                                                                                                                                  |
| 8 adapter removal                                    | DONE: direct target/pinned APIs; removed adapter callers migrated, no shim.                                                                                                                                                                         |
| 9 detail-only callback ModError                      | DONE across HostMod implementations.                                                                                                                                                                                                                |
| 10 superseded HostError rename                       | REJECTED merger of the types; retained separate detail-only ModError and attributed HostError.                                                                                                                                                      |
| 11 bootstrap loader signatures                       | DONE: failed_by, save_access and LoaderSaves envelopes reconciled.                                                                                                                                                                                  |
| 12 registry digests                                  | DONE using sanctioned package digest producer and check.                                                                                                                                                                                            |
| 13 SevTech WP0 schema                                | REJECTED/out of scope as assigned to later WP7.                                                                                                                                                                                                     |
| 14 fallible SavedDataMod declaration                 | DONE; fallible declarations explicitly handled and Mod imports migrated.                                                                                                                                                                            |
| 15 FailureSurface production population              | DONE: real event and registry invocation surfaces, with a production-event regression test. Persistence remains None.                                                                                                                               |
| 16 RegistryKey display/wire fixtures                 | DONE: direct Display escaping without intermediate allocation; block-prefixed wire keys and server-tick fixtures corrected.                                                                                                                         |
| 17 flags, typed RenderStage, optional unknown values | DONE in live APIs/callers. Historical M5 proposal bytes restored rather than rewritten.                                                                                                                                                             |
| 18 both error exports                                | DONE; no mod-constructible attributed HostError constructor.                                                                                                                                                                                        |
| 19 refused map_err conversion                        | DONE using error.to_string().                                                                                                                                                                                                                       |
| 20 all package digest refreshes                      | DONE for all 21 package directories (9 SDK/test/example plus 12 registry), followed by --check. No digest hand edits.                                                                                                                               |
| 21 conformance dev-dependencies/count                | DONE: 12 counterpart dev-dependencies (0.1.0 except rvl-180 4.6.1), registry_claim_count and conformance fixture target corrected.                                                                                                                  |
| 22 bounded notes                                     | DONE in observability: deduplicating FIFO ring of 32 with tests and documentation, no duplicate bootstrap queue.                                                                                                                                    |
| 23 panic hook                                        | DONE: one redacting host-level panic hook through observability.                                                                                                                                                                                    |
| 24 placeholder/name-list/sdk-surface deletion        | REJECTED: restored dependency-base, dependency-consumer, failure-cases, all name lists and sdk-surface.json exactly; M4 binds them and there is no producer. No M4 amendment.                                                                       |
| 25 WireError ContentId                               | DONE: retained ContentId and as_str changes together; guide definition synchronized.                                                                                                                                                                |
| 26 manual bootstrap patch                            | DONE across bootstrap/composed-host/build generator/host-runtime/JVM/conformance/docs. Permission bridge did not land, so its commands limitation is correctly retained.                                                                            |
| 27 rvl-146 singleton removal                         | REJECTED: no applicable M6 machinery-amendment mechanism; restored singleton, AlreadyConstructed and tests/NOTES. Counts remain 51/5/4/56. rvl-210 verdict correction retained at 36/5/6/41. M6 recorded result bytes and row-count JSON unchanged. |
| 28 loader environment/error APIs                     | DONE: drive/drive_borrowed/stop_all use real FailureEnvironment, open_saved_data migrated, DriveError::Host callers reconciled. Owning loader versions replace tooling duplicates; MissingMod duplicate removed.                                    |
| 30 bootstrap naming/errors                           | DONE: record_registry_claims, InterceptRefused and ModError callback signatures.                                                                                                                                                                    |
| 31 SDK old error literals                            | DONE: obsolete four-field ModError construction removed.                                                                                                                                                                                            |
| 32 testkit pipeline                                  | DONE: loader-owned driving boundary and errors.                                                                                                                                                                                                     |
| 33 conformance HostMod                               | DONE: HostMod objects supplied to loader.                                                                                                                                                                                                           |
| 34 proposal parsed Version edits                     | REJECTED for historical probe files: restored exact bound M5 bytes; current compiled callers use typed Version. See evidence details below.                                                                                                         |
| 35 M2 wording                                        | DONE: loader restored evidence-bound "refused to enter" wording, tested; evidence bytes unchanged.                                                                                                                                                  |
| 36-39 SevTech WP7                                    | REJECTED/out of scope: grant handling, licence-family additions and Prism/surfaces prerequisites remain the separately assigned SevTech/tooling work. No lvvlpxum edits.                                                                            |

## Additional requested reconciliation and tooling fixes

DONE: carry_outward now goes through the shared catch boundary and skips failed owners; regressions cover panic
containment and no re-drain. Saves have a single catch_unwind boundary; required/optional failures preserve attribution
and optional envelopes. open_saved_data receives the real environment; tests verify environment and persistence surface
None. Repeated initialize checks the slot before composing mods, and the second initialize test proves the construction
closure is not run. These changes were squashed into bootstrap `umuovykp` (shared catch implementation in owning
loader).

DONE: all six tooling test failures resolved: closure/M4 figure wording aligned to M6/M7; two repository fixtures
initialize Git and stage files with git add -A; test-only revision fallback removed; host-refusal fixture has
installation Cargo.toml/Cargo.lock; scaffold test honors the restored evidence-bound placeholder set. Full gate executes
the tests successfully.

## Evidence and sanctioned changes

- Input digests use the **historical revision pinned in each manifest**, not arbitrary current content
  (`tools/ravel-xtask/src/evidence/records.rs:317-377`). The path must still exist now. In contrast, benchmark results
  and review digests bind **working-tree bytes**.
- Kept lane ROADMAP edits: ROADMAP is a historically pinned input, so current edits are allowed. README/STATUS edits are
  not live-byte bound and remain. Despite the distinction above, Main explicitly required exact restoration of M4
  sdk-surface.json and placeholder paths/name lists; that restoration was made, with no invented amendment.
- Kept only tooling's removal of two malformed optional changeId fields in `evidence/m6/manifest.json`. Evidence
  reviews, registry records and other manifests were searched: no external digest binds that manifest's own bytes; M6
  review prose mentions it but does not hash it, and self-binding is forbidden. This is a manifest schema repair, not a
  rewritten result. Every M6 result artifact and `registry/records/m6-notes-row-counts.json` remains untouched.
- The M5 precedent nwlmuoyz permits a declared inventory machinery amendment (primitiveScan.filesScanned 39 to 40), not
  a silent rewrite of M6 material row counts. Therefore the rvl-146 finding was rejected rather than fabricating an M6
  amendment.
- Package file digests were produced with `mise exec -- cargo run -q -p ravel-xtask -- package digests <dir>` and
  checked with `package digests --check <dir>`. The final registry gate recomputed 243 counterpart digests. No
  digest/evidence content was hand-authored.
- Historical proposal facts: `registry/checks/verify.sh` invokes `verify-proposals.sh`, which checks SHA-256 bindings
  and closed vocabulary; it does **not** compile or run `registry/proposals/tests/*.rs`. Searches through Cargo targets
  under crates/tests/tools plus Makefile and gate scripts found no include or compilation of those files.
  `rebuild-proposals.sh` is a producer, but its generated `registry/proposals/sdk-gaps.json` is M5 **material**, bound
  by evidence/m5/inventory-summary.json:48-49. Therefore regeneration is not sanctioned as a machinery change. Restored
  rvl-gap-03-t1, 04-t1, 04-t2, 06-t1 and 06-t2 exactly from vmrzqpou in owning lanes; current SDK APIs remain typed
  without shims. This simultaneously restores proposal hashes and removes the accidental afterterrain/renderstage
  lexicon drift. Final proposal verification passes.

## Remaining scope boundaries

Offline fixture obligations are explicitly not claimed. The ignored local tooling fixture symlink points to the existing
main-workspace fixture directory for the two JVM versions; no private fixture files were committed or modified.
Workspace integrate is retained for Main; no cleanup of that workspace or pushes were performed.

## Completed independent-build repair

The approved cohesive cutover replaces the original SDK → loader → bootstrap sequence: `ouxnkxzt` and `umuovykp` were
squashed into `kvxkzwqr`, described as **refactor: cut the sdk, loader and bootstrap over to detail-only mod errors,
typed descriptors and owner-attributed failures**. Registry and tooling remain separate. The four historical
intermediate failures are no longer accepted exceptions.

Caller migrations now land with the APIs they consume: lifecycle/side/metric/support `as_str`, renamed Java generation,
detail-only callback failures, typed render stages, borrowed settings reads, and loader-driven testkit/conformance
callers. Descendant conflict resolutions preserve their original final content.

Each retained change passed `mise exec -- cargo clippy --workspace --all-targets --all-features` with its own fresh
`CARGO_TARGET_DIR`:

| Change                 | Final commit                               | Fresh target directory | Result                | Output       |
| ---------------------- | ------------------------------------------ | ---------------------- | --------------------- | ------------ |
| API cutover `kvxkzwqr` | `302b70364028d59c45737ca4c0b8483383a92ee5` | fresh target dir       | PASS, exit 0 (6.92s)  | the gate log |
| Registry `rkplrnrx`    | `b68ce8275eafe684f68aa8b15a77f2a41fc51ca7` | fresh target dir       | PASS, exit 0 (10.02s) | the gate log |
| Tooling `tsllrspl`     | `d23d53f4ba94d8c58ad907db492ade108437896c` | fresh target dir       | PASS, exit 0 (15.29s) | the gate log |

Original tip recorded before repair: `5d8a0a6ddaf21fedb7766655d4c23fd09c96647a`.
`jj diff --from 5d8a0a6ddaf21fedb7766655d4c23fd09c96647a --to tsllrspl` produced no output; the tip tree is unchanged.
`jj log --no-graph -r 'conflicts()'` also produced no output. All three fresh target directories and the exploratory
cutover target were deleted after the checks. The integrate workspace is left at `tsllrspl`; `lvvlpxum` was untouched
and nothing was pushed.
