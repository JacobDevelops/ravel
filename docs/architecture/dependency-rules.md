# Dependency rules

Dependencies point from orchestration toward stable contracts, never from
stable contracts toward concrete host internals.

- `ravel-core` owns identity, version, path, side, lifecycle, and diagnostic
  types. It has no loader, SDK, or Minecraft dependency. `RelativePath` is here
  rather than in `ravel-package` because two unrelated layers need a validated
  package-relative path and only one of them reads a manifest: `ravel-data` names
  the path a resource is supplied at and the path a generated file is written to,
  and reaching up for the type would put `ravel-package`'s SHA-256 dependency and
  its `digest` subtree into the dependency closure of every mod. The grammar is
  still `schemas/package.schema.json`'s, and `ravel-package` re-exports the type
  so a manifest field is still declared where a manifest is.
- Package and resolver layers may depend on core and serialization or graph
  libraries. They do not depend on bootstrap, SDK, or a concrete adapter.
- The SDK is a safe author-facing facade. It must not expose bootstrap details,
  generated JVM types, or Minecraft-version internals. Which Ravel crates it
  depends on is not listed here: `tests/conformance/tests/integration/sdk_boundary.rs` pins
  that closure package by package from `Cargo.lock`, so it is the enumeration,
  and a second one in prose is a list that goes stale the next package that adds
  an edge — which is what happened to the list this sentence replaced. Read the
  pinned set there and `crates/ravel-sdk/Cargo.toml` for the direct edges. Each
  edge needs the same kind of reason the test records beside it: an author-facing
  item a mod cannot be written without. `ravel-data` is there because the content
  declaration a mod
  writes — registry entries, tag memberships, supplied resources, and the pack
  they generate — is that crate's model, and `ravel-pack` reads the same
  declarations to check a pack's registry closure and to merge its members' tags;
  a model living in the facade would make a feature crate depend on the facade.
- `ravel-minecraft-generate` is used only by build tooling and depends only on
  `ravel-core`, `serde`, `serde_json`, `sha2`, `toml`, `noak`, `zip` and `proguard`.
  `noak` retains raw JVM descriptors and decodes modified UTF-8; `zip` provides
  archive integrity and bounded DEFLATE reads; `proguard` parses mapping records.
  Artifact admission, descriptor validation and mapping uniqueness remain ours.
  JVM names and descriptor tokens live in its classfile module, shared by source
  rendering, hook-shape classification and tooling inventory counts.
  Xtask consumes typed adapter admission and hook inventory facts instead of
  maintaining a second schema. Its hooks module derives counts from those facts.
  Java-shape checks read version listing and eligibility from version_boundary;
  that dependency runs in one direction only.
  Shared text loading, diagnostic paths and prose counts live in xtask's text
  module, independent of JSON schema checks and milestone accounting rules.
- Version-neutral Minecraft traits are separate from the shared adapter runtime
  and per-version data. M4-W6 discharges this against a second version rather than
  against a promise: `tests/fixtures/public/versions/` holds a synthetic version's
  data, laid out as a drop-in replacement for `minecraft/versions/<version>/`, and
  the shared adapter's generator reads it through the same code path — no branch,
  no fixture-only argument, no crate added. What it generates names that version
  and never the pinned one. Two rules hold the boundary in place: no file under
  `crates/` may spell either version's name or any game class name either
  version's data supplies, in either spelling, which `ravel:verify-scaffold`
  enforces over both directories; and a hook id is the one version-derived name a
  general crate may hold, which is why the fixture reviews the pinned version's
  own id set mapped onto its own classes. That sweep reads `crates/` and nothing
  else, because `crates/` is what a general crate is. `tests/` and `test-mods/`
  are outside it on purpose — a conformance test or a test mod may pin the
  version it was written against — and every violation the sweep reports says so,
  so the exclusion stays a decision rather than a hole. Member names are not
  swept: `width`, `tick` and `flush` are ordinary English, and the class name is
  the long unambiguous thing a general crate has no business spelling. Where an
  occurrence under `crates/` is legitimate, it is listed with its reason in
  `VERSION_CLASS_EXCUSES`, and an entry that stops matching fails the gate as
  stale.
  What is *not* proved is a general crate executing against the second version's
  target table. A build compiles exactly one table, because the generated bindings
  are `include!`d from `OUT_DIR`; separate versions ship separate artifacts rather
  than sharing hook classes or exported symbol sets. So the general crates' claim
  rests on the id set being identical and on their holding no other version-derived
  name, both of which are checked, rather than on a second table being run.
