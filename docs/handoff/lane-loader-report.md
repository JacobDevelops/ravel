# Loader lane report

Implementation owns `ravel-loader`, `ravel-host-api`, `ravel-host-runtime`, `ravel-composed-host`, and the approved
minimal `ravel-save` extension. Cross-lane production caller migration is intentionally reserved to Main;
bootstrap/test-mods/registry/examples code was not edited here.

## Findings

1. `ravel_loader::open_saved_data enforces migration-chain compl` — FIXED: one loader implementation collects code
   declarations/chains, validates completeness, migrates once and delivers current envelopes. Files:
   loader/src/saves.rs; host-api/src/lib.rs. Bootstrap's coordinated stage_world integration is in its lane.
2. `In the same function, the loader attributes declaration-hook` — FIXED: callbacks stamp the actual dispatched owner.
   Files: loader/src/{lib,saves,services}.rs; host-api/src/lib.rs.
3. `Every HostMod hook returns HostError {mod_id, state, detail}` — FIXED: detail-only ModError, attributed HostError
   boundary, with_context. Files: host-api/src/lib.rs; loader callback callers; SDK re-export; conformance fixtures.
4. `Delivery and Round return the failure without naming which m` — FIXED: failed_by records actual owner independently
   of the error. Files: loader/src/events.rs; host-api/src/interception.rs.
5. `Sibling loader entry points take mods as &[Box<dyn HostMod>]` — FIXED: driving entry points use mutable borrowed
   trait-object slices. Files: loader/src/{config,resources,saves,services,driver}.rs; host-runtime/src/lib.rs;
   conformance callers.
6. `Provision::provides takes a Provided struct whose version is` — FIXED: Provided::new takes parsed Version; removed
   unreadable-version branch. File: host-api/src/provision.rs.
7. `Registry and command claims accept keys in another mod's nam` — FIXED: typed keys enforce namespace ownership.
   Files: loader/src/{lifecycle,registry}.rs; lifecycle/error.rs; host-api/src/lib.rs;
   conformance/content_declarations.rs.
8. `The post-mutation halt gate is never armed in production, an` — FIXED in loader: command/packet shared halted gate
   and regressions. Production arming is coordinated with bootstrap lane. File: loader/src/events.rs.
9. `Saved-data ownership and open sequencing are implemented twi` — FIXED: same unified loader path as finding 1,
   consumed by bootstrap lane. File: loader/src/saves.rs.
10. `The four-field`ModError::Refused { mod_id, state, detail }`` — FIXED API/root cause: ModError::refused consumes
    detail only. Files: host-api/src/lib.rs; SDK re-export; owned loader/conformance/testkit fixtures. Out-of-lane
    duplicated helpers require Main's coordinated migration.
11. `World saved data is opened by two divergent implementations:` — FIXED: unified migration implementation and
    failed-owner preservation. Files: loader/src/saves.rs; save/src/lib.rs. Production caller cutover belongs to
    bootstrap lane.
12. `crates/ravel-composed-host is the JVM harness host hard-wire` — FIXED description/classification: manifest/module
    identify tracked JVM harness; architecture explicitly classifies fixture exception. Files: composed-host/Cargo.toml
    and src/lib.rs; docs/architecture/dependency-rules.md. REJECTED physical relocation/rename: existing tooling
    inventories reference this fixture path and the architecture now accurately distinguishes it from generated hosts.
13. `Delivery and the interception round do not say which mod fai` — FIXED: actual-owner failed_by accessors, same as
    finding 4. Files: loader/src/events.rs; host-api/src/interception.rs.
14. `deliver_packet and route_profiled repeat the same owner-reac` — FIXED: shared to_owner dispatch tail. File:
    loader/src/events.rs.
15. `The documented 'failure after world mutation halts the set a` — FIXED loader half; production arming is owned by
    bootstrap lane. The requested deletion of all failed-owner carryover is superseded by Jacob's explicit opaque
    pre-load preservation decision. Files: loader/src/{events,saves}.rs; save/src/lib.rs.
