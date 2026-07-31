# Dependency rules

Dependencies point from orchestration toward stable contracts, never from
stable contracts toward concrete host internals.

- `ravel-core` owns identity, version, side, lifecycle, and diagnostic types.
  It has no loader, SDK, or Minecraft dependency.
- Package and resolver layers may depend on core and serialization or graph
  libraries. They do not depend on bootstrap, SDK, or a concrete adapter.
- The SDK is a safe author-facing facade. It must not expose bootstrap details,
  generated JVM types, or Minecraft-version internals.
- Version-neutral Minecraft traits are separate from the shared adapter runtime
  and per-version data.
  General crates must remain buildable against a synthetic second adapter.
- `ravel-minecraft` is the highest crate that may name a JNI type, and
  `JavaHandle` is the only one it names. A hook argument that refers to a game
  object is a borrowed JNI local reference, and the lifetime that makes it safe
  to hold is `jni`'s rather than one Ravel could invent; modelling it here would
  be a second scheme with the same job and no borrow checker behind it. Nothing
  above this crate names `JObject` — an observer sees a handle's declared class
  and whether it is null, and no JNI type at all. Only `JavaHandle::new` names
  one, and it is public solely because the generated exports that call it are
  compiled into `ravel-bootstrap`. The SDK does not depend on this crate and a
  mod cannot reach it; a change that makes it mod-reachable must seal that
  constructor rather than move the dependency.
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
- Measurement experiments under `experiments/` are not production crates. No
  production crate depends on one, and `ravel:verify-scaffold` enforces that.

Architecture-gated crates may exist as empty boundaries before a decision, but
production code must not select an ADG-1 through ADG-5 mechanism until its ADR
is accepted. ADG-1, ADG-3, ADG-4 and ADG-5 are accepted; see
[`docs/decisions/`](../decisions/). ADG-2 and ADG-6 are accepted too; a mod is
distributed as source the player's machine compiles, and every screen Ravel owns
is declared in data and drawn in Java.
