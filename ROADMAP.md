# Ravel implementation roadmap

Status: implementation-ready plan  
Authority: subordinate to [`SPEC.md`](SPEC.md)  
Initial-completion gate: M10  
Last updated: 2026-07-30

## Capability

Ravel enables a player to run a complete, precompiled Rust modpack inside
official Minecraft: Java Edition, initially for Minecraft 26.1.2, without
another Java mod loader. Initial completion means that the separate private
ATM11-equivalent pack passes its automated compatibility suites, meets the
constrained M1 Pro performance contract, and completes the pinned progression
path in a fresh Ravel world.

The public `ravel` repository owns the reusable loader, SDK, package contracts,
tooling, unrestricted test mods, and sanitized evidence. Rights-sensitive
counterparts, official fixtures, private behaviour research, and private worlds
remain in a separate private workspace.

## Constraints

These rules apply to every work package and milestone:

- Official Minecraft remains the initial host.
- Ravel loads Rust mods only and never uses NeoForge, Fabric, Forge, Quilt, or
  another loader as its bootstrap.
- No ATM11 counterpart implementation begins before M3 passes.
- Maintained Rust and mod-author code uses `#![forbid(unsafe_code)]`. Audited
  dependencies may encapsulate unavoidable native/JVM boundaries.
- No mod code runs before package validation and dependency resolution.
- The installed mod graph is fixed for the process lifetime.
- Minecraft 26.1.2 details remain behind a version adapter.
- The public SDK exposes safe Rust and does not expose generated JVM or
  Minecraft-version internals.
- Required-mod, side, registry, integrity, and saved-data failures fail closed
  before world access or further mutation.
- Restricted files, official mod JARs, ATM-owned content, credentials, launcher
  tokens, and rights-sensitive research never enter this repository.
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

M0 through M10 are required for initial completion. M11 is a later
interoperability release. M12 requires a new capability amendment. M13 is
post-initial publication and registry work.

This interpretation follows the spec's single-player-first ordering, pack
completion evidence, M9 constrained-device gate, M10 progression acceptance,
and explicit description of official ATM11 multiplayer as later work.

## Non-goals

This roadmap does not authorize a native client, custom launcher, public
registry, Java mod execution, automatic KubeJS translation, world import from
official ATM11, Windows support, Intel macOS support, Linux ARM support,
hot-loading, a wholly Rust server, or publication of uncleared counterparts.

## Verification system

### Gate model

Every work package has:

1. an ID such as `M2-W3`;
2. declared inputs and predecessor IDs;
3. one accountable output;
4. automated checks where automation is possible;
5. an evidence manifest;
6. an independent review record when the change is nontrivial.

The repository-native fast and environment gate commands will be:

```text
devenv tasks run ravel:verify
devenv tasks run ravel:verify-environment
devenv tasks run ravel:verify-milestone:m<N>
```

The milestone task wraps:

```text
cargo run -p ravel-xtask -- verify milestone M<N>
```

M0-W2 creates these commands. `ravel:verify` is the fast code-policy graph.
`ravel:verify-environment` runs `devenv test` and `nix flake check`, with
`devenv test` executing the same code-policy graph inside the pinned shell.

### Evidence contract

Each milestone produces `evidence/m<N>/manifest.json` conforming to
`schemas/evidence.schema.json`. The manifest records:

- milestone and work-package IDs;
- Ravel revision and dirty-state status;
- exact Minecraft, Prism, fixture, Java, Rust, Nix, and devenv versions;
- platform, architecture, hardware profile, and relevant system settings;
- input artifact names and cryptographic hashes;
- commands executed, exit status, start time, duration, and log hashes;
- test counts, failures, coverage, and skipped-test count;
- benchmark protocol version and result file hashes;
- deviations, open risks, and blocking/non-blocking classification;
- reviewer identity or agent record and review result;
- manual attestation fields where automation is impossible.

Large raw captures, official fixture content, private worlds, and sensitive logs
remain outside the public repository. The public manifest stores sanitized
summaries and hashes. Redaction tests run before a report can be admitted.

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

The complete `ravel:verify-environment` gate additionally runs:

```text
devenv test
nix flake check
```

Additional policies:

- No skipped or ignored tests are accepted in a milestone gate.
- The 80% line-coverage floor applies to testable maintained Rust workspace
  code. Generated bindings, benchmark-only spikes, and external process glue
  may be excluded only by a reviewed, explicit allowlist.
- Behavioural conformance is required even when line coverage passes.
- Linux x86_64 is required from M0 onward.
- Apple Silicon macOS evidence is required at M0, M1 where the selected design
  affects viability, and every loader/SDK milestone from M3 onward.
- Tangled Spindles provide the Linux gate. Because Spindle engines are
  Linux-based, the required Darwin gate runs on an actual `aarch64-darwin`
  machine via `docs/runbooks/darwin-verification.md`; a Linux evaluation never
  substitutes for that evidence.
- A gate rejects untracked restricted fixture patterns, credential patterns,
  forbidden loader artifacts, or maintained Rust that permits unsafe code.

### Performance evidence

