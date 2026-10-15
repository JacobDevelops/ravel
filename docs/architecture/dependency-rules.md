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
  depends on is not listed here: `tests/conformance/tests/sdk_boundary.rs` pins
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
  are `include!`d from `OUT_DIR` and ADG-3 declined to decide how two hook classes
  and two exported symbol sets would be told apart. So the general crates' claim
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
  compiled into `ravel-bootstrap`. The SDK does not depend on this crate, so a
  mod reaches it through nothing the SDK offers and needs it for nothing; a change
  that puts it in a mod's graph must seal that constructor rather than move the
  dependency.
- Feature crates for configuration, data, saves, networking, observability, and
  packs depend on public contracts rather than concrete hook implementations.
- `ravel-observability` owns the redaction policy `SPEC.md` §9.1 requires, and it
  is the only crate that can produce a `Redaction::Applied` message:
  `ravel-core` names the obligation as `RedactionPolicy` and holds none of the
  rules, and `DiagnosticMessage::redacted` earns the tag by running a policy
  rather than by being told one ran. Every failure in the workspace reports its
  own text as `Redaction::Raw`; the report surface redacts it on the way in.
- `ravel-observability` also owns the opt-in profiling coordinator, and owns the
  statement of what it can attribute. `Metric::support` is one table naming, per
  metric, whether a collector measures it exactly, approximately, or not at all,
  and the evidence record, the in-game report, and the tests all read that table
  rather than restating it — so a metric cannot be supported in one of them and
  not the others. The coordinator names no JVM type and no loader type: it is
  told the answer to the opt-in switch rather than reading it, because the switch
  is a JVM system property and this crate is version-neutral.
- Testkit may depend on the loader and public contracts to construct process
  harnesses, but production libraries never depend on testkit. Restricted fixtures
  are never package or source dependencies.

- `ravel-bootstrap` is the JVM entry crate. It holds no unsafe code — see
  [`safety-boundary.md`](safety-boundary.md) — it exposes only safe Rust upward, and no other
  maintained crate depends on it. The SDK never re-exports it, and a mod cannot
  reach it.

- **Two crates contradict the sentence above and this record does not resolve
  it.** `ravel-composed-host` has depended on `ravel-bootstrap` since it was
  written, and `ravel-host-runtime` — the composition both hosts run — does now.
  Both are hosts: a host is the binary the JVM loads, which is the one place a
  JVM entry belongs, and neither is reachable from a mod. So the rule as written
  is either about *non-host* crates and has always been stated too widely, or the
  two hosts are a defect. Which of those it is decides whether a third host may
  exist, and that is an architecture decision rather than an implementation one.
  It is recorded here rather than edited away.

- `ravel-build` generates a host and links none of it. It depends on
  `ravel-loader`, `ravel-package` and `ravel-core` to read what an instance holds,
  and on nothing below them; the generated workspace hands an installed mod
  `ravel-sdk` and nothing else, so a player's mod reaches no bootstrap, JVM or
  version internal either.
- Measurement experiments under `experiments/` are not production crates. No
  production crate depends on one, and `ravel:verify-scaffold` enforces that.

Architecture-gated crates may exist as empty boundaries before a decision, but
production code must not select an ADG-1 through ADG-5 mechanism until its ADR
is accepted. ADG-1, ADG-3, ADG-4 and ADG-5 are accepted; see
[`docs/decisions/`](../decisions/). ADG-2 and ADG-6 are accepted too; a mod is
distributed as source the player's machine compiles, and every screen Ravel owns
is declared in data and drawn in Java.