16. `broadcast refuses a halted set, but route/route_profiled and` — FIXED: ensure_active called first for all
    dispatches; regression includes commands and packets. File: loader/src/events.rs.
17. `Registrar::loads is documented as 'whether mod_id is running` — FIXED documentation. File: host-api/src/lib.rs.
18. `HostError's Display says every refusal is a failure to enter` — FIXED neutral phase wording and assertions. File:
    host-api/src/lib.rs. Digest-bound expected text is not hand-edited; see integration list.
19. `audience() and broadcast_profiled() re-look-up ids taken fro` — FIXED: tracked entry iteration supplies state
    directly. Files: loader/src/{events,lifecycle}.rs.
20. `ravel-composed-host sits among the production crates and cal` — FIXED fixture description and explicit architecture
    classification. REJECTED relocation for the same path-contract reason as finding 12. Files:
    composed-host/{Cargo.toml,src/lib.rs}; architecture/dependency-rules.md.
21. `LifecycleMachine stores and exposes registry claims as &str/` — FIXED typed RegistryKey maps/APIs, no parse/drop
    round-trip. Files: loader/src/{lifecycle,registry}.rs; lifecycle/error.rs; conformance/content_declarations.rs.
22. `CommandInvocation.key is any RegistryKey and route() never c` — FIXED: reject non-command kind before invoking
    code; regression test. File: loader/src/events.rs.
23. `In ravel-loader, interception.rs names its error and variant` — FIXED InterceptRefused, Mod and ForeignDeclaration,
    owner accessor. Files: loader/src/{interception,lib}.rs; bootstrap caller migration reserved to its owner.
24. ``registry_claims`is a count getter on LifecycleMachine and` — FIXED registry_claim_count and
    record_registry_claims. Files: loader/src/lifecycle.rs; observability/src/profiling.rs;
    conformance/content_declarations.rs. Runtime caller migration reserved to bootstrap.
25. ``ravel_host_api::HostError`is a mod's own refusal, yet it i` — FIXED semantic split agreed by Main: ModError is the
    mod-authored refusal; HostError is loader-attributed boundary error. No alias. Files: host-api/src/lib.rs;
    SDK/src/lib.rs; loader callers.
26. ``LifecycleMachine::request_save(&self)`only decides whether` — FIXED save_access. Files: loader/src/lifecycle.rs
    and tests. Bootstrap caller migration reserved to its owner.
27. `The loader migrates saved data before delivery, but the live` — FIXED one loader migration owner; callback docs
    explicitly forbid replay. Files: loader/src/saves.rs; host-api/src/lib.rs. Bootstrap and sample mod caller cutovers
    coordinated externally.
28. `LifecycleMachine stores registry claims under String keys, a` — FIXED same typed-key cutover as 21. Files:
    loader/src/{lifecycle,registry}.rs.
29. `The public launch API returns`Composition`, a 4-tuple type` — FIXED named Composition fields. Files:
    host-runtime/src/lib.rs; composed-host/src/lib.rs; conformance callers.
30. `ravel-bootstrap is the only internal crate wired through a r` — FIXED workspace dependency edge and obsolete
    manifest comment. Files: Cargo.toml; host-runtime/Cargo.toml; bootstrap/Cargo.toml (comment only);
    architecture/dependency-rules.md.
31. `LifecycleMachine keys registry claims by String although its` — FIXED same typed-key cutover as 21. Files:
    loader/src/{lifecycle,registry}.rs.
32. `Delivery and interception rounds do not say which mod failed` — FIXED same actual-owner dispatch attribution as 4
    and 13. Files: loader/src/{events,interception}.rs; host-api/src/interception.rs.
33. `Every loader entry point takes the LifecycleMachine and the` — REJECTED owned Driven redesign: callers
    intentionally support borrowed mod values (testkit, conformance and runtime), so forcing allocation/ownership would
    discard those seams. See loader/src/driver.rs::drive_borrowed and all borrowed entry points. Fixed inconsistent
    collection shape instead (finding 5); no new owning abstraction.
