# Registry lane report

Worktree: its isolated worktree.

## Issue disposition

Labels below are the first 60 characters of each supplied summary. Paths abbreviated `mods/` are beneath `registry/`.

1. **Four counterpart mods link sibling counterpart crates at the** — FIXED. Added required/after package dependencies
   in `mods/rvl-{002,086,132,321}-counterpart/package.json`; removed duplicate dependency/order constants from002.
   Tooling lane owns the Cargo/manifest consistency regression.
2. **Registry counterparts test register() only against copy-past** — BLOCKED on integration of ToolHarness's twelve
   counterpart dev-dependencies into `tests/conformance/Cargo.toml`. Real-loader tests implemented in
   `tests/conformance/tests/integration/registry_lifecycle.rs`, wired in `integration/main.rs`: all12 alone/together on
   all3 sides; independent literal claim censuses and total counts, actual owners/barrier/running states, saved-data
   declaration/manifest equality, dependency order. Removed redundant registration-only mocks/tests in counterpart lib
   files. Retained mocks only for actual dynamic/dependent contributions, commands, adjustments and capabilities.
   Integration compile currently fails only missing counterpart dependencies; no workaround added to installable
   packages.
3. **Five rvl-180 unit tests assert `started.elapsed() < Duration** — FIXED. Removed wall-clock timing and
   Instant/Duration scaffolding from180 grouping/population/listing tests; preserved count/bound/revisit assertions.
4. **Counterpart setting documents repeat each `title` as its `de** — BLOCKED locally on SDK lane integration.
   LaneSdk.SdkTypedConsumers explicitly owns the eight registry settings documents and reported removal of duplicate
   descriptions and templated key docs as part of its typed-settings migration. No conflicting local settings edits;
   Main merges that lane.
5. **rvl-210 helpers.rs exports wrappers that add nothing: one on** — FIXED. Deleted210 `src/helpers.rs` and
   module/reexport; moved meaningful `insert_first` admission into `menus.rs`, fluid `fill` into `storage.rs`; callers
   use Store/Tank, direct closure and Side. Updated210 NOTES and lexicon.
6. **rvl-146 keeps a process-wide construction slot that no host** — FIXED. Deleted singleton slot/sole/error/dead test
   from146 `src/lib.rs`, documented actual independent construction in NOTES. Evidence row-count amendment intentionally
   left to Main; see exact figures below.
7. **insert() says an over-limit sum comes back whole as the rest** — FIXED.042 `src/arithmetic.rs` documents partial
   merge and overflow correctly; regression test renamed to state that rule.
8. **Registry counterparts store namespace:path identifiers as St** — FIXED. Typed321 Tier/Crop ids and002 dependent
   constructors;025 Stack identity;287 qualify result and directory/catalogue keys. Replaced hand-splitting at covered
   consumers with ContentId accessors. Updated callers/tests across002/025/120/287/321. Invalid identifiers are rejected
   at boundaries with distinct errors; added025 saved/wire malformed-id regressions and321 service invalid tier/kind
   regression. Removed fabricated Stack Default instead of inventing an identity. Error-message strings intentionally
   remain strings.
9. **Crop.second_drop_chance is a public f64 that uses -1.0 to me** — FIXED.321 model/harvest use Option with None tier
   fallback, Some(0.0) preserved; sentinel deleted, tests and NOTES updated.
10. **In the counterpart crates, the private ModError helpers reus** — BLOCKED locally on SDK detail-only ModError
    integration, which supersedes phase-specific helpers entirely. LaneSdk.SdkRefusal owns all12 registry lib files
    and120 census. Local180 dump-specific duplicate deleted in favour of Entry::packaging; Main merges remaining
    clean-cutover SDK edits. Current APIs deliberately retained as Main requested.
11. **The counterpart template says the entry type is `Entry`, but** — FIXED. Every counterpart now exports Entry;120
    constructor new(side); all local callers/docs migrated.025 ordering-domain Entry became OrderingEntry to avoid
    colliding with canonical mod Entry.
