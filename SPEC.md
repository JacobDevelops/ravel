# Ravel Specification

Status: Draft product and capability contract  
Initial target: Minecraft: Java Edition 26.1.2  
Proof workload: an ATM11-equivalent Rust modpack pinned to ATM11 0.2.1  
Last updated: 2026-07-30

## 1. Capability

Ravel is a general-purpose Minecraft: Java Edition mod loader and SDK for
building safe, precompiled mods in Rust.

Ravel initially hosts Rust mods inside the official Minecraft runtime. It does
not load NeoForge, Fabric, or other Java mods. The first large-scale proof is a
separate Rust modpack containing a Rust counterpart for every mod referenced by
the pinned ATM11 manifest.

The immediate user outcome is that a 16 GB M1 Pro Mac can play the complete
pack in single-player and can eventually join players using the official ATM11
pack, without the resident-memory cost of hundreds of Java mods. Building a
reusable Rust modding ecosystem is a required means to that outcome and a
valuable secondary result.

## 2. Product principles

1. **Measure before replacing.** Baseline official ATM11, then attribute memory
   and frame-time costs before choosing component boundaries.
2. **Ravel is a new loader.** NeoForge is a requirements reference, not an
   implementation, API, runtime dependency, or compatibility host.
3. **Rust mods are first-class packages.** Users install precompiled Rust mods;
   mod authors do not also write Java wrappers.
4. **Keep official Minecraft while it works.** Replacing the client is not part
   of the committed scope. Failure to meet the performance target triggers a
   new architecture decision, not an automatic native-client project.
5. **Build the loader before the pack.** Original test mods prove loader
   semantics before any ATM11 counterpart is admitted.
6. **Prefer idiomatic Rust.** NeoForge's lifecycle, registries, networking, and
   sided-execution concerns inform Ravel, but do not dictate its authoring
   model.
7. **Compatibility is evidence-backed.** Automated behavioural, save/reload,
   registry, and protocol tests replace claims based on visual inspection.
8. **Public and private work stay separable.** Loader infrastructure is
   public-ready. Rights-sensitive compatibility work stays in a private
   workspace until cleared.
9. **No calendar-driven shortcuts.** Milestones close only when their exit
   criteria pass.

## 3. Fixed product decisions

### 3.1 Loader identity

- The project and ecosystem name is **Ravel**.
- The ecosystem may include Ravel Loader, Ravel SDK, Ravel Pack tooling, and a
  later Ravel Registry.
- `atm11-rust` is not a public product name.
- Public names must not imply endorsement by Mojang, Microsoft, NeoForged, the
  ATM team, or original mod authors.

### 3.2 Runtime model

- Initial Ravel versions run with official Minecraft: Java Edition.
- Ravel loads Rust mods only.
- Mixed execution with NeoForge, Fabric, Quilt, Forge, or other Java mod
  loaders is out of scope.
- Ravel cannot use another mod loader as its bootstrap, including in the first
  prototype.
- A minimal generated Java/JVM bootstrap is permissible only when its resource
  cost is measured and accepted.
- A Java bootstrap is loader plumbing, not a Java mod API or a NeoForge bridge.
- Ravel must not execute existing NeoForge mod JARs.
- Installed mods are trusted, matching the effective NeoForge trust model.
- The mod set is fixed from process startup until exit. Runtime hot-loading and
  unloading are not required.

### 3.3 Rust safety boundary

- Code maintained in Ravel repositories and code written by mod authors uses
  `#![forbid(unsafe_code)]`.
- Audited dependencies may encapsulate the unavoidable unsafe operations
  required for JNI, JVMTI, native loading, operating-system integration, or
  equivalent boundaries.
- Ravel must not quietly weaken this rule. If safe wrappers make the loader
  impossible or prevent it from meeting measured targets, the project pauses
  for an explicit decision.
- The public mod API must be safe Rust.

### 3.4 Mod packaging

- Users install precompiled mod packages; users do not compile Rust locally.
- A package may contain target-specific binaries, resources, metadata, and
  schemas.
- The exact execution and packaging model—native dynamic libraries, process
  isolation, WebAssembly components, or another mechanism—is an architecture
  decision, not yet fixed.
- The package model must support at least:
  - stable mod identity and version;
  - Minecraft and Ravel compatibility ranges;
  - client, integrated-server, and dedicated-server side declarations;
  - required, optional, incompatible, and discouraged dependencies;
  - deterministic load ordering where ordering is semantically necessary;
  - declared resources, configuration schemas, and saved-data versions;
  - target platform and architecture metadata;
  - author, source, licence, and provenance metadata;
  - integrity verification.