34. `Typed RegistryKey claims are turned into strings to enter th` — FIXED same typed-key cutover as 21. Files:
    loader/src/{lifecycle,registry}.rs.
35. `host-runtime::compose builds a throwaway LifecycleMachine ju` — FIXED: configuration takes admitted entries+side
    and player resolver, collects declaration once; no throwaway machine; named result. Files: loader/src/config.rs;
    host-runtime/src/lib.rs; composed-host/conformance callers.
36. `Both hosts decode the agent's side code to build members and` — FIXED initialize(Side, members), composed host
    decodes once. Files: host-runtime/src/lib.rs; composed-host/src/lib.rs. Generated host callsite is owned by tooling
    lane.

## Explicit decisions

- Callback catch_unwind boundary stamps owner+phase for lifecycle, registration, event, command, packet, interception,
  service and saved-data callbacks/migrations. Optional startup failures drop out; required startup failures stop
  startup. Regression fixtures cover both panic policies. Profiles explicitly select unwind; bootstrap JNI guard remains
  untouched as final host-fault guard. Files: loader/src/{lib,driver,events,interception,saves,services}.rs; Cargo.toml;
  SPEC.md.
- Halt gates, disowned interception attribution and detail-only forged-text attribution have regressions.
- Jacob's latest fallback decision supersedes pre-failure code validation: failed owners with no code declaration remain
  opaque, including version 99 despite manifest versions 1/2. No declaration callback or synthetic declaration is used.
  open_preserving checks structure, preserves envelope, skips declaration/package/version interpretation only for failed
  owners. Loader and save regressions cover preservation across serialization.
- Code review (LoaderDispatchReview) found more defects, and all of them are fixed with regressions:
  - `stop_all` now records shutdown failures, keeps calling the remaining mods until they reach Stopped, then returns
    the first required failure.
  - Callback phase now uses the owner's actual state. Services are stamped Registered and broadcasts during Stopping are
    stamped Stopping.
  - When a provider's `answer_service` panics or refuses, the provider is failed and the optional consumer sees absence,
    so it still reaches Running.
  - A panic is now `HostError::Panicked { mod_id, state }`, which is host-authored and quotes no untrusted text, so it
    can't be confused with a mod refusal.
  - Command panics are charged in the profiler, because the callback now sits inside `measure`.
  - A non-command key gets `DispatchRefused::NotACommand { key }`.
  - `intercept` refuses a halted set with `InterceptRefused::Inactive`.
  - An optional owner's save-callback or migration failure now fails that owner and keeps its envelope opaque, while a
    required owner still refuses the world.
  - `SavedDataAggregate::migrate` was deleted because the loader's loop is now the only migration implementation.
  - `drive`, `drive_borrowed` and `stop_all` take `&FailureEnvironment`, filled from host-runtime `environment()`, so
    attribution no longer says "not supplied".
  - `DriveError::Host { source, attribution: Box<FailureAttribution> }` exposes the required failure's attribution.
- Public `open_services` takes `&LifecycleMachine` again; the internal driver path is the only one that mutates.
- Regression tests were written alongside each fix, but no run was captured showing them failing before the fix.

## Cross-lane edits made

- crates/ravel-save/src/{lib.rs,migrate.rs}: approved minimal open_preserving admission path and its regressions;
  removed the unused `SavedDataAggregate::migrate`.
- crates/ravel-observability/src/profiling.rs: record_registry_claims setter rename and tests.
- crates/ravel-sdk/src/lib.rs: direct ModError re-export/documentation only.
- crates/ravel-testkit/tests/integration/pipeline.rs: detail-only callback fixture migration only.
- crates/ravel-bootstrap/Cargo.toml: obsolete output comment only; no bootstrap code edits.
- tests/conformance/tests/integration/{content_declarations,config_saves_and_services,optional_services,sides_and_packets,mod_capabilities,pack_round_trip,pre_world_gate,profiling_report,sdk_gaps}.rs:
  API cutovers, typed claims and regression expectations.