12. **Every counterpart's dumps.rs serves the same generated/ docu** — FIXED. Aggregators documents; static *_PATH
    constants; fallible packaged renderers return Result<_, ModError> through existing refusal helpers. Updated
    declarations, callers, golden tests and NOTES.
13. **In rvl-180, `census` is a world-entity population model, whi** — FIXED. Renamed180 census.rs/Census to
    population.rs/Population, migrated references; updated lexicon and package source rows/digests.
14. **Some registry counterpart rule functions are named for an ad** — FIXED.060 matches_leniently,042
    combination_family; all callers/tests/NOTES migrated.
15. **rvl-210's public helpers module is mostly pass-throughs: an** — FIXED. Same real cutover as issue5, not a second
    abstraction.
16. **Eight installable registry counterparts inherit `num-traits.** — FIXED (seven actual manifests, not
    eight).025/042/060/086/120/146/321 explicitly pin num-traits0.2.19 without default features; root workspace
    dependency removed because unused. ToolBuild owns standalone installed120 regression.

## Additional requested fixes

- Corrected `savedDataVersions` to `[1]` in120/146/210/321 package manifests, matching actual saved-data
  declarations/provenance. Coordinated ToolHarness registry consistency coverage.
- Reworded exactly23 `registry/provenance/m6-foundation/RVL-*.json` publication notes to reflect specifications
  published in-tree:002,025,042,053,060,086,097,103,120,129,132,146,159,162,164,180,199,201,203,210,217,287,321. No
  other provenance fact changed. Checked evidence references before editing: none of these23 was digest-bound there; the
  separate bound atm11 provenance record was untouched.
- Refreshed source/provenance hashes in all12 package manifests, including deleted/renamed source entries. Final210
  NOTES source hash refreshed after verdict correction. No evidence file edited.

## Cross-lane edits and integration prerequisites

Actual edits outside registry:

- `Cargo.toml`: removed now-unused workspace num-traits dependency.
- `crates/ravel-network/src/lib.rs`: WireError::ContentId diagnostic variant plus regression; malformed stack identity
  must not pretend to be a contract mismatch. Coordinated LaneSdk.
- `tests/conformance/tests/integration/{main,registry_lifecycle}.rs`: real counterpart lifecycle suite. ToolHarness owns
  the required12 dev-dependencies, not present in this isolated tree.
- Cargo.lock follows dependency graph changes.

Main requested retention of current SDK names in this tree. Affected integration sites:

1. Detail-only ModError: every counterpart `src/lib.rs` refusal constructor/helper
   (002,025,042,060,086,120,132,146,180,210,287,321), plus120 `src/census.rs:88`.042/086/180 dumps now call
   Entry::packaging;120 dumps calls Entry::refusal; these callers must migrate if helpers disappear. SDK worker already
   owns these clean-cutover changes.
2. Side::name -> as_str:180 `src/commands.rs:177`;210 `src/dumps.rs:185`. Other `.name()` methods on local domain/config
   types are not this API.
3. SavedDataMod declaration Result: no registry implementation/reference to SavedDataMod found; registry uses
   Mod::saved_data_declaration, including the new conformance test.
4. SavedPayload: no registry references.
5. VersionAdapter/PinnedAdapter: no registry references.
6. Loader API rename outside SDK list: conformance `registry_lifecycle.rs:164` still calls current registry_claims; Main
   must migrate to registry_claim_count with loader changes.

## Required evidence amendment (not edited)

146 NOTES singleton row changes exercised -> excluded. Both `registry/records/m6-notes-row-counts.json` and
`evidence/m6/counterpart-summary.json` still describe old counts:

- BEFORE: exercised51, declarable-only5, excluded4, observableAreas56.
- AFTER: exercised50, declarable-only5, excluded5, observableAreas55. The evidence summary may only store
  exercised/declarable/observable values, not excluded. Amend through sanctioned path, then refresh affected evidence
  digests centrally.210 remains36/5/6/41 after correcting its row to begin with published verdict `answered`.

## Verification actually exercised

- `mise exec -- cargo fmt --all` — PASS (repeated after final Rust edits).
- (command below)

  ```console
  mise exec -- cargo clippy -p rvl-002-counterpart -p rvl-025-counterpart -p rvl-042-counterpart -p rvl-060-counterpart -p rvl-086-counterpart -p rvl-120-counterpart -p rvl-132-counterpart -p rvl-146-counterpart -p rvl-180-counterpart -p rvl-210-counterpart -p rvl-287-counterpart -p rvl-321-counterpart --all-targets --all-features
  ```

  — initial runs exposed mechanical compiler/lint defects, fixed.

- (command below)

  ```console
  mise exec -- cargo clippy -p rvl-002-counterpart -p rvl-025-counterpart -p rvl-042-counterpart -p rvl-060-counterpart -p rvl-086-counterpart -p rvl-120-counterpart -p rvl-132-counterpart -p rvl-146-counterpart -p rvl-180-counterpart -p rvl-210-counterpart -p rvl-287-counterpart -p rvl-321-counterpart -p ravel-network --all-targets --all-features
  ```

  — PASS; earlier failed iterations corrected Copy derives, assert style and needless borrows. Required registry gate
  also linted/formatted final snapshot correction successfully.

- (command below)

  ```console
  mise exec -- cargo nextest run -p rvl-002-counterpart -p rvl-025-counterpart -p rvl-042-counterpart -p rvl-060-counterpart -p rvl-086-counterpart -p rvl-120-counterpart -p rvl-132-counterpart -p rvl-146-counterpart -p rvl-180-counterpart -p rvl-210-counterpart -p rvl-287-counterpart -p rvl-321-counterpart
  ```

  — initial compile failure025 duplicate Entry corrected.

- (command below)

  ```console
  mise exec -- cargo nextest run -p rvl-002-counterpart -p rvl-025-counterpart -p rvl-042-counterpart -p rvl-060-counterpart -p rvl-086-counterpart -p rvl-120-counterpart -p rvl-132-counterpart -p rvl-146-counterpart -p rvl-180-counterpart -p rvl-210-counterpart -p rvl-287-counterpart -p rvl-321-counterpart -p ravel-network
  ```

  —1965/1966 passed; new service regression wrongly assumed initially empty default tiers, fixed by asserting unchanged
  actual snapshots instead.

- (command below)

  ```console
  mise exec -- cargo nextest run -p rvl-002-counterpart -p rvl-025-counterpart -p rvl-042-counterpart -p rvl-060-counterpart -p rvl-086-counterpart -p rvl-120-counterpart -p rvl-132-counterpart -p rvl-146-counterpart -p rvl-180-counterpart -p rvl-210-counterpart -p rvl-287-counterpart -p rvl-321-counterpart -p ravel-network --status-level fail --final-status-level fail
  ```

  — PASS,1966 tests, zero skipped.

- `mise exec -- cargo make ravel:registry-verify` — exit105. All12 counterpart crates compiled/formatted/linted;1949
  tests passed. All243 package digests accounted/recomputed; boundary/provenance/spec checks passed or explicitly
  reported unavailable ignored fixture inputs. Notes accounting failed on146 evidence counts above and210 invalid
  leading verdict.210 verdict fixed and package hash refreshed;146 evidence amendment intentionally reserved to Main.
  Therefore not claimed as passing.
- `mise exec -- cargo clippy -p ravel-conformance-tests --all-targets --all-features` — dependency failure101: all12
  counterpart crates missing from isolated conformance dev-deps; ToolHarness merge required.
- `mise exec -- cargo nextest run -p ravel-conformance-tests --status-level fail --final-status-level fail` — same
  dependency failure101, no conformance tests executed.
- `ripwire registry --quality-delta` — exit2, reports288 regressions/121 gating under git-HEAD baseline; includes Rust
  trait-test/mock and renamed-entry dead-code classifications. Not represented as a clean quality gate. Independent
  focused reviews found and corrected lifecycle-test placement, total census coverage, an introduced bare-id startup
  panic, caller parse panics, invented Stack default and malformed-id diagnostic confusion.

No full workspace verification, commit or evidence edits performed.
