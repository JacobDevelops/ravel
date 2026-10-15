# SDK lane report

## Issue dispositions

1. In ravel-minecraft-generate, the Java-rendering module rende — FIXED. Shared descriptor/name primitives moved to
   classfile; render no longer forms the shared dependency hub. Files: generator
   src/{classfile,render,roles,screen,stub_render,mapping,inputs,lib}.rs, docs/architecture/dependency-rules.md.
2. mods_are_reported_costliest_first_and_ties_break_on_identity — FIXED. Zero-time claims now assert complete
   deterministic order. Files: observability/src/profiling.rs.
3. StageOrder::at takes a stage name as &str and returns an emp — FIXED. Typed RenderStage argument; SDK/test-mod docs
   and tests migrated. Files: data/src/render_stages.rs, sdk/src/render_stages.rs, test-mods/render-stages/src/lib.rs.
   External registry/proposal callers require central migration below.
4. AudioError::unknown_entry and StageRefused::unknown_stage re — FIXED. Both return Option<&str>; positive/negative
   coverage updated. Files: data/src/{audio,render_stages}.rs, test-mods/audio/src/lib.rs. External conformance/registry
   callers below.
5. Credential detection rules are copied by hand across the run — FIXED in owned producer. Public canonical redaction
   constants exported for tooling reuse; LaneTooling handles tooling consumers. Files:
   observability/src/{redaction,lib}.rs.
6. StructuredFailure::quotes_untrusted_text defaults to false, — FIXED. Required explicit classification, freeform
   Minecraft compatibility refusal classified untrusted with regression. Files: core/src/diagnostics.rs,
   observability/src/report.rs, package/src/compatibility.rs, resolve/src/error.rs. Lane-owned implementations below
   need central explicit classification.
