# Architecture overview

Ravel is a Rust mod loader and SDK hosted initially inside official Minecraft:
Java Edition. Discovery, package validation, dependency resolution, lifecycle
orchestration, version adapters, safe author APIs, registries, configuration,
saved data, diagnostics, profiling, and pack validation are Ravel
responsibilities. Authentication, entitlement, game installation, instance
management, downloads, and general launcher UI remain with Prism Launcher or
their existing owners.

The required lifecycle is:

```text
Discovered -> Validated -> Resolved -> Loaded -> Constructed
  -> Registered -> Running -> Stopping -> Stopped
```

`Failed` is terminal. No mod code runs before validation and resolution,
registry identifiers freeze at the registration barrier, required failures
prevent world access, side-incompatible code never loads, and lifecycle order
is deterministic for an identical package set.

Minecraft-specific hooks, mappings, and bindings live behind an explicit
version adapter. General loader, package, SDK, and pack contracts must not
encode any Minecraft version's internals.

Core owns identifiers, sides and lifecycle vocabulary. Declarative block/item
behaviour belongs to `ravel-data`, with the SDK retaining its author-facing
content reexports; its declaration failure is `ContentDeclarationError`, distinct
from configuration declarations. Lifecycle errors retain typed registry keys.

The build default stays 26.1.2 and 1.21.1 support remains. Forward proof targets
immutable SevTech: Ages 3.2.3 / Minecraft 1.12.2 on the exact JDK 27 pin in
`mise.toml`. It means native behavioural parity through Rust counterparts,
not hosting the release's Forge jars. A descriptor or mapping change does not
prove live 1.12.2 support; the forward WP1–WP9 gates in
[`ROADMAP.md`](../../ROADMAP.md#forward-sevtech-work-packages) separate these stages.

Official-server interoperability is a later protocol layer; it does not turn
Ravel into a Java mod host.

## Mechanisms

- **JVM entry.** A `java.lang.instrument` premain agent rewrites hook targets with
  the runtime's `java.lang.classfile` API and binds callbacks through generated
  JNI-mangled exports; the generated `Hook` class owns native loading and the sole
  native declaration surface.
- **Entry-point initialization.** The agent jar joins the bootstrap loader's
  search path so class loaders share one copy of Ravel's classes.
  The first distribution entry point names the running side and initializes the
  Rust runtime; `premain` does not infer the side.
- **Observation hooks.** Generated hooks return no verdict and do not decide
  whether a game call proceeds. World-open admission uses its own persistence
  crossing; supported legacy semantic refusals use explicit adapter paths.
- **Mod execution.** Mods ship independently as portable Rust source, and a
  Ravel-supplied toolchain compiles the installed set on the player's machine.
  Compiled mod artifacts are not distribution inputs, so Ravel owes no stable mod ABI.
- **Bindings.** Build-time generation resolves each reviewed hook exactly once
  against hash-pinned class files and records the result in `mappings.lock`.
  Existing targets use unobfuscated class files or official ProGuard mappings;
  input drift or a mismatched shape prevents generation. The proposed legacy
  path normalizes pinned Tiny namespaces into the same strict resolver:
  Feather (CC0) is canonical only after official-jar coverage proof, with Legacy
  Yarn fallback and no MCP human-name CSV import. Keep source bytecode level,
  runtime JDK and optional DFU capability distinct; generation is not live support.
- **State ownership.** Minecraft keeps its state; hooks receive values or
  call-scoped opaque `JavaHandle` references and return results without retaining
  a mirror. A mirror needs measured justification and a way to detect divergence;
  batching reduces boundary traffic without duplicating state.
- **Launcher integration.** Prism owns authentication, downloads, Java selection
  and launch; Ravel uses stock instance arguments rather than a launcher fork.
  The generated `Hook` loads the native library with `System.load` from the
  absolute path named by `-Dravel.host`, beside the `-javaagent` argument.
- **Configuration and screens.** Mods declare configuration as data, and Java
  draws every Ravel-owned screen from declarations and values supplied by Rust.
  Values resolve from author defaults through per-setting pack overrides to
  player values, with `locked` or `required` pack values suppressing the player
  layer; the per-frame extraction hook observes rather than draws.
- **World persistence.** Minecraft stores one opaque Rust-owned, owner-sorted
  canonical JSON aggregate in one `SavedData` object, bounded to 1 MiB before
  parsing or replacement, through batched open and save crossings.
  Rust owns admission, migration and staged/live world identity; Java owns game
  calls and vanilla file handling, and a failed replacement preserves the prior
  aggregate without aborting vanilla saving.
  The loader applies each owner's migration chain before calling
  `Mod::open_saved_data`, which receives only the current-version envelope.
  The installed composition supplies package versions for
  required-package admission on real game opens, not only in fixture tests.
  The 1.12.2 port must use legacy WorldSavedData/MapStorage semantics rather than
  claim modern SavedData factories/codecs exist; numeric IDs, metadata and
  global/per-dimension identity require version-local evidence before live closure.
- **Outward actions.** Rust answers an inbound crossing with data describing an
  action, and the receiver-aware Java adapter performs it against the game.
  Rust does not call game methods, and `JavaHandle` exposes no such capability.
- **Runtime ownership.** Bootstrap installation consumes a complete composition,
  launch directory, pre-world report and profiler; no installed runtime lacks
  launch state. Host initialization owns panic-hook installation and the shared
  JNI entry body. The loader applies one owner-attribution operation to callback
  failures across lifecycle driving, saves and bootstrap dispatch. Service,
  resource and configuration entry points use the same owner-participation
  and lifecycle rules; a refusing service provider is an error, not an absent
  optional provider.
- **Diagnostic publication.** `DiagnosticMessage::redacted` applies the shared
  core redactor rather than trusting a caller-supplied policy to assert safety.
  Tracked-text checks import the credential detector directly from core.
- **Claims and checks.** Measured claims have an independent reading proved by
  mutation; judged claims carry per-input verdicts, reasons and bases in data,
  checked for two-way congruence with behaviour, and mechanisms disclose their
  actual bounds rather than asserting completeness.
  From M5 onward, deviations naming detectable Rust symbols carry checked
  caller-scope anchors; this does not turn the whole judgement into a measurement.
- **Evidence classes.** Material digests freeze findings, while machinery
  digests describe the measured tree and can be rebound with an amendment trail;
  later dispositions belong to later records rather than frozen findings.
  Closed M4 inventories resolve their names against the latest ancestor revision
  modifying the inventory record itself, including recorded amendments, rather
  than the manifest's input-baseline revision or today's workspace. Later layout
  changes do not rewrite that inventory. Both jj and Git checkouts resolve this
  provenance. Verification requires complete repository history: shallow Git
  clones and source archives without repository metadata fail explicitly rather
  than substituting the current tree for unreachable inventory history.
  Historical evidence stays frozen under its recorded workload: M0–M5 measured
  ATM11 0.2.1, M6 measured ATM10 To the Sky 2.0.2, and M7 is open.
  Replay uses the recorded fixture, never a blanket ATM10 substitution.
  New workload findings belong to new records. SevTech's 254 manifest references,
  5 bundled jars and 259 total jar inputs are distinct metrics; loaded containers
  remain unknown until observed, never inferred from the input denominator.
  M7 `library-summary` records using protocol `m7-library-v2` require
  `fixture = { id, artifactSet: { count, sha256 } }`; that identity and artifact
  set must match the selected `registry/inventory/<id>/graph.json` fixture.
  SevTech's denominator is 259 jar inputs, not 254 manifest references or 273
  declared identities. Historical `m7-library-v1` rejects the `fixture` field and
  retains its original M5 inventory-summary binding and arithmetic unchanged.
  Protocol choice is explicit: historical open-M7 replay retains v1's original
  criteria; fixture-bound v2 can record new-workload library evidence, but does
  not close historical M7 or forward WP9 by changing the denominator.
  A graph node's `loader: ""` means the artifact declares no loader identifier;
  it does not mean the reference pack is vanilla or authorize hosting its jars.
  Legacy reference loader identity comes from the pinned pack descriptor.
- **Counterpart identity.** Counterparts use their own Ravel-native identity;
  load-time identity mapping belongs to the host.
  A genuine compatibility alias lives only in
  `registry/mods/<package>/aliases.json`, whose rows contain the upstream id,
  native id and canonical replacement id.
- **Toolchain and cache.** Player mod builds share one exactly pinned toolchain
  and one reconstructible Cargo build cache per machine across instances.
  Cargo's cache keys separate different builds, and its lock serializes concurrent
  access; cache size and eviction remain open work.
  Each generated workspace selects the installation's pinned toolchain channel
  with the minimal profile, without developer components or extra targets.
  That generated toolchain file and the installation's `Cargo.lock` participate
  in its build fingerprint.
  Cargo runs from that workspace so rustup selects its pinned compiler.
  Cargo resolves generated local host and mod entries offline; every resolved external name, version, source,
  and checksum must match an installation lock entry before compilation runs
  with `--locked`. An unvetted third-party resolution is refused, not silently
  added to the installation's dependency graph.
  Installed mods may reach only the SDK dependency closure and the explicitly
  supported serde, serde_json and num-traits dependency closures, not every
  tooling or host dependency that happens to occur in the installation lock.
  Source-less packages must be
  planned members, the generated host, or identities already in the installation
  lock. A member's direct path dependencies may reach only the SDK or another
  planned member, not host internals or vendored third-party code. Generated
  SDK-author serde dependencies and host JNI dependencies come from the root manifest.
- **Acceptance runs.** The acceptance model describes structured
  `GameEvent`/`Observation` data by registered content keys and planned
  `Directive` data for the existing outward instruction crossing.
  Verdict arithmetic lives in the recorded `Transcript`; execution and a live
  game driver are not implemented or exposed as placeholder interfaces.
- **Version selection.** One build emits one version's artifacts and reads only
  that version directory; one tracked version defaults automatically, while
  multiple versions require `RAVEL_MINECRAFT_VERSION` and unknown names fail closed.
  The distribution carries artifacts per supported version, and the scaffold
  gate independently sweeps every tracked version directory.
- **Forward platform and renderer.** Own the minimal 1.12.2 runtime cutover:
  pinned JDK27, LWJGL3 window/input/audio/context handling and capability-selected
  GL. Remove the LWJGL2 production path rather than maintaining a general legacy
  mod host. Study Cleanroom/LWJGLX, not wholesale adoption; review exact licences
  before reuse. Java retains context/thread ownership and batched GL submission;
  Rust may own bounded immutable snapshots, mesh jobs and deduplicated geometry,
  not an unmeasured permanent mirror of authoritative game state.
- **Forward parity and rights.** Legacy registration needs an explicit barrier,
  stable numeric IDs/metadata, main-thread bounded packet dispatch and real
  resource/recipe seams, not renamed modern hooks. Every release input requires
  a lawful behavioural or non-behavioural disposition. Rights gates are never
  waived; a critical unresolved input blocks full parity. Live progression,
  saves, rendering and total-resource evidence remain required and undemonstrated
  by this architecture plan.

The original Linux measurements live in
[`evidence/m1/spikes/`](../../evidence/m1/spikes/).
The ADG-1 through ADG-5 candidate sources and `ravel:spike-adg-1` through
`ravel:spike-adg-5` tasks are retired; `cargo make ravel:verify-spikes` still
validates their saved results. The ADG names refer to JVM entry, mod execution,
bindings, state ownership, launcher integration, configuration and screens
(ADG-6), and world persistence (ADG-7). ADG-6 selected its mechanism without
a new measurement. The world-persistence byte-preservation and outward-action
crossing measurement runners are retired; saved evidence remains historical.
A primitive crossing's roughly 10 ns median and zero allocation are a warm,
trivial-call floor, not a frame budget or a prediction for a real hook; see
[`evidence/m1/README.md`](../../evidence/m1/README.md).
No maintained crate holds unsafe code; see
[`safety-boundary.md`](safety-boundary.md).