### 3.5 Loader responsibilities

Ravel owns:

- mod discovery from an installed instance;
- metadata parsing and validation;
- dependency resolution and compatibility diagnostics;
- lifecycle orchestration;
- Minecraft-version adapters;
- safe Rust APIs for Minecraft-facing capabilities;
- registries and cross-mod composition;
- sided execution;
- configuration APIs and an in-game mod/configuration UI;
- resource and data integration;
- saved-state APIs and migrations for Ravel worlds;
- structured crash and failure attribution;
- opt-in per-mod memory and CPU telemetry;
- pack validation;
- development and compatibility-test tooling.

Ravel does not own:

- Microsoft account authentication;
- Minecraft purchase or entitlement;
- general launcher functionality;
- mod discovery marketplaces;
- downloading and updating installed mods;
- Java mod execution;
- an initial public package registry.

Prism Launcher is the preferred owner for authentication, Minecraft
installation, instance management, and pack acquisition. If stock Prism cannot
launch Ravel cleanly, the project first evaluates a small upstream integration
or maintained patch. A standalone launcher is considered only if that change
would be unreasonably large.

### 3.6 Modpack authoring

Ravel supports two pack-authoring layers:

1. Schema-validated declarative data for ordinary recipes, tags, loot,
   progression, quests, configuration, and integration.
2. Compiled safe Rust modules for custom pack behaviour that cannot be expressed
   declaratively.

There is no embedded JavaScript runtime or general scripting-language
requirement. ATM11's KubeJS behaviour is manually reimplemented as declarative
data or Rust modules.

Vanilla datapacks and resource packs must continue to work unchanged.

### 3.7 Version support

- The first implementation supports Minecraft 26.1.2 only.
- Minecraft-specific hooks, mappings, and bindings must live behind explicit
  version adapters.
- Core loader, package, and SDK code must not hardcode 26.1.2 assumptions where
  a version boundary can reasonably isolate them.
- Supporting a second Minecraft version is not in the initial delivery scope,
  but the design must make adding one an adapter task rather than a rewrite.
- The public mod API remains experimental and may break until the complete
  ATM11-equivalent proof workload passes.

## 4. Users and surfaces

### 4.1 Player

The player:

- installs and launches a Ravel instance through Prism;
- views installed mods and dependency problems in game;
- edits supported configuration through the in-game UI;
- opens and saves Ravel single-player worlds;
- can join a vanilla server when only protocol-safe client mods are installed;
- eventually joins an official ATM11 server through a later interoperability
  layer.

### 4.2 Mod author

The mod author:

- uses the Ravel SDK and safe Rust APIs;
- builds platform-specific packages through supported tooling;
- declares capabilities, dependencies, sides, resources, configuration, and
  saved data;
- tests against deterministic loader and Minecraft harnesses;
- does not need to write Java.

### 4.3 Modpack author

The pack author:

- selects compatible Ravel mods;
- pins exact versions;
- configures common behaviour declaratively;
- writes Rust only for genuinely custom behaviour;
- validates dependency, resource, registry, and progression consistency;
- produces a reproducible manifest without bundling unauthorised third-party
  content.

### 4.4 Maintainer

The initial maintainer is one human developer using an agent-driven workflow.
The repository is not optimized for outside contributors until a credible
prototype exists. Diagnostics, specifications, provenance, and test fixtures
must nevertheless be machine-readable and reproducible from the beginning.

## 5. Initial platform contract

### 5.1 Reference platform

- Linux Mint 22.3, x86_64
- Linux kernel 6.8
- AMD Ryzen 7 9800X3D, 8 cores / 16 threads
- 48 GiB RAM
- NVIDIA GeForce RTX 5090
- Vulkan 1.4

This is the authoritative compatibility and high-performance reference machine.
Constrained-memory and throttled profiles must supplement it; it is not a
substitute for target-device testing.

### 5.2 Constrained target

- Apple Silicon MacBook Pro
- Apple M1 Pro
- 16 GB unified memory
- macOS Sequoia 15.4.1 at initial discovery
- 16-inch 3456 × 2234 built-in display
- approximately 52 GB free storage at initial discovery

Initial macOS scope is this Apple Silicon class only. Windows, Linux ARM, and
Intel macOS are not initial requirements.

### 5.3 Distribution targets

