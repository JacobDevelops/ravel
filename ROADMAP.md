# Ravel implementation roadmap

Status: implementation-ready plan  
Authority: subordinate to [`SPEC.md`](SPEC.md)  
Initial-completion gate: M10  
Last updated: 2026-07-30

## Capability

Ravel enables a player to run a complete, natively compiled Rust modpack inside
official Minecraft: Java Edition, initially for Minecraft 26.1.2, without
another Java mod loader. Initial completion means that the separate private
ATM11-equivalent pack passes its automated compatibility suites, meets the
Linux performance contract, and completes the pinned progression path in a
fresh Ravel world. Apple Silicon support is a separate phase after that complete
Linux result. The reason to build it is resource cost: a complete Rust pack
should cost significantly less RAM and CPU than the equivalent Java pack, and
[`SPEC.md`](SPEC.md) §1 states that goal against §6's thresholds.

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

M0 through M10 are required for Linux initial completion. Apple Silicon
platform expansion starts only after that gate. M11 is a later interoperability
release, M12 requires a new capability amendment, and M13 is post-initial
publication and registry work.

This interpretation follows the spec's Linux-first, single-player ordering,
pack completion evidence, M9 Linux performance gate, M10 progression
acceptance, and explicit description of macOS and official ATM11 multiplayer as
later work.

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
- Linux x86_64 is the only required gate for M0 through M10.
- Apple Silicon macOS work starts after Linux M10 has passed with the loader,
  counterpart library, complete pack, constrained performance, and progression
  evidence already accepted. No macOS build, launch, capture, packaging,
  verification, or platform-specific investigation belongs in M0 through M10.
- Tangled Spindles provide the Linux gate. Because Spindle engines are
  Linux-based, any future Darwin gate runs on an actual `aarch64-darwin`
  machine via `docs/runbooks/darwin-verification.md`; a Linux evaluation never
  substitutes for required Mac evidence once that post-M10 phase begins.
- A gate rejects untracked restricted fixture patterns, credential patterns,
  forbidden loader artifacts, or maintained Rust that permits unsafe code.

### Performance evidence

M0 records enough Linux launch evidence to prove vanilla 26.1.2 and official
ATM11 0.2.1 run from the pinned Prism setup without restricted material or
credentials entering the repository. It does not require performance capture
or repeated launches.

The single-pair performance protocol lives in
`tests/performance/profiles/initial.toml` for later M9 acceptance. It contains
the canonical world, route, warm-up, duration, graphics and
simulation settings, active content, sampling intervals, and pass/fail
calculations. M9 cannot close until those values are explicit and the complete
Linux workload passes them.

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
 └─> M1 ADG-1..5 architecture evidence
      └─> M2 loader skeleton
           └─> M3 credible Linux loader
                ├─> M4 general SDK breadth
                │    └─> M5 private compatibility inventory
                │         └─> M6 foundation counterparts
                │              └─> M7 complete counterpart library
                │                   └─> M8 complete pack
                │                        └─> M9 Linux performance
                │                             └─> M10 progression acceptance
                │                                  └─> Linux initial completion
                └─> ADG-6 configuration/UI decision before M3 closes

M10 └─> ADG-8 └─> M11 official-server interoperability
M10 └─> post-Linux-M10 Apple Silicon platform expansion
M9 failure └─> ADG-9; it does not relax M9 or authorize a native client
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
├── jvm/                            # [ADG-1] agent jar sources; ADG-6's screen is a separate artifact
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
| `ravel-package` | Manifest parsing, compatibility ranges, source declarations, source-file hashes, provenance fields | core, serialization, version/range libraries | loader, bootstrap, Minecraft adapter |
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
wrappers.

M1 set out to admit one exception — the exported JNI symbol — and the M1-W8 review
refuted the premise: `jni` 0.22.4's `jni_mangle` attribute emits the export, and a
crate keeping `#![forbid(unsafe_code)]` verbatim compiles and exports the mangled
symbol. No exception is needed and none exists;
[`safety-boundary.md`](docs/architecture/safety-boundary.md) records an empty list.
Adding one takes an accepted ADR plus an independent review, and the bar is showing
that no audited dependency can hold the operation.

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
Linux only. The Darwin runbook is dormant until the post-Linux-M10 Apple
Silicon phase begins.

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
The Tangled Spindle pipeline runs the complete Linux environment gate. Native
Darwin evidence is introduced only in the post-Linux-M10 platform phase. Hooks
never replace CI.
CLI launch commands are limited to reproducible development/test invocation and
handoff to a Prism-managed instance. They do not own authentication, Minecraft
installation, instance management, pack acquisition, or general launcher UI.