- Root Cargo.toml, SPEC.md and docs/architecture/dependency-rules.md: profile/dependency/contract updates.

## Integration callsites reserved to Main

- Bootstrap runtime: registry_claims -> registry_claim_count, profiler.registry_claims -> record_registry_claims; string
  claim/describe/owner APIs -> RegistryKey; request_save -> save_access; InterceptionError -> InterceptRefused; actual
  failure owner from Delivery/Round.failed_by; save stage reuses loader; mod callback returns -> ModError.
- Bootstrap profiling.rs, persistence/tests.rs and runtime/tests.rs HostMod implementations must return ModError;
  provider versions must be parsed Version. Bootstrap persistence owner is independently implementing panic-safe writes
  and stage_world reuse.
- SDK content.rs and lib.rs tests still construct old ModError fields in this isolated tree; SDK owner migrates those in
  its tree.
- test-mods, examples, registry proposal fixtures and ravel-build tests need HostMod Result ModError and old four-field
  literal/helper removal; registry rvl-gap-06-t1.rs/t2.rs Provided versions become parsed Version.
- ravel-build/src/generate.rs initialize call decodes Side once; tooling owner coordinates this.
- Final core getter cutover from SDK lane must change the few current .name() calls in host-api HostError Display,
  driver MissingMod Display, and composed-host Side test formatting to .as_str(). They currently use current core APIs
  so standalone owned crates can be checked.
- Digest-bound evidence/m2/pre-world-gate.json expected error message changes from 'refused to enter' to 'refused
  during'; proposal test sources and SPEC.md may require digest regeneration. No evidence/digest file was hand-edited
  here.
- New loader signatures that bootstrap must consume:
  - `open_saved_data(machine: &mut LifecycleMachine, mods, bytes, packages)`
  - `drive(side, &FailureEnvironment, composed)`
  - `drive_borrowed(side, &FailureEnvironment, entries, mods)`
  - `stop_all(machine, &FailureEnvironment, mods)`
  - `DriveError::Host { source, attribution }`
  - `HostError::Panicked`
  - `DispatchRefused::NotACommand`
  - `InterceptRefused::Inactive`
- testkit `src/pipeline.rs` (Tooling lane) still maps `host.enter` errors as HostError and must consume ModError or
  loader `drive_borrowed`/`stop_all`.
- tests/conformance `sides_and_packets.rs` composes `Box<dyn ravel_sdk::Mod>` where `drive` needs `Box<dyn HostMod>`,
  which is part of the SDK boundary migration.

## Verification

All commands were run in its isolated worktree:

- `mise exec -- cargo fmt --all`, then `cargo fmt --all --check`: clean.
- `mise exec -- cargo clippy -p ravel-loader -p ravel-host-api -p ravel-save --all-targets --all-features`: clean.
- `mise exec -- cargo nextest run -p ravel-loader -p ravel-host-api -p ravel-save`: 269 passed, 0 failed.
- `mise exec -- cargo clippy -p ravel-observability --all-targets --all-features`: clean. Earlier,
  `mise exec -- cargo nextest run -p ravel-save -p ravel-observability` passed 63/63.
- `ravel-host-runtime`, `ravel-composed-host` and `ravel-conformance-tests` are BLOCKED. They don't compile in this
  isolated tree because of code outside the lane:
  - Stale bootstrap call sites, e.g. `runtime.rs:89,94` `registry_claims`, `persistence.rs:417` `InterceptionError`, and
    HostError callback returns.
  - The SDK's old four-field ModError literals at `ravel-sdk/src/content.rs:114-115` and `lib.rs:292-293`.
  - The testkit fault at `ravel-testkit/src/pipeline.rs:200,229`.
  - These belong to the Bootstrap, SDK and Tooling lanes and are reconciled at Main integration. Rerun clippy and
    nextest for these crates after the merge.
- `ripwire --quality-delta` was run once mid-change. It was dominated by pre-existing and out-of-scope rows (gating=233,
  scoped-out-gating=32) and was not used as an acceptance gate.