Precompiled mods are built for two initial targets, `linux-x86_64` and
`macos-aarch64`. These are the values the package manifest's `platform` and
`architecture` fields carry, and no other combination is an initial requirement.
A mod author's build matrix is therefore two entries, not the full product of
supported operating systems and architectures.

The macOS target carries a signing obligation that the Linux target does not.
Because Apple Silicon refuses to load unsigned executable code, every macOS
artifact must be signed before it can run, and notarization becomes relevant
separately where downloaded files are quarantined. Requiring mod authors to sign
their own builds would put an Apple Developer enrolment between an author and
their first release, which conflicts with the reason for shipping precompiled
mods at all.

The recorded candidate is therefore for Ravel to ad-hoc sign macOS artifacts on
the installing machine rather than for authors to sign at build time. Signing
without a full Xcode toolchain present appears feasible, which would keep the
user-side toolchain requirement at zero. This is a candidate to prove or
discard at the open question 2 gate, not a settled design.

## 6. Performance contract

### 6.1 Target workload

The constrained Mac must run the complete Ravel ATM11-equivalent pack:

- in single-player with its integrated server;
- at 3456 × 2234 output resolution;
- at an 8-chunk render distance;
- without shaders;
- at a sustained minimum target of 30 FPS under a defined representative
  workload.

Multiplayer client-only measurements are recorded separately after official
server interoperability exists. Running the official ATM11 server on a stronger
machine is the desired multiplayer deployment, but does not replace the
single-player requirement.

### 6.2 Memory budget

Initial single-player acceptance targets:

- no more than 8 GiB steady-state process RSS during representative play;
- no more than 10 GiB transient process RSS during loading or dimension
  changes;
- no sustained macOS memory-pressure warning;
- no sustained swap thrashing.

After M11 establishes official-server interoperability, the same measurements
are reported separately for the multiplayer client-only workload.

The full system report must also record unified-memory pressure and relevant GPU
allocation data where macOS exposes them. Process RSS alone is not a complete
unified-memory measurement.

### 6.3 Frame-time definition

The performance harness must define:

- test world and player route;
- warm-up period;
- measurement duration;
- simulation distance;
- graphics settings;
- active machines, entities, chunks, and dimensions;
- average FPS;
- median, p95, and p99 frame time;
- tick time for the integrated server;
- peak and steady-state memory.

A nominal FPS counter is insufficient if frame pacing makes the game
unplayable.

### 6.4 Startup and world loading

Startup and world-load time have no fixed numerical target. Ravel must report
the best achieved result and compare it with the pinned official baseline.

### 6.5 Architecture failure gate

If a complete Rust mod set hosted by official Minecraft cannot meet both the
memory and frame-time contracts, the project pauses to re-evaluate:

- the measurement and workload;
- remaining JVM and Minecraft costs;
- duplicated Java/Rust state;
- hook and boundary overhead;
- acceptable product compromises;
- a native Rust client.

The current spec does not pre-authorize relaxing targets or building a native
client.

## 7. ATM11-equivalent proof workload

### 7.1 Pinned reference

The initial compatibility oracle is:

- All the Mods 11 version 0.2.1;
- Minecraft 26.1.2;
- NeoForge 26.1.2.78;
- 235 CurseForge project/file references in the client manifest.

This target is pinned. Upstream ATM11 updates do not automatically change the
scope.

### 7.2 Fixture handling

- The official pack is a local, ignored compatibility fixture.
- A repository tool reconstructs the exact pinned fixture from authorized
  sources and verifies hashes.
- Official mod JARs, ATM-owned scripts, quests, menu assets, and other
  restricted files must not be committed to a public-ready repository.
- Credentials and CurseForge API keys must never enter source control.
- The fixture is used as a black-box compatibility oracle and research input.
- The fixture tool must respect current platform API and distribution terms.

### 7.3 Counterpart completeness

- Every mod reference in the pinned manifest receives an individually reusable
  Rust counterpart.
- Counterparts are packages, not pack-internal anonymous rewrites.
- Every counterpart records the original project, author, version, source,
  licence, implementation method, permissions, and attribution.
- Reusable counterparts use canonical Ravel-specific IDs.
- A later interoperability layer may map or advertise original NeoForge mod IDs
  and versions without making them canonical Ravel identities.
- The complete pack includes the pinned configuration and KubeJS behaviour
  reimplemented through Ravel's declarative and Rust authoring layers.

### 7.4 Behavioural target

The Ravel pack must reproduce the complete intended content, integrations, and
progression of the pinned ATM11 release closely enough to complete its endgame
path.

It may intentionally fix upstream bugs when:

