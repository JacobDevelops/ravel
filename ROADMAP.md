# Ravel implementation roadmap

Status: implementation-ready plan  
Authority: subordinate to [`SPEC.md`](SPEC.md)  
Forward initial-completion gate: WP9; historical M10 replay remains separate
Last updated: 2026-10-18

## Capability

Ravel enables a player to run a complete, natively compiled Rust modpack inside
official Minecraft: Java Edition without another Java mod loader. Forward proof
targets immutable SevTech: Ages 3.2.3 on Minecraft 1.12.2 and pinned JDK 27.
The build default remains 26.1.2 and 1.21.1 support remains. Initial completion
requires the native pack's automated compatibility suites, Linux performance
contract and complete pinned progression in a fresh Ravel world. This is a plan,
not a demonstrated playable SevTech result. Apple Silicon remains later work.
The Rust pack should cost significantly less RAM and CPU than the equivalent
Java pack; [`SPEC.md`](SPEC.md) §1 states that goal against §6's thresholds.

The `ravel` repository owns the reusable loader, SDK, package contracts,
tooling, test mods, sanitized evidence, and `registry/` compatibility sources.
Official fixtures and runtime worlds remain authorized, ignored local material.

## Constraints

These rules apply to every work package and milestone:

- Official Minecraft remains the initial host.
- Ravel loads Rust mods only and never uses NeoForge, Fabric, Forge, Quilt, or
  another loader as its bootstrap.
- Counterpart admission requires the loader's M3 semantics and workload-specific rights gates.
- Maintained Rust and mod-author code uses `#![forbid(unsafe_code)]`. Audited
  dependencies may encapsulate unavoidable native/JVM boundaries.
- No mod code runs before package validation and dependency resolution.
- The installed mod graph is fixed for the process lifetime.
- Minecraft version details remain behind a version adapter.
- The public SDK exposes safe Rust and does not expose generated JVM or
  Minecraft-version internals.
- Required-mod, side, registry, integrity, and saved-data failures fail closed
  before world access or further mutation.
- Credentials, launcher tokens, and other secrets never enter this repository.
- Milestones close on recorded evidence, not elapsed time.
- A nontrivial implementation change receives an independent review before its
  work package closes.

## Implementation contract

### Actors and surfaces

| Actor | Initial surface |
| --- | --- |
| Player | Prism launch, in-game mod/dependency view, generated configuration UI, Ravel worlds |
| Mod author | Safe SDK, package builder, test harness, documentation and examples |
| Pack author | Pinned pack manifest, declarative data, compiled Rust pack modules, validator |
| Maintainer | Reproducible environment, architecture experiments, evidence gates, diagnostics |

### Required lifecycle

The loader implements the state sequence defined by the spec:

`Discovered -> Validated -> Resolved -> Loaded -> Constructed -> Registered -> Running -> Stopping -> Stopped`

`Failed` is terminal and carries structured attribution. Every transition has a
unit-testable state-machine rule and an integration-testable process outcome.

### Initial-completion boundary

Forward Linux initial completion requires WP9 and its WP1–WP8 dependencies,
including full native parity, performance and live progression evidence.
Historical M0–M10 sections retain their original workload-bound criteria for
replay, not a second mandatory ATM10 proof for forward SevTech completion.
Apple Silicon platform expansion and official-server interoperability follow
forward Linux completion. M12 still requires a new capability amendment; M13
remains separately planned publication/registry work. No WP result retroactively
closes or reinterprets an M milestone.

## Non-goals

This roadmap does not authorize a native client, custom launcher, public
registry service, Java mod execution, automatic script translation, import of
existing Forge pack worlds, arbitrary Forge-wire interoperability, Windows support,
Intel macOS support, Linux ARM support, hot-loading, or a wholly Rust server.
The in-tree `registry/` is source and compatibility material, not authorization
to deploy the M13 service.

## Verification system

### Gate model

Every work package has:

1. an ID such as `M2-W3`;
2. declared inputs and predecessor IDs;
3. one accountable output;
4. automated checks where automation is possible;
5. an evidence manifest;
6. an independent review record when the change is nontrivial.

The repository-native verification and milestone closure commands are:

```text
cargo make ravel:verify
cargo make ravel:verify-milestone m<N>
```

The milestone task wraps:

```text
cargo run -p ravel-xtask -- verify milestone M<N>
```

M0-W2 creates these commands. `ravel:verify` is the code-policy graph, run
locally and by GitHub Actions after mise provisions the tools.

### Evidence contract

Each milestone produces `evidence/m<N>/manifest.json` conforming to
`schemas/evidence.schema.json`. The manifest records:

- milestone and work-package IDs;
- exact Minecraft, Prism, fixture, Java, mise, and Rust toolchain versions;
- platform, architecture, hardware profile, and relevant system settings;
- input artifact names and cryptographic hashes;
- commands executed, exit status, start time, duration, and log hashes;
- test counts, failures, coverage, and skipped-test count;
- benchmark protocol version and result file hashes;
- deviations, open risks, and blocking/non-blocking classification;
- reviewer identity or agent record and review result;
- manual attestation fields where automation is impossible.

Large raw captures and sensitive logs remain outside the repository. The manifest
stores sanitized summaries and hashes. Redaction tests run before a report can
be admitted.

### Global automated gate

Once M0 scaffolding exists, `ravel:verify` must run:

```text
cargo fmt --all --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-features
cargo test --workspace --doc
cargo llvm-cov nextest --workspace --all-features --fail-under-lines 80
cargo deny check
```

GitHub Actions runs `cargo make ravel:verify` with the tools from `mise.toml`
and the rustup-managed toolchain from `rust-toolchain.toml`.

Additional policies:

- No skipped or ignored tests are accepted in a milestone gate.
- The 80% line-coverage floor applies to testable maintained Rust workspace
  code. Generated bindings, benchmark-only spikes, and external process glue
  may be excluded only by a reviewed, explicit allowlist.
- Behavioural conformance is required even when line coverage passes.
- Linux x86_64 is the only required gate for M0 through M10.
- Apple Silicon macOS work starts after Linux M10 has passed with the loader,
  counterpart library, complete pack, constrained performance, and progression
  evidence already accepted. No macOS build, launch, capture, packaging,
  verification, or platform-specific investigation belongs in M0 through M10.
- GitHub Actions provides the Linux gate. Any future Darwin gate runs on an
  actual `aarch64-darwin` machine via `docs/runbooks/darwin-verification.md`;
  a Linux evaluation never substitutes for required Mac evidence once that
  post-M10 phase begins.
- A gate rejects credential patterns, forbidden loader artifacts, or maintained
  Rust that permits unsafe code.

### Performance evidence

M0 records enough Linux launch evidence to prove vanilla 26.1.2 and official
ATM11 0.2.1 run from the pinned Prism setup without credentials entering the
repository. It does not require performance capture
or repeated launches. Those are the versions M0 measured, and `evidence/m0/manifest.json`
pins them; the proof workload was later retargeted to ATM10 To the Sky 2.0.2 on
Minecraft 1.21.1, and a closed milestone's record describes the material it
measured rather than the target that replaced it.

M9 requires an explicit single-pair performance protocol covering the canonical
world, route, warm-up, duration, graphics and simulation settings, active content,
sampling intervals, and pass/fail calculations. The former in-repository
performance collectors and measurement tasks are retired; their removal does
not establish that M9 passed or waive its acceptance criteria.

The fixed M9 values come from the spec:

- native Linux x86_64 reference hardware;
- single-player with integrated server;
- 3440 x 1440 output;
- 8-chunk render distance;
- no shaders;
- sustained 30 FPS under the canonical workload;
- at most 8 GiB steady-state process RSS;
- at most 10 GiB transient process RSS;
- no sustained memory-pressure warning or swap thrashing;
- recorded median, p95, and p99 frame time and integrated-server tick time.

The profile must define “sustained,” “steady-state,” “transient,” “warning,” and
“thrashing” numerically before M9 acceptance evidence is accepted. M9 runs one
official-pack comparison and one Ravel-pack acceptance under the same profile
hash. It does not require repeated runs or claim run-to-run reproducibility.

## Delivery dependency map

```text
M0 Linux launch smoke proof
 └─> M1 architecture evidence
      └─> M2 loader skeleton
           └─> M3 credible Linux loader
                ├─> M4 general SDK breadth
                │    └─> M5 compatibility inventory
                │         └─> M6 foundation counterparts
                │              └─> M7 complete counterpart library
                │                   └─> M8 complete pack
                │                        └─> M9 Linux performance
                │                             └─> M10 progression acceptance
                │                                  └─> Linux initial completion
                └─> data-declared configuration and Java UI before M3 closes

M10 └─> server protocol specification └─> M11 official-server interoperability
M10 └─> post-Linux-M10 Apple Silicon platform expansion
M9 failure └─> host viability review; it does not relax M9 or authorize a native client
M12 and M13 require separately approved post-initial scope
```

## Forward SevTech work packages

This sequence supersedes ATM10 as the forward workload, not as historical
evidence. Preserve each record's actual inputs, results, counts, digests and
verdicts: M0–M5 measured ATM11 0.2.1 (235 inputs), M6 measured ATM10 To the Sky
2.0.2, and M7 is still open. Explicit replay selects the record's fixture:
M0–M5 use `atm11-0.2.1`; M6 uses `atm10-tts-2.0.2`.
Historical unclosed M7–M10 retain their original criteria and protocols for
replay, separate from forward WP closure. WP9 is forward initial completion.
WP IDs do not imply that `verify-milestone` implements WP closure commands.
Fixture-bound v2 library evidence may support forward work; it neither closes
historical M7 nor changes its v1 324-or-evidence-correction replay criteria.