M0 creates a versioned `tests/performance/profiles/initial.toml` containing the
canonical world, route, warm-up, duration, repetitions, graphics and simulation
settings, active content, sampling intervals, and pass/fail calculations. M0
cannot close until those values are explicit.

The fixed M9 values come from the spec:

- Apple M1 Pro with 16 GB unified memory;
- single-player with integrated server;
- 3456 x 2234 output;
- 8-chunk render distance;
- no shaders;
- sustained 30 FPS under the canonical workload;
- at most 8 GiB steady-state process RSS;
- at most 10 GiB transient process RSS;
- no sustained memory-pressure warning or swap thrashing;
- recorded median, p95, and p99 frame time and integrated-server tick time.

The profile must define “sustained,” “steady-state,” “transient,” “warning,”
“thrashing,” and run-to-run reproducibility numerically before the first
baseline is accepted. M9 uses the same profile version as the official
baseline, or records and reruns a reviewed migration of both baselines.

## Delivery dependency map

```text
M0 baselines and reproducibility
 └─> M1 ADG-1..5 architecture evidence
      └─> M2 loader skeleton
           └─> M3 credible cross-platform loader
                ├─> M4 general SDK breadth
                │    └─> M5 private compatibility inventory
                │         └─> M6 foundation counterparts
                │              └─> M7 complete counterpart library
                │                   └─> M8 complete pack
                │                        └─> M9 constrained performance
                │                             └─> M10 progression acceptance
                │                                  └─> initial completion
                └─> ADG-6 configuration/UI decision before M3 closes

M10 └─> ADG-7 └─> M11 official-server interoperability
M9 failure └─> ADG-8; it does not relax M9 or authorize a native client
M12 and M13 require separately approved post-initial scope
```

## Desired end-goal repository

### Public `ravel` tree

Items marked `[decision]` are created only if the named architecture gate
selects that mechanism.

```text
ravel/
├── .cargo/
│   └── config.toml                 # aliases and target-neutral Cargo policy
├── .config/
│   └── nextest.toml                # deterministic test profiles and retries policy
├── .tangled/
│   └── workflows/
│       └── ci.yml                  # Spindle pipeline mirroring the verification tasks
├── crates/
│   ├── ravel-core/                 # IDs, versions, sides, capability and lifecycle types
│   ├── ravel-package/              # package/manifest schemas and integrity contract
│   ├── ravel-resolve/              # dependency graph, compatibility, deterministic order
│   ├── ravel-loader/               # discovery pipeline and lifecycle orchestration
│   ├── ravel-sdk/                  # safe public mod-author facade
│   ├── ravel-sdk-macros/           # derives/attributes that generate SDK declarations
│   ├── ravel-host-api/             # [ADG-2] stable loader-to-mod execution contract
│   ├── ravel-bootstrap/            # [ADG-1] selected official-JVM entry/hook plumbing
│   ├── ravel-minecraft/            # version-neutral Minecraft capability traits
│   ├── ravel-minecraft-adapter/    # shared version-data/codegen adapter runtime
│   ├── ravel-config/               # schema, validation, overrides, restart/side semantics
│   ├── ravel-data/                 # resources, recipes, tags, loot, progression data
│   ├── ravel-save/                 # versioned saved data and fail-closed migrations
│   ├── ravel-network/              # Ravel networking abstractions; later wire adapters
│   ├── ravel-observability/        # diagnostics, redaction, metrics and opt-in profiling
│   ├── ravel-pack/                 # pack manifest, composition and validation library
│   └── ravel-testkit/              # deterministic mod, loader and process test APIs
├── tools/
│   ├── ravel-cli/                  # package, pack, fixture, Prism/test handoff and benchmarks
│   └── ravel-xtask/                # codegen, policy, evidence and milestone gates
├── jvm/                            # [ADG-1] absent unless minimal Java output is selected
│   ├── bootstrap/                  # generated/minimal classes, never a Java mod API
│   └── build/                      # reproducible JVM artifact build description
├── minecraft/
│   └── versions/
│       └── 26.1.2/
│           ├── README.md            # M0 boundary; no guessed mapping data
│           ├── adapter.toml         # [M1/ADG-3] inputs and exact upstream hashes
│           ├── hooks.toml           # [M1/ADG-3] reviewed hook capability inventory
│           └── mappings.lock        # [M1/ADG-3] mapping provenance and pinned inputs
├── schemas/
│   ├── package.schema.json
│   ├── pack.schema.json
│   ├── configuration.schema.json
│   ├── saved-data.schema.json
│   ├── provenance.schema.json
│   └── evidence.schema.json
├── test-mods/
│   ├── noop/
│   ├── dependency-base/
│   ├── dependency-consumer/
│   ├── registries/
│   ├── resources/
│   ├── events/
│   ├── commands/
│   ├── configuration/
│   ├── saved-data/
│   ├── networking/
│   ├── inter-mod-service/
│   └── failure-cases/
├── tests/
│   ├── conformance/                 # package, graph, lifecycle and SDK contract suites
│   ├── minecraft-process/           # real official-Minecraft process harness
│   ├── save-reload/                 # world persistence and migration scenarios
│   ├── performance/
│   │   ├── profiles/                # versioned workload definitions
│   │   └── collectors/              # Linux/macOS metric collection adapters
│   ├── fixtures/
│   │   ├── public/                  # original unrestricted inputs
│   │   └── local/                   # ignored official/restricted material
│   └── snapshots/                   # redacted deterministic expected outputs
├── experiments/
│   ├── adg-1-jvm-entry/
│   ├── adg-2-mod-execution/
│   ├── adg-3-bindings/
│   ├── adg-4-state-ownership/
│   └── adg-5-prism/
├── examples/
│   ├── minimal-mod/
│   ├── content-mod/
│   └── pack-module/
├── docs/
│   ├── architecture/
│   │   ├── overview.md
│   │   ├── dependency-rules.md
│   │   ├── safety-boundary.md
│   │   └── public-private-boundary.md
│   ├── decisions/                   # ADG ADRs and rejected-option evidence
│   ├── sdk/                         # mod-author guide and generated API links
│   ├── pack-authoring/
│   ├── runbooks/                    # Prism, baseline, release and recovery procedures
│   └── rights/                      # public policy and blank provenance templates only
├── evidence/
│   ├── m0/ ... m11/                 # sanitized manifests and small summaries
│   └── hardware/                    # non-identifying reference profiles
├── nix/
│   ├── common.nix
│   ├── linux.nix
│   ├── darwin.nix
│   └── ci.nix
├── .envrc
├── .gitignore
├── Cargo.lock
├── Cargo.toml
├── LICENSE-APACHE
├── LICENSE-MIT
├── README.md
├── ROADMAP.md
├── SPEC.md
├── deny.toml
├── devenv.lock
├── devenv.nix
├── devenv.yaml
├── flake.lock
├── flake.nix
├── rust-toolchain.toml
└── rustfmt.toml
```

