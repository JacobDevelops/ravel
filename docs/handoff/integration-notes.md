# Integration notes

## Integration method

Each lane is its own sibling change on vmrzqpou: SDK `kvxkzwqr`, registry `rkplrnrx`, bootstrap `umuovykp`, loader
`ouxnkxzt`, and WP0 pins `lvvlpxum`, which is @ in the main workspace until SevTechRights finishes. Tooling is still to
add. Then `jj new` a merge of all of them, resolve the conflicts file by file, and do the mechanical pass. Snapshot with
`--config snapshot.max-new-file-size=2000000` while graph-evidence.json is about 805 KB. Do all further lane and merge
work in the jj workspace `integrate` at ../ravel-integrate, and leave the main working copy on `lvvlpxum` so
SevTechRights' writes land there. Remove the workspace (`jj workspace forget integrate`, then rm the dir) when the work
is done.

## Post-merge mechanical pass

- `.name()` → `.as_str()` for Side, LifecycleState, FailureKind across loader, bootstrap, xtask, registry, test-mods and
  examples (SdkDiagnostics).
- `quotes_untrusted_text` default removed. Pass an explicit `false` at bootstrap persistence/tests.rs `Unresolvable` and
  loader/world.rs `Unresolvable`. Pass an explicit flag in ravel-package compatibility.rs/integrity.rs (SDK lane handles
  these).
- `VersionAdapter`/`PinnedAdapter` removed. Use `adapter::target` and `pinned::{TARGETS, NON_GOALS, MINECRAFT_VERSION}`,
  and replace `NON_GOALS.contains(&HookId)` with direct pinned constants. Migrate callers in bootstrap lib.rs tests and
  runtime/tests.rs.
- `ModError` is now detail-only (`Refused{detail}` / `Unsupported`) in host-api. Out-of-lane `HostMod` implementations
  need migrating; LoaderApi reports their paths.
- `HostError` → `ModError` rename (finding 25) is superseded: `ModError` is now the mod-side type and `HostError` stays
  as the attributed boundary type. Recheck finding 25 against that design.
- Bootstrap needs loader signatures for failed_by, save_access and the LoaderSaves envelope. LaneBootstrap reports them.
- Registry digests: refresh once, after the SDK lane's cross-lane edits merge.
- SevTech WP0: the schema blocks are the Forge loader regex at schemas/fixture.schema.json:115, plus the runtime schema
  fields. Handle these in WP7.
- `SavedDataMod::declaration` now returns `Result<SavedDataDeclaration, SavedDataError>`. Unwrap the declaration in
  bootstrap persistence/tests.rs (about lines 970 and 1008). Old test-mod callers import `ravel_sdk::Mod`.
- Bootstrap runtime.rs:302: `Inner::record` derives `Some(FailureSurface::Event(event.as_str().to_owned()))` or
  `Some(FailureSurface::Registry(invocation.key.clone()))` and passes it to `record_failure`; persistence passes None.
  Add a test that a real event failure records its surface. Apply this after the SDK lane merges.
- Bootstrap mods.rs `surface_of`: `RegistryKey` has no `as_str`. Write the Registry field directly in the
  `attribution()` branch through `Display`, so the escaping writer gets no allocation. Registry wire keys are now
  `block:example_mod:block`, so fix any old `example_mod:block` fixtures. Stale `player_tick` in tests becomes
  `server-tick`.
- `quotes_untrusted_text: false` also needed in resolve/error.rs `ResolutionError` and loader lifecycle/error.rs
  `LifecycleError`. Stage-at now takes `RenderStage`, and `unknown_entry`/`unknown_stage` now take `Option`. The full
  list is in the LaneSdk report.
- The SDK exports both `ModError`, which is detail-only and returned by mods, and `HostError`, which is attributed and
  observed through `ServiceCallFailed::Refused`. Keep both exports when merging LoaderApi's SDK edits. `HostError` has
  no constructor that mods can call.
- `ModError::refused(detail: impl Into<String>)` accepts authored reasons. Convert printable error objects with
  `map_err(ModError::from_display)` instead; it preserves their display text without adding host attribution.
- After the merge, refresh package digests for the 9 package dirs listed in the LaneSdk report, plus the registry
  digests (`package digests <dir>`, then `--check`).
- tests/conformance registry_lifecycle.rs depends on the 12 counterpart dev-deps from ToolHarness and on a loader API
  `registry_claim_count`; check that both exist after the merge.
