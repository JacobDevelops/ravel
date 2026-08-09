# Minimal mod example

The smallest complete Ravel mod. It claims one command, runs it, and counts the
server ticks it saw, which is enough to touch every phase of the lifecycle a mod
takes part in.

Read [`src/lib.rs`](src/lib.rs) first. Two things about it are the point:

- `Cargo.toml` declares one dependency, `ravel-sdk`. That is a mod author's whole
  manifest. No bootstrap crate, no adapter, no loader, no `jni`.
- `#[declare_mod(id = "minimal")]` binds the identity the package declares to the
  `impl Mod for` block and generates the identity method. The mod never parses
  its own id and never stores it.

[`src/boundary.rs`](src/boundary.rs) is M4-W1's compile-pass/fail suite. It is
doctests and no code: `cargo test --doc` compiles each case and fails if a
passing one breaks or a failing one starts to compile. The refusals are the
evidence — they are why "an author needs no loader internals and no Java" is a
checked claim rather than a sentence in a document.
`tests/conformance/tests/sdk_boundary.rs` keeps those refusals honest by
recomputing what this crate and `ravel-sdk` are allowed to see.
