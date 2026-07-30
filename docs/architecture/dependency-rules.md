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
- Feature crates for configuration, data, saves, networking, observability, and
  packs depend on public contracts rather than concrete hook implementations.
- Testkit may depend on the loader and public contracts to construct process
  harnesses, but production libraries never depend on testkit. Restricted fixtures
  are never package or source dependencies.

Architecture-gated crates may exist as empty boundaries before a decision, but
production code must not select an ADG-1 through ADG-5 mechanism until its ADR
is accepted.
