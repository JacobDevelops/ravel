# Client-only package

A source-only package whose manifest declares the client side and nothing else.
It exists so that "client-only code never loads on a server side" can be checked
from the thing a mod author actually writes — a `sides` list in a manifest —
rather than from a `LifecycleEntry` a test constructed by hand. It is original
Ravel content, holds no third-party code or assets, and names no local path.

| File | Purpose |
| --- | --- |
| `package.json` | The manifest, validated against [`schemas/package.schema.json`](../../../../../schemas/package.schema.json). |
| `Cargo.toml` | The portable Rust source package declaration. |
| `src/lib.rs` | The declared source file, integrity-checked before mod code. |
| `provenance.json` | The provenance record required by `SPEC.md` §3.4. |

The runnable test body is
[`test-mods/client-only`](../../../../../test-mods/client-only), which declares
the same identity and the same single side.
[`tests/conformance/tests/sides_and_packets.rs`](../../../../../tests/conformance/tests/sides_and_packets.rs)
drives this package through validate, resolve and drive on every side and
requires the server sides to park it.

The digests are the SHA-256 of the files as committed. Editing a source or
provenance file means recomputing its manifest digest, and the conformance suite
fails until you do.