- normal Ravel play and saves remain stable;
- the difference is documented;
- later server interoperability is not broken.

Pixel-level reproduction is preferred, but copying restricted assets is not
permitted. Visual parity remains subject to per-mod asset rights.

### 7.5 Save compatibility

- Importing worlds created by official Java/NeoForge ATM11 is explicitly out of
  initial scope.
- Ravel owns its world and mod-data compatibility contract.
- Ravel worlds must survive supported Ravel upgrades through versioned schemas
  and migrations.
- Pre-release development worlds may be declared disposable until M10. Saved
  data is still versioned from the first prototype so migration mechanics can
  be tested before the compatibility promise becomes stable.
- Worlds must fail closed when required mods or incompatible data versions are
  missing.
- Ravel must not offer a recovery mode that opens and saves a required-mod world
  with suspected mods and dependants disabled.

### 7.6 Completion evidence

Pack completion requires:

- every manifest counterpart present;
- every counterpart's automated suite passing;
- dependency and registry closure;
- all pack-level configuration and scripted behaviour ported;
- clean fresh-world creation and repeated save/reload;
- performance contract passing on the constrained target;
- a manual end-to-end playthrough of the full intended quest/progression path.

The final progression playthrough is a manual acceptance test, supported by
automated tests for its underlying mechanics.

## 8. Compatibility roadmap

### 8.1 Single-player first

Single-player is the first pack-scale compatibility target. Ravel must run both
the official client and integrated-server sides required by Rust mods.

### 8.2 Vanilla multiplayer

A Ravel client with only protocol-safe client mods must be able to join ordinary
vanilla servers.

### 8.3 Official ATM11 multiplayer

Official ATM11 server interoperability is a later milestone. It requires
behavioural emulation of:

- the NeoForge client/server handshake;
- mod identity and version negotiation;
- registry synchronization;
- custom payload channels;
- each counterpart mod's packet formats and state transitions.

This is protocol compatibility only. Ravel must not load NeoForge or execute
Java mods.

### 8.4 Dedicated Ravel server

Running Rust mods on a dedicated official-Minecraft server is a stretch
capability. A wholly Rust server may be evaluated later. Pumpkin and similar
projects are research and reuse candidates, not an assumed base.

## 9. Diagnostics and observability

### 9.1 Failure reporting

Ravel must attribute failures to:

- mod identity and version;
- dependency chain;
- lifecycle phase;
- hook, event, registry, or packet;
- Minecraft and Ravel versions;
- platform and target architecture.

Reports must redact authentication tokens, secrets, and unnecessarily identifying
filesystem paths.

### 9.2 Dependency failures

Before world access, the UI must clearly report:

- missing required dependencies;
- incompatible versions;
- dependency cycles;
- side mismatches;
- duplicate identities;
- unsupported targets;
- conflicting registry claims.

Ravel fails closed on an invalid required-mod graph.

### 9.3 Profiling

Opt-in profiling mode exposes per-mod:

- retained and allocated memory where technically measurable;
- CPU time;
- hook and event counts;
- slow callbacks;
- cross-boundary calls and transferred bytes;
- registry and saved-data footprint.

Normal play must incur near-zero profiling overhead. The architecture spike must
prove what can be attributed accurately rather than presenting estimates as
exact measurements.

## 10. Security and trust

- Installed Rust mods are fully trusted and may perform arbitrary actions
  available through their execution model.
- Sandboxing filesystem, network, or native access is not an initial
  requirement.
- Package integrity verification is required.
- Publisher signing and publisher identity are distribution-trust decisions for
  the post-initial registry.
- Platform code signing is a separate execution requirement and is not deferred.
  Apple Silicon requires a signature on all executable code, so an unsigned
  precompiled artifact does not load on the constrained target at all. This
  constrains the packaging and execution model and is an input to open
  question 2 rather than to the registry.
- Microsoft authentication remains outside Ravel.
- Logs and diagnostics must not collect or expose launcher tokens.
- Fixture reconstruction must not bypass Minecraft ownership, authorized
  downloads, or third-party distribution controls.

## 11. Rights, provenance, and publication

This section is project policy, not legal advice.

### 11.1 Source-use policy

- Compatible open-source mod: a source-informed port is allowed when all licence
  terms, notices, and attribution requirements are followed.
- Closed, All Rights Reserved, or unclear mod: implementation proceeds from a
  behavioural specification and independent code. Line-by-line or machine
  translation is prohibited.