### Crate responsibilities and dependency rules

| Crate | Owns | May depend on | Must not depend on |
| --- | --- | --- | --- |
| `ravel-core` | Canonical IDs, versions, side/capability descriptors, lifecycle and diagnostic value types | small general-purpose safe crates | SDK, loader, concrete Minecraft adapter |
| `ravel-package` | Manifest parsing, compatibility ranges, target metadata, hashes, provenance fields | core, serialization, version/range libraries | loader, bootstrap, Minecraft adapter |
| `ravel-resolve` | Required/optional/incompatible/discouraged edges, cycles, side checks, deterministic load plan | core, package, graph/solver dependencies | SDK, concrete adapter |
| `ravel-loader` | Discovery-to-stop orchestration, registry barrier, failure policy, world-access gate | core, package, resolve, host API, Minecraft traits, feature crates | concrete 26.1.2 implementation details outside injection |
| `ravel-sdk` | Safe public API and author-facing types | core plus public parts of config/data/save/network | loader internals, generated bindings, JNI/JVMTI |
| `ravel-sdk-macros` | Compile-time declaration generation | parser/quote libraries and public contract types | loader, adapter, runtime state |
| `ravel-host-api` | Selected stable package execution/ABI boundary | core and serialization/ABI support selected by ADG-2 | concrete Minecraft bindings |
| `ravel-bootstrap` | Selected JVM entry, callback routing, launch bootstrap | loader, host API, Minecraft trait and selected audited boundary dependencies | public SDK implementation policy |
| `ravel-minecraft` | Version-neutral handles, hooks and capability traits | core | 26.1.2 mappings or generated version symbols |
| `ravel-minecraft-adapter` | Shared adapter runtime, code generation, hook dispatch and conversion machinery | Minecraft traits, core, selected audited JVM dependencies, version data | SDK authoring policy and hardcoded assumptions outside selected version data |
| `ravel-config` | Schemas, validation, side/restart flags, layered overrides | core | generated UI widgets or version details |
| `ravel-data` | Declarative resources, recipes, tags, loot and progression contracts | core, package | loader or concrete adapter |
| `ravel-save` | Ownership, schema versioning, migrations and compatibility checks | core | official ATM11 world import logic |
| `ravel-network` | Ravel packet/channel abstractions and side rules | core | NeoForge as canonical identity or runtime dependency |
| `ravel-observability` | Structured reports, redaction, metric schemas, profiling coordinator | core and selected collectors | authentication/token collection |
| `ravel-pack` | Pack pinning, declarative composition, closure validation | package, resolve, config, data | marketplace/download ownership |
| `ravel-testkit` | Deterministic fake hosts, fixture builders, process harness contracts | loader, public contracts and test-only dependencies | production runtime ownership and restricted fixtures |

Dependency direction is inward toward contracts:

```text
SDK/test mods ───────> public contracts and feature schemas
loader ──────────────> contracts + resolver + host/Minecraft traits
shared adapter ──────> version data + version-neutral Minecraft traits
bootstrap ───────────> loader + selected audited boundary dependencies
CLI/xtask/testkit ───> libraries; libraries never depend on tools
```

Cycles between crates are forbidden. A new crate is justified only by an
independent stability, safety, version, test, or distribution boundary. Until
that boundary appears, code stays in the nearest existing crate.

### Safety boundary