- Make `PreWorldReport` note storage bounded and deduplicating, using a ring of 32 like the old persistence ring. Every
  autosave veto currently appends a note, so the list grows without limit. This belongs in observability, not in a
  second queue in bootstrap.
- Panic hook policy: now that the loader catches mod panics, the default hook still prints raw panic text to stderr.
  Install one host-level hook in bootstrap's composed entry that sends a redacted record through observability.
- The SDK lane deleted 3 empty placeholder test-mods: dependency-base, dependency-consumer and failure-cases. Each was
  only a Cargo.toml and lib.rs, so no tests are lost. Confirm the tooling lane removed them from `TEST_MOD_NAMES` (xtask
  lib.rs:58-81) and from scaffold.rs. If it didn't, remove them there. The SDK lane also edited
  `evidence/m4/sdk-surface.json`; check whether a digest binds it, and if one does, revert that edit or record it as a
  machinery amendment.
- The registry lane added `WireError::ContentId` in crates/ravel-network/src/lib.rs and also edited that file for the
  `.as_str()` rename. Keep both edits when merging.
- The bootstrap patch did not apply automatically: the lane patch LaneBootstrap.patch (already applied). Apply it by
  hand after the other lanes merge. It also changes composed-host, build generate.rs and tests.rs, minecraft-generate,
  Agent.java, Hook.java, host-runtime, conformance, and docs/guide/limits.md. It needs the loader symbols
  `InterceptRefused`, `LifecycleMachine::save_access`, `Delivery::failed_by` and `Round::failed_by`. Remove the commands
  entry in limits.md once the permission bridge lands.
- The registry lane removed the rvl-146 singleton, which changes the §2 rows in records/m6-notes-row-counts.json and
  evidence/m6/counterpart-summary.json. Counterpart 146 goes from exercised 51 / declarable 5 / excluded 4 / observable
  56 to 50 / 5 / 5 / 55. Its fix to the rvl-210 verdict restores 36 / 5 / 6 / 41. Gate exit 105 until this lands. Record
  it as a `machineryAmendments` entry, following the precedents in evidence/m5/inventory-summary.json and nwlmuoyz, and
  don't rewrite history silently. If no amendment path covers m6 counts, put the singleton back and reject the finding.
- The loader confirmed InterceptRefused, save_access and both failed_by functions, with exactly the names bootstrap
  expects. Two of its signature changes will break bootstrap call sites:
  `open_saved_data(machine: &mut LifecycleMachine, mods: &mut [&mut dyn HostMod], ...)` (bootstrap assumed the old
  signature), and `drive`/`drive_borrowed`/`stop_all` gaining a `FailureEnvironment` argument. Also new:
  `DriveError::Host { source, attribution }`, `HostError::Panicked { mod_id, state }` and
  `DispatchRefused::NotACommand { key }`.
- Loader reconciliation items:
  - Bootstrap: runtime.rs:89,94 call `registry_claims`; this is now the observability rename `record_registry_claims`.
    persistence.rs:417 uses `InterceptionError`, which is now `InterceptRefused`. Mod callbacks return `HostError` and
    must return `ModError`.
  - SDK: content.rs:114-115 and lib.rs:292-293 still build old four-field `ModError` literals.
  - Testkit: pipeline.rs:200,229.
  - Conformance: sides_and_packets.rs passes `Box<dyn Mod>` where `drive` takes `Box<dyn HostMod>`.
  - Registry: rvl-gap-06-t1.rs and t2.rs need a parsed `Version`.
  - Evidence: evidence/m2/pre-world-gate.json expects "refused to enter", but the loader now says "refused during". Keep
    the evidence-bound wording by reverting the loader message, unless the evidence is regenerated through a sanctioned
    path.
- WP7 (SevTech tooling cutover; not the Integrator's job):
  - sevtech rights.json `licence.declared` holds the cited grant, while the jar's own empty declaration lives in
    rights-evidence.json under `hostDeclaration`. Teach verify-rights to accept a cited grant whose source is in the
    evidence file, rather than requiring the jar's own declaration.
  - Add rows to registry/checks/licence-families.tsv for the new declared strings (GPL-3.0, LGPL-2.1, Unlicense,
    CC-BY-4.0 and the rest in registry/notes/sevtech/sevtech-rights-report.md).
  - The Prism-tree and surfaces.json prerequisites still block `verify-rights --fixture` and
    `verify-provenance --offline`.