WP0 has pinned the release inventory and initial rights ledger; it has not
resolved every right, runtime container or live baseline. The immutable archive,
publisher file 3569915, is bound by the SevTech descriptor and provenance.
Keep **254 references**, **5 bundled jars**, and **259 total jar inputs**
separate; loaded containers remain unknown until observed. Baseline worlds,
seeds, release overrides, scripts, scenes and hardware must be pinned before
comparisons. Detailed design and risks live in
[`registry/notes/sevtech/sevtech-plan.md`](registry/notes/sevtech/sevtech-plan.md).

WP0 exit still requires **reproducible legacy graph derivation
(mcmod.info / @Mod extraction)**. The recorded one-off extraction is not a
replayable producer; phase checks report this obligation as not yet met rather
than treating its structural evidence as a reproduced graph.

| Package | Dependencies and output | Required evidence gate | Demonstrated status |
| --- | --- | --- | --- |
| WP1 — mapping generalization | WP0; tagged mapping formats/namespaces and capability-selected optional DFU | Resolve real pinned 1.12.2 class bytes; descriptor, overload, inheritance, namespace, ambiguity and wrong-hash cases; stable locks and unchanged existing-version semantics | Acceptance not demonstrated; mapping changes are proposed capability, not live support |
| WP2 — complete 1.12.2 version data | WP1; adapters, hooks, reflection, stubs and version matrices | Authenticated client/server/mapping inputs; Java/lock generation, adapter and harness matrices for 1.12.2, 1.21.1 and 26.1.2; cross-version/hash rejection | Acceptance not demonstrated; generation alone is not play |
| WP3 — JDK27/LWJGL3 boot | WP2; own minimal platform/runtime replacement, no hosted Forge jars or LWJGL2 | Exact pinned JDK27 client reaches menu, creates/reopens world, handles resize/fullscreen/input/audio and exits; dedicated server saves/stops without client natives; bounded module/native-access grants | Live acceptance not demonstrated |
| WP4 — semantic hook ports | WP2; live closure depends on WP3 | Stable numeric IDs and metadata, explicit registration barrier, exactly-once ticks, bounded/main-thread networking, legacy resource/recipe reload and WorldSavedData save/reopen/migration/failure preservation on both sides | All thirteen offline harness sections pass, and official integrated session/reopen plus connected-dedicated/restart agent acceptance passes with all 29 hooks; 1.12.2 is live and included in the workspace test matrix, with capability-appropriate obligations |
| WP5 — renderer/model memory pipeline | WP3 and WP4 barriers; LWJGL3 with capability-selected GL, bounded native meshes and Java-owned submission | Golden opaque/translucent/animated/staged scenes; chunk/reload/unload correctness, frame-time tails, upload/compile time, RSS/native/VRAM and first-use stalls on disclosed GPUs | Slices 1–8 established bounded emission, residency, native translucent sorting and offscreen/replay parity. Earlier slice 5–8 selected physical trials measured proof overhead, not ordinary-world replacement. The diagnostic-only resident-world path now captures compiled sections into generation-keyed native/GL tables: at most 4,096 residents, 64 native pending entries, render-distance budgets of 16–256 MiB and distance eviction. Current acknowledged SOLID/CUTOUT_MIPPED/CUTOUT sections replace vanilla VBO draws; fluids, stale/pending/over-budget sections and TRANSLUCENT stay vanilla. Twelve software-GL normal-world triplets span four distinct XYZ positions before reload, after reload and after reopen: four replaced draws versus zero to two opaque and zero to one translucent fallback draws/frame. All three arms share one client tick; an independent raw recount found zero strict RGBA/coverage/depth violations and positive omission footprints. Recorded native/GL peaks were 4,974,472/5,750,640 bytes against separate 16,777,216-byte budgets. Actual acknowledged-then-dirty and acknowledged-then-pending probes increased their vanilla-fallback counters. Selection remains diagnostic-only/refused otherwise. Production translucent replacement and physical trials remain; default cutover, full-world parity and renderer savings are unproven. Vanilla remains the default |
| WP6 — measured optimization | Usable WP3–WP5 baseline, representative WP8 content | Cold/warm startup, exploration and machine/stage scenes; tick/frame distributions, GC, CPU, total memory and ablations; disclose cache eligibility and runtime differences | No resource/performance win demonstrated |
| WP7 — workload/evidence cutover | WP0 ledger and WP2 runtime descriptor | Exact archive/hash and 254+5 extraction; tamper rejection; workload-aware denominators; correct schema/default/registry selection; explicit ATM10 replay, no inherited rights or denominator 324 | Cutover work is not closure; integrated acceptance evidence pending |
| WP8 — rights-cleared native counterparts | WP0 rights/graph and WP4; WP5/WP6 where required | Wave A platform/dependency roles, B first playable progression, C stress anchors; per-package provenance, rights, behavioural and negative scenarios; then disposition every release input and prove complete progression | Starting waves and full parity not demonstrated |
| WP9 — live proof and claim gate | WP3–WP8, every input dispositioned | Actual official 1.12.2 on exact JDK27 through premain/JNI; fresh-world complete progression, late-age scenes, save/reopen, reload, integrated/dedicated paths, all three version matrices and independent observations | Full proof, fastest-loader and lowest-resource claims not demonstrated |

Feather (CC0) becomes canonical only after pinned-artifact coverage against
official jars; Legacy Yarn is fallback, not a blended namespace. No MCP
human-name CSV import. Study Cleanroom/LWJGLX, but own the minimal runtime rather
than adopting them wholesale; any code reuse requires exact licence review.
Rights are never waived. Conditional, prohibited or unresolved material must
retain its restrictions; a critical unresolved implementation basis blocks full parity.

WP9 compares the actual optimized original pack on a suitable supported JVM,
a current-Java comparison where compatible, and Ravel on equal content,
scenes/settings/hardware. Disclose unavoidable runtime differences and empty-loader
cost separately. Broader repeated measurements and ablations belong to forward
WP6/WP9 claims; historical M9 single-pair protocol text is not evidence of those
measurements. No invented numeric improvement result or silently reduced content
may satisfy this gate.

## Desired end-goal repository

### Single-repository `ravel` tree