Every maintained crate has `#![forbid(unsafe_code)]` at its root. The workspace
gate verifies this. JNI, JVMTI, native loading, operating-system calls, and
generated boundary code must be provided by audited dependencies with safe
wrappers. If ADG evidence shows that maintained unsafe code is unavoidable,
implementation stops for a spec amendment; the plan does not pre-create an
“unsafe escape hatch” crate.

### Version boundary

Only `minecraft/versions/26.1.2`, generated outputs derived from it, and
adapter-specific process fixtures may encode 26.1.2 mappings or hook names.
`ravel-minecraft-adapter` is shared across supported versions and selects
version data explicitly. A workspace policy test rejects `26.1.2`, generated
mapping packages, and concrete game symbols in core, package, resolver, loader,
SDK, pack, and general feature crates.

Adding a second version must normally require only a new version-data directory
and generated outputs, not a new crate or changes to package identity,
dependency resolution, lifecycle, or ordinary mod source. ADG-3 may authorize a
per-version crate only if evidence demonstrates that separately compiled types,
artifacts, dependencies, or safety boundaries require it. M4 proves the default
model structurally with synthetic second-version data; shipping that version is
not required.

### Public/private workspace boundary

The private compatibility workspace is a different repository from its first
commit. Its target shape is:

```text
ravel-compat-private/
├── Cargo.toml
├── counterparts/
│   └── <canonical-ravel-id>/        # one reusable package per pinned entry
├── integrations/
│   └── <cluster>/
├── pack/
│   ├── manifest.toml
│   ├── config/
│   ├── data/
│   └── rust-modules/
├── inventory/
│   ├── manifest-entries/
│   ├── dependency-graph/
│   ├── behaviour-specs/
│   └── provenance-ledger/
├── fixtures/
│   └── official-atm11/              # ignored; reconstructed locally
├── tests/
│   ├── counterpart/
│   ├── clusters/
│   ├── differential/
│   └── scenarios/
├── worlds/
│   └── canonical/                   # ignored/private test worlds
└── evidence/
    └── m5-m11/
```

The public CLI and schemas may be consumed by this workspace. The public
repository never consumes private source paths, private crate dependencies, or
private fixture content. General SDK gaps discovered privately return as
capability proposals plus original public test-mod acceptance cases.

## Nix and devenv design

`devenv` is the canonical local task surface. Tangled Spindle CI runs
`devenv test` and `nix flake check` in a Nixery environment. Nix pins the tools
and native dependencies; Cargo remains the Rust build system. Spindle covers
Linux only, while the Darwin runbook defines the required native Mac gate.

cargo2nix is intentionally deferred during M0. At this stage, committing a
generated `Cargo.nix` would duplicate the rapidly changing Cargo dependency
graph without producing a Nix-owned runtime artifact. Re-evaluate it after
ADG-1 and ADG-2, before M2 packages the first runnable mod, when per-crate Nix
caching, native dependency wiring, or distributable Nix package outputs can
justify the additional generated file and regeneration gate.

### Root files

- `devenv.yaml` declares devenv inputs and pinning policy.
- `devenv.lock` records their exact revisions.
- `devenv.nix` imports the modules under `nix/`, enables Rust using
  `languages.rust.toolchainFile = ./rust-toolchain.toml`, and declares tasks,
  tests, packages, environment variables, and git hooks.
- `flake.nix` is a thin export for `x86_64-linux` and `aarch64-darwin`
  `devShells` and checks. It imports the same Nix modules and contains no
  duplicate package or task list.
- `flake.lock` pins the flake-facing inputs.
- `rust-toolchain.toml` pins an exact stable Rust release, minimal profile,
  `rustfmt`, `clippy`, and the two initial compilation targets. WebAssembly
  targets are added only if ADG-2 selects them.

M0-W1 records the exact supported devenv version and establishes a parity check
between `devenv test` and `nix flake check`. If maintaining both lock surfaces
causes input drift, the thin flake is generated and checked from the canonical
devenv inputs rather than maintaining duplicate package definitions.

### Environment contents

Common packages include Cargo/Rust tools, `jj`, `git`, `pkg-config`, archive and
hash tools, `jq`, a licence/advisory checker, nextest, llvm-cov, and benchmark
utilities.

Linux-only packages include the approved process/memory samplers, `perf`,
Vulkan diagnostics, and native link dependencies selected by ADG-1/2. Darwin
configuration contains only Nix-provided build dependencies; Apple system tools
such as Instruments are documented prerequisites and invoked through guarded
collector scripts.

The game JDK is not guessed. M0 reads the pinned Minecraft/Prism metadata,
records the required Java runtime, and then pins the matching development JDK
where loader tooling needs one. This avoids turning a convenience JDK into an
unreviewed runtime decision.

Use `lib.optionals pkgs.stdenv.isLinux` and
`lib.optionals pkgs.stdenv.isDarwin` for packages and `lib.mkIf`/`lib.mkMerge`
for platform modules. Unsupported platforms fail with a clear message.

### Canonical tasks