7. The SDK exposes `SavedPayload` and `JsonValue` as two aliase — FIXED. SavedPayload deleted and all occurrences
   migrated. Files: sdk/src/save.rs, docs/guide/state.md, examples/{minimal-mod,guide-snippets,stateful-mod}/src/*.rs,
   test-mods/{inter-mod-service,waystone-ledger}/src/lib.rs, conformance integration/config_saves_and_services.rs.
8. Seven test-mods still implement `ravel_host_api::HostMod` ag — FIXED. Seven old mods now use SDK Mod/declare_mod;
   behavior assertions retained. Files:
   test-mods/{commands,configuration,events,noop,registries,resources,saved-data}/{Cargo.toml,src/lib.rs},
   sdk/src/content.rs.
9. test-mods/failure-cases, test-mods/dependency-base and test- — FIXED. Removed three empty crates and inventory
   entries (ToolStructure handles xtask/scaffold inventory). Files: six placeholder Cargo.toml/src/lib.rs files deleted;
   ROADMAP.md, evidence/m4/sdk-surface.json.
10. The JVM descriptor grammar is hand-parsed three times, and t — FIXED. One structured classfile tokenizer drives hook
    shapes and source spelling; tooling reuse coordinated with LaneTooling. Files:
    generator/src/{classfile,render,lib}.rs and related callers.
11. SettingKey::validate and its error enum are token-for-token — FIXED. Shared core check_identifier used by ModId and
    SettingKey; SettingKeyError is the existing ModIdError reexport. Files: core/src/{identity,lib}.rs,
    config/src/declaration.rs, package/src/manifest.rs test diagnostic expectation.
12. ConfigurationDeclaration::parse rejects a default that break — FIXED. Validated raw-to-public serde conversion and
    private fields/accessors prevent bypass; direct Deserialize and compile-fail regression coverage. Files:
    config/src/{declaration,schema,layered,file,lib}.rs; direct caller getters migrated.
13. Seven test mods implement ravel_host_api::HostMod directly a — FIXED. Same clean SDK cutover as issue8.
14. test-mods/dependency-base, dependency-consumer and failure-c — FIXED. Same deletions as issue9.
15. ravel-data, documented as content declarations and pack gene — FIXED using explicitly offered documentation
    alternative. Crate responsibility now includes audio and render ordering. File: data/src/lib.rs.
16. Resolved settings are exposed only as serde_json::Value, so — FIXED. Typed boolean/integer/number/text accessors
    enforce declared kinds, preserve borrowing and distinguish range errors. Consumer projections removed, including
    RVL025 Unread. Files: config/src/{layered,schema,lib}.rs; registry settings/dumps listed below. Strict declared
    kinds are intentional (no inferred kind for untyped enums or integer-to-number declaration coercion).
17. ReflectedMember stores an exactly-one-of choice as two indep — FIXED. Validated Lookup enum rejects neither/both at
    deserialization; obsolete invalid runtime fixture replaced with parse regression proving both cases rejected before
    JAR access. Files: generator/src/{inputs/schema,inputs,mapping,render,roles}.rs.
18. FailureSurface carries bare Strings for registry and packet — BLOCKED on central production caller migration.
    Registry surface is now RegistryKey and stale platform docs corrected; surface retained because SPEC section9.1
    requires it. Main must populate event/command surfaces from bootstrap dispatch context. Files:
    core/src/diagnostics.rs; required bootstrap edits below.
19. The SDK exports serde_json::Value under two names, JsonValue — FIXED. Same complete alias removal as issue7.
20. Seven test mods still implement ravel_host_api::HostMod agai — FIXED. Same SDK cutover as issue8.
21. Canonical string spellings of public enums are exposed throu — FIXED in owned/unowned crates, central callers
    pending. Canonical accessor is as_str; wire strings preserved; render-stage snake_case rationale documented. Files:
    core/src/{sides,states,diagnostics}.rs, config/src/{declaration,schema}.rs, observability/src/profiling.rs,
    test-mods/{client-only,server-only}/src/lib.rs, network/src/lib.rs, pack/src/{error,validate/tests}.rs.
22. Examples and test mods still panic on packaging errors insid — FIXED. Fallible builders propagate original packaging
    errors through hooks and callers; literal invariants remain explicit. Files:
    examples/{content-mod,guide-snippets,stateful-mod}/src/lib.rs,
    test-mods/{waystone-ledger,inter-mod-service,saved-data,resources,registries}/src/lib.rs,
    docs/guide/{content,package-manifest}.md, conformance
    integration/{content_declarations,pack_closure,config_saves_and_services,saved_data}.rs, composed-host/src/lib.rs.
    HostError separately exported by SDK to name attributed service failures in fixtures.
23. With no SDK constructor for a mod refusal, each mod writes i — FIXED consumer side against target
    ModError::refused(detail). Redundant refusal helpers removed; deferred errors retained as Result<_,ModError> in
    RVL086/120/132 with regressions. Files: sdk docs/content, examples, newer test-mods, registry mods listed below.
    Host-api constructor implementation belongs to issue27 central lane.
24. ravel-config FileError::Io flattens io::Error and its path i — FIXED. Structured path/source and Error::source;
    filesystem regression checks original path/kind/source. File: config/src/file.rs.
25. Static enum spellings are exposed as `as_str()` on most type — FIXED in owned/unowned crates, central caller
    migration pending, same issue21.
26. The public generator entry points `run_rust`/`run_java` use — FIXED. Renamed
    generate_rust/generate_java/generate_java_and_inspect; adapter caller migrated, tooling caller assigned LaneTooling.
    Files: generator/src/lib.rs, adapter/build.rs, architecture doc.
27. ModError is the host's HostError re-exported, so mod authors — REASSIGNED to LaneLoader-2 by Main. SDK targets
    `pub use ravel_host_api::ModError`, detail-only constructor API, and separately exports HostError for service-call
    diagnostics. Central host-api change is intentionally absent here.
28. VersionAdapter is described as what a per-version adapter of — FIXED. Removed VersionAdapter and PinnedAdapter;
    expose pinned generated data and free target(id). Files: minecraft/src/lib.rs, minecraft-adapter/src/lib.rs. Exact
    bootstrap migration below.
29. Three independent JVM descriptor tokenizers exist, two of th — FIXED. Same consolidation as issue10.

## Cross-lane edits applied

- Registry refusal migration: rvl-{002,025,042,060,086,120,132,146,180,210,287,321}-counterpart/src/lib.rs;
  rvl002/src/data.rs, rvl120/src/census.rs, rvl180/src/dumps.rs. Registry notified to regenerate digests.
- Typed registry consumers: rvl-{002,025,042,086,120,132,210,321}/src/settings.rs; rvl-{042,086,120}/src/dumps.rs;
  rvl025/src/lib.rs. Registry notified source freeze. Regression tests added RVL002/025/042/086/321.
- Getter migration: loader/src/config.rs, host-runtime/src/lib.rs, pack/src/validate.rs.
- Conformance integration/{config_saves_and_services,content_declarations,pack_closure,saved_data}.rs;
  composed-host/src/lib.rs direct declaration callers.
- ravel-package/src/{compatibility,manifest}.rs, ravel-network/src/lib.rs, ravel-pack/src/{error,validate/tests}.rs,
  ravel-resolve/src/error.rs (Main explicitly authorized unowned blocking dependencies).
- ravel-minecraft-adapter/{build.rs,src/lib.rs}; docs/architecture/dependency-rules.md,
  docs/guide/{state,content,package-manifest}.md; ROADMAP.md; evidence/m4/sdk-surface.json inventory removal only.
- Cargo.lock updated by Cargo for generator serde_json and dependency removals.

## Central integration requirements (Main explicitly owns these)

- Final host-api provides detail-only ModError::{Refused{detail},Unsupported}, ModError::refused(detail), changes
  HostMod hooks, and loader wraps owner/phase. Current SDK compilation stops earlier at host-api/lib.rs:142,147 old
  LifecycleState::name; after migration the missing ModError is expected.
- Canonical accessor callers: bootstrap mods/profiling/settings, composed-host lib, host-api lib, host-runtime lib,
  loader admission/events/lifecycle/error, pack tests as necessary, testkit pipeline; registry rvl042 dumps Side::name,
  rvl120 dumps setting.side.name, rvl180 command side.name, rvl210 dumps side.name/Side::name; conformance
  noop_pipeline/pack_closure/pre_world_gate/profiling_report/sides_and_packets.
- StructuredFailure explicit `quotes_untrusted_text=false`: loader lifecycle/error LifecycleError, loader world test
  Unresolvable, bootstrap persistence/tests Unresolvable. Resolve production implementation migrated here.
- StageOrder::at typed callers: registry rvl146 highlights around507 and proposals rvl-gap-04-t1 around23 (import
  RenderStage).
- Option assertions: registry gap03t1 around37 Some("door_opens"), gap04t2 around21 Some("a_stage_nobody_publishes"),
  conformance sdk_gaps.
- FailureSurface bootstrap mods surface_of must render RegistryKey via Display (there is no as_str), and helper cannot
  remain const. Runtime record_failure should receive event/command surface from Work; persistence uses None.
- SavedDataMod declaration() now Result: bootstrap persistence/tests originally970
  `.declaration().expect("valid declaration").versions()`; originally1008 `.declaration().expect("valid declaration")`.
- Adapter exact replacements: remove `VersionAdapter` import; remove `PinnedAdapter` import. `PinnedAdapter.targets()`
  -> `pinned::TARGETS` (borrow or `.iter()` where appropriate); `PinnedAdapter.is_non_goal(id)` ->
  `pinned::NON_GOALS.contains(&id)`; `PinnedAdapter.minecraft_version()` -> `pinned::MINECRAFT_VERSION`; target lookup
  -> `ravel_minecraft_adapter::target(id)`. Bootstrap call sites: src/lib.rs test imports/targets check;
  src/runtime/tests.rs originally604 and624 two adapter coverage/version tests.
- Tooling generator caller renames/tokenizer reuse and deleted placeholder crate inventories are coordinated with
  LaneTooling.
- Makefile.toml comment around95 still says generate::run_rust; update to generate_rust (owned LaneTooling).
- Digest-bound evidence inventory changed by explicit issue9; do not hand-edit evidence digests.

## Verification

- `mise exec -- cargo fmt --all` passed; run again after final config review cleanup.
- `mise exec -- cargo clippy -p ravel-core -p ravel-config -p ravel-data -p ravel-observability --all-targets --all-features`:
  initially doc_markdown for snake_case; fixed with backticks, passed.
- `mise exec -- cargo nextest run -p ravel-core -p ravel-config -p ravel-data -p ravel-observability`: 217 passed, zero
  skipped.
- `mise exec -- cargo test -p ravel-core -p ravel-config -p ravel-data -p ravel-observability --doc`: 24 doctests passed
  including declaration compile-fail test.
- (command below):

  ```console
  mise exec -- cargo clippy -p ravel-minecraft-generate -p ravel-minecraft -p ravel-minecraft-adapter -p ravel-package --all-targets --all-features
  ```

  fixed unnecessary Result/unused import and borrowed adapter iteration; passed.

- `mise exec -- cargo nextest run -p ravel-minecraft-generate -p ravel-minecraft -p ravel-minecraft-adapter -p ravel-package`:
  first obsolete invalid fixture failed; replaced with parse regression; then package diagnostic expected old mod-id
  wording; updated to shared identifier wording. Final combined check below passed.
- (command below):

  ```console
  mise exec -- cargo clippy -p ravel-core -p ravel-config -p ravel-data -p ravel-observability -p ravel-minecraft-generate -p ravel-minecraft -p ravel-minecraft-adapter -p ravel-package --all-targets --all-features
  ```

  passed.

- (command below):

  ```console
  mise exec -- cargo nextest run -p ravel-core -p ravel-config -p ravel-data -p ravel-observability -p ravel-minecraft-generate -p ravel-minecraft -p ravel-minecraft-adapter -p ravel-package
  ```

  486 passed, zero skipped.

- `mise exec -- cargo clippy -p ravel-sdk -p 'ravel-test-mod-*' -p 'ravel-example-*' -p waystone-ledger --all-targets --all-features`:
  dependency failure first network Side::name; migrated. Retry dependency failure host-api LifecycleState::name
  at142/147, central-owned. No SDK/test-mod/example test success claimed.
- `mise exec -- cargo nextest run -p ravel-sdk -p 'ravel-test-mod-*' -p 'ravel-example-*' -p waystone-ledger`:
  dependency failure network Side::name before migration; final retry recorded below.
- `mise exec -- cargo run -q -p ravel-xtask -- package digests examples/minimal-mod`: dependency failure network
  Side::name. `... examples/minimal-mod --check`: dependency failure missing resolve quotes_untrusted_text (subsequently
  migrated). Cannot refresh other package digests until central xtask graph compiles; do not hand-edit. Required package
  dirs: examples/{content-mod,minimal-mod,stateful-mod},
  test-mods/{client-only,configuration,registries,saved-data,server-only,waystone-ledger}; registry source directories
  separately notified LaneRegistry.
- `ripwire crates/ravel-minecraft --quality-delta`: exit2 reports HookTarget::id dead-code under crate-local analysis;
  it is used by adapter::target and its verified test. The other finding is ambient test short-horizon churn. No real
  regression identified.

## Review

Read-only code review completed. Formatting observations fixed by cargo fmt. Canonical non-debug typed-access errors and
exact Display regressions added. Retained ValueError as explicitly requested by issue16 rather than adding a parallel
accessor error abstraction. Retained strict declared-kind lookup; documented string enums explicitly. Empty title
validation predates this change and is outside these issues; no extra schema policy was added.

## Final verification and boundary follow-up

- (command below):

  ```console
  mise exec -- cargo fmt --all && mise exec -- cargo clippy -p ravel-config -p ravel-network -p ravel-resolve -p ravel-pack --all-targets --all-features && mise exec -- cargo nextest run -p ravel-config -p ravel-network -p ravel-resolve -p ravel-pack
  ```

  passed; 161 tests passed, zero skipped, including exact typed error Display regression.

- Final `mise exec -- cargo nextest run -p ravel-sdk -p 'ravel-test-mod-*' -p 'ravel-example-*' -p waystone-ledger`:
  dependency failure at central-owned host-api/src/lib.rs142,147 LifecycleState::name. Missing target ModError is masked
  by these earlier compile failures.
- Final `mise exec -- cargo run -q -p ravel-xtask -- package digests examples/minimal-mod` and same command with
  `--check`: both dependency failure at host-api/src/lib.rs142,147. No digest was hand-edited; central integration must
  refresh/check all nine named package directories after merge.
- Main-requested boundary follow-up: examples/minimal-mod/src/boundary.rs imports actual existing
  `ravel_bootstrap_runtime::LoaderRuntime` and `ravel_bootstrap::compose`; correct two-component unresolved import error
  code E0432. tests/conformance/tests/integration/sdk_boundary.rs derives actual `[lib].name` with Cargo package-name
  fallback and regression assertions for both non-default library names. Library artifact names unchanged.
- Final `mise exec -- cargo fmt --all` after boundary regression landed: passed. Boundary harness execution remains
  central-integration dependent; no execution success claimed.