- For closed, All Rights Reserved, or unclear mods, implementers may use public
  documentation, public metadata, published APIs, and observable black-box
  behaviour. Decompilation, private source, and source-derived notes are
  prohibited unless permission or a compatible licence explicitly allows them.
- Where practical, separate research and implementation agents are used: the
  researcher produces a behaviour-only specification, and the implementer sees
  that specification rather than the original implementation.
- Each counterpart's provenance record identifies every source of behavioural,
  code, and asset evidence, who or what produced its specification, and whether
  permission was obtained. An unresolved provenance record blocks publication.
- Rewriting in another language does not by itself avoid derivative-work or
  asset rights.
- Code, textures, models, sounds, names, branding, quests, scripts, and other
  content are audited separately.

### 11.2 Publication gate

Before publishing a counterpart:

- audit code and asset licences;
- preserve required notices;
- seek author permission where required;
- resolve names and branding;
- verify that distributed assets are authorized;
- record the decision in the provenance ledger;
- credit the original author regardless of whether attribution is legally
  required.

The initial ATM11 compatibility workspace remains private. Publication of its
counterparts, modpack, or registry is post-initial scope.

### 11.3 Project licence

The provisional licence for original Ravel loader and SDK code is dual
MIT/Apache-2.0, subject to dependency and interoperability review before public
release.

## 12. Repository topology

### 12.1 Public-ready loader repository

The `ravel` repository contains:

- loader core;
- Minecraft version adapters;
- safe SDK crates;
- package and manifest definitions;
- pack authoring and validation tools;
- original test mods and fixtures;
- compatibility harnesses that contain no restricted content;
- specifications and architecture decisions.

### 12.2 Private compatibility workspace

A separate private workspace contains:

- the ATM11-equivalent Rust counterparts;
- private pack manifests and integrations;
- ignored official ATM11 fixtures;
- rights-sensitive behavioural research;
- permission and publication status;
- private end-to-end worlds and reports.

The boundary exists from the beginning so that making Ravel public never
requires purging restricted history.

### 12.3 Future registry

A Ravel package registry is post-initial scope. It is considered when individual
counterparts and the loader are ready for publication. Until then, package
acquisition remains file/manifest based and external to the loader.

Open question 9 has a recorded preferred shape. A registry should be an index
over author-owned release records rather than the authoritative store of the
artifacts themselves. Publisher identity should be portable and bound to
something the author already controls, so that losing a registry account does
not invalidate published releases. Published releases must be immutable and
addressed by the content hash the package manifest already requires. Moderation
should act at the index rather than at the distribution layer.

Immutability is the binding constraint. A mechanism that permits an author to
withdraw or rewrite a published release breaks every dependent resolution and
disqualifies itself regardless of its other properties.

Three candidate mechanisms are recorded for M13 evaluation. A Go module style
design distributes from author-controlled URLs and derives immutability and
integrity from an append-only transparency log. The Update Framework and
Sigstore address publisher identity and signing directly, including key
compromise, rollback, and freeze attacks. An AT Protocol style design supplies
portable domain-bound identity and signed records, but its records are mutable
by their author, its blob storage targets small media rather than mod artifacts,
and its aggregation model is sized for social write volume; adopting it would
require reintroducing immutability at the index and so weakens the
author-owned-store property that motivates it.

Recording these candidates does not select one, does not add a signature field
to the package manifest, and does not change M0 through M10. Selection is M13
work behind its own decision gate.

## 13. Testing contract

### 13.1 Loader tests

- unit tests for metadata, dependency resolution, version selection, and
  lifecycle state machines;
- compile tests for the public SDK;
- integration tests with multiple original Rust test mods;
- negative tests for invalid packages and dependency graphs;
- Minecraft-process tests for each supported hook and registration surface;
- save/reload and migration tests;
- crash-attribution and diagnostic-redaction tests;
- platform tests on Linux x86_64 and Apple Silicon macOS;
- performance regression tests for loader overhead and Java/Rust boundaries.

### 13.2 Counterpart tests

Every counterpart requires:

- unit tests for its core mechanics;
- registration and resource validation;
- save/reload tests for persistent state;
- dependency and cross-mod integration tests;
- differential tests against the pinned official behaviour where lawful and
  technically possible;
- client/integrated-server sided tests;
- memory and tick/frame-time characterization;
- provenance and licence metadata validation.

### 13.3 Canonical world and scenarios

The private workspace maintains a canonical fresh compatibility world and a
scenario suite that deliberately exercises every counterpart and integration
cluster. Scripted scenarios run against both the pinned official fixture and
Ravel where equivalent automation is possible.