| Task | Purpose |
| --- | --- |
| `ravel:fmt` | Rust, Nix, shell, TOML and Markdown formatting checks |
| `ravel:typecheck` | `cargo check` for workspace targets and features |
| `ravel:lint` | clippy, Nix lint and shell lint |
| `ravel:test` | nextest, doctests, compile tests and schema tests |
| `ravel:coverage` | llvm-cov with the workspace floor |
| `ravel:audit` | advisories, licences, duplicate dependency policy |
| `ravel:verify-scaffold` | topology, schema, version-boundary and tracked-file policy |
| `ravel:verify` | dependency-ordered aggregate of the preceding tasks |
| `ravel:verify-environment` | complete pinned devenv and Nix flake gates |
| `ravel:fixture-reconstruct` | authorized local reconstruction into ignored paths |
| `ravel:fixture-verify` | manifest count and content hash verification |
| `ravel:baseline` | run a named platform/workload profile |
| `ravel:verify-milestone` | validate an evidence bundle against a milestone |

Git hooks run fast formatting, lint, schema, secret, and forbidden-file checks.
The Tangled Spindle pipeline runs the complete Linux environment gate, and
milestone gates add recorded native Darwin evidence where required. Hooks never
replace CI.
CLI launch commands are limited to reproducible development/test invocation and
handoff to a Prism-managed instance. They do not own authentication, Minecraft
installation, instance management, pack acquisition, or general launcher UI.

Current configuration guidance used by this plan:

- [devenv Rust language configuration](https://github.com/cachix/devenv/blob/main/docs/src/languages/rust.md)
- [devenv cross-platform recipe](https://github.com/cachix/devenv/blob/main/docs/src/recipes/cross-platform.md)
- [devenv pinning](https://github.com/cachix/devenv/blob/main/docs/src/pinning.md)
- [devenv flake-parts integration](https://github.com/cachix/devenv/blob/main/docs/src/guides/using-with-flake-parts.md)

## Milestone roadmap

### M0 — Reproducible baselines

**Purpose:** establish reproducible tooling, legal fixture boundaries, and the
measurement oracle before choosing architecture.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M0-W1 | Initialize Cargo, Nix, and devenv roots | none | root manifests, locks, platform modules, pinned Rust | `devenv test` and `nix flake check` pass on Linux and Mac |
| M0-W2 | Create verification and evidence skeleton | M0-W1 | `ravel-xtask`, evidence/schema contract, base CI | a deliberately incomplete M0 bundle is rejected; a synthetic complete bundle passes |
| M0-W3 | Define fixture manifest and ignore policy | M0-W1 | pinned metadata, authorized-source descriptors, ignore/secret rules | fixture and secret canaries are rejected from the tracked tree |
| M0-W4 | Implement fixture reconstruction and hashing | M0-W3 | CLI command and local-only fixture output | exact MC/NeoForge/ATM versions, expected reference count, and all recorded hashes verify |
| M0-W5 | Specify canonical performance protocol | M0-W2 | versioned workload/profile and collector schemas | schema and dry-run collector tests pass on Linux and Mac |
| M0-W6 | Record hardware, Java, Prism, and launch inputs | M0-W3 | sanitized hardware profiles and exact runtime metadata | clean-machine runbook reproduces vanilla launch on both platforms |
| M0-W7 | Capture vanilla and official ATM11 baselines | M0-W4..W6 | Linux and Mac result bundles | required memory, pressure, frame, tick, startup and world-load fields validate |
| M0-W8 | Set M1/M2 interim overhead budgets | M0-W7 | reviewed numeric budget ADR derived from measured headroom | ADR names thresholds and failure action for every M1 benchmark |
| M0-W9 | Independent milestone review | M0-W1..W8 | review record and M0 evidence manifest | `devenv tasks run ravel:verify-milestone:m0` passes; tracked-tree audit is clean |

M0 may download authorized files only into ignored local paths. A failed rights,
hash, count, or redaction check blocks the gate.

### M1 — Architecture spikes

**Purpose:** select ADG-1 through ADG-5 with comparable evidence rather than
locking the workspace to an assumed runtime.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M1-W1 | Build common spike harness | M0 | shared benchmark protocol and trivial callback workload | every candidate reports the same metric schema |
| M1-W2 | ADG-1 JVM entry/hook comparison | M1-W1 | candidate code, Linux/Mac data, ADR | bootstrap RSS, retained classes, startup, callback, bytes and allocation metrics meet M0 budgets or rejection is explicit |
| M1-W3 | ADG-2 Rust execution/ABI comparison | M1-W1 | dylib/Wasm/process/static evidence as viable, ADR | memory, p50/p95/p99 call cost, crash isolation, packaging, debug and evolution rubric complete |
| M1-W4 | ADG-3 binding strategy comparison | M1-W2 | extraction/codegen/cache prototype and ADR | a generated 26.1.2 binding is reproducible from pinned inputs and fails on drift |
| M1-W5 | ADG-4 state-ownership experiment | M1-W2..W4 | representative object-flow benchmark and ADR | duplicated state and boundary traffic are measured under the canonical spike |
| M1-W6 | ADG-5 Prism integration spike | M0-W6, M1-W2 | stock launch proof or patch estimates and ADR | official Minecraft launches without another mod loader; auth tokens are absent from captures |
| M1-W7 | Architecture synthesis | M1-W2..W6 | overview and dependency/safety boundary docs | all selected/rejected options trace to evidence and have Linux plus viable Mac results |
| M1-W8 | Independent architecture review | M1-W7 | review record | reviewer confirms safe-Rust feasibility and plausible M9 budget; M1 gate passes |

No production crate may encode a selected mechanism before its ADR is accepted.
If safe wrappers cannot support a viable result, work stops for an explicit
spec decision.

### M2 — Loader skeleton

**Purpose:** prove the complete pre-world loader pipeline with one no-op
precompiled Rust package.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M2-W1 | Implement canonical IDs, sides, versions and diagnostics | M1 | `ravel-core` | unit/property tests cover valid and invalid values |
| M2-W2 | Implement package schema, targets and integrity | M2-W1, ADG-2 | `ravel-package` and JSON schema | valid packages round-trip; tamper, target and schema failures precede code execution |
| M2-W3 | Implement dependency resolution and load plans | M2-W2 | `ravel-resolve` | required/optional/incompatible/discouraged, cycles, duplicates, ranges and deterministic-order cases pass |
| M2-W4 | Implement lifecycle state machine | M2-W1 | loader state machine | every legal and illegal transition plus every spec invariant is tested |
| M2-W5 | Implement selected bootstrap and adapter shell | M1 ADRs, M2-W4 | bootstrap, host API, shared adapter plus 26.1.2 version data | official 26.1.2 process reaches Ravel without another loader |
| M2-W6 | Package and execute no-op test mod | M2-W2..W5 | precompiled no-op package | recorded evidence shows validate/resolve/load/construct/register/run/stop in deterministic order |
| M2-W7 | Add world-access and structured-failure gates | M2-W3..W6 | pre-world UI/log diagnostic path | invalid packages, sides, integrity and graphs cannot open a world; reports redact secrets/paths |
| M2-W8 | Measure loader overhead and review | M2-W6..W7 | benchmark vs M0 vanilla and review | M0 overhead budget passes and M2 gate passes |

### M3 — Credible loader prototype

**Purpose:** demonstrate useful loader semantics with original unrestricted test
mods on both target platforms.

Before M3 closes, ADG-6 selects the configuration schema/UI model with a
standalone original test case.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M3-W1 | Implement registry barrier and content registration | M2 | block/item/recipe test mods | duplicate/conflicting claims fail; IDs are immutable after registration |
| M3-W2 | Implement commands, events, resources and data | M3-W1 | command/event/resource test mods | process tests exercise success, ordering, side and failure cases |
| M3-W3 | Implement ADG-6 configuration and generated UI | M2, ADG-6 | schema, layered config and screen test mod | file/UI edits round-trip; validation, sides, restart and pack overrides pass |
| M3-W4 | Implement versioned saved data | M3-W1 | save test mod and world fixture | create/save/exit/reload preserves state; incompatible/missing required data fails closed |
| M3-W5 | Implement in-game mod/dependency surface | M2-W7 | mod list and diagnostics UI | valid and invalid graphs render deterministic actionable messages |
| M3-W6 | Implement opt-in profiling | M2-W8 | collectors and profiling UI/report | per-mod attribution accuracy and unsupported metrics are explicit; normal-mode budget passes |
| M3-W7 | Cross-platform process matrix | M3-W1..W6 | Linux and M1 Pro result bundle | every required surface passes on both; instance scan proves no other loader |
| M3-W8 | Independent prototype review | M3-W7 | review record | M3 gate passes and authorizes private inventory work only after M4 |

### M4 — General SDK breadth

**Purpose:** turn prototype internals into a documented, safe, reusable mod and
pack authoring surface.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M4-W1 | Stabilize experimental SDK facade and macros | M3 | `ravel-sdk`, macros, minimal example | compile-pass/fail suite proves authors need no loader internals or Java |
| M4-W2 | Complete registries, tags, resources and generated data | M4-W1 | public APIs and content example | conformance suite covers declarations, conflicts and deterministic output |
| M4-W3 | Complete hooks, events, sides and networking abstractions | M4-W1 | public APIs and client/server test mods | client-only code never loads on server sides; packet contracts round-trip |
| M4-W4 | Complete config, save migrations and inter-mod services | M4-W1 | public schemas/APIs and examples | migration chains, service dependencies and failure closure pass |
| M4-W5 | Complete pack declarations and validator | M4-W2..W4 | `ravel-pack`, pack example | dependency, resource, registry and progression closure checks pass |
| M4-W6 | Prove version isolation | M4-W1..W5 | synthetic second-version data fixture | shared adapter and general crates compile/test without 26.1.2-specific edits or a new crate |
| M4-W7 | Agent usability acceptance | M4-W1..W6 | docs task and resulting original test mod | a fresh agent receives only public docs and builds a conforming mod without internal access |
| M4-W8 | Cross-platform review and gate | M4-W7 | evidence and review | global gate, process matrix, SDK docs and M4 milestone gate pass |

The SDK remains semver-experimental through M10 even though its behaviour is
testable here.

### M5 — Private compatibility inventory

**Purpose:** convert the pinned official fixture into a rights-aware,
implementation-independent work inventory without porting code.

This milestone runs in the private workspace and consumes released local
artifacts from M4.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M5-W1 | Initialize private repo and boundary checks | M4, M0 fixture tooling | private topology and provenance schema instance | no private path/dependency enters public workspace; official fixture remains ignored |
| M5-W2 | Verify exact manifest population | M5-W1 | one record per pinned reference | count and project/file IDs reconcile to the fixture; corrections require source/hash evidence |
| M5-W3 | Build dependency and integration graph | M5-W2 | machine-readable graph and clusters | every node/edge resolves or has a blocking classification |
| M5-W4 | Catalogue behaviour surfaces | M5-W2 | config/resource/KubeJS/packet/save/side inventories | every source artifact maps to a record or evidenced dead/unreachable classification |
| M5-W5 | Complete rights and provenance triage | M5-W2 | per-code/asset/name/branding rights status | unresolved status blocks publication but not private black-box work; prohibited source use is flagged |
| M5-W6 | Assign canonical IDs and port plans | M5-W3..W5 | owner, method, dependencies, tests and risk score per entry | 100% inventory coverage; IDs are unique and Ravel-native |
| M5-W7 | Produce general SDK gap proposals | M5-W4..W6 | public-safe capability proposals and original test cases | proposals contain no restricted code/assets/source-derived notes |
| M5-W8 | Inventory review and gate | M5-W1..W7 | private evidence manifest | 235/235 or evidence-corrected total has a complete plan; M5 gate passes |

The ranking rubric scores foundation value, dependency centrality,
integration breadth, compatibility uncertainty, rights risk, expected memory
impact, and vertical-slice value on a documented 1–5 scale.

### M6 — Foundation counterpart set

**Purpose:** validate the port factory and the smallest nontrivial gameplay
clusters before scaling to every entry.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M6-W1 | Select foundation wave from M5 rubric | M5 | frozen wave manifest and cluster scenario | every selection reason and dependency is explicit |
| M6-W2 | Resolve general SDK gaps publicly | M5-W7 | public SDK changes plus original test mods | public M4-class gates pass before private counterparts consume them |
| M6-W3 | Implement foundation libraries/APIs | M6-W1..W2 | individually packaged counterparts | per-package unit, registration, save, side, performance and provenance suites pass |
| M6-W4 | Implement smallest gameplay cluster | M6-W3 | cluster packages and integration | fresh play, save, exit and reload scenario passes |
| M6-W5 | Validate port factory | M6-W3..W4 | counterpart template, checklist and evidence schema | a second independent counterpart follows it without hidden loader hooks |
| M6-W6 | Review and gate | M6-W5 | private review/evidence | admitted set is green, provenance complete, M6 gate passes |

### M7 — Complete counterpart library

**Purpose:** execute the validated port factory until every pinned manifest
entry has an accepted reusable Rust counterpart.

Work proceeds in dependency-ordered waves. A wave cannot close with failing or
unclassified entries.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M7-W1 | Freeze waves and cluster ownership | M6 | ordered wave manifests | graph closure proves no wave depends on a later unavailable wave |
| M7-W2 | Implement and test each counterpart | M7-W1 | one package and evidence record per entry | standard per-counterpart suite passes, including lawful differential tests |
| M7-W3 | Test integration clusters after each wave | M7-W2 | cluster reports | cross-mod registry, recipe, resource, side, save and performance cases pass |
| M7-W4 | Re-run full closure and rights status | each wave | burn-up ledger | accepted/blocked/unstarted totals equal verified fixture total |
| M7-W5 | Final 100% audit | all waves | complete library manifest | 235/235 or evidence-corrected total; zero unresolved dependencies/registries; all provenance present |
| M7-W6 | Independent review and gate | M7-W5 | private review/evidence | all package and cluster suites pass; M7 gate passes |

“Counterpart present” means its package builds for its declared targets, passes
the standard suite, implements its accepted behaviour specification, records
provenance and rights status, and participates in dependency/registry closure.
A stub or metadata-only placeholder does not count.

### M8 — Complete Ravel pack

**Purpose:** assemble the counterpart library with pinned configuration and all
reachable executable pack behaviour.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M8-W1 | Pin pack manifest and counterpart versions | M7 | reproducible private pack manifest | package, target, dependency and integrity closure pass |
| M8-W2 | Port configuration and declarative content | M8-W1 | schema-validated config/data | every M5 inventory record maps to output or evidenced exclusion |
| M8-W3 | Port custom KubeJS behaviour to Rust modules | M8-W1 | compiled safe-Rust pack modules | scenario equivalence passes; runtime scan finds no JS engine |
| M8-W4 | Build full scenario suite | M8-W2..W3 | fresh-world and integration scenarios | intended content/progression inventory has complete scenario coverage |
| M8-W5 | Repeated save/reload soak | M8-W4 | world and migration evidence | repeated clean shutdown/reload cycles preserve state; failure cases fail closed |
| M8-W6 | Independent pack review and gate | M8-W1..W5 | private review/evidence | no Java/JavaScript mods execute; all pack/scenario suites and M8 gate pass |

### M9 — Constrained-device performance

**Purpose:** prove the complete single-player pack meets the fixed M1 Pro
contract without silently reducing scope or settings.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M9-W1 | Freeze comparable performance inputs | M8, M0 profile | exact build, world, route and profile hashes | verifier rejects any unrecorded setting or content drift |
| M9-W2 | Attribute current costs | M9-W1 | per-mod/boundary/JVM/GPU/memory profile | exact vs estimated metrics are labelled; profiling overhead is separately measured |
| M9-W3 | Optimize without behavioural regression | M9-W2 | reviewed loader/SDK/counterpart changes | relevant M4, M7 and M8 suites remain green after every optimization |
| M9-W4 | Run final constrained acceptance | M9-W3 | repeated M1 Pro result bundles | all fixed FPS/RSS/pressure/swap and frame/tick reporting requirements pass |
| M9-W5 | Independent performance review | M9-W4 | review and reproducibility rerun | reviewer reproduces the result; M9 gate passes |

If M9-W4 cannot pass with the complete workload, open ADG-8. Do not mark M9
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
| M10-W5 | Final independent audit | M0..M10 | complete evidence index and review | all manifests pass; public/private boundary audit is clean; no blocker remains; M10 gate passes |

Passing M10-W5 is **initial completion**.

The final boundary audit proves that the public tracked tree and Cargo metadata
contain no private path dependency, counterpart source, official fixture,
restricted asset, private world, rights-sensitive source note, credential,
launcher token, or unredacted sensitive log.

### M11 — Official ATM11 server interoperability

M11 begins only after initial completion and an accepted ADG-7 protocol
specification. Inventory handshake, identity/version negotiation, registry sync,
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

Post-initial and not implementation-ready. It requires a separate plan covering
name collision checks, dependency/licence audit, per-counterpart publication
clearance, asset authorization, signing and publisher identity, moderation,
distribution, rollback, and private-history isolation.

## Traceability matrix

| Spec concern | Primary implementation | Proof |
| --- | --- | --- |
| Loader identity/runtime | bootstrap, loader, host API | M1 ADRs; M2/M3 process scans |
| Safe Rust boundary | all maintained crates; audited dependencies | policy scan, dependency audit, architecture review |
| Packages/dependencies | package, resolve, pack | schema/property/negative tests; M7/M8 closure |
| Lifecycle/invariants | core and loader | state-machine and process conformance suites |
| Version isolation | Minecraft traits, shared adapter, and per-version data | forbidden-reference scan; synthetic version-data test |
| Configuration/UI | config and adapter UI integration | ADG-6; round-trip process tests |
| Resources/data | data and pack | schema and Minecraft-process tests |
| Saves/migrations | save and loader world gate | repeated save/reload and migration matrix |
| Diagnostics/profiling | observability | attribution, redaction, accuracy and overhead tests |
| Prism ownership | CLI/runbook/bootstrap | stock launch evidence or accepted ADG-5 patch |
| Public/private separation | separate repos and schemas | dependency/path/content audit |
| Counterpart completeness | private inventory and packages | verified-total ledger and per-package suites |
| Pack completeness | private pack and scenarios | full inventory mapping, closure and soak |
| Performance | performance profiles/collectors | M0 baselines and M9 repeated acceptance |
| Progression | private checklist/world | M10 journal plus regressions |
| Official server protocol | network plus private adapters | ADG-7 and M11 cross-client suite |

## Open questions and decision deadlines

| Question | Deadline | Blocking effect |
| --- | --- | --- |
| Exact fixture reference total and authoritative hashes | M0-W4 | blocks M0 |
| Canonical workload duration, repetitions and metric definitions | M0-W5 | blocks M0 |
| Game Java runtime required by pinned metadata | M0-W6 | blocks reproducible launch |
| Interim loader/boundary overhead budgets | M0-W8 | blocks M1 selection |
| JVM hook mechanism | ADG-1 / M1 | blocks M2 bootstrap |
| Rust execution/package/ABI model | ADG-2 / M1 | blocks M2 package execution |
| Binding extraction and generation | ADG-3 / M1 | blocks real adapter |
| Java/Rust state ownership | ADG-4 / M1 | blocks loader architecture |
| Stock Prism versus minimal patch | ADG-5 / M1 | blocks supported launch path |
| Configuration and UI schema | ADG-6 before M3-W3 | blocks M3 |
| Per-counterpart rights and implementation method | M5 | blocks publication; may block lawful implementation |
| Official server wire surface | ADG-7 after M10 | blocks M11 only |
| Official host viability | ADG-8 only after M9 failure | leaves M9 incomplete |
| Registry identity/signing/moderation | separate M13 plan | no effect on initial completion |

## Immediate handoff

The plan is ready for M0 implementation. The first local change sequence should
be:

1. M0-W1: initialize the Cargo workspace and Nix/devenv environment without
   choosing ADG-1 through ADG-4.
2. M0-W2: implement the evidence schema, `ravel-xtask` gate skeleton, and CI
   parity.
3. M0-W3 through M0-W5: establish the ignored fixture contract and canonical
   performance protocol before downloading or benchmarking anything.
4. Review that foundation, then capture M0 baselines on the two specified
   machines.

Do not begin a loader implementation until M1's selected architecture ADRs pass
their review.
