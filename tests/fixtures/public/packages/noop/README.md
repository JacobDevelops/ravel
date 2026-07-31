# No-op package

The prebuilt no-op package M2-W6 drives through the loader pipeline. It is
original Ravel content, holds no third-party code or assets, and names no local
path.

| File | Purpose |
| --- | --- |
| `package.json` | The manifest, validated against [`schemas/package.schema.json`](../../../../../schemas/package.schema.json). |
| `artifacts/noop.artifact` | The declared artifact, integrity-checked and interpreted by nothing. |
| `provenance.json` | The provenance record required by `SPEC.md` §3.4. |

The mod body is [`test-mods/noop`](../../../../../test-mods/noop), a plain Rust
value implementing `ravel_host_api::HostMod`. Nothing in this package selects a
mod execution model: ADG-2 is deferred to M3, the declared artifact role is
`other` rather than `module` or `library`, and `artifacts/noop.artifact` explains
why the file is a placeholder.

The digests are the SHA-256 of the files as committed. Editing either file means
recomputing its digest, and `tests/conformance/tests/noop_pipeline.rs` fails
until you do.