The suite compares observable behaviour, registries, recipes, resources,
packets, and saved state. It must distinguish exact comparisons from
human-reviewed or tolerance-based comparisons.

### 13.4 Quality gates

- No disabled lint rules, skipped tests, loosened assertions, or ignored errors
  to obtain a passing build.
- Each milestone defines its own coverage and behavioural obligations.
- Loader and SDK changes require Linux and macOS validation once the Mac harness
  exists.
- Nontrivial changes require an independent code review before completion.

## 14. Lifecycle and state model

The exact API is experimental, but implementations must cover these states:

1. **Discovered** — package found, no code executed.
2. **Validated** — metadata, target, integrity, and schema accepted.
3. **Resolved** — dependency graph and load order accepted.
4. **Loaded** — code/resource package made available.
5. **Constructed** — mod instance and declared services created.
6. **Registered** — static registries and resources committed.
7. **Running** — runtime hooks and events active.
8. **Stopping** — saves flushed and callbacks quiesced.
9. **Stopped** — process shutdown complete.
10. **Failed** — terminal failure with structured attribution.

Required invariants:

- no mod code runs before validation and dependency resolution;
- registry identifiers become immutable after the registration barrier;
- required dependency failure prevents world access;
- client-only code never loads on server sides;
- persistent data is versioned and owned by a stable canonical mod ID;
- failure after world mutation cannot silently continue or save partial state;
- lifecycle order is deterministic for an identical package set.

## 15. Architecture decision gates

These decisions require prototypes and evidence before being fixed.

### ADG-1: JVM entry and hook mechanism

Compare:

- Java instrumentation agent;
- JVMTI native agent through safe wrappers;
- generated Java hook classes;
- bytecode transformation libraries;
- viable combinations.

Measure bootstrap RSS, retained classes, startup time, callback cost, transferred
bytes, allocation behaviour, and cross-platform packaging.

### ADG-2: Rust mod execution and ABI

Compare:

- native dynamic libraries behind a stable generated boundary;
- WebAssembly component execution;
- isolated helper processes;
- static composition for pack builds;
- other viable mechanisms.

Evaluate memory, call overhead, safe-Rust enforcement, ABI evolution, crash
behaviour, platform packaging, debugging, and mod-to-mod calls. Trusted execution
does not remove the need for a stable package contract.

### ADG-3: Minecraft bindings

Determine how versioned Minecraft types and calls are:

- extracted;
- generated;
- represented safely;
- cached without duplicating excessive state;
- validated against game updates.

### ADG-4: State ownership

Define which state remains in Minecraft objects and which state belongs in Rust.
Avoid mirrored object graphs unless measurement proves the cost acceptable.

### ADG-5: Prism integration

Prove stock Prism launch and authentication handoff. If insufficient, estimate
an upstream patch, a maintained patch, and a minimal standalone launcher before
expanding scope.

### ADG-6: Configuration and UI

Choose schemas and UI generation that support both files and in-game editing,
side-aware values, restart requirements, validation, and pack overrides.

### ADG-7: Official-server interoperability

After single-player completion, specify the NeoForge and per-mod wire
compatibility surface from captures and primary implementations. This gate does
not alter Ravel's internal API.

### ADG-8: Official Minecraft host

After pack completion and constrained-device profiling, decide whether official
Minecraft remains the final host. A native Rust client requires a new approved
capability specification.

## 16. Milestone plan

Milestones are dependency ordered. They have no promised calendar duration.

### M0 — Reproducible baselines

Deliver:

- Prism-based vanilla 26.1.2 instance;
- ignored, pinned official ATM11 0.2.1 fixture on Linux;
- fixture reconstruction and hash verification tool;
- documented Linux reference hardware;
- reproducible performance capture;
- equivalent vanilla and official ATM11 captures on the constrained Mac.

Exit:

- vanilla and official ATM11 launches are reproducible;
- baseline RSS, system memory pressure, frame times, tick times, startup, and
  world loading are recorded on both target platforms;
- fixture files and secrets are absent from source control.

### M1 — Architecture spikes

Deliver isolated experiments for ADG-1 through ADG-4 and ADG-5.

Exit:

- the selected bootstrap, execution model, binding approach, and state boundary
  have benchmark evidence on Linux and a viable macOS path;
- maintained code still forbids unsafe;
- rejected approaches and reasons are recorded;
- a reviewer confirms that the choice can plausibly meet the product targets.

### M2 — Loader skeleton

