# No-op package

The source-only no-op package M2-W6 drives through the loader pipeline. It is
original Ravel content, holds no third-party code or assets, and names no local
path.

| File | Purpose |
| --- | --- |
| `package.json` | The manifest, validated against [`schemas/package.schema.json`](../../../../../schemas/package.schema.json). |
| `Cargo.toml` | The portable Rust source package declaration. |
| `src/lib.rs` | The declared no-op source file, integrity-checked before mod code. |
| `provenance.json` | The provenance record required by `SPEC.md` §3.4. |

The runnable test body remains [`test-mods/noop`](../../../../../test-mods/noop),
a plain Rust value implementing `ravel_sdk::Mod`, exported with `export_mod!`. Ravel uses source
distribution; compilation and binding turn these portable sources into the value
the pipeline drives.

The digests are the SHA-256 of the files as committed. Editing a source or
provenance file means recomputing its manifest digest, and
`tests/conformance/tests/integration/noop_pipeline.rs` fails until you do.