The selected runtime mechanisms are described in the
[architecture overview](docs/architecture/overview.md#mechanisms).

```text
ravel/
├── .cargo/                        # Cargo aliases and target policy
├── .config/                       # nextest profiles
├── .github/                       # Linux CI gate
├── crates/                        # runtime, SDK, version adapter and build libraries
├── tools/                         # xtask verification, code generation and host tools
├── jvm/                           # agent, screen templates, harness and legacy platform
├── minecraft/                     # pinned per-version adapters and reviewed hook inputs
├── registry/                      # counterparts, packs, workload gates and source records
├── schemas/                       # public document schemas
├── test-mods/                     # executable SDK capability fixtures
├── tests/                         # conformance suites and public/local fixture inputs
├── examples/                      # minimal, content, stateful and compiled guide examples
├── docs/                          # architecture, author guides and operating runbooks
└── evidence/                      # sanitized milestone manifests and summaries
```

### Crate responsibilities and dependency rules

The table records ownership, not a second dependency manifest. Cargo manifests define
direct dependencies; [dependency rules](docs/architecture/dependency-rules.md) constrain
their direction, and the conformance `sdk_boundary` test pins the SDK's transitive closure.

| Crate | Owns |
| --- | --- |
| `ravel-core` | Canonical IDs, content IDs, versions, paths, sides, lifecycle, precedence ordering, diagnostics and redaction |
| `ravel-package` | Manifest parsing, compatibility ranges, source declarations, source-file hashes and provenance |
| `ravel-resolve` | Dependency edges, cycles, side checks and deterministic load plans |
| `ravel-loader` | Discovery-to-stop orchestration, registry barrier, failure policy and world-access gate |
| `ravel-sdk` | Safe public mod-author facade and author-facing types |
| `ravel-sdk-macros` | Compile-time SDK declaration generation |
| `ravel-host-api` | Source-compiled package execution boundary composing the config/data/save/network contracts |
| `ravel-bootstrap-runtime` | Safe JNI integration, installed-runtime hook dispatch, outward-action queue, game resource/data pack writing, persistence and settings crossings |
| `ravel-renderer` | Owned section and baked-model values, CPU geometry/visibility, bounded worker admission, cancellation, completions and drain; no JVM or GL |
| `ravel-host-runtime` | Package admission, configuration and resource composition (`compose`) and host lifecycle |
| `ravel-composed-host` | JVM harness host composing the fixed test-mod set with the `ravel_bootstrap` library name |
| `ravel-build` | Player-local host planning, source composition and compilation |
| `ravel-minecraft` | Version-neutral handles, hooks and pack-format metadata |
| `ravel-minecraft-adapter` | Generated per-version hook target table, lookup and admission environment identity |
| `ravel-minecraft-generate` | Rust bindings, generated Java sources and pinned locks from version data |
| `ravel-config` | Schemas, validation, side/restart flags and layered overrides |
| `ravel-data` | Content and audio declarations, pack generation, tag/resource merging, server data and render-stage ordering |
| `ravel-save` | Ownership, schema versioning, migrations and compatibility checks |
| `ravel-network` | Ravel packet/channel abstractions and side rules |
| `ravel-observability` | Structured reports, metric schemas and profiling coordinator |
| `ravel-pack` | Pack pins, declarative composition and closure validation for library callers; written pins are not enforced by runtime launch |
| `ravel-acceptance` | Declarative game acceptance scenarios, assertions and transcripts |
| `ravel-testkit` | Deterministic library lifecycle pipeline and JVM harness protocol parsing and validation |

Dependency direction is inward toward contracts:

```text
SDK/test mods ───────> public contracts and feature schemas
loader ──────────────> contracts + resolver + host-api
shared adapter ──────> version data + version-neutral Minecraft traits
host cdylibs ────────> host runtime + bootstrap JNI integration
xtask/testkit ───────> libraries; libraries never depend on tools
```

Cycles between crates are forbidden. A new crate is justified only by an
independent stability, safety, version, test, or distribution boundary. Until
that boundary appears, code stays in the nearest existing crate.

### Safety boundary

Every maintained crate has `#![forbid(unsafe_code)]` at its root. The workspace
gate verifies this. JNI, JVMTI, native loading, operating-system calls, and
generated boundary code must be provided by audited dependencies with safe
wrappers.

M1 set out to admit one exception — the exported JNI symbol — and the M1-W8 review
refuted the premise: `jni` 0.22.4's `jni_mangle` attribute emits the export, and a
crate keeping `#![forbid(unsafe_code)]` verbatim compiles and exports the mangled
symbol. No exception is needed and none exists;
[`safety-boundary.md`](docs/architecture/safety-boundary.md) records an empty list.
The safe boundary relies on audited dependencies encapsulating those operations.

### Version boundary

Ravel supports more than one Minecraft version, so the rule is stated over
versions rather than over one of them. Only `minecraft/versions/<version>`,
generated outputs derived from it, and adapter-specific process fixtures may
encode that version's mappings or hook names. `ravel-minecraft-adapter` is
shared across supported versions and selects version data explicitly.

`verify_version_boundary` in [`tools/ravel-xtask/src/version_boundary.rs`](tools/ravel-xtask/src/version_boundary.rs)
enforces that. It reads the directory names under `minecraft/versions/` and
under `tests/fixtures/public/versions/` — the pinned version and M4-W6's
synthetic second one — and fails on any file under `crates/` that spells one of
them, so the rule follows the version data instead of naming a version. It fails
the same way on a game class name taken from version data, unless that file and
that class are listed in `VERSION_CLASS_EXCUSES` with a reason, and it rejects
any `ravel-minecraft-*` crate other than `adapter`. The sweep reads `crates/`
only; `tests/` and `test-mods/` are excluded by design, and the failure text says
so.

Adding a second version must normally require only a new version-data directory
and generated outputs, not a new crate or changes to package identity,
dependency resolution, lifecycle, or ordinary mod source. A per-version crate is
needed only if evidence demonstrates that separately compiled types, artifacts,
dependencies, or safety boundaries require it. M4 proves the default
model structurally with synthetic second-version data; shipping that version is
not required.

### Registry and local-material boundary

`registry/` is part of this checkout. Its `mods/*` crates belong to the root
Cargo workspace: there is one `Cargo.lock` and one target directory, with no
nested registry workspace. Registry checks resolve the repository root and
consume its SDK, schemas, and tools. Cargo path dependencies remain inside
the checkout.

The registry includes counterpart sources, pack declarations, inventory,
specifications, provenance, and original check fixtures. It does not include
official fixture archives, extracted upstream inputs, runtime worlds, scratch
output, or credentials. Authorized fixture material stays ignored under
`tests/fixtures/local/`; evidence contains
sanitized summaries and hashes rather than raw captures or secrets.

Compatibility work uses the public SDK and package contracts. General SDK gaps
become capability proposals with original test-mod acceptance cases. Colocation
does not permit counterparts to depend on loader internals.

The tracked registry sources are distinct from the public publication service
planned for M13. Their presence neither selects its distribution/identity
mechanism nor authorizes publication. See the
[boundary record](docs/architecture/public-private-boundary.md).

## mise and cargo-make design

`mise.toml` pins development tools and sets the default Minecraft selection,
Cargo colour, and backtraces. Run `mise install` once, then activate mise in
the shell or use `mise exec -- <command>`. Rust remains rustup-managed:
`rust-toolchain.toml` tracks the nightly channel with edition 2024 and nightly Cargo
lint support; `rust-version = 1.95` in `Cargo.toml` declares the minimum.
Cargo remains the Rust build system.

`Makefile.toml` is the canonical task surface: `cargo make ravel:verify` runs
the code-policy graph locally and in `.github/workflows/ci.yml`.
`cargo make ravel:verify-milestone <id>` checks milestone closure separately.
Individual task names keep the `ravel:` prefix. Formatting covers Rust,
shell, TOML, and Markdown; lint covers clippy, shell, and workflow checks.

Host composition is owned by `ravel-build`; its command-line adapter and fixture operations run through `ravel-xtask`:
`cargo make ravel:host-build <instance> <cache> <minecraft>`,
`cargo make ravel:fixture-reconstruct`, and `cargo make ravel:fixture-verify`.
Registry validation uses `cargo make ravel:registry-verify`.

The host also supplies coreutils, git, curl, archive utilities, and pkg-config.
macOS hosts need Bash 4+ and GNU findutils/coreutils from the system package
manager (for example, `brew install bash findutils coreutils`), with their
binaries ahead of the system tools on PATH: the task scripts use GNU `find` and
Bash's `mapfile`. A development JDK is not evidence of the game's runtime:
record the Java runtime actually used by the pinned Minecraft/Prism workload.

Development/test commands support reproducible invocation and handoff to a
Prism-managed instance. They do not own authentication, Minecraft installation,
instance management, pack acquisition, or launcher UI.

## Milestone roadmap

### M0 — Linux launch smoke proof

**Purpose:** establish reproducible tooling, legal fixture boundaries, and
Linux launch proof before choosing architecture. Performance capture begins
only with the single M9 comparison pair.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M0-W1 | Initialize Cargo and mise roots | none | root manifests, pinned tools, rustup toolchain | `cargo make ravel:verify` passes on Linux |
| M0-W2 | Create verification and evidence skeleton | M0-W1 | `ravel-xtask`, evidence/schema contract, base CI | a deliberately incomplete M0 bundle is rejected; a synthetic complete bundle passes |
| M0-W3 | Define fixture manifest and secret protection | M0-W1 | pinned metadata, source descriptors, secret rules | secret canaries are rejected from the tracked tree |
| M0-W4 | Implement fixture reconstruction and hashing | M0-W3 | CLI command and local-only fixture output | exact MC/NeoForge/ATM versions, expected reference count, and all recorded hashes verify |
| M0-W5 | Specify later performance protocol shape | M0-W2 | versioned workload/profile and collector schemas | schema and dry-run collector tests pass on Linux; no native repeated capture is required |
| M0-W6 | Record Linux hardware, Java, Prism, and launch inputs | M0-W3 | sanitized Linux hardware profile and exact runtime metadata | clean-machine runbook reproduces vanilla launch on Linux |
| M0-W7 | Prove Linux vanilla and official ATM11 launch | M0-W4..W6 | Linux launch result bundle and workload attestation | vanilla and official ATM11 reach a playable world through Prism; credentials are absent from evidence |
| M0-W8 | Define M1 architecture measurement gate | M0-W7 | reviewed metric and failure-action plan | every M1 candidate reports comparable Linux memory, startup, call-cost, allocation, packaging, and failure evidence without requiring an M0 baseline |
| M0-W9 | Independent milestone review | M0-W1..W8 | review record and M0 evidence manifest | `cargo make ravel:verify-milestone m0` passes; tracked-tree audit is clean |

A failed hash, count, or redaction check blocks the gate.

### M1 — Architecture spikes

**Purpose:** compare JVM entry, mod execution, bindings, state ownership, and
Prism integration with evidence rather than assuming a runtime.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M1-W1 | Build common spike harness | M0 | shared benchmark protocol and trivial callback workload | every candidate reports the same metric schema |
| M1-W2 | ADG-1 JVM entry/hook comparison | M1-W1 | candidate code and Linux data | bootstrap RSS, retained classes, startup, callback, bytes and allocation metrics are measured on Linux; rejection is explicit |
| M1-W3 | ADG-2 Rust execution/ABI comparison | M1-W1 | dylib/Wasm/process/static measurements and tradeoffs | memory, p50/p95/p99 call cost, crash isolation, packaging, debug and evolution rubric complete. Selection was deferred to M3 because it turns on loader properties M1 could not observe. |
| M1-W4 | ADG-3 binding strategy comparison | M1-W2 | extraction/codegen/cache prototype | a generated 26.1.2 binding is reproducible from pinned inputs and fails on drift |
| M1-W5 | ADG-4 state-ownership experiment | M1-W2..W4 | representative object-flow benchmark | duplicated state and boundary traffic are measured under the canonical spike |
| M1-W6 | ADG-5 Prism integration spike | M0-W6, M1-W2 | stock launch proof or patch estimates | official Minecraft launches without another mod loader; auth tokens are absent from captures |
| M1-W7 | Architecture synthesis | M1-W2..W6 | overview and dependency/safety boundary docs | all selected and rejected options trace to Linux evidence |
| M1-W8 | Independent architecture review | M1-W7 | reviewer, result, and record hash claimed in the M1 manifest, and the record tracked beside it at `evidence/m1/review.md`, whose digest the gate recomputes and whose stated verdict the gate requires `review.result` to match | reviewer confirms safe-Rust feasibility, and confirms the per-crossing costs and the rate at which they would become material; M1 gate passes |

If safe wrappers cannot support a viable result, work stops for an explicit
specification change.

### M2 — Loader skeleton

**Purpose:** prove the complete pre-world loader pipeline with one no-op Rust
package, prebuilt as a binary because M2 ran before source distribution was
selected.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M2-W1 | Implement canonical IDs, sides, versions and diagnostics | M1 | `ravel-core` | unit/property tests cover valid and invalid values |
| M2-W2 | Implement package schema and integrity | M2-W1 | `ravel-package` and JSON schema | valid packages round-trip; tamper and schema failures precede code execution. The original target model predated source distribution; schema v1 now requires integrity-protected portable source files and refuses the old target form. |
| M2-W3 | Implement dependency resolution and load plans | M2-W2 | `ravel-resolve` | required/optional/incompatible/discouraged, cycles, duplicates, ranges and deterministic-order cases pass |
| M2-W4 | Implement lifecycle state machine | M2-W1 | loader state machine | every legal and illegal transition plus every spec invariant is tested |
| M2-W5 | Implement selected bootstrap and adapter shell | M1, M2-W4 | bootstrap, host API, shared adapter plus 26.1.2 version data | official 26.1.2 process reaches Ravel without another loader |
| M2-W6 | Package and execute no-op test mod | M2-W2..W5 | source-only no-op package | recorded evidence shows validate/resolve/load/construct/register/run/stop in deterministic order |
| M2-W7 | Add world-access and structured-failure gates | M2-W3..W6 | pre-world UI/log diagnostic path | invalid packages, sides, integrity and graphs cannot open a world; reports redact secrets/paths. The gate answered that question with no caller until 2026-08-09; the world-open crossing now asks it, and the note below says which half of it a launch can reach |
| M2-W8 | Measure loader overhead and set regression budget | M2-W6..W7 | Linux no-op loader benchmark and review | the measured loader cost establishes the numeric regression budget used from M3 onward; M2 gate passes. Three of its six limits have since expired — see below |

**M2-W7's gate is now consulted by the game process, and only one of its two
halves can refuse there.** `LifecycleMachine::open_world` had no caller outside
tests, so "cannot open a world" was proven as *the loader answers no when asked*
and nothing in a launch asked. The world-open crossing asks it now:
`crates/ravel-bootstrap-runtime/src/persistence.rs` consults the gate before it touches a
byte of world state, and a refusal becomes the diagnostic the Java adapter
refuses the load on. The machine's half is consulted on that crossing in a launch,
and it has not yet refused in one: no recorded launch has a required mod fail, and
the single recorded world refusal came from the saved-data version check inside
`SavedDataAggregate::open` rather than from `open_world`, which admitted every
time. What a launch shows is that the gate is asked; that a halted set withholds a
world is proven in-gate by tests that fail when the call is removed. The report's
half is carried
from `install` and holds nothing in a launch, because the pipeline that validates
a package and orders a graph runs when a pack is compiled rather than inside the
game process. So an invalid package cannot reach a launch to be refused in one,
and the refusals `pre-world-gate.json` records remain the loader's own answers.

**M2's closure review returns PASS.** [`evidence/m2/review.md`](evidence/m2/review.md)
holds the cold audits of this gate, bound by digest, and `review.result` is `pass`
on the last of them, which reviewed the repaired state and killed by mutation every
load-bearing claim in the four rows the round before it never examined. The review
that originally closed M2 was recorded by title against a digest matching no
tracked file; both it and the `pass` it carried stay withdrawn, and the pass
recorded now is a different audit's. What that pass rests on is bounded, and the
passing audit is what bounded it: closure is an attestation gate, so the gates bind
which text was attested rather than that the attestation was earned. The audit
confirmed by breaking that four coordinated string edits close M2, and that
flipping only the manifest is refused. Its own reading-only ceiling, including the
launch it did not run, is in the record.

**Three of that budget's six limits have expired, on the budget's own terms.**
[`loader-overhead.json`](evidence/m2/loader-overhead.json) carries a
`budget.expiry` clause stating that its three build limits describe a linked
pack, and that if M3 selected anything other than linking they "describe a shape
that no longer exists and are remeasured rather than carried". M3 selected source
compiled on the player's machine, which is not linking, so the condition fired
exactly as written. `build.packCompile`, `build.relink` and
`build.binaryGrowthPerMod` are void.

This is the record executing a rule it was written with, not a budget being
relaxed because it became inconvenient. The record itself is unchanged and stays
that way: a recorded measurement is not edited after the fact, and the clause
inside it is what does the voiding.

**No remeasurement is owed.** The build cost of source distribution is
[`pack-compile.json`](evidence/m3/pack-compile.json), which already exists and
prices a pack-sized set of mods with real bodies at 113.34 ms per mod against the
3.44 ms the M2 figure implied.

**The three loader limits are unaffected and stand.** Their metric is pinned to
the no-op package, which registers no content, so the registry barrier M3-W1
added cannot contribute to it and a later run remains a like-for-like comparison.

One thing worth carrying into any future budget: those build figures were
declared floors in the record's own `whatThisDoesNotSay` and then used as limits,
and a headroom factor calibrated against run-to-run variance cannot absorb a
category error about what was measured.

### M3 — Credible loader prototype

**Purpose:** demonstrate useful loader semantics with original test
mods on Linux. The target-platform expansion waits until after Linux M10.

**Configuration is declared in data files**, the settings screen is drawn in
Java, and Rust supplies the values; Ravel does not render from Rust. M3-W5 uses
the same drawing mechanism rather than a second one. Pack overrides apply per
setting. The [architecture overview](docs/architecture/overview.md#mechanisms)
describes this mechanism.

**M3 selected source distribution.** Mods are independently shipped and interchangeable and a
pack is not one binary, decided on what players expect of a mod set rather than
on a measurement. The execution mechanism is decided with it: **a mod is
distributed as Rust source and compiled on the player's machine by a toolchain
Ravel ships**, and no compiled artifact is accepted — not an rlib and not a
dylib.

What decides it is not performance. Compiling every mod commits Ravel to no ABI,
so `HostMod`, `Registrar` and the handle types stay free to change. No model that
links mods against `ravel-minecraft` can say that, because each one pins a
version of `jni` into every mod's dependency graph permanently and unremovably.
Source-only is also the reversible choice: binary support can be added later, and
a compatibility surface cannot be withdrawn once mods depend on it. The costs
accepted are a toolchain of about 196 MB, a build cache that has to stay
resident, and Ravel owning compile failures as a support burden.
[`pack-compile.json`](evidence/m3/pack-compile.json) is what prices them.

The package-schema reconciliation is now closed as a breaking schema-v1-in-place
migration. `targets` is no longer admitted; `source` declares a repository and at
least one portable source file with its SHA-256 digest. The old mismatch remains
part of the M2 execution-model history.

M3 uses opaque handles to objects the *game* owns, which lets the generator carry
real hook targets without mirroring game state. Three artifacts enter the game
process rather than two: the native library, agent jar, and separate screen jar.
The always-present agent jar declares no game type while naming some reflectively.

**M3-W6's budget is decided rather than derived.** Every limit in
[`loader-overhead.json`](evidence/m2/loader-overhead.json) is its own measurement
doubled. The normal-mode profiling limit is not: profiling being off costs at
most one per cent of a tick, which at 20 TPS is 500 µs per tick summed across
every crossing in it, and that number was fixed before anything was measured.
What a player is promised is that profiling costs them nothing when it is off,
and that promise is a fraction of a tick rather than whatever today's code
happens to do — so a measurement far under the limit is the expected result and
not a sign the limit is wrong. It is stated per tick rather than per crossing so
that it does not wait on the per-tick crossing count, which nothing has recorded
yet; [`profiling-overhead.json`](evidence/m3/profiling-overhead.json) carries the
crossing count it assumed, says it is an assumption, and carries beside it the
count at which the budget would actually be reached. Profiling-*on* is recorded
there and deliberately not bounded: what the collectors cost while they run
depends on how many mods and collectors are attached, and M9 is where that
number meets a real pack.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M3-W0 | Grow the 26.1.2 hook inventory to M3's targets | M2 | resolved target record, `hooks.toml` entries, re-pinned `adapter.toml` hash and regenerated `mappings.lock` | targets are resolved against the pinned jar by two independent passes and recorded in [`m3-hook-targets.md`](docs/architecture/m3-hook-targets.md); the inventory edit that lands them is separate and is what the hash re-pin and lock regeneration gate |
| M3-W1 | Implement registry barrier and content registration | M2 | block/item/recipe test mods | duplicate/conflicting claims fail; IDs are immutable after registration |
| M3-W2 | Implement commands, events, resources and data | M3-W1 | command/event/resource test mods | process tests exercise success, ordering, side and failure cases |
| M3-W3 | Implement configuration and generated UI | M2 | configuration schema edits, layered config, generated Java screen and a screen test mod | file/UI edits round-trip; validation, sides, restart and per-setting pack overrides pass. The agent appends the sibling screen jar through `Instrumentation.appendToSystemClassLoaderSearch`, so artifact loading no longer blocks this work |
| M3-W4 | Implement versioned saved data | M3-W1 | save test mod and world fixture, over the deciding half `ravel-save` already models — what a version is, who owns it, and whether a world may be opened | the pinned fixer-byte measurement preserves opaque bytes; create/save/exit/reload preserves state; schema, package, migration and size incompatibilities fail closed. What remains is moving the persistence aggregate through opaque game-object handles |
| M3-W5 | Implement in-game mod/dependency surface | M2-W7, M3-W3 | mod list and diagnostics UI, over the model `ravel-loader` already holds ordered by resolved load order | valid and invalid graphs render deterministic actionable messages. Drawing uses the shared Java screen mechanism, so what remains is a screen and not a design |
| M3-W6 | Implement opt-in profiling | M2-W8 | collectors and profiling UI/report | per-mod attribution accuracy and unsupported metrics are explicit; normal-mode budget passes |
| M3-W7 | Linux process matrix | M3-W1..W6 | Linux result bundle | every required surface passes natively on Linux; instance scan proves no other loader |
| M3-W8 | Independent prototype review | M3-W7 | review record | M3 gate passes and authorizes compatibility inventory work only after M4 |

Where a row above says work already exists, that describes what is written, not
what has passed. A row is closed by its Verification column and by
`ravel:verify`, never by a file being present.

### M4 — General SDK breadth

**Purpose:** turn prototype internals into a documented, safe, reusable mod and
pack authoring surface.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M4-W1 | Stabilize experimental SDK facade and macros | M3 | `ravel-sdk`, macros, minimal example | compile-pass/fail suite proves authors need no loader internals or Java. The boundary is derived from the dependency closure `Cargo.lock` resolves for `ravel-sdk` rather than asserted, so the gate recomputes what an author may reach |
| M4-W2 | Complete registries, tags, resources and generated data | M4-W1 | public APIs and content example | conformance suite covers declarations, conflicts and deterministic output, against the loader's own pipeline. No game process registers content in M4 |
| M4-W3 | Complete hooks, events, sides and networking abstractions | M4-W1 | public APIs and client/server test mods | client-only code never loads on server sides; packet contracts round-trip. The row closes on the abstractions and not on delivery: a mod cannot send a packet, because nothing carries one, and the shipped host read no manifest, so no mod's declared sides were honoured by anything that runs. Both were disclosed in `docs/guide/limits.md` and recorded in [`evidence/m4/manifest.json`](evidence/m4/manifest.json); the sides half was repaired after the milestone — the composed host now admits its members through `ravel_loader::admitted` — and `docs/guide/limits.md` states what is left |
| M4-W4 | Complete config, save migrations and inter-mod services | M4-W1 | public schemas/APIs and examples | migration chains, service dependencies and failure closure pass. Composition stays fixed: the shipped host resolves a constant set of mods against a constant configuration path, so a settings document from a package it does not know is never read |
| M4-W5 | Complete pack declarations and validator | M4-W2..W4 | `ravel-pack`, pack example | dependency, resource, registry and progression closure checks pass, and the pack example runs as its own process in the gate with the written tree compared against a list the gate holds independently. The write half only — nothing here reads a written pack, and M8 owns the reading half |
| M4-W6 | Prove version isolation | M4-W1..W5 | synthetic second-version data fixture | the shared adapter's generator reads the second version's data through the same code path and emits only that version's names, and no file under `crates/` spells either version's name or any game class name either version's data supplies. Not proved, and not claimed: a general crate compiled or run against the second version's target table, because one build compiles one table |
| M4-W7 | Agent usability acceptance | M4-W1..W6 | the public author's guide, the example package pair, `cargo xtask package digests`, and the resulting original test mod | a fresh agent receives only public docs and builds a conforming mod without internal access. The row assumed public author documentation existed and none did, so W7 created [`docs/guide/`](docs/guide/) before its own premise could hold, with every Rust block in it compiled by the gate; the run then found defects in the facade and in the guide, which are repaired |
| M4-W8 | Linux review and gate | M4-W7 | evidence and review | global Linux gate, process matrix, SDK docs and M4 milestone gate pass. M4's process matrix is a table of gate processes rather than an operator-run launch — no game process was started for this milestone, which [`evidence/m4/linux-process-matrix.json`](evidence/m4/linux-process-matrix.json) records as a field |

The SDK remains semver-experimental through M10 even though its behaviour is
testable here.

M4 delivers an authoring surface and not a way to run what an author builds. Every
proof it rests on runs inside the gate; the three limits that follow from that —
no package discovery in a launch, no packet delivery, no pack read back — are
recorded as deviations in [`evidence/m4/manifest.json`](evidence/m4/manifest.json)
and stated to authors in [`docs/guide/limits.md`](docs/guide/limits.md). M4 also
claims no binding drift coverage for the agent jar: three of the classes its
persistence adapter resolves are hook targets and therefore pinned, and
everything below a class name — the members, their arities, the two
`LevelResource` field constants and the `SavedDataStorage` constructor — is in no
lock and owned by no work package.

This section's Verification column was restated in M4-W8, in the change that
recorded what M4 shipped. Each row was narrowed to the properties the milestone's
gate actually proves and each narrowing names what it now excludes, because the
original cells read as claims about a running game that nothing in M4 launched.
The restatement is recorded here rather than left to a diff, for the reason the
binding-coverage row below records its own retraction in place: whether a row met its gate
is a question a reader has to be able to answer from the tree.

### M5 — Compatibility inventory

**Purpose:** convert the pinned official fixture into an implementation-independent
work inventory without implementing any counterpart.

This milestone's inventory and checks live in `registry/` and consume the root
workspace's SDK and tools, using authorized ignored local fixture material.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M5-W1 | Initialize compatibility repo | M4, M0 fixture tooling | compatibility workspace topology | workspace metadata resolves |
| M5-W2 | Verify exact manifest population | M5-W1 | one record per pinned reference | count and project/file IDs reconcile to the fixture; corrections require source/hash evidence |
| M5-W3 | Build dependency and integration graph | M5-W2 | machine-readable graph and clusters | every node/edge resolves or has a blocking classification |
| M5-W4 | Catalogue behaviour surfaces | M5-W2 | config/resource/KubeJS/packet/save/side inventories | every source artifact maps to a record or evidenced dead/unreachable classification |
| M5-W6 | Assign canonical IDs and counterpart plans | M5-W3..W4 | owner, method, dependencies, tests and risk score per entry | 100% inventory coverage; IDs are unique and Ravel-native |
| M5-W7 | Produce general SDK gap proposals | M5-W4, M5-W6 | capability proposals and original test cases | proposals identify the required capability and a testable contract |
| M5-W8 | Inventory review and gate | M5-W1..W4, M5-W6..W7 | sanitized evidence manifest | 235/235 or evidence-corrected total has a complete plan; M5 gate passes |

M5's figure is 235 because that is what M5 measured: the pinned fixture when it
ran was ATM11 0.2.1, and `evidence/m5/inventory-summary.json` binds the rows
behind every count it publishes. The proof workload was later retargeted to
ATM10 To the Sky 2.0.2 and its 324 references, which is what M7 audits. A closed
milestone's material figure describes the material it measured, so the retarget
does not reach back into this row; the way the two totals are reconciled is the
`correction` M7's evidence records, and rewriting 235 to 324 here would delete
the discrepancy that correction exists to state.

The ranking rubric scores foundation value, dependency centrality,
integration breadth, compatibility uncertainty, expected memory
impact, and vertical-slice value on a documented 1–5 scale.

### M6 — Foundation counterpart set

**Purpose:** validate the counterpart-authoring process and the smallest
nontrivial gameplay clusters before scaling to every entry.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M6-W1 | Select foundation wave from M5 rubric | M5 | frozen wave manifest and cluster scenario | every selection reason and dependency is explicit |
| M6-W2 | Resolve general SDK gaps publicly | M5-W7 | SDK changes plus original test mods | M4-class gates pass before in-tree counterparts consume them |
| M6-W3 | Specify, then implement, foundation libraries/APIs | M6-W1..W2 | one behaviour specification per entry, then individually packaged counterparts | per-package unit, registration, save, side, performance and package-metadata suites pass |
| M6-W4 | Implement smallest gameplay cluster | M6-W3 | cluster packages and integration | fresh play, save, exit and reload scenario passes |
| M6-W5 | Validate counterpart-authoring process | M6-W3..W4 | counterpart template, checklist and evidence schema | a second independent counterpart follows it without hidden loader hooks |
| M6-W6 | Review and gate | M6-W5 | sanitized review/evidence | admitted set is green, package metadata is complete, each counterpart records which of its observable areas are declarable-only, and the M6 gate passes |

M6-W2 resolved five of the six general SDK capabilities M5-W7 proposed:
server-resolved data, world shaping, audio, drawing stages, and behaviour
interception, each with an original test mod under `test-mods/`. The sixth,
library provision, was deferred to M7 for the reason the open-questions row below
gives, and is now built there: `ravel_sdk::provision`, `test-mods/provision`, and
the last section of `tests/conformance/tests/integration/sdk_gaps.rs`. None of the six
carries anything into a running game — nothing plays a sound, draws at a stage,
or reads a provision — which each module states where an author reads it, in the
same terms `docs/guide/limits.md` already uses about packets. **Interception is
the exception and was not one when this paragraph was written.**
`ravel-bootstrap-runtime` asks a round at `Point::WorldIsSaved` before a world's
persistence is written, so that point is reached by a running game and a mod's
refusal there holds. `BlockIsBroken` and `ItemIsUsed` are still declared and
asked by nothing.

**A green counterpart is not an exercised one, and M6-W6 makes it say so.** The
general capabilities are abstractions a mod declares against and tests against;
the shipped host at this milestone discovered no packages. Each counterpart
therefore records which observable areas are *declarable-only*: the abstraction
exists, the implementation is real, and suites pass, but no running process
exercises it. This is the denominator behind the milestone's green result and
the basis for ordering later carry work.

M6 used separate specification readers and implementers. Readers described
observable behaviour and implementers worked from that prose. This describes
the foundation wave's process, not a source-access requirement.

**Specification cost tracks distinct behaviours rather than file count.**
M6-W3's smallest entry had 32 source files and no data or assets. Specifying it
took roughly forty minutes, with reading accounting for under half. Distinguishing
required mechanisms from incidental implementation details and noticing
load-bearing behaviour accounted for the harder work.

Two independent readers specified that entry from pinned source without seeing
each other's work. They disagreed three times: the first omitted one of two
forms twice, and the second misread a registration path with no opt-out once.
Those defects would otherwise have reached an implementation whose tests agreed
with an incomplete specification.

M7 uses the test kinds in `plans.json` plus behaviour-delta notes rather than
prose for every entry. Content catalogues are data; their systems need contracts,
not prose copies of each catalogue. The M5 dependency graph currently records
that an entry depends on another, but not which capability it consumes, so
library specifications cover more surface than their consumers need.

RVL-097's first implementation produced a parallel handler trio — an entry
handler, a configuration handler and an integration loader — plus an executable
taxonomy of entry kinds, none wired to the SDK's `Mod` lifecycle. Identifier
checks passed it. Design review found that the specification had supplied that
structure; fixing only the Rust would have left the same design defect in the
specification.

### M7 — Complete counterpart library

**Purpose:** execute the validated counterpart-authoring process until every
pinned manifest entry has an accepted reusable Rust counterpart.

Work proceeds in dependency-ordered waves. A wave cannot close with failing or
unclassified entries.

This is the retained historical open-M7 protocol, not SevTech WP8/WP9 closure.
Its v1 final audit keeps the 324-or-evidence-corrected-total rule below.
New `m7-library-v2` summaries require explicit fixture binding and use that
fixture's artifact-set denominator (259 SevTech jar inputs); such records do not
silently satisfy or rewrite historical v1 closure.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M7-W0 | Ship the SDK capability the waves consume | M6 | SDK changes plus original test mods | M4-class gates pass before in-tree counterparts consume them |
| M7-W1 | Freeze waves and cluster ownership | M6 | ordered wave manifests | graph closure proves no wave depends on a later unavailable wave |
| M7-W2 | Implement and test each counterpart | M7-W0..W1 | one package and evidence record per entry | standard per-counterpart suite passes, including differential tests |
| M7-W3 | Test integration clusters after each wave | M7-W2 | cluster reports | cross-mod registry, recipe, resource, side, save and performance cases pass |
| M7-W4 | Re-run full closure | each wave | burn-up ledger | accepted/blocked/unstarted totals equal verified fixture total |
| M7-W5 | Final 100% audit | all waves | complete library manifest | 324/324 or evidence-corrected total; zero unresolved dependencies/registries; package provenance fields present |
| M7-W6 | Independent review and gate | M7-W5 | sanitized review/evidence | all package and cluster suites pass; M7 gate passes |

M7-W0 exists because a counterpart cannot be written against a capability the SDK
does not have, and M6-W2 was the only work package anywhere that shipped one. The
first entry to need it proved the point: library provision was the sixth gap
M5-W7 proposed and the one M6 deferred, thirty-six inventory entries depend on it,
and it shipped here rather than in M6 because M6's scope is the foundation wave
and those thirty-six are not in it. Its number is lower than its dependants
because a work package id is a stable name rather than a position; the *Depends
on* column carries the order, as it does everywhere else in this document.

“Counterpart present” means its package builds for its supported compilation targets, passes
the standard suite, implements its accepted behaviour specification, records
package provenance, and participates in dependency/registry closure.
A stub or metadata-only placeholder does not count.

**A counterpart implements what the original adds to Ravel, not what the original
added to its own loader.** The first foundation entry attempted made this
concrete. Its central idea is declarative registration — declare a configuration
object per thing, hand the set to a handler that registers each into the right
registry, gives it a configuration entry and makes it disableable. Upstream needed
a library for that because its loader did not provide it. **Ravel provides it**:
`register` with a `Registrar`, `configuration_declaration` and `configure`, and
configuration declared in data. A counterpart
reimplementing that mechanism would duplicate the platform, and a dependent mod
would then reach the platform through a second, worse door.

So an entry's counterpart is scoped to the remainder: the content it genuinely
ships, and anything it offers dependents that Ravel does not — offered through
`services`, which is the only cross-mod capability surface and needs no second
one. Where that remainder is small the counterpart is small, and where it is empty
the honest output is a record saying the platform absorbed the entry rather than a
package built to look busy. **An entry that reduces to nothing is a result, not a
failure**, and it must be recorded as one rather than filled with invented content
— the same attempt reached for dummy registry keys to fill the shape, which would
have put false claims about the pack's contents into registry closure.

How far this reduces the library is unmeasured. M7-W1's routing counts, per entry,
how much is absorbed by capability Ravel already ships; until that count exists the
324 figure is an upper bound on packages rather than an estimate of work.

### M8 — Complete Ravel pack

**Purpose:** assemble the counterpart library with pinned configuration and all
reachable executable pack behaviour.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M8-W0 | Carry the loader into a running game | M4 | package discovery in a launch, a build step that composes the discovered set on the player's machine, plus a carry leg for each declared abstraction | a package the player installed — one no Ravel developer compiled — is discovered, resolved, built into the host on the player's machine and observed to act in a running game; every surface M6 recorded as declarable-only is either carried or restated as still declarable-only with its reason |
| M8-W1 | Pin pack manifest and counterpart versions | M7 | reproducible in-tree pack manifest | package, target, dependency and integrity closure pass |
| M8-W2 | Port configuration and declarative content | M8-W1 | schema-validated config/data | every M5 inventory record maps to output or evidenced exclusion |
| M8-W3 | Reimplement custom KubeJS behaviour as Rust modules | M8-W1 | compiled safe-Rust pack modules | scenario equivalence passes; runtime scan finds no JS engine |
| M8-W4 | Build full scenario suite | M8-W2..W3 | fresh-world and integration scenarios | intended content/progression inventory has complete scenario coverage |
| M8-W5 | Repeated save/reload soak | M8-W4 | world and migration evidence | repeated clean shutdown/reload cycles preserve state; failure cases fail closed |
| M8-W6 | Independent pack review and gate | M8-W0..W5 | sanitized review/evidence | no Java/JavaScript mods execute; all pack/scenario suites and M8 gate pass |

M8-W0 exists because nothing else owned the thing every other row here assumes. A
launch discovers no package today: the shipped host runs only mods it was
compiled from, which M4 recorded as a deviation and `docs/guide/limits.md` states
to authors. But `SPEC.md` §3.5 lists "mod discovery from an installed instance"
among the responsibilities Ravel *owns*, and §16 lists "package discovery and
validation" among M2's deliverables. So this is not a capability the plan chose
to defer — it is one the plan believed it already had, and the disclosure that it
does not was written into a milestone's deviations rather than into a work
package that restores it.

What that costs is easy to understate. Every counterpart M6 and M7 produce is
unrunnable by construction until it lands, and three later rows cannot honestly
pass without it: M8-W4's fresh-world scenarios, M8-W5's save/reload soak, and the
whole of M10, which asks a player to walk a progression through a running game.
Those rows would otherwise be met by suites that never launch anything, which is
the same failure the declarable-only rule above exists to prevent — a green that
measures the tests rather than the game.

Its *Depends on* column reads M4 rather than M7 because discovery needs the SDK
and the loader, not the counterparts; it is placed in M8 because M8 is where its
absence first stops a row from passing. As with M7-W0, the id is a stable name
rather than a position.

**This row's verification originally read "run by a launched game without being
compiled into the host", and that was impossible rather than merely hard.** Source
distribution makes a mod's code Rust source, so a discovered package has a manifest and no
value: nothing exists to construct or enter. The first implementer said so
plainly instead of claiming the row, which is the only reason it is being fixed
here rather than at M8's gate. The correction is that "compiled into the host" is
the *mechanism*, not the thing to avoid — what must never happen is a Ravel
developer compiling a player's mod. The host is built on the player's machine
from whatever that player installed, which is what `evidence/m3/pack-compile.json`
measured when it timed a 256-mod pack at about 32 seconds cold and 1.6 seconds
warm, and what the shared toolchain and build cache decision exists to serve.

That splits this row into three pieces rather than two: discovery, which is
shipped; **a build step that composes the discovered set into a host binary on the
player's machine**, also shipped; and the carry legs. The middle piece is where
"seamless for the player" is won or lost, because it is the one that turns
installing a mod into waiting for a compile.

**"Carry legs" is the wrong plural.** A design pass over all four found that
packet-send, audio and drawing are not three problems but one mechanism used
three times. Every one of Ravel's hook targets points inward — the game calls
Ravel — and no Rust in this repository holds any way to call outward: a
`JavaHandle` carries no method but `is_null`. The only thing that acts on the
game is Ravel's own Java, which already does it for the settings screen, reaching
Rust across a hand-written native and performing the result against the game.
Every outbound action is that shape with a different verb at the end, so the
mechanism is built once and reused. Interception shares none of it.

**The order is decided by which cost grows.** Audio costs one stub-seed and lock
re-pin ceremony, and that price is the same whenever it is paid. Interception
costs a change to the `HostMod` contract, because a mod today has no way to
register an answer at all — the surface is missing both ends rather than the one
`docs/guide/limits.md` describes. A contract change obliges every mod to be
swept, so its price rises with every counterpart that exists when it lands, and
the counterpart library is about to be written. **Interception therefore goes
first, while the number of mods to sweep is nearly zero**, and it can be proved
at `WorldIsSaved`, whose hooks are already pinned and carried, so the contract
change needs no ceremony to demonstrate. Audio follows, paying its fixed price
and building the outbound mechanism the rest reuse. Packets need both directions,
two ceremonies and a launched client and server for the full claim, so they come
after. Drawing is not work at all: Ravel-owned screens are drawn in Java,
so it is restated as declarable-only rather than carried.

### M9 — Linux performance acceptance

**Purpose:** prove the complete single-player pack meets the fixed Linux
contract without silently reducing scope or settings.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M9-W1 | Freeze comparable performance inputs | M8, M0 profile | exact build, world, route and profile hashes | verifier rejects any unrecorded setting or content drift |
| M9-W2 | Attribute current costs | M9-W1 | per-mod/boundary/JVM/GPU/memory profile | exact vs estimated metrics are labelled; profiling overhead is separately measured |
| M9-W3 | Optimize without behavioural regression | M9-W2 | reviewed loader/SDK/counterpart changes | relevant M4, M7 and M8 suites remain green after every optimization |
| M9-W4 | Run final Linux acceptance pair | M9-W3 | one official-pack result and one Ravel-pack result, and the RAM/CPU improvement between them | both use the same profile hash; all fixed FPS/RSS/pressure/swap and frame/tick reporting requirements pass. `SPEC.md` §6.6 defers the improvement figure to this row, so the pair is recorded *and* the improvement is stated |
| M9-W5 | Independent performance review | M9-W4 | result and raw-artifact review | reviewer confirms settings, hashes, calculations, and comparison integrity; no rerun is required. The reviewer also rules on whether the improvement is significant, because §6.6 makes that a judgement rather than a threshold |

If M9-W4 cannot pass with the complete workload, review host viability. Do not mark M9
complete, relax the profile, remove required content, or begin a native client
without a new approved capability specification.

### M10 — Progression acceptance

**Purpose:** prove that the passing complete pack can finish the pinned intended
progression in a fresh world and preserve Ravel save compatibility.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M10-W1 | Freeze progression checklist and build sequence | M8 scenarios, M9 build | versioned endgame path and migration matrix | every step maps to underlying automated mechanics or an explicit manual observation |
| M10-W2 | Perform fresh-world playthrough | M10-W1 | timestamped manual acceptance journal and final-world hash | every required progression step and endgame completion is attested |
| M10-W3 | Convert findings to regressions | M10-W2 | automated tests and fixed defects | every encountered release-blocking incompatibility has a failing-then-passing test |
| M10-W4 | Verify save compatibility sequence | M10-W1..W3 | migration reports across declared builds | worlds open only with compatible required mods/data and preserve expected state |
| M10-W5 | Final independent audit | M0..M10 | complete evidence index and review | all manifests pass; secrets and sensitive logs are absent; no blocker remains; M10 gate passes |

Passing M10-W5 is historical Linux initial completion for its recorded workload.
Forward Linux completion requires WP9; historical replay does not substitute
for the SevTech performance and progression contract.

The final security audit proves that the tracked tree and evidence contain no
credential, launcher token, or unredacted sensitive log.

### Post-Linux-M10 — Apple Silicon platform expansion

No macOS build, launch, capture, packaging, signing, or verification work starts
before forward Linux completion at WP9. Afterward, a separately reviewed plan
ports the complete Linux loader and mod set to `macos-aarch64`, proves packaging and
signing, runs the native Apple Silicon process matrix, and evaluates the
constrained M1 Pro performance contract. Linux cross-evaluation never
substitutes for that later native evidence.

### M11 — Official ATM10 To the Sky server interoperability

M11 begins only after initial completion and a protocol specification.
Inventory handshake, identity/version negotiation, registry sync,
channels, packets, and state transitions per networked counterpart. Test joining
an unmodified pinned server, cross-client interactions, capture redaction,
Ravel-native canonical IDs, and separate multiplayer client-only performance.

The gate requires every M5-networked counterpart scenario to pass against the
pinned server and a sanitized evidence bundle. “Can join” alone is insufficient.

### M12 — Dedicated-server stretch

Not implementation-ready and not authorized by this roadmap. The only permitted
work is a new capability amendment defining host model, ownership, compatibility,
security, performance, and exit criteria.

### M13 — Publication and registry

Post-initial and not implementation-ready. The existing `registry/` source tree
does not authorize the publication service. It requires a separate plan covering
name collision checks, dependency/licence audit, signing and publisher identity,
moderation, distribution, and rollback.

## Traceability matrix

| Spec concern | Primary implementation | Proof |
| --- | --- | --- |
| Loader identity/runtime | bootstrap, loader, host API | M1 measurements; M2/M3 process scans |
| Safe Rust boundary | all maintained crates; audited dependencies | policy scan, dependency audit, architecture review |
| Packages/dependencies | package, resolve, pack | schema/property/negative tests; M7/M8 closure |
| Lifecycle/invariants | core and loader | state-machine and process conformance suites |
| Version isolation | Minecraft traits, shared adapter, and per-version data | forbidden-reference scan; synthetic version-data test |
| Configuration/UI | config and adapter UI integration | data declarations, Java screens, round-trip process tests |
| Resources/data | data and pack | schema and Minecraft-process tests |
| Saves/migrations | save and loader world gate | repeated save/reload and migration matrix |
| Diagnostics/profiling | observability | attribution, redaction, accuracy and overhead tests |
| Prism ownership | xtask/runbook/bootstrap | stock launch evidence |
| Counterpart completeness | in-tree inventory and packages | verified-total ledger and per-package suites |
| Pack completeness | in-tree pack and scenarios | full inventory mapping, closure and soak |
| Performance | explicit M9 workload protocol | M0 Linux launch proof and M9 single-pair Linux acceptance |
| Progression | in-tree checklist and ignored local world | M10 journal plus regressions |
| Official server protocol | network and compatibility adapters | protocol specification and M11 cross-client suite |

## Open questions and decision deadlines

| Question | Deadline | Blocking effect |
| --- | --- | --- |
| Exact fixture reference total and authoritative hashes | M0-W4 | discharged; M0 closed on `evidence/m0/manifest.json` |
| Canonical workload duration and metric definitions | M9-W1 | blocks M9 |
| Numeric RAM/CPU improvement target over the Java pack | M9-W4 | blocks M9 acceptance; `SPEC.md` §6.6 defers it because no Java baseline exists until the pair is run |
| Game Java runtime required by pinned metadata | M0-W6 | discharged; M0 closed |
| Architecture comparison metrics and failure actions | M0-W8 | discharged; M1 selected against it |
| JVM hook mechanism | M1 | discharged; refined before M3-W3 on what enters the game process |
| Rust execution/package/ABI model | M1 **and M3** | discharged at M3: source distribution, compiled on the player's machine. Reconciling `package.schema.json` and `SPEC.md` §3.4 is closed as a breaking schema-v1-in-place migration |
| Binding extraction and generation | ADG-3 / M1 | discharged, with one surface partly outside it: the agent jar's persistence adapter resolves game classes and members by name, and the pinned files cover some of the classes and none of the rest. `net.minecraft.client.Minecraft`, `net.minecraft.server.MinecraftServer` and `net.minecraft.world.level.storage.SavedDataStorage` are hook targets in `hooks.toml` and in `mappings.lock`, so a rename of one of those classes shows up in a re-pin diff. **An earlier wording of this row said none of them was pinned, which was wrong and understated the coverage; this is the correction.** What no pinned file holds is `LevelResource`, `DataFixers`, every member name and arity, the two `LevelResource` field constants, and the three-argument `SavedDataStorage` constructor — which is what `Persistence.java`'s comment at the lookup has said all along. `verify_persistence_reflection` pins the class set and cannot see a rename. Bringing the rest of the surface under `stubs.toml` and `mappings.lock` needs the stub-seed and lock re-pin ceremony against the pinned jar. **No work package owns that yet.** M3's packages are closed and none named it; M4-W1 to M4-W8 do not either. It belongs with the version-adapter re-pin work. An earlier reading of this row ruled that **M4-W6 is the first gate that cannot honestly pass without it**, because proving version isolation against a second version is exactly what an unpinned reflective surface defeats. **That ruling is retracted here, and the reason is that it was too broad.** The reflective surface it names lives in the agent jar, which is neither a general crate nor the shared adapter, so what it leaves in doubt is a drift claim over the agent jar — M4-W8's territory — rather than W6's claim that the shared adapter and the general crates carry no version. W6 proves that claim against a synthetic second version's data in `tests/fixtures/public/versions/`, and that fixture cannot reach the reflective surface at all: it declares no jar, so there is no class for a name to resolve against and nothing for a rename to break. No version of W6 could have discharged this row, which is what makes refusing W6 for not discharging it the wrong ruling rather than an unmet one. The re-pin ceremony is what closes this row, and no package has performed one. Until a package owns it and runs it, no milestone may claim drift coverage for the agent jar's persistence adapter |
| Retained 1.21.1 support and live-proof limit | historical M8; forward regression matrix WP9 | Ravel retains Minecraft 1.21.1 support alongside build-default 26.1.2; forward proof is SevTech on 1.12.2, not ATM10 on 1.21.1. **`minecraft/versions/1.21.1/` exists, generates, and the whole suite passes against it.** Its jar and published mapping are pinned by [`tests/fixtures/descriptors/runtime-inputs-1.21.1.json`](tests/fixtures/descriptors/runtime-inputs-1.21.1.json) through its version metadata and sit hash-verified under `tests/fixtures/local/tooling/minecraft-1.21.1/`. The directory carries twenty-nine of 26.1.2's thirty-seven reviewed hooks: twenty-seven unchanged, plus `world.open-persistence` and `screen.title-menu-options` re-reviewed at drifted descriptors because `jvm/bootstrap`'s `Transformer` names those seams. The other eight are explicit non-goals with reasons. Every build entry point exports a selection; screen templates name roles bound by each version's `stubs.toml`; genuinely different APIs override the shared template under `jvm/screen/template/<version>/`. `ravel:test` runs once per tracked live version with acquired local inputs, including 1.12.2; absent local fixtures are loudly reported as `SKIPPED`, not live proof. Legacy official integrated and dedicated agent acceptance is recorded in the [legacy launch runbook](docs/runbooks/legacy-launch.md). The agent jar's `Persistence` still spells 26.1.2's member names and arities, so 1.21.1's saved-data half compiles but is not driven. This is the unowned reflective-surface re-pin described above; the [architecture overview](docs/architecture/overview.md#mechanisms) describes the persistence mechanism. **Nothing here has ever started a 1.21.1 process with Ravel attached**, so no runtime claim follows until a launch exercises it |
| Java/Rust state ownership | M1 | discharged; opaque game-object handles added at M3-W0 |
| Stock Prism versus minimal patch | M1 | discharged; the library-path argument does not work and the fix is `System.load` on a path given by a `-D` property. No work package owns building it, so the native half has no conformance claim until one does |
| Configuration and UI schema | before M3-W3 | discharged; declared in data, drawn in Java |
| Where the generated screen artifact loads | M3-W3 | discharged; the agent uses `Instrumentation.appendToSystemClassLoaderSearch` for the sibling screen jar |
| Whether anything reads the pack `ravel-pack` writes | M8 | M4-W5's independent review found that the writer and its own tests were the only readers of `RESERVED_PREFIX`, while `ravel-loader`'s `open_configuration` accepted a map no production caller filled. That made a write→read round trip unprovable. M4 named no reader work package, so M8 owns the obligation with the shipped-host wiring. M3 review-round counts remain in `evidence/m3/review.md`; `verify_review_round_counts` checks that this document does not restate them, after an earlier restatement proved wrong. **Discharged at change `wnkrynlrprnl`:** `ravel_core::paths::PACK_CONFIG_DIRECTORY` is shared by writer and reader, and `ravel_host_runtime::compose`, called by every host's JNI entry, routes through `ravel_loader::open_configuration` to read the layer from disk. `tests/conformance/tests/integration/pack_round_trip.rs` exercises two configuring mods and mutates the written document to prove the observed value came from disk. A *game* reading a pack remains unproven because this test starts none; that half belongs to M8-W0's host |
| Whether a pack's settings layer is checked when the pack is built | M8 | M4-W5's independent review found the asymmetry and it is stated at the settings write in `ravel-pack`'s `validate`. A typo'd tag member is refused to the pack author; a typo'd settings key is refused to the player, because `LayeredConfiguration::resolve` holds the check and runs at launch. Closing it means handing `validate` the members' configuration declarations and calling that same `resolve`. **Discharged at change `wnkrynlrprnl`, by doing exactly that:** `ravel_pack::validate` takes `&[ConfigurationDeclaration]` and its `settings_closure` resolves each member's overrides through `LayeredConfiguration::resolve`, the same call the launch makes, so the asymmetry is gone rather than papered over — the author is now refused a misspelled key at build time and the player is refused it at launch by the one implementation. `tests/conformance/tests/integration/pack_round_trip.rs` asserts the build-time refusal names the owner and the key, with the correctly spelled pack as its positive control |
| How an agentic acceptance run drives a modded game | M8-W4 / M10 | A run perceives through `ravel_host_api::GameEvent` and acts through outward instructions, adding no new crossing; see the [architecture mechanisms](docs/architecture/overview.md#mechanisms). The version-neutral scenario, step, assertion, observation, and transcript models are built and tested in `crates/ravel-acceptance`; checkpoints use pack-defined `ravel_core::ContentId` values in explicit scenario order. Unimplemented driver traits and the fake-backed runner are not published. **The live driver is not built and model tests are not evidence the game was played.** It depends on a hook-value→`Observation` projection, a host performing outward instructions in a running game (M8-W0, including native library loading), and a signed-in session with a display. M9's performance rig additionally needs a real GPU. Until M8-W0 lands and a run is driven, the acceptance model alone cannot prove the pack was walked |
| Whether any reviewed hook can be projected into a live `Observation` | M8-W4 / M10 | Hook-to-loader wiring landed, but the payload still cannot express the observation. `crates/ravel-bootstrap-runtime/src/runtime/dispatch.rs`'s `Inner::work` binds five hooks through `EVENTS` and one by name. Per `minecraft/versions/26.1.2/hooks.toml`, none carries readable world state: `client.tick` and `resource.reload-client-packs` have no parameters; `server.tick` and `server.tick-integrated` carry a `BooleanSupplier`; `resource.load-server-resources` carries eight references; and `command.perform-prefixed` carries a `CommandSourceStack` and readable typed text. Reference parameters become `HookValue::Handle`, exposing only a declared class string and null check. `Observation` needs `(RegistryKey, u32)` held counts and `RegistryKey` block presence, neither named by the inventory; `registry.register`'s `ResourceKey` is also an unreadable handle. Retaining discarded hook values therefore would not solve it, and testing with values the JVM never supplies would be vacuous. Closing the gap needs a reviewed hook target carrying world state as a primitive or string, resolved against the pinned jar and added to `hooks.toml`. M8's work packages own a driver but no such target; until a hook exists and a run reads it, the model has not observed a running game |
| Whether a mod may provide a shared library other mods build on | M7 | M5-W7 proposed six general SDK capabilities and M6-W2 shipped five of them. The sixth, `RVL-GAP-06`, is a mod declaring that it *provides* a library at a version, one provider resolved per library for a launch, and a refusal when two providers disagree or a required library has none. It is deferred rather than done, and the reason is that **none of M6's frozen foundation wave motivates it**: every entry in that wave is answered by the five that shipped, so building it in M6 would be a capability nothing installed would use and no test could exercise honestly. It is not the same question as [`services`](crates/ravel-sdk/src/services.rs), which orders named calls between two mods that are both installed; this one asks which mod *supplies* a shared thing and at what version. M7 is the first milestone whose set is large enough for two counterparts to want one library, so it owns the row, and M7 has now built it: [`ravel_sdk::provision`](crates/ravel-sdk/src/provision.rs) resolves one provider per library and refuses a contested library, a version outside a declared range, and a requirement nothing provides, with `test-mods/provision` and the last section of `tests/conformance/tests/integration/sdk_gaps.rs` exercising it. What remains open is the half no declaration can settle alone: no launch builds a `Provision` from the packages it composed, which `docs/guide/limits.md` states where an author reads it |
| Official server wire surface | after M10 | blocks M11 only |
| Official host viability | only after M9 failure | leaves M9 incomplete |
| Registry identity/signing/moderation | separate M13 plan | no effect on initial completion |

## Immediate handoff

Current work follows the forward WP1–WP9 dependency/evidence sequence above;
WP9, not a relabelled historical M10, owns SevTech Linux completion.
Preserve existing evidence and select its actual fixture for replay.

The original M0 startup sequence below is historical context, not current work:

1. M0-W1: initialize the Cargo workspace and mise environment without
   assuming JVM entry, execution, bindings, or state-ownership mechanisms.
2. M0-W2: implement the evidence schema, `ravel-xtask` gate skeleton, and CI
   parity.
3. M0-W3 through M0-W5: establish the ignored fixture contract and later
   performance protocol shape before admitting launch evidence.
4. Review that foundation, then admit the already-proven Linux vanilla and
   official ATM11 launch evidence. Performance capture waits for M9, and macOS
   work waits until Linux M10 is complete.

The loader implementation follows M1's measured architecture.