Deliver:

- Prism launch path;
- Ravel bootstrap;
- package discovery and validation;
- dependency graph;
- lifecycle state machine;
- structured logs and failure attribution;
- one no-op precompiled Rust test mod.

Exit:

- official Minecraft 26.1.2 launches without another mod loader;
- the Rust mod runs on Linux;
- invalid packages fail before world access;
- loader overhead is measured.

### M3 — Credible loader prototype

Deliver multiple original test mods demonstrating:

- dependencies;
- a block;
- an item;
- a recipe;
- a command;
- an event handler;
- a generated configuration screen;
- persistent saved state;
- resource/data integration;
- in-game mod list;
- opt-in profiling.

Exit:

- the prototype runs on Linux x86_64 and the target Apple Silicon Mac;
- a world saves, exits, reloads, and preserves state;
- all required test surfaces pass;
- profiling attributes mod costs without unacceptable normal-mode overhead;
- no Fabric, NeoForge, or other loader is present.

### M4 — General SDK breadth

Deliver general API breadth proven through original test mods:

- registries and tags;
- resources and generated data;
- events and hooks;
- networking abstractions even though official-server compatibility is deferred;
- client and integrated-server sides;
- configuration;
- saved-data schemas and migrations;
- inter-mod services;
- pack declarations and validation.

Exit:

- representative original mods exercise each capability;
- SDK documentation and examples are sufficient for an agent to implement a new
  test mod without internal loader access;
- version-specific Minecraft logic remains isolated.

### M5 — Private compatibility inventory

In the private workspace:

- reconstruct the exact pinned ATM11 fixture;
- inventory all 235 manifest entries;
- build the dependency and integration graph;
- catalogue configs, resources, KubeJS behaviour, packets, saves, licences, and
  sides;
- define canonical Ravel IDs and attribution records;
- rank ports by foundation value, compatibility risk, memory impact, and
  vertical-slice usefulness;
- perform a compatibility-driven SDK gap analysis without implementing a
  counterpart.

Exit:

- every manifest entry has an owner record and port plan;
- shared libraries and integration clusters are explicit;
- required SDK additions are expressed as general capability proposals with
  original test-mod acceptance cases;
- no restricted fixture is tracked.

### M6 — Foundation counterpart set

Port shared libraries, APIs, and the smallest end-to-end gameplay clusters.
Extend Ravel only through general capabilities, not one-off hidden ATM hooks.

Exit:

- each admitted counterpart passes its suite;
- at least one nontrivial cluster can be played, saved, and reloaded;
- new SDK surfaces have independent test-mod coverage;
- provenance is complete.

### M7 — Complete counterpart library

Implement an individually reusable Rust counterpart for every pinned manifest
entry.

Exit:

- 235/235 manifest entries have resolved counterparts or a documented manifest
  correction if the source manifest count changes during exact fixture
  verification;
- all per-mod and cluster suites pass;
- dependency and registry closure succeeds;
- every package has attribution and rights status.

### M8 — Complete Ravel pack

Port pinned configurations and all executable KubeJS behaviour to declarative
data or Rust modules. Dead or unreachable fixture content may be excluded only
when the inventory records evidence for that classification. Assemble the
separate Ravel pack.

Exit:

- a fresh world exposes the complete intended content and progression;
- all pack validation and scenario suites pass;
- repeated save/reload is stable;
- no JavaScript or Java mods execute.

### M9 — Constrained-device performance

Profile and optimize the complete pack in single-player on the M1 Pro Mac.

Exit:

- 3456 × 2234, no shaders, 8 chunks;
- sustained 30 FPS target under the canonical workload;
- no more than 8 GiB steady RSS and 10 GiB transient RSS;
- no sustained memory-pressure warning or swap thrashing;
- results are reproducible and include frame-time distributions.

Failure invokes ADG-8 rather than silently changing requirements.

### M10 — Progression acceptance

Perform the manual fresh-world playthrough of the complete intended pinned
ATM11 progression and endgame.

Exit:

- endgame completion is recorded;
- encountered incompatibilities become regression tests;
- release-blocking functional gaps are closed;
- Ravel save compatibility is verified across the tested build sequence.

### M11 — Official ATM11 server interoperability

Implement handshake, identity aliases, registry sync, custom channels, and
per-mod packets needed to join an unmodified pinned ATM11 server.

Exit:

- the Ravel client joins and plays on the pinned official server;
- cross-client interactions for every networked counterpart pass;
- multiplayer client-only RSS, memory pressure, frame time, and tick-independent
  performance are reported separately from M9's single-player results;