- `ravel-minecraft` is the highest crate that may name a JNI type, and
  `JavaHandle` is the only one it names. A hook argument that refers to a game
  object is a borrowed JNI local reference, and the lifetime that makes it safe
  to hold is `jni`'s rather than one Ravel could invent; modelling it here would
  be a second scheme with the same job and no borrow checker behind it. Nothing
  above this crate names `JObject` — an observer sees a handle's declared class
  and whether it is null, and no JNI type at all. Only `JavaHandle::new` names
  one, and it is public solely because the generated exports that call it are
  compiled into `ravel-bootstrap-runtime`. The SDK does not depend on this crate, so a
  mod reaches it through nothing the SDK offers and needs it for nothing; a change
  that puts it in a mod's graph must seal that constructor rather than move the
  dependency.
- Feature crates for configuration, data, saves, networking, observability, and
  packs depend on public contracts rather than concrete hook implementations.
- `ravel-core` implements the redaction rules `SPEC.md` §9.1 requires.
  `DiagnosticMessage::redacted` earns `Redaction::Applied` only by running those
  rules itself, not by trusting a caller-supplied policy or safety assertion.
  `ravel-observability` reexports the same redactor and credential detector for
  reporting and tracked-text checks. Failures report their own text as
  `Redaction::Raw`; the publication surface applies the shared redactor.
- `ravel-observability` also owns the opt-in profiling coordinator, and owns the
  statement of what it can attribute. `Metric::support` is one table naming, per
  metric, whether a collector measures it exactly, approximately, or not at all,
  and the evidence record, the in-game report, and the tests all read that table
  rather than restating it — so a metric cannot be supported in one of them and
  not the others. The coordinator names no JVM type and no loader type: it is
  told the answer to the opt-in switch rather than reading it, because the switch
  is a JVM system property and this crate is version-neutral.
- Testkit may depend on the loader and public contracts to construct process
  harnesses, but production libraries never depend on testkit. Local fixtures
  are never package or source dependencies.
- Counterpart crates in `registry/mods/*` are members of the root Cargo
  workspace and consume the public SDK, not loader or JVM internals. Colocation
  does not relax the mod-author boundary. Cargo path dependencies stay within
  the checkout; authorized ignored local fixtures remain runtime test inputs,
  never package or source dependencies.

- `ravel-bootstrap-runtime` is the safe JNI integration runtime, built as an rlib and linked
  into a host cdylib. Only host crates (`ravel-host-runtime` and the cdylib hosts)
  may depend on it. The composed host and player-generated hosts own the JVM-loadable
  `ravel_bootstrap` library; the runtime's Rust library is `ravel_bootstrap_runtime`.
  It holds no unsafe code — see [`safety-boundary.md`](safety-boundary.md) —
  and exposes only safe Rust upward. SDK and mod dependency graphs must not reach it.
  The host entries delegate JNI side decoding and redacted exceptions to
  `ravel-host-runtime`; its initialization installs the panic reporter before
  constructing members. Bootstrap owns the installed composition, including the
  admitted package-version map used to check saved-data requirements.

- `ravel-composed-host` is the tracked JVM harness fixture, not a generated
  player host. Its fixed test-mod membership exercises shared admission and
  startup through `ravel-host-runtime`. It remains under `crates/` with the other
  host crates; historical evidence inventories describe their pinned revisions,
  not a placement constraint. Its test-mod dependencies are not suitable for other
  runtime crates. Player hosts are emitted by `ravel-build` with the instance's selected members.

- Loader phase entry points accept borrowed mutable host slices so testkit
  callers and owning hosts use the same dispatch surface. A lifecycle machine
  remains independently constructible for admission and lifecycle tests;
  hosts construct borrow adapters at their ownership boundary.

- `ravel-build` generates a host and links none of it. It depends on
  `ravel-loader`, `ravel-package` and `ravel-core` to read what an instance holds,
  and on nothing below them; the generated workspace hands an installed mod
  `ravel-sdk` and nothing else, so a player's mod reaches no bootstrap, JVM or
  version internal either.

The [mechanisms overview](overview.md#mechanisms) describes the current boundaries:
a mod is distributed as source the player's machine compiles, and every screen
Ravel owns is declared in data and drawn in Java.