Current configuration guidance used by this plan:

- [devenv Rust language configuration](https://github.com/cachix/devenv/blob/main/docs/src/languages/rust.md)
- [devenv cross-platform recipe](https://github.com/cachix/devenv/blob/main/docs/src/recipes/cross-platform.md)
- [devenv pinning](https://github.com/cachix/devenv/blob/main/docs/src/pinning.md)
- [devenv flake-parts integration](https://github.com/cachix/devenv/blob/main/docs/src/guides/using-with-flake-parts.md)

## Milestone roadmap

### M0 — Linux launch smoke proof

**Purpose:** establish reproducible tooling, legal fixture boundaries, and
Linux launch proof before choosing architecture. Performance capture begins
only with the single M9 comparison pair.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M0-W1 | Initialize Cargo, Nix, and devenv roots | none | root manifests, locks, platform modules, pinned Rust | `devenv test` and `nix flake check` pass on Linux |
| M0-W2 | Create verification and evidence skeleton | M0-W1 | `ravel-xtask`, evidence/schema contract, base CI | a deliberately incomplete M0 bundle is rejected; a synthetic complete bundle passes |
| M0-W3 | Define fixture manifest and ignore policy | M0-W1 | pinned metadata, authorized-source descriptors, ignore/secret rules | fixture and secret canaries are rejected from the tracked tree |
| M0-W4 | Implement fixture reconstruction and hashing | M0-W3 | CLI command and local-only fixture output | exact MC/NeoForge/ATM versions, expected reference count, and all recorded hashes verify |
| M0-W5 | Specify later performance protocol shape | M0-W2 | versioned workload/profile and collector schemas | schema and dry-run collector tests pass on Linux; no native repeated capture is required |
| M0-W6 | Record Linux hardware, Java, Prism, and launch inputs | M0-W3 | sanitized Linux hardware profile and exact runtime metadata | clean-machine runbook reproduces vanilla launch on Linux |
| M0-W7 | Prove Linux vanilla and official ATM11 launch | M0-W4..W6 | Linux launch result bundle and workload attestation | vanilla and official ATM11 reach a playable world through Prism; credentials and restricted content are absent from evidence |
| M0-W8 | Define M1 architecture measurement gate | M0-W7 | reviewed metric and failure-action plan | every M1 candidate reports comparable Linux memory, startup, call-cost, allocation, packaging, and failure evidence without requiring an M0 baseline |
| M0-W9 | Independent milestone review | M0-W1..W8 | review record and M0 evidence manifest | `devenv tasks run ravel:verify-milestone:m0` passes; tracked-tree audit is clean |

M0 may download authorized files only into ignored local paths. A failed rights,
hash, count, or redaction check blocks the gate.

### M1 — Architecture spikes

**Purpose:** select ADG-1 through ADG-5 with comparable evidence rather than
locking the workspace to an assumed runtime.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M1-W1 | Build common spike harness | M0 | shared benchmark protocol and trivial callback workload | every candidate reports the same metric schema |
| M1-W2 | ADG-1 JVM entry/hook comparison | M1-W1 | candidate code, Linux data, ADR | bootstrap RSS, retained classes, startup, callback, bytes and allocation metrics are measured on Linux; rejection is explicit |
| M1-W3 | ADG-2 Rust execution/ABI comparison | M1-W1 | dylib/Wasm/process/static evidence as viable, ADR recording the measurements and the trade each makes | memory, p50/p95/p99 call cost, crash isolation, packaging, debug and evolution rubric complete. The ADR records measurements only: the selection is deferred to M3, because it turns on loader properties M1 cannot observe. |
| M1-W4 | ADG-3 binding strategy comparison | M1-W2 | extraction/codegen/cache prototype and ADR | a generated 26.1.2 binding is reproducible from pinned inputs and fails on drift |
| M1-W5 | ADG-4 state-ownership experiment | M1-W2..W4 | representative object-flow benchmark and ADR | duplicated state and boundary traffic are measured under the canonical spike |
| M1-W6 | ADG-5 Prism integration spike | M0-W6, M1-W2 | stock launch proof or patch estimates and ADR | official Minecraft launches without another mod loader; auth tokens are absent from captures |
| M1-W7 | Architecture synthesis | M1-W2..W6 | overview and dependency/safety boundary docs | all selected and rejected options trace to Linux evidence |
| M1-W8 | Independent architecture review | M1-W7 | reviewer, result, and record hash claimed in the M1 manifest, and the record tracked beside it at `evidence/m1/review.md`, whose digest the gate recomputes and whose stated verdict the gate requires `review.result` to match | reviewer confirms safe-Rust feasibility, and confirms the per-crossing costs and the rate at which they would become material; M1 gate passes |

No production crate may encode a selected mechanism before its ADR is accepted.
If safe wrappers cannot support a viable result, work stops for an explicit
spec decision.

### M2 — Loader skeleton

**Purpose:** prove the complete pre-world loader pipeline with one no-op Rust
package, prebuilt as a binary because M2 ran before ADG-2 chose source
distribution.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M2-W1 | Implement canonical IDs, sides, versions and diagnostics | M1 | `ravel-core` | unit/property tests cover valid and invalid values |
| M2-W2 | Implement package schema and integrity | M2-W1 | `ravel-package` and JSON schema | valid packages round-trip; tamper and schema failures precede code execution. The original target model reflected M2 before ADG-2; schema v1 now requires integrity-protected portable source files and refuses the old target form. |
| M2-W3 | Implement dependency resolution and load plans | M2-W2 | `ravel-resolve` | required/optional/incompatible/discouraged, cycles, duplicates, ranges and deterministic-order cases pass |
| M2-W4 | Implement lifecycle state machine | M2-W1 | loader state machine | every legal and illegal transition plus every spec invariant is tested |
| M2-W5 | Implement selected bootstrap and adapter shell | M1 ADRs, M2-W4 | bootstrap, host API, shared adapter plus 26.1.2 version data | official 26.1.2 process reaches Ravel without another loader |
| M2-W6 | Package and execute no-op test mod | M2-W2..W5 | source-only no-op package | recorded evidence shows validate/resolve/load/construct/register/run/stop in deterministic order |
| M2-W7 | Add world-access and structured-failure gates | M2-W3..W6 | pre-world UI/log diagnostic path | invalid packages, sides, integrity and graphs cannot open a world; reports redact secrets/paths. The gate answered that question with no caller until 2026-08-09; ADG-7's world-open crossing now asks it, and the note below says which half of it a launch can reach |
| M2-W8 | Measure loader overhead and set regression budget | M2-W6..W7 | Linux no-op loader benchmark and review | the measured loader cost establishes the numeric regression budget used from M3 onward; M2 gate passes. Three of its six limits have since expired — see below |

**M2-W7's gate is now consulted by the game process, and only one of its two
halves can refuse there.** `LifecycleMachine::open_world` had no caller outside
tests, so "cannot open a world" was proven as *the loader answers no when asked*
and nothing in a launch asked. ADG-7's open crossing asks it now:
`crates/ravel-bootstrap/src/persistence.rs` consults the gate before it touches a
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

**Purpose:** demonstrate useful loader semantics with original unrestricted test
mods on Linux. The target-platform expansion waits until after Linux M10.

**ADG-6 is accepted**, before M3-W3 as this table requires. Configuration is
declared in data files, the settings screen is drawn in Java, and Rust supplies
the values; Ravel does not render from Rust, which is what makes M3-W5's screen
the same mechanism as this one rather than a second one. The record is
[`adg-6-configuration-ui.md`](docs/decisions/adg-6-configuration-ui.md). It
resolves the repository's two incompatible readings of "pack override" in favour
of the per-setting model, and names three schema changes M3-W3 makes rather than
making them.

**M3 closes ADG-2.** Mods are independently shipped and interchangeable and a
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
part of the M2 and ADG-2 historical record.

Accepted records were amended for M3 without any of them changing what it decided,
and each amendment is recorded in the record it amends rather than counted here —
the `Status` line of every file in [`docs/decisions/`](docs/decisions/) is what
says which were touched and when. ADG-4 admits an opaque handle to an object the
*game* owns, which is what lets the generator carry M3's real hook targets —
essentially all of them carry game objects, and under the strict reading the
generator admitted almost none of this milestone's work. ADG-1 records that three
artifacts enter the game process rather than two, that the screen ADG-6 decided is
not the agent jar's to carry, and that the always-present jar declares no game
type while naming some reflectively.

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
| M3-W0 | Grow the 26.1.2 hook inventory to M3's targets | M2, ADG-4 | resolved target record, `hooks.toml` entries, re-pinned `adapter.toml` hash and regenerated `mappings.lock` | targets are resolved against the pinned jar by two independent passes and recorded in [`m3-hook-targets.md`](docs/architecture/m3-hook-targets.md); the inventory edit that lands them is separate and is what the hash re-pin and lock regeneration gate |
| M3-W1 | Implement registry barrier and content registration | M2 | block/item/recipe test mods | duplicate/conflicting claims fail; IDs are immutable after registration |
| M3-W2 | Implement commands, events, resources and data | M3-W1 | command/event/resource test mods | process tests exercise success, ordering, side and failure cases |
| M3-W3 | Implement ADG-6 configuration and generated UI | M2, ADG-6 | the three schema edits ADG-6 names, layered config, generated Java screen and a screen test mod | file/UI edits round-trip; validation, sides, restart and per-setting pack overrides pass. ADG-1 selected the agent appending the sibling screen jar through `Instrumentation.appendToSystemClassLoaderSearch`, so artifact loading no longer blocks this work |
| M3-W4 | Implement versioned saved data | M3-W1, ADG-7 | save test mod and world fixture, over the deciding half `ravel-save` already models — what a version is, who owns it, and whether a world may be opened | the pinned fixer-byte measurement ADG-7 records preserves opaque bytes; create/save/exit/reload preserves state; schema, package, migration and size incompatibilities fail closed. What remains is moving the ADG-7 aggregate through the game objects ADG-4's amendment admits |
| M3-W5 | Implement in-game mod/dependency surface | M2-W7, ADG-6 | mod list and diagnostics UI, over the model `ravel-loader` already holds ordered by resolved load order | valid and invalid graphs render deterministic actionable messages. The drawing half is ADG-6's mechanism rather than a second one, so what remains is a screen and not a design |
| M3-W6 | Implement opt-in profiling | M2-W8 | collectors and profiling UI/report | per-mod attribution accuracy and unsupported metrics are explicit; normal-mode budget passes |
| M3-W7 | Linux process matrix | M3-W1..W6 | Linux result bundle | every required surface passes natively on Linux; instance scan proves no other loader |
| M3-W8 | Independent prototype review | M3-W7 | review record | M3 gate passes and authorizes private inventory work only after M4 |

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
claims no ADG-3 drift coverage for the agent jar: three of the classes its
persistence adapter resolves are hook targets and therefore pinned, and
everything below a class name — the members, their arities, the two
`LevelResource` field constants and the `SavedDataStorage` constructor — is in no
lock and owned by no work package.

This section's Verification column was restated in M4-W8, in the change that
recorded what M4 shipped. Each row was narrowed to the properties the milestone's
gate actually proves and each narrowing names what it now excludes, because the
original cells read as claims about a running game that nothing in M4 launched.
The restatement is recorded here rather than left to a diff, for the reason the
ADG-3 row above records its own retraction in place: whether a row met its gate
is a question a reader has to be able to answer from the tree.

### M5 — Private compatibility inventory

**Purpose:** convert the pinned official fixture into a rights-aware,
implementation-independent work inventory without implementing any counterpart.

This milestone runs in the private workspace and consumes released local
artifacts from M4.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M5-W1 | Initialize private repo and boundary checks | M4, M0 fixture tooling | private topology and provenance schema instance | no private path/dependency enters public workspace; official fixture remains ignored |
| M5-W2 | Verify exact manifest population | M5-W1 | one record per pinned reference | count and project/file IDs reconcile to the fixture; corrections require source/hash evidence |
| M5-W3 | Build dependency and integration graph | M5-W2 | machine-readable graph and clusters | every node/edge resolves or has a blocking classification |
| M5-W4 | Catalogue behaviour surfaces | M5-W2 | config/resource/KubeJS/packet/save/side inventories | every source artifact maps to a record or evidenced dead/unreachable classification |
| M5-W5 | Complete rights and provenance triage | M5-W2 | per-code/asset/name/branding rights status | unresolved status blocks publication but not private black-box work; prohibited source use is flagged |
| M5-W6 | Assign canonical IDs and counterpart plans | M5-W3..W5 | owner, method, dependencies, tests and risk score per entry | 100% inventory coverage; IDs are unique and Ravel-native |
| M5-W7 | Produce general SDK gap proposals | M5-W4..W6 | public-safe capability proposals and original test cases | proposals contain no restricted code/assets/source-derived notes |
| M5-W8 | Inventory review and gate | M5-W1..W7 | private evidence manifest | 235/235 or evidence-corrected total has a complete plan; M5 gate passes |

The ranking rubric scores foundation value, dependency centrality,
integration breadth, compatibility uncertainty, rights risk, expected memory
impact, and vertical-slice value on a documented 1–5 scale.

### M6 — Foundation counterpart set

**Purpose:** validate the counterpart-authoring process and the smallest
nontrivial gameplay clusters before scaling to every entry.

| ID | Work package | Depends on | Output | Verification |
| --- | --- | --- | --- | --- |
| M6-W1 | Select foundation wave from M5 rubric | M5 | frozen wave manifest and cluster scenario | every selection reason and dependency is explicit |
| M6-W2 | Resolve general SDK gaps publicly | M5-W7 | public SDK changes plus original test mods | public M4-class gates pass before private counterparts consume them |
| M6-W3 | Specify, then implement, foundation libraries/APIs | M6-W1..W2 | one accepted behaviour specification per entry, then individually packaged counterparts | no specification carries an upstream identifier; per-package unit, registration, save, side, performance and provenance suites pass |
| M6-W4 | Implement smallest gameplay cluster | M6-W3 | cluster packages and integration | fresh play, save, exit and reload scenario passes |
| M6-W5 | Validate counterpart-authoring process | M6-W3..W4 | counterpart template, checklist and evidence schema | a second independent counterpart follows it without hidden loader hooks; the rewrite-integrity check below runs over both counterparts and passes |
| M6-W6 | Review and gate | M6-W5 | private review/evidence | admitted set is green, provenance complete, the rewrite-integrity check is enforcing rather than advisory, each counterpart records which of its observable areas are declarable-only, and the M6 gate passes |

M6-W2 resolved five of the six general SDK capabilities M5-W7 proposed:
server-resolved data, world shaping, audio, drawing stages, and behaviour
interception, each with an original test mod under `test-mods/`. The sixth,
library provision, was deferred to M7 for the reason the open-questions row below
gives, and is now built there: `ravel_sdk::provision`, `test-mods/provision`, and
the last section of `tests/conformance/tests/sdk_gaps.rs`. None of the six
carries anything into a running game — nothing plays a sound, draws at a stage,
asks at an interception point, or reads a provision — which each module states
where an author reads it, in the same terms `docs/guide/limits.md` already uses
about packets.

Every counterpart is an original Rust implementation of an accepted behaviour
specification. Nothing decompiles, transliterates, or mechanically translates
Java, and no step of this plan produces Rust from a jar.

**A green counterpart is not an exercised one, and M6-W6 makes it say so.** The
six general capabilities are abstractions a mod declares against and tests
against; none of them carries anything into a running game, and the shipped host
discovers no package at all, so every counterpart in this milestone is a
library-tested artefact rather than something a player has seen work. A gate that
reported only "the admitted set is green" would therefore be satisfied by a pack
whose behaviour has never run once — the tests would be agreeing with the
declarations, and the case that contradicts them would be the one nobody built.
So each counterpart records which of its observable areas are *declarable-only*:
the abstraction exists, the implementation is real, the suites pass, and no
running process exercises it. That figure is not a defect to be driven to zero
here — it is the honest denominator this milestone's green sits over, and it is
what later carry work is ordered by.

**A specification and its implementation are written by different hands.** A
reader reads upstream source and writes prose stating what the entry does — what
registers, what is read and written, what is observable, what invariants hold,
which mechanisms are required and which are incidental. An implementer works from
that prose alone and never opens the source. The split is what makes a
specification safe to implement from: a document describing *behaviour* yields an
independent design, while one describing the original's structure yields a
transliteration wearing new names. Which route a specification took — source, or
declared and observed behaviour where no source exists — is recorded per entry,
and the private workspace's source-access policy governs what a reader may read.

**The specification step is the expensive half, and its cost is judgement rather
than reading.** M6-W3's five entries put a floor under the estimate: the smallest,
at 32 source files with no data or assets, took roughly forty minutes, of which
reading was under half. Effort tracks the number of distinct behaviours rather
than file count. The two costly parts are deciding whether a mechanism is required
or incidental — which cannot be answered from the source, because it turns on what
Ravel's own plan ports — and noticing behaviour that is load-bearing while looking
like a detail, which only a whole reading catches and whose loss produces a
counterpart that passes every test and is quietly wrong.

**A single reader's specification carries a defect rate nothing can measure.**
M6-W3 ran the one experiment available: two readers specified the wave's smallest
entry — 32 source files, no data, no assets — from the pinned source without
seeing each other's work. They disagreed three times. Twice the first was wrong by
omission, having read one of two forms a thing takes; once the second was wrong
about a registration path that has no opt-out. Three defects on the easiest entry,
surfaced only because two people read it independently, and each one would
otherwise have reached a counterpart that then passed its tests. Prose is
therefore paired where it is load-bearing or it is not trusted, and agreement
between a reader and a reviewer of that reader's document measures nothing —
the second reading has to start from the source.

That cost does not multiply across the inventory unchanged, and M7 does not
attempt it. Prose is not M7's default artefact: an entry is contracted by the test
kinds `plans.json` already lists for it, plus a behaviour-delta note for what a
Rust author would otherwise guess, and prose is reserved for entries whose
behaviour is executed and whose contract a test list cannot carry. A conformance
description is machine-checkable, which is what makes a single author's error
visible; prose is not, which is why prose pairs or does not scale. **A content
mod never gets a prose specification of its catalogue** — its systems are
specified and its catalogue is data, and restating a catalogue in prose is both
wasted and the transliteration failure this process exists to prevent.

A library specification currently covers every facility at equal
weight because the M5 graph records that one entry depends on another without
recording *what for*, so most of that volume specifies dead surface. Recording
consumed capability per dependency edge is mechanical, and it is the precondition
for scaling. Beyond the foundation wave, an entry whose behaviour is predominantly
*declared* — recipes, loot, tags, models, worldgen — is contracted by its data and
by the test kinds `plans.json` already lists per entry, which is cheaper than prose
and machine-checkable in a way prose is not; prose is reserved for entries whose
behaviour is *executed*, where an implementer would otherwise guess.

**No check can see a transliteration through prose.** The rewrite-integrity check
below reads names, and structure is not a name, so a specification that carries the
original's shape under entirely new words passes every rule and so does the Rust
written from it. The defence is the reader's discipline, and discipline is the
first thing a schedule erodes. M6-W5 therefore samples: a fresh reader who has seen
neither the specification nor the source judges whether a finished counterpart
reads as an independent design. That is a weak instrument and it is the strongest
one available, so its answers are recorded over time rather than taken one at a
time.

**Rewrite integrity is M6's obligation to enforce, not to assert.** M6 ships a
check of the same shape as M5-W7's public-safety screen: a published lexicon of the
names a Ravel counterpart is allowed to carry, plus an identifier set derived
from the M5 inventory holding the upstream package, class, member, and field
names that identify the original implementation. The check reads every
counterpart source file and every generated artefact in the counterpart's
package, and a file carrying an identifier from the upstream set — or a name
outside the lexicon that matches one under the repository's documented
normalization — fails it. Failing means a nonzero exit that fails the M6 gate,
not a warning. A transliteration carries the original's names; a rewrite does
not, so this is the property that distinguishes them mechanically.

The check is proved by mutation before M6-W6 may close: inject an upstream
identifier into a counterpart source file and into a generated artefact, and
record that each injection produces a nonzero exit. An M6 whose check exits zero
on both injections has not satisfied this obligation, and neither has one that
enforces the rule over source files only. M7-W2's per-counterpart suite and
M8-W3's Rust pack modules run the same check with the same failure meaning.

### M7 — Complete counterpart library

**Purpose:** execute the validated counterpart-authoring process until every
pinned manifest entry has an accepted reusable Rust counterpart.

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

“Counterpart present” means its package builds for its supported compilation targets, passes
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
| M8-W3 | Reimplement custom KubeJS behaviour as Rust modules | M8-W1 | compiled safe-Rust pack modules | scenario equivalence passes; runtime scan finds no JS engine |
| M8-W4 | Build full scenario suite | M8-W2..W3 | fresh-world and integration scenarios | intended content/progression inventory has complete scenario coverage |
| M8-W5 | Repeated save/reload soak | M8-W4 | world and migration evidence | repeated clean shutdown/reload cycles preserve state; failure cases fail closed |
| M8-W6 | Independent pack review and gate | M8-W1..W5 | private review/evidence | no Java/JavaScript mods execute; all pack/scenario suites and M8 gate pass |

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

If M9-W4 cannot pass with the complete workload, open ADG-9. Do not mark M9
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

Passing M10-W5 is **Linux initial completion**: the loader, SDK, counterpart
library, complete pack, Linux performance, and progression goal are all
accepted before macOS work begins.

The final boundary audit proves that the public tracked tree and Cargo metadata
contain no private path dependency, counterpart source, official fixture,
restricted asset, private world, rights-sensitive source note, credential,
launcher token, or unredacted sensitive log.

### Post-Linux-M10 — Apple Silicon platform expansion

No macOS build, launch, capture, packaging, signing, or verification work starts
before M10-W5 passes. Afterward, a separately reviewed plan ports the already
complete Linux loader and mod set to `macos-aarch64`, proves packaging and
signing, runs the native Apple Silicon process matrix, and evaluates the
constrained M1 Pro performance contract. Linux cross-evaluation never
substitutes for that later native evidence.

### M11 — Official ATM11 server interoperability

M11 begins only after initial completion and an accepted ADG-8 protocol
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
| Performance | performance profiles/collectors | M0 Linux launch proof and M9 single-pair Linux acceptance |
| Progression | private checklist/world | M10 journal plus regressions |
| Official server protocol | network plus private adapters | ADG-8 and M11 cross-client suite |

## Open questions and decision deadlines

| Question | Deadline | Blocking effect |
| --- | --- | --- |
| Exact fixture reference total and authoritative hashes | M0-W4 | discharged; M0 closed on `evidence/m0/manifest.json` |
| Canonical workload duration and metric definitions | M9-W1 | blocks M9 |
| Numeric RAM/CPU improvement target over the Java pack | M9-W4 | blocks M9 acceptance; `SPEC.md` §6.6 defers it because no Java baseline exists until the pair is run |
| Game Java runtime required by pinned metadata | M0-W6 | discharged; M0 closed |
| Architecture comparison metrics and failure actions | M0-W8 | discharged; M1 selected against it |
| JVM hook mechanism | ADG-1 / M1 | discharged; amended before M3-W3 on what enters the game process |
| Rust execution/package/ABI model | ADG-2 / M1 **and M3** | discharged at M3: source distribution, compiled on the player's machine. Reconciling `package.schema.json` and `SPEC.md` §3.4 is closed as a breaking schema-v1-in-place migration |
| Binding extraction and generation | ADG-3 / M1 | discharged, with one surface partly outside it: the agent jar's persistence adapter resolves game classes and members by name, and the pinned files cover some of the classes and none of the rest. `net.minecraft.client.Minecraft`, `net.minecraft.server.MinecraftServer` and `net.minecraft.world.level.storage.SavedDataStorage` are hook targets in `hooks.toml` and in `mappings.lock`, so a rename of one of those classes shows up in a re-pin diff. **An earlier wording of this row said none of them was pinned, which was wrong and understated the coverage; this is the correction.** What no pinned file holds is `LevelResource`, `DataFixers`, every member name and arity, the two `LevelResource` field constants, and the three-argument `SavedDataStorage` constructor — which is what `Persistence.java`'s comment at the lookup has said all along. `verify_persistence_reflection` pins the class set and cannot see a rename. Bringing the rest of the surface under `stubs.toml` and `mappings.lock` needs the stub-seed and lock re-pin ceremony against the pinned jar. **No work package owns that yet.** M3's packages are closed and none named it; M4-W1 to M4-W8 do not either. It belongs with the version-adapter re-pin work. An earlier reading of this row ruled that **M4-W6 is the first gate that cannot honestly pass without it**, because proving version isolation against a second version is exactly what an unpinned reflective surface defeats. **That ruling is retracted here, and the reason is that it was too broad.** The reflective surface it names lives in the agent jar, which is neither a general crate nor the shared adapter, so what it leaves in doubt is a drift claim over the agent jar — M4-W8's territory — rather than W6's claim that the shared adapter and the general crates carry no version. W6 proves that claim against a synthetic second version's data in `tests/fixtures/public/versions/`, and that fixture cannot reach the reflective surface at all: it declares no jar, so there is no class for a name to resolve against and nothing for a rename to break. No version of W6 could have discharged this row, which is what makes refusing W6 for not discharging it the wrong ruling rather than an unmet one. The re-pin ceremony is what closes this row, and no package has performed one. Until a package owns it and runs it, no milestone may claim drift coverage for the agent jar's persistence adapter |
| Java/Rust state ownership | ADG-4 / M1 | discharged; amended at M3-W0 to admit opaque handles |
| Stock Prism versus minimal patch | ADG-5 / M1 | discharged; the library-path argument does not work and ADG-5's amendment chose the fix — `System.load` on a path given by a `-D` property. No work package owns building it, so the native half has no conformance claim until one does |
| Configuration and UI schema | ADG-6 before M3-W3 | discharged; declared in data, drawn in Java |
| Where the generated screen artifact loads | M3-W3 | discharged; ADG-1 records M3-W3 selecting `Instrumentation.appendToSystemClassLoaderSearch` for the sibling screen jar |
| Whether anything reads the pack `ravel-pack` writes | M8 | M4-W5's independent review recorded it and this row is where it lives. Nothing in this repository reads a written pack: no code outside `ravel-pack` and its own suite reads `RESERVED_PREFIX`, and `ravel-loader`'s `open_configuration` takes its pack documents from a caller-supplied map that no production caller fills. So the pack's output is validated only against itself and **the write→read round trip is unprovable by construction** — the same construction that hid blocking defects from M3's independent review round after round, each one found only after the construction that hid it was described. How many rounds that took is a figure `evidence/m3/review.md` keeps and no other document restates, `verify_review_round_counts` now sweeps this file for one, and the count that used to sit in this sentence was wrong. M4 names no work package for reading a pack and M4-W6 did not invent one. M8 is the first milestone whose verification cannot pass without a game reading a written pack, so it owns the obligation; the public-side half is a host that fills that map and belongs with the shipped-host wiring that ADG-5's row already leaves unowned. Until something reads a pack back, no milestone may claim the pack writes correct bytes |
| Whether a pack's settings layer is checked when the pack is built | M8 | M4-W5's independent review found the asymmetry and it is stated at the settings write in `ravel-pack`'s `validate`. A typo'd tag member is refused to the pack author; a typo'd settings key is refused to the player, because `LayeredConfiguration::resolve` holds the check and runs at launch. Closing it means handing `validate` the members' configuration declarations and calling that same `resolve`. No work package owns it; M8 pins a real pack's configuration and cannot ship an unchecked layer |
| Whether a mod may provide a shared library other mods build on | M7 | M5-W7 proposed six general SDK capabilities and M6-W2 shipped five of them. The sixth, `RVL-GAP-06`, is a mod declaring that it *provides* a library at a version, one provider resolved per library for a launch, and a refusal when two providers disagree or a required library has none. It is deferred rather than done, and the reason is that **none of M6's frozen foundation wave motivates it**: every entry in that wave is answered by the five that shipped, so building it in M6 would be a capability nothing installed would use and no test could exercise honestly. It is not the same question as [`services`](crates/ravel-sdk/src/services.rs), which orders named calls between two mods that are both installed; this one asks which mod *supplies* a shared thing and at what version. M7 is the first milestone whose set is large enough for two counterparts to want one library, so it owns the row, and M7 has now built it: [`ravel_sdk::provision`](crates/ravel-sdk/src/provision.rs) resolves one provider per library and refuses a contested library, a version outside a declared range, and a requirement nothing provides, with `test-mods/provision` and the last section of `tests/conformance/tests/sdk_gaps.rs` exercising it. What remains open is the half no declaration can settle alone: no launch builds a `Provision` from the packages it composed, which `docs/guide/limits.md` states where an author reads it |
| Per-counterpart rights and implementation method | M5 | blocks publication; may block lawful implementation |
| Official server wire surface | ADG-8 after M10 | blocks M11 only |
| Official host viability | ADG-9 only after M9 failure | leaves M9 incomplete |
| Registry identity/signing/moderation | separate M13 plan | no effect on initial completion |

## Immediate handoff

The plan is ready for M0 implementation. The first local change sequence should
be:

1. M0-W1: initialize the Cargo workspace and Nix/devenv environment without
   choosing ADG-1 through ADG-4.
2. M0-W2: implement the evidence schema, `ravel-xtask` gate skeleton, and CI
   parity.
3. M0-W3 through M0-W5: establish the ignored fixture contract and later
   performance protocol shape before admitting launch evidence.
4. Review that foundation, then admit the already-proven Linux vanilla and
   official ATM11 launch evidence. Performance capture waits for M9, and macOS
   work waits until Linux M10 is complete.

Do not begin a loader implementation until M1's selected architecture ADRs pass
their review.