- protocol captures contain no authentication secrets;
- Ravel's canonical IDs and internal APIs remain independent of NeoForge.

### M12 — Dedicated-server stretch

Evaluate dedicated-server capability. Implementation requires a new capability
amendment with explicit exit criteria. A wholly Rust server is not implied.

### M13 — Publication and registry

Post-initial scope:

- verify public-name collisions and finalize repository/package presentation;
- complete loader/SDK dependency and licence audit;
- publish cleared counterparts individually;
- obtain permissions and replace assets where required;
- design registry identity, signing, moderation, and distribution;
- rename or create remotes without exposing private history.

## 17. Explicit non-goals

The initial scope does not include:

- a native Rust Minecraft client;
- a custom launcher or Microsoft authentication;
- loading or translating Java mod binaries at runtime;
- Fabric, NeoForge, Forge, or Quilt coexistence;
- runtime hot-loading/unloading;
- importing official ATM11 worlds;
- shaders as part of the performance target;
- Windows, Intel macOS, or Linux ARM support;
- a public mod registry;
- public distribution of uncleared counterparts or ATM assets;
- automatic KubeJS translation or JavaScript execution;
- official ATM11 multiplayer before single-player completion;
- a wholly Rust dedicated server;
- safe-mode world loading with required mods disabled;
- pixel-identical visuals when asset rights do not permit them.

## 18. Open questions

These are intentionally unresolved:

1. Which JVM hook mechanism meets safety and resource constraints?
2. Which precompiled mod execution/package model best balances memory, ABI
   stability, and platform support?
3. How much Minecraft state can remain authoritative in Java without expensive
   boundary traffic?
4. Can stock Prism launch Ravel without modification?
5. What are the actual official ATM11 baselines on Linux and the target Mac?
6. Which ATM11 counterparts permit source-informed ports and asset reuse?
7. Can official Minecraft plus Ravel meet the constrained Mac target?
8. What protocol surface is required for pinned ATM11 server interoperability?
9. What rights, identity, signing, and moderation model should a future registry
   use?

None of these questions blocks M0. Each has an explicit decision gate before it
can affect implementation. Question 2 has a recorded platform-signing constraint
and candidate in section 5.3. Question 9 has a recorded preferred shape and
three candidate mechanisms in section 12.3. Recording a candidate does not
resolve a question.

## 19. Research basis

Primary and project sources used to establish this scope:

- [ATM11 CurseForge project](https://www.curseforge.com/minecraft/modpacks/all-the-mods-11)
- [ATM11 official repository](https://github.com/AllTheMods/ATM-11)
- [NeoForge mod-file and entrypoint documentation](https://docs.neoforged.net/docs/gettingstarted/modfiles)
- [NeoForge event documentation](https://docs.neoforged.net/docs/concepts/events)
- [NeoForge registry documentation](https://docs.neoforged.net/docs/concepts/registries/)
- [NeoForge access-transformer documentation](https://docs.neoforged.net/docs/advanced/accesstransformers/)
- [SpongePowered Mixin](https://github.com/SpongePowered/Mixin)
- [Minecraft EULA](https://www.minecraft.net/en-us/eula)
- [Minecraft Usage Guidelines](https://www.minecraft.net/en-us/usage-guidelines)
- [All the Mods licensing guidance](https://allthemods.github.io/alltheguides/about/license/)
- [CurseForge REST API](https://docs.curseforge.com/rest-api/)
- [Prism Launcher](https://github.com/PrismLauncher/PrismLauncher)
- [Pumpkin](https://github.com/Pumpkin-MC/Pumpkin)
- [Azalea](https://github.com/azalea-rs/azalea)
- [Valence](https://github.com/valence-rs/valence)
- [jni-rs](https://github.com/jni-rs/jni-rs)
- [Oracle Java Virtual Machine Specification](https://docs.oracle.com/javase/specs/jvms/se25/html/index.html)

## 20. Handoff

The scope is ready for research and implementation beginning at M0.

The immediate execution lane is:

1. initialize the public-ready Ravel repository structure;
2. write the pinned fixture manifest and reconstruction design without
   downloading restricted files into source control;
3. establish Prism-based vanilla and official ATM11 baselines on Linux and the
   constrained Mac;
4. turn ADG-1 through ADG-5 into isolated benchmark plans;
5. review M1 evidence before selecting the loader architecture.

No work on ATM11 counterparts begins before the credible loader prototype exits
M3.
