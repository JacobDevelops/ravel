# Ravel handoff

This file preserves cross-session context, not a second project contract or a gate result.
Read `AGENTS.md` for repository rules, `SPEC.md` for required behavior, `ROADMAP.md`
for work status, and `docs/architecture/dependency-rules.md` for crate boundaries.
Session-specific lane reports and integration checklists were removed because their
unfinished-work claims contradicted later integration results. Current verification
must come from the integration owner's results for the current tree, not old counts.

## Runtime ownership context

- Bootstrap owns `Composition` and its installed-package versions. Runtime construction
  requires `LaunchContext`; installation consumes the composition and profiler and
  returns `InstallError` on refusal.
- Loader owns saved-data admission and callback attribution, outward-request dispatch,
  and game-pack replacement paths (`replaced_paths`). Bootstrap consumes these decisions;
  interception attribution remains in bootstrap because loader interception has no
  failure environment. `Round::refused_by` supplies save-veto ownership.
- Failed owners' saved payloads remain opaque. Installed package metadata does not
  substitute for the mod's actual readable saved-data declarations.

## Durable decisions and evidence boundaries

- SevTech: Ages 3.2.3 targets Minecraft 1.12.2 on the pinned modern JDK, with native
  behavioral counterparts rather than hosting Forge jars. Feather mappings, LWJGL3,
  and a replacement renderer are the selected direction. Keep the other supported
  version targets and historical ATM10 evidence.
- The placeholder test mods (`dependency-base`, `dependency-consumer`, `failure-cases`)
  were retained because M4 binds their inventory. The rvl-146 singleton was retained
  because its removal would alter M6-bound counts. SDK crate-root reexports remain.
- Historical evidence results and package source digests have producer-specific binding
  rules; consult their manifests and producer before changing them. Rejected historical
  changes are not new deletion opportunities.

## WP4 acceptance boundary

- Minecraft 1.12.2 remains generation-only even though official live acceptance
  passed (see below). Promotion is blocked on gate semantics, not on the port:
  with `live`, `ravel:test` runs the whole workspace suite at 1.12.2, and 6 of
  3,952 tests fail on modern-only obligations. Three `content_declarations`
  conformance tests compose `examples/content-mod`, which correctly declares
  `>=1.21.0, <27.0.0`. The adapter test
  `the_table_carries_both_a_method_and_a_field_target` requires an
  `InstanceField` hook target, and the legacy inventory has none. The xtask
  `java_shape` and `scaffold` reflection checks require `reflection.toml` to
  declare the modern `GameNames` roles used by shared `Content.java` and
  `Outward.java`, which the legacy source closure does not compile. Promotion
  needs live obligations gated on the `legacy-1-12` capability, not on eligibility.
- Keep native access limited to the named `dev.ravel.agent` module and the four
  existing LWJGL modules. Do not grant unnamed modules native access; narrator/JNA
  remains intentionally unsupported.
- Optional mod callback failures must remain attributed diagnostics rather than Java
  exceptions escaping into vanilla ticks. Every remote packet decoding refusal,
  including wrong-side or wrong-direction contract selection, must drop without
  failing its owner or halting the admitted set.
- Legacy resource bytes must be captured on the owning runtime thread before vanilla
  sound/resource workers open streams. Lazy native reads on those workers cannot use
  the thread-owned client runtime.
- The global block-state palette must size storage from the highest packed state ID,
  not only the number of registered states. Block ID 512 exceeds vanilla's initial
  thirteen-bit palette; the full twelve-bit block-ID space needs sixteen-bit state IDs.
- Legacy recipe sources are immutable for a launched composition. Identical reloads
  preserve registered recipe identities; replacement or duplicate declarations refuse
  without overwriting vanilla recipes.
- Refresh the reviewed hook, stub, and reflection pins together with generated locks
  before generating Java bindings. Feather omits invariant initializer mappings;
  their resolution still requires exact translated descriptors and concrete bytecode.
- Integration checks now pass for the legacy agent/screen build, official linkage
  scan (3,310 classes and 726 references), platform math/shutdown/loader checks,
  generator/runtime/package suites, and all-targets clippy. The full verification
  gate passes with 3,952 tests for each modern version and 92.93% line coverage.
  Generic transformation covers all 29 reviewed hooks, typed
  execution and descriptor refusals, palette call provenance, hierarchy,
  initialization, side selection, loader identity, and JNI semantic boundaries.
- All thirteen offline harness sections pass: transformation, refusal, hierarchy,
  initialization, side, load, cluster, content, members, outward, settings,
  screens, and persistence. Client-backed fixtures use the production loader's
  LWJGL2 reference rewriting with default JDK module roots resolved. Content
  includes actual widened global-palette storage and split-nibble high-state
  identity round trips. Native persistence covers dedicated and integrated
  compositions, migration, reopen identity, failure preservation, and dimensions.
- Legacy recipe fixtures use the official parser's unqualified crafting type
  names. Source digests are regenerated through the package producer. Settings
  exercise the exact mapped click override and native save/reopen admission with
  a client-directory fixture marker; native lock and invalid-value refusals pass.
- Reflection shape validation distinguishes field and method descriptors.
  Runtime dispatch uses generated adapter capabilities rather than version names.
- The port is split into four compiling changes: generated hook contracts, native
  runtime and test mods, Java bridges, and acceptance harness/tasks/docs. Each
  passes workspace all-targets/all-features clippy with warnings denied alone.
- Official live acceptance passes: `cargo make ravel:legacy-agent-live` exited 0 with
  an integrated session and reopen, a connected-dedicated session and a dedicated
  restart. All 29 hooks appeared in fresh logs, and persisted native visits advanced
  by exactly one on reopen and restart. `docs/runbooks/legacy-launch.md` records the
  durations and which hooks gameplay drove rather than acceptance probes. Four fixes
  found by the first live run are in the stack. Server-bound fixture packets decode
  through the official no-argument constructor and `Packet.read`, because dedicated
  jars strip the client-only constructor. F3+T holds F3 until the reload is observed.
  `live.py` reads the aggregate's `owners`. The legacy proof report writes to the
  stderr descriptor, because the game's redirected streams drop output after logger
  shutdown.

## Research pointers

- `registry/notes/sevtech/sevtech-plan.md`: work-package design and dependencies.
- `registry/notes/sevtech/sevtech-wp0-report.md` and `sevtech-rights-report.md` in that
  directory: input-specific provenance, rights research, and disposition history.
- `jvm-crate-audit.md`: dated JVM/library research at its stated snapshot, not current
  implementation status or an instruction to repeat already completed migrations.
